// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Titles, descriptions, comments and wiki pages in Markdown.
//!
//! The text reads like a real work log but is made of fixed word lists, so it
//! never contains a real person's name or address. Links point at the
//! reserved `example.invalid` domain.

use std::fmt::Write as _;

use crate::rng::Rng;

const VERBS: &[&str] = &[
    "Add", "Fix", "Refactor", "Document", "Measure", "Remove", "Rename", "Split", "Speed up",
    "Clean up", "Review", "Test", "Move", "Simplify", "Migrate", "Design", "Draft", "Check",
];

const OBJECTS: &[&str] = &[
    "the board drag handle",
    "the export dialog",
    "keyboard focus in the list",
    "the backup rotation",
    "date parsing in the quick-add field",
    "the search ranking",
    "the settings page",
    "the Gantt zoom levels",
    "the time report totals",
    "checklist progress on cards",
    "the wiki sidebar",
    "error messages for invalid keys",
    "the import preview",
    "the reminder schedule",
    "colour contrast in dark mode",
    "the empty state of the calendar",
    "column limits on the board",
    "the attachment thumbnails",
    "the task history panel",
    "the command palette",
    "undo after a bulk move",
    "label colours",
    "the sub-project switcher",
    "the onboarding checklist",
    "the release notes",
];

const QUALIFIERS: &[&str] = &[
    "",
    "",
    "",
    " on small screens",
    " for long titles",
    " after a restart",
    " with 1,000 cards",
    " in right-to-left text",
    " when offline",
    " for keyboard users",
    " in the CLI",
];

/// Titles in other scripts, so sorting, folding and wrapping meet more than
/// ASCII.
const WORLD_TITLES: &[&str] = &[
    "Задача: проверить экспорт",
    "Обновить документацию",
    "检查看板的拖放",
    "更新设置页面",
    "ボードのドラッグを確認する",
    "Prüfen, ob Fälligkeitsdaten stimmen",
    "Révision des étiquettes",
    "Ελέγχος αναζήτησης",
    "בדיקת כיוון טקסט מימין לשמאל",
    "مراجعة النص من اليمين إلى اليسار",
    "Überarbeitung: ½ Tag Puffer einplanen",
    "Release 🚀 checklist",
];

const SENTENCES: &[&str] = &[
    "The current behaviour is confusing when the column is collapsed.",
    "This came up while sorting the backlog last week.",
    "We should keep the change small and easy to revert.",
    "The numbers below were measured on the development machine.",
    "Nothing in the data model has to change for this.",
    "The old path keeps working until the next release.",
    "Screenshots of both states are attached.",
    "It only happens after the window has been resized.",
    "The fix needs a test that fails without it.",
    "Keep the wording consistent with the rest of the app.",
    "A shorter label would fit on one line.",
    "This blocks the next round of performance work.",
    "Most of the time goes into layout, not into the query.",
    "The keyboard shortcut should match the menu entry.",
    "Edge cases: empty titles, very long titles, and titles with emoji.",
    "Dates are days, not moments, so time zones do not apply.",
    "The estimate assumes the parser change lands first.",
    "Undo has to restore the previous order exactly.",
];

const CHECK_ITEMS: &[&str] = &[
    "Write the failing test",
    "Update the screenshots",
    "Check keyboard navigation",
    "Check the dark theme",
    "Measure before and after",
    "Update the changelog",
    "Review the error messages",
    "Try it with data set L",
    "Ask for a second review",
    "Remove the temporary flag",
    "Check right-to-left layout",
    "Update the help page",
];

const CALLOUTS: &[&str] = &["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];

const FORMULAS_INLINE: &[&str] = &[
    "$O(n \\log n)$",
    "$p_{95} < 150\\,\\text{ms}$",
    "$\\frac{done}{total}$",
    "$e^{i\\pi} + 1 = 0$",
    "$\\sqrt{x^2 + y^2}$",
];

const FORMULAS_BLOCK: &[&str] = &[
    "\\sum_{i=1}^{n} w_i \\cdot p_i \\Big/ \\sum_{i=1}^{n} w_i",
    "\\text{remaining} = \\max(0,\\ \\text{estimate} - \\text{spent})",
    "\\begin{aligned}\n  a &= b + c \\\\\n  d &= e \\cdot f\n\\end{aligned}",
    "\\int_0^1 x^2\\,dx = \\tfrac{1}{3}",
];

const MERMAID: &[&str] = &[
    "flowchart LR\n  A[Backlog] --> B[To do]\n  B --> C{Review?}\n  C -->|yes| D[Review]\n  C -->|no| E[Done]\n  D --> E",
    "sequenceDiagram\n  participant UI\n  participant Engine\n  UI->>Engine: move task\n  Engine-->>UI: changed event\n  UI->>Engine: read board",
    "gantt\n  dateFormat YYYY-MM-DD\n  section Release\n  Parser   :a1, 2026-02-02, 5d\n  Board    :a2, after a1, 4d\n  Polish   :after a2, 3d",
    "stateDiagram-v2\n  [*] --> Open\n  Open --> Closed\n  Closed --> Open: reopen\n  Closed --> [*]",
    "pie title Time by type\n  \"Task\" : 42\n  \"Bug\" : 31\n  \"Feature\" : 27",
];

const CODE: &[(&str, &str)] = &[
    (
        "rust",
        "fn remaining(estimate: u32, spent: u32) -> u32 {\n    estimate.saturating_sub(spent)\n}",
    ),
    (
        "sql",
        "SELECT status_id, count(*)\nFROM tasks\nWHERE deleted_at IS NULL\nGROUP BY status_id;",
    ),
    ("sh", "glyphkeel task list --project TM --json | head -n 20"),
    (
        "ts",
        "export function isOverdue(due: string, today: string): boolean {\n  return due < today;\n}",
    ),
    ("text", "- [ ] this is code, not a checklist item\n"),
];

const COMMENTS: &[&str] = &[
    "Looks good to me.",
    "I can reproduce this on the latest build.",
    "Moved to the next milestone; it is not urgent.",
    "Could we split this into two tasks?",
    "Done, see the attached screenshot.",
    "The test passes now.",
    "This needs a decision first.",
    "Measured again: 112 ms instead of 340 ms.",
    "Not sure about the label; any better idea?",
    "Reopened: the old key still shows in the header.",
    "Fixed in the parser; the board needed no change.",
    "Blocked until the export format is settled.",
];

pub const WIKI_TITLES: &[&str] = &[
    "Home",
    "Release checklist",
    "Keyboard shortcuts",
    "Glossary",
    "Board conventions",
    "Meeting notes",
    "Architecture overview",
    "How we estimate",
    "Style guide",
    "Known issues",
    "Onboarding",
    "Data model notes",
    "Performance log",
    "Decisions",
    "Ideas",
];

pub const LABEL_NAMES: &[&str] = &[
    "frontend",
    "backend",
    "docs",
    "ux",
    "performance",
    "security",
    "good first task",
    "needs decision",
    "regression",
    "Отчёты",
];

/// What the description of a task may refer to.
#[derive(Debug)]
pub struct Context<'a> {
    pub project_key: &'a str,
    /// Highest task number already created in the project (0 if none).
    pub max_number: i64,
    pub wiki_titles: &'a [String],
}

/// A Markdown text and the task-list items it contains.
#[derive(Debug, Clone, Default)]
pub struct Markdown {
    pub text: String,
    pub checklist_done: i64,
    pub checklist_total: i64,
}

pub fn task_title(rng: &mut Rng, number: i64) -> String {
    if rng.chance(40) {
        return (*rng.pick(WORLD_TITLES)).to_owned();
    }
    let mut title = format!(
        "{} {}{}",
        rng.pick(VERBS),
        rng.pick(OBJECTS),
        rng.pick(QUALIFIERS)
    );
    if rng.chance(15) {
        // A long title, close to what a pasted error message looks like.
        let _ = write!(
            title,
            " (error 0x{:08x} after step {number}; see the log in the attached file for the full trace)",
            rng.next_u64() as u32
        );
    }
    title
}

fn sentence(rng: &mut Rng) -> &'static str {
    rng.pick(SENTENCES)
}

fn paragraph(rng: &mut Rng, min: i64, max: i64) -> String {
    let n = rng.range(min, max);
    (0..n).map(|_| sentence(rng)).collect::<Vec<_>>().join(" ")
}

fn task_ref(rng: &mut Rng, ctx: &Context<'_>) -> Option<String> {
    (ctx.max_number > 0).then(|| format!("{}-{}", ctx.project_key, rng.range(1, ctx.max_number)))
}

fn wiki_ref(rng: &mut Rng, ctx: &Context<'_>) -> String {
    if ctx.wiki_titles.is_empty() || rng.chance(100) {
        // A link to a page that does not exist yet.
        "[[Ideas for later]]".to_owned()
    } else {
        format!("[[{}]]", rng.pick(ctx.wiki_titles))
    }
}

fn checklist(rng: &mut Rng, out: &mut Markdown, heading: &str) {
    let n = rng.range(2, 7);
    let _ = write!(out.text, "## {heading}\n\n");
    for i in 0..n {
        let done = rng.chance(450);
        let mark = if done { 'x' } else { ' ' };
        let _ = writeln!(out.text, "- [{mark}] {}", rng.pick(CHECK_ITEMS));
        out.checklist_total += 1;
        out.checklist_done += i64::from(done);
        if i == 0 && rng.chance(200) {
            let sub_done = rng.chance(500);
            let mark = if sub_done { 'x' } else { ' ' };
            let _ = writeln!(out.text, "  - [{mark}] {}", rng.pick(CHECK_ITEMS));
            out.checklist_total += 1;
            out.checklist_done += i64::from(sub_done);
        }
    }
    out.text.push('\n');
}

/// A task description. About one in seven is empty; `rich` raises the share
/// of diagrams, formulas and tables.
pub fn description(rng: &mut Rng, ctx: &Context<'_>, rich: bool) -> Markdown {
    let mut md = Markdown::default();
    if rng.chance(140) {
        return md;
    }
    md.text.push_str(&paragraph(rng, 1, 4));
    md.text.push_str("\n\n");
    let extra = if rich { 300 } else { 0 };

    if rng.chance(600) {
        checklist(rng, &mut md, "Steps");
    }
    if rng.chance(450) {
        let mut line = String::from("See ");
        line.push_str(&wiki_ref(rng, ctx));
        if let Some(key) = task_ref(rng, ctx) {
            let _ = write!(line, " and {key}");
        }
        line.push_str(".\n\n");
        md.text.push_str(&line);
    }
    if rng.chance(200 + extra) {
        let kind = rng.pick(CALLOUTS);
        let _ = write!(md.text, "> [!{kind}]\n> {}\n\n", sentence(rng));
    }
    if rng.chance(120 + extra) {
        let _ = write!(
            md.text,
            "The cost grows as {} here.\n\n",
            rng.pick(FORMULAS_INLINE)
        );
    }
    if rng.chance(80 + extra) {
        let _ = write!(md.text, "$$\n{}\n$$\n\n", rng.pick(FORMULAS_BLOCK));
    }
    if rng.chance(80 + extra) {
        let _ = write!(md.text, "```mermaid\n{}\n```\n\n", rng.pick(MERMAID));
    }
    if rng.chance(150) {
        let (lang, code) = rng.pick(CODE);
        let _ = write!(md.text, "```{lang}\n{code}\n```\n\n");
    }
    if rng.chance(100 + extra) {
        md.text
            .push_str("| Case | Before | After |\n|---|---:|---:|\n");
        for i in 0..rng.range(2, 5) {
            let _ = writeln!(
                md.text,
                "| case {} | {} ms | {} ms |",
                i + 1,
                rng.range(100, 900),
                rng.range(10, 99)
            );
        }
        md.text.push('\n');
    }
    if rng.chance(100) {
        let _ = write!(
            md.text,
            "Log: <https://example.invalid/logs/{}>, details in [the notes](https://example.invalid/notes#{}).\n\n",
            rng.range(1000, 9999),
            rng.range(1, 50)
        );
    }
    if rng.chance(250) {
        checklist(rng, &mut md, "Done when");
    }
    while md.text.ends_with('\n') {
        md.text.pop();
    }
    md.text.push('\n');
    md
}

pub fn comment(rng: &mut Rng, ctx: &Context<'_>) -> String {
    let mut body = (*rng.pick(COMMENTS)).to_owned();
    if rng.chance(200) {
        body.push(' ');
        body.push_str(sentence(rng));
    }
    if rng.chance(150)
        && let Some(key) = task_ref(rng, ctx)
    {
        let _ = write!(body, " Same cause as {key}.");
    }
    if rng.chance(80) {
        body.push_str("\n\n```text\nerror: index out of range\n  at board::move_card\n```");
    }
    if rng.chance(60) {
        body = format!("> {}\n\n{body}", sentence(rng));
    }
    body
}

pub fn time_note(rng: &mut Rng) -> String {
    const NOTES: &[&str] = &[
        "",
        "",
        "investigation",
        "implementation",
        "review",
        "tests",
        "pairing",
        "docs",
    ];
    (*rng.pick(NOTES)).to_owned()
}

pub fn wiki_body(rng: &mut Rng, title: &str, ctx: &Context<'_>) -> Markdown {
    let mut md = Markdown::default();
    let _ = write!(md.text, "# {title}\n\n{}\n\n", paragraph(rng, 2, 5));
    if rng.chance(500) {
        checklist(rng, &mut md, "Checklist");
    }
    let _ = write!(md.text, "Related: {}", wiki_ref(rng, ctx));
    if let Some(key) = task_ref(rng, ctx) {
        let _ = write!(md.text, ", {key}");
    }
    md.text.push_str(".\n");
    if rng.chance(300) {
        let _ = write!(md.text, "\n```mermaid\n{}\n```\n", rng.pick(MERMAID));
    }
    if rng.chance(200) {
        let _ = write!(md.text, "\n$$\n{}\n$$\n", rng.pick(FORMULAS_BLOCK));
    }
    md
}

/// Counts task-list items: list items (`-`, `*`, `+` or `1.`, also inside
/// block quotes) whose text starts with `[ ]`, `[x]` or `[X]` and a space.
/// Fenced code, lines indented four or more spaces and HTML comments are
/// skipped.
///
/// This is a simplified form of the GFM rule, enough for the generated data
/// and for checking it. The engine computes the authoritative counts when it
/// imports an archive.
pub fn count_checklist(text: &str) -> (i64, i64) {
    let (mut done, mut total) = (0, 0);
    let mut fence = false;
    let mut comment = false;
    for line in text.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let trimmed = line.trim_start();
        if !comment && indent < 4 && trimmed.starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if comment || trimmed.starts_with("<!--") {
            comment = !trimmed.contains("-->");
            continue;
        }
        if indent >= 4 && !is_nested_item(line) {
            continue;
        }
        let rest = trimmed.trim_start_matches(['>', ' ']);
        let marker = if rest.starts_with(['-', '*', '+']) {
            1
        } else {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 && rest[digits..].starts_with('.') {
                digits + 1
            } else {
                continue;
            }
        };
        let after = &rest[marker..];
        let item = after.trim_start_matches(' ');
        let gap = after.len() - item.len();
        if !(1..=4).contains(&gap) {
            continue;
        }
        if item.starts_with("[ ] ") {
            total += 1;
        } else if item.starts_with("[x] ") || item.starts_with("[X] ") {
            total += 1;
            done += 1;
        }
    }
    (done, total)
}

/// Generated nested items are indented in steps of two spaces under a
/// parent item; deeper indentation of a list marker still counts as a list.
fn is_nested_item(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    indent.is_multiple_of(2) && line.trim_start().starts_with("- [")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklist_counts() {
        let text = "- [ ] a\n- [x] b\n  - [X] c\n* [ ] d\n1. [x] e\n> - [ ] f\n- [ ]g\n- [  ] h\n\n```\n- [ ] code\n```\n<!--\n- [ ] comment\n-->\n";
        assert_eq!(count_checklist(text), (3, 6));
    }

    #[test]
    fn generated_counts_match() {
        let titles = vec!["Home".to_owned()];
        let ctx = Context {
            project_key: "TM",
            max_number: 10,
            wiki_titles: &titles,
        };
        let mut rng = Rng::new(5, 0);
        for _ in 0..2_000 {
            let md = description(&mut rng, &ctx, true);
            assert_eq!(
                count_checklist(&md.text),
                (md.checklist_done, md.checklist_total),
                "{}",
                md.text
            );
        }
    }
}
