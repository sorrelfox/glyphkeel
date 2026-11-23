// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask check-headers`: every source file starts with the licence
//! header, in its own comment syntax.

use std::fs;
use std::path::{Path, PathBuf};

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
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
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
        Some("rs" | "toml" | "sh" | "ts" | "tsx" | "js" | "css" | "yaml" | "yml")
    )
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
        } else if checked(&path) && !SKIP_FILES.iter().any(|skip| rel == Path::new(skip)) {
            out.push(rel);
        }
    }
    out.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::has_header;

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
}
