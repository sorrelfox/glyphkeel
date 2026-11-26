// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Recurring tasks and templates from the command line, each command its
//! own process against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;

#[test]
fn a_rule_in_words_makes_a_task_recurring_and_closing_it_makes_the_next() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Home", "--key", "HM"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "HM",
        "--title",
        "Bins",
        "--due-date",
        "2030-01-07",
    ]);
    let set = ws.json(&[
        "recurrence",
        "set",
        "HM-1",
        "--text",
        "every 2 weeks on mon",
    ]);
    assert_eq!(
        set["recurrence"]["summary"], "Every 2 weeks on Monday",
        "{set}"
    );
    assert_eq!(set["recurrence"]["next_date"], "2030-01-21");
    let shown = ws.ok(&[
        "recurrence",
        "preview",
        "--text",
        "monthly on the last fri",
        "--limit",
        "2",
    ]);
    assert!(shown.contains("Every month on the last Friday"), "{shown}");
    ws.ok(&["-q", "task", "move", "HM-1", "--status", "Done"]);
    let next = ws.json(&["task", "show", "HM-2"]);
    assert_eq!(next["items"][0]["due_date"], "2030-01-21", "{next}");
    let skipped = ws.json(&["recurrence", "skip", "HM-2"]);
    assert_eq!(skipped["due_date"], "2030-02-04");
    let err = ws.fails(1, &["recurrence", "set", "HM-2", "--text", "now and then"]);
    assert!(err.contains("is not a rule"), "{err}");
}

#[test]
fn templates_are_made_from_tasks_and_used() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Home", "--key", "HM"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "HM",
        "--title",
        "Trip",
        "--description=- [x] tickets\n- [ ] bags\n",
    ]);
    ws.ok(&["-q", "template", "create", "Packing", "--from-task", "HM-1"]);
    let list = ws.ok(&["template", "list"]);
    assert!(list.contains("Packing"), "{list}");
    let used = ws.json(&[
        "template",
        "use",
        "Packing",
        "--project",
        "HM",
        "--title",
        "Weekend away",
    ]);
    assert_eq!(used["task"]["title"], "Weekend away", "{used}");
    assert_eq!(used["task"]["description"], "- [ ] tickets\n- [ ] bags\n");
}
