// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask release-check` and `cargo xtask check-versions`: the checks
//! before a release, in order, with a record of what ran.
//!
//! The steps reuse the other commands: the static checks, the migration
//! samples and their tests, the UI bundle check, every `cargo xtask ci` run
//! the pre-release workflow makes (each distribution, the packages, the
//! Windows cross build, the Arch package, the Flatpak, Flathub and the
//! repositories), then the checksums of a release folder and the signatures
//! of built repositories. The result goes to a Markdown record with the
//! version, the commit and the tools, followed by the checks that people
//! make by hand.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

const HELP: &str = "\
Usage: cargo xtask release-check [--dry-run] [--only S[,S…]] [--skip S[,S…]]
                                 [--engine E] [--cache-dir DIR] [--release-dir DIR]
                                 [--repos DIR] [--record FILE] [--keep-going]
                                 [--allow-dirty]
       cargo xtask check-versions

Runs the checks before a release, in order, and writes a Markdown record of
the results (default: target/release-check/release-check-<version>.md).

Options:
  --dry-run        Print the steps and their commands; run nothing, write
                   nothing.
  --only S         Run only these steps (comma-separated or repeated).
  --skip S         Leave these steps out.
  --engine E       Passed to `cargo xtask ci` (podman or docker).
  --cache-dir DIR  Passed to `cargo xtask ci`.
  --release-dir DIR
                   The folder of release folders (DIR/<version>/). The
                   Windows installer is copied to DIR/<version>/windows/, and
                   the `checksums` step writes and checks
                   DIR/<version>/SHA256SUMS. Without it, `checksums` is
                   skipped.
  --repos DIR      Repositories built by `cargo xtask repos --out DIR`: the
                   `signatures` step checks their signatures with their own
                   public key. Without it, `signatures` is skipped.
  --record FILE    Where the record goes.
  --keep-going     Run every step even after one fails.
  --allow-dirty    Run with uncommitted changes (the record says so).

`check-versions` checks only that the version agrees everywhere: Cargo.toml,
the lock files, ui/package.json, tauri.conf.json, CHANGELOG.md, the AppStream
file and the PKGBUILD.
";

/// The steps that take hours or need containers, for the dry run's notes.
const LONG: &str = "container";

/// What a step does.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    /// `git status` shows no changes.
    CleanTree,
    /// The version agrees everywhere.
    Versions,
    /// Another `cargo xtask` command.
    Xtask(Vec<String>),
    /// A program with arguments.
    Program(String, Vec<String>),
    /// SHA256SUMS of a release folder.
    Checksums(PathBuf),
    /// The signatures of built repositories.
    Signatures(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    name: String,
    about: &'static str,
    action: Action,
    /// Whether it runs in containers (slow).
    containers: bool,
}

impl Step {
    fn command(&self) -> String {
        match &self.action {
            Action::CleanTree => "git status --porcelain".to_owned(),
            Action::Versions => "cargo xtask check-versions".to_owned(),
            Action::Xtask(args) => format!("cargo xtask {}", args.join(" ")),
            Action::Program(program, args) => format!("{program} {}", args.join(" ")),
            Action::Checksums(dir) => format!(
                "sha256sum (every file of {}) > SHA256SUMS; sha256sum -c",
                dir.display()
            ),
            Action::Signatures(dir) => {
                format!("gpg --verify and rpmkeys --checksig on {}", dir.display())
            }
        }
    }
}

#[derive(Debug, Default)]
struct Options {
    dry_run: bool,
    only: Vec<String>,
    skip: Vec<String>,
    engine: Option<String>,
    cache_dir: Option<PathBuf>,
    release_dir: Option<PathBuf>,
    repos: Option<PathBuf>,
    record: Option<PathBuf>,
    keep_going: bool,
    allow_dirty: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Option<Self>> {
        let mut options = Self::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_owned())),
                _ => (arg.as_str(), None),
            };
            let mut value = |what: &str| -> Result<String> {
                match inline.clone() {
                    Some(v) => Ok(v),
                    None => args
                        .next()
                        .cloned()
                        .with_context(|| format!("`{flag}` needs {what}")),
                }
            };
            match flag {
                "--dry-run" => options.dry_run = true,
                "--keep-going" => options.keep_going = true,
                "--allow-dirty" => options.allow_dirty = true,
                "--only" => options.only.extend(split_list(&value("step names")?)),
                "--skip" => options.skip.extend(split_list(&value("step names")?)),
                "--engine" => options.engine = Some(value("an engine")?),
                "--cache-dir" => options.cache_dir = Some(value("a folder")?.into()),
                "--release-dir" => options.release_dir = Some(value("a folder")?.into()),
                "--repos" => options.repos = Some(value("a folder")?.into()),
                "--record" => options.record = Some(value("a file")?.into()),
                "-h" | "--help" => return Ok(None),
                other => bail!("unknown option `{other}`\n\n{HELP}"),
            }
        }
        Ok(Some(options))
    }
}

fn split_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Every step, in the order they run.
fn all_steps(options: &Options, version: &str) -> Vec<Step> {
    let x = |args: &[&str]| Action::Xtask(args.iter().map(|&s| s.to_owned()).collect());
    let cargo = |args: &[&str]| {
        Action::Program(
            "cargo".to_owned(),
            args.iter().map(|&s| s.to_owned()).collect(),
        )
    };
    let mut ci_extra = Vec::new();
    if let Some(engine) = &options.engine {
        ci_extra.extend(["--engine".to_owned(), engine.clone()]);
    }
    if let Some(dir) = &options.cache_dir {
        ci_extra.extend(["--cache-dir".to_owned(), dir.display().to_string()]);
    }
    let ci = |args: &[&str]| {
        let mut all = vec!["ci".to_owned()];
        all.extend(args.iter().map(|&s| s.to_owned()));
        all.extend(ci_extra.iter().cloned());
        Action::Xtask(all)
    };
    let step = |name: &str, about: &'static str, action: Action| Step {
        name: name.to_owned(),
        about,
        containers: matches!(&action, Action::Xtask(args) if args[0] == "ci"),
        action,
    };

    let mut steps = vec![
        step(
            "clean-tree",
            "no uncommitted changes, so the record names what was checked",
            Action::CleanTree,
        ),
        step("versions", "one version everywhere", Action::Versions),
        step(
            "migration-samples",
            "a sample of every released schema; released migrations unchanged",
            x(&["migration-samples", "--check"]),
        ),
        step(
            "store-upgrades",
            "every sample upgrades and keeps its rows (store)",
            cargo(&[
                "test",
                "--package",
                "gk-store",
                "--test",
                "migration_samples",
                "--locked",
            ]),
        ),
        step(
            "engine-upgrades",
            "every sample opens in the app's engine with its data (engine)",
            cargo(&[
                "test",
                "--package",
                "gk-engine",
                "--test",
                "migration_samples",
                "--locked",
            ]),
        ),
        step("fmt", "formatting", cargo(&["fmt", "--all", "--check"])),
        step("headers", "licence headers", x(&["check-headers"])),
        step("deps", "dependency rules", x(&["check-deps"])),
        step("gen", "generated files up to date", x(&["gen", "--check"])),
        step(
            "clippy",
            "lints",
            cargo(&[
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ]),
        ),
        step(
            "notices",
            "third-party notices and licences",
            x(&["notices", "--check"]),
        ),
        step(
            "ui-build",
            "the release UI",
            Action::Program("pnpm".to_owned(), vec!["build".to_owned()]),
        ),
        step(
            "bundle",
            "the release UI names no remote address",
            x(&["check-bundle"]),
        ),
    ];
    for distro in ["ubuntu-22.04", "ubuntu-24.04", "debian-12", "fedora"] {
        steps.push(step(
            &format!("ci-{distro}"),
            "the default suites: static, rust, ui, app",
            ci(&["--distro", distro]),
        ));
    }
    steps.extend([
        step(
            "ci-deny-audit",
            "cargo-deny and pnpm audit",
            ci(&["--distro", "ubuntu-24.04", "--suite", "deny,audit"]),
        ),
        step(
            "ci-packages",
            "deb and rpm built, installed in clean containers, never online",
            ci(&["--suite", "packages,offline"]),
        ),
        step(
            "ci-perf",
            "performance with data set L (report only)",
            ci(&["--distro", "ubuntu-24.04", "--suite", "perf"]),
        ),
        step(
            "ci-pseudo",
            "screenshots in the pseudo-locale",
            ci(&["--distro", "ubuntu-22.04", "--suite", "pseudo"]),
        ),
        step(
            "ci-arch",
            "the Arch Linux package and the AUR files",
            ci(&["--distro", "arch"]),
        ),
        step(
            "ci-flatpak",
            "the Flatpak bundle",
            ci(&["--suite", "flatpak"]),
        ),
        step(
            "ci-flathub",
            "the Flathub manifest, linted and built",
            ci(&["--suite", "flathub"]),
        ),
        step(
            "ci-repos",
            "the signed APT and DNF repositories, with a throwaway key",
            ci(&["--suite", "repos"]),
        ),
    ]);
    let mut windows = vec!["--distro", "windows-cross"];
    let release_dir = options
        .release_dir
        .as_ref()
        .map(|d| d.display().to_string());
    if let Some(dir) = &release_dir {
        windows.extend(["--release-dir", dir.as_str()]);
    }
    steps.push(step(
        "ci-windows",
        "the Windows build, unit tests under Wine and the installer",
        ci(&windows),
    ));
    if let Some(dir) = &options.release_dir {
        steps.push(step(
            "checksums",
            "SHA256SUMS of the release folder",
            Action::Checksums(dir.join(version)),
        ));
    }
    if let Some(dir) = &options.repos {
        steps.push(step(
            "signatures",
            "the repositories' signatures, checked with their public key",
            Action::Signatures(dir.clone()),
        ));
    }
    steps
}

/// The steps chosen with `--only` and `--skip`. Unknown names are errors.
fn chosen_steps(options: &Options, version: &str) -> Result<Vec<Step>> {
    let steps = all_steps(options, version);
    let known = |name: &String| {
        steps.iter().any(|s| &s.name == name) || matches!(name.as_str(), "checksums" | "signatures")
    };
    for name in options.only.iter().chain(&options.skip) {
        if !known(name) {
            let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
            bail!("unknown step `{name}`; the steps are: {}", names.join(", "));
        }
    }
    Ok(steps
        .into_iter()
        .filter(|s| options.only.is_empty() || options.only.contains(&s.name))
        .filter(|s| !options.skip.contains(&s.name))
        .filter(|s| !(options.allow_dirty && s.action == Action::CleanTree))
        .collect())
}

/// The checks people make by hand, for the record.
const MANUAL: &[(&str, &str)] = &[
    (
        "Manual matrix",
        "the pre-release installed and used on Ubuntu 22.04 and 24.04, Debian 12 and \
         Fedora (current and previous); GNOME and KDE Plasma, Wayland and X11; Intel, \
         AMD and NVIDIA graphics; display scaling at 100, 125, 150 and 200 % and on \
         mixed-DPI monitors; once on a machine that has never been online",
    ),
    (
        "MCP client matrix",
        "`glyphkeel mcp` set up and used from each client in docs/mcp-clients.md, \
         as that page says",
    ),
    (
        "Signing keys",
        "FLATPAK_GPG_PRIVATE_KEY and PACKAGES_GPG_PRIVATE_KEY in the repository's \
         secrets, GitHub Pages deploying from `v*` tags (docs/releasing.md)",
    ),
    (
        "Published checksums and signatures",
        "after the final release: SHA256SUMS and the attestations on the release page; \
         `apt update` and `dnf makecache` accept the repositories",
    ),
    (
        "Documentation",
        "the guide, README, CHANGELOG and AppStream notes describe this release",
    ),
];

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let Some(options) = Options::parse(args)? else {
        eprint!("{HELP}");
        return Ok(true);
    };
    let version = workspace_version(root)?;
    let steps = chosen_steps(&options, &version)?;
    if options.dry_run {
        print_plan(&steps, &version, &options);
        return Ok(true);
    }
    let started = SystemTime::now();
    let mut results = Vec::new();
    let mut failed = false;
    for step in &steps {
        if failed && !options.keep_going {
            results.push((step, Outcome::NotRun, 0));
            continue;
        }
        eprintln!("==> release-check: {} ({})", step.name, step.command());
        let clock = Instant::now();
        let outcome = match run_step(root, step) {
            Ok(true) => Outcome::Passed,
            Ok(false) => Outcome::Failed(String::new()),
            Err(err) => Outcome::Failed(format!("{err:#}")),
        };
        let seconds = clock.elapsed().as_secs();
        if let Outcome::Failed(why) = &outcome {
            eprintln!("==> release-check: {} FAILED {why}", step.name);
            failed = true;
        }
        results.push((step, outcome, seconds));
    }
    let record = options.record.clone().unwrap_or_else(|| {
        root.join("target/release-check")
            .join(format!("release-check-{version}.md"))
    });
    let text = record_text(root, &version, started, &options, &results);
    if let Some(parent) = record.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    fs::write(&record, text).with_context(|| format!("cannot write {}", record.display()))?;
    eprintln!(
        "release-check: {} — record in {}",
        if failed { "FAILED" } else { "all steps passed" },
        record.display()
    );
    Ok(!failed)
}

fn print_plan(steps: &[Step], version: &str, options: &Options) {
    eprintln!(
        "release-check for {version}: {} steps, in this order",
        steps.len()
    );
    for (i, step) in steps.iter().enumerate() {
        eprintln!(
            "{:>3}. {:<18} {}{}",
            i + 1,
            step.name,
            step.command(),
            if step.containers {
                format!("   [{LONG}]")
            } else {
                String::new()
            }
        );
        eprintln!("      {}", step.about);
    }
    if options.release_dir.is_none() {
        eprintln!("  (checksums: skipped, give --release-dir)");
    }
    if options.repos.is_none() {
        eprintln!("  (signatures: skipped, give --repos)");
    }
    eprintln!("Then by hand:");
    for (item, how) in MANUAL {
        eprintln!("  - {item}: {how}");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    Failed(String),
    NotRun,
}

fn run_step(root: &Path, step: &Step) -> Result<bool> {
    match &step.action {
        Action::CleanTree => {
            let out = capture(root, "git", &["status", "--porcelain"])?;
            if out.trim().is_empty() {
                Ok(true)
            } else {
                eprintln!("{out}");
                bail!("the working tree has changes; commit them or pass --allow-dirty")
            }
        }
        Action::Versions => check_versions(root),
        Action::Xtask(args) => {
            let exe = std::env::current_exe().context("cannot find the xtask program")?;
            crate::ci::status(Command::new(exe).args(args).current_dir(root))
        }
        Action::Program(program, args) => {
            let program = if program == "cargo" {
                crate::cargo()
            } else {
                program.clone()
            };
            crate::ci::status(Command::new(program).args(args).current_dir(root))
        }
        Action::Checksums(dir) => write_checksums(dir),
        Action::Signatures(dir) => verify_signatures(dir),
    }
}

fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .stderr(Stdio::null())
        .output()
        .with_context(|| format!("cannot run {program}"))?;
    if !out.status.success() {
        bail!("`{program} {}` failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn first_line(root: &Path, program: &str, args: &[&str]) -> String {
    capture(root, program, args)
        .ok()
        .and_then(|s| s.lines().next().map(str::to_owned))
        .unwrap_or_else(|| "not found".to_owned())
}

fn record_text(
    root: &Path,
    version: &str,
    started: SystemTime,
    options: &Options,
    results: &[(&Step, Outcome, u64)],
) -> String {
    let commit = first_line(root, "git", &["rev-parse", "HEAD"]);
    let branch = first_line(root, "git", &["rev-parse", "--abbrev-ref", "HEAD"]);
    let dirty = capture(root, "git", &["status", "--porcelain"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(true);
    let mut out = String::new();
    let _ = writeln!(out, "# Release check: Glyphkeel {version}\n");
    let _ = writeln!(out, "- Started: {}", utc_text(started));
    let _ = writeln!(
        out,
        "- Commit: `{commit}` on `{branch}`{}",
        if dirty {
            " (with uncommitted changes)"
        } else {
            ""
        }
    );
    let tools: [(&str, &str, &[&str]); 6] = [
        ("rustc", "rustc", &["--version"]),
        ("cargo", "cargo", &["--version"]),
        ("node", "node", &["--version"]),
        ("pnpm", "pnpm", &["--version"]),
        ("podman", "podman", &["--version"]),
        ("docker", "docker", &["--version"]),
    ];
    for (name, program, args) in tools {
        let _ = writeln!(out, "- {name}: {}", first_line(root, program, args));
    }
    if let Some(engine) = &options.engine {
        let _ = writeln!(out, "- Container engine: {engine}");
    }
    let _ = writeln!(out, "\n## Automated steps\n");
    let _ = writeln!(out, "| Step | Command | Result | Time |");
    let _ = writeln!(out, "| --- | --- | --- | --- |");
    for (step, outcome, seconds) in results {
        let result = match outcome {
            Outcome::Passed => "passed".to_owned(),
            Outcome::Failed(why) if why.is_empty() => "**failed**".to_owned(),
            Outcome::Failed(why) => format!("**failed**: {}", why.replace('|', "\\|")),
            Outcome::NotRun => "not run".to_owned(),
        };
        let _ = writeln!(
            out,
            "| {} | `{}` | {result} | {} |",
            step.name,
            step.command().replace('|', "\\|"),
            duration_text(*seconds)
        );
    }
    let _ = writeln!(out, "\n## By hand\n");
    let _ = writeln!(out, "| Item | What | Result |");
    let _ = writeln!(out, "| --- | --- | --- |");
    for (item, how) in MANUAL {
        let _ = writeln!(out, "| {item} | {how} | to do |");
    }
    out
}

fn duration_text(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{} min {} s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds} s")
    }
}

/// `YYYY-MM-DD HH:MM UTC`.
fn utc_text(at: SystemTime) -> String {
    let secs = at.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (y, m, d) = civil_from_days(days);
    let rest = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rest / 3600,
        (rest % 3600) / 60
    )
}

/// The calendar date of a day number since 1970-01-01 (Howard Hinnant's
/// algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

// Versions ------------------------------------------------------------------

/// The `[workspace.package]` version of the root Cargo.toml.
pub(crate) fn workspace_version(root: &Path) -> Result<String> {
    let text = read(root, "Cargo.toml")?;
    let manifest: toml::Table = text.parse().context("Cargo.toml is not valid TOML")?;
    manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("Cargo.toml has no [workspace.package] version")
}

fn read(root: &Path, file: &str) -> Result<String> {
    fs::read_to_string(root.join(file)).with_context(|| format!("cannot read {file}"))
}

/// `cargo xtask check-versions`.
pub(crate) fn check_versions(root: &Path) -> Result<bool> {
    let version = workspace_version(root)?;
    let mut found: Vec<(String, Option<String>)> = Vec::new();
    for lock in ["Cargo.lock", "fuzz/Cargo.lock"] {
        if root.join(lock).is_file() {
            for (name, v) in lock_versions(&read(root, lock)?) {
                found.push((format!("{lock}: {name}"), Some(v)));
            }
        }
    }
    found.push((
        "ui/package.json".to_owned(),
        json_version(&read(root, "ui/package.json")?),
    ));
    let tauri = "crates/gk-app/tauri.conf.json";
    if let Some(v) = json_version(&read(root, tauri)?) {
        // Without a version of its own it takes Cargo's.
        found.push((tauri.to_owned(), Some(v)));
    }
    found.push((
        "CHANGELOG.md (top release)".to_owned(),
        changelog_version(&read(root, "CHANGELOG.md")?),
    ));
    let metainfo = "crates/gk-app/packaging/io.github.sorrelfox.Glyphkeel.metainfo.xml";
    found.push((
        format!("{metainfo} (top release)"),
        metainfo_version(&read(root, metainfo)?),
    ));
    let pkgbuild = "crates/gk-app/packaging/arch/PKGBUILD";
    found.push((
        pkgbuild.to_owned(),
        pkgbuild_version(&read(root, pkgbuild)?),
    ));

    let problems = version_problems(&version, &found);
    for problem in &problems {
        eprintln!("check-versions: {problem}");
    }
    if problems.is_empty() {
        eprintln!(
            "check-versions: {version} in Cargo.toml and {} other places",
            found.len()
        );
    }
    Ok(problems.is_empty())
}

fn version_problems(version: &str, found: &[(String, Option<String>)]) -> Vec<String> {
    found
        .iter()
        .filter_map(|(place, v)| match v {
            Some(v) if v == version => None,
            Some(v) => Some(format!("{place} says {v}, Cargo.toml says {version}")),
            None => Some(format!("{place} names no version")),
        })
        .collect()
}

/// The versions of this repository's own packages in a Cargo.lock: the
/// `gk-*` crates and xtask, which have no `source` (they are paths).
fn lock_versions(lock: &str) -> Vec<(String, String)> {
    let Ok(table) = lock.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?;
            // The fuzz targets have a version of their own.
            let own = (name.starts_with("gk-") || name == "xtask")
                && name != "gk-fuzz"
                && p.get("source").is_none();
            let version = p.get("version")?.as_str()?;
            own.then(|| (name.to_owned(), version.to_owned()))
        })
        .collect()
}

fn json_version(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("version")?.as_str().map(str::to_owned)
}

/// The first `## [X.Y.Z]` heading, after `## [Unreleased]`.
fn changelog_version(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("## [")?;
        let (version, _) = rest.split_once(']')?;
        crate::samples::parse_version(version).map(|_| version.to_owned())
    })
}

/// The first `<release version="…">` of an AppStream file, the newest.
fn metainfo_version(text: &str) -> Option<String> {
    let start = text.find("<release ")?;
    let tag = &text[start..];
    let rest = &tag[tag.find("version=\"")? + 9..];
    Some(rest[..rest.find('"')?].to_owned())
}

fn pkgbuild_version(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("pkgver="))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_owned())
}

// Checksums and signatures ---------------------------------------------------

/// Writes `dir/SHA256SUMS` for every file under `dir` (paths relative to it,
/// sorted) and checks it with `sha256sum -c`.
fn write_checksums(dir: &Path) -> Result<bool> {
    let files = release_files(dir)?;
    if files.is_empty() {
        bail!("{} holds no files", dir.display());
    }
    let out = Command::new("sha256sum")
        .arg("--")
        .args(&files)
        .current_dir(dir)
        .output()
        .context("cannot run sha256sum")?;
    if !out.status.success() {
        bail!("sha256sum failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let sums = dir.join("SHA256SUMS");
    fs::write(&sums, &out.stdout).with_context(|| format!("cannot write {}", sums.display()))?;
    eprintln!("checksums: {} files in {}", files.len(), sums.display());
    crate::ci::status(
        Command::new("sha256sum")
            .args(["--check", "--quiet", "SHA256SUMS"])
            .current_dir(dir),
    )
}

/// The files under `dir` as relative paths, sorted, without SHA256SUMS.
fn release_files(dir: &Path) -> Result<Vec<String>> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else if path.is_file() {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                let rel = rel.to_string_lossy().replace('\\', "/");
                if rel != "SHA256SUMS" {
                    out.push(rel);
                }
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// Checks the signatures of the repositories in `dir` (as `cargo xtask
/// repos` writes them) with their own public key, in a throwaway keyring.
fn verify_signatures(dir: &Path) -> Result<bool> {
    let key = dir.join("rpm/glyphkeel.asc");
    if !key.is_file() {
        bail!("{} is missing; build the repositories first", key.display());
    }
    let home = std::env::temp_dir().join(format!("gk-release-check-{}", std::process::id()));
    fs::create_dir_all(home.join("gnupg"))?;
    let gnupg = home.join("gnupg");
    let result = (|| -> Result<bool> {
        let gpg = |args: &[&Path]| -> Result<bool> {
            crate::ci::status(
                Command::new("gpg")
                    .arg("--batch")
                    .arg("--homedir")
                    .arg(&gnupg)
                    .args(args),
            )
        };
        let mut ok = gpg(&[Path::new("--import"), &key])?;
        let dist = dir.join("apt/dists/stable");
        ok &= gpg(&[Path::new("--verify"), &dist.join("InRelease")])?;
        ok &= gpg(&[
            Path::new("--verify"),
            &dist.join("Release.gpg"),
            &dist.join("Release"),
        ])?;
        let repodata = dir.join("rpm/repodata");
        ok &= gpg(&[
            Path::new("--verify"),
            &repodata.join("repomd.xml.asc"),
            &repodata.join("repomd.xml"),
        ])?;
        let rpms: Vec<PathBuf> = fs::read_dir(dir.join("rpm"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "rpm"))
            .collect();
        if rpms.is_empty() {
            bail!("{} holds no rpm", dir.join("rpm").display());
        }
        let db = home.join("rpmdb");
        fs::create_dir_all(&db)?;
        ok &= crate::ci::status(
            Command::new("rpmkeys")
                .arg("--dbpath")
                .arg(&db)
                .arg("--import")
                .arg(&key),
        )?;
        ok &= crate::ci::status(
            Command::new("rpmkeys")
                .arg("--dbpath")
                .arg(&db)
                .arg("--checksig")
                .args(&rpms),
        )?;
        Ok(ok)
    })();
    let _ = fs::remove_dir_all(&home);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        Options::parse(&args.iter().map(|&s| s.to_owned()).collect::<Vec<_>>())
            .unwrap()
            .unwrap()
    }

    fn names(steps: &[Step]) -> Vec<&str> {
        steps.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn the_plan_runs_quick_checks_first_and_reuses_the_ci_runs() {
        let steps = chosen_steps(&parse(&[]), "1.0.0").unwrap();
        let names = names(&steps);
        assert_eq!(names[..3], ["clean-tree", "versions", "migration-samples"]);
        let first_container = steps.iter().position(|s| s.containers).unwrap();
        assert!(
            steps[..first_container].iter().all(|s| !s.containers)
                && steps[first_container..].iter().all(|s| s.containers),
            "every container run comes after the quick checks"
        );
        for distro in ["ubuntu-22.04", "ubuntu-24.04", "debian-12", "fedora"] {
            assert!(names.contains(&format!("ci-{distro}").as_str()));
        }
        for name in [
            "ci-windows",
            "ci-arch",
            "ci-packages",
            "ci-repos",
            "gen",
            "notices",
        ] {
            assert!(names.contains(&name), "{name}");
        }
        assert!(!names.contains(&"checksums") && !names.contains(&"signatures"));
        let windows = steps.iter().find(|s| s.name == "ci-windows").unwrap();
        assert_eq!(windows.command(), "cargo xtask ci --distro windows-cross");
    }

    #[test]
    fn options_reach_the_ci_runs_and_add_steps() {
        let options = parse(&[
            "--engine",
            "docker",
            "--cache-dir=/c",
            "--release-dir",
            "/r",
            "--repos",
            "/p",
        ]);
        let steps = chosen_steps(&options, "1.0.0").unwrap();
        let fedora = steps.iter().find(|s| s.name == "ci-fedora").unwrap();
        assert_eq!(
            fedora.command(),
            "cargo xtask ci --distro fedora --engine docker --cache-dir /c"
        );
        let windows = steps.iter().find(|s| s.name == "ci-windows").unwrap();
        assert!(windows.command().contains("--release-dir /r"));
        let sums = steps.iter().find(|s| s.name == "checksums").unwrap();
        assert_eq!(sums.action, Action::Checksums(PathBuf::from("/r/1.0.0")));
        assert_eq!(
            names(&steps).last(),
            Some(&"signatures"),
            "signatures come last"
        );
    }

    #[test]
    fn only_and_skip_choose_steps_by_name() {
        let steps = chosen_steps(&parse(&["--only", "versions,fmt"]), "1.0.0").unwrap();
        assert_eq!(names(&steps), ["versions", "fmt"]);
        let steps = chosen_steps(&parse(&["--skip", "ci-perf", "--skip=ci-pseudo"]), "1").unwrap();
        assert!(!names(&steps).contains(&"ci-perf") && !names(&steps).contains(&"ci-pseudo"));
        let steps = chosen_steps(&parse(&["--allow-dirty"]), "1").unwrap();
        assert!(!names(&steps).contains(&"clean-tree"));
        assert!(chosen_steps(&parse(&["--only", "nonsense"]), "1").is_err());
        // Named although skipped for lack of a folder: not an error.
        assert!(chosen_steps(&parse(&["--skip", "checksums"]), "1").is_ok());
        assert!(Options::parse(&["--bogus".to_owned()]).is_err());
        assert!(Options::parse(&["--engine".to_owned()]).is_err());
    }

    #[test]
    fn versions_are_read_from_each_file() {
        let lock = r#"
version = 4

[[package]]
name = "gk-core"
version = "1.2.3"

[[package]]
name = "gk-fancy"
version = "0.9.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "xtask"
version = "1.2.3"
"#;
        assert_eq!(
            lock_versions(lock),
            [
                ("gk-core".to_owned(), "1.2.3".to_owned()),
                ("xtask".to_owned(), "1.2.3".to_owned())
            ]
        );
        assert_eq!(
            json_version(r#"{"name":"ui","version":"1.2.3"}"#).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(json_version(r#"{"productName":"x"}"#), None);
        let changelog = "# Changelog\n\n## [Unreleased]\n\n## [1.2.3] - 2026-11-27\n\n## [1.2.2]\n";
        assert_eq!(changelog_version(changelog).as_deref(), Some("1.2.3"));
        let metainfo = "<releases>\n    <release version=\"1.2.3\" date=\"2026-11-27\">\n\
                        <release version=\"1.2.2\" date=\"x\">";
        assert_eq!(metainfo_version(metainfo).as_deref(), Some("1.2.3"));
        assert_eq!(
            pkgbuild_version("pkgname=glyphkeel\npkgver=1.2.3\npkgrel=1\n").as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn a_version_that_differs_or_is_missing_is_a_problem() {
        let found = [
            ("a".to_owned(), Some("1.0.0".to_owned())),
            ("b".to_owned(), Some("0.9.0".to_owned())),
            ("c".to_owned(), None),
        ];
        let problems = version_problems("1.0.0", &found);
        assert_eq!(
            problems,
            ["b says 0.9.0, Cargo.toml says 1.0.0", "c names no version"]
        );
    }

    #[test]
    fn this_checkout_agrees_with_itself() {
        let root = crate::workspace_root();
        assert!(check_versions(&root).unwrap());
    }

    #[test]
    fn dates_are_utc_calendar_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_419), (2025, 11, 27));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        let at = UNIX_EPOCH + std::time::Duration::from_secs(1_795_428_000);
        assert_eq!(utc_text(at), "2026-11-23 10:00 UTC");
        assert_eq!(duration_text(59), "59 s");
        assert_eq!(duration_text(125), "2 min 5 s");
    }

    #[test]
    fn checksums_cover_every_file_but_themselves() {
        let dir = std::env::temp_dir().join(format!(
            "gk-xtask-sums-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("windows")).unwrap();
        fs::write(dir.join("b.rpm"), "rpm").unwrap();
        fs::write(dir.join("windows/a.exe"), "exe").unwrap();
        fs::write(dir.join("SHA256SUMS"), "old").unwrap();
        assert_eq!(release_files(&dir).unwrap(), ["b.rpm", "windows/a.exe"]);
        assert!(write_checksums(&dir).unwrap());
        let sums = fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
        assert_eq!(sums.lines().count(), 2);
        assert!(sums.contains("  windows/a.exe\n"));
        fs::write(dir.join("b.rpm"), "changed").unwrap();
        assert!(
            !crate::ci::status(
                Command::new("sha256sum")
                    .args(["--check", "--quiet", "SHA256SUMS"])
                    .current_dir(&dir)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
            )
            .unwrap()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_record_lists_every_step_and_the_manual_checks() {
        let options = parse(&[]);
        let steps = chosen_steps(&options, "1.0.0").unwrap();
        let results: Vec<(&Step, Outcome, u64)> = vec![
            (&steps[0], Outcome::Passed, 1),
            (&steps[1], Outcome::Failed("a | b".to_owned()), 61),
            (&steps[2], Outcome::NotRun, 0),
        ];
        let text = record_text(
            &crate::workspace_root(),
            "1.0.0",
            UNIX_EPOCH,
            &options,
            &results,
        );
        assert!(text.starts_with("# Release check: Glyphkeel 1.0.0\n"));
        assert!(text.contains("- Started: 1970-01-01 00:00 UTC"));
        assert!(text.contains("| clean-tree | `git status --porcelain` | passed | 1 s |"));
        assert!(text.contains("**failed**: a \\| b | 1 min 1 s |"));
        assert!(text.contains("| not run |"));
        for (item, _) in MANUAL {
            assert!(text.contains(&format!("| {item} |")), "{item}");
        }
    }
}
