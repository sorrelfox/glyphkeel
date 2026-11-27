// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask migration-samples`: workspaces written by earlier releases,
//! for the tests that upgrade them (`crates/gk-store/tests/migration_samples.rs`
//! and `crates/gk-engine/tests/migration_samples.rs`).
//!
//! A sample is made by running a released `glyphkeel` program: a fixed set
//! of commands (`SCENARIO`, each from the release that has it) builds a
//! small workspace in a throwaway home folder, and the store's test tool
//! dumps its database as SQL to `crates/gk-store/tests/samples/<version>.sql`.
//! The program comes from a release package (`--binary`), or is built from
//! the release's commit (`--commit`).
//!
//! `--check` finds the releases in the history (`chore(release): X.Y.Z`
//! commits, plus the version being made) and fails when a schema version
//! one of them shipped has no sample, or when a migration that a release
//! shipped has changed since.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

const HELP: &str = "\
Usage: cargo xtask migration-samples --check
       cargo xtask migration-samples --binary FILE [--force]
       cargo xtask migration-samples --commit REV [--force]

Workspaces written by earlier releases, kept as SQL in
crates/gk-store/tests/samples/<version>.sql for the upgrade tests.

Options:
  --check        Fail when a schema version that a release shipped has no
                 sample, or when a migration a release shipped has changed.
                 Releases are the `chore(release): X.Y.Z` commits and the
                 workspace version.
  --binary FILE  Make the sample of the release whose `glyphkeel` program
                 FILE is (for example /usr/bin/glyphkeel from its package).
                 It runs in a throwaway home folder and never touches yours.
  --commit REV   Build `glyphkeel` from commit REV (a release's commit or
                 tag) in target/migration-samples/ and make its sample.
  --force        Replace a sample that exists. Samples stand for the
                 workspaces people have, so replace one only when it was
                 made wrongly.
";

/// Where the samples live, from the workspace root.
pub(crate) const SAMPLES_DIR: &str = "crates/gk-store/tests/samples";
/// Where the migrations live, from the workspace root.
const MIGRATIONS_DIR: &str = "crates/gk-store/migrations";
/// The subject of a release commit, before the version.
const RELEASE_SUBJECT: &str = "chore(release): ";

/// One command of the scenario: the release that first has it and the
/// arguments after `glyphkeel`.
struct Step {
    since: &'static str,
    args: &'static [&'static str],
}

const fn step(since: &'static str, args: &'static [&'static str]) -> Step {
    Step { since, args }
}

/// What every sample holds, as far as its release can: two projects, a
/// label, tasks with a subtask, a dependency, an edited description, a
/// comment, logged time, a reminder, a closed task and a running timer;
/// then wiki pages, the wiki's home page and a template, and versions,
/// custom fields, kinds of work, an epic type and a recurring task. The
/// engine's upgrade test checks these by name.
const SCENARIO: &[Step] = &[
    step(
        "0.0.4",
        &[
            "project",
            "create",
            "--key",
            "TM",
            "--name",
            "Home tasks",
            "--description",
            "Things to do at home and in the garden.",
            "--hue",
            "green",
            "--icon",
            "rocket",
        ],
    ),
    step(
        "0.0.4",
        &[
            "project",
            "create",
            "--key",
            "ACM",
            "--name",
            "Acme website",
            "--hue",
            "blue",
        ],
    ),
    step(
        "0.0.4",
        &[
            "label",
            "add",
            "--project",
            "TM",
            "--name",
            "Urgent",
            "--hue",
            "red",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task",
            "create",
            "--project",
            "TM",
            "--title",
            "Plan the garden",
            "--description",
            "Beds, paths and a pond.\n\n- [ ] measure\n- [x] sketch",
            "--due-date",
            "2026-12-01",
            "--priority",
            "high",
            "--labels",
            "Urgent",
            "--estimate",
            "2h",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task",
            "create",
            "--project",
            "TM",
            "--title",
            "Buy seeds",
            "--parent",
            "TM-1",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task",
            "create",
            "--project",
            "TM",
            "--title",
            "Fix the fence",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task",
            "create",
            "--project",
            "ACM",
            "--title",
            "Launch the homepage",
            "--description",
            "Ship it **soon**.",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task", "link", "TM-3", "--kind", "blocks", "--other", "TM-1",
        ],
    ),
    step(
        "0.0.4",
        &[
            "task",
            "edit",
            "TM-1",
            "--description",
            "Beds, paths, a pond and a shed.\n\n- [ ] measure\n- [x] sketch",
        ],
    ),
    step(
        "0.0.4",
        &[
            "comment",
            "add",
            "TM-1",
            "--body",
            "The neighbours like the garden plan.",
        ],
    ),
    step(
        "0.0.4",
        &[
            "time",
            "log",
            "TM-1",
            "--duration",
            "1h30m",
            "--date",
            "2026-11-20",
            "--note",
            "First sketch",
        ],
    ),
    step(
        "0.0.4",
        &["reminder", "set", "TM-1", "--at", "2026-11-30 09:00"],
    ),
    step("0.0.4", &["task", "move", "TM-3", "--status", "Done"]),
    step("0.0.4", &["timer", "start", "TM-2"]),
    step(
        "0.0.6",
        &[
            "wiki",
            "new",
            "--project",
            "TM",
            "--title",
            "Garden notes",
            "--body",
            "The plan is in TM-1. Seeds are on [[Seeds]].",
        ],
    ),
    step(
        "0.0.6",
        &[
            "wiki",
            "new",
            "--project",
            "TM",
            "--title",
            "Seeds",
            "--parent",
            "Garden notes",
            "--body",
            "Tomatoes and beans.",
        ],
    ),
    step(
        "0.0.6",
        &[
            "wiki",
            "put",
            "Seeds",
            "--project",
            "TM",
            "--body",
            "Tomatoes, beans and peas.",
        ],
    ),
    step(
        "0.1.4",
        &[
            "wiki",
            "put",
            "Garden notes",
            "--project",
            "TM",
            "--home",
            "true",
        ],
    ),
    step(
        "0.1.4",
        &[
            "wiki",
            "new",
            "--project",
            "TM",
            "--title",
            "Meeting notes",
            "--body",
            "## Who\n\n## What",
        ],
    ),
    step(
        "0.1.4",
        &[
            "wiki",
            "put",
            "Meeting notes",
            "--project",
            "TM",
            "--template",
            "true",
        ],
    ),
    step(
        "0.1.6",
        &[
            "version",
            "create",
            "--project",
            "TM",
            "--name",
            "Spring",
            "--due-date",
            "2027-03-01",
        ],
    ),
    step(
        "0.1.6",
        &["task", "edit", "TM-1", "--fix-version", "Spring"],
    ),
    step(
        "0.1.6",
        &[
            "field",
            "add",
            "--project",
            "TM",
            "--name",
            "Cost",
            "--kind",
            "decimal",
        ],
    ),
    step(
        "0.1.6",
        &["task", "edit", "TM-1", "--fields", "{\"Cost\": 12.5}"],
    ),
    step("0.1.6", &["work-kind", "create", "--name", "Digging"]),
    step(
        "0.1.6",
        &[
            "time",
            "log",
            "TM-1",
            "--duration",
            "45m",
            "--date",
            "2026-11-21",
            "--work-kind",
            "Digging",
        ],
    ),
    step(
        "0.1.6",
        &[
            "type",
            "add",
            "--project",
            "TM",
            "--name",
            "Epic",
            "--glyph",
            "epic",
        ],
    ),
    step(
        "0.1.6",
        &[
            "task",
            "create",
            "--project",
            "TM",
            "--title",
            "Water the plants",
            "--due-date",
            "2026-12-02",
        ],
    ),
    step("0.1.6", &["recurrence", "set", "TM-4", "--every", "week"]),
];

#[derive(Debug, Default)]
struct Options {
    check: bool,
    binary: Option<PathBuf>,
    commit: Option<String>,
    force: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Option<Self>> {
        let mut options = Self::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--check" => options.check = true,
                "--force" => options.force = true,
                "--binary" => {
                    options.binary = Some(PathBuf::from(
                        args.next().context("`--binary` needs a file")?,
                    ));
                }
                "--commit" => {
                    options.commit =
                        Some(args.next().context("`--commit` needs a commit")?.clone());
                }
                "-h" | "--help" => return Ok(None),
                other => bail!("unknown option `{other}`\n\n{HELP}"),
            }
        }
        let modes = [
            options.check,
            options.binary.is_some(),
            options.commit.is_some(),
        ]
        .iter()
        .filter(|&&on| on)
        .count();
        if modes != 1 {
            bail!("give one of --check, --binary or --commit\n\n{HELP}");
        }
        if options.check && options.force {
            bail!("`--force` makes no sense with `--check`");
        }
        Ok(Some(options))
    }
}

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let Some(options) = Options::parse(args)? else {
        eprint!("{HELP}");
        return Ok(true);
    };
    if options.check {
        return check(root);
    }
    let binary = match (&options.binary, &options.commit) {
        (Some(binary), _) => binary.clone(),
        (None, Some(commit)) => build_at(root, commit)?,
        (None, None) => unreachable!("checked by Options::parse"),
    };
    make(root, &binary, options.force)?;
    Ok(true)
}

/// Runs the checks and prints what is wrong. Returns whether all is well.
pub(crate) fn check(root: &Path) -> Result<bool> {
    let releases = releases(root)?;
    let samples = read_samples(&root.join(SAMPLES_DIR))?;
    let mut ok = true;
    for problem in missing(&releases, &samples) {
        eprintln!("migration-samples: {problem}");
        ok = false;
    }
    for problem in changed_migrations(root, &releases)? {
        eprintln!("migration-samples: {problem}");
        ok = false;
    }
    if ok {
        let schemas: BTreeSet<i64> = releases.iter().map(|r| r.schema).collect();
        eprintln!(
            "migration-samples: {} samples cover the {} schema versions of {} releases",
            samples.len(),
            schemas.len(),
            releases.len()
        );
    }
    Ok(ok)
}

/// A release: its version, the commit that made it (none for the version
/// being made) and the schema version it shipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) commit: Option<String>,
    pub(crate) schema: i64,
}

/// The releases in the history, oldest first, then the workspace version
/// with the current schema when it isn't one of them yet.
pub(crate) fn releases(root: &Path) -> Result<Vec<Release>> {
    let log = git(root, &["log", "--format=%H %s"])?;
    let mut found = Vec::new();
    for (commit, version) in parse_release_log(&log) {
        let files = git(
            root,
            &[
                "ls-tree",
                "--name-only",
                &commit,
                &format!("{MIGRATIONS_DIR}/"),
            ],
        )?;
        let schema = files.lines().filter(|l| is_migration(l)).count();
        found.push(Release {
            version,
            commit: Some(commit),
            schema: i64::try_from(schema)?,
        });
    }
    found.reverse();
    let current = crate::release::workspace_version(root)?;
    if !found.iter().any(|r| r.version == current) {
        found.push(Release {
            version: current,
            commit: None,
            schema: i64::try_from(current_migrations(root)?.len())?,
        });
    }
    Ok(found)
}

/// `(commit, version)` of every release commit in `git log --format='%H %s'`
/// output, newest first as git lists them.
fn parse_release_log(log: &str) -> Vec<(String, String)> {
    log.lines()
        .filter_map(|line| {
            let (commit, subject) = line.split_once(' ')?;
            let version = subject.strip_prefix(RELEASE_SUBJECT)?.trim();
            parse_version(version)?;
            Some((commit.to_owned(), version.to_owned()))
        })
        .collect()
}

fn is_migration(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.len() > 5
        && name.ends_with(".sql")
        && name[..4].chars().all(|c| c.is_ascii_digit())
        && name.as_bytes()[4] == b'_'
}

/// The migration files of the working tree, by path from the root, sorted.
fn current_migrations(root: &Path) -> Result<Vec<String>> {
    let dir = root.join(MIGRATIONS_DIR);
    let mut files: Vec<String> = fs::read_dir(&dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_migration(name))
        .map(|name| format!("{MIGRATIONS_DIR}/{name}"))
        .collect();
    files.sort();
    Ok(files)
}

/// A sample's header: the release that wrote it and its schema version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SampleInfo {
    pub(crate) release: String,
    pub(crate) schema: i64,
}

fn read_samples(dir: &Path) -> Result<Vec<SampleInfo>> {
    let mut samples = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(samples);
    };
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "sql") {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        samples.push(
            parse_header(&text)
                .with_context(|| format!("{} has no sample header", path.display()))?,
        );
    }
    samples.sort_by_key(|s| parse_version(&s.release));
    Ok(samples)
}

fn parse_header(text: &str) -> Option<SampleInfo> {
    let field = |name: &str| {
        text.lines()
            .take(10)
            .find_map(|line| line.strip_prefix(&format!("-- {name}: ")))
            .map(str::trim)
    };
    Some(SampleInfo {
        release: field("release")?.to_owned(),
        schema: field("schema")?.parse().ok()?,
    })
}

/// What is missing: every schema version a release shipped needs a sample.
fn missing(releases: &[Release], samples: &[SampleInfo]) -> Vec<String> {
    let covered: BTreeSet<i64> = samples.iter().map(|s| s.schema).collect();
    let mut by_schema: BTreeMap<i64, Vec<&Release>> = BTreeMap::new();
    for release in releases {
        by_schema.entry(release.schema).or_default().push(release);
    }
    by_schema
        .into_iter()
        .filter(|(schema, _)| !covered.contains(schema))
        .map(|(schema, shipped)| {
            let versions: Vec<&str> = shipped.iter().map(|r| r.version.as_str()).collect();
            let how = match shipped.iter().find_map(|r| r.commit.as_deref()) {
                Some(commit) => format!(
                    "`cargo xtask migration-samples --binary <its glyphkeel>` or `--commit {}`",
                    &commit[..commit.len().min(12)]
                ),
                None => "`cargo xtask migration-samples --binary target/debug/glyphkeel` \
                         after `cargo build -p gk-cli`"
                    .to_owned(),
            };
            format!(
                "schema version {schema} (shipped by {}) has no sample; make one with {how}",
                versions.join(", ")
            )
        })
        .collect()
}

/// Migrations that changed after a release shipped them. Migrations run
/// once per workspace, so a released one must never change.
fn changed_migrations(root: &Path, releases: &[Release]) -> Result<Vec<String>> {
    let current = current_migrations(root)?;
    let mut args = vec!["hash-object", "--"];
    args.extend(current.iter().map(String::as_str));
    let hashes = git(root, &args)?;
    let now: BTreeMap<&str, &str> = current
        .iter()
        .map(String::as_str)
        .zip(hashes.lines())
        .collect();
    let mut problems = BTreeSet::new();
    for release in releases {
        let Some(commit) = &release.commit else {
            continue;
        };
        let tree = git(root, &["ls-tree", commit, &format!("{MIGRATIONS_DIR}/")])?;
        for (path, blob) in parse_ls_tree(&tree) {
            if !is_migration(&path) {
                continue;
            }
            match now.get(path.as_str()) {
                Some(hash) if *hash == blob => {}
                Some(_) => {
                    problems.insert(format!(
                        "{path} changed after release {} shipped it; add a new migration instead",
                        release.version
                    ));
                }
                None => {
                    problems.insert(format!(
                        "{path} was removed after release {} shipped it",
                        release.version
                    ));
                }
            }
        }
    }
    Ok(problems.into_iter().collect())
}

/// `(path, blob hash)` from `git ls-tree` output.
fn parse_ls_tree(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| {
            let (meta, path) = line.split_once('\t')?;
            let mut parts = meta.split_whitespace();
            let (_mode, kind, hash) = (parts.next()?, parts.next()?, parts.next()?);
            (kind == "blob").then(|| (path.to_owned(), hash.to_owned()))
        })
        .collect()
}

/// `X.Y.Z` as numbers, for ordering and for the scenario's `since`.
pub(crate) fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}

/// The steps of the scenario that release `version` can run.
fn steps_for(version: (u64, u64, u64)) -> impl Iterator<Item = &'static Step> {
    SCENARIO
        .iter()
        .filter(move |step| parse_version(step.since).is_some_and(|since| since <= version))
}

/// Builds `glyphkeel` from `commit` and returns the program's path.
fn build_at(root: &Path, commit: &str) -> Result<PathBuf> {
    let full = git(
        root,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    let full = full.trim();
    let base = root.join("target/migration-samples");
    let src = base.join(format!("src-{}", &full[..12]));
    if !src.join("Cargo.toml").is_file() {
        fs::create_dir_all(&src).with_context(|| format!("cannot create {}", src.display()))?;
        // `git archive` reads the commit without touching the repository.
        let mut archive = Command::new("git")
            .args(["archive", "--format=tar", full])
            .current_dir(root)
            .stdout(Stdio::piped())
            .spawn()
            .context("cannot run git archive")?;
        let tar_in = archive
            .stdout
            .take()
            .context("no output from git archive")?;
        let tar = Command::new("tar")
            .arg("-x")
            .arg("-C")
            .arg(&src)
            .stdin(tar_in)
            .status()
            .context("cannot run tar")?;
        if !archive.wait()?.success() || !tar.success() {
            bail!("cannot unpack commit {commit} into {}", src.display());
        }
    }
    let target = base.join("target");
    eprintln!("migration-samples: building glyphkeel at {commit} (a few minutes)");
    let status = Command::new(crate::cargo())
        .args(["build", "--package", "gk-cli", "--locked"])
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(&src)
        .status()
        .context("cannot run cargo")?;
    if !status.success() {
        bail!("building glyphkeel at {commit} failed");
    }
    Ok(target.join("debug/glyphkeel"))
}

/// Makes the sample of the release whose program is `binary`.
fn make(root: &Path, binary: &Path, force: bool) -> Result<()> {
    let binary =
        fs::canonicalize(binary).with_context(|| format!("cannot find {}", binary.display()))?;
    let out = Command::new(&binary)
        .arg("--version")
        .env("NO_COLOR", "1")
        .output()
        .with_context(|| format!("cannot run {}", binary.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let version_text = text
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_owned();
    let version = parse_version(&version_text)
        .with_context(|| format!("{} printed no version: {text}", binary.display()))?;
    let sample = root.join(SAMPLES_DIR).join(format!("{version_text}.sql"));
    if sample.exists() && !force {
        bail!(
            "{} exists; samples are never regenerated (use --force only to replace a wrong one)",
            sample.display()
        );
    }

    if let Some(dir) = sample.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let work = root.join("target/migration-samples").join(&version_text);
    if work.exists() {
        fs::remove_dir_all(&work).with_context(|| format!("cannot clear {}", work.display()))?;
    }
    let home = work.join("home");
    let workspace = work.join("workspace");
    fs::create_dir_all(&home).with_context(|| format!("cannot create {}", home.display()))?;

    eprintln!("migration-samples: glyphkeel {version_text}");
    for step in steps_for(version) {
        let mut cmd = Command::new(&binary);
        cmd.args(step.args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("XDG_DATA_HOME", home.join("data"))
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_RUNTIME_DIR", home.join("runtime"))
            .env("GLYPHKEEL_WORKSPACE", &workspace)
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null());
        let out = cmd
            .output()
            .with_context(|| format!("cannot run {}", binary.display()))?;
        if !out.status.success() {
            bail!(
                "`glyphkeel {}` failed:\n{}",
                step.args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        eprintln!("  ok  glyphkeel {}", step.args[..2].join(" "));
    }

    let database = workspace.join("workspace.db");
    if !database.is_file() {
        bail!(
            "glyphkeel {version_text} left no database in {}",
            workspace.display()
        );
    }
    let status = Command::new(crate::cargo())
        .args([
            "test",
            "--package",
            "gk-store",
            "--test",
            "migration_samples",
            "--",
            "--ignored",
            "--exact",
            "write_sample",
        ])
        .env("GK_SAMPLE_DB", &database)
        .env("GK_SAMPLE_OUT", &sample)
        .env("GK_SAMPLE_RELEASE", &version_text)
        .current_dir(root)
        .status()
        .context("cannot run cargo")?;
    if !status.success() || !sample.is_file() {
        bail!("dumping the workspace of glyphkeel {version_text} failed");
    }
    eprintln!("migration-samples: wrote {}", sample.display());
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("cannot run git")?;
    if !out.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str, schema: i64, commit: Option<&str>) -> Release {
        Release {
            version: version.to_owned(),
            commit: commit.map(str::to_owned),
            schema,
        }
    }

    #[test]
    fn options_take_one_mode() {
        let parse =
            |args: &[&str]| Options::parse(&args.iter().map(|&s| s.to_owned()).collect::<Vec<_>>());
        assert!(parse(&["--check"]).unwrap().unwrap().check);
        let options = parse(&["--binary", "/usr/bin/glyphkeel", "--force"])
            .unwrap()
            .unwrap();
        assert_eq!(options.binary.unwrap(), Path::new("/usr/bin/glyphkeel"));
        assert!(options.force);
        assert_eq!(
            parse(&["--commit", "v0.1.0"])
                .unwrap()
                .unwrap()
                .commit
                .unwrap(),
            "v0.1.0"
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&["--check", "--commit", "x"]).is_err());
        assert!(parse(&["--check", "--force"]).is_err());
        assert!(parse(&["--binary"]).is_err());
        assert!(parse(&["--help"]).unwrap().is_none());
    }

    #[test]
    fn release_commits_are_found_by_their_subject() {
        let log = "aaa chore(release): 0.1.6\n\
                   bbb chore(release): 0.1.5 \n\
                   ccc chore(release): bump things\n\
                   ddd feat: chore(release): 9.9.9\n";
        assert_eq!(
            parse_release_log(log),
            [
                ("aaa".to_owned(), "0.1.6".to_owned()),
                ("bbb".to_owned(), "0.1.5".to_owned())
            ]
        );
    }

    #[test]
    fn migrations_are_numbered_sql_files() {
        assert!(is_migration("crates/gk-store/migrations/0001_initial.sql"));
        assert!(is_migration("0016_epic_and_story_glyphs.sql"));
        assert!(!is_migration("README.md"));
        assert!(!is_migration("01_short.sql"));
        assert!(!is_migration("0001-initial.sql"));
    }

    #[test]
    fn versions_order_by_number() {
        assert_eq!(parse_version("0.1.10"), Some((0, 1, 10)));
        assert!(parse_version("0.1.10") > parse_version("0.1.9"));
        assert_eq!(parse_version("0.1"), None);
        assert_eq!(parse_version("0.1.0-rc.1"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
    }

    #[test]
    fn the_scenario_grows_with_the_release() {
        let first = steps_for((0, 0, 4)).count();
        let wiki = steps_for((0, 0, 8)).count();
        let all = steps_for((9, 0, 0)).count();
        assert!(first > 0 && first < wiki && wiki < all);
        assert_eq!(all, SCENARIO.len());
        assert!(
            SCENARIO.iter().all(|s| parse_version(s.since).is_some()),
            "every step names a release"
        );
        assert_eq!(steps_for((0, 0, 3)).count(), 0);
        // Only commands that run on their own: none starts the app.
        assert!(SCENARIO.iter().all(|s| s.args.first() != Some(&"open")));
    }

    #[test]
    fn sample_headers_name_the_release_and_schema() {
        let text = "-- A Glyphkeel workspace written by glyphkeel 0.0.4.\n\
                    -- release: 0.0.4\n-- schema: 4\nPRAGMA foreign_keys = OFF;\n";
        assert_eq!(
            parse_header(text),
            Some(SampleInfo {
                release: "0.0.4".to_owned(),
                schema: 4
            })
        );
        assert_eq!(parse_header("CREATE TABLE x (y);"), None);
    }

    #[test]
    fn every_shipped_schema_needs_a_sample() {
        let releases = [
            release("0.0.4", 4, Some("c4c4c4c4c4c4c4c4")),
            release("0.0.5", 5, Some("c5c5c5c5c5c5c5c5")),
            release("0.0.6", 6, Some("c6")),
            release("0.0.7", 6, Some("c7")),
            release("0.2.0", 17, None),
        ];
        let samples = [
            SampleInfo {
                release: "0.0.4".to_owned(),
                schema: 4,
            },
            SampleInfo {
                release: "0.0.7".to_owned(),
                schema: 6,
            },
        ];
        let problems = missing(&releases, &samples);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("schema version 5 (shipped by 0.0.5)"));
        assert!(problems[0].contains("--commit c5c5c5c5c5c5"));
        assert!(problems[1].contains("schema version 17 (shipped by 0.2.0)"));
        assert!(problems[1].contains("target/debug/glyphkeel"));
    }

    #[test]
    fn ls_tree_gives_paths_and_blobs() {
        let listing = "100644 blob 1111\tcrates/gk-store/migrations/0001_initial.sql\n\
                       040000 tree 2222\tcrates/gk-store/migrations/sub\n";
        assert_eq!(
            parse_ls_tree(listing),
            [(
                "crates/gk-store/migrations/0001_initial.sql".to_owned(),
                "1111".to_owned()
            )]
        );
    }
}
