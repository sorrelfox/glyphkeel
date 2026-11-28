// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Changes that wait for the person's approval, in both protocol eras: a
//! changing tool answers `pending_approval` with the proposal's ID, the
//! agent reads the outcome with get_proposals and hears of it through
//! glyphkeel://proposals, and each profile follows its own setting.

#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::{connect, connect_person, lifecycles, open_as_person, text, workspace};
use gk_api::Profile;
use rmcp::model::{CallToolResult, ElicitationAction, ServerNotification, SubscriptionFilter};
use serde_json::{Value, json};

const PROPOSALS: &str = "glyphkeel://proposals";

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

/// Sets the approvals for read-only, standard and full agents, with the
/// workspace letting agents do everything.
fn approvals(options: &gk_mcp::Options, read_only: &str, standard: &str, full: &str) {
    let (engine, person) = open_as_person(options);
    engine
        .call(
            &person,
            "agent.policy.update",
            json!({ "max_profile": "full", "approvals": {
                "read_only": read_only, "standard": standard, "full": full, "expire_days": 14 } }),
        )
        .unwrap();
    engine.close();
}

/// The proposal ID a `pending_approval` result names.
fn proposal_of(r: &CallToolResult) -> String {
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let t = text(r);
    assert!(t.starts_with("Error (pending_approval)"), "{t}");
    assert!(t.contains("Don't send it again"), "{t}");
    t.lines()
        .find_map(|l| l.strip_prefix("Proposal: "))
        .unwrap_or_else(|| panic!("no proposal in {t}"))
        .to_owned()
}

/// Approves (or rejects with `reason`) proposal `id` as the person, in
/// another program.
fn decide(options: &gk_mcp::Options, id: &str, reject: Option<&str>) {
    let (engine, person) = open_as_person(options);
    let out = match reject {
        None => engine
            .call(&person, "proposal.approve", json!({ "ids": [id] }))
            .unwrap(),
        Some(reason) => engine
            .call(
                &person,
                "proposal.reject",
                json!({ "ids": [id], "reason": reason }),
            )
            .unwrap(),
    };
    if reject.is_none() {
        assert_eq!(out["approved"], 1, "{out}");
    }
    engine.close();
}

#[tokio::test]
#[allow(deprecated)]
async fn a_change_waits_for_approval_and_the_agent_hears_the_outcome() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, options) = workspace();
        approvals(&options, "off", "all_writes", "off");
        let mut c = connect_person(&options, mode, asked, ElicitationAction::Accept).await;

        let overview = output(&c.call("get_workspace", json!({})).await);
        assert_eq!(overview["caller"]["approvals"], "all_writes", "{label}");

        let r = c
            .call("create_project", json!({ "name": "Home lab", "key": "ZZ" }))
            .await;
        let id = proposal_of(&r);
        assert!(c.questions().is_empty(), "{label}: nothing is asked");
        let missing = c.call("get_project", json!({ "project": "ZZ" })).await;
        assert_eq!(missing.is_error, Some(true), "{label}: not made yet");

        let listed = output(
            &c.call("get_proposals", json!({ "status": "pending" }))
                .await,
        );
        assert_eq!(listed["items"][0]["id"], id.as_str(), "{label}");
        assert_eq!(listed["items"][0]["op"], "project.create", "{label}");
        assert_eq!(listed["pending"], 1, "{label}");

        // Watch the proposals, then the person approves in the app.
        let mut subscription = None;
        if label.starts_with("initialize") {
            c.client
                .subscribe(rmcp::model::SubscribeRequestParams::new(PROPOSALS))
                .await
                .unwrap();
        } else {
            let filter = SubscriptionFilter::builder()
                .resource_subscriptions([PROPOSALS.to_owned()])
                .build();
            let s = c.client.peer().listen(filter).await.unwrap();
            assert_eq!(
                s.acknowledged().resource_subscriptions,
                Some(vec![PROPOSALS.to_owned()]),
                "{label}"
            );
            subscription = Some(s);
        }
        decide(&options, &id, None);
        let heard = match subscription.as_mut() {
            None => tokio::time::timeout(Duration::from_secs(10), c.updates.recv())
                .await
                .expect("an update in time")
                .unwrap(),
            Some(s) => loop {
                let next = tokio::time::timeout(Duration::from_secs(10), s.next())
                    .await
                    .expect("a notification in time")
                    .unwrap()
                    .expect("the subscription is open");
                if let ServerNotification::ResourceUpdatedNotification(n) = next {
                    break n.params.uri;
                }
            },
        };
        assert_eq!(heard, PROPOSALS, "{label}");
        drop(subscription);

        let decided = output(&c.call("get_proposals", json!({ "ids": [id] })).await);
        assert_eq!(decided["items"][0]["status"], "approved", "{label}");
        assert!(decided["items"][0]["group"].is_string(), "{label}");
        output(&c.call("get_project", json!({ "project": "ZZ" })).await);

        // A rejected change says why.
        let r = c
            .call("create_task", json!({ "project": "ZZ", "title": "Rack" }))
            .await;
        let id = proposal_of(&r);
        decide(&options, &id, Some("Use the existing rack task."));
        let decided = output(
            &c.call(
                "get_proposals",
                json!({ "session": "current", "status": "rejected" }),
            )
            .await,
        );
        assert_eq!(
            decided["items"][0]["reason"], "Use the existing rack task.",
            "{label}"
        );
        c.close().await;
    }
}

#[tokio::test]
async fn read_only_and_full_agents_follow_their_own_setting() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, mut options) = workspace();
        {
            let (engine, person) = open_as_person(&options);
            engine
                .call(
                    &person,
                    "project.create",
                    json!({ "name": "Home lab", "key": "ZZ" }),
                )
                .unwrap();
            engine.close();
        }

        // A read-only agent without approvals sees no changing tools.
        options.profile = Profile::ReadOnly;
        approvals(&options, "off", "off", "off");
        let c = connect(&options, mode.clone(), asked.clone(), "codex").await;
        assert!(
            !c.tool_names().await.iter().any(|t| t == "create_task"),
            "{label}"
        );
        c.close().await;

        // With approvals it sees a standard agent's tools and proposes.
        approvals(&options, "all_writes", "off", "off");
        let c = connect(&options, mode.clone(), asked.clone(), "codex").await;
        let names = c.tool_names().await;
        assert!(names.iter().any(|t| t == "create_task"), "{label}");
        assert!(!names.iter().any(|t| t == "purge_trash"), "{label}");
        let r = c
            .call("create_task", json!({ "project": "ZZ", "title": "Rack" }))
            .await;
        proposal_of(&r);
        c.close().await;

        // A full agent whose deletions need approval: changes run, the
        // deletion waits.
        options.profile = Profile::Full;
        approvals(&options, "off", "off", "destructive");
        let c = connect(&options, mode.clone(), asked.clone(), "codex").await;
        output(
            &c.call("create_task", json!({ "project": "ZZ", "title": "Cables" }))
                .await,
        );
        let r = c.call("delete_project", json!({ "project": "ZZ" })).await;
        let id = proposal_of(&r);
        output(&c.call("get_project", json!({ "project": "ZZ" })).await);
        c.close().await;
        decide(&options, &id, None);
        let (engine, person) = open_as_person(&options);
        let gone = engine.call(&person, "project.get", json!({ "project": "ZZ" }));
        assert!(gone.is_err(), "{label}: deleted once approved");
        engine.close();
    }
}
