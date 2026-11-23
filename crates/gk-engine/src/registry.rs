// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The handler of each operation, and how JSON input reaches it.

use std::collections::HashMap;
use std::sync::Arc;

use gk_api::{OpDescriptor, OpError, OpKind, Operation, Profile};
use gk_store::rusqlite::Transaction;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::write::WriteCtx;

/// Facts about the open workspace that handlers may need.
#[derive(Debug, Clone)]
pub struct Env {
    /// The workspace's ID.
    pub workspace_id: gk_core::Id,
    /// Its name (the folder name).
    pub name: String,
    /// The program's version.
    pub program_version: String,
    /// The schema version the program was built for.
    pub schema_version: i64,
    /// The workspace's folder, for its stored files.
    pub layout: gk_store::WorkspaceLayout,
    /// The time zone of local days and times of day.
    pub(crate) zone: crate::time::Zone,
}

/// A query's view of the workspace: one read transaction, one snapshot.
#[derive(Debug)]
pub struct QueryCtx<'a> {
    /// The read transaction.
    pub tx: &'a Transaction<'a>,
    /// The caller.
    pub caller: &'a Context,
    /// The agent's profile after the workspace's cap; `None` for the person.
    pub profile: Option<Profile>,
    /// The open workspace.
    pub env: &'a Env,
    /// The query's time, in milliseconds since 1970 UTC.
    pub now: i64,
}

pub(crate) type QueryFn = Arc<dyn Fn(&QueryCtx<'_>, Value) -> Result<Value, Failure> + Send + Sync>;
pub(crate) type CommandFn =
    Arc<dyn Fn(&mut WriteCtx<'_>, Value) -> Result<Value, Failure> + Send + Sync>;
pub(crate) type EngineFn =
    Arc<dyn Fn(&Engine, &Context, Value) -> Result<Value, OpError> + Send + Sync>;

/// How an operation runs.
#[derive(Clone)]
pub(crate) enum Handler {
    /// On a read connection.
    Query(QueryFn),
    /// In one write transaction, with history and a change event.
    Command(CommandFn),
    /// Through the engine itself, for operations that use its state (the
    /// Undo stack) or run commands of their own (revert).
    Engine(EngineFn),
}

/// One registered operation.
#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) desc: &'static OpDescriptor,
    pub(crate) validate: fn(&Value) -> Result<(), OpError>,
    pub(crate) handler: Handler,
    /// For an operation that runs several catalog operations as one
    /// change: the ones a given input asks for, each checked against its
    /// own safety class.
    pub(crate) parts: Option<PartsFn>,
}

/// The catalog operations one input of a combined operation runs.
pub(crate) type PartsFn = fn(&Value) -> Vec<&'static str>;

/// The handlers the engine dispatches to, by operation name.
#[derive(Clone, Default)]
pub struct Registry {
    entries: HashMap<&'static str, Entry>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<&&str> = self.entries.keys().collect();
        names.sort();
        f.debug_struct("Registry").field("ops", &names).finish()
    }
}

impl Registry {
    /// A registry with every catalog operation's handler.
    #[must_use]
    pub fn standard() -> Self {
        let mut r = Self::default();
        crate::ops::register(&mut r);
        r
    }

    /// Registers a query. `desc` is normally the catalog's descriptor of
    /// `O`; tests may pass their own.
    ///
    /// # Panics
    /// If `desc` is not a query.
    pub fn query<O, F>(&mut self, desc: &'static OpDescriptor, f: F)
    where
        O: Operation,
        F: Fn(&QueryCtx<'_>, O::Input) -> Result<O::Output, Failure> + Send + Sync + 'static,
    {
        assert_eq!(desc.kind, OpKind::Query, "{} is not a query", desc.id);
        let handler: QueryFn = Arc::new(move |q, input| encode(&f(q, decode(input)?)?));
        self.insert::<O>(desc, Handler::Query(handler));
    }

    /// Registers a command. The handler may run more than once when another
    /// program holds the lock, so it must have no effects outside the
    /// transaction.
    ///
    /// # Panics
    /// If `desc` is not a command.
    pub fn command<O, F>(&mut self, desc: &'static OpDescriptor, f: F)
    where
        O: Operation,
        F: Fn(&mut WriteCtx<'_>, O::Input) -> Result<O::Output, Failure> + Send + Sync + 'static,
    {
        assert_eq!(desc.kind, OpKind::Command, "{} is not a command", desc.id);
        let handler: CommandFn = Arc::new(move |w, input| encode(&f(w, decode(input)?)?));
        self.insert::<O>(desc, Handler::Command(handler));
    }

    /// Registers a command that runs several catalog operations as one
    /// change; `parts` names the ones an input asks for, so each is
    /// authorized (and, when permanent, confirmed) by its own class.
    pub(crate) fn combined<O, F>(&mut self, desc: &'static OpDescriptor, parts: PartsFn, f: F)
    where
        O: Operation,
        F: Fn(&mut WriteCtx<'_>, O::Input) -> Result<O::Output, Failure> + Send + Sync + 'static,
    {
        self.command::<O, F>(desc, f);
        if let Some(entry) = self.entries.get_mut(desc.id) {
            entry.parts = Some(parts);
        }
    }

    pub(crate) fn engine_op<O, F>(&mut self, desc: &'static OpDescriptor, f: F)
    where
        O: Operation,
        F: Fn(&Engine, &Context, O::Input) -> Result<O::Output, OpError> + Send + Sync + 'static,
    {
        let handler: EngineFn = Arc::new(move |e, ctx, input| {
            let output = f(e, ctx, decode(input).map_err(OpError::from)?)?;
            encode(&output).map_err(OpError::from)
        });
        self.insert::<O>(desc, Handler::Engine(handler));
    }

    fn insert<O: Operation>(&mut self, desc: &'static OpDescriptor, handler: Handler) {
        assert_eq!(desc.id, O::ID, "descriptor and operation differ");
        self.entries.insert(
            desc.id,
            Entry {
                desc,
                validate: validate::<O::Input>,
                handler,
                parts: None,
            },
        );
    }

    pub(crate) fn get(&self, op: &str) -> Option<&Entry> {
        self.entries.get(op)
    }

    /// Whether `op` has a handler.
    #[must_use]
    pub fn contains(&self, op: &str) -> bool {
        self.entries.contains_key(op)
    }

    /// The names of the registered operations, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.entries.keys().copied().collect();
        names.sort_unstable();
        names
    }
}

/// Decodes an operation's input; a bad field becomes `invalid` with its
/// JSON path.
pub(crate) fn decode<T: DeserializeOwned>(input: Value) -> Result<T, Failure> {
    serde_path_to_error::deserialize::<_, T>(input).map_err(|e| {
        let path = e.path().to_string();
        let reason = e.into_inner().to_string();
        let error = if path == "." {
            OpError::invalid(format!("Invalid input: {reason}."))
        } else {
            OpError::invalid_at(
                path.clone(),
                format!("Invalid input at `{path}`: {reason}."),
            )
        };
        Failure::Op(error.with_hint("See the operation's input schema."))
    })
}

fn validate<T: DeserializeOwned>(input: &Value) -> Result<(), OpError> {
    decode::<T>(input.clone()).map(drop).map_err(OpError::from)
}

fn encode<T: Serialize>(output: &T) -> Result<Value, Failure> {
    serde_json::to_value(output).map_err(|e| {
        Failure::Op(OpError::storage(format!(
            "The result could not be written as JSON: {e}."
        )))
    })
}
