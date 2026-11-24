// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Finding task keys (`TM-41`) in text.
//!
//! A key is a project key (a capital letter, then 1 to 9 capital letters or
//! digits), a hyphen and a task number (1 to 9 digits, no leading zero). It
//! must stand on its own: no letter, digit, `_` or `-` directly before or
//! after it, so `xTM-7`, `TM-8x`, `A-TM-10` and `TM-9-b` are not keys.
//! Lower-case `tm-41` is not a key in text either, which keeps words like
//! `utf-8` out.
//!
//! A key may be followed by a comment anchor, `#c-` and the last eight
//! hexadecimal digits of a comment's ID in lower case (`TM-41#c-1a2b3c4d`):
//! a link to that comment of the task. The anchor must stand on its own too.

/// The byte ranges of the task keys in `text`, in order.
pub(crate) fn find(text: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_uppercase()
            && !joined(text[..i].chars().next_back())
            && let Some(end) = key_end(bytes, i)
            && !joined(text[end..].chars().next())
        {
            found.push(i..end);
            i = end;
            continue;
        }
        i += 1;
    }
    found
}

/// The length of the comment anchor (`#c-1a2b3c4d`) at the start of
/// `rest`, the text right after a key, if there is one.
pub(crate) fn comment_anchor(rest: &str) -> Option<usize> {
    let digits = rest.strip_prefix(COMMENT_ANCHOR)?;
    let short = digits.get(..COMMENT_DIGITS)?;
    let len = COMMENT_ANCHOR.len() + COMMENT_DIGITS;
    let hex = short
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (hex && !joined(rest[len..].chars().next())).then_some(len)
}

/// What comes between a task key and a comment's short ID.
pub(crate) const COMMENT_ANCHOR: &str = "#c-";
/// How many hexadecimal digits of a comment's ID a link names.
const COMMENT_DIGITS: usize = 8;

/// Whether `ch` would join a key to the text around it.
fn joined(ch: Option<char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// The end of the key starting at `start`, if one starts there.
fn key_end(bytes: &[u8], start: usize) -> Option<usize> {
    let project = bytes[start..]
        .iter()
        .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        .count();
    if !(2..=10).contains(&project) {
        return None;
    }
    let hyphen = start + project;
    if bytes.get(hyphen) != Some(&b'-') {
        return None;
    }
    let digits_start = hyphen + 1;
    let digits = bytes[digits_start..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if !(1..=9).contains(&digits) || bytes[digits_start] == b'0' {
        return None;
    }
    Some(digits_start + digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Vec<&str> {
        find(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn finds_standalone_keys() {
        assert_eq!(
            keys("TM-41, (TM-1). AB12-3 at end TM-2"),
            ["TM-41", "TM-1", "AB12-3", "TM-2"]
        );
        assert_eq!(keys("«TM-5» TM-6:"), ["TM-5", "TM-6"]);
    }

    #[test]
    fn rejects_joined_or_malformed_keys() {
        for text in [
            "xTM-7",
            "TM-8x",
            "A-TM-10",
            "TM-9-b",
            "tm-11",
            "T-1",
            "TM-0",
            "TM-01",
            "ABCDEFGHIJK-1",
            "TM-1234567890",
            "éTM-1",
            "TM_1",
            "_TM-1",
            "TM-",
        ] {
            assert!(keys(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn comment_anchors_follow_a_key() {
        assert_eq!(comment_anchor("#c-1a2b3c4d"), Some(11));
        assert_eq!(comment_anchor("#c-1a2b3c4d, then"), Some(11));
        for rest in [
            "#c-1a2b3c4",
            "#c-1a2b3c4d5",
            "#c-1A2B3C4D",
            "#c-1a2b3c4g",
            "#c-1a2b3c4d-x",
            "#C-1a2b3c4d",
            "#1a2b3c4d",
            " #c-1a2b3c4d",
            "#c-日本語",
        ] {
            assert_eq!(comment_anchor(rest), None, "{rest}");
        }
    }

    #[test]
    fn longest_project_and_number() {
        assert_eq!(keys("ABCDEFGHIJ-123456789"), ["ABCDEFGHIJ-123456789"]);
    }
}
