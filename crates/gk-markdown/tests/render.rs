// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The reader's HTML for each extension, references and plain text.

#![allow(clippy::unwrap_used)]

mod common;

use common::violations;
use gk_markdown::{RenderOptions, analyze, plain_text, references, render};

fn tm(key: &str) -> bool {
    key.starts_with("TM-")
}

fn html(source: &str) -> String {
    render(source, &RenderOptions::new("d1").with_task_keys(&tm))
}

#[test]
fn task_keys_link_only_when_known_and_not_in_code_or_links() {
    let out = html(
        "TM-41, TM-1. (TM-2) HIDDEN-3 `TM-4` [TM-5](https://x.example) [[TM-6]] xTM-7 TM-8x \
         TM-9-b A-TM-10 tm-11\n\n```\nTM-12\n```",
    );
    for key in ["TM-41", "TM-1", "TM-2"] {
        assert!(out.contains(&format!("data-ref=\"{key}\"")), "{key}: {out}");
    }
    for key in [
        "HIDDEN-3", "TM-4", "TM-5", "TM-6", "TM-7", "TM-8", "TM-9", "TM-10", "TM-12",
    ] {
        assert!(
            !out.contains(&format!("data-ref=\"{key}\"")),
            "{key}: {out}"
        );
    }
    assert!(out.contains("HIDDEN-3"), "unknown keys stay as text: {out}");
}

#[test]
fn without_a_resolver_no_key_is_linked() {
    let out = render("TM-41", &RenderOptions::new("d"));
    assert_eq!(out, "<p>TM-41</p>\n");
}

#[test]
fn wiki_links_carry_target_and_label() {
    let out = html("[[Release plan]] [[Release plan#Dates]] [[Release plan#Dates|the dates]]");
    assert!(
        out.contains("<a data-wiki=\"Release plan\" rel=\"noopener noreferrer\">Release plan</a>"),
        "{out}"
    );
    assert!(out.contains("data-wiki=\"Release plan#Dates\""), "{out}");
    assert!(out.contains(">the dates</a>"), "{out}");
}

#[test]
fn five_callouts() {
    for (kind, class) in [
        ("NOTE", "note"),
        ("TIP", "tip"),
        ("IMPORTANT", "important"),
        ("WARNING", "warning"),
        ("CAUTION", "caution"),
    ] {
        let out = html(&format!("> [!{kind}]\n> body *x*\n"));
        assert!(
            out.contains(&format!("class=\"markdown-alert markdown-alert-{class}\"")),
            "{out}"
        );
        assert!(out.contains("<p class=\"markdown-alert-title\">"), "{out}");
        assert!(out.contains("<em>x</em>"));
    }
    assert!(html("> [!note]\n> x\n").contains("markdown-alert-note"));
    assert!(
        html("> [!TIP] Custom <b>title</b>\n> x\n")
            .contains(">Custom &lt;b&gt;title&lt;/b&gt;</p>")
    );
}

#[test]
fn math_and_diagrams_pass_through_as_source() {
    let out = html(
        "Inline $a<b$ and $$\\sum_i x_i$$\n\n$$\nE=mc^2\n$$\n\n```math\n\\frac12\n```\n\n\
         ```mermaid\ngraph TD; A-->B\n```\n\nPrice $5 and $6.",
    );
    assert!(
        out.contains("<span data-math=\"inline\">a&lt;b</span>"),
        "{out}"
    );
    assert!(
        out.contains("<span data-math=\"display\">\\sum_i x_i</span>"),
        "{out}"
    );
    assert!(out.contains("E=mc^2"));
    assert!(
        out.contains("<span data-math=\"display\">\\frac12\n</span>"),
        "{out}"
    );
    assert!(
        out.contains("<pre data-lang=\"mermaid\"><code>graph TD; A--&gt;B\n</code></pre>"),
        "{out}"
    );
    assert!(out.contains("Price $5 and $6."), "{out}");
}

#[test]
fn code_languages_keep_non_ascii_names() {
    let out = html("```C♯ extra words\nx\n```\n\n```日本語\ny\n```\n\n```\nplain\n```\n");
    assert!(out.contains("<pre data-lang=\"c♯\"><code>x\n"), "{out}");
    assert!(out.contains("<pre data-lang=\"日本語\"><code>y\n"), "{out}");
    assert!(out.contains("<pre><code>plain\n"), "{out}");
    let out = html("```\"><x\nz\n```\n");
    assert!(out.contains("data-lang=\"&quot;&gt;&lt;x\""), "{out}");
}

#[test]
fn images_load_only_from_attachments() {
    let out = html(
        "![shot](gk-file://abc123 \"t\") ![ext](https://example.com/x.png) ![rel](x.png) \
         ![data](data:image/png;base64,AA==)",
    );
    assert!(
        out.contains("<img src=\"gk-file://abc123\" alt=\"shot\" title=\"t\">"),
        "{out}"
    );
    assert!(
        out.contains("<img data-external-src=\"https://example.com/x.png\" alt=\"ext\">"),
        "{out}"
    );
    assert!(out.contains("<img alt=\"rel\">"), "{out}");
    assert!(out.contains("<img alt=\"data\">"), "{out}");
}

#[test]
fn links_keep_only_safe_targets() {
    let out = html(
        "[a](https://a.example) [b](mailto:x@example.com) [c](relative/page) [d](#heading) \
         <https://auto.example> www.bare.example",
    );
    assert!(
        out.contains("<a href=\"https://a.example\" rel=\"noopener noreferrer\">a</a>"),
        "{out}"
    );
    assert!(out.contains("href=\"mailto:x@example.com\""), "{out}");
    assert!(
        out.contains("<a rel=\"noopener noreferrer\">c</a>"),
        "{out}"
    );
    assert!(out.contains("href=\"#heading\""), "{out}");
    assert!(out.contains("href=\"https://auto.example\""), "{out}");
    assert!(out.contains("href=\"http://www.bare.example\""), "{out}");
}

#[test]
fn footnote_ids_carry_the_document_id() {
    let source = "One[^a] two[^a].\n\n[^a]: The note.\n";
    let one = render(source, &RenderOptions::new("task-7"));
    assert!(one.contains("href=\"#fn-task_7-a\""), "{one}");
    assert!(one.contains("id=\"fnref-task_7-a\""), "{one}");
    assert!(one.contains("<li id=\"fn-task_7-a\">"), "{one}");
    assert!(one.contains("href=\"#fnref-task_7-a\""), "{one}");
    let two = render(source, &RenderOptions::new("task-8"));
    assert!(two.contains("id=\"fn-task_8-a\""), "{two}");
    assert!(violations(&one).is_empty(), "{:?}", violations(&one));
}

#[test]
fn the_sample_document_renders_completely() {
    let source = include_str!("fixtures/sample.md");
    let out = html(source);
    for needle in [
        "data-ref=\"TM-41\"",
        "data-wiki=\"Release plan#Dates\"",
        "data-item=\"1\"",
        "data-item=\"4\" checked",
        "data-math=\"inline\"",
        "data-math=\"display\"",
        "data-lang=\"rust\"",
        "data-lang=\"mermaid\"",
        "class=\"markdown-alert markdown-alert-warning\"",
        "<li class=\"task-item\">",
        "src=\"gk-file://abc123\"",
        "data-external-src=\"https://example.com/x.png\"",
        "<kbd>Ctrl</kbd>",
        "href=\"#fn-d1-1\"",
        "id=\"fn-d1-1\"",
        "<th align=\"center\">a</th>",
        "<details>",
    ] {
        assert!(out.contains(needle), "missing {needle} in {out}");
    }
    assert!(
        !out.contains("data-ref=\"TM-9\""),
        "a code span must not link"
    );
    assert!(violations(&out).is_empty(), "{:?}", violations(&out));
}

#[test]
fn references_follow_the_reader() {
    let refs = references(include_str!("fixtures/sample.md"));
    assert_eq!(refs.task_keys, ["TM-41", "XX-1", "TM-7"]);
    assert_eq!(refs.wiki_links.len(), 1);
    assert_eq!(refs.wiki_links[0].page, "Release plan");
    assert_eq!(refs.wiki_links[0].heading.as_deref(), Some("Dates"));
    let refs = references("[[Page]] [[ Page ]] [[#Local]] [[Page#A|x]]");
    let links: Vec<_> = refs
        .wiki_links
        .iter()
        .map(|l| (l.page.as_str(), l.heading.as_deref()))
        .collect();
    assert_eq!(
        links,
        [("Page", None), ("", Some("Local")), ("Page", Some("A"))]
    );
}

#[test]
fn plain_text_for_search() {
    let text = plain_text(
        "# Title\n\nSee **TM-41** and [[Page|label]], `code`, $x^2$.\n\n- [ ] item one\n\n\
         ```sql\nSELECT 1;\n```\n\n<details><summary>More</summary>hidden &amp; found</details>\n\n\
         [link text](https://url.example/should-not-appear)\n\n> [!TIP] Tip title\n> tip body\n\n\
         | h1 | h2 |\n|---|---|\n| c1 | c2 |\n",
    );
    assert!(text.contains("Title\n"));
    assert!(text.contains("See TM-41 and label, code, x^2."));
    assert!(text.contains("item one"));
    assert!(text.contains("SELECT 1;"));
    assert!(text.contains("hidden & found"), "{text}");
    assert!(text.contains("link text"));
    assert!(text.contains("Tip title\ntip body"), "{text}");
    assert!(text.contains("c1 c2"), "{text}");
    assert!(!text.contains("url.example"));
    assert!(!text.contains('<'));
}

#[test]
fn analyze_matches_the_single_jobs() {
    let source = include_str!("fixtures/typical.md");
    let all = analyze(source);
    assert_eq!(all.checklist, gk_markdown::checklist(source));
    assert_eq!(all.references, references(source));
    assert_eq!(all.plain_text, plain_text(source));
}
