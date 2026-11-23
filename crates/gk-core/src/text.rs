// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Text rules: case-insensitive matching keys and size limits.

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

/// Checks a task title: not blank, at most [`TITLE_MAX_CHARS`] characters.
/// Returns the title with surrounding white space removed.
pub fn check_title(title: &str) -> Result<&str, TextError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(TextError::Empty("title"));
    }
    let len = trimmed.chars().count();
    if len > TITLE_MAX_CHARS {
        return Err(TextError::TooLong {
            what: "title",
            len,
            max: TITLE_MAX_CHARS,
            unit: "characters",
        });
    }
    Ok(trimmed)
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
        assert_eq!(check_title("  Fix it  "), Ok("Fix it"));
        assert_eq!(check_title(" \t\n"), Err(TextError::Empty("title")));
        let max = "ж".repeat(TITLE_MAX_CHARS);
        assert!(check_title(&max).is_ok());
        let over = format!("{max}x");
        let err = check_title(&over).unwrap_err();
        assert!(err.to_string().contains("501 characters"), "{err}");
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
