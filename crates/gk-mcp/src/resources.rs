// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Resources an agent can read and watch: a task, a project, and the
//! result of a query.
//!
//! | URI                                   | Reads                       |
//! |---------------------------------------|-----------------------------|
//! | `glyphkeel://tasks/{key}`             | the task, as `get_task`     |
//! | `glyphkeel://projects/{key}`          | the project, as `get_project` |
//! | `glyphkeel://queries{?query}`         | the first 50 matches of a query |
//!
//! Watching uses the engine's change events: this program's own commits,
//! and for `glyphkeel mcp` those of other programs too (the app watches
//! them already). A task's URI changes when the task does, a project's
//! when the project or anything in it does, and a query's when its result
//! does. Notifications are throttled: at most one per URI a second.
//!
//! The reads and watches happen within the agent's project when the
//! session is limited to one; every profile may read. The limit is checked
//! again on every change, where the task or project is now: one that moves
//! out of it is no longer watched.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use gk_api::{Changed, OpError};
use gk_engine::{Context, Engine};
use rmcp::model::{ResourceContents, ResourceTemplate};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::messages;

/// The scheme of every URI.
const SCHEME: &str = "glyphkeel://";
/// At most one notification per URI in this time.
pub(crate) const THROTTLE: Duration = Duration::from_secs(1);
/// The most URIs one session watches.
pub(crate) const MAX_WATCHED: usize = 100;
/// Matches a query resource reads.
const QUERY_LIMIT: u64 = 50;
/// How often `glyphkeel mcp` looks for other programs' commits.
const OTHERS_INTERVAL: Duration = Duration::from_millis(500);
/// Change events kept for a slow watcher; one that falls further behind
/// treats everything as changed.
const FEED_CAPACITY: usize = 256;

/// What a URI names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resource {
    /// A task, by key or ID.
    Task(String),
    /// A project, by key or ID.
    Project(String),
    /// A query in the query language.
    Query(String),
}

impl Resource {
    /// The resource `uri` names, if it names one.
    pub(crate) fn parse(uri: &str) -> Option<Self> {
        let rest = uri.strip_prefix(SCHEME)?;
        if let Some(key) = rest.strip_prefix("tasks/") {
            return single(key).map(Self::Task);
        }
        if let Some(key) = rest.strip_prefix("projects/") {
            return single(key).map(Self::Project);
        }
        let query = rest.strip_prefix("queries?")?;
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix("query="))
            .and_then(percent_decode)
            .filter(|q| !q.trim().is_empty())
            .map(Self::Query)
    }
}

/// One path segment, decoded.
fn single(segment: &str) -> Option<String> {
    if segment.is_empty() || segment.contains('/') || segment.contains('?') {
        return None;
    }
    percent_decode(segment).filter(|s| !s.trim().is_empty())
}

/// `%XX` escapes decoded (and `+` as a space, as forms send it); `None`
/// for a broken escape or text that isn't UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// The resource templates, for `resources/templates/list`.
pub(crate) fn templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate::new("glyphkeel://tasks/{key}", "task")
            .with_title("Task")
            .with_description(
                "A task by key (TM-41) or ID, as get_task returns it. Subscribe to hear when it changes.",
            )
            .with_mime_type("application/json"),
        ResourceTemplate::new("glyphkeel://projects/{key}", "project")
            .with_title("Project")
            .with_description(
                "A project by key (TM) or ID, as get_project returns it. Subscribe to hear when it or anything in it changes.",
            )
            .with_mime_type("application/json"),
        ResourceTemplate::new("glyphkeel://queries{?query}", "query")
            .with_title("Query")
            .with_description(
                "The first 50 tasks a query in the query language matches, as search_tasks returns them. Subscribe to hear when the result changes.",
            )
            .with_mime_type("application/json"),
    ]
}

/// The error for a URI that names nothing.
pub(crate) fn unknown(uri: &str) -> OpError {
    messages::hint(
        messages::error("mcp-no-such-resource", &[("uri", uri.to_owned())], |m| {
            OpError::not_found(m, &[] as &[&str])
        }),
        "mcp-no-such-resource-hint",
        &[],
    )
}

/// Reads `resource` for `ctx`: its JSON.
pub(crate) fn read(engine: &Engine, ctx: &Context, resource: &Resource) -> Result<Value, OpError> {
    match resource {
        Resource::Task(key) => {
            let out = engine.call(ctx, "task.get", json!({ "keys": [key] }))?;
            Ok(out["items"][0].clone())
        }
        Resource::Project(key) => engine.call(ctx, "project.get", json!({ "project": key })),
        Resource::Query(query) => engine.call(
            ctx,
            "task.search",
            json!({ "query": query, "limit": QUERY_LIMIT }),
        ),
    }
}

/// `resource` as the one content of a `resources/read` result.
pub(crate) fn contents(uri: &str, value: &Value) -> ResourceContents {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    ResourceContents::text(text, uri).with_mime_type("application/json")
}

/// A watched resource: what its change events look like.
#[derive(Debug, Clone)]
pub(crate) enum Target {
    /// Changes to the task with this ID.
    Task(String),
    /// Changes to the project with this ID or to anything in it.
    Project(String),
    /// Any change, then the query's result is compared with the last one.
    Query {
        /// The query.
        query: String,
        /// A hash of its last result.
        last: Option<u64>,
    },
}

impl Target {
    /// What watching `resource` means, looked up for `ctx`.
    ///
    /// # Errors
    /// When the task or project isn't there or is outside the caller's
    /// project, or the query doesn't parse.
    pub(crate) fn resolve(
        engine: &Engine,
        ctx: &Context,
        resource: &Resource,
    ) -> Result<Self, OpError> {
        let value = read(engine, ctx, resource)?;
        Ok(match resource {
            Resource::Task(_) => Self::Task(id_of(&value)),
            Resource::Project(_) => Self::Project(id_of(&value)),
            Resource::Query(query) => Self::Query {
                query: query.clone(),
                last: Some(hash(&value)),
            },
        })
    }

    /// The resource this target watches, by ID.
    fn resource(&self) -> Resource {
        match self {
            Self::Task(id) => Resource::Task(id.clone()),
            Self::Project(id) => Resource::Project(id.clone()),
            Self::Query { query, .. } => Resource::Query(query.clone()),
        }
    }

    /// Whether `event` may have changed the resource.
    fn touched_by(&self, event: &Changed) -> bool {
        match self {
            Self::Task(id) => event
                .changes
                .iter()
                .any(|c| c.entity == "task" && &c.id == id),
            Self::Project(id) => event
                .changes
                .iter()
                .any(|c| (c.entity == "project" && &c.id == id) || c.project.as_ref() == Some(id)),
            Self::Query { .. } => !event.changes.is_empty(),
        }
    }
}

fn id_of(value: &Value) -> String {
    value["id"].as_str().unwrap_or_default().to_owned()
}

fn hash(value: &Value) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.to_string().hash(&mut h);
    h.finish()
}

/// The resources a session watches, by URI.
pub(crate) type Watched = Arc<Mutex<BTreeMap<String, Target>>>;

/// The URIs in `watched` that `event` may have changed.
pub(crate) fn touched(watched: &Watched, event: &Changed, into: &mut BTreeSet<String>) {
    let watched = watched.lock().unwrap_or_else(PoisonError::into_inner);
    for (uri, target) in watched.iter() {
        if target.touched_by(event) {
            into.insert(uri.clone());
        }
    }
}

/// Every URI in `watched`, after the watcher fell behind.
pub(crate) fn all(watched: &Watched, into: &mut BTreeSet<String>) {
    let watched = watched.lock().unwrap_or_else(PoisonError::into_inner);
    into.extend(watched.keys().cloned());
}

/// Adds to `dirty` what one receive from the feed may have changed: the
/// URIs an event touches, or every URI in `watched` when the watcher fell
/// more than the feed's capacity behind and events were lost. `false` when
/// the feed has closed.
pub(crate) fn note(
    event: Result<Arc<Changed>, broadcast::error::RecvError>,
    watched: &Watched,
    dirty: &mut BTreeSet<String>,
) -> bool {
    match event {
        Ok(event) => touched(watched, &event, dirty),
        Err(broadcast::error::RecvError::Lagged(_)) => all(watched, dirty),
        Err(broadcast::error::RecvError::Closed) => return false,
    }
    true
}

/// Of the `dirty` URIs, those that really changed: a query's only when its
/// result differs from the last one, read again for `ctx`.
///
/// When `ctx` is limited to a project, a watched task or project is looked
/// up again by its ID within that limit, where it is now: one that has
/// moved out of it is no longer watched and its change is not sent.
/// `unlimited` is the same caller without the limit, to tell a resource
/// that moved away from one that is gone (whose change is still sent).
pub(crate) fn changed(
    engine: &Engine,
    ctx: &Context,
    unlimited: &Context,
    watched: &Watched,
    dirty: BTreeSet<String>,
) -> Vec<String> {
    let mut watched = watched.lock().unwrap_or_else(PoisonError::into_inner);
    let mut out = Vec::new();
    for uri in dirty {
        let keep = match watched.get_mut(&uri) {
            Some(Target::Query { query, last }) => {
                let now = read(engine, ctx, &Resource::Query(query.clone()))
                    .ok()
                    .map(|v| hash(&v));
                let differs = now != *last;
                *last = now;
                if differs {
                    out.push(uri.clone());
                }
                true
            }
            Some(target) => {
                let in_scope = ctx.project_scope.is_none() || {
                    let resource = target.resource();
                    read(engine, ctx, &resource).is_ok()
                        || read(engine, unlimited, &resource).is_err()
                };
                if in_scope {
                    out.push(uri.clone());
                }
                in_scope
            }
            None => true,
        };
        if !keep {
            watched.remove(&uri);
        }
    }
    out
}

/// The workspace's change events for watchers, started on first use. One
/// feed serves every session over the same engine.
#[derive(Debug)]
pub(crate) struct Feed {
    sender: OnceLock<broadcast::Sender<Arc<Changed>>>,
    /// Whether to look for other programs' commits too.
    watch_others: bool,
    others: Mutex<Option<gk_store::WatcherHandle>>,
}

impl Feed {
    /// A feed that, once started, also publishes other programs' commits
    /// when `watch_others` (for `glyphkeel mcp`; the app's engine does it
    /// already).
    pub(crate) fn new(watch_others: bool) -> Self {
        Self {
            sender: OnceLock::new(),
            watch_others,
            others: Mutex::new(None),
        }
    }

    /// A receiver of `engine`'s change events from now on.
    pub(crate) fn receiver(&self, engine: &Engine) -> broadcast::Receiver<Arc<Changed>> {
        self.sender
            .get_or_init(|| {
                let (sender, _) = broadcast::channel(FEED_CAPACITY);
                let events = engine.subscribe();
                let forward = sender.clone();
                // The engine's events come on a blocking channel; this
                // thread ends when the engine closes.
                let started = std::thread::Builder::new()
                    .name("change-feed".to_owned())
                    .spawn(move || {
                        while let Ok(event) = events.recv() {
                            let _ = forward.send(Arc::new(event));
                        }
                    });
                if let Err(e) = started {
                    tracing::warn!(error = %e, "watching changes failed to start");
                }
                if self.watch_others {
                    match engine.watch_other_processes(OTHERS_INTERVAL) {
                        Ok(handle) => {
                            *self.others.lock().unwrap_or_else(PoisonError::into_inner) =
                                Some(handle);
                        }
                        Err(e) => tracing::warn!(error = %e, "watching other programs failed"),
                    }
                }
                sender
            })
            .subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gk_api::event::{EntityChange, Origin};
    use gk_api::ops::common::{Actor, ActorKind, Via};

    fn event(task: &str) -> Arc<Changed> {
        Arc::new(Changed {
            seq: 1,
            group: "g".to_owned(),
            origin: Origin {
                via: Via::Cli,
                actor: Actor {
                    kind: ActorKind::User,
                    name: "You".to_owned(),
                },
                session: None,
            },
            changes: vec![EntityChange {
                entity: "task".to_owned(),
                id: task.to_owned(),
                project: Some("p1".to_owned()),
                fields: vec!["title".to_owned()],
            }],
        })
    }

    fn watching() -> Watched {
        let watched: Watched = Watched::default();
        {
            let mut w = watched.lock().unwrap();
            w.insert(
                "glyphkeel://tasks/A".to_owned(),
                Target::Task("a".to_owned()),
            );
            w.insert(
                "glyphkeel://tasks/B".to_owned(),
                Target::Task("b".to_owned()),
            );
            w.insert(
                "glyphkeel://projects/Q".to_owned(),
                Target::Project("q".to_owned()),
            );
        }
        watched
    }

    #[test]
    fn a_watcher_that_falls_behind_treats_everything_as_changed() {
        let watched = watching();
        let (sender, mut receiver) = broadcast::channel(FEED_CAPACITY);

        // Within the feed's capacity, only what the events touch.
        sender.send(event("a")).unwrap();
        let mut dirty = BTreeSet::new();
        assert!(note(Ok(receiver.try_recv().unwrap()), &watched, &mut dirty));
        assert_eq!(dirty, BTreeSet::from(["glyphkeel://tasks/A".to_owned()]));

        // More events than the feed keeps: the lost ones may have touched
        // anything, so every watched URI is dirty.
        for _ in 0..=FEED_CAPACITY {
            sender.send(event("a")).unwrap();
        }
        let mut dirty = BTreeSet::new();
        let lagged = match receiver.try_recv() {
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                Err(broadcast::error::RecvError::Lagged(n))
            }
            other => panic!("the watcher fell behind: {other:?}"),
        };
        assert!(note(lagged, &watched, &mut dirty));
        assert_eq!(dirty.len(), 3, "{dirty:?}");
        assert!(dirty.contains("glyphkeel://projects/Q"));

        // A closed feed ends the watcher.
        drop(sender);
        assert!(!note(
            Err(broadcast::error::RecvError::Closed),
            &watched,
            &mut BTreeSet::new()
        ));
    }

    #[test]
    fn uris_name_tasks_projects_and_queries() {
        assert_eq!(
            Resource::parse("glyphkeel://tasks/TM-41"),
            Some(Resource::Task("TM-41".to_owned()))
        );
        assert_eq!(
            Resource::parse("glyphkeel://projects/TM"),
            Some(Resource::Project("TM".to_owned()))
        );
        assert_eq!(
            Resource::parse("glyphkeel://queries?query=status%3Aopen+label%3Aui"),
            Some(Resource::Query("status:open label:ui".to_owned()))
        );
        for bad in [
            "glyphkeel://tasks/",
            "glyphkeel://tasks/TM-1/x",
            "glyphkeel://queries?query=",
            "glyphkeel://queries?query=%zz",
            "file:///etc/passwd",
            "glyphkeel://wiki/x",
        ] {
            assert_eq!(Resource::parse(bad), None, "{bad}");
        }
    }
}
