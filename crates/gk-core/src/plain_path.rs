// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Paths as people write them.
//!
//! On Windows a canonical path starts with `\\?\` (`\\?\C:\Users\…`,
//! `\\?\UNC\server\share\…`). Programs and people don't write that form,
//! and a path with it doesn't compare equal to the same path without it.
//! [`plain_path`] takes the prefix off wherever the shorter form names the
//! same file; on other systems paths are left as they are.

use std::path::PathBuf;

/// The longest path Windows takes without the `\\?\` prefix.
const PLAIN_MAX_CHARS: usize = 259;

/// `path` without Windows' `\\?\` prefix when the plain form names the
/// same file; otherwise, and on other systems, `path` unchanged.
#[must_use]
pub fn plain_path(path: PathBuf) -> PathBuf {
    if cfg!(windows)
        && let Some(plain) = path.to_str().and_then(without_verbatim_prefix)
    {
        return PathBuf::from(plain);
    }
    path
}

/// `text`, a Windows path, without its `\\?\` prefix: a drive path
/// (`\\?\C:\…` becomes `C:\…`) or a share (`\\?\UNC\server\share\…`
/// becomes `\\server\share\…`). `None` when it has no such prefix or when
/// the plain form would name something else or be too long: a part that
/// is `.` or `..`, ends in a dot or a space, holds a `/` or a `:`, or is a
/// device name.
///
/// ```
/// use gk_core::without_verbatim_prefix as plain;
/// assert_eq!(plain(r"\\?\C:\Users\me").as_deref(), Some(r"C:\Users\me"));
/// assert_eq!(plain(r"\\?\UNC\nas\team\a").as_deref(), Some(r"\\nas\team\a"));
/// assert_eq!(plain(r"\\?\C:\Users\me\con"), None);
/// assert_eq!(plain(r"C:\Users\me"), None);
/// ```
#[must_use]
pub fn without_verbatim_prefix(text: &str) -> Option<String> {
    let rest = text.strip_prefix(r"\\?\")?;
    let (plain, parts) = match rest.get(..4) {
        Some(unc) if unc.eq_ignore_ascii_case(r"UNC\") => {
            let tail = &rest[4..];
            let mut it = tail.splitn(3, '\\');
            let (server, share) = (it.next()?, it.next()?);
            if server.is_empty() || share.is_empty() {
                return None;
            }
            (format!(r"\\{tail}"), it.next().unwrap_or_default())
        }
        _ => {
            let bytes = rest.as_bytes();
            let drive = bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && bytes[2] == b'\\';
            if !drive {
                return None;
            }
            (rest.to_owned(), &rest[3..])
        }
    };
    let usable = |part: &str| {
        part != "."
            && part != ".."
            && !part.ends_with(['.', ' '])
            && !part.contains(['/', ':'])
            && !crate::is_windows_device(part)
    };
    let fits = plain.chars().count() <= PLAIN_MAX_CHARS;
    (fits && parts.split('\\').filter(|p| !p.is_empty()).all(usable)).then_some(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prefix_goes_where_the_plain_form_names_the_same_file() {
        for (given, plain) in [
            (r"\\?\C:\Users\me\exports", r"C:\Users\me\exports"),
            (r"\\?\c:\", r"c:\"),
            (r"\\?\UNC\nas\team\docs", r"\\nas\team\docs"),
            (r"\\?\unc\nas\team", r"\\nas\team"),
        ] {
            assert_eq!(without_verbatim_prefix(given).as_deref(), Some(plain));
        }
        for kept in [
            r"C:\Users\me",
            r"\\nas\team",
            r"\\?\Volume{0b1c}\x",
            r"\\?\C:\a\..\b",
            r"\\?\C:\a\trailing.",
            r"\\?\C:\a\space ",
            r"\\?\C:\a\NUL.txt",
            r"\\?\C:\a\b:stream",
            r"\\?\C:\a/b",
            r"\\?\UNC\nas",
            r"\\?\C:",
        ] {
            assert_eq!(without_verbatim_prefix(kept), None, "{kept}");
        }
        let long = format!(r"\\?\C:\{}", "a".repeat(300));
        assert_eq!(without_verbatim_prefix(&long), None);
    }

    #[test]
    fn other_systems_keep_their_paths() {
        if !cfg!(windows) {
            let path = PathBuf::from(r"\\?\C:\x");
            assert_eq!(plain_path(path.clone()), path);
        }
        assert_eq!(
            plain_path(PathBuf::from("/home/me")),
            PathBuf::from("/home/me")
        );
    }
}
