// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What a wiki page's macros show: its subpages, the pages it includes and
//! the tasks its queries find, read in the caller's transaction.
//!
//! Everything goes through the ordinary lookups, so an agent limited to one
//! project sees only that project's pages and tasks here too. A `{{tasks}}`
//! query that names no project looks in the page's project and the
//! projects below it.

use gk_api::ops::common::Category;
use gk_core::Id;
use gk_markdown::{Included, MacroSource, NoteMessage, PageItem, TaskItem, TaskList};
use gk_query::check::NameKind;
use gk_query::{Compiled, Scope, SqlEnv};
use gk_store::rusqlite::Transaction;

use super::description::{self, Owner};
use super::lookup;
use super::wiki::{self, PageRow};
use crate::time::Zone;

/// The most tasks a `{{tasks}}` macro lists; a note says how many more
/// match.
pub(crate) const MAX_TASKS: usize = 50;

/// The macros of page `root`'s text, or of a text shown as that page's.
pub(crate) struct WikiMacros<'a> {
    tx: &'a Transaction<'a>,
    zone: &'a Zone,
    now: i64,
    root: PageRow,
    root_id: String,
}

impl<'a> WikiMacros<'a> {
    pub(crate) fn new(tx: &'a Transaction<'a>, zone: &'a Zone, now: i64, root: PageRow) -> Self {
        Self {
            tx,
            zone,
            now,
            root_id: root.id.to_string(),
            root,
        }
    }

    /// The page a macro's text belongs to: the root, or an included page.
    fn page_of(&self, page: Option<&str>) -> Option<PageRow> {
        match page {
            None => Some(self.root.clone()),
            Some(id) => {
                let id: Id = id.parse().ok()?;
                wiki::by_id(self.tx, id).ok().flatten()
            }
        }
    }

    fn project_key(&self, project: Id) -> Option<String> {
        lookup::project_by_id(self.tx, project)
            .ok()
            .flatten()
            .map(|p| p.key)
    }
}

impl MacroSource for WikiMacros<'_> {
    fn page(&self) -> Option<&str> {
        Some(&self.root_id)
    }

    fn children(&self, page: Option<&str>) -> Vec<PageItem> {
        let Some(page) = self.page_of(page) else {
            return Vec::new();
        };
        let Ok(kids) = wiki::children(self.tx, page.project, Some(page.id)) else {
            return Vec::new();
        };
        // Links resolve in the root page's project; a subpage of a page
        // included from another project is linked with its project's key.
        let prefix = (page.project != self.root.project)
            .then(|| self.project_key(page.project))
            .flatten();
        kids.into_iter()
            .map(|kid| PageItem {
                target: match &prefix {
                    Some(key) => format!("{key}:{}", kid.title),
                    None => kid.title.clone(),
                },
                title: kid.title,
            })
            .collect()
    }

    fn include(&self, page: Option<&str>, name: &str) -> Option<Included> {
        let from = self.page_of(page)?;
        let found = super::links::resolve_page(self.tx, from.project, name)
            .ok()
            .flatten()?;
        let (text, _) = description::current_text(self.tx, Owner::Wiki(found.id)).ok()?;
        Some(Included {
            id: found.id.to_string(),
            text,
        })
    }

    fn tasks(&self, page: Option<&str>, query: &str) -> Result<TaskList, NoteMessage> {
        let from = self.page_of(page).ok_or_else(unreadable)?;
        let compiled = super::query::try_compile(self.tx, self.zone, self.now, query)
            .map_err(|_| unreadable())?;
        let (_, checked) = compiled.map_err(|e| {
            e.args
                .iter()
                .fold(NoteMessage::new(e.id), |message, (name, value)| {
                    message.arg(*name, value.clone())
                })
        })?;
        let names_project = checked.names().iter().any(|n| n.kind == NameKind::Project);
        let scope = if names_project {
            None
        } else {
            self.project_key(from.project).map(|project| Scope {
                project,
                subprojects: true,
            })
        };
        let compiled = Compiled::new(
            &checked,
            &SqlEnv {
                now: self.now,
                scope,
                default_sort: Vec::new(),
            },
        );
        let compiled =
            super::query::with_schedule(self.tx, &checked, compiled).map_err(|_| unreadable())?;
        let ids = super::query::matching_ids(self.tx, &compiled).map_err(|_| unreadable())?;
        let mut items = Vec::with_capacity(ids.len().min(MAX_TASKS));
        for id in ids.iter().take(MAX_TASKS) {
            let task = super::query::listed(self.tx, *id, self.now).map_err(|_| unreadable())?;
            items.push(TaskItem {
                key: task.key,
                title: task.title,
                done: matches!(task.status.category, Category::Done | Category::Cancelled),
                status: task.status.name,
            });
        }
        Ok(TaskList {
            items,
            total: ids.len(),
        })
    }
}

/// The note for a list the engine failed to read: the failure itself is
/// the engine's own business, not the reader's.
fn unreadable() -> NoteMessage {
    NoteMessage::new("wiki-macro-tasks-unreadable")
}
