// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Read limits on agent sessions, in both protocol eras: a burst goes
//! through, reading on at more than the sustained rate is refused with
//! `rate_limited` and the seconds to wait, the person can lift the limits,
//! a read-only agent may read more, and the refusals are counted in the
//! session.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use gk_api::Profile;
use rmcp::model::{CallToolResult, ReadResourceRequestParams};
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

/// The seconds to wait that a refusal names.
fn retry_after(err: &str) -> u64 {
    err.lines()
        .find_map(|l| l.strip_prefix("Retry after (s): "))
        .unwrap_or_else(|| panic!("no retry-after in {err}"))
        .parse()
        .unwrap()
}

fn set_limits(engine: &gk_engine::Engine, person: &gk_engine::Context, limits: Value) {
    engine.call(person, "agent.limits.update", limits).unwrap();
}

#[tokio::test]
async fn a_burst_goes_through_then_reads_wait_in_both_eras() {
    for (label, mode, version, _) in lifecycles() {
        let (_dir, options) = workspace();
        let (engine, person) = open_as_person(&options);
        // Three at once, then one a minute.
        set_limits(
            &engine,
            &person,
            json!({ "read_burst": 3, "reads_per_minute": 1 }),
        );
        let c = connect(&options, mode, version, "Reader").await;
        output(
            &c.call("create_project", json!({ "name": "Agents", "key": "ZZ" }))
                .await,
        );
        for _ in 0..3 {
            output(&c.call("get_workspace", json!({})).await);
        }
        let refused = c.call("get_workspace", json!({})).await;
        assert_eq!(refused.is_error, Some(true), "{label}: {refused:?}");
        let err = text(&refused);
        assert!(err.starts_with("Error (rate_limited)"), "{label}: {err}");
        let wait = retry_after(&err);
        assert!((1..=60).contains(&wait), "{label}: {err}");

        // Resources count as reads too.
        let resource = c
            .client
            .read_resource(ReadResourceRequestParams::new("glyphkeel://projects/ZZ"))
            .await
            .expect_err("over the limit");
        assert!(
            resource.to_string().contains("rate_limited"),
            "{label}: {resource}"
        );

        // Changes have their own limit, and the person reads as much as
        // they like.
        output(
            &c.call("create_task", json!({ "project": "ZZ", "title": "Still" }))
                .await,
        );
        for _ in 0..200 {
            engine.call(&person, "workspace.get", json!({})).unwrap();
        }
        c.close().await;

        let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
        let session = &sessions["items"][0];
        assert_eq!(session["rate_limited"], 2, "{label}: {sessions}");
        assert_eq!(session["reads"], 3, "{label}: {sessions}");
        engine.close();
    }
}

#[tokio::test]
async fn the_person_lifts_the_read_limit_while_the_agent_works() {
    for (label, mode, version, _) in lifecycles() {
        let (_dir, options) = workspace();
        let (engine, person) = open_as_person(&options);
        set_limits(
            &engine,
            &person,
            json!({ "read_burst": 2, "reads_per_minute": 1 }),
        );
        let c = connect(&options, mode, version, "Reader").await;
        for _ in 0..2 {
            output(&c.call("get_workspace", json!({})).await);
        }
        let refused = c.call("get_workspace", json!({})).await;
        assert_eq!(refused.is_error, Some(true), "{label}: {refused:?}");

        set_limits(
            &engine,
            &person,
            json!({ "no_limit": ["reads_per_minute", "read_mb_per_minute"] }),
        );
        for _ in 0..100 {
            output(&c.call("get_workspace", json!({})).await);
        }
        // Set again, the agent starts with a full burst.
        set_limits(&engine, &person, json!({ "reset": ["reads_per_minute"] }));
        let workspace = output(&c.call("get_workspace", json!({})).await);
        assert_eq!(
            workspace["agent_limits"]["reads_per_minute"], 600,
            "{label}"
        );
        assert_eq!(workspace["agent_limits"]["read_burst"], 2, "{label}");
        c.close().await;
        engine.close();
    }
}

#[tokio::test]
async fn a_read_only_agent_may_read_twice_as_much() {
    for (label, mode, version, _) in lifecycles() {
        for (profile, allowed) in [(Profile::Standard, 3), (Profile::ReadOnly, 6)] {
            let (_dir, mut options) = workspace();
            options.profile = profile;
            let (engine, person) = open_as_person(&options);
            set_limits(
                &engine,
                &person,
                json!({ "read_burst": 3, "reads_per_minute": 1 }),
            );
            engine.close();
            let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
            for _ in 0..allowed {
                output(&c.call("get_workspace", json!({})).await);
            }
            let refused = c.call("get_workspace", json!({})).await;
            assert_eq!(
                refused.is_error,
                Some(true),
                "{label} {profile}: {refused:?}"
            );
            assert!(text(&refused).contains("rate_limited"), "{label} {profile}");
            c.close().await;
        }
    }
}

#[tokio::test]
async fn the_default_limits_let_an_agent_work_normally() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(1);
    let c = connect(&options, mode, version, "Reader").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "ZZ" }))
            .await,
    );
    // A burst of reads as an agent finding its way around.
    for _ in 0..50 {
        output(
            &c.call("search_tasks", json!({ "query": "project = ZZ" }))
                .await,
        );
    }
    c.close().await;
}
