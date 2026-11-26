// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki macros in the reader's HTML.

#![allow(clippy::unwrap_used)]

mod common;

use std::collections::HashMap;

use common::violations;
use gk_markdown::{
    Included, Macro, MacroSource, Note, PageItem, RenderOptions, TaskItem, TaskList, checklist,
    macros, render,
};

/// Pages by ID, each with its text and subpages.
struct Wiki {
    root: &'static str,
    pages: HashMap<&'static str, (&'static str, Vec<&'static str>)>,
}

impl Wiki {
    fn new(root: &'static str, pages: &[(&'static str, &'static str, &[&'static str])]) -> Self {
        Self {
            root,
            pages: pages
                .iter()
                .map(|(id, text, kids)| (*id, (*text, kids.to_vec())))
                .collect(),
        }
    }
}

impl MacroSource for Wiki {
    fn page(&self) -> Option<&str> {
        Some(self.root)
    }

    fn children(&self, page: Option<&str>) -> Vec<PageItem> {
        let id = page.unwrap_or(self.root);
        self.pages
            .get(id)
            .map(|(_, kids)| {
                kids.iter()
                    .map(|k| PageItem {
                        target: (*k).to_owned(),
                        title: (*k).to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn include(&self, _page: Option<&str>, name: &str) -> Option<Included> {
        self.pages.get(name).map(|(text, _)| Included {
            id: name.to_owned(),
            text: (*text).to_owned(),
        })
    }

    fn tasks(&self, _page: Option<&str>, query: &str) -> Result<TaskList, String> {
        match query {
            "bad" => Err("Unknown field “bad”.".to_owned()),
            "none" => Ok(TaskList::default()),
            _ => Ok(TaskList {
                items: vec![
                    TaskItem {
                        key: "TM-1".into(),
                        title: "Write <notes>".into(),
                        status: "Open".into(),
                        done: false,
                    },
                    TaskItem {
                        key: "TM-2".into(),
                        title: "Tag".into(),
                        status: "Done".into(),
                        done: true,
                    },
                ],
                total: 5,
            }),
        }
    }
}

fn wiki() -> Wiki {
    Wiki::new(
        "home",
        &[
            ("home", "", &["Plan", "Notes"]),
            ("Plan", "## Steps\n\n- [ ] ship\n\n{{include Notes}}\n", &[]),
            ("Notes", "Notes text[^a].\n\n[^a]: A note.\n", &[]),
            ("Loop", "{{include Loop}}\n", &[]),
            ("Back", "{{include home}}\n", &[]),
            ("A", "{{include B}}\n", &[]),
            ("B", "{{include C}}\n", &[]),
            ("C", "{{include D}}\n", &[]),
            ("D", "deep\n", &[]),
        ],
    )
}

fn html(source: &str) -> String {
    let wiki = wiki();
    let out = render(source, &RenderOptions::new("wiki-home").with_macros(&wiki));
    assert!(violations(&out).is_empty(), "{:?}: {out}", violations(&out));
    out.replace(" rel=\"noopener noreferrer\"", "")
}

#[test]
fn a_macro_is_a_line_of_its_own() {
    let found = macros(
        "{{toc}}\n\ntext {{toc}}\n\n`{{toc}}`\n\n\\{{toc}}\n\n```\n{{toc}}\n```\n\n> {{children}}\n\n- {{include Plan}}\n\n{{TASKS status:open}}\n\n{{nope}}\n",
    );
    let kinds: Vec<_> = found.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(
        kinds,
        [
            Macro::Toc,
            Macro::Children,
            Macro::Include("Plan".into()),
            Macro::Tasks("status:open".into())
        ]
    );
    assert_eq!(found[0].0, 1);
}

#[test]
fn macros_stay_text_without_a_source() {
    let out = render("{{toc}}\n", &RenderOptions::new("d"));
    assert_eq!(out, "<p>{{toc}}</p>\n");
    // Unknown macros and macros inside a sentence stay text with one.
    let out = html("{{weather}}\n\nsee {{toc}}\n");
    assert!(!out.contains("data-macro"), "{out}");
    assert!(
        out.contains("{{weather}}") && out.contains("see {{toc}}"),
        "{out}"
    );
}

#[test]
fn toc_lists_the_headings_as_links_nested_by_level() {
    let out = html("# One\n\n{{toc}}\n\n## Two *a*\n\n### Three\n\n## Four\n");
    let toc = &out[out.find("data-macro=\"toc\"").unwrap()..];
    assert!(
        toc.starts_with(
            "data-macro=\"toc\"><ul><li><a data-wiki=\"#One\">One</a><ul><li><a data-wiki=\"#Two a\">Two a</a><ul><li><a data-wiki=\"#Three\">Three</a></li></ul></li><li><a data-wiki=\"#Four\">Four</a></li></ul></li></ul></div>"
        ),
        "{out}"
    );
    let empty = html("{{toc}}\n");
    assert!(
        empty.contains("<p data-macro-note=\"toc-empty\"></p>"),
        "{empty}"
    );
}

#[test]
fn children_link_the_subpages() {
    let out = html("{{children}}\n");
    assert!(
        out.contains("<ul><li><a data-wiki=\"Plan\">Plan</a></li><li><a data-wiki=\"Notes\">Notes</a></li></ul>"),
        "{out}"
    );
    let wiki = Wiki::new("x", &[("x", "", &[])]);
    let out = render(
        "{{children}}\n",
        &RenderOptions::new("d").with_macros(&wiki),
    );
    assert!(out.contains("data-macro-note=\"children-empty\""), "{out}");
}

#[test]
fn include_renders_the_page_with_checkboxes_that_cannot_be_ticked() {
    let source = "- [ ] mine\n\n{{include Plan}}\n\n- [ ] mine too\n";
    let out = html(source);
    assert!(out.contains("<h2>Steps</h2>"), "{out}");
    assert!(out.contains("Notes text"), "nested include: {out}");
    // The page's own checkboxes keep their numbers; the included one has none.
    assert!(out.contains("data-item=\"1\""), "{out}");
    assert!(out.contains("data-item=\"2\""), "{out}");
    assert!(!out.contains("data-item=\"3\""), "{out}");
    assert!(
        out.contains("<input type=\"checkbox\" disabled=\"\""),
        "{out}"
    );
    assert_eq!(checklist(source).len(), 2);
    // Footnotes of an included page get ids of their own.
    assert!(out.contains("id=\"fn-wiki_home-i1-i2-a\""), "{out}");
}

#[test]
fn include_refuses_loops_depth_and_missing_pages() {
    let out = html("{{include Loop}}\n");
    assert!(
        out.contains("<p data-macro-note=\"include-loop\" data-macro-arg=\"Loop\"></p>"),
        "{out}"
    );
    let out = html("{{include Back}}\n");
    assert!(
        out.contains("data-macro-note=\"include-loop\" data-macro-arg=\"home\""),
        "{out}"
    );
    let out = html("{{include A}}\n");
    assert!(!out.contains("<p>deep</p>"), "{out}");
    assert!(
        out.contains("data-macro-note=\"include-too-deep\" data-macro-arg=\"D\""),
        "{out}"
    );
    let out = html("{{include Nowhere}}\n");
    assert!(
        out.contains("data-macro-note=\"include-missing\" data-macro-arg=\"Nowhere\""),
        "{out}"
    );
    let many = "{{include D}}\n\n".repeat(25);
    let out = html(&many);
    assert_eq!(out.matches("<p>deep</p>").count(), 20, "{out}");
    assert_eq!(out.matches("include-too-many").count(), 5, "{out}");
}

#[test]
fn tasks_list_keys_titles_and_states() {
    let out = html("{{tasks status:open}}\n");
    assert!(out.contains("<a data-ref=\"TM-1\">TM-1</a> Write &lt;notes&gt; <span data-macro=\"status\">Open</span>"), "{out}");
    assert!(out.contains("<del>Tag</del>"), "{out}");
    assert!(
        out.contains("<p data-macro-note=\"tasks-more\" data-macro-arg=\"3\"></p>"),
        "{out}"
    );
    let out = html("{{tasks none}}\n");
    assert!(out.contains("data-macro-note=\"tasks-empty\""), "{out}");
    let out = html("{{tasks bad}}\n");
    assert!(
        out.contains("data-macro-note=\"tasks-error\" data-macro-arg=\"Unknown field “bad”.\""),
        "{out}"
    );
}

#[test]
fn notes_are_worded_when_asked() {
    let wiki = wiki();
    let word = |note: &Note| format!("[{}:{}]", note.kind(), note.value());
    let out = render(
        "{{include Nowhere}}\n",
        &RenderOptions::new("d")
            .with_macros(&wiki)
            .with_macro_notes(&word),
    );
    assert!(out.contains(">[include-missing:Nowhere]</p>"), "{out}");
}

#[test]
fn raw_html_cannot_forge_macro_markup() {
    let out = html(
        "<div data-macro=\"tasks\"><p data-macro-note=\"tasks-error\" data-macro-arg=\"x\">a</p></div>\n\n<input type=\"checkbox\" disabled>\n",
    );
    assert!(!out.contains("data-macro"), "{out}");
    assert!(!out.contains("<input"), "{out}");
}
