// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent keeps the wiki and the schedule: it reads the project's brief,
//! follows links in both directions, moves, deletes and exports pages, and
//! reschedules tasks with their dependents. An agent limited to one
//! project sees nothing of the others in schedules and backlinks.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn refused(r: &CallToolResult) {
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

fn titles(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|b| b["title"].as_str().unwrap().to_owned())
        .collect()
}

fn problem_kinds(schedule: &Value) -> Vec<(String, String)> {
    schedule["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["kind"].as_str().unwrap().to_owned(),
                p["task"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn row_keys(schedule: &Value) -> Vec<String> {
    schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn dates_of(schedule: &Value, key: &str) -> (Value, Value) {
    let row = schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .unwrap_or_else(|| panic!("{key} in {schedule}"));
    (row["start_date"].clone(), row["due_date"].clone())
}

#[tokio::test]
async fn an_agent_reads_the_brief_and_follows_links_both_ways() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call("project.create", json!({ "name": "Empty", "key": "EM" }));
        call(
            "task.create",
            json!({ "project": "TM", "title": "Ship", "description": "Follow [[Release plan#Dates]]." }),
        );
        call(
            "wiki.create",
            json!({ "project": "TM", "title": "Release plan",
                    "body": "# Release plan\n\n## Dates\n\nShip TM-1, see [[Conventions]] and [[Missing]].\n" }),
        );
        call(
            "wiki.create",
            json!({ "project": "TM", "title": "Conventions", "body": "Use small commits.\n" }),
        );
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Brief Reader").await;

    // A project with nothing to say has an empty brief; a project with a
    // description but no marked page gives its description.
    let empty = output(&c.call("get_agent_brief", json!({ "project": "EM" })).await);
    assert_eq!(empty["source"], "none", "{empty}");
    assert_eq!(empty["body"], "");
    output(
        &c.call(
            "update_project",
            json!({ "project": "TM", "description": "Ask before deleting." }),
        )
        .await,
    );
    let described = output(&c.call("get_agent_brief", json!({ "project": "TM" })).await);
    assert_eq!(described["source"], "description", "{described}");
    assert_eq!(described["body"], "Ask before deleting.");

    // Only the person marks a page as the brief.
    refused(
        &c.call(
            "update_wiki_page",
            json!({ "project": "TM", "page": "Conventions", "agent_brief": true }),
        )
        .await,
    );
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "wiki.update",
                json!({ "project": "TM", "page": "Conventions", "agent_brief": true }),
            )
            .unwrap();
        engine.close();
    }
    let brief = output(&c.call("get_agent_brief", json!({ "project": "tm" })).await);
    assert_eq!(
        (&brief["source"], &brief["title"], &brief["body"]),
        (
            &json!("wiki_page"),
            &json!("Conventions"),
            &json!("Use small commits.\n")
        ),
        "{brief}"
    );
    // The brief comes from its own tool: reading a task doesn't carry it.
    let task = output(&c.call("get_task", json!({ "keys": ["TM-1"] })).await);
    assert!(
        !task.to_string().contains("Use small commits."),
        "get_task names no brief: {task}"
    );

    // A page's backlinks and references through `include`.
    let page = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "TM", "page": "release-plan", "include": ["backlinks", "references"] }),
        )
        .await,
    );
    assert_eq!(titles(&page["backlinks"]), ["Ship"], "{page}");
    assert_eq!(page["backlinks"][0]["task"], "TM-1");
    assert_eq!(page["backlinks"][0]["heading"], "Dates");
    let refs: Vec<(Value, Value, Value)> = page["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["kind"].clone(), r["target"].clone(), r["found"].clone()))
        .collect();
    assert_eq!(
        refs,
        [
            (json!("page"), json!("Conventions"), json!(true)),
            (json!("page"), json!("Missing"), json!(false)),
            (json!("task"), json!("TM-1"), json!(true)),
        ]
    );
    // Without `include` the page has neither.
    let plain = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "TM", "page": "Release plan" }),
        )
        .await,
    );
    assert!(plain.get("backlinks").is_none() && plain.get("references").is_none());

    // The same for a task.
    let task = output(
        &c.call(
            "get_task",
            json!({ "keys": ["TM-1"], "include": ["backlinks", "references"] }),
        )
        .await,
    );
    let item = &task["items"][0];
    assert_eq!(titles(&item["backlinks"]), ["Release plan"], "{item}");
    assert_eq!(item["references"][0]["kind"], "page");
    assert_eq!(item["references"][0]["title"], "Release plan");
    assert_eq!(item["references"][0]["heading"], "Dates");
    c.close().await;
}

#[tokio::test]
async fn an_agent_moves_deletes_restores_and_exports_pages() {
    let (dir, mut options) = workspace();
    options.toolsets = Some(gk_api::Toolset::ALL.to_vec());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Gardener").await;
    output(
        &c.call("create_project", json!({ "name": "Docs", "key": "DO" }))
            .await,
    );
    for (title, parent, body) in [
        ("Guide", None, "Start with [[Setup]].\n"),
        ("Reference", None, ""),
        ("Setup", Some("Guide"), "Install it.\n"),
        ("Usage", Some("Guide"), "Read [[Guide]].\n"),
    ] {
        let mut input = json!({ "project": "DO", "title": title, "body": body });
        if let Some(p) = parent {
            input["parent"] = json!(p);
        }
        output(&c.call("create_wiki_page", input).await);
    }
    let tree = |list: &Value| -> Vec<String> {
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{}{}",
                    "  ".repeat(usize::try_from(p["depth"].as_u64().unwrap()).unwrap()),
                    p["title"].as_str().unwrap()
                )
            })
            .collect()
    };
    let listed = output(&c.call("list_wiki_pages", json!({ "project": "DO" })).await);
    assert_eq!(tree(&listed), ["Guide", "  Setup", "  Usage", "Reference"]);

    // Into another parent, before a page there; then to the top, first.
    let moved = output(
        &c.call(
            "update_wiki_page",
            json!({ "project": "DO", "page": "Setup", "parent": "Reference" }),
        )
        .await,
    );
    assert_eq!(moved["page"]["title"], "Setup", "{moved}");
    output(
        &c.call(
            "update_wiki_page",
            json!({ "project": "DO", "page": "Usage", "parent": "Reference", "before": "Setup" }),
        )
        .await,
    );
    let listed = output(&c.call("list_wiki_pages", json!({ "project": "DO" })).await);
    assert_eq!(tree(&listed), ["Guide", "Reference", "  Usage", "  Setup"]);
    output(
        &c.call(
            "update_wiki_page",
            json!({ "project": "DO", "page": "Setup", "clear": ["parent"], "at": "top" }),
        )
        .await,
    );
    let listed = output(&c.call("list_wiki_pages", json!({ "project": "DO" })).await);
    assert_eq!(tree(&listed), ["Setup", "Guide", "Reference", "  Usage"]);
    // Never into itself or a page inside it.
    refused(
        &c.call(
            "update_wiki_page",
            json!({ "project": "DO", "page": "Reference", "parent": "Usage" }),
        )
        .await,
    );
    // Links keep working after the moves.
    let guide = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "DO", "page": "Guide", "include": ["references", "backlinks"] }),
        )
        .await,
    );
    assert_eq!(guide["references"][0]["found"], true, "{guide}");
    assert_eq!(titles(&guide["backlinks"]), ["Usage"]);

    // Deleting puts the page and those inside it in the trash.
    let deleted = output(
        &c.call(
            "delete_wiki_page",
            json!({ "project": "DO", "page": "Reference" }),
        )
        .await,
    );
    let entry = &deleted["items"][0];
    assert_eq!(entry["contains"], 1, "{deleted}");
    refused(
        &c.call("get_wiki_page", json!({ "project": "DO", "page": "Usage" }))
            .await,
    );
    let guide = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "DO", "page": "Guide", "include": ["backlinks"] }),
        )
        .await,
    );
    assert_eq!(
        guide["backlinks"],
        json!([]),
        "a page in the trash links nowhere"
    );
    let trash = output(&c.call("list_trash", json!({})).await);
    assert_eq!(trash["items"][0]["title"], "Reference", "{trash}");
    assert_eq!(trash["items"][0]["deleted_by"]["kind"], "agent", "{trash}");
    let restored = output(
        &c.call(
            "restore_from_trash",
            json!({ "items": [entry["id"].clone()] }),
        )
        .await,
    );
    assert_eq!(restored["items"].as_array().unwrap().len(), 1, "{restored}");
    let listed = output(&c.call("list_wiki_pages", json!({ "project": "DO" })).await);
    assert_eq!(tree(&listed), ["Setup", "Guide", "Reference", "  Usage"]);

    // Exports go to the workspace's exports folder under a plain name.
    let zip = output(
        &c.call(
            "export_wiki_page",
            json!({ "project": "DO", "path": "whole-wiki" }),
        )
        .await,
    );
    assert_eq!(
        (&zip["format"], &zip["pages"]),
        (&json!("zip"), &json!(4)),
        "{zip}"
    );
    let zip_path = std::path::PathBuf::from(zip["path"].as_str().unwrap());
    assert!(zip_path.is_file(), "{zip_path:?}");
    assert!(zip_path.starts_with(dir.path()), "{zip_path:?}");
    assert!(
        zip_path
            .parent()
            .is_some_and(|p| p.file_name().is_some_and(|n| n == "exports")),
        "{zip_path:?}"
    );
    let folder = output(
        &c.call(
            "export_wiki_page",
            json!({ "project": "DO", "page": "Reference", "subpages": true,
                    "format": "folder", "path": "reference" }),
        )
        .await,
    );
    assert_eq!(folder["pages"], 2, "{folder}");
    let folder_path = std::path::PathBuf::from(folder["path"].as_str().unwrap());
    let mut files = Vec::new();
    collect(&folder_path, &folder_path, &mut files);
    files.sort();
    assert_eq!(files.len(), 2, "{files:?}");
    assert!(files.iter().all(|f| f.ends_with(".md")), "{files:?}");
    let usage = files.iter().find(|f| f.contains("usage")).unwrap();
    let text = std::fs::read_to_string(folder_path.join(usage)).unwrap();
    assert!(text.contains("Read "), "{text}");
    // An agent can't write anywhere else.
    let outside = dir.path().join("elsewhere.zip");
    refused(
        &c.call(
            "export_wiki_page",
            json!({ "project": "DO", "path": outside.to_string_lossy() }),
        )
        .await,
    );
    assert!(!outside.exists());
    c.close().await;
}

/// Every file under `dir`, relative to `root`, with `/` between parts.
fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).unwrap();
            out.push(
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}

#[tokio::test]
async fn an_agent_reschedules_a_task_with_its_dependents_as_one_change() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Roadmap", "key": "RM" }));
        call(
            "task.create",
            json!({ "project": "RM", "title": "Release", "type": "Milestone", "due_date": "2027-03-01" }),
        );
        for (title, start, due) in [
            ("Design", "2027-01-04", "2027-01-08"),
            ("Build", "2027-01-11", "2027-01-22"),
            ("Test", "2027-01-25", "2027-01-29"),
        ] {
            call(
                "task.create",
                json!({ "project": "RM", "title": title, "start_date": start, "due_date": due }),
            );
        }
        for (from, to) in [("RM-2", "RM-3"), ("RM-3", "RM-4")] {
            call(
                "relation.add",
                json!({ "key": from, "kind": "blocks", "other": to }),
            );
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Planner").await;

    let schedule = output(&c.call("get_schedule", json!({ "project": "RM" })).await);
    assert_eq!(
        row_keys(&schedule),
        ["RM-2", "RM-3", "RM-4", "RM-1"],
        "{schedule}"
    );
    assert_eq!(schedule["tasks"][3]["shape"], "milestone");
    assert_eq!(schedule["dependencies"].as_array().unwrap().len(), 2);
    assert!(problem_kinds(&schedule).is_empty(), "{schedule}");
    // The project or a query must be given.
    refused(&c.call("get_schedule", json!({})).await);

    // Without the option the dependent now starts too early.
    output(
        &c.call(
            "update_task",
            json!({ "key": "RM-2", "due_date": "2027-01-12" }),
        )
        .await,
    );
    let schedule = output(&c.call("get_schedule", json!({ "project": "RM" })).await);
    assert_eq!(
        problem_kinds(&schedule),
        [("starts_before_blocker_ends".to_owned(), "RM-3".to_owned())]
    );

    // With it, the dependents move as far as they must, in one change.
    let changed = output(
        &c.call(
            "update_task",
            json!({ "key": "RM-2", "due_date": "2027-01-14", "move_dependents": true }),
        )
        .await,
    );
    let moved: Vec<(Value, Value)> = changed["moved"]
        .as_array()
        .unwrap_or_else(|| panic!("{changed}"))
        .iter()
        .map(|m| (m["key"].clone(), m["days"].clone()))
        .collect();
    assert_eq!(
        moved,
        [(json!("RM-3"), json!(4)), (json!("RM-4"), json!(2))]
    );
    let schedule = output(&c.call("get_schedule", json!({ "project": "RM" })).await);
    assert!(problem_kinds(&schedule).is_empty(), "{schedule}");
    assert_eq!(
        dates_of(&schedule, "RM-3"),
        (json!("2027-01-15"), json!("2027-01-26"))
    );
    assert_eq!(
        dates_of(&schedule, "RM-4"),
        (json!("2027-01-27"), json!("2027-01-31"))
    );

    // A window narrows the rows.
    let window = output(
        &c.call(
            "get_schedule",
            json!({ "project": "RM", "from": "2027-01-27", "to": "2027-02-10" }),
        )
        .await,
    );
    assert_eq!(row_keys(&window), ["RM-4"], "{window}");

    // The activity log shows one change for the move with its dependents,
    // and reverting it puts every date back.
    let feed = output(
        &c.call("get_activity", json!({ "session": "current" }))
            .await,
    );
    let groups: Vec<Value> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["group"].clone())
        .collect();
    let mut distinct = groups.clone();
    distinct.dedup();
    assert_eq!(distinct.len(), 2, "two changes by the agent: {feed}");
    let last = distinct[0].clone();
    let touched: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["group"] == last)
        .filter_map(|i| i["task"]["key"].as_str())
        .collect();
    for key in ["RM-2", "RM-3", "RM-4"] {
        assert!(touched.contains(&key), "{key} in the last change: {feed}");
    }
    output(&c.call("revert_changes", json!({ "groups": [last] })).await);
    let schedule = output(&c.call("get_schedule", json!({ "project": "RM" })).await);
    assert_eq!(
        dates_of(&schedule, "RM-3"),
        (json!("2027-01-11"), json!("2027-01-22"))
    );
    assert_eq!(
        dates_of(&schedule, "RM-4"),
        (json!("2027-01-25"), json!("2027-01-29"))
    );
    assert_eq!(
        dates_of(&schedule, "RM-2"),
        (json!("2027-01-04"), json!("2027-01-12"))
    );
    c.close().await;
}

#[tokio::test]
async fn a_limited_agent_sees_only_its_project_in_schedules_and_backlinks() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call("project.create", json!({ "name": "Private", "key": "PRV" }));
        call(
            "task.create",
            json!({ "project": "TM", "title": "Mine", "start_date": "2027-01-04", "due_date": "2027-01-08" }),
        );
        call(
            "task.create",
            json!({ "project": "TM", "title": "Mine later", "start_date": "2027-01-11", "due_date": "2027-01-12" }),
        );
        call(
            "task.create",
            json!({ "project": "PRV", "title": "Secret", "start_date": "2027-01-11",
                    "due_date": "2027-01-12", "description": "Waits for TM-1." }),
        );
        call(
            "relation.add",
            json!({ "key": "TM-1", "kind": "blocks", "other": "PRV-1" }),
        );
        call(
            "relation.add",
            json!({ "key": "TM-1", "kind": "blocks", "other": "TM-2" }),
        );
        call(
            "wiki.create",
            json!({ "project": "TM", "title": "Plan", "body": "Mine: TM-1.\n" }),
        );
        call(
            "wiki.create",
            json!({ "project": "PRV", "title": "Notes", "body": "See [[TM:Plan]] and TM-1.\n" }),
        );
        engine.close();
    }
    options.project = Some("TM".to_owned());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scoped Planner").await;

    // A query alone covers the agent's project only.
    let all = output(&c.call("get_schedule", json!({ "query": "is:open" })).await);
    assert_eq!(row_keys(&all), ["TM-1", "TM-2"], "{all}");
    assert!(!all.to_string().contains("PRV"), "{all}");
    let named = output(
        &c.call("get_schedule", json!({ "query": "project:PRV" }))
            .await,
    );
    assert_eq!(row_keys(&named), Vec::<String>::new(), "{named}");
    refused(&c.call("get_schedule", json!({ "project": "PRV" })).await);

    // Backlinks from the other project are not shown.
    let page = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "TM", "page": "Plan", "include": ["backlinks"] }),
        )
        .await,
    );
    assert_eq!(page["backlinks"], json!([]), "{page}");
    let task = output(
        &c.call(
            "get_task",
            json!({ "keys": ["TM-1"], "include": ["backlinks", "links"] }),
        )
        .await,
    );
    assert_eq!(titles(&task["items"][0]["backlinks"]), ["Plan"], "{task}");
    assert!(!task.to_string().contains("PRV"), "{task}");

    // A link to a page of the other project finds nothing for this agent.
    output(
        &c.call(
            "create_wiki_page",
            json!({ "project": "TM", "title": "Links", "body": "See [[PRV:Notes]].\n" }),
        )
        .await,
    );
    let links = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "TM", "page": "Links", "include": ["references"] }),
        )
        .await,
    );
    let reference = &links["references"][0];
    assert_eq!(reference["found"], false, "{links}");
    assert!(
        reference.get("page").is_none() && reference.get("title").is_none(),
        "{links}"
    );

    // Moving the task moves its dependents in this project only.
    let changed = output(
        &c.call(
            "update_task",
            json!({ "key": "TM-1", "due_date": "2027-01-13", "move_dependents": true }),
        )
        .await,
    );
    let moved: Vec<Value> = changed["moved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["key"].clone())
        .collect();
    assert_eq!(moved, [json!("TM-2")], "{changed}");
    assert!(!changed.to_string().contains("PRV"), "{changed}");
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let secret = engine
        .call(&person, "task.get", json!({ "keys": ["PRV-1"] }))
        .unwrap();
    assert_eq!(secret["items"][0]["start_date"], "2027-01-11");
    engine.close();
}
