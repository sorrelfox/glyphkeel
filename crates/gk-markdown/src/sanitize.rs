// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The final allowlist on the reader's HTML, enforced with ammonia.

use std::collections::{HashMap, HashSet};

use ammonia::{Builder, UrlRelative};

/// `<script>` and `<style>` after the raw-HTML filter renamed them.
const RENAMED_SCRIPT: &str = "gkx-script";
const RENAMED_STYLE: &str = "gkx-style";

/// The two allowlists of the reader's HTML.
pub mod allowlist {
    /// Tags a user may write as raw HTML in Markdown: details and summary,
    /// keyboard keys, sub- and superscript, highlighted text, line breaks,
    /// images (attachments only, with a size) and links (to the web and to
    /// mail addresses). Every other tag is removed and its text kept.
    pub const USER_TAGS: &[&str] = &[
        "details", "summary", "kbd", "sub", "sup", "mark", "br", "img", "a",
    ];

    /// Tags the reader's HTML may contain: what the renderer writes for
    /// CommonMark, GitHub Flavored Markdown and the extensions, plus
    /// [`USER_TAGS`].
    pub const READER_TAGS: &[&str] = &[
        "p",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "ul",
        "ol",
        "li",
        "pre",
        "code",
        "em",
        "strong",
        "del",
        "hr",
        "br",
        "a",
        "img",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "section",
        "sup",
        "span",
        "div",
        "input",
        "details",
        "summary",
        "kbd",
        "sub",
        "mark",
    ];

    /// Attributes allowed on every tag of [`READER_TAGS`].
    pub const GENERIC_ATTRIBUTES: &[&str] = &["title"];

    /// Attributes only the renderer writes. The reader acts on them, so the
    /// sanitizer drops any that a render did not mark as its own.
    pub const RENDERER_ATTRIBUTES: &[&str] = &[
        "class",
        "data-item",
        "data-ref",
        "data-comment",
        "data-wiki",
        "data-math",
        "data-lang",
        "data-external-src",
        "data-macro",
        "data-macro-note",
        "data-macro-arg",
    ];

    /// Schemes a link (`<a href>`) may use. Links to a fragment (`#fn-…`) are
    /// allowed too, and links to an attachment in the exact form of
    /// [`is_attachment_link`].
    pub const LINK_SCHEMES: &[&str] = &["http", "https", "mailto"];

    /// The only scheme an image (`<img src>`) may load from: attachments.
    pub const IMAGE_SCHEME: &str = "gk-file";

    /// Whether `url` is a link to an attachment exactly as the editor writes
    /// it: `gk-file://` and the attachment's ID, a UUID in lower case. No
    /// other host, path, query, fragment or spelling is accepted.
    #[must_use]
    pub fn is_attachment_link(url: &str) -> bool {
        url.strip_prefix("gk-file://").is_some_and(|id| {
            id.len() == 36
                && id.bytes().enumerate().all(|(i, b)| match i {
                    8 | 13 | 18 | 23 => b == b'-',
                    _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
                })
        })
    }

    /// The attributes allowed on `tag` besides [`GENERIC_ATTRIBUTES`].
    #[must_use]
    pub fn reader_attributes(tag: &str) -> &'static [&'static str] {
        match tag {
            "a" => &[
                "href",
                "id",
                "class",
                "data-ref",
                "data-comment",
                "data-wiki",
                "data-footnote-ref",
                "data-footnote-backref",
                "rel",
            ],
            "img" => &["src", "alt", "width", "height", "data-external-src"],
            "input" => &["type", "checked", "disabled", "data-item"],
            "li" => &["id", "class"],
            "ul" => &["data-macro"],
            "ol" => &["start"],
            "th" | "td" => &["align"],
            "sup" => &["class"],
            "section" => &["class", "data-footnotes"],
            "span" => &["data-math", "data-macro"],
            "pre" => &["data-lang"],
            "div" => &["class", "data-macro"],
            "p" => &["class", "data-macro-note", "data-macro-arg"],
            "details" => &["open"],
            _ => &[],
        }
    }
}

use allowlist::{
    GENERIC_ATTRIBUTES, IMAGE_SCHEME, LINK_SCHEMES, READER_TAGS, RENDERER_ATTRIBUTES,
    is_attachment_link, reader_attributes,
};

/// Sanitizes `html` written by one render. `mark` is that render's mark on
/// its own attributes; `footnote_prefix` is the document's footnote id
/// prefix.
pub(crate) fn clean(html: &str, mark: &str, footnote_prefix: &str) -> String {
    builder(mark.to_owned(), footnote_prefix.to_owned())
        .clean(html)
        .to_string()
}

fn builder(mark: String, footnote_prefix: String) -> Builder<'static> {
    // `Builder::empty()` empties only the tags; every other option keeps
    // ammonia's default. So every option is set here.
    let mut b = Builder::empty();
    b.tags(READER_TAGS.iter().copied().collect());
    // Script and style text is removed with the element, also when the
    // raw-HTML filter renamed it.
    b.clean_content_tags(["script", "style", RENAMED_SCRIPT, RENAMED_STYLE].into());
    b.generic_attributes(GENERIC_ATTRIBUTES.iter().copied().collect());
    b.generic_attribute_prefixes(HashSet::new());
    let mut per_tag: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    for &tag in READER_TAGS {
        let attributes = reader_attributes(tag)
            .iter()
            .copied()
            // `rel` is written by ammonia itself (`link_rel`); a user's own
            // is dropped.
            .filter(|&a| a != "rel")
            .collect::<HashSet<_>>();
        if !attributes.is_empty() {
            per_tag.insert(tag, attributes);
        }
    }
    b.tag_attributes(per_tag);
    b.tag_attribute_values(HashMap::new());
    b.set_tag_attribute_values(HashMap::new());
    b.allowed_classes(HashMap::new());
    b.filter_style_properties(HashSet::new());
    b.url_schemes(LINK_SCHEMES.iter().copied().chain([IMAGE_SCHEME]).collect());
    // Relative URLs reach the attribute filter, which allows fragments only.
    b.url_relative(UrlRelative::PassThrough);
    b.link_rel(Some("noopener noreferrer"));
    b.strip_comments(true);
    b.id_prefix(None);
    let mark = format!("{mark}:");
    b.attribute_filter(move |tag, attribute, value| {
        filter_attribute(tag, attribute, value, &mark, &footnote_prefix).map(Into::into)
    });
    b
}

/// Decides one attribute: `None` drops it, otherwise its new value.
fn filter_attribute(
    tag: &str,
    attribute: &str,
    value: &str,
    mark: &str,
    footnote_prefix: &str,
) -> Option<String> {
    if RENDERER_ATTRIBUTES.contains(&attribute) {
        // comrak's footnote markup uses fixed classes that the reader doesn't
        // act on.
        if attribute == "class"
            && matches!(
                (tag, value),
                ("section", "footnotes") | ("a", "footnote-backref") | ("sup", "footnote-ref")
            )
        {
            return Some(value.to_owned());
        }
        let value = value.strip_prefix(mark)?;
        if attribute == "data-external-src" && !is_web_url(value) {
            return None;
        }
        return Some(value.to_owned());
    }
    match (tag, attribute) {
        ("a", "href") => {
            let trimmed = value.trim();
            let fragment = trimmed.starts_with('#') && !trimmed.starts_with("#/");
            let allowed = fragment
                || is_web_url(trimmed)
                || has_scheme(trimmed, "mailto")
                || is_attachment_link(value);
            allowed.then(|| value.to_owned())
        }
        ("img", "src") => has_scheme(value.trim(), IMAGE_SCHEME).then(|| value.to_owned()),
        ("input", "type") => Some("checkbox".to_owned()),
        ("input", "disabled") => Some(String::new()),
        (_, "id") => {
            // Only this document's footnote ids, so raw HTML can't take the
            // id of anything else on the page.
            let own = ["fn-", "fnref-"].iter().any(|kind| {
                value
                    .strip_prefix(kind)
                    .and_then(|rest| rest.strip_prefix(footnote_prefix))
                    .is_some_and(|rest| rest.starts_with('-'))
            });
            let plain = value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '%' | '.'));
            (own && plain).then(|| value.to_owned())
        }
        ("img", "width" | "height") => {
            let ok = (1..=5).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit());
            ok.then(|| value.to_owned())
        }
        ("th" | "td", "align") => {
            matches!(value, "left" | "right" | "center").then(|| value.to_owned())
        }
        _ => Some(value.to_owned()),
    }
}

fn is_web_url(value: &str) -> bool {
    has_scheme(value, "https") || has_scheme(value, "http")
}

/// Whether `value` starts with `scheme:`, in any case.
fn has_scheme(value: &str, scheme: &str) -> bool {
    value.len() > scheme.len()
        && value.as_bytes()[scheme.len()] == b':'
        && value[..scheme.len()].eq_ignore_ascii_case(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw_html::RENAMED_PREFIX;

    #[test]
    fn renamed_names_follow_the_filter() {
        assert_eq!(RENAMED_SCRIPT, format!("{RENAMED_PREFIX}script"));
        assert_eq!(RENAMED_STYLE, format!("{RENAMED_PREFIX}style"));
    }

    #[test]
    fn schemes_are_matched_in_any_case() {
        assert!(has_scheme("HTTPS://x", "https"));
        assert!(!has_scheme("https", "https"));
        assert!(!has_scheme("httpsx://", "https"));
        assert!(!has_scheme("é:", "https"));
    }

    #[test]
    fn attachment_links_have_one_exact_form() {
        let id = "0b5e2c1a-7f3d-4e8a-9c21-5d6f7a8b9c0d";
        assert!(is_attachment_link(&format!("gk-file://{id}")));
        for bad in [
            String::from("gk-file://"),
            String::from("gk-file://abc"),
            format!("GK-FILE://{id}"),
            format!("gk-file:{id}"),
            format!("gk-file:/{id}"),
            format!("gk-file:///{id}"),
            format!("gk-file://localhost/{id}"),
            format!("gk-file://{id}/"),
            format!("gk-file://{id}/thumb"),
            format!("gk-file://{id}/../x"),
            format!("gk-file://{id}?x=1"),
            format!("gk-file://{id}#x"),
            format!("gk-file://{id} "),
            format!(" gk-file://{id}"),
            format!("gk-file://{}", id.to_uppercase()),
            format!("gk-file://{}", id.replace('-', "")),
            format!("gk-file://{}", id.replacen('-', "%2d", 1)),
            format!("gk-file://{id}0"),
            format!("gk-file://{}", &id[..35]),
            format!("gk-file://user@{}", &id[5..]),
        ] {
            assert!(!is_attachment_link(&bad), "{bad:?}");
        }
    }
}
