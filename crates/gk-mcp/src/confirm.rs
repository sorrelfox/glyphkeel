// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Asking the person, through the agent's client, to allow one call.
//!
//! An agent may ask for a call its profile doesn't allow, and is asked
//! before calls that delete a lot: deleting a project, purging the trash
//! and moving more tasks to the trash than the per-call limit. The client
//! shows the question ("Claude Code wants to delete the project Home lab
//! with 42 tasks. Allow?"); when the person allows it, the call runs once,
//! as if the agent had the full profile, and its history says that the
//! person confirmed it. When the person declines, or doesn't answer in
//! time, the call fails with `forbidden`, as it would without asking.
//!
//! The question goes out the way the protocol era has it: an
//! `elicitation/create` request in the old era, and an `input_required`
//! result in the 2026-07-28 revision, which the client answers by sending
//! the call again with the answer and the request state. That state is an
//! opaque handle to the question kept here, valid once, for the same tool
//! and arguments, for a few minutes.
//!
//! Only clients that declare form elicitation are asked, and only while
//! the workspace setting allows it; other clients get what they got
//! before.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use gk_api::{McpTool, OpError, Profile, Safety};
use rmcp::model::{
    ElicitRequest, ElicitRequestParams, ElicitResult, ElicitationAction, ElicitationSchema,
    InputRequest, InputRequests, InputRequiredResult, ProtocolVersion,
};
use rmcp::service::RequestContext;
use rmcp::{Peer, RoleServer};
use serde_json::{Map, Value};

use crate::messages;

/// How long the person has to answer an `elicitation/create` request, and
/// how long the request state of an `input_required` result stays valid.
pub(crate) const ANSWER_TIME: Duration = Duration::from_secs(5 * 60);

/// The key of the one input request in an `input_required` result.
const INPUT_KEY: &str = "confirm";

/// Questions waiting for the 2026-07-28 client to send the call again.
const MAX_PENDING: usize = 32;

/// How the client can be asked, if it can.
#[derive(Debug, Clone)]
pub(crate) enum Asker {
    /// The old era: a request to the client, answered while the call waits.
    Request(Peer<RoleServer>),
    /// The 2026-07-28 revision: an `input_required` result.
    InputRequired,
}

impl Asker {
    /// How the client of `context` can be asked: when it declares form
    /// elicitation for this request (2026-07-28) or for the session.
    pub(crate) fn of(context: &RequestContext<RoleServer>) -> Option<Self> {
        let capability = context.client_capabilities()?.elicitation?;
        // A client that names neither mode means forms.
        let form = capability.form.is_some() || capability.url.is_none();
        if !form {
            return None;
        }
        let stateless = context
            .protocol_version()
            .is_some_and(|v| v.as_str() >= ProtocolVersion::V_2026_07_28.as_str());
        Some(if stateless {
            Self::InputRequired
        } else {
            Self::Request(context.peer.clone())
        })
    }
}

/// Why a call needs the person's consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reason {
    /// The tool makes changes of a class the profile doesn't allow.
    BeyondProfile {
        /// The agent's profile after the workspace's cap.
        profile: Profile,
        /// The tool's class.
        safety: Safety,
    },
    /// Deleting a project for good, with its tasks.
    DeleteProject {
        /// The project's name.
        name: String,
        /// Its tasks, sub-projects' and trashed ones included.
        tasks: u64,
    },
    /// Deleting entries of the trash for good.
    PurgeTrash {
        /// How many entries.
        count: usize,
    },
    /// Moving more tasks to the trash than one call may.
    DeleteTasks {
        /// How many tasks, subtasks included when known.
        count: u64,
    },
}

/// Why calling `tool` with `args` as an agent with `profile` needs the
/// person's consent, before it runs; `None` when it doesn't. `project`
/// looks up a project's name and task count; `trash_per_call` is the
/// workspace's limit on items moved to the trash in one call.
pub(crate) fn reason_before(
    tool: &McpTool,
    profile: Profile,
    args: &Map<String, Value>,
    trash_per_call: Option<u32>,
    project: impl FnOnce(&str) -> Option<(String, u64)>,
) -> Option<Reason> {
    if !tool.allowed_for(profile) {
        return Some(Reason::BeyondProfile {
            profile,
            safety: tool.safety(),
        });
    }
    match tool.name {
        "delete_project" => {
            let key = args.get("project").and_then(Value::as_str)?;
            let (name, tasks) = project(key)?;
            Some(Reason::DeleteProject { name, tasks })
        }
        "purge_trash" => {
            let count = args
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            (count > 0).then_some(Reason::PurgeTrash { count })
        }
        "delete_tasks" => {
            let count = args
                .get("keys")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64;
            trash_per_call
                .filter(|max| count > u64::from(*max))
                .map(|_| Reason::DeleteTasks { count })
        }
        _ => None,
    }
}

/// Why a call that failed with `error` could run with the person's
/// consent: the engine counted more items for the trash than one call may
/// move (subtasks count), or a part of the call needs more than the
/// profile allows.
pub(crate) fn reason_after(error: &OpError) -> Option<Reason> {
    let arg = |name: &str| {
        error
            .details
            .get("args")
            .and_then(|a| a.get(name))
            .and_then(Value::as_str)
    };
    match error.details.get("message_id").and_then(Value::as_str)? {
        "trash-call-limit" => Some(Reason::DeleteTasks {
            count: arg("count")?.parse().ok()?,
        }),
        "engine-profile-refused" => Some(Reason::BeyondProfile {
            profile: arg("profile")?.parse().ok()?,
            safety: safety_named(arg("safety")?)?,
        }),
        _ => None,
    }
}

/// The class named `name`; `app` never, since nobody confirms those for
/// agents.
fn safety_named(name: &str) -> Option<Safety> {
    [
        Safety::Read,
        Safety::Write,
        Safety::Trash,
        Safety::Permanent,
    ]
    .into_iter()
    .find(|s| s.as_str() == name)
}

/// The question the person sees, from `agent`, about calling `tool`.
pub(crate) fn question(reason: &Reason, agent: &str, tool: &McpTool) -> String {
    let agent = agent.to_owned();
    match reason {
        Reason::BeyondProfile { profile, safety } => messages::text(
            "mcp-confirm-beyond-profile",
            &[
                ("agent", agent),
                ("title", tool.title.to_owned()),
                ("profile", profile.to_string()),
                ("safety", safety.as_str().to_owned()),
            ],
        ),
        Reason::DeleteProject { name, tasks } => messages::text(
            "mcp-confirm-delete-project",
            &[
                ("agent", agent),
                ("project", name.clone()),
                ("tasks", tasks.to_string()),
            ],
        ),
        Reason::PurgeTrash { count } => messages::text(
            "mcp-confirm-purge-trash",
            &[("agent", agent), ("count", count.to_string())],
        ),
        Reason::DeleteTasks { count } => messages::text(
            "mcp-confirm-delete-tasks",
            &[("agent", agent), ("count", count.to_string())],
        ),
    }
}

/// The error of a call the person declined or didn't answer.
pub(crate) fn declined(tool: &str) -> OpError {
    messages::hint(
        messages::error(
            "mcp-confirm-declined",
            &[("tool", tool.to_owned())],
            OpError::forbidden,
        ),
        "mcp-confirm-declined-hint",
        &[],
    )
}

/// The error of a call sent again with a request state that isn't (or no
/// longer) valid for it.
pub(crate) fn stale(tool: &str) -> OpError {
    messages::hint(
        messages::error(
            "mcp-confirm-stale",
            &[("tool", tool.to_owned())],
            OpError::forbidden,
        ),
        "mcp-confirm-stale-hint",
        &[],
    )
}

/// The elicitation that asks `question`: a form without fields, so the
/// client shows the question with its accept and decline buttons.
pub(crate) fn elicitation(question: String) -> ElicitRequestParams {
    ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: question,
        requested_schema: ElicitationSchema::new(BTreeMap::new()),
    }
}

/// Whether the person allowed it: accepted, and not with `allow: false`
/// for clients that add a field of their own.
pub(crate) fn allowed(result: &ElicitResult) -> bool {
    result.action == ElicitationAction::Accept
        && result
            .content
            .as_ref()
            .and_then(|c| c.get("allow"))
            .and_then(Value::as_bool)
            != Some(false)
}

/// Asks through an `elicitation/create` request and waits for the answer;
/// no answer in time, or a failed request, is a no.
pub(crate) async fn ask_now(peer: &Peer<RoleServer>, question: String) -> bool {
    match peer
        .create_elicitation_with_timeout(elicitation(question), Some(ANSWER_TIME))
        .await
    {
        Ok(result) => allowed(&result),
        Err(e) => {
            tracing::info!(error = %e, "the confirmation got no answer");
            false
        }
    }
}

/// The answer in a call sent again: what the client put under the input
/// request's key.
pub(crate) fn answer_in(responses: Option<&BTreeMap<String, Value>>) -> bool {
    responses
        .and_then(|r| r.get(INPUT_KEY))
        .and_then(|v| serde_json::from_value::<ElicitResult>(v.clone()).ok())
        .is_some_and(|r| allowed(&r))
}

/// Questions sent as `input_required` results, waiting for the call to
/// come again.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    waiting: Mutex<HashMap<String, Waiting>>,
}

#[derive(Debug)]
struct Waiting {
    tool: String,
    args: String,
    until: Instant,
}

impl Pending {
    /// The `input_required` result that asks `question` about calling
    /// `tool` with `args`, remembered until the call comes again.
    pub(crate) fn ask(
        &self,
        tool: &str,
        args: &Map<String, Value>,
        question: String,
        now: Instant,
    ) -> InputRequiredResult {
        let state = new_state();
        let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        waiting.retain(|_, w| w.until > now);
        if waiting.len() >= MAX_PENDING
            && let Some(oldest) = waiting
                .iter()
                .min_by_key(|(_, w)| w.until)
                .map(|(k, _)| k.clone())
        {
            waiting.remove(&oldest);
        }
        waiting.insert(
            state.clone(),
            Waiting {
                tool: tool.to_owned(),
                args: args_key(args),
                until: now + ANSWER_TIME,
            },
        );
        let mut requests = InputRequests::new();
        requests.insert(
            INPUT_KEY.to_owned(),
            InputRequest::Elicitation(ElicitRequest::new(elicitation(question))),
        );
        InputRequiredResult::new(Some(requests), Some(state))
    }

    /// Whether `state` names a question about calling `tool` with `args`
    /// that is still open; it is closed either way.
    pub(crate) fn take(
        &self,
        state: &str,
        tool: &str,
        args: &Map<String, Value>,
        now: Instant,
    ) -> bool {
        let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        waiting
            .remove(state)
            .is_some_and(|w| w.until > now && w.tool == tool && w.args == args_key(args))
    }
}

/// The arguments of a call, to compare the call sent again with.
fn args_key(args: &Map<String, Value>) -> String {
    Value::Object(args.clone()).to_string()
}

/// A new request state: 128 random bits as hex, never derived from the
/// call, so a client can't forge one.
fn new_state() -> String {
    let mut bytes = [0_u8; 16];
    crate::random::fill(&mut bytes);
    crate::random::hex(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> &'static McpTool {
        gk_api::mcp::TOOLS
            .iter()
            .find(|t| t.name == name)
            .expect("a catalog tool")
    }

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn calls_beyond_the_profile_and_large_deletions_ask() {
        let none = |_: &str| None;
        let r = reason_before(
            tool("create_task"),
            Profile::ReadOnly,
            &Map::new(),
            Some(20),
            none,
        );
        assert!(matches!(r, Some(Reason::BeyondProfile { .. })), "{r:?}");
        let r = reason_before(
            tool("delete_project"),
            Profile::Standard,
            &args(serde_json::json!({"project": "ZZ"})),
            Some(20),
            |_| Some(("Home lab".to_owned(), 42)),
        );
        assert_eq!(
            r,
            Some(Reason::DeleteProject {
                name: "Home lab".to_owned(),
                tasks: 42
            })
        );
        let keys: Vec<String> = (1..=21).map(|n| format!("ZZ-{n}")).collect();
        let many = args(serde_json::json!({ "keys": keys }));
        assert_eq!(
            reason_before(
                tool("delete_tasks"),
                Profile::Standard,
                &many,
                Some(20),
                none
            ),
            Some(Reason::DeleteTasks { count: 21 })
        );
        assert_eq!(
            reason_before(tool("delete_tasks"), Profile::Standard, &many, None, none),
            None
        );
        assert_eq!(
            reason_before(
                tool("create_task"),
                Profile::Standard,
                &Map::new(),
                Some(20),
                none
            ),
            None
        );
    }

    #[test]
    fn the_question_names_the_agent_and_what_it_deletes() {
        let q = question(
            &Reason::DeleteProject {
                name: "Home lab".to_owned(),
                tasks: 42,
            },
            "Claude Code",
            tool("delete_project"),
        );
        assert!(
            q.contains("Claude Code") && q.contains("Home lab") && q.contains("42"),
            "{q}"
        );
        let q = question(
            &Reason::PurgeTrash { count: 1 },
            "Codex",
            tool("purge_trash"),
        );
        assert!(q.contains("Codex") && q.contains("one item"), "{q}");
    }

    #[test]
    fn a_request_state_is_valid_once_for_the_same_call() {
        let pending = Pending::default();
        let now = Instant::now();
        let a = args(serde_json::json!({"project": "ZZ"}));
        let asked = pending.ask("delete_project", &a, "Allow?".to_owned(), now);
        let state = asked.request_state.clone().expect("a state");
        let other = args(serde_json::json!({"project": "TM"}));
        assert!(!pending.take(&state, "delete_project", &other, now));
        let state = pending
            .ask("delete_project", &a, "Allow?".to_owned(), now)
            .request_state
            .expect("a state");
        assert!(pending.take(&state, "delete_project", &a, now));
        assert!(!pending.take(&state, "delete_project", &a, now));
        let state = pending
            .ask("delete_project", &a, "Allow?".to_owned(), now)
            .request_state
            .expect("a state");
        assert!(!pending.take(&state, "delete_project", &a, now + ANSWER_TIME));
    }
}
