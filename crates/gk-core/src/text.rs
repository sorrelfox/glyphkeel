// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Text rules: case-insensitive matching keys, cleaning single-line text and
//! size limits.

use icu_casemap::CaseMapper;
use icu_normalizer::ComposingNormalizerBorrowed;

/// Longest task title, in characters (Unicode scalar values).
pub const TITLE_MAX_CHARS: usize = 500;
/// Largest Markdown text (description, wiki page), in bytes of UTF-8.
pub const MARKDOWN_MAX_BYTES: usize = 1024 * 1024;
/// Largest comment, in bytes of UTF-8.
pub const COMMENT_MAX_BYTES: usize = 256 * 1024;

/// Why a text was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// Empty, or only white space.
    #[error("the {0} is empty")]
    Empty(&'static str),
    /// Over the limit.
    #[error("the {what} is {len} {unit} long; the limit is {max}")]
    TooLong {
        /// What was checked ("title", "description", "comment").
        what: &'static str,
        /// Its length.
        len: usize,
        /// The limit.
        max: usize,
        /// "characters" or "bytes".
        unit: &'static str,
    },
}

/// The matching key of a name or title: Unicode compatibility normalisation
/// (NFKC) and full case folding, so `ЗАДАЧА`, `задача` and `Задача` give the
/// same key, as do `ﬁle` and `FILE`. Stored in the `*_fold` columns next to
/// the text and compared with plain equality, `LIKE` or `instr`.
///
/// This is the usual approximation of Unicode's `NFKC_Casefold`:
/// normalise, fold, normalise again. It does not remove default-ignorable
/// characters.
#[must_use]
pub fn fold(text: &str) -> String {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let folded = CaseMapper::new()
        .fold_string(&nfkc.normalize(text))
        .into_owned();
    nfkc.normalize(&folded).into_owned()
}

/// Cleans text that must stay on one line (titles and names): tabs and line
/// breaks become spaces (`\r\n` one space), every other control character
/// is removed, and surrounding white space is trimmed.
#[must_use]
pub fn clean_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(' ');
            }
            '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

/// Checks a task title: cleaned as [`clean_line`], then not blank and at
/// most [`TITLE_MAX_CHARS`] characters. Returns the cleaned title.
pub fn check_title(title: &str) -> Result<String, TextError> {
    check_line("title", title, TITLE_MAX_CHARS)
}

/// Checks a one-line name (a project, status, label or type name): cleaned
/// as [`clean_line`], then not blank and at most `max` characters. Returns
/// the cleaned name.
pub fn check_line(what: &'static str, text: &str, max: usize) -> Result<String, TextError> {
    let cleaned = clean_line(text);
    if cleaned.is_empty() {
        return Err(TextError::Empty(what));
    }
    let len = cleaned.chars().count();
    if len > max {
        return Err(TextError::TooLong {
            what,
            len,
            max,
            unit: "characters",
        });
    }
    Ok(cleaned)
}

/// Checks a Markdown field (description, wiki page) against
/// [`MARKDOWN_MAX_BYTES`].
pub fn check_markdown(what: &'static str, text: &str) -> Result<(), TextError> {
    check_bytes(what, text, MARKDOWN_MAX_BYTES)
}

/// Checks a comment against [`COMMENT_MAX_BYTES`].
pub fn check_comment(text: &str) -> Result<(), TextError> {
    if text.trim().is_empty() {
        return Err(TextError::Empty("comment"));
    }
    check_bytes("comment", text, COMMENT_MAX_BYTES)
}

fn check_bytes(what: &'static str, text: &str, max: usize) -> Result<(), TextError> {
    if text.len() > max {
        return Err(TextError::TooLong {
            what,
            len: text.len(),
            max,
            unit: "bytes",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_matches_across_case_and_compatibility_forms() {
        assert_eq!(fold("ЗАДАЧА"), fold("Задача"));
        assert!(fold("Задача номер 1").contains(&fold("ЗАДАЧА")));
        assert_eq!(fold("Straße"), fold("STRASSE"));
        assert_eq!(fold("ﬁle"), "file");
        assert_eq!(fold("ΣΊΣΥΦΟΣ"), fold("σίσυφος"));
        assert_eq!(fold("Ｆｕｌｌ"), "full");
        assert_eq!(fold("e\u{301}"), fold("É"));
        assert_eq!(fold(""), "");
    }

    #[test]
    fn titles() {
        assert_eq!(check_title("  Fix it  "), Ok("Fix it".to_owned()));
        assert_eq!(check_title(" \t\n"), Err(TextError::Empty("title")));
        let max = "ж".repeat(TITLE_MAX_CHARS);
        assert!(check_title(&max).is_ok());
        let over = format!("{max}x");
        let err = check_title(&over).unwrap_err();
        assert!(err.to_string().contains("501 characters"), "{err}");
    }

    #[test]
    fn titles_lose_control_characters() {
        assert_eq!(clean_line("Fix\tthe\nbug"), "Fix the bug");
        assert_eq!(clean_line("one\r\ntwo\rthree"), "one two three");
        assert_eq!(clean_line("bell\u{7}less\u{1b}[0m"), "bellless[0m");
        assert_eq!(clean_line("zero\u{0}width"), "zerowidth");
        assert_eq!(clean_line("\n  padded \t"), "padded");
        assert_eq!(clean_line("Задача\u{2028}1"), "Задача 1");
        assert_eq!(check_title("\u{7}\u{8}"), Err(TextError::Empty("title")));
        assert_eq!(check_title("a\u{7f}b"), Ok("ab".to_owned()));
    }

    #[test]
    fn markdown_and_comment_limits() {
        assert!(check_markdown("description", &"a".repeat(MARKDOWN_MAX_BYTES)).is_ok());
        assert!(check_markdown("description", &"a".repeat(MARKDOWN_MAX_BYTES + 1)).is_err());
        assert!(check_comment(&"a".repeat(COMMENT_MAX_BYTES)).is_ok());
        assert!(check_comment(&"a".repeat(COMMENT_MAX_BYTES + 1)).is_err());
        assert!(check_comment(" ").is_err());
    }
}
