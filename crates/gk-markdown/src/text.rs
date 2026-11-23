// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Small text helpers shared by the jobs.

use comrak::nodes::{AstNode, NodeValue};

/// Appends the visible text of `node`'s inline content: text, code spans and
/// math as written, a space for each line break. Raw HTML is left out.
pub(crate) fn push_inline_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for descendant in node.descendants() {
        match &descendant.data().value {
            NodeValue::Text(text) => out.push_str(text),
            NodeValue::Code(code) => out.push_str(&code.literal),
            NodeValue::Math(math) => out.push_str(&math.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
    }
}

/// `text` with surrounding whitespace removed and every run of whitespace
/// inside it replaced by one space.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Escapes text for an HTML attribute value in double quotes.
pub(crate) fn push_attribute(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// Decodes the character references that can appear in raw HTML text: the
/// five markup entities, `&nbsp;`, and decimal and hexadecimal references.
/// Anything else is kept as written.
pub(crate) fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => name.strip_prefix('#').and_then(|number| {
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => number.parse().ok(),
                    };
                    code.and_then(char::from_u32).filter(|&c| c != '\0')
                }),
            };
            ch.map(|ch| (ch, end))
        });
        match decoded {
            Some((ch, end)) => {
                out.push(ch);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace() {
        assert_eq!(collapse_whitespace("  a \t b\n c  "), "a b c");
        assert_eq!(collapse_whitespace(""), "");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("a &amp; b &lt;c&gt;"), "a & b <c>");
        assert_eq!(decode_entities("&#65;&#x42;&#X43;"), "ABC");
        assert_eq!(
            decode_entities("&unknown; & &#0; &#xD800;"),
            "&unknown; & &#0; &#xD800;"
        );
        assert_eq!(decode_entities("tail &"), "tail &");
    }
}
