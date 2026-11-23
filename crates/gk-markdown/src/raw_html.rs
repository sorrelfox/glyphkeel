// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The filter on raw HTML typed by the user, before the whole document is
//! sanitized.
//!
//! - Every tag whose name is not in [`USER_TAGS`] is renamed to
//!   `gkx-<name>`, which the sanitizer doesn't allow, so it is removed with
//!   its attributes. This happens everywhere in the raw text, also inside
//!   comments and attribute values, so no tokenizer state can let a tag
//!   through: an HTML tag only opens with `<` or `</` and a letter.
//! - A piece of raw HTML that ends inside a tag, a quoted value or a comment
//!   is closed, so the renderer's own markup after it can't become part of
//!   the user's tag.
//!
//! This is not the security boundary on its own: the sanitizer runs after it
//! on everything.

use crate::sanitize::allowlist::USER_TAGS;

/// The prefix that renamed tags get.
pub(crate) const RENAMED_PREFIX: &str = "gkx-";

/// Appends `raw` to `out`, filtered.
pub(crate) fn filter(raw: &str, out: &mut String) {
    rename_tags(raw, out);
    out.push_str(closing(raw));
}

fn rename_tags(raw: &str, out: &mut String) {
    let bytes = raw.as_bytes();
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let name_start = if bytes.get(i + 1) == Some(&b'/') {
            i + 2
        } else {
            i + 1
        };
        if !bytes.get(name_start).is_some_and(u8::is_ascii_alphabetic) {
            i += 1;
            continue;
        }
        // A tag name runs to whitespace, `/` or `>`, as in an HTML tokenizer.
        // Stopping at `<` as well means every `<` is looked at, also one
        // inside a name such as `<scr<script>`.
        let name_len = bytes[name_start..]
            .iter()
            .take_while(|&&b| {
                !matches!(
                    b,
                    b'\t' | b'\n' | b'\x0c' | b'\r' | b' ' | b'/' | b'>' | b'<'
                )
            })
            .count();
        let name_end = name_start + name_len;
        let name = raw[name_start..name_end].to_ascii_lowercase();
        if !USER_TAGS.contains(&name.as_str()) {
            out.push_str(&raw[copied..name_start]);
            out.push_str(RENAMED_PREFIX);
            out.push_str(&name);
            copied = name_end;
        }
        i = name_end;
    }
    out.push_str(&raw[copied..]);
}

/// What closes a piece of raw HTML that ends in the middle of markup.
fn closing(raw: &str) -> &'static str {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        /// Inside a tag; `after_equals` when the next value may be quoted.
        Tag {
            after_equals: bool,
        },
        Quoted(u8),
        Comment,
        /// `<!…>`, `<?…>` and `</` without a letter: until the next `>`.
        Declaration,
    }

    let bytes = raw.as_bytes();
    let mut state = State::Text;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        state = match state {
            State::Text if b == b'<' => {
                let rest = &bytes[i + 1..];
                if rest.starts_with(b"!--") {
                    i += 3;
                    // `<!-->` and `<!--->` are complete comments.
                    if bytes[i + 1..].starts_with(b">") {
                        i += 1;
                        State::Text
                    } else if bytes[i + 1..].starts_with(b"->") {
                        i += 2;
                        State::Text
                    } else {
                        State::Comment
                    }
                } else if rest.first().is_some_and(u8::is_ascii_alphabetic)
                    || (rest.first() == Some(&b'/')
                        && rest.get(1).is_some_and(u8::is_ascii_alphabetic))
                {
                    State::Tag {
                        after_equals: false,
                    }
                } else if matches!(rest.first(), Some(b'!' | b'?' | b'/')) {
                    State::Declaration
                } else {
                    State::Text
                }
            }
            State::Text => State::Text,
            State::Tag { after_equals } => match b {
                b'>' => State::Text,
                b'=' => State::Tag { after_equals: true },
                b'"' | b'\'' if after_equals => State::Quoted(b),
                b'\t' | b'\n' | b'\x0c' | b'\r' | b' ' => State::Tag { after_equals },
                _ => State::Tag {
                    after_equals: false,
                },
            },
            State::Quoted(quote) if b == quote => State::Tag {
                after_equals: false,
            },
            State::Quoted(quote) => State::Quoted(quote),
            State::Comment => {
                let rest = &bytes[i..];
                if rest.starts_with(b"-->") {
                    i += 2;
                    State::Text
                } else if rest.starts_with(b"--!>") {
                    i += 3;
                    State::Text
                } else {
                    State::Comment
                }
            }
            State::Declaration if b == b'>' => State::Text,
            State::Declaration => State::Declaration,
        };
        i += 1;
    }
    match state {
        State::Text => "",
        State::Tag { .. } | State::Declaration => ">",
        State::Quoted(b'"') => "\">",
        State::Quoted(_) => "'>",
        State::Comment => "-->",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filtered(raw: &str) -> String {
        let mut out = String::new();
        filter(raw, &mut out);
        out
    }

    #[test]
    fn keeps_user_tags_and_renames_the_rest() {
        assert_eq!(
            filtered("<details open><SUMMARY>s</SUMMARY><kbd>k</kbd></details>"),
            "<details open><SUMMARY>s</SUMMARY><kbd>k</kbd></details>"
        );
        assert_eq!(
            filtered("<script>x</script><DIV class=a>"),
            "<gkx-script>x</gkx-script><gkx-div class=a>"
        );
        assert_eq!(filtered("<svg/onload=x>"), "<gkx-svg/onload=x>");
        assert_eq!(filtered("<scr<script>ipt>"), "<gkx-scr<gkx-script>ipt>");
    }

    #[test]
    fn renames_inside_comments_and_values() {
        assert_eq!(filtered("<!-- --!><script>"), "<!-- --!><gkx-script>");
        assert_eq!(
            filtered("<a title=\"<iframe>\">"),
            "<a title=\"<gkx-iframe>\">"
        );
    }

    #[test]
    fn leaves_text_alone() {
        assert_eq!(filtered("a < b, 1<2, <3, </ >"), "a < b, 1<2, <3, </ >");
    }

    #[test]
    fn closes_unfinished_markup() {
        assert_eq!(filtered("<a data-ref"), "<a data-ref>");
        assert_eq!(filtered("<a title=\"x"), "<a title=\"x\">");
        assert_eq!(filtered("<a title = 'x"), "<a title = 'x'>");
        assert_eq!(filtered("<a title=x\"y"), "<a title=x\"y>");
        assert_eq!(filtered("<!-- open"), "<!-- open-->");
        assert_eq!(filtered("<!-->"), "<!-->");
        assert_eq!(filtered("<!--->"), "<!--->");
        assert_eq!(filtered("<!DOCTYPE html"), "<!DOCTYPE html>");
        assert_eq!(filtered("<a href=\"x\">done</a>"), "<a href=\"x\">done</a>");
        assert_eq!(filtered("<!-- a --> <b"), "<!-- a --> <gkx-b>");
    }
}
