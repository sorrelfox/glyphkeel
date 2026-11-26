// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask ci`: the full check suites, run in containers of the
//! supported distributions, so each is checked against its own system
//! libraries (WebKitGTK, GTK, SQLite).
//!
//! On the host it builds one image per distribution from
//! `ci/containers/<distro>.Containerfile`, copies the source (the files git
//! tracks, plus untracked ones it doesn't ignore) into a fresh container and
//! runs `cargo xtask ci --here` inside it. Build caches (the
//! cargo registry, each distribution's `target/` and the pnpm store) live in
//! named volumes, so runs after the first are incremental.
//!
//! The suites `packages`, `offline`, `perf` and `flatpak` check what ships,
//! from the host (see `installed.rs`): the deb and rpm are built once in the
//! Ubuntu 22.04 image, then installed in clean containers of each
//! distribution and checked there.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

const HELP: &str = "\
Usage: cargo xtask ci [--suite S[,S…]] [--distro D[,D…]] [--engine E] [--rebuild]
                      [--cache-dir DIR] [--package-release N] [--ui-flows F[,F…]]
                      [--pseudo-locale L] [--perf-enforce]
       cargo xtask ci --here [--suite S[,S…]] [--package-release N] [--ui-flows F[,F…]]
                      [--pseudo-locale L]
       cargo xtask ci --list

Runs check suites in containers, one image per distribution.

Options:
  --suite S     Suites to run (repeatable or comma-separated). Default:
                static, rust, ui and app. See `--list`.
  --distro D    Distributions to run on (repeatable or comma-separated).
                Default: every distribution. For `packages`, `offline` and
                `perf` these are the distributions the packages are installed
                on; they are always built on ubuntu-22.04.
  --engine E    `podman` or `docker`. Default: podman when it is installed,
                docker otherwise.
  --rebuild     Rebuild the images from scratch, without the layer cache.
  --cache-dir DIR
                Keep the build caches in DIR (bind mounts) instead of named
                volumes: DIR/cargo-registry, DIR/pnpm-store and
                DIR/target-<distro>. Packages land in
                DIR/target-<distro>/dist/bundle/.
  --package-release N
                The rpm's release number, for a rebuild of the same version
                (`package` suite only). Default: 1.
  --ui-flows F  Run only these browser flows in the `ui` suite (repeatable or
                comma-separated; see `--list`). Default: every flow, one
                after another, each against a fresh workspace.
  --pseudo-locale L
                The locale of the `pseudo` screenshots. Default: en-XA.
  --perf-enforce
                Fail the `perf` suite when a result is over its budget.
                Without it results over budget are only reported. The
                suite fails either way when the app or its web view dies
                or a result is missing.
  --here        Run the suites on this machine, without containers. This is
                what runs inside each container.
  --list        List the suites and distributions.

CARGO_BUILD_JOBS and CARGO_TERM_COLOR, when set, are passed into the
containers. GLYPHKEEL_CI_PREFIX renames the images, volumes and containers
this command makes (default: glyphkeel-ci), so that several checkouts can
run side by side.
";

/// The default prefix of every image, volume and container this command
/// creates.
const PREFIX: &str = "glyphkeel-ci";

/// The prefix in use: `GLYPHKEEL_CI_PREFIX`, else [`PREFIX`].
pub(crate) fn prefix() -> String {
    std::env::var("GLYPHKEEL_CI_PREFIX")
        .ok()
        .filter(|value| {
            !value.is_empty()
                && value
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        .unwrap_or_else(|| PREFIX.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Suite {
    /// Formatting, licence headers, dependency rules, generated files,
    /// clippy, ESLint, stylelint and the TypeScript check.
    Static,
    /// Every Rust test.
    Rust,
    /// The UI's component tests, its production build (checked for network
    /// and bridge code) and every browser flow in Chromium, on sample data
    /// and against the real engine.
    Ui,
    /// The desktop app built with the UI embedded, as it ships.
    App,
    /// Licences, advisories, bans and sources of the crates (cargo-deny).
    Deny,
    /// The deb and rpm packages, built by the Tauri bundler with the
    /// release profile `dist`, and the check of the UI they embed.
    Package,
    /// The seed tool's data sets S, L and hostile, for the checks of the
    /// installed packages.
    TestData,
    /// Screenshots of the main screens in a pseudo-locale.
    Pseudo,
    /// The packages installed in clean containers: files, programs and a
    /// command-line session.
    Packages,
    /// The never-online check of the installed app, command line and MCP
    /// server.
    Offline,
    /// Performance with data set L on the installed release build.
    Perf,
    /// The Flatpak, built with flatpak-builder on the host.
    Flatpak,
}

impl Suite {
    const ALL: [Self; 12] = [
        Self::Static,
        Self::Rust,
        Self::Ui,
        Self::App,
        Self::Deny,
        Self::Package,
        Self::TestData,
        Self::Pseudo,
        Self::Packages,
        Self::Offline,
        Self::Perf,
        Self::Flatpak,
    ];
    const DEFAULT: [Self; 4] = [Self::Static, Self::Rust, Self::Ui, Self::App];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Rust => "rust",
            Self::Ui => "ui",
            Self::App => "app",
            Self::Deny => "deny",
            Self::Package => "package",
            Self::TestData => "testdata",
            Self::Pseudo => "pseudo",
            Self::Packages => "packages",
            Self::Offline => "offline",
            Self::Perf => "perf",
            Self::Flatpak => "flatpak",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Self::Static => {
                "fmt, licence headers, dependency rules, generated files, clippy, ESLint, \
                 stylelint, tsc"
            }
            Self::Rust => "cargo test for the whole workspace",
            Self::Ui => {
                "the UI's component tests, release build and bundle check, every browser \
                 flow (Chromium)"
            }
            Self::App => "the desktop app with the UI embedded",
            Self::Deny => "cargo-deny: licences, advisories, bans, sources",
            Self::Package => {
                "deb and rpm packages with the app and the command line (`dist` profile), \
                 and the UI they embed checked for remote addresses"
            }
            Self::TestData => "the seed tool's data sets S, L and hostile into target/ci-data",
            Self::Pseudo => "screenshots of the main screens in a pseudo-locale (Chromium)",
            Self::Packages => {
                "the packages installed in clean containers of each distribution: files, \
                 programs, a command-line session (from the host)"
            }
            Self::Offline => {
                "the installed app, command line and MCP server with no network, every \
                 process traced for IPv4 and IPv6 connections (from the host)"
            }
            Self::Perf => {
                "start, memory, board, list, MCP and command-line timings with data set L \
                 on the installed packages (from the host)"
            }
            Self::Flatpak => {
                "the Flatpak, built with flatpak-builder on the host (skipped without the \
                 generated source lists)"
            }
        }
    }

    /// Suites run from the host rather than inside a CI image: they need
    /// the built packages, or tools of the host.
    pub(crate) fn on_host(self) -> bool {
        matches!(
            self,
            Self::Packages | Self::Offline | Self::Perf | Self::Flatpak
        )
    }

    fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|suite| suite.name() == name)
            .with_context(|| format!("unknown suite `{name}` (see `cargo xtask ci --list`)"))
    }

    fn needs_node_modules(self) -> bool {
        matches!(
            self,
            Self::Static | Self::Ui | Self::App | Self::Package | Self::Pseudo
        )
    }

    /// The commands of this suite, in order. `package_release` is the rpm's
    /// release number, for the `package` suite; `flows` are the browser
    /// flows of the `ui` suite; `locale` is the pseudo-locale. Suites run
    /// from the host have no steps here.
    fn steps(self, package_release: Option<u32>, flows: &[Flow], locale: &str) -> Vec<Step> {
        match self {
            Self::Static => vec![
                Step::cargo(&["fmt", "--all", "--check"]),
                Step::Headers,
                Step::Deps,
                Step::GenCheck,
                Step::cargo(&[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ]),
                Step::pnpm(&["lint"]),
                Step::pnpm(&["typecheck"]),
            ],
            Self::Rust => vec![Step::cargo(&["test", "--workspace", "--locked"])],
            Self::Ui => vec![
                Step::pnpm(&["--filter", "ui", "exec", "vitest", "run"]),
                Step::pnpm(&["build"]),
                // The release bundle has no network code and no bridge.
                Step::pnpm(&["--filter", "ui", "check:bundle"]),
                // The browser flows: on sample data, and against the engine
                // through the development bridge.
                Step::Flows(flows.to_vec()),
            ],
            Self::App => vec![
                Step::pnpm(&["build"]),
                Step::cargo(&[
                    "build",
                    "--package",
                    "gk-app",
                    "--locked",
                    "--features",
                    "tauri/custom-protocol",
                ]),
            ],
            Self::Deny => vec![Step::cargo(&["deny", "--locked", "check"])],
            // The packages hold both programs: the command line is built
            // first and added to /usr/bin next to the app, where it looks
            // for the app, with its shell completions and man page, which it
            // writes itself. `bundle.active` is off in tauri.conf.json, so
            // everyday builds never bundle; this suite switches it on. The
            // UI is built by the config's `beforeBuildCommand`.
            Self::Package => vec![
                Step::cargo(&[
                    "build",
                    "--package",
                    "gk-cli",
                    "--locked",
                    "--profile",
                    "dist",
                ]),
                Step::CliFiles,
                Step::Pnpm(
                    [
                        "tauri",
                        "build",
                        "--ci",
                        "--bundles",
                        "deb,rpm",
                        "--config",
                        &package_config(package_release),
                        "--",
                        "--profile",
                        "dist",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                ),
                // The UI as it was embedded: no development transport and
                // no remote address.
                Step::pnpm(&["--filter", "ui", "check:bundle"]),
                Step::Bundle,
            ],
            Self::TestData => DATA_SETS.iter().map(|set| Step::Seed(set)).collect(),
            Self::Pseudo => vec![Step::Pseudo(locale.to_owned())],
            Self::Packages | Self::Offline | Self::Perf | Self::Flatpak => Vec::new(),
        }
    }
}

/// The seed tool's data sets the checks of the installed packages use.
pub(crate) const DATA_SETS: [&str; 3] = ["S", "L", "hostile"];

/// Where the `testdata` suite writes the data sets, relative to the
/// repository.
pub(crate) const DATA_DIR: &str = "target/ci-data";

/// Writes one data set into [`DATA_DIR`], replacing an earlier copy.
fn write_data_set(root: &Path, set: &str) -> Result<bool> {
    let out = root.join(DATA_DIR).join(set);
    if out.exists() {
        fs::remove_dir_all(&out).with_context(|| format!("cannot remove {}", out.display()))?;
    }
    status(
        Command::new(crate::cargo())
            .args(["run", "--locked", "--quiet", "--package", "gk-seed", "--"])
            .args(["--set", set, "--seed", "1", "--out"])
            .arg(&out)
            .current_dir(root),
    )
}

/// The Playwright configuration of the pseudo-locale screenshots, relative
/// to ui/. It belongs to the `pseudo` suite, not to the `ui` suite's flows.
const PSEUDO_CONFIG: &str = "playwright.pseudo.config.ts";

/// The default pseudo-locale.
const PSEUDO_LOCALE: &str = "en-XA";

/// Takes the pseudo-locale screenshots into target/screenshots/pseudo.
fn run_pseudo(root: &Path, locale: &str) -> Result<bool> {
    let shots = root.join("target/screenshots/pseudo").join(locale);
    let mut cmd = Command::new("pnpm");
    cmd.args(["--filter", "ui", "exec", "playwright", "test", "--config"])
        .arg(PSEUDO_CONFIG)
        .arg("--project=chromium")
        .arg("--output")
        .arg(root.join("target/playwright/pseudo"))
        .env("GK_PSEUDO_LOCALE", locale)
        .env("GK_SCREENSHOT_DIR", &shots)
        .current_dir(root);
    let ok = status(&mut cmd)?;
    eprintln!("==> [pseudo] screenshots: {}", shots.display());
    Ok(ok)
}

/// Where the command line's generated files land, relative to the
/// repository: the shell completions and the man page.
const CLI_FILES_DIR: &str = "target/dist/cli-files";

/// The files `glyphkeel` writes for the packages: the file name, then the
/// arguments that print it.
const CLI_FILES: [(&str, &[&str]); 4] = [
    ("glyphkeel.bash", &["completions", "bash"]),
    ("_glyphkeel", &["completions", "zsh"]),
    ("glyphkeel.fish", &["completions", "fish"]),
    ("glyphkeel.1", &["man"]),
];

/// The Tauri configuration the `package` suite adds: bundling on, the
/// command line `glyphkeel` in /usr/bin with its completions and man page,
/// and the rpm's release number. Source paths are relative to crates/gk-app,
/// where tauri.conf.json is. zsh looks in a different folder on Debian and
/// on Fedora.
fn package_config(package_release: Option<u32>) -> String {
    let files = |zsh: &str| {
        let generated = |name: &str| format!("../../{CLI_FILES_DIR}/{name}");
        serde_json::json!({
            "/usr/bin/glyphkeel": "../../target/dist/glyphkeel",
            "/usr/share/bash-completion/completions/glyphkeel": generated("glyphkeel.bash"),
            format!("{zsh}/_glyphkeel"): generated("_glyphkeel"),
            "/usr/share/fish/vendor_completions.d/glyphkeel.fish": generated("glyphkeel.fish"),
            "/usr/share/man/man1/glyphkeel.1.gz": generated("glyphkeel.1.gz"),
        })
    };
    let mut rpm = serde_json::json!({ "files": files("/usr/share/zsh/site-functions") });
    if let Some(release) = package_release {
        rpm["release"] = release.to_string().into();
    }
    serde_json::json!({
        "bundle": {
            "active": true,
            "linux": {
                "deb": { "files": files("/usr/share/zsh/vendor-completions") },
                "rpm": rpm,
            }
        }
    })
    .to_string()
}

/// Runs the built command line to write its completions and man page into
/// [`CLI_FILES_DIR`]; the man page is compressed, as the distributions ship
/// them.
fn write_cli_files(root: &Path) -> Result<bool> {
    let program = root.join("target/dist/glyphkeel");
    let dir = root.join(CLI_FILES_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    for (name, args) in CLI_FILES {
        let output = Command::new(&program)
            .args(args)
            .output()
            .with_context(|| format!("cannot run {}", program.display()))?;
        if !output.status.success() || output.stdout.is_empty() {
            eprintln!(
                "`glyphkeel {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(false);
        }
        let file = dir.join(name);
        fs::write(&file, &output.stdout)
            .with_context(|| format!("cannot write {}", file.display()))?;
    }
    // `-n`: no name or time stamp in the header, so the file is reproducible.
    status(
        Command::new("gzip")
            .args(["-9", "-n", "-f", "glyphkeel.1"])
            .current_dir(&dir),
    )
}

/// One Playwright configuration of the UI: its flows, its servers and its
/// ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Flow {
    /// The name `--ui-flows` takes, the same as the `test:<name>` script in
    /// ui/package.json.
    name: &'static str,
    /// The configuration file, relative to ui/.
    config: &'static str,
    about: &'static str,
}

impl Flow {
    /// Every configuration, in the order the `ui` suite runs them. Several
    /// share ports (5310–5319 for pages, 5320–5329 for the bridge), so they
    /// run one after another, never side by side. Those against the engine
    /// each make a fresh workspace of their own.
    const ALL: [Self; 13] = [
        Self {
            name: "flow",
            config: "playwright.config.ts",
            about: "the release build: smoke test and themes",
        },
        Self {
            name: "board",
            config: "playwright.fake.config.ts",
            about: "project screens and preferences on sample data",
        },
        Self {
            name: "editor",
            config: "playwright.editor.config.ts",
            about: "the Markdown editor",
        },
        Self {
            name: "bridge",
            config: "playwright.bridge.config.ts",
            about: "workspace, zones, labels and toolbar against the engine",
        },
        Self {
            name: "list",
            config: "playwright.list.config.ts",
            about: "the list, against the engine and on sample data",
        },
        Self {
            name: "palette",
            config: "playwright.palette.config.ts",
            about: "command palette and quick add against the engine",
        },
        Self {
            name: "today",
            config: "playwright.today.config.ts",
            about: "Today, search and editor links against the engine",
        },
        Self {
            name: "safety",
            config: "playwright.safety.config.ts",
            about: "trash, activity, backups, export and import against the engine",
        },
        Self {
            name: "prefs",
            config: "playwright.prefs.config.ts",
            about: "preferences, first run and agent set-up against the engine",
        },
        Self {
            name: "wiki",
            config: "playwright.wiki.config.ts",
            about: "the wiki on sample data",
        },
        Self {
            name: "gantt",
            config: "playwright.gantt.config.ts",
            about: "the Gantt chart on sample data",
        },
        Self {
            name: "calendar",
            config: "playwright.calendar.config.ts",
            about: "the calendar and the time report on sample data",
        },
        Self {
            name: "planning",
            config: "playwright.planning.config.ts",
            about: "wiki, Gantt chart and calendar against the engine",
        },
    ];

    fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|flow| flow.name == name)
            .with_context(|| format!("unknown UI flow `{name}` (see `cargo xtask ci --list`)"))
    }

    /// Where Playwright writes this configuration's results and traces.
    fn output(self, root: &Path) -> PathBuf {
        root.join("target/playwright").join(self.name)
    }

    /// Runs this configuration's flows in Chromium.
    fn run(self, root: &Path) -> Result<bool> {
        let mut cmd = Command::new("pnpm");
        cmd.args(["--filter", "ui", "exec", "playwright", "test", "--config"])
            .arg(self.config)
            .arg("--project=chromium")
            // Traces of failed flows land in the build cache, which outlives
            // the container (under `--cache-dir` it is on the host).
            .arg("--output")
            .arg(self.output(root))
            .current_dir(root);
        // The configurations make a fresh workspace and token unless these
        // are set; one left in the environment would be shared by all.
        for name in FLOW_ENV_CLEARED {
            cmd.env_remove(name);
        }
        status(&mut cmd)
    }
}

/// Variables removed from the flows' environment, so that each
/// configuration starts its bridge on a new workspace.
const FLOW_ENV_CLEARED: [&str; 3] = [
    "GK_BRIDGE_WORKSPACE",
    "GK_BRIDGE_FRESH_WORKSPACE",
    "GK_BRIDGE_TOKEN",
];

/// Runs the flows one after another, all of them even when one fails, and
/// prints how each went.
fn run_flows(root: &Path, flows: &[Flow]) -> Result<bool> {
    let mut results = Vec::new();
    for &flow in flows {
        eprintln!("==> [ui] flows `{}` ({})", flow.name, flow.config);
        let started = Instant::now();
        let ok = flow.run(root)?;
        let seconds = started.elapsed().as_secs();
        eprintln!(
            "==> [ui] flows `{}`: {} in {seconds} s",
            flow.name,
            if ok { "passed" } else { "FAILED" }
        );
        results.push((flow, ok, seconds));
    }
    eprintln!("==> [ui] browser flows:");
    for (flow, ok, seconds) in &results {
        let verdict = if *ok { "passed" } else { "FAILED" };
        eprintln!("      {:<8} {verdict:<7} {seconds:>5} s", flow.name);
    }
    for (flow, ok, _) in &results {
        if !ok {
            eprintln!(
                "==> [ui] traces of `{}`: {}",
                flow.name,
                flow.output(root).display()
            );
        }
    }
    Ok(results.iter().all(|(_, ok, _)| *ok))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Distro {
    Ubuntu2204,
    Ubuntu2404,
    Debian12,
    Fedora,
}

impl Distro {
    const ALL: [Self; 4] = [
        Self::Ubuntu2204,
        Self::Ubuntu2404,
        Self::Debian12,
        Self::Fedora,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ubuntu2204 => "ubuntu-22.04",
            Self::Ubuntu2404 => "ubuntu-24.04",
            Self::Debian12 => "debian-12",
            Self::Fedora => "fedora",
        }
    }

    fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|distro| distro.name() == name)
            .with_context(|| format!("unknown distribution `{name}` (see `cargo xtask ci --list`)"))
    }

    fn image(self) -> String {
        format!("{}:{}", prefix(), self.name())
    }

    /// The distribution's own image, as users have it.
    pub(crate) fn base_image(self) -> &'static str {
        match self {
            Self::Ubuntu2204 => "docker.io/library/ubuntu:22.04",
            Self::Ubuntu2404 => "docker.io/library/ubuntu:24.04",
            Self::Debian12 => "docker.io/library/debian:12",
            Self::Fedora => "registry.fedoraproject.org/fedora:45",
        }
    }

    /// Whether the distribution installs the rpm rather than the deb.
    pub(crate) fn uses_rpm(self) -> bool {
        self == Self::Fedora
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Engine {
    Podman,
    Docker,
}

impl Engine {
    pub(crate) fn program(self) -> &'static str {
        match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        }
    }

    fn detect() -> Result<Self> {
        for engine in [Self::Podman, Self::Docker] {
            let found = Command::new(engine.program())
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success());
            if found {
                return Ok(engine);
            }
        }
        bail!("neither podman nor docker is installed")
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Cargo(Vec<String>),
    Pnpm(Vec<String>),
    /// The UI's browser flows, one configuration after another.
    Flows(Vec<Flow>),
    Headers,
    Deps,
    GenCheck,
    /// The command line's completions and man page, for the packages.
    CliFiles,
    /// The built UI checked for remote addresses.
    Bundle,
    /// One of the seed tool's data sets.
    Seed(&'static str),
    /// The pseudo-locale screenshots, in this locale.
    Pseudo(String),
}

impl Step {
    fn cargo(args: &[&str]) -> Self {
        Self::Cargo(args.iter().map(|&arg| arg.to_owned()).collect())
    }

    fn pnpm(args: &[&str]) -> Self {
        Self::Pnpm(args.iter().map(|&arg| arg.to_owned()).collect())
    }

    fn run(&self, root: &Path) -> Result<bool> {
        match self {
            Self::Cargo(args) => status(Command::new(crate::cargo()).args(args).current_dir(root)),
            Self::Pnpm(args) => status(Command::new("pnpm").args(args).current_dir(root)),
            Self::Flows(flows) => run_flows(root, flows),
            Self::Headers => crate::headers::run(root),
            Self::Deps => crate::deps::run(root),
            Self::GenCheck => crate::codegen::run(root, &["--check".to_owned()]),
            Self::CliFiles => write_cli_files(root),
            Self::Bundle => crate::bundle::run(root, &[]),
            Self::Seed(set) => write_data_set(root, set),
            Self::Pseudo(locale) => run_pseudo(root, locale),
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cargo(args) => write!(f, "cargo {}", args.join(" ")),
            Self::Pnpm(args) => write!(f, "pnpm {}", args.join(" ")),
            Self::Flows(flows) => {
                let names: Vec<&str> = flows.iter().map(|flow| flow.name).collect();
                write!(f, "playwright flows: {}", names.join(", "))
            }
            Self::Headers => f.write_str("cargo xtask check-headers"),
            Self::Deps => f.write_str("cargo xtask check-deps"),
            Self::GenCheck => f.write_str("cargo xtask gen --check"),
            Self::CliFiles => write!(f, "glyphkeel completions and man into {CLI_FILES_DIR}"),
            Self::Bundle => f.write_str("cargo xtask check-bundle"),
            Self::Seed(set) => write!(f, "gk-seed --set {set} into {DATA_DIR}/{set}"),
            Self::Pseudo(locale) => write!(f, "playwright {PSEUDO_CONFIG} in {locale}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    suites: Vec<Suite>,
    distros: Vec<Distro>,
    engine: Option<Engine>,
    rebuild: bool,
    cache_dir: Option<PathBuf>,
    /// The rpm's release number; Tauri's default (1) when not given.
    package_release: Option<u32>,
    /// The browser flows of the `ui` suite: every one unless some are named.
    ui_flows: Vec<Flow>,
    /// The locale of the `pseudo` suite.
    pseudo_locale: String,
    /// Whether the `perf` suite fails on a result over budget.
    perf_enforce: bool,
    here: bool,
    list: bool,
    help: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            suites: Vec::new(),
            distros: Vec::new(),
            engine: None,
            rebuild: false,
            cache_dir: None,
            package_release: None,
            ui_flows: Vec::new(),
            pseudo_locale: PSEUDO_LOCALE.to_owned(),
            perf_enforce: false,
            here: false,
            list: false,
            help: false,
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) => (flag, Some(value.to_owned())),
                None => (arg.as_str(), None),
            };
            let mut value = || -> Result<String> {
                match inline.clone() {
                    Some(value) => Ok(value),
                    None => args
                        .next()
                        .cloned()
                        .with_context(|| format!("`{flag}` needs a value")),
                }
            };
            match flag {
                "--suite" => {
                    for name in value()?.split(',') {
                        push_unique(&mut options.suites, Suite::parse(name.trim())?);
                    }
                }
                "--distro" => {
                    for name in value()?.split(',') {
                        push_unique(&mut options.distros, Distro::parse(name.trim())?);
                    }
                }
                "--engine" => {
                    options.engine = Some(match value()?.as_str() {
                        "podman" => Engine::Podman,
                        "docker" => Engine::Docker,
                        other => bail!("unknown engine `{other}`: use podman or docker"),
                    });
                }
                "--rebuild" => options.rebuild = true,
                "--cache-dir" => options.cache_dir = Some(PathBuf::from(value()?)),
                "--package-release" => {
                    let given = value()?;
                    match given.parse::<u32>() {
                        Ok(release) if release >= 1 => options.package_release = Some(release),
                        _ => {
                            bail!("`--package-release` needs a whole number from 1, not `{given}`")
                        }
                    }
                }
                "--ui-flows" => {
                    for name in value()?.split(',') {
                        push_unique(&mut options.ui_flows, Flow::parse(name.trim())?);
                    }
                }
                "--pseudo-locale" => {
                    let locale = value()?;
                    let valid = !locale.is_empty()
                        && locale.split('-').all(|part| {
                            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric())
                        });
                    if !valid {
                        bail!("`--pseudo-locale` needs a locale such as en-XA, not `{locale}`");
                    }
                    options.pseudo_locale = locale;
                }
                "--perf-enforce" => options.perf_enforce = true,
                "--here" => options.here = true,
                "--list" => options.list = true,
                "-h" | "--help" => options.help = true,
                other => bail!("unknown option `{other}`\n\n{HELP}"),
            }
        }
        if options.suites.is_empty() {
            options.suites = Suite::DEFAULT.to_vec();
        }
        if options.distros.is_empty() {
            options.distros = Distro::ALL.to_vec();
        }
        if options.package_release.is_some() && !options.suites.contains(&Suite::Package) {
            bail!("`--package-release` applies to the `package` suite only");
        }
        if !options.ui_flows.is_empty() && !options.suites.contains(&Suite::Ui) {
            bail!("`--ui-flows` applies to the `ui` suite only");
        }
        if options.ui_flows.is_empty() {
            options.ui_flows = Flow::ALL.to_vec();
        }
        if options.pseudo_locale != PSEUDO_LOCALE && !options.suites.contains(&Suite::Pseudo) {
            bail!("`--pseudo-locale` applies to the `pseudo` suite only");
        }
        if options.perf_enforce && !options.suites.contains(&Suite::Perf) {
            bail!("`--perf-enforce` applies to the `perf` suite only");
        }
        if options.here
            && let Some(suite) = options.suites.iter().find(|suite| suite.on_host())
        {
            bail!(
                "the `{}` suite runs from the host and needs containers; drop `--here`",
                suite.name()
            );
        }
        if options.here
            && (options.engine.is_some() || options.rebuild || options.cache_dir.is_some())
        {
            bail!(
                "`--here` runs without containers; `--engine`, `--rebuild` and \
                 `--cache-dir` don't apply"
            );
        }
        Ok(options)
    }
}

fn push_unique<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let options = Options::parse(args)?;
    if options.help {
        eprint!("{HELP}");
        return Ok(true);
    }
    if options.list {
        list();
        return Ok(true);
    }
    if options.here {
        return run_here(
            root,
            &options.suites,
            options.package_release,
            &options.ui_flows,
            &options.pseudo_locale,
        );
    }
    let engine = match options.engine {
        Some(engine) => engine,
        None => Engine::detect()?,
    };
    let pins = Pins::read(root)?;
    let (host, inside): (Vec<Suite>, Vec<Suite>) =
        options.suites.iter().partition(|suite| suite.on_host());
    let mut failed = Vec::new();
    for &distro in options.distros.iter().filter(|_| !inside.is_empty()) {
        let started = Instant::now();
        eprintln!(
            "==> {}: building the image {}",
            distro.name(),
            distro.image()
        );
        let ok = build_image(root, engine, distro, &pins, options.rebuild)?
            && run_in_container(
                root,
                engine,
                distro,
                &ContainerRun {
                    suites: &inside,
                    package_release: options.package_release,
                    flows: &options.ui_flows,
                    pseudo_locale: &options.pseudo_locale,
                    cache_dir: options.cache_dir.as_deref(),
                },
            )?;
        let seconds = started.elapsed().as_secs();
        if ok {
            eprintln!("==> {}: passed in {seconds} s", distro.name());
        } else {
            eprintln!("==> {}: FAILED after {seconds} s", distro.name());
            failed.push(distro.name().to_owned());
        }
    }
    if !host.is_empty() {
        let run = crate::installed::HostRun {
            root,
            engine,
            pins: &pins,
            distros: &options.distros,
            suites: &host,
            rebuild: options.rebuild,
            package_release: options.package_release,
            perf_enforce: options.perf_enforce,
            cache_dir: options
                .cache_dir
                .clone()
                .unwrap_or_else(|| root.join(crate::installed::DEFAULT_CACHE)),
        };
        failed.extend(run.run()?);
    }
    if failed.is_empty() {
        eprintln!("==> ci: every distribution passed");
        Ok(true)
    } else {
        eprintln!("==> ci: failed: {}", failed.join(", "));
        Ok(false)
    }
}

fn list() {
    eprintln!("Suites (default: static, rust, ui, app):");
    for suite in Suite::ALL {
        eprintln!("  {:<9} {}", suite.name(), suite.about());
    }
    eprintln!("Distributions (default: all):");
    for distro in Distro::ALL {
        eprintln!("  {}", distro.name());
    }
    eprintln!("UI flows of the ui suite (default: all, in this order):");
    for flow in Flow::ALL {
        eprintln!("  {:<8} {} ({})", flow.name, flow.about, flow.config);
    }
}

/// Runs the suites on this machine, stopping at the first failure.
fn run_here(
    root: &Path,
    suites: &[Suite],
    package_release: Option<u32>,
    flows: &[Flow],
    pseudo_locale: &str,
) -> Result<bool> {
    if suites.iter().any(|suite| suite.needs_node_modules()) {
        eprintln!("==> pnpm install --frozen-lockfile");
        if !Step::pnpm(&["install", "--frozen-lockfile"]).run(root)? {
            return Ok(false);
        }
    }
    for &suite in suites {
        for step in suite.steps(package_release, flows, pseudo_locale) {
            eprintln!("==> [{}] {step}", suite.name());
            if !step.run(root)? {
                eprintln!("==> FAILED: [{}] {step}", suite.name());
                return Ok(false);
            }
        }
    }
    let names: Vec<&str> = suites.iter().map(|suite| suite.name()).collect();
    eprintln!("==> passed: {}", names.join(", "));
    Ok(true)
}

/// The tool versions the images install, read from the repository's pins.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Pins {
    rust: String,
    node: String,
    pnpm: String,
    /// The UI's Playwright, whose Chromium the images install.
    playwright: String,
    /// cargo-deny, the version the pre-release workflow installs, so that
    /// the local `deny` suite and GitHub's agree.
    cargo_deny: String,
}

impl Pins {
    pub(crate) fn read(root: &Path) -> Result<Self> {
        let toolchain = read(root, "rust-toolchain.toml")?;
        let node = read(root, ".node-version")?;
        let package: serde_json::Value = serde_json::from_str(&read(root, "package.json")?)
            .context("cannot parse package.json")?;
        let ui: serde_json::Value = serde_json::from_str(&read(root, "ui/package.json")?)
            .context("cannot parse ui/package.json")?;
        let workflow = read(root, PRERELEASE_WORKFLOW)?;
        Self::from_files(&toolchain, &node, &package, &ui, &workflow)
    }

    fn from_files(
        toolchain: &str,
        node: &str,
        package: &serde_json::Value,
        ui: &serde_json::Value,
        workflow: &str,
    ) -> Result<Self> {
        let rust = toolchain
            .lines()
            .find_map(|line| line.trim().strip_prefix("channel"))
            .and_then(|rest| rest.trim().strip_prefix('='))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .context("rust-toolchain.toml names no channel")?;
        let pnpm = package["packageManager"]
            .as_str()
            .and_then(|value| value.strip_prefix("pnpm@"))
            .map(|value| value.split('+').next().unwrap_or(value).to_owned())
            .context("package.json names no pnpm version in `packageManager`")?;
        let playwright = ui["devDependencies"]["@playwright/test"]
            .as_str()
            .filter(|version| version.chars().all(|c| c.is_ascii_digit() || c == '.'))
            .map(str::to_owned)
            .context("ui/package.json pins no exact version of @playwright/test")?;
        let cargo_deny = workflow
            .lines()
            .find_map(|line| line.trim().strip_prefix("tool: cargo-deny@"))
            .map(str::trim)
            .filter(|version| {
                !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.')
            })
            .map(str::to_owned)
            .with_context(|| format!("{PRERELEASE_WORKFLOW} pins no exact cargo-deny version"))?;
        Ok(Self {
            rust,
            node: node.trim().to_owned(),
            pnpm,
            playwright,
            cargo_deny,
        })
    }
}

/// The workflow whose `tool: cargo-deny@<version>` pins cargo-deny.
const PRERELEASE_WORKFLOW: &str = ".github/workflows/prerelease.yml";

fn read(root: &Path, file: &str) -> Result<String> {
    fs::read_to_string(root.join(file)).with_context(|| format!("cannot read {file}"))
}

pub(crate) fn build_image(
    root: &Path,
    engine: Engine,
    distro: Distro,
    pins: &Pins,
    rebuild: bool,
) -> Result<bool> {
    let context = root.join("ci/containers");
    let file = context.join(format!("{}.Containerfile", distro.name()));
    let mut cmd = Command::new(engine.program());
    cmd.arg("build")
        .args(NO_CORE_DUMPS)
        .arg("--file")
        .arg(&file)
        .arg("--tag")
        .arg(distro.image())
        .arg("--build-arg")
        .arg(format!("RUST_TOOLCHAIN={}", pins.rust))
        .arg("--build-arg")
        .arg(format!("NODE_VERSION={}", pins.node))
        .arg("--build-arg")
        .arg(format!("PNPM_VERSION={}", pins.pnpm))
        .arg("--build-arg")
        .arg(format!("PLAYWRIGHT_VERSION={}", pins.playwright))
        .arg("--build-arg")
        .arg(format!("CARGO_DENY_VERSION={}", pins.cargo_deny));
    if rebuild {
        cmd.arg("--no-cache");
    }
    cmd.arg(&context);
    status(&mut cmd)
}

/// Keeps a crash inside a container away from the host's crash reporter.
///
/// When the host's `core_pattern` pipes core dumps to a program (as
/// systemd-coredump does), the kernel runs that program on the host for a
/// crash in any container, whatever the core size limit, with one
/// exception: a limit of exactly 1 byte skips the dump. A limit of 0 is not
/// enough; the host still records the crash and may show it on the desktop.
/// The hard limit is 1 byte too, so nothing in the container can raise it;
/// a script must not lower it either (`ulimit -c 0` sets 0).
pub(crate) const NO_CORE_DUMPS: [&str; 2] = ["--ulimit", "core=1:1"];

/// Environment variables passed from the host into the containers, when set.
const FORWARDED_ENV: [&str; 2] = ["CARGO_BUILD_JOBS", "CARGO_TERM_COLOR"];

/// The build caches of one distribution, as `--volume` arguments: named
/// volumes by default, or directories under `cache_dir`.
fn cache_volumes(distro: Distro, cache_dir: Option<&Path>) -> Result<Vec<String>> {
    let mounts = [
        (format!("target-{}", distro.name()), "/work/target"),
        ("cargo-registry".to_owned(), "/usr/local/cargo/registry"),
        ("pnpm-store".to_owned(), "/pnpm-store"),
    ];
    let mut volumes = Vec::new();
    for (name, target) in mounts {
        let source = match cache_dir {
            Some(dir) => {
                let dir = dir.join(&name);
                fs::create_dir_all(&dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
                let dir = fs::canonicalize(&dir)
                    .with_context(|| format!("cannot resolve {}", dir.display()))?;
                dir.to_str()
                    .context("the cache path is not valid UTF-8")?
                    .to_owned()
            }
            None => format!("{}-{name}", prefix()),
        };
        volumes.push(format!("{source}:{target}"));
    }
    Ok(volumes)
}

/// What one container run does: its suites and their options.
pub(crate) struct ContainerRun<'a> {
    pub(crate) suites: &'a [Suite],
    pub(crate) package_release: Option<u32>,
    pub(crate) flows: &'a [Flow],
    pub(crate) pseudo_locale: &'a str,
    pub(crate) cache_dir: Option<&'a Path>,
}

pub(crate) fn run_in_container(
    root: &Path,
    engine: Engine,
    distro: Distro,
    run: &ContainerRun<'_>,
) -> Result<bool> {
    let ContainerRun {
        suites,
        package_release,
        flows,
        pseudo_locale,
        cache_dir,
    } = *run;
    let names: Vec<&str> = suites.iter().map(|suite| suite.name()).collect();
    // Only a choice of flows is passed on; the default is every flow.
    let flows: Vec<&str> = if suites.contains(&Suite::Ui) && flows != Flow::ALL {
        flows.iter().map(|flow| flow.name).collect()
    } else {
        Vec::new()
    };
    // The source is mounted read-only and copied into the container, so the
    // host's tree is never written to. In a git checkout the copy is the
    // list of files git knows about, given on standard input, so ignored
    // files (build output, generated files) stay out; otherwise the whole
    // tree is copied without the build output.
    let files = crate::headers::git_files(root);
    let locale = (suites.contains(&Suite::Pseudo) && pseudo_locale != PSEUDO_LOCALE)
        .then_some(pseudo_locale);
    let script = copy_script(files.is_some(), &names, package_release, &flows, locale);
    let source = root
        .to_str()
        .context("the repository path is not valid UTF-8")?;
    let mut cmd = Command::new(engine.program());
    cmd.args(["run", "--rm", "--init", "--interactive"])
        .args(NO_CORE_DUMPS)
        // The source is only read; relabelling it for SELinux is not needed.
        .args(["--security-opt", "label=disable"])
        .arg("--volume")
        .arg(format!("{source}:/src:ro"));
    for volume in cache_volumes(distro, cache_dir)? {
        cmd.arg("--volume").arg(volume);
    }
    cmd.args(["--env", "PNPM_CONFIG_STORE_DIR=/pnpm-store"]);
    for name in FORWARDED_ENV {
        // `--env NAME` without a value passes the host's value on.
        if std::env::var_os(name).is_some() {
            cmd.args(["--env", name]);
        }
    }
    cmd.arg(distro.image()).args(["sh", "-c", &script]);
    let shown = format!("{cmd:?}");
    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot run {shown}"))?;
    {
        let mut stdin = child.stdin.take().context("no standard input")?;
        for file in files.iter().flatten() {
            let name = file
                .to_str()
                .with_context(|| format!("{} is not valid UTF-8", file.display()))?;
            stdin.write_all(name.as_bytes())?;
            stdin.write_all(b"\0")?;
        }
    }
    Ok(child
        .wait()
        .with_context(|| format!("cannot wait for {shown}"))?
        .success())
}

/// The shell script run in each container: copy the source into `/work`,
/// then run the suites there.
fn copy_script(
    from_list: bool,
    suites: &[&str],
    package_release: Option<u32>,
    flows: &[&str],
    pseudo_locale: Option<&str>,
) -> String {
    let copy = if from_list {
        "tar -C /src --null --files-from=- -cf - | tar -C /work -xf -"
    } else {
        "tar -C /src --exclude=./.git --exclude=./target --exclude=./node_modules \
         --exclude=./ui/node_modules --exclude=./ui/dist -cf - . | tar -C /work -xf -"
    };
    let release = package_release
        .map(|release| format!(" --package-release {release}"))
        .unwrap_or_default();
    let flows = if flows.is_empty() {
        String::new()
    } else {
        format!(" --ui-flows {}", flows.join(","))
    };
    let locale = pseudo_locale
        .map(|locale| format!(" --pseudo-locale {locale}"))
        .unwrap_or_default();
    format!(
        "set -e; {copy}; exec cargo xtask ci --here --suite {}{release}{flows}{locale} </dev/null",
        suites.join(",")
    )
}

pub(crate) fn status(cmd: &mut Command) -> Result<bool> {
    let shown = format!("{cmd:?}");
    Ok(cmd
        .status()
        .with_context(|| format!("cannot run {shown}"))?
        .success())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use std::path::PathBuf;

    use super::{
        CLI_FILES, CLI_FILES_DIR, Distro, Engine, FLOW_ENV_CLEARED, Flow, Options, Pins, Step,
        Suite, cache_volumes, copy_script, package_config,
    };

    fn parse(args: &[&str]) -> anyhow::Result<Options> {
        Options::parse(&args.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn defaults_cover_every_distribution_and_the_default_suites() {
        let options = parse(&[]).unwrap();
        assert_eq!(options.suites, Suite::DEFAULT);
        assert_eq!(options.distros, Distro::ALL);
        assert_eq!(options.engine, None);
        assert_eq!(options.ui_flows, Flow::ALL);
        assert!(!options.here);
    }

    #[test]
    fn ui_flows_can_be_chosen() {
        let options = parse(&[
            "--suite",
            "ui",
            "--ui-flows",
            "today,list",
            "--ui-flows=today",
        ])
        .unwrap();
        let names: Vec<&str> = options.ui_flows.iter().map(|flow| flow.name).collect();
        assert_eq!(names, ["today", "list"]);
        let options = parse(&["--here", "--ui-flows", "bridge"]).unwrap();
        assert_eq!(options.ui_flows, [Flow::parse("bridge").unwrap()]);
        assert!(parse(&["--ui-flows", "nope"]).is_err());
        assert!(parse(&["--ui-flows"]).is_err());
        assert!(
            parse(&["--suite", "rust", "--ui-flows", "list"]).is_err(),
            "ui suite only"
        );
    }

    #[test]
    fn every_playwright_config_is_a_flow_with_its_script() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui");
        let mut configs: Vec<String> = std::fs::read_dir(&ui)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("playwright") && name.ends_with(".config.ts"))
            .collect();
        configs.sort();
        // The pseudo-locale screenshots have a suite of their own.
        let mut known: Vec<String> = Flow::ALL
            .iter()
            .map(|flow| flow.config.to_owned())
            .chain([super::PSEUDO_CONFIG.to_owned()])
            .collect();
        known.sort();
        assert_eq!(
            configs, known,
            "every Playwright config runs in the ui suite"
        );
        let package: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(ui.join("package.json")).unwrap())
                .unwrap();
        for flow in Flow::ALL {
            let script = package["scripts"][format!("test:{}", flow.name)]
                .as_str()
                .unwrap_or_else(|| panic!("ui/package.json has no `test:{}`", flow.name));
            assert!(
                script == "playwright test" && flow.config == "playwright.config.ts"
                    || script.ends_with(&format!("--config {}", flow.config)),
                "`test:{}` runs {}",
                flow.name,
                flow.config
            );
        }
        assert!(FLOW_ENV_CLEARED.contains(&"GK_BRIDGE_WORKSPACE"));
        let today = Flow::parse("today").unwrap();
        assert_eq!(
            today.output(std::path::Path::new("/work")),
            std::path::Path::new("/work/target/playwright/today")
        );
    }

    #[test]
    fn suites_and_distributions_accept_lists_and_repeats() {
        let options = parse(&[
            "--suite",
            "static,rust",
            "--suite=static",
            "--distro",
            "fedora",
        ])
        .unwrap();
        assert_eq!(options.suites, [Suite::Static, Suite::Rust]);
        assert_eq!(options.distros, [Distro::Fedora]);
        let options = parse(&["--distro=ubuntu-22.04,debian-12", "--engine", "docker"]).unwrap();
        assert_eq!(options.distros, [Distro::Ubuntu2204, Distro::Debian12]);
        assert_eq!(options.engine, Some(Engine::Docker));
    }

    #[test]
    fn unknown_names_and_conflicts_are_errors() {
        assert!(parse(&["--suite", "nope"]).is_err());
        assert!(parse(&["--distro", "arch"]).is_err());
        assert!(parse(&["--engine", "lxc"]).is_err());
        assert!(parse(&["--suite"]).is_err());
        assert!(parse(&["--here", "--rebuild"]).is_err());
        assert!(parse(&["--here", "--cache-dir", "/tmp/x"]).is_err());
        assert!(parse(&["--cache-dir"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(
            parse(&["--package-release", "2"]).is_err(),
            "package suite only"
        );
        assert!(parse(&["--suite", "package", "--package-release", "0"]).is_err());
        assert!(parse(&["--suite", "package", "--package-release", "x"]).is_err());
        assert!(parse(&["--suite", "package", "--package-release"]).is_err());
    }

    #[test]
    fn packaging_is_opt_in() {
        assert!(!Suite::DEFAULT.contains(&Suite::Package));
        assert!(!Suite::DEFAULT.contains(&Suite::Deny));
        let options = parse(&["--suite", "package", "--cache-dir", "ci-cache"]).unwrap();
        assert_eq!(options.suites, [Suite::Package]);
        assert_eq!(options.cache_dir, Some(PathBuf::from("ci-cache")));
        assert_eq!(options.package_release, None);
        assert!(Suite::Package.needs_node_modules());
        let options = parse(&["--suite=package", "--package-release=3"]).unwrap();
        assert_eq!(options.package_release, Some(3));
        let options = parse(&["--here", "--suite", "package", "--package-release", "2"]).unwrap();
        assert_eq!(options.package_release, Some(2));
    }

    #[test]
    fn caches_are_named_volumes_or_directories() {
        let volumes = cache_volumes(Distro::Debian12, None).unwrap();
        assert_eq!(
            volumes,
            [
                "glyphkeel-ci-target-debian-12:/work/target",
                "glyphkeel-ci-cargo-registry:/usr/local/cargo/registry",
                "glyphkeel-ci-pnpm-store:/pnpm-store",
            ]
        );
        let dir = std::env::temp_dir().join(format!("xtask-ci-cache-{}", std::process::id()));
        let volumes = cache_volumes(Distro::Fedora, Some(&dir)).unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(
            volumes[0],
            format!("{}:/work/target", dir.join("target-fedora").display())
        );
        assert!(dir.join("cargo-registry").is_dir());
        assert_eq!(
            volumes[2],
            format!("{}:/pnpm-store", dir.join("pnpm-store").display())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_distribution_has_a_containerfile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for distro in Distro::ALL {
            let file = root.join(format!("ci/containers/{}.Containerfile", distro.name()));
            assert!(file.is_file(), "missing {}", file.display());
        }
    }

    #[test]
    fn the_repository_pins_every_tool() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Pins::read(&root).unwrap();
    }

    #[test]
    fn the_packages_hold_the_app_and_the_command_line() {
        let config: serde_json::Value = serde_json::from_str(&package_config(None)).unwrap();
        assert_eq!(config["bundle"]["active"], true);
        for (format, zsh) in [
            ("deb", "/usr/share/zsh/vendor-completions"),
            ("rpm", "/usr/share/zsh/site-functions"),
        ] {
            let files = &config["bundle"]["linux"][format]["files"];
            assert_eq!(files["/usr/bin/glyphkeel"], "../../target/dist/glyphkeel");
            let generated = |name: &str| format!("../../{CLI_FILES_DIR}/{name}");
            assert_eq!(
                files["/usr/share/bash-completion/completions/glyphkeel"],
                generated("glyphkeel.bash")
            );
            assert_eq!(files[format!("{zsh}/_glyphkeel")], generated("_glyphkeel"));
            assert_eq!(
                files["/usr/share/fish/vendor_completions.d/glyphkeel.fish"],
                generated("glyphkeel.fish")
            );
            assert_eq!(
                files["/usr/share/man/man1/glyphkeel.1.gz"],
                generated("glyphkeel.1.gz")
            );
            assert_eq!(files.as_object().unwrap().len(), 5);
        }
        // Every packaged generated file is one the command line writes.
        for (name, _) in CLI_FILES {
            let packaged = if name.ends_with(".1") {
                format!("{name}.gz")
            } else {
                name.to_owned()
            };
            assert!(package_config(None).contains(&format!("{CLI_FILES_DIR}/{packaged}")));
        }
        // Tauri's default release unless one is given.
        assert!(config["bundle"]["linux"]["rpm"].get("release").is_none());
        let config: serde_json::Value = serde_json::from_str(&package_config(Some(3))).unwrap();
        assert_eq!(config["bundle"]["linux"]["rpm"]["release"], "3");

        let steps: Vec<String> = Suite::Package
            .steps(Some(3), &Flow::ALL, "en-XA")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(steps[0].starts_with("cargo build --package gk-cli"));
        assert!(steps[0].ends_with("--profile dist"));
        assert_eq!(
            Suite::Package.steps(None, &Flow::ALL, "en-XA")[1],
            Step::CliFiles
        );
        assert!(steps[2].starts_with("pnpm tauri build"));
        assert!(steps[2].contains(r#""release":"3""#));
    }

    #[test]
    fn pins_come_from_the_repository_files() {
        let toolchain = "[toolchain]\nchannel = \"1.100.0\"\ncomponents = [\"clippy\"]\n";
        let package = json!({ "packageManager": "pnpm@12.21.0+sha512.abc" });
        let ui = json!({ "devDependencies": { "@playwright/test": "1.65.1" } });
        let wf =
            "      - uses: some/action@abc\n        with:\n          tool: cargo-deny@0.21.0\n";
        let pins = Pins::from_files(toolchain, "24.18.0\n", &package, &ui, wf).unwrap();
        assert_eq!(
            pins,
            Pins {
                rust: "1.100.0".into(),
                node: "24.18.0".into(),
                pnpm: "12.21.0".into(),
                playwright: "1.65.1".into(),
                cargo_deny: "0.21.0".into(),
            }
        );
        assert!(Pins::from_files("[toolchain]\n", "24", &package, &ui, wf).is_err());
        assert!(Pins::from_files(toolchain, "24", &json!({}), &ui, wf).is_err());
        assert!(Pins::from_files(toolchain, "24", &package, &json!({}), wf).is_err());
        let range = json!({ "devDependencies": { "@playwright/test": "^1.65.1" } });
        assert!(Pins::from_files(toolchain, "24", &package, &range, wf).is_err());
        let latest = "          tool: cargo-deny@latest\n";
        assert!(Pins::from_files(toolchain, "24", &package, &ui, latest).is_err());
        assert!(Pins::from_files(toolchain, "24", &package, &ui, "").is_err());
    }

    #[test]
    fn the_suites_check_generated_files_the_bundle_and_every_flow() {
        assert!(
            Suite::Static
                .steps(None, &Flow::ALL, "en-XA")
                .contains(&Step::GenCheck)
        );
        let ui: Vec<String> = Suite::Ui
            .steps(None, &Flow::ALL, "en-XA")
            .iter()
            .map(ToString::to_string)
            .collect();
        let build = ui.iter().position(|step| step == "pnpm build").unwrap();
        let bundle = ui
            .iter()
            .position(|step| step == "pnpm --filter ui check:bundle")
            .unwrap();
        assert!(build < bundle, "the bundle is checked after it is built");
        let flows = ui
            .iter()
            .position(|step| step.starts_with("playwright flows: flow, board, editor, bridge"))
            .unwrap();
        assert!(
            bundle < flows,
            "the flows rebuild dist/ after the bundle check"
        );
        assert!(
            ui[flows]
                .ends_with("list, palette, today, safety, prefs, wiki, gantt, calendar, planning")
        );
        assert_eq!(Step::GenCheck.to_string(), "cargo xtask gen --check");
    }

    #[test]
    fn the_container_copies_the_listed_files_or_the_tree() {
        let script = copy_script(true, &["static", "ui"], None, &[], None);
        assert!(script.contains("--null --files-from=-"));
        assert!(script.ends_with("exec cargo xtask ci --here --suite static,ui </dev/null"));
        let script = copy_script(true, &["ui"], None, &["list", "today"], None);
        assert!(script.ends_with("--suite ui --ui-flows list,today </dev/null"));
        let script = copy_script(true, &["package"], Some(2), &[], None);
        assert!(script.ends_with("--suite package --package-release 2 </dev/null"));
        let script = copy_script(false, &["rust"], None, &[], None);
        assert!(script.contains("--exclude=./target"));
        assert!(!script.contains("--files-from"));
    }
}
