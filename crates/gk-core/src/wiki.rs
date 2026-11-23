// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki page names: what a title may hold, and the slug made from it.
//!
//! A wiki link names a page by its title (`[[Release plan#Dates|the
//! dates]]`), so a title can't hold the characters that end a link's parts:
//! `[`, `]`, `#` and `|`. Titles are unique within a project ignoring case,
//! and the slug, the page's name in addresses, is made from the title in
//! lower case.

use crate::text::{TextError, check_line, fold};

/// The longest page title, in characters.
pub const PAGE_TITLE_MAX_CHARS: usize = 200;

/// Why a page title was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PageTitleError {
    /// Empty or too long.
    #[error(transparent)]
    Text(#[from] TextError),
    /// It holds a character that ends a part of a wiki link.
    #[error("a page title can't contain \"{0}\"")]
    Reserved(char),
}

/// The characters a page title can't hold.
pub const RESERVED_IN_TITLES: [char; 4] = ['[', ']', '#', '|'];

/// `title` cleaned (control characters removed, whitespace runs kept as
/// written, ends trimmed) and checked.
///
/// # Errors
/// Empty, longer than [`PAGE_TITLE_MAX_CHARS`], or holding `[`, `]`, `#`
/// or `|`.
pub fn check_page_title(title: &str) -> Result<String, PageTitleError> {
    let cleaned = check_line("title", title, PAGE_TITLE_MAX_CHARS)?;
    if let Some(c) = cleaned.chars().find(|c| RESERVED_IN_TITLES.contains(c)) {
        return Err(PageTitleError::Reserved(c));
    }
    Ok(cleaned)
}

/// The slug of a page title: folded to lower case, letters and digits
/// kept, every other run of characters one `-`. A title with no letters or
/// digits gets `page`.
///
/// ```
/// assert_eq!(gk_core::slug("Release plan: 1.0 / Dates"), "release-plan-1-0-dates");
/// assert_eq!(gk_core::slug("Über Straße"), "über-strasse");
/// assert_eq!(gk_core::slug("!!!"), "page");
/// ```
#[must_use]
pub fn slug(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut gap = false;
    for c in fold(title).chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        "page".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_refuse_link_characters() {
        assert_eq!(
            check_page_title("  Setup\tguide "),
            Ok("Setup guide".to_owned())
        );
        for bad in ["A [draft]", "C# notes", "a|b"] {
            assert!(matches!(
                check_page_title(bad),
                Err(PageTitleError::Reserved(_))
            ));
        }
        assert!(check_page_title(" ").is_err());
        assert!(check_page_title(&"x".repeat(PAGE_TITLE_MAX_CHARS + 1)).is_err());
    }

    #[test]
    fn slugs_ignore_case_and_punctuation() {
        assert_eq!(slug("Setup"), slug("SETUP"));
        assert_eq!(slug("  a -- b  "), "a-b");
        assert_eq!(slug("ЗАДАЧА 1"), "задача-1");
    }
}
