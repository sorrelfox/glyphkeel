// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The project templates that come with Glyphkeel.
//!
//! They aren't stored in the workspace: every workspace lists them, they
//! can be read and used like any project template, and they can't be
//! changed or deleted. A workspace's own template of the same name wins
//! when a template is named. Their content is data, like the statuses a
//! new project starts with, so it is written in English; the app shows
//! their name and description in the person's language by their `key`.

use gk_api::ops::common::{Category, Glyph, Hue, Priority};
use gk_api::ops::rules::StatusRules;
use gk_api::ops::template::{
    LabelBlueprint, NamedBlueprint, PageBlueprint, ProjectBlueprint, StatusBlueprint,
    TaskBlueprint, TemplateKind, TypeBlueprint,
};
use gk_core::Id;

/// A template that comes with the program.
#[derive(Debug, Clone)]
pub(crate) struct BuiltIn {
    /// Its fixed ID, the same in every workspace.
    pub(crate) id: Id,
    /// The app's key for its translated name and description.
    pub(crate) key: &'static str,
    pub(crate) kind: TemplateKind,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) setup: ProjectBlueprint,
}

/// The ID of the "Software project" template.
pub(crate) const SOFTWARE_ID: Id = Id::from_bytes([
    0x01, 0x9a, 0xb0, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
]);

/// The name of the creator the built-in templates show.
pub(crate) const CREATOR: &str = "Glyphkeel";

/// Every built-in template, in the order lists show them.
pub(crate) fn all() -> Vec<BuiltIn> {
    vec![software()]
}

/// The built-in template `id`, if it is one.
pub(crate) fn get(id: Id) -> Option<BuiltIn> {
    all().into_iter().find(|t| t.id == id)
}

fn status(name: &str, category: Category, hue: Hue, wip_limit: Option<u32>) -> StatusBlueprint {
    StatusBlueprint {
        name: name.to_owned(),
        category,
        hue,
        wip_limit,
        wip_min: None,
        rules: StatusRules::default(),
    }
}

fn kind(name: &str, glyph: Glyph, hue: Hue, is_default: bool) -> TypeBlueprint {
    TypeBlueprint {
        name: name.to_owned(),
        glyph,
        hue,
        is_default,
        due_offset_days: None,
    }
}

fn label(name: &str, hue: Hue) -> LabelBlueprint {
    LabelBlueprint {
        name: name.to_owned(),
        hue,
    }
}

const OVERVIEW: &str = "\
## Goal

What the software is for, and for whom.

## Scope

- In:
- Out:

## Links

- Repository:
- Documentation:
";

const RELEASES: &str = "\
## Next release

- Version:
- Planned for:

## Earlier releases
";

const STORY: &str = "\
As a …, I want … so that ….

## Acceptance criteria

- [ ]
";

const BUG: &str = "\
## Steps to reproduce

1.

## Expected

## Actual

## Version and system
";

/// A project for building software: a backlog, a board with WIP limits and
/// a review step, epics, stories, tasks and bugs.
fn software() -> BuiltIn {
    let done_rules = StatusRules {
        stop_timer: true,
        release_claim: true,
        agents_may_not_enter: true,
        ..StatusRules::default()
    };
    let mut done = status("Done", Category::Done, Hue::Green, None);
    done.rules = done_rules;
    let setup = ProjectBlueprint {
        statuses: vec![
            status("Backlog", Category::Todo, Hue::Grey, None),
            status("To do", Category::Todo, Hue::Blue, None),
            status("In progress", Category::InProgress, Hue::Orange, Some(5)),
            status("In review", Category::InProgress, Hue::Purple, Some(3)),
            done,
            status("Won't do", Category::Cancelled, Hue::Grey, None),
        ],
        types: vec![
            kind("Story", Glyph::Story, Hue::Green, true),
            kind("Task", Glyph::Task, Hue::Blue, false),
            kind("Bug", Glyph::Bug, Hue::Red, false),
            kind("Epic", Glyph::Epic, Hue::Purple, false),
        ],
        labels: vec![
            label("Frontend", Hue::Teal),
            label("Backend", Hue::Blue),
            label("Documentation", Hue::Yellow),
        ],
        wiki: vec![
            PageBlueprint {
                title: "Overview".to_owned(),
                body: OVERVIEW.to_owned(),
                parent: None,
            },
            PageBlueprint {
                title: "Releases".to_owned(),
                body: RELEASES.to_owned(),
                parent: None,
            },
        ],
        task_templates: vec![
            NamedBlueprint {
                name: "Story".to_owned(),
                description: "A user story with its acceptance criteria.".to_owned(),
                task: TaskBlueprint {
                    title: "Story".to_owned(),
                    description: STORY.to_owned(),
                    task_type: Some("Story".to_owned()),
                    ..TaskBlueprint::default()
                },
            },
            NamedBlueprint {
                name: "Bug report".to_owned(),
                description: "A bug with the steps that show it.".to_owned(),
                task: TaskBlueprint {
                    title: "Bug".to_owned(),
                    description: BUG.to_owned(),
                    task_type: Some("Bug".to_owned()),
                    priority: Priority::High,
                    ..TaskBlueprint::default()
                },
            },
        ],
        ..ProjectBlueprint::default()
    };
    BuiltIn {
        id: SOFTWARE_ID,
        key: "software",
        kind: TemplateKind::Project,
        name: "Software project",
        description: "Epics, stories, tasks and bugs on a board from the backlog through review to done.",
        setup,
    }
}
