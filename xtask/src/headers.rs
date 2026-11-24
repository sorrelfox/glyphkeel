// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask check-headers`: every source file starts with the licence
//! header, in its own comment syntax.
//!
//! The files are the ones git knows about, tracked or untracked but not
//! ignored, so generated files that `.gitignore` lists are left alone. Outside
//! a git checkout (in the CI containers, which get a copy of exactly those
//! files) the directory tree is walked instead.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

const SPDX: &str = "SPDX-License-Identifier: GPL-3.0-or-later";

/// Directories never searched: build output, dependencies, generated code.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "gen",
    "third_party",
    "test-results",
    "playwright-report",
];

/// Files that keep their own form: the design's two stylesheets, and the
/// lockfile, which pnpm writes.
const SKIP_FILES: &[&str] = &[
    "ui/src/styles/tokens.css",
    "ui/src/styles/components.css",
    "pnpm-lock.yaml",
];

pub(crate) fn run(root: &Path) -> Result<bool> {
    let files: Vec<PathBuf> = match git_files(root) {
        Some(files) => files,
        None => {
            let mut files = Vec::new();
            collect(root, root, &mut files)?;
            files
        }
    }
    .into_iter()
    .filter(|file| wanted(file))
    .collect();
    let mut missing = Vec::new();
    for file in &files {
        let text = fs::read_to_string(root.join(file))
            .with_context(|| format!("cannot read {}", file.display()))?;
        if !has_header(&text) {
            missing.push(file);
        }
    }
    if missing.is_empty() {
        eprintln!(
            "check-headers: {} files, all with the licence header",
            files.len()
        );
        return Ok(true);
    }
    for file in &missing {
        eprintln!("missing licence header: {}", file.display());
    }
    eprintln!(
        "check-headers: {} of {} files lack the header",
        missing.len(),
        files.len()
    );
    Ok(false)
}

/// The header is in the first five lines (after a shebang, for scripts).
fn has_header(text: &str) -> bool {
    text.lines().take(5).any(|line| line.contains(SPDX))
}

fn checked(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if name.ends_with(".Containerfile") {
        return true;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "toml" | "sh" | "py" | "ts" | "tsx" | "js" | "css" | "yaml" | "yml")
    )
}

/// Whether a file, relative to the root, is one the check covers.
fn wanted(rel: &Path) -> bool {
    let skipped_dir = rel.parent().is_some_and(|dir| {
        dir.components()
            .any(|part| SKIP_DIRS.iter().any(|skip| part.as_os_str() == *skip))
    });
    !skipped_dir && checked(rel) && !SKIP_FILES.iter().any(|skip| rel == Path::new(skip))
}

/// The files git knows about under `root`, tracked or untracked but not
/// ignored, that exist on disk. `None` when git is missing or `root` is not
/// in a git checkout.
pub(crate) fn git_files(root: &Path) -> Option<Vec<PathBuf>> {
    let out = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut files = parse_ls_files(&out.stdout);
    // A tracked file deleted in the working tree is still listed.
    files.retain(|file| root.join(file).is_file());
    Some(files)
}

/// Splits the NUL-separated output of `git ls-files -z`, sorted and without
/// repeats (a file with merge conflicts is listed once per stage).
fn parse_ls_files(stdout: &[u8]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = stdout
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| PathBuf::from(String::from_utf8_lossy(name).into_owned()))
        .collect();
    files.sort();
    files.dedup();
    files
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("cannot list {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name) {
                collect(root, &path, out)?;
            }
        } else {
            out.push(rel);
        }
    }
    out.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{git_files, has_header, parse_ls_files, wanted};

    #[test]
    fn header_is_found_in_any_comment_syntax() {
        assert!(has_header("// SPDX-License-Identifier: GPL-3.0-or-later\n"));
        assert!(has_header(
            "#!/bin/sh\n# SPDX-License-Identifier: GPL-3.0-or-later\n"
        ));
        assert!(has_header("/* SPDX-License-Identifier: GPL-3.0-or-later\n"));
        assert!(!has_header("fn main() {}\n"));
        assert!(!has_header(
            "a\nb\nc\nd\ne\n// SPDX-License-Identifier: GPL-3.0-or-later\n"
        ));
    }

    #[test]
    fn ls_files_output_is_split_sorted_and_deduplicated() {
        let files = parse_ls_files(b"b.rs\0a/c.ts\0b.rs\0\0");
        assert_eq!(files, [PathBuf::from("a/c.ts"), PathBuf::from("b.rs")]);
        assert!(parse_ls_files(b"").is_empty());
    }

    #[test]
    fn skipped_directories_files_and_extensions_are_left_out() {
        assert!(wanted(Path::new("crates/gk-core/src/lib.rs")));
        assert!(wanted(Path::new("ci/containers/fedora.Containerfile")));
        assert!(!wanted(Path::new("ui/node_modules/x/index.js")));
        assert!(!wanted(Path::new("third_party/a/lib.rs")));
        assert!(!wanted(Path::new("ui/src/styles/tokens.css")));
        assert!(!wanted(Path::new("pnpm-lock.yaml")));
        assert!(!wanted(Path::new("README.md")));
        // A file named like a skipped directory is still checked.
        assert!(wanted(Path::new("xtask/src/gen.rs")));
    }

    #[test]
    fn ignored_files_are_not_listed() {
        let dir = std::env::temp_dir().join(format!("xtask-headers-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("generated")).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .is_ok_and(|out| out.status.success())
        };
        if !git(&["init", "--quiet"]) {
            // No git on this machine: the walk is used instead.
            std::fs::remove_dir_all(&dir).unwrap();
            return;
        }
        std::fs::write(dir.join(".gitignore"), "generated/\n").unwrap();
        std::fs::write(dir.join("kept.toml"), "").unwrap();
        std::fs::write(dir.join("generated/skipped.toml"), "").unwrap();
        let files = git_files(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            files,
            [PathBuf::from(".gitignore"), PathBuf::from("kept.toml")]
        );
    }
}
