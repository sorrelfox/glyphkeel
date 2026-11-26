// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The Flathub submission.
//!
//! - `cargo xtask flathub --tag TAG --commit SHA --out DIR` writes the files
//!   of the Flathub repository: the manifest with a `git` source naming the
//!   release tag and its commit (in place of the `dir` source it builds from
//!   here) and without `default-branch`, flathub.json, and the generated
//!   offline source lists, which it copies from `--sources`.
//! - `cargo xtask ci --suite flathub` (from the host) checks them in the
//!   image of `ci/containers/flathub.Containerfile` (`ci/flathub/build.sh`):
//!   it generates the source lists with flatpak-builder-tools at the commit
//!   the pre-release workflow pins, lints the manifest and the AppStream file
//!   with Flathub's linter, builds the manifest with flatpak-builder from
//!   this checkout, validates the installed AppStream and desktop files in
//!   the build, runs `glyphkeel --version` there and lints the built
//!   repository. Nothing is installed or started outside the build sandbox.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::ci::{self, Engine};

const HELP: &str = "\
Usage: cargo xtask flathub --tag TAG --commit SHA --out DIR [--sources DIR]

Writes the files of the Flathub repository into DIR: the manifest
io.github.sorrelfox.Glyphkeel.yml with a git source naming TAG and its commit
SHA, flathub.json, and cargo-sources.json and node-sources.json from the
sources folder (default: the manifest's own folder, where the source
generators write them).
";

/// The application ID, which names the manifest.
pub(crate) const APP_ID: &str = "io.github.sorrelfox.Glyphkeel";

/// The folder of the manifest, relative to the repository.
const FLATPAK_DIR: &str = "crates/gk-app/packaging/flatpak";

/// The repository Flathub builds from.
const GIT_URL: &str = "https://github.com/sorrelfox/glyphkeel.git";

/// The generated source lists the manifest names.
const SOURCE_LISTS: [&str; 2] = ["cargo-sources.json", "node-sources.json"];

/// The pre-release workflow, which pins flatpak-builder-tools.
const WORKFLOW: &str = ".github/workflows/prerelease.yml";

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let (mut tag, mut commit, mut out, mut sources) = (None, None, None, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .with_context(|| format!("`{arg}` needs a value"))
        };
        match arg.as_str() {
            "--tag" => tag = Some(value()?),
            "--commit" => commit = Some(value()?),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--sources" => sources = Some(PathBuf::from(value()?)),
            "-h" | "--help" => {
                eprint!("{HELP}");
                return Ok(true);
            }
            other => bail!("unknown option `{other}`\n\n{HELP}"),
        }
    }
    let tag = tag.context("`--tag` is required")?;
    let commit = commit.context("`--commit` is required")?;
    let out = out.context("`--out` is required")?;
    let sources = sources.unwrap_or_else(|| root.join(FLATPAK_DIR));
    write_submission(root, &Source::Git { tag, commit }, &sources, &out)?;
    eprintln!("flathub: {}", out.display());
    Ok(true)
}

/// The source of the app's own module.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// The release tag and its commit, as Flathub requires.
    Git { tag: String, commit: String },
    /// The checkout the manifest is in, as it is in the repository.
    Checkout,
}

/// Writes the manifest, flathub.json and the source lists into `out`.
fn write_submission(root: &Path, source: &Source, sources: &Path, out: &Path) -> Result<()> {
    let dir = root.join(FLATPAK_DIR);
    let manifest = fs::read_to_string(dir.join(format!("{APP_ID}.yml")))
        .with_context(|| format!("cannot read the manifest in {}", dir.display()))?;
    fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    fs::write(
        out.join(format!("{APP_ID}.yml")),
        flathub_manifest(&manifest, source)?,
    )
    .context("cannot write the manifest")?;
    fs::copy(dir.join("flathub.json"), out.join("flathub.json"))
        .context("cannot copy flathub.json")?;
    for name in SOURCE_LISTS {
        fs::copy(sources.join(name), out.join(name)).with_context(|| {
            format!(
                "{name} is missing in {}; generate it as the manifest's header says",
                sources.display()
            )
        })?;
    }
    Ok(())
}

/// The manifest as Flathub takes it: a header of its own, no
/// `default-branch` (Flathub chooses the branch) and, for `Source::Git`, the
/// `dir` source of the app's module replaced by the tag and its commit.
pub(crate) fn flathub_manifest(manifest: &str, source: &Source) -> Result<String> {
    let lines: Vec<&str> = manifest.lines().collect();
    // The header: the comment lines at the top. The licence lines stay.
    let body_start = lines
        .iter()
        .position(|line| !line.starts_with('#'))
        .context("the manifest is only comments")?;
    let mut out: Vec<String> = lines[..body_start]
        .iter()
        .filter(|line| line.starts_with("# SPDX-") || line.starts_with("# Copyright"))
        .map(|line| (*line).to_owned())
        .collect();
    out.push("#".to_owned());
    out.push(
        "# The Flatpak of Glyphkeel, written by `cargo xtask flathub` from the manifest in"
            .to_owned(),
    );
    out.push(format!(
        "# {} (crates/gk-app/packaging/flatpak/).",
        GIT_URL.trim_end_matches(".git")
    ));
    out.push(
        "# cargo-sources.json and node-sources.json are generated from its lock files".to_owned(),
    );
    out.push("# with flatpak-builder-tools.".to_owned());

    let mut dropped_branch = false;
    let mut replaced_source = false;
    let mut index = body_start;
    while index < lines.len() {
        let line = lines[index];
        if line.starts_with("default-branch:") {
            dropped_branch = true;
            // Its comment, the line before, goes with it.
            if out.last().is_some_and(|last| last.starts_with('#')) && index > body_start {
                out.pop();
            }
            index += 1;
            continue;
        }
        if line.trim() == "- type: dir" {
            let indent = &line[..line.len() - line.trim_start().len()];
            // The `dir` entry runs until the next entry of the same list.
            let mut end = index + 1;
            while end < lines.len() {
                let next = lines[end];
                let next_indent = next.len() - next.trim_start().len();
                if next.trim_start().starts_with("- ") && next_indent <= indent.len() {
                    break;
                }
                end += 1;
            }
            match source {
                Source::Git { tag, commit } => {
                    if out
                        .last()
                        .is_some_and(|last| last.trim_start().starts_with('#'))
                    {
                        out.pop();
                    }
                    out.push(format!("{indent}# The release, by its tag and commit."));
                    out.push(format!("{indent}- type: git"));
                    out.push(format!("{indent}  url: {GIT_URL}"));
                    out.push(format!("{indent}  tag: {tag}"));
                    out.push(format!("{indent}  commit: {commit}"));
                }
                Source::Checkout => {
                    out.extend(lines[index..end].iter().map(|line| (*line).to_owned()));
                }
            }
            replaced_source = true;
            index = end;
            continue;
        }
        out.push(line.to_owned());
        index += 1;
    }
    if !dropped_branch {
        bail!("the manifest has no `default-branch` line");
    }
    if !replaced_source {
        bail!("the manifest has no `- type: dir` source");
    }
    if let Source::Git { tag, commit } = source {
        if !tag.starts_with('v') || tag.contains(char::is_whitespace) {
            bail!("`{tag}` is not a release tag such as v1.0.0");
        }
        if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("`{commit}` is not a full commit hash");
        }
    }
    Ok(out.join("\n") + "\n")
}

/// The flatpak-builder-tools repository and commit the pre-release
/// workflow pins.
fn builder_tools(root: &Path) -> Result<(String, String)> {
    let text = fs::read_to_string(root.join(WORKFLOW))
        .with_context(|| format!("cannot read {WORKFLOW}"))?;
    let find = |key: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .map(|value| value.trim().to_owned())
            .with_context(|| format!("{WORKFLOW} sets no {key}"))
    };
    Ok((find("BUILDER_TOOLS_REPO:")?, find("BUILDER_TOOLS_COMMIT:")?))
}

/// The `flathub` suite of `cargo xtask ci`, from the host.
pub(crate) struct Check<'a> {
    pub(crate) root: &'a Path,
    pub(crate) engine: Engine,
    pub(crate) pins: &'a ci::Pins,
    pub(crate) rebuild: bool,
    pub(crate) cache_dir: &'a Path,
}

/// The image's name, under the CI prefix.
const IMAGE: &str = "flathub";

impl Check<'_> {
    pub(crate) fn run(&self) -> Result<bool> {
        let started = Instant::now();
        let out = self.cache_dir.join("flathub");
        if out.exists() {
            fs::remove_dir_all(&out).with_context(|| format!("cannot remove {}", out.display()))?;
        }
        fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
        // The manifest Flathub gets (linted, never built here: its tag may
        // not exist yet) and the same one with this checkout as its source
        // (built).
        let manifest =
            fs::read_to_string(self.root.join(FLATPAK_DIR).join(format!("{APP_ID}.yml")))
                .context("cannot read the Flatpak manifest")?;
        let stage = out.join("stage");
        fs::create_dir_all(&stage).with_context(|| format!("cannot create {}", stage.display()))?;
        let example = Source::Git {
            tag: format!("v{}", env!("CARGO_PKG_VERSION")),
            commit: head_commit(self.root).unwrap_or_else(|| "0".repeat(40)),
        };
        fs::write(
            stage.join("flathub.yml"),
            flathub_manifest(&manifest, &example)?,
        )
        .context("cannot write the Flathub manifest")?;
        fs::write(
            stage.join("local.yml"),
            flathub_manifest(&manifest, &Source::Checkout)?,
        )
        .context("cannot write the local manifest")?;

        eprintln!("==> flathub: building the image {}:{IMAGE}", ci::prefix());
        if !ci::build_named_image(self.root, self.engine, IMAGE, self.pins, self.rebuild)? {
            return Ok(false);
        }
        let (tools_repo, tools_commit) = builder_tools(self.root)?;
        let files = crate::headers::git_files(self.root);
        let source = self
            .root
            .to_str()
            .context("the repository path is not valid UTF-8")?;
        let mut cmd = ci::container_command(self.engine, source);
        // flatpak-builder runs each build step in a bubblewrap sandbox,
        // which needs namespaces the default container profile denies.
        // Rootless, this grants nothing beyond the user's own rights.
        cmd.arg("--privileged");
        for (host, inside, mode) in [
            (self.root.join("ci/flathub"), "/ci", "ro"),
            (out.clone(), "/out", "rw"),
        ] {
            cmd.arg("--volume").arg(format!(
                "{}:{inside}:{mode}",
                fs::canonicalize(&host)
                    .with_context(|| format!("cannot resolve {}", host.display()))?
                    .display()
            ));
        }
        for volume in ci::named_volumes(
            &[
                ("flatpak-user".to_owned(), "/root/.local/share/flatpak"),
                ("flathub-state".to_owned(), "/cache"),
            ],
            None,
        )? {
            cmd.arg("--volume").arg(volume);
        }
        cmd.args(["--env", &format!("BUILDER_TOOLS_REPO={tools_repo}")])
            .args(["--env", &format!("BUILDER_TOOLS_COMMIT={tools_commit}")])
            .args(["--env", &format!("APP_ID={APP_ID}")])
            .args(["--env", &format!("FLATPAK_DIR={FLATPAK_DIR}")]);
        let script = format!(
            "set -e; {}; exec sh /ci/build.sh </dev/null",
            ci::copy_command(files.is_some())
        );
        cmd.arg(format!("{}:{IMAGE}", ci::prefix()))
            .args(["sh", "-c", &script]);
        let ok = ci::run_with_files(cmd, files.as_deref())?;
        let seconds = started.elapsed().as_secs();
        if ok {
            eprintln!(
                "==> flathub: passed in {seconds} s; the submission's files: {}",
                out.join("submission").display()
            );
        } else {
            eprintln!(
                "==> flathub: FAILED after {seconds} s (reports: {})",
                out.display()
            );
        }
        Ok(ok)
    }
}

/// The commit checked out, for the example manifest.
fn head_commit(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    let commit = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && commit.len() == 40).then_some(commit)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{APP_ID, FLATPAK_DIR, Source, builder_tools, flathub_manifest};

    fn manifest() -> String {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        std::fs::read_to_string(root.join(FLATPAK_DIR).join(format!("{APP_ID}.yml"))).unwrap()
    }

    fn git() -> Source {
        Source::Git {
            tag: "v1.2.3".to_owned(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        }
    }

    #[test]
    fn flathub_gets_the_tag_and_commit_and_no_branch() {
        let out = flathub_manifest(&manifest(), &git()).unwrap();
        assert!(out.starts_with("# SPDX-License-Identifier: GPL-3.0-or-later\n"));
        assert!(!out.contains("default-branch"));
        assert!(!out.contains("type: dir"));
        assert!(!out.contains("path: ../../../.."));
        assert!(out.contains(
            "      - type: git\n        url: https://github.com/sorrelfox/glyphkeel.git\n        \
             tag: v1.2.3\n        commit: 0123456789abcdef0123456789abcdef01234567\n      \
             - cargo-sources.json\n      - node-sources.json\n"
        ));
        assert!(out.contains(&format!("\nid: {APP_ID}\n")));
        // Everything else is kept, the build commands among them.
        assert!(out.contains("cargo --offline build --locked --profile dist --package gk-app"));
        assert!(out.contains("- --socket=wayland"));
        assert!(!out.contains("  - --share=network"));
        assert!(
            !out.contains("  - --own-name"),
            "Flatpak grants the app's own name"
        );
    }

    #[test]
    fn the_local_build_keeps_the_checkout_as_its_source() {
        let out = flathub_manifest(&manifest(), &Source::Checkout).unwrap();
        assert!(!out.contains("default-branch"));
        assert!(out.contains("      - type: dir\n        path: ../../../..\n"));
        assert!(out.contains("          - ui/dist\n      - cargo-sources.json\n"));
    }

    #[test]
    fn only_a_release_tag_and_a_full_commit_are_taken() {
        let bad_tag = Source::Git {
            tag: "main".to_owned(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        };
        assert!(flathub_manifest(&manifest(), &bad_tag).is_err());
        let short = Source::Git {
            tag: "v1.0.0".to_owned(),
            commit: "0123abc".to_owned(),
        };
        assert!(flathub_manifest(&manifest(), &short).is_err());
        assert!(flathub_manifest("# x\nid: a\n", &git()).is_err());
    }

    #[test]
    fn flathub_json_builds_where_the_toolchain_is_pinned() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = std::fs::read_to_string(root.join(FLATPAK_DIR).join("flathub.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["only-arches"], serde_json::json!(["x86_64"]));
        // The Rust archive in the manifest is for x86_64 only.
        assert!(manifest().contains("only-arches:\n          - x86_64"));
    }

    #[test]
    fn the_source_generators_are_pinned_in_the_workflow() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (repo, commit) = builder_tools(&root).unwrap();
        assert!(repo.starts_with("https://github.com/flatpak/flatpak-builder-tools"));
        assert_eq!(commit.len(), 40);
    }
}
