// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The hostile-content set: Markdown, formulas, diagrams, SVG and names made
//! to break a renderer, a sanitizer or a layout.
//!
//! Every text stays within the limits the engine accepts (titles up to 500
//! characters, Markdown up to 1 MB), so the set can be imported and then
//! rendered. What must hold for each item is in `fixtures.json`. Every
//! address points at the reserved `example.invalid` domain.

use std::fmt::Write as _;

use serde_json::json;

use crate::archive::Archive;
use crate::clock::{self, DAY_MS, HOUR_MS, MINUTE_MS};
use crate::generate::{Actor, Builder, USER, exported_at};
use crate::model::{
    Attachment, Fixture, Label, Modules, Project, ProjectFile, Revision, Status, Task, TaskType,
    WikiPage,
};
use crate::text;

/// Project key of the hostile set.
pub const KEY: &str = "HOST";

/// Largest Markdown text the engine accepts.
pub const MARKDOWN_LIMIT: usize = 1024 * 1024;
/// Longest title the engine accepts, in characters.
pub const TITLE_LIMIT: usize = 500;

/// One hostile item.
#[derive(Debug)]
pub struct Item {
    pub name: &'static str,
    pub title: String,
    pub targets: &'static [&'static str],
    pub purpose: &'static str,
    pub expect: &'static str,
    pub body: String,
}

const RENDER: &[&str] = &["markdown", "sanitizer", "webview"];
const MATH: &[&str] = &["markdown", "katex", "webview"];
const DIAGRAM: &[&str] = &["markdown", "mermaid", "webview"];
const LAYOUT: &[&str] = &["markdown", "webview", "layout"];
const NO_SCRIPT: &str =
    "Renders as text or inert markup; no script runs, no request leaves, no CSP violation.";
const BOUNDED: &str = "Renders within the time and size limits, or falls back to showing the source; the app stays responsive.";

fn repeat_until(unit: &str, budget: usize) -> String {
    let n = budget / unit.len().max(1);
    unit.repeat(n)
}

#[allow(clippy::too_many_lines)]
pub fn items() -> Vec<Item> {
    let mut v = Vec::new();

    v.push(Item {
        name: "raw-html-script",
        title: "<script>alert(document.domain)</script> in a title".into(),
        targets: RENDER,
        purpose: "Raw HTML that runs script in every way a browser allows.",
        expect: NO_SCRIPT,
        body: r#"Plain text before.

<script>alert(document.domain)</script>
<img src="x" onerror="alert(document.domain)">
<iframe src="https://example.invalid/frame" srcdoc="<script>alert(1)</script>"></iframe>
<object data="https://example.invalid/x.swf"></object>
<embed src="https://example.invalid/x.swf">
<style>body { background: url("https://example.invalid/track.png") }</style>
<link rel="stylesheet" href="https://example.invalid/x.css">
<meta http-equiv="refresh" content="0; url=https://example.invalid/">
<base href="https://example.invalid/">
<form action="https://example.invalid/steal"><input name="q"><button>Send</button></form>
<details open ontoggle="alert(1)"><summary>toggle</summary></details>
<a href="javascript:alert(1)">click</a>
<div style="position:fixed;inset:0;background:red" onclick="alert(1)">overlay</div>
<svg onload="alert(1)"><circle r="10"/></svg>
<math><mtext><table><mglyph><style><img src=x onerror=alert(1)></style></mglyph></table></mtext></math>
<noscript><p title="</noscript><img src=x onerror=alert(1)>"></noscript>
<template><script>alert(1)</script></template>
<!-- an unclosed comment hides everything after it
"#
        .into(),
    });

    v.push(Item {
        name: "dangerous-links",
        title: "Links with javascript:, data: and file: targets".into(),
        targets: RENDER,
        purpose: "Link and image targets that run script, read local files or leak a request.",
        expect: "Unsafe schemes are not clickable; remote images do not load; only the app's own link kinds work.",
        body: r#"[plain](javascript:alert(1))
[mixed case](JaVaScRiPt:alert(1))
[entity](&#106;avascript:alert(1))
[spaces]( javascript:alert(1))
[tab](java	script:alert(1))
[data](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)
[vbscript](vbscript:msgbox(1))
[file](file:///etc/passwd)
[relative](../../../etc/passwd)
[protocol relative](//example.invalid/x)
[tauri](tauri://localhost/index.html)
[ipc](ipc://localhost/plugin)
<javascript:alert(1)>
<https://example.invalid/auto>

![remote image](https://example.invalid/track.png)
![data image](data:image/svg+xml,%3Csvg%20onload%3Dalert(1)%3E)

[ref]: javascript:alert(1)
Reference: [click][ref]
"#
        .into(),
    });

    let mut table = String::from("| ");
    let cols = 60;
    for c in 0..cols {
        let _ = write!(table, "col {c} | ");
    }
    table.push_str("\n|");
    table.push_str(&"---|".repeat(cols));
    table.push('\n');
    let mut row = 0;
    while table.len() < 900_000 {
        table.push('|');
        for c in 0..cols {
            let _ = write!(table, " r{row}c{c} |");
        }
        table.push('\n');
        row += 1;
    }
    v.push(Item {
        name: "huge-table",
        title: "A table of 60 columns and thousands of rows".into(),
        targets: LAYOUT,
        purpose: "Layout and memory with a table close to the 1 MB limit.",
        expect: BOUNDED,
        body: table,
    });

    let mut nesting = String::new();
    nesting.push_str(&">".repeat(5_000));
    nesting.push_str(" deep quote\n\n");
    for i in 0..500 {
        let _ = writeln!(nesting, "{}- level {i}", "  ".repeat(i));
    }
    nesting.push('\n');
    nesting.push_str(&"*".repeat(10_000));
    nesting.push_str("emphasis");
    nesting.push_str(&"*".repeat(10_000));
    nesting.push_str("\n\n");
    nesting.push_str(&"[".repeat(20_000));
    nesting.push('x');
    nesting.push_str(&"](https://example.invalid)".repeat(2_000));
    nesting.push_str("\n\n");
    nesting.push_str(&"<div>".repeat(10_000));
    nesting.push_str("inside");
    nesting.push('\n');
    v.push(Item {
        name: "deep-nesting",
        title: "Quotes, lists, emphasis and brackets nested thousands deep".into(),
        targets: LAYOUT,
        purpose: "Stack depth and quadratic paths in the parser, the sanitizer and the DOM.",
        expect: BOUNDED,
        body: nesting,
    });

    let mut patho = String::new();
    patho.push_str(&repeat_until("*a **a ", 200_000));
    patho.push_str("\n\n");
    patho.push_str(&repeat_until("_a __a ", 100_000));
    patho.push_str("\n\n");
    patho.push_str(&repeat_until("`a ``a ", 100_000));
    patho.push_str("\n\n");
    patho.push_str(&repeat_until("<a ", 100_000));
    patho.push_str("\n\n");
    patho.push_str(&repeat_until("[a](", 100_000));
    patho.push('\n');
    v.push(Item {
        name: "pathological-inlines",
        title: "Unclosed emphasis, code spans and links".into(),
        targets: &["markdown", "webview"],
        purpose: "Inputs known to make naive inline parsers quadratic.",
        expect: BOUNDED,
        body: patho,
    });

    let mut long_line = String::from("A single word of 800,000 letters follows.\n\n");
    long_line.push_str(&"x".repeat(800_000));
    long_line.push('\n');
    v.push(Item {
        name: "long-line",
        title: format!("Long title {}", "w".repeat(TITLE_LIMIT - 11)),
        targets: LAYOUT,
        purpose: "Wrapping, horizontal overflow and text measurement with one huge word; a title at the length limit.",
        expect: "Wraps or scrolls inside its box; nothing overflows the window.",
        body: long_line,
    });

    let mut unicode = String::new();
    unicode.push_str("Right-to-left override: \u{202E}gnp.exe\u{202C} (shows as exe.png).\n\n");
    unicode.push_str("Isolates: \u{2067}مرحبا\u{2069} and \u{2066}hello\u{2069}.\n\n");
    unicode
        .push_str("Zero width: a\u{200B}b\u{200C}c\u{200D}d\u{FEFF}e and soft\u{00AD}hyphen.\n\n");
    unicode.push_str("Combining marks: Z");
    unicode.push_str(&"\u{0336}\u{0337}\u{0338}\u{0489}".repeat(200));
    unicode.push_str("\n\nFamily emoji: 👨‍👩‍👧‍👦 flags: 🇺🇳🏳️‍🌈 skin tones: 👋🏽\n\n");
    unicode.push_str("Homoglyphs: Тask-1 (Cyrillic Т), ТМ-1, TM‐1 (hyphen U+2010).\n\n");
    unicode.push_str("NUL: [\u{0}] BEL: [\u{7}] ESC: [\u{1b}[31mred\u{1b}[0m] DEL: [\u{7f}]\n\n");
    unicode.push_str("Line separator:\u{2028}paragraph separator:\u{2029}end.\n\n");
    unicode.push_str("Private use: \u{E000}\u{F8FF} and the replacement character: \u{FFFD}.\n\n");
    unicode.push_str("Astral: 𝔘𝔫𝔦𝔠𝔬𝔡𝔢 𓀀 🀄\n");
    v.push(Item {
        name: "unicode-tricks",
        title: "Title with \u{202E}lortnoc\u{202C}, zero\u{200B}width and 👨‍👩‍👧‍👦".into(),
        targets: &["markdown", "webview", "search"],
        purpose: "Bidirectional overrides, invisible and control characters, homoglyphs of task keys.",
        expect: "Shown without reordering the surrounding interface; control characters are visible or removed; homoglyphs do not link as task keys.",
        body: unicode,
    });

    let mut katex = String::from(
        "Inline: $\\href{javascript:alert(1)}{click}$ and $\\url{javascript:alert(1)}$.\n\n",
    );
    for f in [
        "\\htmlClass{x}{y} \\htmlId{x}{y} \\htmlStyle{position:fixed;inset:0}{y} \\htmlData{foo=bar}{y}",
        "\\includegraphics[height=100em]{https://example.invalid/x.png}",
        "\\def\\a{\\b\\b}\\def\\b{\\c\\c}\\def\\c{\\d\\d}\\def\\d{\\e\\e}\\def\\e{\\f\\f}\\def\\f{\\g\\g}\\def\\g{xx}\\a\\a\\a\\a\\a\\a\\a\\a",
        "\\newcommand{\\r}{\\r}\\r",
        "\\rule{100000em}{100000em}",
        "\\color{red}{\\textcolor{expression(alert(1))}{x}}",
        "\\verb|<script>alert(1)</script>|",
        "\\text{<img src=x onerror=alert(1)>}",
        "\\left( \\right. \\begin{matrix} a & b \\\\ c",
        "\\operatorname{<script>}x",
    ] {
        let _ = write!(katex, "$$\n{f}\n$$\n\n");
    }
    katex.push_str("$$\n");
    katex.push_str(&"\\frac{1}{".repeat(2_000));
    katex.push('x');
    katex.push_str(&"}".repeat(2_000));
    katex.push_str("\n$$\n\n$$\n");
    for i in 0..20_000 {
        let _ = write!(katex, "x_{{{i}}} + ");
    }
    katex.push_str("0\n$$\n");
    v.push(Item {
        name: "katex-hostile",
        title: "Formulas: links, HTML commands, macro bombs, deep fractions".into(),
        targets: MATH,
        purpose: "Formulas aimed at KaTeX: trust-gated commands, expansion bombs, huge sizes, 2,000 nested fractions, a 20,000-term sum.",
        expect: "No link or HTML command takes effect; the macro limit stops expansion; huge formulas render bounded or show their source.",
        body: katex,
    });

    let mut mermaid = String::from(
        "```mermaid\n%%{init: {\"securityLevel\": \"loose\", \"startOnLoad\": true}}%%\nflowchart LR\n  A[\"<img src=x onerror=alert(1)>\"] --> B[\"<script>alert(1)</script>\"]\n  B --> C[\"<a href='javascript:alert(1)'>link</a>\"]\n  click A call alert(1)\n  click B href \"javascript:alert(1)\"\n  click C href \"https://example.invalid/\" _blank\n  style A fill:url(https://example.invalid/x.png)\n```\n\n",
    );
    mermaid.push_str("```mermaid\nsequenceDiagram\n  participant A as <img src=x onerror=alert(1)>\n  A->>A: <script>alert(1)</script>\n  Note over A: <iframe src=\"https://example.invalid\"></iframe>\n```\n\n");
    mermaid.push_str("```mermaid\nflowchart TD\n");
    for i in 0..5_000 {
        let _ = writeln!(mermaid, "  n{i}[node {i}] --> n{}[node {}]", i + 1, i + 1);
    }
    mermaid.push_str("```\n\n```mermaid\nflowchart TD\n");
    for i in 0..200 {
        let _ = writeln!(mermaid, "{}subgraph s{i}", "  ".repeat(i % 40));
    }
    mermaid.push_str("  x\n");
    for _ in 0..200 {
        mermaid.push_str("end\n");
    }
    mermaid.push_str("```\n\n```mermaid\nflowchart LR\n  A[");
    mermaid.push_str(&"long label ".repeat(10_000));
    mermaid.push_str("] --> B\n```\n\n```mermaid\nthis is not ]] a valid { diagram\n```\n");
    v.push(Item {
        name: "mermaid-hostile",
        title: "Diagrams: HTML labels, click handlers, init directives, 5,000 nodes".into(),
        targets: DIAGRAM,
        purpose: "Diagrams aimed at Mermaid: HTML and links in labels, click callbacks, a directive asking for loose security, a giant graph, deep subgraphs, a malformed diagram.",
        expect: "Strict security level holds; labels are text; no callback runs; large diagrams render bounded or show their source; the malformed one shows an error, not a crash.",
        body: mermaid,
    });

    v.push(Item {
        name: "malformed-svg",
        title: "Inline SVG with script, foreign objects and entity bombs".into(),
        targets: RENDER,
        purpose: "SVG written into Markdown as raw HTML, including malformed and recursive forms.",
        expect: NO_SCRIPT,
        body: format!("{}\n", hostile_svg()),
    });

    let mut checks = String::from("## Many items\n\n");
    for i in 0..5_000 {
        let mark = if i % 3 == 0 { 'x' } else { ' ' };
        let _ = writeln!(checks, "- [{mark}] item {i}");
    }
    checks.push_str("\n## Odd forms\n\n- [X] upper-case X\n* [ ] star bullet\n+ [x] plus bullet\n1. [ ] ordered\n- [ ]no space after\n- [  ] two spaces\n-  [ ] two spaces before\n> - [ ] in a quote\n\n```\n- [ ] inside a code block\n```\n\n    - [ ] indented code\n\n<!-- - [ ] in a comment -->\n");
    v.push(Item {
        name: "checklist-edge-cases",
        title: "5,000 checklist items and odd task-list forms".into(),
        targets: &["markdown", "engine", "webview"],
        purpose: "Checkbox numbering, progress counts and the editor's one-character toggle with many and malformed items.",
        expect: "Counts follow the GFM task-list rule; items in code blocks and comments do not count; ticking changes exactly one character.",
        body: checks,
    });

    v.push(Item {
        name: "links-and-keys",
        title: "Wiki links and task keys that try to escape".into(),
        targets: &["markdown", "engine", "webview"],
        purpose: "Wiki links and task keys with paths, markup, overflow and unclosed brackets.",
        expect: "Links resolve only inside the project; overflowing keys stay text; nothing opens outside the app.",
        body: "[[../../etc/passwd]] [[<script>alert(1)</script>]] [[Home|<img src=x onerror=alert(1)>]] [[ ]] [[\u{202E}emoH]] [[unclosed\n\nTM-0 TM-99999999999999999999999 tm-1 HOST-1 HOST-1a xHOST-1 HOST-1x `HOST-1`\n\n[HOST-1](javascript:alert(1)) [[HOST-1]]\n".into(),
    });

    let mut callouts = String::new();
    for _ in 0..300 {
        callouts.push_str("> [!NOTE]\n");
    }
    callouts.push_str("> deep\n\n> [!<script>alert(1)</script>]\n> type is markup\n\n> [!UNKNOWNTYPE] title\n> body\n\n> [!WARNING]- folded <img src=x onerror=alert(1)>\n> body\n");
    v.push(Item {
        name: "callouts",
        title: "Callouts nested 300 deep and with markup as type".into(),
        targets: &["markdown", "webview"],
        purpose: "Callout parsing with nesting, unknown types and markup in the type and title.",
        expect: "Renders bounded; unknown types fall back to a note; markup stays text.",
        body: callouts,
    });

    let mut images = String::new();
    for i in 0..3_000 {
        let _ = write!(images, "![i{i}](https://example.invalid/img/{i}.png) ");
    }
    images.push_str("\n\n[^a]: footnote [^b]\n[^b]: footnote [^a]\n\nUses [^a].\n\n<![CDATA[ x ]]> <?php echo 1; ?> <!DOCTYPE html>\n");
    v.push(Item {
        name: "many-remote-images",
        title: "3,000 remote images and looping footnotes".into(),
        targets: RENDER,
        purpose: "Request storms from images, footnotes that refer to each other, processing instructions.",
        expect: "No request leaves; footnotes render once; no endless loop.",
        body: images,
    });

    v
}

/// SVG content that tries every known way to run script or fetch.
pub fn hostile_svg() -> String {
    r##"<?xml version="1.0"?>
<!DOCTYPE svg [
  <!ENTITY a "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
  <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
  <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">
  <!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;">
  <!ENTITY ext SYSTEM "file:///etc/passwd">
]>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="200" height="100" onload="alert(1)">
  <script>alert(document.domain)</script>
  <script xlink:href="https://example.invalid/x.js"/>
  <text x="0" y="20">&d; &ext;</text>
  <a xlink:href="javascript:alert(1)"><text x="0" y="40">click</text></a>
  <a href="javascript:alert(1)"><rect width="10" height="10"/></a>
  <foreignObject width="100" height="50"><body xmlns="http://www.w3.org/1999/xhtml"><iframe src="https://example.invalid/"></iframe></body></foreignObject>
  <use href="https://example.invalid/sprite.svg#a"/>
  <image href="https://example.invalid/x.png" width="10" height="10"/>
  <animate attributeName="href" to="javascript:alert(1)" begin="0s"/>
  <set attributeName="onmouseover" to="alert(1)"/>
  <style>@import url("https://example.invalid/x.css"); rect { fill: url(https://example.invalid/p.svg#g) }</style>
  <rect width="10" height="10" style="behavior:url(x.htc)">
  <g><g><g><unclosed
</svg"##
        .to_owned()
}

/// File names that try path traversal, reserved names and direction tricks.
const FILE_NAMES: &[(&str, &str)] = &[
    ("../../../../etc/passwd", "text/plain"),
    (
        "..\\..\\windows\\system32\\drivers\\etc\\hosts",
        "text/plain",
    ),
    ("/absolute/path.txt", "text/plain"),
    ("CON.txt", "text/plain"),
    ("invoice\u{202E}gnp.exe", "application/octet-stream"),
    ("<img src=x onerror=alert(1)>.png", "image/png"),
    ("name with\nnewline.txt", "text/plain"),
    (" leading and trailing spaces .txt ", "text/plain"),
    ("hostile.svg", "image/svg+xml"),
    ("looks-like-a.png", "image/png"),
];

/// Builds the hostile-content archive: one project, one task per item, a
/// wiki page with a hostile title, and attachments with hostile names.
#[allow(clippy::too_many_lines)]
pub(crate) fn build(seed: u64) -> anyhow::Result<Archive> {
    let mut b = Builder::new(seed);
    let created = clock::anchor_ms();
    let me = Actor {
        kind: "user",
        name: USER.into(),
        via: "app",
        session: None,
    };
    let project_id = b.id(created);
    let p = b.add_project_key(&project_id, KEY, "Hostile content");
    b.record(
        p,
        &project_id,
        created,
        "project.create",
        &me,
        "project",
        &project_id,
        json!({ "key": KEY }),
    );
    let todo = Status {
        id: b.id(created),
        name: "To do".into(),
        category: "todo".into(),
        hue: "blue".into(),
        rank: clock::rank(0),
        wip_limit: None,
    };
    let task_type = TaskType {
        id: b.id(created),
        name: "Task".into(),
        glyph: "task".into(),
        hue: "blue".into(),
        rank: clock::rank(0),
        is_default: true,
    };
    let label = Label {
        id: b.id(created),
        name: "<b>bold</b>\u{202E}lebal".into(),
        hue: "red".into(),
        rank: clock::rank(0),
    };

    let items = items();
    let mut tasks = Vec::new();
    let mut revisions = Vec::new();
    let mut fixtures = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let number = i as i64 + 1;
        let key = format!("{KEY}-{number}");
        let at = created + HOUR_MS + i as i64 * MINUTE_MS;
        let id = b.id(at);
        let rev = b.id(at);
        revisions.push(Revision {
            id: rev.clone(),
            owner_kind: "task".into(),
            owner_id: id.clone(),
            body: item.body.clone(),
            base_rev: None,
            kind: "normal".into(),
            message: None,
            actor_kind: "user".into(),
            actor_name: USER.into(),
            session_id: None,
            created_at: at,
            resolved_at: None,
        });
        b.record(
            p,
            &project_id,
            at,
            "task.create",
            &me,
            "task",
            &id,
            json!({ "key": key, "title": item.title }),
        );
        let (done, total) = text::count_checklist(&item.body);
        b.archive.files.insert(
            format!("projects/{KEY}/tasks/{key}.md"),
            item.body.clone().into_bytes(),
        );
        fixtures.push(Fixture {
            key: key.clone(),
            name: item.name.into(),
            targets: item.targets.iter().map(|s| (*s).to_owned()).collect(),
            purpose: item.purpose.into(),
            expect: item.expect.into(),
        });
        tasks.push(Task {
            id,
            project_id: project_id.clone(),
            number,
            key,
            title: item.title.clone(),
            description_rev: Some(rev),
            status_id: todo.id.clone(),
            status_entered_at: at,
            type_id: task_type.id.clone(),
            priority: 0,
            parent_id: None,
            rank: clock::rank(i),
            start_date: None,
            due_date: None,
            estimate_min: None,
            progress_override: None,
            checklist_done: done,
            checklist_total: total,
            spent_min: 0,
            claim: None,
            labels: vec![label.id.clone()],
            relations: Vec::new(),
            created_at: at,
            created_by_kind: "user".into(),
            created_by_name: USER.into(),
            edited_at: at,
            updated_at: at,
            closed_at: None,
            deleted_at: None,
            deleted_group: None,
            version: 1,
        });
    }
    b.counts.tasks = tasks.len() as u64;
    b.counts.open_tasks = tasks.len() as u64;

    // Attachments with hostile names; the SVG ones carry the hostile SVG.
    let mut files = Vec::new();
    let svg_sha = b.put_blob(hostile_svg().into_bytes());
    let text_sha = b.put_blob(b"plain text\n".to_vec());
    for (i, (name, mime)) in FILE_NAMES.iter().enumerate() {
        let at = created + DAY_MS + i as i64 * MINUTE_MS;
        let (sha, size) = if mime.starts_with("image/") {
            (svg_sha.clone(), hostile_svg().len() as u64)
        } else {
            (text_sha.clone(), 11)
        };
        let owner = &tasks[i % tasks.len()];
        let id = b.id(at);
        b.record(
            p,
            &project_id,
            at,
            "file.attach",
            &me,
            "task",
            &owner.id,
            json!({ "file": name }),
        );
        files.push(Attachment {
            id,
            owner_kind: "task".into(),
            owner_id: owner.id.clone(),
            file_name: (*name).into(),
            mime: (*mime).into(),
            size,
            sha256: sha,
            width: None,
            height: None,
            actor_kind: "user".into(),
            actor_name: USER.into(),
            created_at: at,
            deleted_at: None,
            deleted_group: None,
        });
    }
    b.counts.attachments = files.len() as u64;

    // One wiki page whose title tries to escape the wiki folder.
    let wiki_at = created + 2 * DAY_MS;
    let page_id = b.id(wiki_at);
    let page_rev = b.id(wiki_at);
    let title = "../../<script>alert(1)</script>";
    let slug = clock::slug(title);
    let body = items
        .iter()
        .find(|i| i.name == "raw-html-script")
        .map(|i| i.body.clone())
        .unwrap_or_default();
    revisions.push(Revision {
        id: page_rev.clone(),
        owner_kind: "wiki_page".into(),
        owner_id: page_id.clone(),
        body: body.clone(),
        base_rev: None,
        kind: "normal".into(),
        message: None,
        actor_kind: "user".into(),
        actor_name: USER.into(),
        session_id: None,
        created_at: wiki_at,
        resolved_at: None,
    });
    b.record(
        p,
        &project_id,
        wiki_at,
        "wiki.create",
        &me,
        "wiki_page",
        &page_id,
        json!({ "title": title }),
    );
    let path = format!("{slug}.md");
    b.archive
        .files
        .insert(format!("projects/{KEY}/wiki/{path}"), body.into_bytes());
    let pages = vec![WikiPage {
        id: page_id,
        project_id: project_id.clone(),
        parent_id: None,
        rank: clock::rank(0),
        title: title.into(),
        slug,
        path,
        current_rev: page_rev,
        is_agent_brief: false,
        created_at: wiki_at,
        updated_at: wiki_at,
        deleted_at: None,
        deleted_group: None,
    }];
    b.counts.wiki_pages = 1;
    b.counts.revisions = revisions.len() as u64;

    let dir = format!("projects/{KEY}");
    let project_file = ProjectFile {
        project: Project {
            id: project_id,
            parent_id: None,
            rank: clock::rank(0),
            key: KEY.into(),
            name: "Hostile content".into(),
            description: "Hostile content for renderer and sanitizer tests. See fixtures.json.\n"
                .into(),
            description_rev: None,
            hue: "red".into(),
            icon: None,
            state: "active".into(),
            modules: Modules {
                board: 1,
                list: 1,
                gantt: 1,
                wiki: 1,
                time: 1,
            },
            next_number: tasks.len() as i64 + 1,
            created_at: created,
            updated_at: wiki_at,
            deleted_at: None,
            deleted_group: None,
        },
        statuses: vec![todo],
        types: vec![task_type],
        labels: vec![label],
        views: Vec::new(),
        layouts: Vec::new(),
    };
    debug_assert!(wiki_at < exported_at());
    b.put_json(format!("{dir}/project.json"), &project_file)?;
    b.put_jsonl(format!("{dir}/tasks.jsonl"), &tasks)?;
    b.put_jsonl::<crate::model::Comment>(format!("{dir}/comments.jsonl"), &[])?;
    b.put_jsonl::<crate::model::TimeEntry>(format!("{dir}/time.jsonl"), &[])?;
    b.put_jsonl(format!("{dir}/revisions.jsonl"), &revisions)?;
    b.put_jsonl(format!("{dir}/wiki.jsonl"), &pages)?;
    b.put_jsonl(format!("{dir}/files.jsonl"), &files)?;
    b.put_json("fixtures.json".into(), &fixtures)?;
    b.finish("hostile")
}
