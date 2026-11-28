// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `get_task_context` in both protocol eras: on data set S a task's context
//! holds its parent, subtasks, links, comments, the agent brief and the
//! pages it links to; `max_bytes` trims the oldest comments first and says
//! so; an agent limited to a project doesn't see links to other projects;
//! and one context is one read, sized by its bytes.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

/// Imports data set S (seed 1) into the workspace of `options`, and returns
/// a subtask that has subtasks of its own, with a link to a task of
/// another project, a comment that links to a wiki page of its project,
/// and a page of its project marked as the brief.
fn data_set_s(options: &gk_mcp::Options, dir: &std::path::Path) -> (String, String, String) {
    let archive = dir.join("seed-S");
    gk_seed::generate(gk_seed::DataSet::Small, 1)
        .unwrap()
        .write_to(&archive)
        .unwrap();
    let (engine, person) = open_as_person(options);
    let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
    call(
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    );
    let found = call(
        "task.search",
        json!({ "query": "is:subtask is:parent", "limit": 1 }),
    );
    let task = found["items"][0].clone();
    let key = task["key"].as_str().unwrap().to_owned();
    let project = task["project"].as_str().unwrap().to_owned();
    let other = call(
        "task.search",
        json!({ "query": format!("-project:{project}/*"), "limit": 1 }),
    )["items"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        "relation.add",
        json!({ "key": key, "kind": "relates", "other": other }),
    );
    call(
        "wiki.create",
        json!({ "project": project, "title": "Conventions", "body": "Write tests first." }),
    );
    call(
        "wiki.create",
        json!({ "project": project, "title": "Working notes", "body": "Notes for this task." }),
    );
    call(
        "wiki.update",
        json!({ "project": project, "page": "Conventions", "agent_brief": true }),
    );
    call(
        "comment.add",
        json!({ "key": key, "body": "See [[Working notes]] before you start." }),
    );
    engine.close();
    (key, project, other)
}

#[tokio::test]
async fn a_tasks_context_on_data_set_s_holds_what_an_agent_needs_in_both_eras() {
    for (label, mode, version, _) in lifecycles() {
        let (dir, options) = workspace();
        let (key, project, other) = data_set_s(&options, dir.path());
        let c = connect(&options, mode, version, "Worker").await;
        let ctx = output(&c.call("get_task_context", json!({ "key": key })).await);

        assert_eq!(ctx["task"]["key"], key.as_str(), "{label}");
        assert_eq!(ctx["task"]["project"], project.as_str(), "{label}");
        let parents = keys(&ctx["parents"]);
        assert!(!parents.is_empty(), "{label}: a subtask has a parent");
        assert_eq!(
            ctx["task"]["parent"],
            parents[0].as_str(),
            "{label}: the parent comes first"
        );
        let subtasks = ctx["subtasks"].as_array().unwrap();
        assert!(!subtasks.is_empty(), "{label}: it has subtasks");
        assert!(
            subtasks
                .iter()
                .all(|s| s["level"] == 1 && s["parent"] == key.as_str())
        );
        let links = ctx["task"]["links"].as_array().unwrap();
        assert!(
            links.iter().any(|l| l["task"]["key"] == other.as_str()),
            "{label}: {links:?}"
        );
        let comments = ctx["task"]["comments"].as_array().unwrap();
        assert!(
            comments.last().unwrap()["body"]
                .as_str()
                .unwrap()
                .contains("[[Working notes]]"),
            "{label}: the newest comment comes last"
        );
        assert!(comments.len() <= 20, "{label}");
        assert_eq!(ctx["brief"]["source"], "wiki_page", "{label}");
        assert_eq!(ctx["brief"]["title"], "Conventions", "{label}");
        assert_eq!(ctx["brief"]["body"], "Write tests first.", "{label}");
        let pages = ctx["pages"].as_array().unwrap();
        assert!(
            pages.iter().any(|p| p["page"]["title"] == "Working notes"
                && p["excerpt"] == "Notes for this task."
                && p["cut"] == false),
            "{label}: {pages:?}"
        );
        assert!(
            pages.iter().all(|p| p["page"]["title"] != "Conventions"),
            "{label}: the brief isn't repeated"
        );
        let statuses = ctx["statuses"].as_array().unwrap();
        assert_eq!(
            statuses.iter().filter(|s| s["current"] == true).count(),
            1,
            "{label}"
        );
        assert!(
            ctx["task"]["checklist_items"].is_array(),
            "{label}: the checklist is numbered"
        );
        assert!(!ctx["activity"].as_array().unwrap().is_empty(), "{label}");
        let bytes = ctx["bytes"].as_u64().unwrap();
        assert!(bytes <= ctx["max_bytes"].as_u64().unwrap(), "{label}");
        assert_eq!(ctx["max_bytes"], 32768, "{label}");

        // Two levels reach the subtasks' own subtasks, each after its parent.
        let deep = output(
            &c.call("get_task_context", json!({ "key": key, "depth": 2 }))
                .await,
        );
        let deep = deep["subtasks"].as_array().unwrap();
        assert!(deep.len() >= subtasks.len(), "{label}");
        for (i, s) in deep.iter().enumerate() {
            if s["level"] == 2 {
                assert!(
                    deep[..i].iter().any(|p| p["task"]["key"] == s["parent"]),
                    "{label}: {s} comes after its parent"
                );
            }
        }
        let none = output(
            &c.call("get_task_context", json!({ "key": key, "depth": 0 }))
                .await,
        );
        assert!(none["subtasks"].as_array().unwrap().is_empty(), "{label}");

        let refused = c
            .call("get_task_context", json!({ "key": key, "max_bytes": 10 }))
            .await;
        assert_eq!(refused.is_error, Some(true), "{label}");
        assert!(
            text(&refused).contains("4096"),
            "{label}: {}",
            text(&refused)
        );
        c.close().await;
    }
}

/// TM-1 with a long description, 30 comments of about 600 bytes each and a
/// brief of 1400 bytes; OT-1 in another project, linked to it.
fn long_task(options: &gk_mcp::Options) {
    let (engine, person) = open_as_person(options);
    let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
    call(
        "project.create",
        json!({ "name": "Task manager", "key": "TM" }),
    );
    call("project.create", json!({ "name": "Other", "key": "OT" }));
    call(
        "task.create",
        json!({
            "project": "TM",
            "title": "A long one",
            "description": format!("- [ ] First step\n\n{}", "Detail. ".repeat(1000)),
        }),
    );
    call(
        "task.create",
        json!({ "project": "OT", "title": "Elsewhere" }),
    );
    call(
        "relation.add",
        json!({ "key": "TM-1", "kind": "blocks", "other": "OT-1" }),
    );
    for n in 1..=30 {
        call(
            "comment.add",
            json!({ "key": "TM-1", "body": format!("Comment {n}. {}", "Words. ".repeat(80)) }),
        );
    }
    call(
        "project.update",
        json!({ "project": "TM", "description": "Brief. ".repeat(200) }),
    );
    engine.close();
}

#[tokio::test]
async fn max_bytes_trims_the_oldest_comments_first_and_says_so_in_both_eras() {
    for (label, mode, version, _) in lifecycles() {
        let (_dir, options) = workspace();
        long_task(&options);
        let c = connect(&options, mode, version, "Worker").await;

        // By default: the 20 most recent comments, then cut to 32 KB.
        let ctx = output(&c.call("get_task_context", json!({ "key": "TM-1" })).await);
        let comments = ctx["task"]["comments"].as_array().unwrap();
        assert!(comments.len() <= 20, "{label}");
        assert!(
            comments.last().unwrap()["body"]
                .as_str()
                .unwrap()
                .starts_with("Comment 30."),
            "{label}"
        );
        assert!(
            ctx["trimmed"]["comments"].as_u64().unwrap() >= 10,
            "{label}"
        );
        assert!(ctx["bytes"].as_u64().unwrap() <= 32768, "{label}");

        // Small: every comment goes before the brief and the description
        // are cut.
        let small = output(
            &c.call(
                "get_task_context",
                json!({ "key": "TM-1", "max_bytes": 4096 }),
            )
            .await,
        );
        let bytes = small["bytes"].as_u64().unwrap();
        assert!(bytes <= 4096, "{label}: {bytes}");
        assert_eq!(
            serde_json::to_vec(&small).unwrap().len() as u64,
            bytes,
            "{label}: `bytes` is the size"
        );
        assert!(
            small["task"]["comments"].as_array().unwrap().is_empty(),
            "{label}"
        );
        assert_eq!(small["trimmed"]["comments"], 30, "{label}");
        assert_eq!(small["trimmed"]["brief"], true, "{label}");
        assert!(
            small["brief"]["body"].as_str().unwrap().len() < "Brief. ".len() * 200,
            "{label}"
        );
        // The links stay: they can't be trimmed.
        assert_eq!(small["task"]["links"][0]["task"]["key"], "OT-1", "{label}");
        assert_eq!(
            small["task"]["checklist_items"][0]["text"], "First step",
            "{label}"
        );
        c.close().await;
    }
}

#[tokio::test]
async fn an_agent_limited_to_a_project_sees_no_links_outside_it() {
    for (label, mode, version, _) in lifecycles() {
        let (_dir, mut options) = workspace();
        long_task(&options);
        options.project = Some("TM".to_owned());
        let c = connect(&options, mode, version, "Worker").await;
        let ctx = output(&c.call("get_task_context", json!({ "key": "TM-1" })).await);
        assert!(
            ctx["task"]["links"].as_array().unwrap().is_empty(),
            "{label}: {}",
            ctx["task"]["links"]
        );
        assert!(
            !serde_json::to_string(&ctx).unwrap().contains("OT-1"),
            "{label}: the other project's task isn't named"
        );
        let other = c.call("get_task_context", json!({ "key": "OT-1" })).await;
        assert_eq!(other.is_error, Some(true), "{label}");
        assert!(
            text(&other).starts_with("Error (not_found)"),
            "{label}: {}",
            text(&other)
        );
        c.close().await;
    }
}

#[tokio::test]
async fn one_context_is_one_read_sized_by_its_bytes() {
    let (_dir, options) = workspace();
    long_task(&options);
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "agent.limits.update",
                json!({ "read_burst": 2, "reads_per_minute": 1 }),
            )
            .unwrap();
        engine.close();
    }
    let (label, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    output(&c.call("get_task_context", json!({ "key": "TM-1" })).await);
    output(&c.call("get_workspace", json!({})).await);
    let refused = c.call("get_workspace", json!({})).await;
    assert_eq!(refused.is_error, Some(true), "{label}: {refused:?}");
    assert!(
        text(&refused).starts_with("Error (rate_limited)"),
        "{label}"
    );
    c.close().await;

    // Megabytes: what a context costs against the data limit.
    let (engine, person) = open_as_person(&options);
    let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
    assert_eq!(
        sessions["items"][0]["reads"], 2,
        "the context counted as one read: {sessions}"
    );
    engine
        .call(
            &person,
            "agent.limits.update",
            json!({
                "read_burst": 1000,
                "reads_per_minute": 1000,
                "read_mb_per_minute": 1,
            }),
        )
        .unwrap();
    engine
        .call(
            &person,
            "task.update",
            json!({ "key": "TM-1", "description": "Detail. ".repeat(40_000) }),
        )
        .unwrap();
    engine.close();
    // Small contexts leave the megabyte alone.
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    for _ in 0..10 {
        output(
            &c.call(
                "get_task_context",
                json!({ "key": "TM-1", "max_bytes": 4096 }),
            )
            .await,
        );
    }
    c.close().await;

    // A new session starts with a full megabyte. Each large context costs
    // about its `bytes` (its text is the same compact JSON, counted once),
    // so the session reads ceil(1 MiB / cost) of them before it waits.
    let c = connect(&options, mode, version, "Reader").await;
    let budget = 1_048_576_f64;
    let mut large = 0_u64;
    let mut reported = 0_f64;
    loop {
        let r = c
            .call(
                "get_task_context",
                json!({ "key": "TM-1", "max_bytes": 200_000 }),
            )
            .await;
        if r.is_error == Some(true) {
            assert!(text(&r).starts_with("Error (rate_limited)"), "{}", text(&r));
            break;
        }
        let bytes = output(&r)["bytes"].as_u64().unwrap() as f64;
        assert!(bytes > 190_000.0, "{bytes}");
        let shown = text(&r).len() as f64;
        assert!(
            (shown - bytes).abs() <= bytes * 0.1,
            "text {shown} vs bytes {bytes}"
        );
        reported = bytes;
        large += 1;
        assert!(large <= 10, "the data limit never stopped the reads");
    }
    let fewest = (budget / (reported * 1.1)).ceil() as u64;
    let most = (budget / (reported * 0.9)).ceil() as u64;
    assert!(
        (fewest..=most).contains(&large),
        "{large} contexts of {reported} bytes: charged more than 10 % off their size"
    );
    c.close().await;
}
