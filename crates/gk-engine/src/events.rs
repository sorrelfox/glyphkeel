// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Change events: published after every commit of this process, and for
//! commits of other processes found by the change watcher.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, PoisonError};

use gk_api::ops::common::{Actor, ActorKind, Via};
use gk_api::{Changed, EntityChange, Origin};
use gk_store::ActivityRecord;
use serde_json::Value;

/// Hands every event to every subscriber, in the order they were
/// published.
#[derive(Debug, Default)]
pub(crate) struct Bus {
    subscribers: Mutex<Vec<Sender<Changed>>>,
}

impl Bus {
    pub(crate) fn subscribe(&self) -> Receiver<Changed> {
        let (tx, rx) = mpsc::channel();
        self.lock().push(tx);
        rx
    }

    /// Publishes under the lock, so events leave in the order they come in.
    /// Subscribers that went away are dropped.
    pub(crate) fn publish(&self, event: &Changed) {
        self.lock().retain(|s| s.send(event.clone()).is_ok());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Sender<Changed>>> {
        self.subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Turns history rows from other processes into events, one per group, in
/// sequence order.
pub(crate) fn from_records(records: &[ActivityRecord]) -> Vec<Changed> {
    let mut order: Vec<gk_core::Id> = Vec::new();
    let mut groups: BTreeMap<gk_core::Id, Changed> = BTreeMap::new();
    for r in records {
        let entry = groups.entry(r.group_id).or_insert_with(|| {
            order.push(r.group_id);
            Changed {
                seq: r.seq,
                group: r.group_id.to_string(),
                origin: Origin {
                    via: parse_via(&r.via),
                    actor: Actor {
                        kind: ActorKind::from_stored(&r.actor_kind),
                        name: r.actor_name.clone(),
                    },
                    session: r.session_id.map(|s| s.to_string()),
                },
                changes: Vec::new(),
            }
        });
        entry.seq = entry.seq.max(r.seq);
        entry.changes.push(EntityChange {
            entity: r.entity_kind.clone(),
            id: r.entity_id.to_string(),
            project: r.project_id.map(|p| p.to_string()),
            fields: field_names(&r.changes),
        });
    }
    order
        .into_iter()
        .filter_map(|g| groups.remove(&g))
        .collect()
}

fn parse_via(s: &str) -> Via {
    match s {
        "cli" => Via::Cli,
        "mcp" => Via::Mcp,
        _ => Via::App,
    }
}

/// The field names of a history row's `changes` column.
pub(crate) fn field_names(changes: &str) -> Vec<String> {
    serde_json::from_str::<Value>(changes)
        .ok()
        .and_then(|v| {
            v.get("fields")
                .and_then(Value::as_object)
                .map(|f| f.keys().cloned().collect())
        })
        .unwrap_or_default()
}
