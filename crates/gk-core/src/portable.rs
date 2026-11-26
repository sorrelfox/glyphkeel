// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Names for the files and folders the app writes from what people typed:
//! exported pages and their folders, copied attachments, archive entries,
//! saved exports and opened copies.
//!
//! A name is made valid on Linux, macOS and Windows alike, the same on
//! each, so an export made on one opens on the others:
//!
//! - no `/`, `\`, `:`, `<`, `>`, `"`, `|`, `?`, `*` and no control
//!   characters (each becomes `_`);
//! - no trailing dots or spaces, which Windows drops;
//! - none of the device names Windows reserves in every folder (`CON`,
//!   `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, `CONIN$`,
//!   `CONOUT$`), in any case and with any extension: they get a `_` in
//!   front;
//! - at most [`PORTABLE_NAME_MAX_BYTES`] bytes of UTF-8, the extension
//!   kept.

/// The longest name, in bytes of UTF-8. Well below the 255 that file
/// systems allow, so a folder of nested pages stays usable on Windows.
pub const PORTABLE_NAME_MAX_BYTES: usize = 120;

/// The longest extension (with its dot) kept when a name is cut.
const EXTENSION_MAX_BYTES: usize = 16;

/// Device names Windows reserves in every folder, with any extension. The
/// superscript digits count as digits there.
const DEVICES: &[&str] = &[
    "con", "prn", "aux", "nul", "conin$", "conout$", "com0", "com1", "com2", "com3", "com4",
    "com5", "com6", "com7", "com8", "com9", "com¹", "com²", "com³", "lpt0", "lpt1", "lpt2", "lpt3",
    "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "lpt¹", "lpt²", "lpt³",
];

/// Whether Windows takes the file `name` for a device: its part before the
/// first dot, without trailing spaces, is a device name.
///
/// ```
/// assert!(gk_core::is_windows_device("con"));
/// assert!(gk_core::is_windows_device("Lpt1.tar.gz"));
/// assert!(!gk_core::is_windows_device("console.txt"));
/// ```
#[must_use]
pub fn is_windows_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    DEVICES.contains(&stem.to_lowercase().as_str())
}

/// `name` as a file or folder name that is valid everywhere (see the
/// module's rules); `file` when nothing is left.
///
/// ```
/// assert_eq!(gk_core::portable_file_name("a:b?.md"), "a_b_.md");
/// assert_eq!(gk_core::portable_file_name("CON"), "_CON");
/// assert_eq!(gk_core::portable_file_name("nul.txt"), "_nul.txt");
/// assert_eq!(gk_core::portable_file_name("notes. . "), "notes");
/// assert_eq!(gk_core::portable_file_name(".."), "file");
/// ```
#[must_use]
pub fn portable_file_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut out = cut(&replaced, PORTABLE_NAME_MAX_BYTES);
    out.truncate(out.trim_end_matches(['.', ' ']).len());
    if out.is_empty() {
        return "file".to_owned();
    }
    if is_windows_device(&out) {
        out.insert(0, '_');
        if out.len() > PORTABLE_NAME_MAX_BYTES {
            // Only a long tail after the device name gets here: one
            // character before the first dot makes room for the `_`.
            let dot = out.find('.').unwrap_or(out.len());
            let before = out[..dot].chars().next_back().map_or(0, char::len_utf8);
            out.replace_range(dot - before..dot, "");
        }
    }
    out
}

/// The portable `name` with `-n` before its extension (`a.png`, 2 →
/// `a-2.png`), for a name that is taken; just the portable name for
/// `n <= 1`.
///
/// ```
/// assert_eq!(gk_core::numbered_file_name("plan.md", 2), "plan-2.md");
/// assert_eq!(gk_core::numbered_file_name("CON", 3), "_CON-3");
/// ```
#[must_use]
pub fn numbered_file_name(name: &str, n: u32) -> String {
    let name = portable_file_name(name);
    if n <= 1 {
        return name;
    }
    let (stem, ext) = split_extension(&name);
    let suffix = format!("-{n}");
    let room = PORTABLE_NAME_MAX_BYTES - ext.len() - suffix.len();
    format!("{}{suffix}{ext}", cut_at_char(stem, room))
}

/// The first of `name`, `name-2`, `name-3`… (made portable) that `claim`
/// accepts. `claim` should take the name and say whether it was free;
/// compare names ignoring case, as Windows and macOS do.
///
/// ```
/// use std::collections::HashSet;
/// let mut taken = HashSet::new();
/// let mut claim = |n: &str| taken.insert(n.to_lowercase());
/// assert_eq!(gk_core::unique_file_name("a:b", &mut claim), "a_b");
/// assert_eq!(gk_core::unique_file_name("A?B", &mut claim), "A_B-2");
/// ```
pub fn unique_file_name(name: &str, mut claim: impl FnMut(&str) -> bool) -> String {
    let mut n = 1;
    loop {
        let candidate = numbered_file_name(name, n);
        if claim(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// `name` split before its extension, which keeps its dot: the part from
/// the last dot, unless the name starts there or the part is too long to
/// be one.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= EXTENSION_MAX_BYTES => name.split_at(i),
        _ => (name, ""),
    }
}

/// `name` in at most `max` bytes, its extension kept when it has one.
fn cut(name: &str, max: usize) -> String {
    if name.len() <= max {
        return name.to_owned();
    }
    let (stem, ext) = split_extension(name);
    format!("{}{ext}", cut_at_char(stem, max - ext.len()))
}

/// The start of `text` in at most `max` bytes, at a character boundary.
fn cut_at_char(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::*;

    /// Whether `name` is valid on every supported system.
    fn valid(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= PORTABLE_NAME_MAX_BYTES
            && !name.chars().any(|c| {
                c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            })
            && !name.ends_with(['.', ' '])
            && !is_windows_device(name)
            && name != "."
            && name != ".."
    }

    #[test]
    fn device_names_get_a_mark_in_any_case_and_with_any_extension() {
        for name in [
            "CON", "con", "Prn", "aux", "NUL", "com1", "COM9", "lpt1", "LPT9", "com0", "conin$",
            "CONOUT$", "COM¹", "lpt³",
        ] {
            assert_eq!(portable_file_name(name), format!("_{name}"));
            assert_eq!(
                portable_file_name(&format!("{name}.md")),
                format!("_{name}.md")
            );
        }
        assert_eq!(portable_file_name("con.tar.gz"), "_con.tar.gz");
        assert_eq!(portable_file_name("CON .txt"), "_CON .txt");
        for fine in [
            "console",
            "com10",
            "lpt",
            "connect.md",
            "nul-1",
            "_con",
            "acon",
        ] {
            assert_eq!(portable_file_name(fine), fine);
        }
    }

    #[test]
    fn forbidden_characters_and_trailing_dots_go() {
        assert_eq!(portable_file_name("a:b"), "a_b");
        assert_eq!(portable_file_name("what?<x>|\"*\".md"), "what__x_____.md");
        assert_eq!(portable_file_name("a.txt:evil.exe"), "a.txt_evil.exe");
        assert_eq!(portable_file_name("../a/b\\c"), ".._a_b_c");
        assert_eq!(portable_file_name("tab\there\u{7f}"), "tab_here_");
        assert_eq!(portable_file_name("run.exe. . "), "run.exe");
        assert_eq!(portable_file_name("nul. "), "_nul");
        assert_eq!(portable_file_name(""), "file");
        assert_eq!(portable_file_name(". ."), "file");
        // Letters of any script stay.
        assert_eq!(portable_file_name("Über Straße.md"), "Über Straße.md");
    }

    #[test]
    fn long_names_are_cut_keeping_the_extension() {
        let long = format!("{}.png", "é".repeat(200));
        let cut = portable_file_name(&long);
        assert!(cut.len() <= PORTABLE_NAME_MAX_BYTES);
        assert!(cut.ends_with(".png"));
        assert!(cut.trim_end_matches(".png").chars().all(|c| c == 'é'));
        // A device name with a long tail still fits after its mark.
        let tail = format!("con{}.txt", " ".repeat(200));
        let named = portable_file_name(&tail);
        assert!(valid(&named), "{named:?}");
        assert!(named.starts_with("_con"));
    }

    #[test]
    fn numbers_go_before_the_extension() {
        assert_eq!(numbered_file_name("a.png", 1), "a.png");
        assert_eq!(numbered_file_name("a.png", 12), "a-12.png");
        assert_eq!(numbered_file_name(".hidden", 2), ".hidden-2");
        assert_eq!(numbered_file_name("nul.md", 2), "_nul-2.md");
        let long = numbered_file_name(&"x".repeat(300), 99);
        assert_eq!(long.len(), PORTABLE_NAME_MAX_BYTES);
        assert!(long.ends_with("-99"));
    }

    proptest! {
        #[test]
        fn every_name_becomes_valid_and_stays_so(name in "\\PC{0,200}|[cCoOnNpPrRuUlLtTaAxX1-9$. :]{0,12}") {
            let once = portable_file_name(&name);
            prop_assert!(valid(&once), "{:?} -> {:?}", name, once);
            prop_assert_eq!(portable_file_name(&once), once);
        }

        #[test]
        fn names_stay_unique_after_cleaning(names in prop::collection::vec(
            "[aA:?*. ]{0,4}|(CON|con|Nul|lpt1)(\\.md)?|\\PC{0,130}", 1..40,
        )) {
            let mut taken = HashSet::new();
            let mut out = Vec::new();
            for name in &names {
                let got = unique_file_name(name, |n| taken.insert(n.to_lowercase()));
                prop_assert!(valid(&got), "{:?} -> {:?}", name, got);
                out.push(got.to_lowercase());
            }
            let distinct: HashSet<&String> = out.iter().collect();
            prop_assert_eq!(distinct.len(), names.len());
        }
    }
}
