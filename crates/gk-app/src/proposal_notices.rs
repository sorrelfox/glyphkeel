// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A desktop notification when agents propose changes that wait for the
//! person's approval, made by this app's agents or by `glyphkeel mcp`
//! (whose commits the app's change watcher turns into events too).
//!
//! A burst of proposals shows as one notification. While the app is locked
//! it names no agent, as reminders name no task.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gk_api::Changed;

use crate::messages::text;
use crate::reminders::{Notice, Notifier};

/// How long to wait for more proposals before showing one notification.
const BURST: Duration = Duration::from_millis(800);

/// The kind of history row a proposal writes, and the field only a new
/// proposal's row has.
const ENTITY: &str = "proposal";
const NEW_FIELD: &str = "op";

/// How many new proposals `event` holds.
fn proposed(event: &Changed) -> usize {
    event
        .changes
        .iter()
        .filter(|c| c.entity == ENTITY && c.fields.iter().any(|f| f == NEW_FIELD))
        .count()
}

/// The notification for `count` new proposals from `agents`.
fn notice(count: usize, agents: &BTreeSet<String>) -> Notice {
    let count = count.to_string();
    let body = match agents.iter().next() {
        Some(agent) if agents.len() == 1 => text(
            "notification-proposals-body",
            &[("agent", agent.clone()), ("count", count.clone())],
        ),
        _ => text(
            "notification-proposals-body-agents",
            &[("count", count.clone())],
        ),
    };
    Notice {
        title: text("notification-proposals", &[("count", count)]),
        body,
        task: None,
    }
}

/// What shows instead while the app is locked.
fn locked_notice() -> Notice {
    Notice {
        title: text("lock-notice-proposals-title", &[]),
        body: text("lock-notice-proposals-body", &[]),
        task: None,
    }
}

/// Starts showing new proposals from `events` through `notifier`, on a
/// thread of its own that ends with the engine's event bus.
pub(crate) fn start(
    events: Receiver<Changed>,
    notifier: Arc<dyn Notifier>,
) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("gk-app-proposals".into())
        .spawn(move || run(&events, &*notifier))
        .inspect_err(|err| eprintln!("glyphkeel-app: cannot watch for proposals: {err}"))
        .ok()
}

fn run(events: &Receiver<Changed>, notifier: &dyn Notifier) {
    while let Ok(event) = events.recv() {
        let mut count = proposed(&event);
        if count == 0 {
            continue;
        }
        let mut agents = BTreeSet::from([event.origin.actor.name.clone()]);
        let until = Instant::now() + BURST;
        let mut open = true;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match events.recv_timeout(left) {
                Ok(more) => {
                    let n = proposed(&more);
                    if n > 0 {
                        count += n;
                        agents.insert(more.origin.actor.name.clone());
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        notifier.show_private(&notice(count, &agents), &locked_notice());
        if !open {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Mutex;

    use gk_api::ops::common::{Actor, ActorKind, Via};
    use gk_api::{EntityChange, Origin};

    use super::*;

    #[derive(Default)]
    struct Seen(Mutex<Vec<Notice>>);

    impl Notifier for Seen {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(notice.clone());
        }
    }

    fn event(agent: &str, entity: &str, fields: &[&str]) -> Changed {
        Changed {
            seq: 1,
            group: "g".to_owned(),
            origin: Origin {
                via: Via::Mcp,
                actor: Actor {
                    kind: ActorKind::Agent,
                    name: agent.to_owned(),
                },
                session: None,
            },
            changes: vec![EntityChange {
                entity: entity.to_owned(),
                id: "p".to_owned(),
                project: None,
                fields: fields.iter().map(|f| (*f).to_owned()).collect(),
            }],
        }
    }

    #[test]
    fn a_burst_of_proposals_shows_once_and_decisions_show_nothing() {
        let (send, receive) = std::sync::mpsc::channel();
        let seen = Arc::new(Seen::default());
        let thread = start(receive, Arc::clone(&seen) as Arc<dyn Notifier>).unwrap();
        send.send(event("Helper", "task", &["title"])).unwrap();
        send.send(event("Helper", "proposal", &["status"])).unwrap();
        send.send(event("Helper", "proposal", &["op", "status"]))
            .unwrap();
        send.send(event("Helper", "proposal", &["op", "status"]))
            .unwrap();
        std::thread::sleep(BURST * 2);
        drop(send);
        thread.join().unwrap();
        let shown = seen.0.lock().unwrap();
        assert_eq!(shown.len(), 1, "{shown:?}");
        assert!(shown[0].body.contains("Helper"), "{shown:?}");
        assert_eq!(shown[0].task, None);
    }

    #[test]
    fn several_agents_are_not_named() {
        let agents = BTreeSet::from(["A".to_owned(), "B".to_owned()]);
        let n = notice(3, &agents);
        assert_eq!(
            n.body,
            text(
                "notification-proposals-body-agents",
                &[("count", "3".to_owned())]
            )
        );
        assert_ne!(locked_notice().body, n.body);
    }
}
