// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent creates a project and a task and moves it on the board, and the
//! person sees the result with the agent named in history.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

#[tokio::test]
async fn an_agent_creates_and_moves_a_task() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Round Trip").await;

    let p = output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    assert_eq!(p["project"]["key"], "AG");
    let request = json!({ "project": "AG", "title": "Plan the work", "client_request_id": "c-1" });
    let created = output(&c.call("create_task", request.clone()).await);
    assert_eq!(created["task"]["key"], "AG-1");
    assert_eq!(created["task"]["created_by"]["kind"], "agent");
    // A retry with the same request ID answers the same and creates nothing.
    assert_eq!(output(&c.call("create_task", request).await), created);

    let version = created["task"]["version"].as_i64().unwrap();
    let moved = output(
        &c.call(
            "move_task",
            json!({ "key": "ag-1", "status": "In progress", "expected_version": version }),
        )
        .await,
    );
    assert_eq!(moved["task"]["status"]["name"], "In progress");
    let stale = c
        .call(
            "move_task",
            json!({ "key": "AG-1", "status": "Done", "expected_version": version }),
        )
        .await;
    assert_eq!(stale.is_error, Some(true));

    let board = output(&c.call("get_board", json!({ "project": "AG" })).await);
    let doing = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|col| col["name"] == "In progress")
        .unwrap();
    assert_eq!(doing["cards"][0]["key"], "AG-1");

    // Removing a status is permanent: the standard profile can't.
    let removal = c
        .call(
            "update_project",
            json!({ "project": "AG", "statuses": [{ "status": "Review", "remove": true }] }),
        )
        .await;
    assert_eq!(removal.is_error, Some(true));
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let feed = engine
        .call(&person, "history.feed", json!({ "actor": "Round Trip" }))
        .unwrap();
    let ops: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["op"].as_str().unwrap())
        .collect();
    assert!(ops.contains(&"task.move"), "{ops:?}");
    assert_eq!(ops.iter().filter(|o| **o == "task.create").count(), 1);
}

#[tokio::test]
async fn an_agent_edits_a_description_and_ticks_its_checklist() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Checklist Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call(
            "create_task",
            json!({ "project": "AG", "title": "Ship", "description": "Plan\n\n- [ ] build\n- [ ] test\n" }),
        )
        .await,
    );
    let task = output(
        &c.call(
            "get_task",
            json!({ "keys": ["AG-1"], "include": ["checklist", "revisions"] }),
        )
        .await,
    );
    let item = &task["items"][0]["checklist_items"][1];
    assert_eq!(
        (item["number"].clone(), item["text"].clone()),
        (json!(2), json!("test"))
    );
    let base = task["items"][0]["description_revision"].clone();

    let ticked = output(
        &c.call(
            "update_checklist",
            json!({ "key": "AG-1", "toggle": [{ "number": 2, "text": "test", "checked": true }] }),
        )
        .await,
    );
    assert_eq!(ticked["checklist"], json!({ "done": 1, "total": 2 }));

    // A whole description from the old base merges with the tick.
    let saved = output(
        &c.call(
            "update_task",
            json!({ "key": "AG-1", "description": "Plan, revised\n\n- [ ] build\n- [ ] test\n",
                    "base_revision": base }),
        )
        .await,
    );
    assert_eq!(saved["description_save"]["outcome"], "merged");
    assert_eq!(
        saved["task"]["description"],
        "Plan, revised\n\n- [ ] build\n- [x] test\n"
    );
    let revision = saved["description_save"]["revision"].clone();
    let detail = output(
        &c.call(
            "get_description_revision",
            json!({ "revision": revision, "diff_from": base }),
        )
        .await,
    );
    assert!(detail["diff"].as_str().unwrap().contains("+Plan, revised"));
    c.close().await;
}

#[tokio::test]
async fn an_agent_reorders_subtasks_and_reads_one_tasks_history() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Order Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call("create_task", json!({ "project": "AG", "title": "Parent" }))
            .await,
    );
    for title in ["First", "Second"] {
        output(
            &c.call(
                "create_task",
                json!({ "project": "AG", "title": title, "parent": "AG-1" }),
            )
            .await,
        );
    }
    let reordered = output(
        &c.call(
            "reorder_subtask",
            json!({ "key": "AG-3", "before": "AG-2" }),
        )
        .await,
    );
    assert_eq!(reordered["task"]["key"], "AG-1");
    assert_eq!(reordered["task"]["subtasks"][0]["key"], "AG-3");
    assert_eq!(reordered["task"]["subtasks"][1]["key"], "AG-2");

    let feed = output(&c.call("get_activity", json!({ "task": "AG-3" })).await);
    let ops: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["task.reorder_subtask", "task.create"]);
    c.close().await;
}

#[tokio::test]
async fn an_agent_logs_time_runs_the_timer_and_sets_a_reminder() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Time Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call("create_task", json!({ "project": "AG", "title": "Parser" }))
            .await,
    );
    let request = json!({ "key": "AG-1", "duration": "1h 30m", "date": "2026-11-23",
                          "note": "Wrote it", "client_request_id": "t-1" });
    let logged = output(&c.call("log_time", request.clone()).await);
    assert_eq!(logged["spent_minutes"], 90);
    assert_eq!(logged["entry"]["author"]["kind"], "agent");
    // A retry logs nothing more.
    assert_eq!(output(&c.call("log_time", request).await), logged);

    let report = output(
        &c.call(
            "get_time_report",
            json!({ "from": "2026-11-23", "to": "2026-11-23" }),
        )
        .await,
    );
    assert_eq!(report["items"][0]["key"], "AG-1");
    assert_eq!(report["total_duration"], "1h 30m");

    let started = output(&c.call("start_timer", json!({ "key": "AG-1" })).await);
    assert_eq!(started["timer"]["started_by"]["name"], "Time Agent");
    let overview = output(&c.call("get_workspace", json!({})).await);
    assert_eq!(overview["timer"]["task"]["key"], "AG-1");
    let stopped = output(&c.call("stop_timer", json!({ "discard": true })).await);
    assert!(stopped.get("timer").is_none());

    let set = output(
        &c.call(
            "set_reminder",
            json!({ "key": "AG-1", "at": "2020-01-01T09:00:00Z" }),
        )
        .await,
    );
    assert_eq!(set["reminder"]["at"], "2020-01-01T09:00:00.000Z");
    let due = output(&c.call("get_reminders", json!({})).await);
    assert_eq!(due["items"][0]["kind"], "reminder");
    let id = due["items"][0]["id"].clone();
    let ack = output(
        &c.call("acknowledge_reminders", json!({ "ids": [id] }))
            .await,
    );
    assert_eq!(ack["acknowledged"].as_array().unwrap().len(), 1);
    c.close().await;
}

#[tokio::test]
async fn an_agent_searches_changes_in_bulk_and_saves_a_view() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Searcher").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    for title in ["Plan the work", "Do the work", "Review the work"] {
        output(
            &c.call("create_task", json!({ "project": "AG", "title": title }))
                .await,
        );
    }

    let found = output(
        &c.call(
            "search_tasks",
            json!({ "query": "work -review sort:-key", "limit": 1 }),
        )
        .await,
    );
    assert_eq!(found["total"], 2);
    assert_eq!(found["items"][0]["key"], "AG-2");
    assert_eq!(found["next_cursor"], "1");
    let wrong = c
        .call("search_tasks", json!({ "query": "stauts:Done" }))
        .await;
    assert_eq!(wrong.is_error, Some(true));

    let preview = output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "project:AG", "changes": { "priority": "high" } }),
        )
        .await,
    );
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["count"], 3);
    let done = output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "project:AG", "changes": { "priority": "high" },
                    "dry_run": false, "expected_count": 3 }),
        )
        .await,
    );
    assert_eq!(done["changed"], 3);
    let group = done["group"].as_str().unwrap().to_owned();
    let reverted = output(&c.call("revert_changes", json!({ "groups": [group] })).await);
    assert!(
        reverted["reverted"]
            .as_array()
            .is_some_and(|r| !r.is_empty()),
        "{reverted}"
    );
    let high = output(
        &c.call("search_tasks", json!({ "query": "priority:high" }))
            .await,
    );
    assert_eq!(high["total"], 0);

    let saved = output(
        &c.call(
            "create_view",
            json!({ "name": "Open work", "project": "AG", "query": "is:open" }),
        )
        .await,
    );
    assert_eq!(saved["view"]["name"], "Open work");
    let views = output(&c.call("list_views", json!({ "project": "AG" })).await);
    assert_eq!(views["total"], 1);
    output(
        &c.call(
            "delete_view",
            json!({ "view": "Open work", "project": "AG" }),
        )
        .await,
    );
    c.close().await;
}

#[tokio::test]
async fn an_agent_keeps_a_wiki_page_and_searches_everything() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Writer").await;
    output(
        &c.call("create_project", json!({ "name": "Docs", "key": "DOC" }))
            .await,
    );
    let made = output(
        &c.call(
            "create_wiki_page",
            json!({ "project": "DOC", "title": "Style guide",
                    "body": "# Headings\n\nUse sentence case.\n",
                    "client_request_id": "w-1" }),
        )
        .await,
    );
    assert_eq!(made["page"]["slug"], "style-guide");
    let taken = c
        .call(
            "create_wiki_page",
            json!({ "project": "DOC", "title": "STYLE GUIDE" }),
        )
        .await;
    assert_eq!(taken.is_error, Some(true));
    let edited = output(
        &c.call(
            "update_wiki_page",
            json!({ "project": "DOC", "page": "style guide",
                    "edits": [{ "old_text": "sentence case", "new_text": "sentence case, always" }] }),
        )
        .await,
    );
    assert_eq!(edited["save"]["outcome"], "saved");
    let page = output(
        &c.call(
            "get_wiki_page",
            json!({ "project": "DOC", "page": "Style guide" }),
        )
        .await,
    );
    assert!(page["body"].as_str().unwrap().contains("always"));
    let pages = output(&c.call("list_wiki_pages", json!({ "project": "DOC" })).await);
    assert_eq!(pages["total"], 1);
    let headings = output(
        &c.call("search_wiki_pages", json!({ "text": "style#head" }))
            .await,
    );
    assert_eq!(headings["items"][0]["heading"]["text"], "Headings");
    let history = output(
        &c.call(
            "get_wiki_history",
            json!({ "project": "DOC", "page": "Style guide" }),
        )
        .await,
    );
    assert_eq!(history["total"], 2);
    output(
        &c.call(
            "create_task",
            json!({ "project": "DOC", "title": "Apply the style guide", "due_date": "in 3d" }),
        )
        .await,
    );
    let found = output(&c.call("search", json!({ "text": "style" })).await);
    assert_eq!(found["total"], 2);
    let mut kinds: Vec<&str> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["kind"].as_str().unwrap())
        .collect();
    kinds.sort_unstable();
    assert_eq!(kinds, ["task", "wiki"]);
    let today = output(&c.call("get_today", json!({})).await);
    assert_eq!(today["counts"]["recent"], 1);
    c.close().await;
}

#[tokio::test]
async fn an_agent_deletes_within_its_limits_and_the_person_restores() {
    let (_dir, mut options) = workspace();
    options.toolsets = Some(gk_api::Toolset::ALL.to_vec());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Tidy").await;
    output(
        &c.call("create_project", json!({ "name": "Tidy", "key": "TD" }))
            .await,
    );
    let mut keys = Vec::new();
    for i in 0..21 {
        let t = output(
            &c.call(
                "create_task",
                json!({ "project": "TD", "title": format!("T{i}") }),
            )
            .await,
        );
        keys.push(t["task"]["key"].as_str().unwrap().to_owned());
    }
    let too_many = c.call("delete_tasks", json!({ "keys": keys })).await;
    assert_eq!(too_many.is_error, Some(true), "{too_many:?}");
    let deleted = c.call("delete_tasks", json!({ "keys": keys[..2] })).await;
    let out = output(&deleted);
    assert_eq!(out["total"], 2);
    assert!(common::text(&deleted).starts_with("Moved to the trash: 2 items"));
    let listed = c.call("list_trash", json!({})).await;
    assert_eq!(output(&listed)["total"], 2);
    assert!(common::text(&listed).contains("TD-1"));
    // Emptying the whole trash is the person's.
    let all = c.call("purge_trash", json!({ "all": true })).await;
    assert_eq!(all.is_error, Some(true));
    // Archives stay in the workspace's exports folder.
    let outside = c.call("export_data", json!({ "path": "../out.zip" })).await;
    assert_eq!(outside.is_error, Some(true));
    let exported = output(&c.call("export_data", json!({ "path": "td.zip" })).await);
    // The archive carries the trash too.
    assert_eq!(exported["counts"]["tasks"], 21, "{exported}");
    let check = output(
        &c.call("import_data", json!({ "path": "td.zip", "dry_run": true }))
            .await,
    );
    assert_eq!(check["taken_keys"], json!(["TD"]));
    let made = c.call("create_backup", json!({})).await;
    assert!(common::text(&made).starts_with("Backup made:"), "{made:?}");
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let trash = engine.call(&person, "trash.list", json!({})).unwrap();
    assert_eq!(trash["items"][0]["deleted_by"]["kind"], "agent");
    let ids: Vec<Value> = trash["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone())
        .collect();
    let back = engine
        .call(&person, "trash.restore", json!({ "items": ids }))
        .unwrap();
    assert_eq!(back["total"], 2);
}

#[tokio::test]
async fn the_person_sets_the_limit_on_changes_a_minute_while_the_agent_works() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let (engine, person) = open_as_person(&options);
    let c = connect(&options, mode, version, "Busy").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );

    // A limit set now counts the change already made in this minute.
    engine
        .call(
            &person,
            "agent.limits.update",
            json!({ "writes_per_minute": 3 }),
        )
        .unwrap();
    for i in 0..2 {
        output(
            &c.call(
                "create_task",
                json!({ "project": "AG", "title": format!("T{i}") }),
            )
            .await,
        );
    }
    let refused = c
        .call(
            "create_task",
            json!({ "project": "AG", "title": "Too fast" }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    let err = common::text(&refused);
    assert!(err.starts_with("Error (limit)"), "{err}");
    assert!(err.contains("Most allowed: 3"), "{err}");
    // Reading goes on.
    output(&c.call("get_workspace", json!({})).await);

    // Without a limit, the agent goes on at once.
    engine
        .call(
            &person,
            "agent.limits.update",
            json!({ "no_limit": ["writes_per_minute"] }),
        )
        .unwrap();
    let workspace = output(&c.call("get_workspace", json!({})).await);
    assert!(
        workspace["agent_limits"].get("writes_per_minute").is_none(),
        "{workspace}"
    );
    for i in 0..5 {
        output(
            &c.call(
                "create_task",
                json!({ "project": "AG", "title": format!("U{i}") }),
            )
            .await,
        );
    }
    c.close().await;
}

#[tokio::test]
async fn an_agent_reads_the_flow_reports_and_its_own_closes() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Flow Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    for title in ["One", "Two"] {
        output(
            &c.call("create_task", json!({ "project": "AG", "title": title }))
                .await,
        );
    }
    output(
        &c.call("move_task", json!({ "key": "AG-1", "status": "Done" }))
            .await,
    );
    let input = json!({ "project": "AG", "bucket": "day", "from": "today" });
    let created = output(&c.call("get_created_closed_report", input.clone()).await);
    assert_eq!(created["created"], 2);
    assert_eq!(created["closed"], 1);
    let mine = output(
        &c.call(
            "get_created_closed_report",
            json!({ "bucket": "day", "from": "today", "actor": "agent" }),
        )
        .await,
    );
    assert_eq!(mine["closed"], 1, "the agent closed it");
    let people = output(
        &c.call(
            "get_created_closed_report",
            json!({ "bucket": "day", "from": "today", "actor": "me" }),
        )
        .await,
    );
    assert_eq!(people["closed"], 0);
    let flow = output(&c.call("get_cumulative_flow", input.clone()).await);
    assert_eq!(flow["items"].as_array().unwrap().len(), 1);
    let cycle = output(&c.call("get_cycle_time", input).await);
    assert_eq!(cycle["lead"]["count"], 1);
    assert_eq!(cycle["cycle"]["count"], 0, "never in progress");
    c.close().await;
}

#[tokio::test]
async fn an_agent_reads_velocity_burnup_and_estimate_accuracy() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Plan Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Plans", "key": "PL" }))
            .await,
    );
    output(
        &c.call("create_version", json!({ "project": "PL", "name": "1.0" }))
            .await,
    );
    for title in ["One", "Two"] {
        output(
            &c.call(
                "create_task",
                json!({ "project": "PL", "title": title, "estimate": "1h", "fix_version": "1.0" }),
            )
            .await,
        );
    }
    output(
        &c.call(
            "create_sprint",
            json!({ "project": "PL", "name": "S1", "tasks": ["PL-1", "PL-2"] }),
        )
        .await,
    );
    output(
        &c.call("start_sprint", json!({ "sprint": "S1", "project": "PL" }))
            .await,
    );
    output(
        &c.call("log_time", json!({ "key": "PL-1", "duration": "90m" }))
            .await,
    );
    output(
        &c.call("move_task", json!({ "key": "PL-1", "status": "Done" }))
            .await,
    );
    output(
        &c.call(
            "complete_sprint",
            json!({ "sprint": "active", "project": "PL" }),
        )
        .await,
    );
    let velocity = output(&c.call("get_velocity", json!({ "project": "PL" })).await);
    assert_eq!(velocity["sprints"][0]["committed"], 2.0);
    assert_eq!(velocity["sprints"][0]["completed"], 1.0);
    assert_eq!(velocity["average"], 1.0);
    let burnup = output(
        &c.call("get_burnup", json!({ "version": "1.0", "project": "PL" }))
            .await,
    );
    let today = &burnup["days"][0];
    assert_eq!(
        (today["scope"].clone(), today["completed"].clone()),
        (json!(2.0), json!(1.0))
    );
    let accuracy = output(
        &c.call(
            "get_estimate_accuracy",
            json!({ "project": "PL", "group_by": "type" }),
        )
        .await,
    );
    assert_eq!(accuracy["total"]["count"], 1);
    assert_eq!(accuracy["total"]["median_ratio"], 1.5);
    assert_eq!(accuracy["without_time"], 0);
    c.close().await;
}

#[tokio::test]
async fn an_agent_reads_and_sets_board_swimlanes_and_moves_a_card_between_lanes() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Lanes").await;
    output(
        &c.call("create_project", json!({ "name": "Lanes", "key": "LN" }))
            .await,
    );
    output(
        &c.call(
            "create_task",
            json!({ "project": "LN", "title": "Crash", "priority": "urgent" }),
        )
        .await,
    );

    let board = output(
        &c.call("get_board", json!({ "project": "LN", "lanes": "priority" }))
            .await,
    );
    let ids: Vec<&str> = board["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["urgent", "high", "normal", "low", "none"]);
    assert_eq!(board["lanes"][0]["count"], 1);
    assert_eq!(board["columns"][0]["cards"][0]["lane"], "urgent");

    let set = output(
        &c.call(
            "set_board_layout",
            json!({ "project": "LN", "lanes": "priority" }),
        )
        .await,
    );
    assert_eq!(set["layout"]["lanes"], "priority");

    let moved = output(
        &c.call(
            "move_task",
            json!({ "key": "LN-1", "status": "In progress", "changes": { "priority": "low" } }),
        )
        .await,
    );
    assert_eq!(moved["task"]["priority"], "low");
    assert_eq!(moved["task"]["status"]["name"], "In progress");
    let board = output(&c.call("get_board", json!({ "project": "LN" })).await);
    let low = board["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == "low")
        .unwrap();
    assert_eq!(low["count"], 1);
    assert_eq!(board["lanes"][4]["empty"], true);

    // The board across projects: a column per category, lanes by project,
    // each card with its real status; a move goes to the column's status.
    output(
        &c.call("create_project", json!({ "name": "Other", "key": "OT" }))
            .await,
    );
    output(
        &c.call(
            "create_task",
            json!({ "project": "OT", "title": "Elsewhere" }),
        )
        .await,
    );
    let across = output(&c.call("get_board_across", json!({})).await);
    let ids: Vec<&str> = across["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|col| col["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["todo", "in_progress", "done"]);
    let lanes: Vec<&str> = across["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(lanes, ["LN", "OT"]);
    assert_eq!(across["columns"][1]["cards"][0]["key"], "LN-1");
    assert_eq!(
        across["columns"][1]["cards"][0]["status"]["name"],
        "In progress"
    );
    let status = across["columns"][1]["statuses"]["OT"].clone();
    let moved = output(
        &c.call("move_task", json!({ "key": "OT-1", "status": status }))
            .await,
    );
    assert_eq!(moved["task"]["status"]["name"], "In progress");
    let mine = output(
        &c.call(
            "get_board_across",
            json!({ "query": "project:OT", "lanes": "none" }),
        )
        .await,
    );
    assert_eq!(mine["columns"][1]["cards"][0]["key"], "OT-1");
    assert!(mine.get("lanes").is_none());
    c.close().await;
}
