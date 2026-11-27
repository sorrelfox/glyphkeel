// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki pages as web pages of their own, for the export.
//!
//! The text is rendered by the reader's renderer (sanitized, as the app
//! shows it); then the parts only the app understands are rewritten for a
//! browser: wiki links become relative links to the exported pages,
//! attachments relative links to the copied files, headings get the
//! anchors that links to them use, and checkboxes can't be ticked. Each
//! page carries its own small style sheet, so it needs nothing else.

use std::collections::HashMap;

use gk_core::Id;

use super::wiki_export::anchor;
use crate::messages;

/// The page that lists the exported pages, when there are several.
pub(super) const INDEX: &str = "index.html";

/// The style sheet of every exported page: the system's fonts, light and
/// dark, sizes in `rem` and `em`, and only logical properties, so pages
/// read the same in any writing direction.
const STYLE: &str = "\
:root{color-scheme:light dark;--fg:#1d1d1f;--muted:#5f6368;--line:#d9dce1;--code:#f3f4f6;\
--accent:#2457c5;--note:#2457c5;--tip:#1a7f37;--warning:#9a6700;--caution:#cf222e}\
@media (prefers-color-scheme:dark){:root{--fg:#e6e6e6;--muted:#a0a4aa;--line:#3a3d42;\
--code:#24272b;--accent:#7aa2f7;--note:#7aa2f7;--tip:#57c27a;--warning:#d9a53a;--caution:#f07178}}\
body{margin:0;font:1rem/1.6 system-ui,sans-serif;color:var(--fg);background:Canvas}\
.page{max-inline-size:48rem;margin-inline:auto;padding-block:2rem 4rem;padding-inline:1.25rem}\
nav{font-size:.875rem;color:var(--muted);margin-block-end:1.5rem}\
nav a{color:inherit}nav ol{list-style:none;margin:0;padding:0;display:flex;flex-wrap:wrap;gap:.25rem .5rem}\
nav li+li::before{content:'/';margin-inline-end:.5rem;opacity:.6}\
h1,h2,h3,h4,h5,h6{line-height:1.25;margin-block:1.5em .5em}\
a{color:var(--accent)}img{max-inline-size:100%;block-size:auto}\
pre,code{font-family:ui-monospace,monospace;font-size:.9em}\
code{background:var(--code);padding-inline:.25em;border-radius:.25em}\
pre{background:var(--code);padding:.75em 1em;overflow:auto;border-radius:.375em}pre code{padding:0}\
blockquote{margin-inline:0;padding-inline-start:1em;border-inline-start:.25em solid var(--line);\
color:var(--muted)}\
table{border-collapse:collapse;display:block;overflow:auto}\
th,td{border:1px solid var(--line);padding:.375em .75em}\
li.task-item{list-style:none}li.task-item input{margin-inline-end:.5em}\
.markdown-alert{padding-inline-start:1em;border-inline-start:.25em solid var(--note)}\
.markdown-alert-tip{border-color:var(--tip)}.markdown-alert-important{border-color:var(--accent)}\
.markdown-alert-warning{border-color:var(--warning)}.markdown-alert-caution{border-color:var(--caution)}\
.markdown-alert-title{font-weight:600}\
.contents ul{padding-inline-start:1.25em}";

/// A page's text to export: the Markdown, the id its footnotes are made
/// from, and what its macros show (without it they stay as written).
pub(super) struct Body<'a> {
    pub(super) text: &'a str,
    pub(super) document_id: &'a str,
    pub(super) macros: Option<&'a dyn gk_markdown::MacroSource>,
}

/// One exported page as a web page: `title`, the rendered `body` (its
/// macros expanded, with their notes in English) with its links rewritten
/// by `page_link` (a wiki link's target, `Page#Heading`, to an address, or
/// `None` when the page isn't exported) and `file_link` (an attachment to
/// an address); `trail`, the exported pages above it (title, address), and
/// `index`, the address of the list of pages.
pub(super) fn page(
    title: &str,
    body: &Body<'_>,
    trail: &[(String, String)],
    index: Option<&str>,
    page_link: &dyn Fn(&str) -> Option<String>,
    file_link: &dyn Fn(Id) -> Option<String>,
) -> String {
    let notes = |note: &gk_markdown::Note| macro_note(note);
    let mut options = gk_markdown::RenderOptions::new(body.document_id).with_macro_notes(&notes);
    if let Some(macros) = body.macros {
        options = options.with_macros(macros);
    }
    let rendered = gk_markdown::render(body.text, &options);
    let content = rewrite(&rendered, page_link, file_link);
    let mut nav = String::new();
    if index.is_some() || !trail.is_empty() {
        nav.push_str(&format!(
            "<nav aria-label=\"{}\"><ol>",
            escape(&messages::text("wiki-export-trail", &[]))
        ));
        if let Some(index) = index {
            nav.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                escape(index),
                escape(&messages::text("wiki-export-contents", &[]))
            ));
        }
        for (name, href) in trail {
            nav.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                escape(href),
                escape(name)
            ));
        }
        nav.push_str("</ol></nav>\n");
    }
    document(
        title,
        &format!(
            "{nav}<article>\n<h1 class=\"title\">{}</h1>\n{content}</article>",
            escape(title)
        ),
    )
}

/// A macro's note in words: the engine's English text.
pub(crate) fn macro_note(note: &gk_markdown::Note) -> String {
    use gk_markdown::Note;
    let value = note.value();
    match note {
        Note::NoHeadings => messages::text("wiki-macro-toc-empty", &[]),
        Note::NoChildren => messages::text("wiki-macro-children-empty", &[]),
        Note::IncludeMissing(_) => messages::text("wiki-macro-include-missing", &[("page", value)]),
        Note::IncludeLoop(_) => messages::text("wiki-macro-include-loop", &[("page", value)]),
        Note::IncludeTooDeep(_) => messages::text(
            "wiki-macro-include-too-deep",
            &[
                ("page", value),
                ("max", gk_markdown::MAX_INCLUDE_DEPTH.to_string()),
            ],
        ),
        Note::IncludeTooMany(_) => messages::text(
            "wiki-macro-include-too-many",
            &[
                ("page", value),
                ("max", gk_markdown::MAX_INCLUDES.to_string()),
            ],
        ),
        Note::NoTasks => messages::text("wiki-macro-tasks-empty", &[]),
        Note::MoreTasks(_) => messages::text("wiki-macro-tasks-more", &[("count", value)]),
        Note::TasksError(error) => {
            let args: Vec<(&str, String)> = error
                .args
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()))
                .collect();
            // The engine's own message, or the query language's mistake.
            let file = if messages::ENGINE.has(&error.id) {
                messages::ENGINE
            } else {
                messages::Messages::new(gk_query::error::MESSAGES_FTL)
            };
            let why = file.english(&error.id, &args);
            messages::text("wiki-macro-tasks-error", &[("error", why)])
        }
    }
}

/// The list of exported pages: `entries` are (depth, title, address), in
/// tree order.
pub(super) fn index(heading: &str, entries: &[(usize, String, String)]) -> String {
    let mut list = String::new();
    let mut depth = 0usize;
    let mut open = false;
    list.push_str("<ul>");
    for (level, name, href) in entries {
        let level = (*level).min(depth + 1);
        if level > depth {
            list.push_str("<ul>");
        } else {
            if open {
                list.push_str("</li>");
            }
            for _ in level..depth {
                list.push_str("</ul></li>");
            }
        }
        depth = level;
        list.push_str(&format!(
            "<li><a href=\"{}\">{}</a>",
            escape(href),
            escape(name)
        ));
        open = true;
    }
    if open {
        list.push_str("</li>");
    }
    for _ in 0..depth {
        list.push_str("</ul></li>");
    }
    list.push_str("</ul>");
    document(
        heading,
        &format!(
            "<h1>{}</h1>\n<div class=\"contents\">{list}</div>",
            escape(heading)
        ),
    )
}

fn document(title: &str, main: &str) -> String {
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main class=\"page\">\n{main}\n</main>\n</body>\n</html>\n",
        escape(title)
    )
}

/// Text for HTML: in an element or a quoted attribute.
pub(super) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The text of an attribute value or of text between tags, with the
/// entities the renderer writes read back.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// An opening tag as written: its name, its attributes (values as
/// written, still escaped) and its length in the text.
struct Tag {
    name: String,
    attributes: Vec<(String, String)>,
    len: usize,
}

/// Reads the opening tag at the start of `text` (which starts with `<`).
fn parse_tag(text: &str) -> Option<Tag> {
    let bytes = text.as_bytes();
    let mut i = 1;
    if !bytes.get(i)?.is_ascii_alphabetic() {
        return None;
    }
    while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
        i += 1;
    }
    let name = text[1..i].to_ascii_lowercase();
    let mut attributes = Vec::new();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i)? {
            b'>' => {
                return Some(Tag {
                    name,
                    attributes,
                    len: i + 1,
                });
            }
            b'/' => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let start = i;
        while i < bytes.len() && !matches!(bytes[i], b'=' | b'>' | b'/') {
            if bytes[i].is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        let attribute = text[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            match bytes.get(i)? {
                quote @ (b'"' | b'\'') => {
                    let close = text[i + 1..].find(char::from(*quote))? + i + 1;
                    value = text[i + 1..close].to_owned();
                    i = close + 1;
                }
                _ => {
                    let from = i;
                    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                        i += 1;
                    }
                    value = text[from..i].to_owned();
                }
            }
        }
        if attribute.is_empty() {
            return None;
        }
        attributes.push((attribute, value));
    }
}

/// The attachment an address names: `gk-file://<id>`.
fn attachment(url: &str) -> Option<Id> {
    url.strip_prefix("gk-file://")?.parse().ok()
}

/// Rewrites the rendered HTML for a browser without the app.
fn rewrite(
    html: &str,
    page_link: &dyn Fn(&str) -> Option<String>,
    file_link: &dyn Fn(Id) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(html.len() + html.len() / 8);
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        let Some(tag) = parse_tag(rest) else {
            out.push('<');
            rest = &rest[1..];
            continue;
        };
        let after = &rest[tag.len..];
        let mut attributes: Vec<(String, String)> = Vec::new();
        match tag.name.as_str() {
            "a" => {
                for (name, value) in &tag.attributes {
                    let text = unescape(value);
                    match name.as_str() {
                        "data-wiki" => {
                            if let Some(href) = page_link(&text) {
                                attributes.push(("href".to_owned(), href));
                            }
                        }
                        "href" => {
                            if let Some(id) = attachment(&text) {
                                if let Some(href) = file_link(id) {
                                    attributes.push(("href".to_owned(), href));
                                }
                            } else {
                                attributes.push((name.clone(), text));
                            }
                        }
                        "data-ref" | "data-comment" => {}
                        _ => attributes.push((name.clone(), text)),
                    }
                }
            }
            "img" => {
                for (name, value) in &tag.attributes {
                    let text = unescape(value);
                    match name.as_str() {
                        "src" => {
                            if let Some(src) = attachment(&text).and_then(file_link) {
                                attributes.push(("src".to_owned(), src));
                            }
                        }
                        "data-external-src" => attributes.push(("src".to_owned(), text)),
                        _ => attributes.push((name.clone(), text)),
                    }
                }
            }
            "input" => {
                attributes = tag
                    .attributes
                    .iter()
                    .map(|(n, v)| (n.clone(), unescape(v)))
                    .collect();
                attributes.push(("disabled".to_owned(), String::new()));
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                attributes = tag
                    .attributes
                    .iter()
                    .map(|(n, v)| (n.clone(), unescape(v)))
                    .collect();
                let close = format!("</{}>", tag.name);
                let inner = after.find(&close).map_or("", |end| &after[..end]);
                let base = anchor(&plain(inner));
                if !base.is_empty() {
                    let n = ids.entry(base.clone()).or_insert(0);
                    let id = if *n == 0 {
                        base.clone()
                    } else {
                        format!("{base}-{n}")
                    };
                    *n += 1;
                    attributes.push(("id".to_owned(), id));
                }
            }
            _ => {
                out.push_str(&rest[..tag.len]);
                rest = after;
                continue;
            }
        }
        out.push('<');
        out.push_str(&tag.name);
        for (name, value) in attributes {
            out.push(' ');
            out.push_str(&name);
            if !value.is_empty() || name != "disabled" {
                out.push_str("=\"");
                out.push_str(&escape(&value));
                out.push('"');
            }
        }
        out.push('>');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The text of some HTML: tags dropped, entities read.
fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    unescape(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn wiki_links_attachments_and_headings_are_made_for_a_browser() {
        let id = "0193b2a4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
        let body = format!(
            "## Dates & times\n\nSee [[Plan#Dates & times|the plan]], [[Gone]] and \
![chart](gk-file://{id}).\n\n- [x] done\n\n## Dates & times\n"
        );
        let html = page(
            "Home <1>",
            &Body {
                text: &body,
                document_id: "doc",
                macros: None,
            },
            &[("Top".to_owned(), "../top.html".to_owned())],
            Some("../index.html"),
            &|link| (link.starts_with("Plan")).then(|| "plan.html#dates--times".to_owned()),
            &|file| (file.to_string() == id).then(|| "../files/chart.png".to_owned()),
        );
        assert!(html.starts_with("<!doctype html>"), "{html}");
        assert!(html.contains("<title>Home &lt;1&gt;</title>"), "{html}");
        assert!(
            html.contains(
                "<a href=\"plan.html#dates--times\" rel=\"noopener noreferrer\">the plan</a>"
            ),
            "{html}"
        );
        assert!(!html.contains("data-wiki"), "{html}");
        assert!(
            html.contains("<a rel=\"noopener noreferrer\">Gone</a>"),
            "{html}"
        );
        assert!(html.contains("src=\"../files/chart.png\""), "{html}");
        assert!(!html.contains("gk-file"), "{html}");
        assert!(html.contains("<h2 id=\"dates--times\">"), "{html}");
        assert!(html.contains("<h2 id=\"dates--times-1\">"), "{html}");
        assert!(html.contains(" disabled"), "{html}");
        assert!(html.contains("<a href=\"../top.html\">Top</a>"), "{html}");
        assert!(html.contains("href=\"../index.html\""), "{html}");
        assert!(!html.contains("<script"), "{html}");
    }

    #[test]
    fn hostile_text_stays_text() {
        let html = page(
            "x",
            &Body {
                text: "<script>alert(1)</script><img src=x onerror=alert(1)> [a](javascript:alert(1))",
                document_id: "doc",
                macros: None,
            },
            &[],
            None,
            &none,
            &|_| None,
        );
        assert!(!html.contains("<script>alert"), "{html}");
        assert!(!html.contains("onerror"), "{html}");
        assert!(!html.contains("href=\"javascript"), "{html}");
        assert!(!html.contains("<nav"), "{html}");
    }

    #[test]
    fn the_index_nests_like_the_tree() {
        let html = index(
            "Wiki",
            &[
                (0, "A".into(), "a.html".into()),
                (1, "B".into(), "a/b.html".into()),
                (2, "C".into(), "a/b/c.html".into()),
                (0, "D".into(), "d.html".into()),
            ],
        );
        assert!(
            html.contains(
                "<ul><li><a href=\"a.html\">A</a><ul><li><a href=\"a/b.html\">B</a><ul><li>\
<a href=\"a/b/c.html\">C</a></li></ul></li></ul></li><li><a href=\"d.html\">D</a></li></ul>"
            ),
            "{html}"
        );
    }

    #[test]
    fn entities_are_read_back() {
        assert_eq!(
            unescape("a &amp; b &lt;c&gt; &#39;d&#x27;"),
            "a & b <c> 'd'"
        );
        assert_eq!(unescape("&unknown; &"), "&unknown; &");
    }
}
