// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! [`Engine`]: one open workspace in one process, and the pipeline every
//! operation runs through.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use gk_api::ops::common::Via;
use gk_api::ops::history::RevertResult;
use gk_api::{Changed, ErrorCode, OpDescriptor, OpError, OpKind, Operation, Profile, Safety};
use gk_core::Id;
use gk_store::rusqlite::params;
use gk_store::{
    CheckpointOutcome, CheckpointPolicy, CheckpointSchedule, Checkpointer, LockFile,
    SCHEMA_VERSION, Store, StoreConfig, WatcherHandle, WorkspaceLayout,
};
use serde_json::{Value, json};

use crate::context::{ConfirmRequest, Confirmation, Context};
use crate::events::{self, Bus};
use crate::failure::{Failure, store_error};
use crate::ops::{history as history_ops, settings};
use crate::registry::{CommandFn, Entry, Env, Handler, QueryCtx, QueryFn, Registry};
use crate::time::{Clock, system_clock};
use crate::write::{CommandMeta, WriteCtx};

/// Which program opens the workspace. It decides the locks the engine
/// takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The desktop app: holds the app lock (one app per workspace) and
    /// tries for the jobs lock.
    App,
    /// A one-shot `glyphkeel` command: no long-held lock.
    Cli,
    /// `glyphkeel mcp`: tries for the jobs lock when no app holds it.
    Mcp,
}

/// How the engine opens a workspace.
#[derive(Clone)]
pub struct EngineConfig {
    /// The program opening it.
    pub role: Role,
    /// The storage settings.
    pub store: StoreConfig,
    /// The program's version, as `workspace.get` reports it.
    pub program_version: String,
    /// The clock.
    pub clock: Clock,
    /// The handlers; [`Registry::standard`] unless a test adds its own.
    pub registry: Registry,
    /// When the write-ahead log is checkpointed by the jobs-lock holder.
    pub checkpoints: CheckpointPolicy,
}

impl EngineConfig {
    /// The defaults for `role`.
    #[must_use]
    pub fn new(role: Role) -> Self {
        Self {
            role,
            store: StoreConfig::default(),
            program_version: env!("CARGO_PKG_VERSION").to_owned(),
            clock: system_clock(),
            registry: Registry::standard(),
            checkpoints: CheckpointPolicy::default(),
        }
    }
}

impl std::fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineConfig")
            .field("role", &self.role)
            .field("store", &self.store)
            .field("program_version", &self.program_version)
            .finish_non_exhaustive()
    }
}

/// How long an answer to a repeated `client_request_id` is kept.
const IDEMPOTENCY_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
/// How many groups the app's Undo stack keeps.
const UNDO_DEPTH: usize = 200;

/// What an agent session records when it starts.
#[derive(Debug, Clone)]
pub struct SessionStart {
    /// The agent's display name.
    pub name: String,
    /// The client's own ID, if it sent one.
    pub client_id: Option<String>,
    /// The client's version, if it sent one.
    pub client_version: Option<String>,
    /// The profile it asked for.
    pub profile: Profile,
}

/// One open workspace. Every operation of every adapter runs through
/// [`Engine::call`]:
///
/// 1. find the operation; refuse when a newer program upgraded the schema;
/// 2. decode the input (`invalid` with the JSON path);
/// 3. authorize: app-only operations, the agent's profile capped by the
///    workspace setting, the confirmation of permanent operations;
/// 4. queries run in one read transaction; commands in one write
///    transaction with their history rows, under a group registered before
///    the commit;
/// 5. after the commit, publish one change event and, for the app, push
///    the group onto the Undo stack.
pub struct Engine {
    store: Store,
    env: Env,
    role: Role,
    clock: Clock,
    registry: Registry,
    bus: Arc<Bus>,
    /// Held across a command's commit and its event, so events leave in
    /// commit order.
    commit: Mutex<()>,
    upgraded: AtomicBool,
    undo: Mutex<UndoStacks>,
    idempotency: Mutex<HashMap<String, (i64, Value)>>,
    pending_reads: Mutex<HashMap<Id, i64>>,
    jobs: Mutex<Jobs>,
    _app_lock: Option<LockFile>,
}

#[derive(Debug, Default)]
struct UndoStacks {
    undo: Vec<Id>,
    redo: Vec<Id>,
}

struct Jobs {
    lock: Option<LockFile>,
    checkpoints: Option<(Checkpointer, CheckpointSchedule)>,
    policy: CheckpointPolicy,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("workspace", &self.env.name)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Opens the workspace at `layout`, creating it if it doesn't exist,
    /// and takes the locks of `config.role`.
    pub fn open(layout: &WorkspaceLayout, config: EngineConfig) -> Result<Self, OpError> {
        layout.create_dirs().map_err(|e| store_error(&e))?;
        let app_lock = if config.role == Role::App {
            let lock = LockFile::try_acquire(&layout.app_lock()).map_err(|e| store_error(&e))?;
            Some(lock.ok_or_else(|| {
                OpError::new(
                    ErrorCode::Busy,
                    "Another Glyphkeel app already has this workspace open.",
                )
                .with_hint("Switch to the open window.")
            })?)
        } else {
            None
        };
        let store = Store::open(layout, config.store.clone()).map_err(|e| store_error(&e))?;
        let jobs_lock = if config.role == Role::Cli {
            None
        } else {
            LockFile::try_acquire(&layout.jobs_lock()).map_err(|e| store_error(&e))?
        };
        let name = layout
            .root()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let env = Env {
            workspace_id: store.workspace_id(),
            name,
            program_version: config.program_version,
            schema_version: SCHEMA_VERSION,
        };
        Ok(Self {
            store,
            env,
            role: config.role,
            clock: config.clock,
            registry: config.registry,
            bus: Arc::new(Bus::default()),
            commit: Mutex::new(()),
            upgraded: AtomicBool::new(false),
            undo: Mutex::new(UndoStacks::default()),
            idempotency: Mutex::new(HashMap::new()),
            pending_reads: Mutex::new(HashMap::new()),
            jobs: Mutex::new(Jobs {
                lock: jobs_lock,
                checkpoints: None,
                policy: config.checkpoints,
            }),
            _app_lock: app_lock,
        })
    }

    /// The storage layer, for diagnostics and tests.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The open workspace's facts.
    #[must_use]
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Whether a newer program upgraded the schema while this one ran. The
    /// agent server exits when it sees this; every later call fails with
    /// `workspace_upgraded`.
    #[must_use]
    pub fn must_exit(&self) -> bool {
        self.upgraded.load(Ordering::Acquire)
    }

    /// A receiver of every change event from now on: this process's
    /// commands, and other processes' once [`Engine::watch_other_processes`]
    /// runs.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Changed> {
        self.bus.subscribe()
    }

    /// Runs operation `op` with JSON `input` for `ctx`. `null` input counts
    /// as `{}`.
    pub fn call(&self, ctx: &Context, op: &str, input: Value) -> Result<Value, OpError> {
        let entry = self.registry.get(op).ok_or_else(|| {
            let names = self.registry.names();
            OpError::not_found(format!("There is no operation \"{op}\"."), &names)
        })?;
        let result = self.call_entry(ctx, entry, input);
        if let Err(e) = &result
            && e.code == ErrorCode::WorkspaceUpgraded
        {
            self.upgraded.store(true, Ordering::Release);
        }
        result
    }

    /// Runs operation `O` with typed input and output.
    pub fn run<O: Operation>(&self, ctx: &Context, input: &O::Input) -> Result<O::Output, OpError> {
        let input = serde_json::to_value(input)
            .map_err(|e| OpError::invalid(format!("The input could not be encoded: {e}.")))?;
        let output = self.call(ctx, O::ID, input)?;
        serde_json::from_value(output).map_err(|e| {
            OpError::storage(format!("The result of {} could not be read: {e}.", O::ID))
        })
    }

    fn call_entry(&self, ctx: &Context, entry: &Entry, input: Value) -> Result<Value, OpError> {
        if self.must_exit() {
            return Err(OpError::new(
                ErrorCode::WorkspaceUpgraded,
                "A newer Glyphkeel upgraded this workspace while this program was running.",
            )
            .with_hint("Restart the program."));
        }
        let input = if input.is_null() { json!({}) } else { input };
        (entry.validate)(&input)?;
        let profile = self.authorize(ctx, entry.desc)?;
        self.confirm(ctx, entry.desc, &input)?;

        let key = idempotency_key(entry.desc, ctx, &input);
        if let Some(key) = &key
            && let Some(answer) = self.remembered(key)
        {
            return Ok(answer);
        }
        let output = match &entry.handler {
            Handler::Query(f) => self.run_query(ctx, profile, f, input),
            Handler::Command(f) => self
                .execute(ctx, entry.desc.id, Arc::clone(f), input)
                .map(|(output, _)| output),
            Handler::Engine(f) => f(self, ctx, input),
        }?;
        if let Some(key) = key {
            self.remember(key, &output);
        }
        Ok(output)
    }

    /// Who may run `desc`, and the agent's effective profile.
    fn authorize(&self, ctx: &Context, desc: &OpDescriptor) -> Result<Option<Profile>, OpError> {
        if desc.is_app_only() {
            let reachable = match ctx.via {
                Via::App => !ctx.is_agent(),
                Via::Cli => !ctx.is_agent() && desc.cli.is_some(),
                Via::Mcp => false,
            };
            if !reachable {
                return Err(OpError::forbidden(format!(
                    "{} is available in the app only.",
                    desc.id
                )));
            }
        }
        if !ctx.is_agent() {
            return Ok(None);
        }
        let requested = ctx.profile.unwrap_or(Profile::Standard);
        let cap = self
            .store
            .read(|tx| settings::load(tx).map_err(Failure::from))
            .map_err(OpError::from)?
            .agent_max_profile;
        let profile = requested.capped_by(cap);
        if profile.allows(desc.safety) {
            Ok(Some(profile))
        } else {
            Err(OpError::forbidden(format!(
                "Your profile ({profile}) doesn't allow {} operations such as {}.",
                desc.safety.as_str(),
                desc.id
            ))
            .with_hint(if profile < cap {
                "Ask the person to start the agent server with a higher --profile."
            } else {
                "The workspace setting \"most an agent may do\" doesn't allow it; ask the person."
            })
            .with_detail("profile", profile.as_str()))
        }
    }

    /// Permanent operations by the person need a confirmation; agents are
    /// governed by their profile instead.
    fn confirm(
        &self,
        ctx: &Context,
        desc: &'static OpDescriptor,
        input: &Value,
    ) -> Result<(), OpError> {
        if desc.safety != Safety::Permanent || ctx.is_agent() {
            return Ok(());
        }
        let confirmed = match &ctx.confirm {
            Confirmation::Refuse => false,
            Confirmation::Given => true,
            Confirmation::Ask(ask) => ask(&ConfirmRequest {
                op: desc,
                input: input.clone(),
            }),
        };
        if confirmed {
            Ok(())
        } else {
            Err(OpError::forbidden(format!(
                "{} can't be undone and was not confirmed.",
                desc.id
            ))
            .with_hint("Confirm it in the dialog, or pass --yes on the command line."))
        }
    }

    fn run_query(
        &self,
        ctx: &Context,
        profile: Option<Profile>,
        f: &QueryFn,
        input: Value,
    ) -> Result<Value, OpError> {
        let output = self.store.read(|tx| {
            f(
                &QueryCtx {
                    tx,
                    caller: ctx,
                    profile,
                    env: &self.env,
                },
                input,
            )
        })?;
        if let Some(session) = ctx.session {
            *self.lock_reads().entry(session).or_insert(0) += 1;
        }
        Ok(output)
    }

    /// Runs one command: one write transaction with its history rows, then
    /// the change event. Returns the output and the group, when something
    /// changed.
    pub(crate) fn execute(
        &self,
        ctx: &Context,
        op: &'static str,
        f: CommandFn,
        input: Value,
    ) -> Result<(Value, Option<Id>), OpError> {
        let at = (self.clock)();
        let group = Id::generate_at(u64::try_from(at).unwrap_or(0));
        let meta = CommandMeta {
            op,
            group,
            at,
            ctx: ctx.clone(),
            workspace_id: self.env.workspace_id,
        };
        let reads = ctx
            .session
            .and_then(|s| self.lock_reads().remove(&s))
            .unwrap_or(0);

        let order = self.commit.lock().unwrap_or_else(PoisonError::into_inner);
        // Registered before the commit, so the change watcher never reports
        // this process's own command as another program's.
        self.store.own_groups().insert(group);
        let in_tx = meta.clone();
        let result = self.store.write(op, move |tx| {
            let mut w = WriteCtx::new(tx, &in_tx);
            let output = f(&mut w, input.clone())?;
            let finished = w.finish()?;
            if let (Some(session), true) = (in_tx.ctx.session, reads > 0) {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2, last_seen_at = ?3 WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads, in_tx.at],
                )?;
            }
            Ok::<_, Failure>((output, finished))
        });
        let (output, finished) = match result {
            Ok(done) => done,
            Err(e) => {
                if let (Some(session), true) = (ctx.session, reads > 0) {
                    *self.lock_reads().entry(session).or_insert(0) += reads;
                }
                return Err(e.into());
            }
        };
        let Some(event) = finished.event(&meta) else {
            return Ok((output, None));
        };
        self.bus.publish(&event);
        drop(order);
        if ctx.via == Via::App && !ctx.is_agent() && !op.starts_with("history.") {
            let mut stacks = self.lock_undo();
            stacks.undo.push(group);
            if stacks.undo.len() > UNDO_DEPTH {
                stacks.undo.remove(0);
            }
            stacks.redo.clear();
        }
        Ok((output, Some(group)))
    }

    /// Reverts `groups` (newest first) as one command named `op`.
    pub(crate) fn revert(
        &self,
        ctx: &Context,
        op: &'static str,
        groups: Vec<Id>,
        dry_run: bool,
    ) -> Result<RevertResult, OpError> {
        let f: CommandFn = Arc::new(move |w, _| {
            let result = history_ops::revert_groups(w, &groups, dry_run)?;
            serde_json::to_value(result).map_err(|e| Failure::Op(OpError::storage(e.to_string())))
        });
        let (output, _) = self.execute(ctx, op, f, Value::Null)?;
        serde_json::from_value(output).map_err(|e| OpError::storage(e.to_string()))
    }

    /// Takes the last group off the Undo stack (or the Redo stack) and
    /// reverts it. A group that something else changed since is refused
    /// with the reason and dropped from the stack.
    pub(crate) fn undo_or_redo(
        &self,
        ctx: &Context,
        redo: bool,
    ) -> Result<(Id, Id, String), OpError> {
        let op = if redo { "history.redo" } else { "history.undo" };
        let target = {
            let mut stacks = self.lock_undo();
            let stack = if redo {
                &mut stacks.redo
            } else {
                &mut stacks.undo
            };
            stack.pop()
        }
        .ok_or_else(|| {
            OpError::not_found(
                if redo {
                    "Nothing to redo."
                } else {
                    "Nothing to undo."
                },
                &[],
            )
        })?;
        let original_op = self
            .store
            .read(|tx| history_ops::group_op(tx, target).map_err(Failure::from))
            .map_err(OpError::from)?
            .unwrap_or_default();
        let result = self.revert(ctx, op, vec![target], false)?;
        if let Some(skipped) = result.skipped.first() {
            return Err(OpError::conflict(format!(
                "Can't {}: {}",
                if redo { "redo" } else { "undo" },
                skipped.reason
            ))
            .with_detail("group", target.to_string()));
        }
        let group: Id = result
            .group
            .as_deref()
            .and_then(|g| g.parse().ok())
            .ok_or_else(|| OpError::storage("The revert recorded no history group."))?;
        let mut stacks = self.lock_undo();
        if redo {
            stacks.undo.push(group);
        } else {
            stacks.redo.push(group);
        }
        Ok((target, group, original_op))
    }

    /// Starts an agent session and returns its ID.
    pub fn start_session(&self, start: &SessionStart) -> Result<Id, OpError> {
        let at = (self.clock)();
        let id = Id::generate_at(u64::try_from(at).unwrap_or(0));
        let start = start.clone();
        let pid = i64::from(std::process::id());
        self.store
            .write("agent.session_start", move |tx| {
                tx.execute(
                    "INSERT INTO agent_sessions (id, client_id, client_version, display_name,
                                                 profile, pid, started_at, last_seen_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                    params![
                        id.as_bytes().as_slice(),
                        start.client_id,
                        start.client_version,
                        start.name,
                        start.profile.as_str(),
                        pid,
                        at
                    ],
                )?;
                Ok::<_, Failure>(())
            })
            .map_err(OpError::from)?;
        Ok(id)
    }

    /// Ends an agent session: records the end time and the reads not yet
    /// counted.
    pub fn end_session(&self, session: Id) -> Result<(), OpError> {
        let at = (self.clock)();
        let reads = self.lock_reads().remove(&session).unwrap_or(0);
        self.store
            .write("agent.session_end", move |tx| {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2, ended_at = ?3, last_seen_at = ?3
                      WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads, at],
                )?;
                Ok::<_, Failure>(())
            })
            .map_err(OpError::from)
    }

    /// Publishes other processes' commits as change events, polling every
    /// `interval`, until the handle is dropped.
    pub fn watch_other_processes(&self, interval: Duration) -> Result<WatcherHandle, OpError> {
        let bus = Arc::clone(&self.bus);
        self.store
            .watch_changes(None)
            .and_then(|w| {
                w.spawn(interval, move |batch| match batch {
                    Ok(batch) => {
                        for event in events::from_records(&batch.records) {
                            bus.publish(&event);
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "reading other programs' changes failed"),
                })
            })
            .map_err(|e| store_error(&e))
    }

    /// Whether this process holds the jobs lock (checkpoints, backups,
    /// purging the trash).
    #[must_use]
    pub fn holds_jobs_lock(&self) -> bool {
        self.lock_jobs().lock.is_some()
    }

    /// One round of background work, called every few seconds by
    /// long-running programs: takes the jobs lock when it is free (another
    /// holder may have ended) and, while holding it, checkpoints the
    /// write-ahead log when it is due.
    pub fn jobs_tick(&self) -> Result<Option<CheckpointOutcome>, OpError> {
        if self.role == Role::Cli {
            return Ok(None);
        }
        let mut jobs = self.lock_jobs();
        if jobs.lock.is_none() {
            jobs.lock = LockFile::try_acquire(&self.store.layout().jobs_lock())
                .map_err(|e| store_error(&e))?;
            if jobs.lock.is_none() {
                return Ok(None);
            }
        }
        if jobs.checkpoints.is_none() {
            let checkpointer = self.store.checkpointer().map_err(|e| store_error(&e))?;
            jobs.checkpoints = Some((checkpointer, CheckpointSchedule::new(jobs.policy)));
        }
        let idle_for = self
            .store
            .writer_stats()
            .last_commit_at
            .map_or(Duration::MAX, |t| t.elapsed());
        let Some((checkpointer, schedule)) = jobs.checkpoints.as_mut() else {
            return Ok(None);
        };
        schedule
            .tick(Instant::now(), idle_for, checkpointer)
            .map_err(|e| store_error(&e))
    }

    /// Closes the workspace: counts the open sessions' reads, checkpoints
    /// the log if this process holds the jobs lock, and stops the writer.
    pub fn close(self) {
        let sessions: Vec<Id> = self.lock_reads().keys().copied().collect();
        for session in sessions {
            let reads = self.lock_reads().remove(&session).unwrap_or(0);
            let _ = self.store.write("agent.session_reads", move |tx| {
                tx.execute(
                    "UPDATE agent_sessions SET reads = reads + ?2 WHERE id = ?1",
                    params![session.as_bytes().as_slice(), reads],
                )?;
                Ok::<_, Failure>(())
            });
        }
        let jobs = self
            .jobs
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        if jobs.lock.is_some()
            && let Ok(checkpointer) = self.store.checkpointer()
            && let Err(e) = checkpointer.truncate()
        {
            tracing::debug!(error = %e, "checkpoint on close failed");
        }
        self.store.close();
    }

    fn remembered(&self, key: &str) -> Option<Value> {
        let now = (self.clock)();
        let map = self
            .idempotency
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        map.get(key)
            .filter(|(at, _)| now - at < IDEMPOTENCY_WINDOW_MS)
            .map(|(_, v)| v.clone())
    }

    fn remember(&self, key: String, output: &Value) {
        let now = (self.clock)();
        let mut map = self
            .idempotency
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        map.retain(|_, (at, _)| now - *at < IDEMPOTENCY_WINDOW_MS);
        map.insert(key, (now, output.clone()));
    }

    fn lock_reads(&self) -> MutexGuard<'_, HashMap<Id, i64>> {
        self.pending_reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_undo(&self) -> MutexGuard<'_, UndoStacks> {
        self.undo.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_jobs(&self) -> MutexGuard<'_, Jobs> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Commands that carry a `client_request_id` answer a repeat of the same ID
/// by the same caller with the first answer instead of running again.
fn idempotency_key(desc: &OpDescriptor, ctx: &Context, input: &Value) -> Option<String> {
    if desc.kind != OpKind::Command {
        return None;
    }
    let id = input.get("client_request_id")?.as_str()?;
    let caller = ctx.session.map_or_else(
        || format!("{}:{}", ctx.via.as_str(), ctx.actor.name),
        |s| s.to_string(),
    );
    Some(format!("{}\u{1f}{caller}\u{1f}{id}", desc.id))
}
