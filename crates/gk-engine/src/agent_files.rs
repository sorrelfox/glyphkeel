// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Files that agents name by path.
//!
//! A few operation inputs name a file or a folder on disk: the file to
//! attach, the archive, CSV file or folder of Markdown files to import, and
//! where an export goes. Each one is listed in [`PATH_INPUTS`], and
//! [`Engine::call`](crate::Engine::call) passes every agent's input through
//! [`confine`] before the operation runs, whichever tool or program it came
//! through. The person's own paths, in the app and on the command line, are
//! taken as given.
//!
//! An agent may only use files inside the folders agents may use: the
//! workspace's `exports` folder, and the folders the person added in their
//! personal settings (`agent_folders`, see [`gk_store::UserSettings`]).
//!
//! - A relative path, normally a plain file name, is a file in `exports`.
//!   Each of its parts must be a plain name: no `..` or `.`, no hidden
//!   names (a leading `.`), no characters or names that Windows reserves.
//! - An absolute path must lie inside one of the folders.
//! - Links are followed, and the place they lead to must still be inside
//!   one of the folders; a link as the target of an export is refused.
//! - Inside an allowed folder, hidden files and folders stay off limits.
//!
//! The input is rewritten to the checked, canonical path, so the operation
//! reads or writes exactly the file that was checked.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_store::{UserSettings, WorkspaceLayout};
use serde_json::Value;

use crate::context::Context;
use crate::messages;

/// Where the folders agents may use, besides the workspace's `exports`
/// folder, come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentFolders {
    /// The person's own setting, `agent_folders` in their personal
    /// settings file, read again at every check: the person can change it
    /// while an agent runs. The programs choose it; the engine's default is
    /// none, so that nothing reads the person's file unasked.
    Personal,
    /// `agent_folders` in this personal settings file, read again at every
    /// check (for tests and programs that keep the file elsewhere).
    File(PathBuf),
    /// These folders, whatever the personal settings say (for tests, and
    /// for programs that keep the choice themselves).
    Fixed(Vec<PathBuf>),
}

impl AgentFolders {
    /// The folders the person added, as given: absolute paths only.
    fn added(&self) -> Vec<PathBuf> {
        let folders = match self {
            Self::Personal => folders_in(&UserSettings::load_default()),
            Self::File(file) => folders_in(&UserSettings::load(file)),
            Self::Fixed(folders) => folders.clone(),
        };
        folders.into_iter().filter(|f| f.is_absolute()).collect()
    }
}

fn folders_in(settings: &UserSettings) -> Vec<PathBuf> {
    settings.agent_folders.iter().map(PathBuf::from).collect()
}

/// Whether an input names something to read or a place to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    /// An existing file or folder to read.
    Read,
    /// A new file or folder to write; it may not exist yet.
    Write,
}

/// Every operation input that names a place on disk and that an agent can
/// reach: the operation, the field and what is done with it. Operations
/// only the person runs (such as moving the backup folder) are not here; a
/// test checks that the list misses none.
pub(crate) const PATH_INPUTS: &[(&str, &str, Access)] = &[
    ("file.attach", "path", Access::Read),
    ("transfer.import", "path", Access::Read),
    ("transfer.import_csv", "path", Access::Read),
    ("wiki.import", "path", Access::Read),
    ("transfer.export", "path", Access::Write),
    ("wiki.export", "path", Access::Write),
];

/// The folders agents may use: the workspace's `exports` folder first,
/// then the ones the person added, as configured.
#[must_use]
pub(crate) fn folders(layout: &WorkspaceLayout, source: &AgentFolders) -> Vec<PathBuf> {
    let mut all = vec![layout.exports_dir()];
    for folder in source.added() {
        if !all.contains(&folder) {
            all.push(folder);
        }
    }
    all
}

/// The folders agents may use, as an agent is told them: canonical where
/// they exist, the ones that don't left out, except `exports`, which is
/// made when it is first needed.
#[must_use]
pub(crate) fn listed(layout: &WorkspaceLayout, source: &AgentFolders) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, folder) in folders(layout, source).into_iter().enumerate() {
        let shown = match gk_store::canonical_path(&folder) {
            Ok(canonical) => canonical,
            Err(_) if i == 0 => folder,
            Err(_) => continue,
        };
        let shown = display(&shown);
        if !out.contains(&shown) {
            out.push(shown);
        }
    }
    out
}

/// Checks every path an agent gives in `input` to operation `op`, and puts
/// the checked, canonical path in its place. The person's input comes back
/// unchanged.
///
/// # Errors
/// `invalid` on the field for a path that isn't a usable name, goes up with
/// `..`, names a hidden file or doesn't exist; `forbidden` for one outside
/// the folders agents may use, also through a link. The hint names the
/// folders.
pub(crate) fn confine(
    ctx: &Context,
    layout: &WorkspaceLayout,
    source: &AgentFolders,
    op: &str,
    mut input: Value,
) -> Result<Value, OpError> {
    if !ctx.is_agent() {
        return Ok(input);
    }
    for (_, field, access) in PATH_INPUTS.iter().filter(|(o, _, _)| *o == op) {
        let Some(given) = input.get(*field).and_then(Value::as_str) else {
            continue;
        };
        let checked = check(layout, source, field, given, *access)?;
        if let Some(slot) = input.get_mut(*field) {
            *slot = Value::String(checked.to_string_lossy().into_owned());
        }
    }
    Ok(input)
}

/// The checked, canonical place of `given`, an agent's path in `field`.
///
/// # Errors
/// As [`confine`].
pub(crate) fn check(
    layout: &WorkspaceLayout,
    source: &AgentFolders,
    field: &str,
    given: &str,
    access: Access,
) -> Result<PathBuf, OpError> {
    let allowed = folders(layout, source);
    let off_limits = [
        layout.root().to_path_buf(),
        crate::backup::default_folder(layout),
    ];
    resolve(&allowed, given, access)
        .and_then(|path| keep_off(path, &allowed[0], &off_limits))
        .map_err(|refusal| {
            error(
                &refusal,
                field,
                given,
                &listed(layout, source),
                &display(&allowed[0]),
            )
        })
}

/// Refuses `path` inside one of `off_limits` (the workspace's own folder
/// and its backups, which hold all of its data), unless it is inside
/// `exports`, even when the person added a folder around them.
fn keep_off(path: PathBuf, exports: &Path, off_limits: &[PathBuf]) -> Result<PathBuf, Refusal> {
    let canonical = |p: &Path| {
        gk_store::canonical_path(p)
            .ok()
            .and_then(|c| split_host(&c).ok())
    };
    let Ok(at) = split_host(&path) else {
        return Err(Refusal::Name);
    };
    if canonical(exports).is_some_and(|e| below(&at, &e, Style::HOST).is_some()) {
        return Ok(path);
    }
    let inside = off_limits
        .iter()
        .filter_map(|p| canonical(p))
        .any(|f| below(&at, &f, Style::HOST).is_some());
    if inside {
        Err(Refusal::Outside)
    } else {
        Ok(path)
    }
}

/// Why a path was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Not a usable name or path.
    Name,
    /// It goes up a folder with `..` (or names `.`).
    Parent,
    /// It names a hidden file or folder.
    Hidden,
    /// It is outside the folders.
    Outside,
    /// It is inside a folder as written, but a link leads outside.
    Link,
    /// It doesn't exist, or can't be read: the error.
    Missing(String),
}

fn error(
    refusal: &Refusal,
    field: &str,
    given: &str,
    folders: &[String],
    exports: &str,
) -> OpError {
    let mut args = vec![("field", field.to_owned()), ("path", shorten(given))];
    let id = match refusal {
        Refusal::Name => "agent-path-name",
        Refusal::Parent => "agent-path-parent",
        Refusal::Hidden => "agent-path-hidden",
        Refusal::Outside => "agent-path-outside",
        Refusal::Link => "agent-path-link",
        Refusal::Missing(e) => {
            args.push(("error", e.clone()));
            "agent-path-missing"
        }
    };
    let made = match refusal {
        Refusal::Outside | Refusal::Link => {
            messages::forbidden(id, &args).with_detail("path", field.to_owned())
        }
        _ => messages::invalid_at(field, id, &args),
    };
    messages::hint(
        made,
        "agent-path-hint",
        &[
            ("folders", folders.join(", ")),
            ("exports", exports.to_owned()),
        ],
    )
    .with_detail("allowed", folders.to_vec())
}

/// A long path cut for a message.
fn shorten(given: &str) -> String {
    const MAX: usize = 300;
    if given.chars().count() <= MAX {
        return given.to_owned();
    }
    let mut cut: String = given.chars().take(MAX).collect();
    cut.push('…');
    cut
}

fn display(path: &Path) -> String {
    let text = path.to_string_lossy();
    // Windows' canonical paths start with `\\?\`, which people don't write.
    match text.strip_prefix(r"\\?\") {
        Some(rest) if cfg!(windows) => match rest.strip_prefix(r"UNC\") {
            Some(unc) => format!(r"\\{unc}"),
            None => rest.to_owned(),
        },
        _ => text.into_owned(),
    }
}

// ---------- paths as text, on either system ----------

/// How a system writes paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    /// `/` separates, `/` starts an absolute path.
    Unix,
    /// `\` and `/` separate; a drive (`C:\`) or a share (`\\server\share`)
    /// starts an absolute path; names compare without case.
    Windows,
}

impl Style {
    /// The style of the system this program runs on.
    pub(crate) const HOST: Self = if cfg!(windows) {
        Self::Windows
    } else {
        Self::Unix
    };
}

/// A path as its root (`None` for a relative path) and its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Split {
    root: Option<String>,
    parts: Vec<String>,
}

/// Splits `path`; `None` for a form agents may not use: a Windows device
/// path (`\\.\…`), a path relative to a drive (`C:file`) or to the current
/// drive's root (`\file`).
pub(crate) fn split(path: &str, style: Style) -> Option<Split> {
    let parts = |rest: &str, seps: &[char]| -> Vec<String> {
        rest.split(seps)
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    };
    match style {
        Style::Unix => Some(match path.strip_prefix('/') {
            Some(rest) => Split {
                root: Some("/".to_owned()),
                parts: parts(rest, &['/']),
            },
            None => Split {
                root: None,
                parts: parts(path, &['/']),
            },
        }),
        Style::Windows => {
            let seps = ['\\', '/'];
            let p = path.replace('/', "\\");
            if p.starts_with(r"\\.\") || p.starts_with(r"\??\") {
                return None;
            }
            // `\\?\UNC\server\share\…`, `\\?\C:\…`: the long forms.
            let (p, verbatim) = match p.strip_prefix(r"\\?\") {
                Some(rest) => match rest.get(..4) {
                    Some(unc) if unc.eq_ignore_ascii_case(r"UNC\") => {
                        (format!(r"\\{}", &rest[4..]), true)
                    }
                    _ => (rest.to_owned(), true),
                },
                None => (p, false),
            };
            if let Some(rest) = p.strip_prefix(r"\\") {
                let mut it = rest.splitn(3, '\\');
                let server = it.next().unwrap_or_default();
                let share = it.next().unwrap_or_default();
                if server.is_empty() || share.is_empty() {
                    return None;
                }
                return Some(Split {
                    root: Some(format!(r"\\{server}\{share}")),
                    parts: parts(it.next().unwrap_or_default(), &seps),
                });
            }
            let bytes = p.as_bytes();
            if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                let rest = &p[2..];
                if !rest.starts_with('\\') && !(verbatim && rest.is_empty()) {
                    return None;
                }
                return Some(Split {
                    root: Some(p[..2].to_ascii_uppercase()),
                    parts: parts(rest, &seps),
                });
            }
            if p.starts_with('\\') {
                return None;
            }
            Some(Split {
                root: None,
                parts: parts(&p, &seps),
            })
        }
    }
}

/// The parts of `path` below `folder`, when it is inside it (or is it).
fn below<'a>(path: &'a Split, folder: &Split, style: Style) -> Option<&'a [String]> {
    let same = |a: &str, b: &str| match style {
        Style::Unix => a == b,
        Style::Windows => a.to_lowercase() == b.to_lowercase(),
    };
    let (Some(root), Some(folder_root)) = (&path.root, &folder.root) else {
        return None;
    };
    if !same(root, folder_root) || path.parts.len() < folder.parts.len() {
        return None;
    }
    let inside = path
        .parts
        .iter()
        .zip(&folder.parts)
        .all(|(a, b)| same(a, b));
    inside.then(|| &path.parts[folder.parts.len()..])
}

/// Names Windows keeps for devices, in any case and with any extension.
const DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM0", "COM1", "COM2", "COM3", "COM4",
    "COM5", "COM6", "COM7", "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9",
];

/// Why `part`, one part of an agent's path, can't be used; `None` when it
/// is a plain name, usable on every system.
pub(crate) fn check_part(part: &str) -> Option<Refusal> {
    if part == "." || part == ".." {
        return Some(Refusal::Parent);
    }
    if part.starts_with('.') {
        return Some(Refusal::Hidden);
    }
    let stem = part.split('.').next().unwrap_or_default().trim_end();
    let bad = part.is_empty()
        || part.len() > 255
        || part.ends_with(['.', ' '])
        || part.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        || DEVICE_NAMES.iter().any(|d| d.eq_ignore_ascii_case(stem));
    bad.then_some(Refusal::Name)
}

/// The first refusal of the parts of `parts`.
fn check_parts(parts: &[String]) -> Result<(), Refusal> {
    match parts.iter().find_map(|p| check_part(p)) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// The checks on an agent's path as text: what it is relative to, and its
/// parts. A relative path must be all plain names; an absolute one may not
/// go up a folder or, on Windows, name a stream (`file:stream`).
pub(crate) fn check_text(given: &str, style: Style) -> Result<Split, Refusal> {
    if given.trim().is_empty() || given.len() > 4096 || given.chars().any(char::is_control) {
        return Err(Refusal::Name);
    }
    let split = split(given, style).ok_or(Refusal::Name)?;
    match split.root {
        None => {
            if split.parts.is_empty() {
                return Err(Refusal::Name);
            }
            check_parts(&split.parts)?;
        }
        Some(_) => {
            if split.parts.iter().any(|p| p == "." || p == "..") {
                return Err(Refusal::Parent);
            }
            if style == Style::Windows && split.parts.iter().any(|p| p.contains(':')) {
                return Err(Refusal::Name);
            }
        }
    }
    Ok(split)
}

/// Whether `path` (canonical, absolute) is inside one of `folders`
/// (canonical); the parts below the folder when it is.
fn inside_any<'a>(path: &'a Split, folders: &[Split], style: Style) -> Option<&'a [String]> {
    folders.iter().find_map(|f| below(path, f, style))
}

fn split_host(path: &Path) -> Result<Split, Refusal> {
    let text = path.to_str().ok_or(Refusal::Name)?;
    split(text, Style::HOST).ok_or(Refusal::Name)
}

/// Where `given` is, checked against `folders` (the first is `exports`,
/// where relative paths go), following links.
pub(crate) fn resolve(
    folders: &[PathBuf],
    given: &str,
    access: Access,
) -> Result<PathBuf, Refusal> {
    let style = Style::HOST;
    let text = check_text(given, style)?;
    let Some(exports) = folders.first() else {
        return Err(Refusal::Outside);
    };
    // `exports` belongs to the workspace and is made when first needed.
    let _ = fs::create_dir_all(exports);
    let canonical: Vec<(PathBuf, Split)> = folders
        .iter()
        .filter_map(|f| gk_store::canonical_path(f).ok())
        .filter_map(|c| split_host(&c).ok().map(|s| (c, s)))
        .collect();
    let canonical_splits: Vec<Split> = canonical.iter().map(|(_, s)| s.clone()).collect();
    let written: Vec<Split> = folders.iter().filter_map(|f| split_host(f).ok()).collect();

    let candidate = match text.root {
        None => {
            let base = canonical
                .first()
                .map_or_else(|| exports.clone(), |(c, _)| c.clone());
            text.parts.iter().fold(base, |p, part| p.join(part))
        }
        Some(_) => {
            // Outside every folder as written: refused before anything on
            // disk is looked at, so an agent can't probe for files.
            let as_written = inside_any(&text, &written, style)
                .or_else(|| inside_any(&text, &canonical_splits, style));
            if as_written.is_none() {
                return Err(Refusal::Outside);
            }
            PathBuf::from(given)
        }
    };

    let (existing, rest) = match access {
        Access::Read => (candidate.clone(), Vec::new()),
        Access::Write => {
            if fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Refusal::Link);
            }
            let mut existing = candidate.clone();
            let mut rest = Vec::new();
            loop {
                match fs::symlink_metadata(&existing) {
                    Ok(_) => break,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        let Some(name) = existing.file_name().map(|n| n.to_os_string()) else {
                            return Err(Refusal::Missing(e.to_string()));
                        };
                        rest.push(name);
                        if !existing.pop() {
                            return Err(Refusal::Missing(e.to_string()));
                        }
                    }
                    Err(e) => return Err(Refusal::Missing(e.to_string())),
                }
            }
            rest.reverse();
            (existing, rest)
        }
    };
    let real = gk_store::canonical_path(&existing).map_err(|e| Refusal::Missing(e.to_string()))?;
    let real_split = split_host(&real)?;
    let Some(below) = inside_any(&real_split, &canonical_splits, style) else {
        return Err(Refusal::Link);
    };
    // Inside a folder, hidden files and folders stay off limits.
    if below.iter().any(|p| p.starts_with('.')) {
        return Err(Refusal::Hidden);
    }
    let mut path = real;
    for name in &rest {
        let name = name.to_str().ok_or(Refusal::Name)?;
        if let Some(refusal) = check_part(name) {
            return Err(refusal);
        }
        path.push(name);
    }
    if access == Access::Write && below.is_empty() && rest.is_empty() {
        // An export can't take the place of an allowed folder itself.
        return Err(Refusal::Name);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn w(path: &str) -> Split {
        split(path, Style::Windows).unwrap()
    }

    #[test]
    fn plain_names_are_usable_and_others_are_not() {
        for good in [
            "report.pdf",
            "tm-backup.zip",
            "notes",
            "Ärger über ß.md",
            "a b.txt",
            "console.txt",
            "LPT10",
        ] {
            assert_eq!(check_part(good), None, "{good}");
        }
        for (bad, why) in [
            ("..", Refusal::Parent),
            (".", Refusal::Parent),
            (".ssh", Refusal::Hidden),
            (".env", Refusal::Hidden),
            ("a:b", Refusal::Name),
            ("file.txt:stream", Refusal::Name),
            ("a\\b", Refusal::Name),
            ("a*", Refusal::Name),
            ("a?", Refusal::Name),
            ("x<y", Refusal::Name),
            ("pipe|", Refusal::Name),
            ("quote\"", Refusal::Name),
            ("trailing.", Refusal::Name),
            ("trailing ", Refusal::Name),
            ("CON", Refusal::Name),
            ("con.txt", Refusal::Name),
            ("Nul.tar.gz", Refusal::Name),
            ("com1", Refusal::Name),
            ("LPT9.log", Refusal::Name),
            ("line\nbreak", Refusal::Name),
        ] {
            assert_eq!(check_part(bad), Some(why), "{bad:?}");
        }
        assert_eq!(check_part(&"a".repeat(256)), Some(Refusal::Name));
    }

    #[test]
    fn relative_paths_are_plain_names_all_the_way_down() {
        for style in [Style::Unix, Style::Windows] {
            assert!(check_text("a.zip", style).is_ok());
            assert!(check_text("sub/a.zip", style).is_ok());
            assert_eq!(check_text("../a.zip", style), Err(Refusal::Parent));
            assert_eq!(check_text("sub/../../a.zip", style), Err(Refusal::Parent));
            assert_eq!(check_text("./a.zip", style), Err(Refusal::Parent));
            assert_eq!(check_text("sub/.git/config", style), Err(Refusal::Hidden));
            assert_eq!(check_text("", style), Err(Refusal::Name));
            assert_eq!(check_text("  ", style), Err(Refusal::Name));
            assert_eq!(check_text("a\0b", style), Err(Refusal::Name));
        }
        // Backslashes separate on Windows, so `..` behind one is seen; on
        // Unix they are part of the name, which is refused.
        assert_eq!(
            check_text(r"sub\..\..\a.zip", Style::Windows),
            Err(Refusal::Parent)
        );
        assert_eq!(
            check_text(r"sub\..\..\a.zip", Style::Unix),
            Err(Refusal::Name)
        );
    }

    #[test]
    fn absolute_paths_may_not_go_up_or_name_streams() {
        assert!(check_text("/home/me/Documents/a.pdf", Style::Unix).is_ok());
        assert_eq!(
            check_text("/home/me/exports/../.ssh/id_ed25519", Style::Unix),
            Err(Refusal::Parent)
        );
        assert!(check_text(r"C:\Users\me\a.pdf", Style::Windows).is_ok());
        assert_eq!(
            check_text(r"C:\Users\me\..\x", Style::Windows),
            Err(Refusal::Parent)
        );
        assert_eq!(
            check_text(r"C:\Users\me\a.txt:secret", Style::Windows),
            Err(Refusal::Name)
        );
        // Forms with no fixed place, and device paths.
        for bad in [
            r"C:a.pdf",
            r"\Users\me\a.pdf",
            r"\\.\PhysicalDrive0",
            r"\\?\",
            r"\\server",
            r"\??\C:\x",
        ] {
            assert_eq!(check_text(bad, Style::Windows), Err(Refusal::Name), "{bad}");
        }
    }

    #[test]
    fn windows_paths_compare_without_case_and_in_every_form() {
        let folder = w(r"C:\Users\Me\Shared");
        for path in [
            r"c:\users\me\shared\a.pdf",
            r"C:/Users/Me/Shared/a.pdf",
            r"\\?\C:\USERS\ME\SHARED\a.pdf",
        ] {
            assert_eq!(
                below(&w(path), &folder, Style::Windows),
                Some(&["a.pdf".to_owned()][..]),
                "{path}"
            );
        }
        for path in [
            r"C:\Users\Me\SharedOther\a.pdf",
            r"D:\Users\Me\Shared\a.pdf",
            r"C:\Users\Me",
        ] {
            assert_eq!(below(&w(path), &folder, Style::Windows), None, "{path}");
        }
        let share = w(r"\\nas\team\docs");
        assert!(below(&w(r"\\?\UNC\NAS\Team\Docs\x.csv"), &share, Style::Windows).is_some());
        assert!(below(&w(r"\\nas\other\docs\x.csv"), &share, Style::Windows).is_none());
        // Case counts on Unix.
        let unix = split("/home/me/Shared", Style::Unix).unwrap();
        let other = split("/home/me/shared/a.pdf", Style::Unix).unwrap();
        assert_eq!(below(&other, &unix, Style::Unix), None);
        let prefix = split("/home/me/SharedOther/a.pdf", Style::Unix).unwrap();
        assert_eq!(below(&prefix, &unix, Style::Unix), None);
    }

    struct Folders {
        _dir: tempfile::TempDir,
        root: PathBuf,
        exports: PathBuf,
        shared: PathBuf,
        secret: PathBuf,
    }

    impl Folders {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = gk_store::canonical_path(dir.path()).unwrap();
            let exports = root.join("ws").join("exports");
            let shared = root.join("shared");
            let secret = root.join("secret");
            for d in [&exports, &shared, &secret] {
                fs::create_dir_all(d).unwrap();
            }
            fs::write(exports.join("in.csv"), "a\n").unwrap();
            fs::write(shared.join("doc.pdf"), "pdf").unwrap();
            fs::write(secret.join("key"), "secret").unwrap();
            Self {
                _dir: dir,
                root,
                exports,
                shared,
                secret,
            }
        }

        fn allowed(&self) -> Vec<PathBuf> {
            vec![self.exports.clone(), self.shared.clone()]
        }

        fn resolve(&self, given: &str, access: Access) -> Result<PathBuf, Refusal> {
            resolve(&self.allowed(), given, access)
        }
    }

    /// An absolute path outside every test folder, written as this
    /// system writes paths.
    const ELSEWHERE: &str = if cfg!(windows) {
        r"C:\Windows\win.ini"
    } else {
        "/etc/passwd"
    };

    fn s(p: &Path) -> String {
        p.to_str().unwrap().to_owned()
    }

    #[test]
    fn names_go_to_exports_and_absolute_paths_must_be_inside_a_folder() {
        let f = Folders::new();
        assert_eq!(
            f.resolve("in.csv", Access::Read),
            Ok(f.exports.join("in.csv"))
        );
        assert_eq!(
            f.resolve("new.zip", Access::Write),
            Ok(f.exports.join("new.zip"))
        );
        assert_eq!(
            f.resolve("sub/new.zip", Access::Write),
            Ok(f.exports.join("sub").join("new.zip"))
        );
        assert_eq!(
            f.resolve(&s(&f.shared.join("doc.pdf")), Access::Read),
            Ok(f.shared.join("doc.pdf"))
        );
        assert_eq!(
            f.resolve(&s(&f.exports.join("in.csv")), Access::Read),
            Ok(f.exports.join("in.csv"))
        );
        assert_eq!(
            f.resolve(&s(&f.secret.join("key")), Access::Read),
            Err(Refusal::Outside)
        );
        // Outside: refused before looking, whether the file exists or not.
        assert_eq!(
            f.resolve(&s(&f.secret.join("none")), Access::Read),
            Err(Refusal::Outside)
        );
        assert_eq!(f.resolve(ELSEWHERE, Access::Read), Err(Refusal::Outside));
        assert_eq!(
            f.resolve(&s(&f.root.join("sharedx").join("a")), Access::Read),
            Err(Refusal::Outside)
        );
        assert!(matches!(
            f.resolve("missing.csv", Access::Read),
            Err(Refusal::Missing(_))
        ));
        // A folder that may be read, but not replaced by an export.
        assert_eq!(f.resolve(&s(&f.shared), Access::Read), Ok(f.shared.clone()));
        assert_eq!(f.resolve(&s(&f.shared), Access::Write), Err(Refusal::Name));
        assert_eq!(
            f.resolve(
                &s(&f.shared.join("..").join("secret").join("key")),
                Access::Read
            ),
            Err(Refusal::Parent)
        );
    }

    #[test]
    fn hidden_files_inside_a_folder_are_refused() {
        let f = Folders::new();
        fs::create_dir_all(f.shared.join(".git")).unwrap();
        fs::write(f.shared.join(".git").join("config"), "x").unwrap();
        assert_eq!(
            f.resolve(&s(&f.shared.join(".git").join("config")), Access::Read),
            Err(Refusal::Hidden)
        );
        assert_eq!(
            f.resolve(&s(&f.shared.join(".new")), Access::Write),
            Err(Refusal::Hidden)
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_must_stay_inside_the_folders() {
        use std::os::unix::fs::symlink;
        let f = Folders::new();
        symlink(f.secret.join("key"), f.exports.join("key.txt")).unwrap();
        symlink(&f.secret, f.exports.join("out")).unwrap();
        symlink(f.shared.join("doc.pdf"), f.exports.join("doc.pdf")).unwrap();
        symlink(&f.secret, f.shared.join("also-secret")).unwrap();
        // A link to a file outside, or through a linked folder.
        assert_eq!(f.resolve("key.txt", Access::Read), Err(Refusal::Link));
        assert_eq!(f.resolve("out/key", Access::Read), Err(Refusal::Link));
        assert_eq!(
            f.resolve(&s(&f.shared.join("also-secret").join("key")), Access::Read),
            Err(Refusal::Link)
        );
        // An export through a linked folder, or onto a link.
        assert_eq!(f.resolve("out/new.zip", Access::Write), Err(Refusal::Link));
        assert_eq!(f.resolve("key.txt", Access::Write), Err(Refusal::Link));
        // A link between allowed folders is fine; the real place is used.
        assert_eq!(
            f.resolve("doc.pdf", Access::Read),
            Ok(f.shared.join("doc.pdf"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_allowed_folder_counts_where_it_leads() {
        use std::os::unix::fs::symlink;
        let f = Folders::new();
        let link = f.root.join("shared-link");
        symlink(&f.shared, &link).unwrap();
        let allowed = vec![f.exports.clone(), link.clone()];
        assert_eq!(
            resolve(&allowed, &s(&link.join("doc.pdf")), Access::Read),
            Ok(f.shared.join("doc.pdf"))
        );
        assert_eq!(
            resolve(&allowed, &s(&f.shared.join("doc.pdf")), Access::Read),
            Ok(f.shared.join("doc.pdf"))
        );
    }

    /// Words in an input field's name that say it may be a place on disk.
    const PATH_WORDS: &[&str] = &["path", "folder", "dir", "file_name", "location"];

    /// Fields with such a name that are not a place on disk: a project
    /// folder in the sidebar is named by its name or ID.
    const NOT_ON_DISK: &[(&str, &str)] = &[
        ("folder.update", "folder"),
        ("folder.rename", "folder"),
        ("folder.delete", "folder"),
        ("folder.move", "folder"),
        // The folder a new top-level project goes in: looked up by ID or
        // name in the workspace, never opened on disk.
        ("project.create", "folder"),
        ("template.apply", "folder"),
    ];

    /// The top-level fields of an input schema, also those of each variant.
    fn fields(schema: &Value, out: &mut Vec<String>) {
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            out.extend(props.keys().cloned());
        }
        for key in ["anyOf", "oneOf", "allOf"] {
            for variant in schema
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                fields(variant, out);
            }
        }
    }

    /// Every input an agent can give that could name a place on disk is
    /// checked: a new one can't be added without being listed.
    #[test]
    fn every_path_an_agent_can_give_is_checked() {
        let mut missing = Vec::new();
        for desc in gk_api::CATALOG {
            if desc.is_app_only() {
                continue;
            }
            let mut names = Vec::new();
            fields(&(desc.input_schema)().to_value(), &mut names);
            for name in names {
                let lower = name.to_ascii_lowercase();
                let listed = PATH_INPUTS
                    .iter()
                    .any(|(o, f, _)| *o == desc.id && *f == name)
                    || NOT_ON_DISK.contains(&(desc.id, name.as_str()));
                if PATH_WORDS.iter().any(|w| lower.contains(w)) && !listed {
                    missing.push(format!("{}.{name}", desc.id));
                }
            }
        }
        assert!(missing.is_empty(), "add these to PATH_INPUTS: {missing:?}");
        for (op, field, _) in PATH_INPUTS {
            let desc =
                gk_api::descriptor(op).unwrap_or_else(|| panic!("{op} is not in the catalog"));
            assert!(!desc.is_app_only(), "{op} is only for the person");
            let mut names = Vec::new();
            fields(&(desc.input_schema)().to_value(), &mut names);
            assert!(
                names.iter().any(|n| n == field),
                "{op} has no input {field}"
            );
        }
    }

    #[test]
    fn the_workspace_and_its_backups_stay_off_limits_inside_an_added_folder() {
        let dir = tempfile::tempdir().unwrap();
        let data = gk_store::canonical_path(dir.path()).unwrap();
        let layout = WorkspaceLayout::new(data.join("workspaces").join("Personal"));
        let backups = data.join("backups").join("Personal");
        fs::create_dir_all(layout.root()).unwrap();
        fs::create_dir_all(&backups).unwrap();
        fs::create_dir_all(layout.exports_dir()).unwrap();
        fs::write(layout.root().join("workspace.db"), "db").unwrap();
        fs::write(backups.join("copy.db"), "db").unwrap();
        fs::write(layout.exports_dir().join("in.csv"), "a").unwrap();
        fs::write(data.join("mine.txt"), "x").unwrap();
        // The person added the whole data folder.
        let source = AgentFolders::Fixed(vec![data.clone()]);
        let at = |p: &Path| check(&layout, &source, "path", p.to_str().unwrap(), Access::Read);
        assert!(at(&data.join("mine.txt")).is_ok());
        assert!(at(&layout.exports_dir().join("in.csv")).is_ok());
        for refused in [layout.root().join("workspace.db"), backups.join("copy.db")] {
            let e = at(&refused).unwrap_err();
            assert_eq!(e.code, gk_api::ErrorCode::Forbidden, "{refused:?}");
            assert_eq!(e.details["message_id"], "agent-path-outside");
        }
        let e = check(
            &layout,
            &source,
            "path",
            &data
                .join("workspaces")
                .join("Personal")
                .join("new.zip")
                .to_string_lossy(),
            Access::Write,
        )
        .unwrap_err();
        assert_eq!(e.code, gk_api::ErrorCode::Forbidden);
    }

    #[test]
    fn the_engine_reads_no_personal_settings_unless_told() {
        assert_eq!(
            crate::EngineConfig::new(crate::Role::Cli).agent_folders,
            AgentFolders::Fixed(Vec::new())
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let source = AgentFolders::File(file.clone());
        assert!(source.added().is_empty(), "a missing file adds nothing");
        let shared = dir.path().join("shared");
        let stored = serde_json::json!({"agent_folders": [s(&shared), "relative"]});
        fs::write(&file, stored.to_string()).unwrap();
        assert_eq!(source.added(), vec![shared]);
    }

    #[test]
    fn errors_name_the_folders_in_their_hint() {
        let f = Folders::new();
        let layout = WorkspaceLayout::new(f.exports.parent().unwrap());
        let source = AgentFolders::Fixed(vec![f.shared.clone()]);
        let e = check(&layout, &source, "path", ELSEWHERE, Access::Read).unwrap_err();
        let hint = e.hint.clone().unwrap();
        assert!(
            hint.contains(&s(&f.exports)) && hint.contains(&s(&f.shared)),
            "{hint}"
        );
        assert_eq!(e.details["hint_id"], "agent-path-hint");
        assert_eq!(
            e.details["allowed"],
            serde_json::json!([s(&f.exports), s(&f.shared)])
        );
        assert_eq!(e.details["path"], "path");
        let e = check(&layout, &source, "path", "../x", Access::Read).unwrap_err();
        assert_eq!(e.code, gk_api::ErrorCode::Invalid);
        assert_eq!(e.details["message_id"], "agent-path-parent");
    }

    #[test]
    fn the_workspace_exports_folder_is_made_when_needed() {
        let dir = tempfile::tempdir().unwrap();
        let exports = dir.path().join("ws").join("exports");
        let made = resolve(std::slice::from_ref(&exports), "a.zip", Access::Write).unwrap();
        assert!(exports.is_dir());
        assert_eq!(
            made,
            gk_store::canonical_path(&exports).unwrap().join("a.zip")
        );
    }
}
