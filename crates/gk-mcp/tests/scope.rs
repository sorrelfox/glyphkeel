// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent started with `--project TM` works only in TM and its
//! sub-projects: other projects don't appear in lists or searches, and
//! naming or changing anything in them fails.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use gk_api::Toolset;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn refused(r: &CallToolResult) {
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

fn keys(list: &Value) -> Vec<String> {
    let mut keys: Vec<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn an_agent_limited_to_a_project_sees_and_changes_only_that_project() {
    let (_dir, mut options) = workspace();
    let (other_comment, other_page) = {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Its docs", "key": "DOC", "parent": "TM" }),
        );
        call("project.create", json!({ "name": "Private", "key": "PRV" }));
        for (project, title) in [
            ("TM", "Shared word in TM"),
            ("DOC", "Shared word in DOC"),
            ("PRV", "Shared word in PRV"),
        ] {
            call(
                "task.create",
                json!({ "project": project, "title": title, "due_date": "today" }),
            );
        }
        let comment = call(
            "comment.add",
            json!({ "key": "PRV-1", "body": "A private remark." }),
        )["comment"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let page = call(
            "wiki.create",
            json!({ "project": "PRV", "title": "Secrets", "body": "Shared word." }),
        )["page"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        engine.close();
        (comment, page)
    };
    options.project = Some("TM".to_owned());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scoped Helper").await;

    // Lists and searches show TM and its sub-project only.
    let projects = output(&c.call("list_projects", json!({})).await);
    assert_eq!(keys(&projects), ["DOC", "TM"]);
    let tasks = output(&c.call("search_tasks", json!({})).await);
    assert_eq!(keys(&tasks), ["DOC-1", "TM-1"]);
    let named = output(
        &c.call("search_tasks", json!({ "query": "project:PRV" }))
            .await,
    );
    assert_eq!(named["total"], 0);
    let found = output(&c.call("search", json!({ "text": "shared" })).await);
    assert!(
        found["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["task"].as_str().is_some_and(|k| !k.starts_with("PRV"))),
        "{found}"
    );
    assert_eq!(found["total"], 2, "{found}");
    let today = output(&c.call("get_today", json!({})).await);
    assert!(!today.to_string().contains("PRV-1"), "{today}");
    // The board across projects is the limit's project and those below it.
    let across = output(&c.call("get_board_across", json!({})).await);
    let todo: Vec<&str> = across["columns"][0]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["key"].as_str().unwrap())
        .collect();
    assert_eq!(todo, ["DOC-1", "TM-1"], "{across}");
    assert!(!across.to_string().contains("PRV"), "{across}");
    let feed = output(&c.call("get_activity", json!({})).await);
    assert!(
        !feed["items"].as_array().unwrap().is_empty() && !feed.to_string().contains("Private"),
        "{feed}"
    );

    // Anything of another project is missing.
    refused(&c.call("get_task", json!({ "keys": ["PRV-1"] })).await);
    refused(&c.call("get_project", json!({ "project": "PRV" })).await);
    refused(&c.call("get_board", json!({ "project": "PRV" })).await);
    refused(&c.call("search_tasks", json!({ "project": "PRV" })).await);
    refused(&c.call("get_wiki_page", json!({ "page": other_page })).await);
    refused(
        &c.call(
            "update_comment",
            json!({ "comment": other_comment, "body": "Changed." }),
        )
        .await,
    );
    refused(
        &c.call(
            "create_task",
            json!({ "project": "PRV", "title": "Sneaked in" }),
        )
        .await,
    );
    refused(
        &c.call(
            "link_tasks",
            json!({ "key": "TM-1", "kind": "relates", "other": "PRV-1" }),
        )
        .await,
    );
    // A new project outside TM is a change outside the limit.
    refused(
        &c.call(
            "create_project",
            json!({ "name": "Elsewhere", "key": "ELS" }),
        )
        .await,
    );

    // Inside TM the agent works as usual, sub-projects included.
    let made = output(
        &c.call(
            "create_task",
            json!({ "project": "DOC", "title": "Write the guide" }),
        )
        .await,
    );
    assert_eq!(made["task"]["key"], "DOC-2");
    let sub = output(
        &c.call(
            "create_project",
            json!({ "name": "Below", "key": "BLW", "parent": "TM" }),
        )
        .await,
    );
    assert_eq!(sub["project"]["key"], "BLW");
    output(
        &c.call(
            "create_task",
            json!({ "project": "BLW", "title": "In the new sub-project" }),
        )
        .await,
    );
    let bulk = output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "title:shared", "changes": { "priority": "high" } }),
        )
        .await,
    );
    assert_eq!(bulk["count"], 2, "{bulk}");
    output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "title:shared", "changes": { "priority": "high" },
                    "dry_run": false, "expected_count": 2 }),
        )
        .await,
    );
    c.close().await;

    // The private project is untouched.
    let (engine, person) = open_as_person(&options);
    let prv = engine
        .call(&person, "task.get", json!({ "keys": ["PRV-1"] }))
        .unwrap();
    assert_eq!(prv["items"][0]["priority"], "none");
    let tm = engine
        .call(&person, "task.get", json!({ "keys": ["TM-1"] }))
        .unwrap();
    assert_eq!(tm["items"][0]["priority"], "high");
    assert_eq!(prv["items"][0]["title"], "Shared word in PRV");
}

/// The projects of a list's items, sorted and without repeats.
fn projects_of(list: &Value, at: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let mut v = item;
            for step in at {
                v = &v[*step];
            }
            v.as_str().unwrap().to_owned()
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

#[tokio::test]
async fn a_limited_agent_sees_nothing_of_other_projects_in_references_and_errors() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Its docs", "key": "DOC", "parent": "TM" }),
        );
        call("project.create", json!({ "name": "Private", "key": "PRV" }));
        for project in ["TM", "DOC", "PRV"] {
            for title in ["First", "Second"] {
                call(
                    "task.create",
                    json!({ "project": project, "title": format!("{title} in {project}") }),
                );
            }
            call(
                "wiki.create",
                json!({ "project": project, "title": format!("Notes of {project}"),
                        "body": "A shared word." }),
            );
        }
        // Links made by the person, across projects.
        for (key, kind, other) in [
            ("TM-1", "relates", "DOC-1"),
            ("TM-1", "blocked_by", "PRV-1"),
        ] {
            call(
                "relation.add",
                json!({ "key": key, "kind": kind, "other": other }),
            );
        }
        // Something of every project in the trash.
        call("task.delete", json!({ "keys": ["TM-2", "DOC-2", "PRV-2"] }));
        call(
            "project.create",
            json!({ "name": "Gone", "key": "GON", "parent": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Gone too", "key": "GTO", "parent": "PRV" }),
        );
        call(
            "project.delete",
            json!({ "project": "GON", "expected_task_count": 0 }),
        );
        call(
            "project.delete",
            json!({ "project": "GTO", "expected_task_count": 0 }),
        );
        engine.close();
    }
    options.project = Some("TM".to_owned());
    options.toolsets = Some(vec![
        Toolset::Workspace,
        Toolset::Projects,
        Toolset::Tasks,
        Toolset::Wiki,
        Toolset::Admin,
    ]);
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scoped Helper").await;

    // Wiki search and the trash cover the sub-projects, nothing else.
    let pages = output(
        &c.call("search_wiki_pages", json!({ "text": "shared" }))
            .await,
    );
    assert_eq!(
        projects_of(&pages, &["page", "project"]),
        ["DOC", "TM"],
        "{pages}"
    );
    let recent = output(&c.call("search_wiki_pages", json!({})).await);
    assert_eq!(
        projects_of(&recent, &["page", "project"]),
        ["DOC", "TM"],
        "{recent}"
    );
    refused(
        &c.call(
            "search_wiki_pages",
            json!({ "text": "shared", "project": "PRV" }),
        )
        .await,
    );
    let trash = output(&c.call("list_trash", json!({})).await);
    assert_eq!(
        projects_of(&trash, &["project"]),
        ["DOC", "GON", "TM"],
        "{trash}"
    );
    assert!(!trash.to_string().contains("PRV"), "{trash}");
    let doc = output(&c.call("list_trash", json!({ "project": "DOC" })).await);
    assert_eq!(projects_of(&doc, &["project"]), ["DOC"], "{doc}");
    refused(&c.call("list_trash", json!({ "project": "PRV" })).await);
    refused(&c.call("list_trash", json!({ "project": "GTO" })).await);

    // Links to tasks outside the limit are left out; the task still shows
    // as blocked, and closing it says so without naming the blocker.
    let task = output(
        &c.call(
            "get_task",
            json!({ "keys": ["TM-1"], "include": ["links", "subtasks"] }),
        )
        .await,
    );
    let links = task["items"][0]["links"].as_array().unwrap();
    assert_eq!(links.len(), 1, "{task}");
    assert_eq!(links[0]["task"]["key"], "DOC-1");
    assert!(!task.to_string().contains("PRV"), "{task}");
    assert_eq!(task["items"][0]["blocked"], true);
    let other_side = output(
        &c.call(
            "get_task",
            json!({ "keys": ["DOC-1"], "include": ["links"] }),
        )
        .await,
    );
    assert_eq!(other_side["items"][0]["links"][0]["task"]["key"], "TM-1");
    let closed = output(
        &c.call("move_task", json!({ "key": "TM-1", "status": "Done" }))
            .await,
    );
    let warnings = closed["warnings"].to_string();
    assert!(
        warnings.contains("TM-1") && warnings.contains("outside") && !warnings.contains("PRV"),
        "{closed}"
    );
    assert!(!closed.to_string().contains("PRV"), "{closed}");

    // A key used outside the limit is refused without saying so; one used
    // inside it gets the usual message.
    for call in [
        c.call(
            "create_project",
            json!({ "name": "Again", "key": "PRV", "parent": "TM" }),
        )
        .await,
        c.call("create_project", json!({ "name": "Again", "key": "GTO" }))
            .await,
        c.call("update_project", json!({ "project": "DOC", "key": "PRV" }))
            .await,
    ] {
        refused(&call);
        let message = common::text(&call);
        assert!(!message.contains("already used"), "{message}");
        assert!(message.contains("can't be used here"), "{message}");
    }
    let inside = c
        .call(
            "create_project",
            json!({ "name": "Again", "key": "DOC", "parent": "TM" }),
        )
        .await;
    refused(&inside);
    assert!(common::text(&inside).contains("already used"), "{inside:?}");
    // An import makes top-level projects, so it is refused before the
    // archive is read: a dry run doesn't list keys taken elsewhere. (The
    // file is there, so the check of the path lets it through.)
    let exports = options.layout().unwrap().exports_dir();
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::write(exports.join("none.zip"), b"not read").unwrap();
    let import = c
        .call(
            "import_data",
            json!({ "path": "none.zip", "dry_run": true }),
        )
        .await;
    refused(&import);
    assert!(common::text(&import).contains("limited to"), "{import:?}");
    c.close().await;

    // No second PRV was made.
    let (engine, person) = open_as_person(&options);
    let all = engine.call(&person, "project.list", json!({})).unwrap();
    assert_eq!(keys(&all), ["DOC", "PRV", "TM"]);
}

#[tokio::test]
async fn an_agent_limited_to_a_sub_project_is_not_told_about_the_projects_above() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Its docs", "key": "DOC", "parent": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Guides", "key": "GDE", "parent": "DOC" }),
        );
        engine.close();
    }
    options.project = Some("DOC".to_owned());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scoped Helper").await;

    let projects = output(&c.call("list_projects", json!({})).await);
    assert_eq!(keys(&projects), ["DOC", "GDE"]);
    let items = projects["items"].as_array().unwrap();
    assert_eq!(items[0]["key"], "DOC");
    assert!(items[0].get("parent").is_none(), "{projects}");
    assert_eq!(items[0]["path"], "Its docs");
    assert_eq!(items[0]["depth"], 0);
    assert_eq!(items[1]["parent"], "DOC");
    assert_eq!(items[1]["path"], "Its docs / Guides");
    assert_eq!(items[1]["depth"], 1);
    assert!(!projects.to_string().contains("Task manager"), "{projects}");
    for key in ["DOC", "GDE"] {
        let project = output(&c.call("get_project", json!({ "project": key })).await);
        let text = project.to_string();
        assert!(
            !text.contains("Task manager") && !text.contains("\"TM\""),
            "{project}"
        );
    }
    let made = output(
        &c.call(
            "create_project",
            json!({ "name": "Below", "key": "BLW", "parent": "DOC" }),
        )
        .await,
    );
    assert_eq!(made["project"]["path"], "Its docs / Below");
    assert_eq!(made["project"]["parent"], "DOC");
    c.close().await;

    // Without the limit the full path shows as before.
    let (engine, person) = open_as_person(&options);
    let doc = engine
        .call(&person, "project.get", json!({ "project": "DOC" }))
        .unwrap();
    assert_eq!(doc["parent"], "TM");
    assert_eq!(doc["path"], "Task manager / Its docs");
}
