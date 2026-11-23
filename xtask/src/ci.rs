// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask ci`: the full check suites, run in containers of the
//! supported distributions, so each is checked against its own system
//! libraries (WebKitGTK, GTK, SQLite).
//!
//! On the host it builds one image per distribution from
//! `ci/containers/<distro>.Containerfile`, copies the source into a fresh
//! container and runs `cargo xtask ci --here` inside it. Build caches (the
//! cargo registry, each distribution's `target/` and the pnpm store) live in
//! named volumes, so runs after the first are incremental.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};

const HELP: &str = "\
Usage: cargo xtask ci [--suite S[,S…]] [--distro D[,D…]] [--engine E] [--rebuild]
                      [--cache-dir DIR]
       cargo xtask ci --here [--suite S[,S…]]
       cargo xtask ci --list

Runs check suites in containers, one image per distribution.

Options:
  --suite S     Suites to run (repeatable or comma-separated). Default: all
                but `deny` and `package`.
  --distro D    Distributions to run on (repeatable or comma-separated).
                Default: every distribution.
  --engine E    `podman` or `docker`. Default: podman when it is installed,
                docker otherwise.
  --rebuild     Rebuild the images from scratch, without the layer cache.
  --cache-dir DIR
                Keep the build caches in DIR (bind mounts) instead of named
                volumes: DIR/cargo-registry, DIR/pnpm-store and
                DIR/target-<distro>. Packages land in
                DIR/target-<distro>/dist/bundle/.
  --here        Run the suites on this machine, without containers. This is
                what runs inside each container.
  --list        List the suites and distributions.

CARGO_BUILD_JOBS and CARGO_TERM_COLOR, when set, are passed into the
containers.
";

/// The prefix of every image and volume this command creates.
const PREFIX: &str = "glyphkeel-ci";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suite {
    /// Formatting, licence headers, dependency rules, clippy, ESLint,
    /// stylelint and the TypeScript check.
    Static,
    /// Every Rust test.
    Rust,
    /// The UI's component tests and its production build.
    Ui,
    /// The desktop app built with the UI embedded, as it ships.
    App,
    /// Licences, advisories, bans and sources of the crates (cargo-deny).
    Deny,
    /// The deb and rpm packages, built by the Tauri bundler with the
    /// release profile `dist`.
    Package,
}

impl Suite {
    const ALL: [Self; 6] = [
        Self::Static,
        Self::Rust,
        Self::Ui,
        Self::App,
        Self::Deny,
        Self::Package,
    ];
    const DEFAULT: [Self; 4] = [Self::Static, Self::Rust, Self::Ui, Self::App];

    fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Rust => "rust",
            Self::Ui => "ui",
            Self::App => "app",
            Self::Deny => "deny",
            Self::Package => "package",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Self::Static => {
                "fmt, licence headers, dependency rules, clippy, ESLint, stylelint, tsc"
            }
            Self::Rust => "cargo test for the whole workspace",
            Self::Ui => "the UI's component tests and production build",
            Self::App => "the desktop app with the UI embedded",
            Self::Deny => "cargo-deny: licences, advisories, bans, sources",
            Self::Package => "deb and rpm packages (Tauri bundler, `dist` profile)",
        }
    }

    fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|suite| suite.name() == name)
            .with_context(|| format!("unknown suite `{name}` (see `cargo xtask ci --list`)"))
    }

    fn needs_node_modules(self) -> bool {
        matches!(self, Self::Static | Self::Ui | Self::App | Self::Package)
    }

    /// The commands of this suite, in order.
    fn steps(self) -> Vec<Step> {
        match self {
            Self::Static => vec![
                Step::cargo(&["fmt", "--all", "--check"]),
                Step::Headers,
                Step::Deps,
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
            // `bundle.active` is off in tauri.conf.json, so everyday builds
            // never bundle; this suite switches it on. The UI is built by
            // the config's `beforeBuildCommand`.
            Self::Package => vec![Step::pnpm(&[
                "tauri",
                "build",
                "--ci",
                "--bundles",
                "deb,rpm",
                "--config",
                r#"{"bundle":{"active":true}}"#,
                "--",
                "--profile",
                "dist",
            ])],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Distro {
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

    fn name(self) -> &'static str {
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
        format!("{PREFIX}:{}", self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Podman,
    Docker,
}

impl Engine {
    fn program(self) -> &'static str {
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
    Cargo(Vec<&'static str>),
    Pnpm(Vec<&'static str>),
    Headers,
    Deps,
}

impl Step {
    fn cargo(args: &[&'static str]) -> Self {
        Self::Cargo(args.to_vec())
    }

    fn pnpm(args: &[&'static str]) -> Self {
        Self::Pnpm(args.to_vec())
    }

    fn run(&self, root: &Path) -> Result<bool> {
        match self {
            Self::Cargo(args) => status(Command::new(crate::cargo()).args(args).current_dir(root)),
            Self::Pnpm(args) => status(Command::new("pnpm").args(args).current_dir(root)),
            Self::Headers => crate::headers::run(root),
            Self::Deps => crate::deps::run(root),
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cargo(args) => write!(f, "cargo {}", args.join(" ")),
            Self::Pnpm(args) => write!(f, "pnpm {}", args.join(" ")),
            Self::Headers => f.write_str("cargo xtask check-headers"),
            Self::Deps => f.write_str("cargo xtask check-deps"),
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
        return run_here(root, &options.suites);
    }
    let engine = match options.engine {
        Some(engine) => engine,
        None => Engine::detect()?,
    };
    let pins = Pins::read(root)?;
    let mut failed = Vec::new();
    for &distro in &options.distros {
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
                &options.suites,
                options.cache_dir.as_deref(),
            )?;
        let seconds = started.elapsed().as_secs();
        if ok {
            eprintln!("==> {}: passed in {seconds} s", distro.name());
        } else {
            eprintln!("==> {}: FAILED after {seconds} s", distro.name());
            failed.push(distro.name());
        }
    }
    if failed.is_empty() {
        eprintln!("==> ci: every distribution passed");
        Ok(true)
    } else {
        eprintln!("==> ci: failed on {}", failed.join(", "));
        Ok(false)
    }
}

fn list() {
    eprintln!("Suites (default: all but deny and package):");
    for suite in Suite::ALL {
        eprintln!("  {:<8} {}", suite.name(), suite.about());
    }
    eprintln!("Distributions (default: all):");
    for distro in Distro::ALL {
        eprintln!("  {}", distro.name());
    }
}

/// Runs the suites on this machine, stopping at the first failure.
fn run_here(root: &Path, suites: &[Suite]) -> Result<bool> {
    if suites.iter().any(|suite| suite.needs_node_modules()) {
        eprintln!("==> pnpm install --frozen-lockfile");
        if !Step::pnpm(&["install", "--frozen-lockfile"]).run(root)? {
            return Ok(false);
        }
    }
    for &suite in suites {
        for step in suite.steps() {
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
struct Pins {
    rust: String,
    node: String,
    pnpm: String,
}

impl Pins {
    fn read(root: &Path) -> Result<Self> {
        let toolchain = read(root, "rust-toolchain.toml")?;
        let node = read(root, ".node-version")?;
        let package: serde_json::Value = serde_json::from_str(&read(root, "package.json")?)
            .context("cannot parse package.json")?;
        Self::from_files(&toolchain, &node, &package)
    }

    fn from_files(toolchain: &str, node: &str, package: &serde_json::Value) -> Result<Self> {
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
        Ok(Self {
            rust,
            node: node.trim().to_owned(),
            pnpm,
        })
    }
}

fn read(root: &Path, file: &str) -> Result<String> {
    fs::read_to_string(root.join(file)).with_context(|| format!("cannot read {file}"))
}

fn build_image(
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
        .arg("--file")
        .arg(&file)
        .arg("--tag")
        .arg(distro.image())
        .arg("--build-arg")
        .arg(format!("RUST_TOOLCHAIN={}", pins.rust))
        .arg("--build-arg")
        .arg(format!("NODE_VERSION={}", pins.node))
        .arg("--build-arg")
        .arg(format!("PNPM_VERSION={}", pins.pnpm));
    if rebuild {
        cmd.arg("--no-cache");
    }
    cmd.arg(&context);
    status(&mut cmd)
}

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
            None => format!("{PREFIX}-{name}"),
        };
        volumes.push(format!("{source}:{target}"));
    }
    Ok(volumes)
}

fn run_in_container(
    root: &Path,
    engine: Engine,
    distro: Distro,
    suites: &[Suite],
    cache_dir: Option<&Path>,
) -> Result<bool> {
    let names: Vec<&str> = suites.iter().map(|suite| suite.name()).collect();
    // The source is mounted read-only and copied into the container, so the
    // host's tree is never written to. Build output stays out of the copy.
    let script = format!(
        "set -e; tar -C /src --exclude=./.git --exclude=./target --exclude=./node_modules \
         --exclude=./ui/node_modules --exclude=./ui/dist -cf - . | tar -C /work -xf -; \
         exec cargo xtask ci --here --suite {}",
        names.join(",")
    );
    let source = root
        .to_str()
        .context("the repository path is not valid UTF-8")?;
    let mut cmd = Command::new(engine.program());
    cmd.args(["run", "--rm", "--init"])
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
    status(&mut cmd)
}

fn status(cmd: &mut Command) -> Result<bool> {
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

    use super::{Distro, Engine, Options, Pins, Suite, cache_volumes};

    fn parse(args: &[&str]) -> anyhow::Result<Options> {
        Options::parse(&args.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn defaults_cover_every_distribution_and_the_default_suites() {
        let options = parse(&[]).unwrap();
        assert_eq!(options.suites, Suite::DEFAULT);
        assert_eq!(options.distros, Distro::ALL);
        assert_eq!(options.engine, None);
        assert!(!options.here);
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
    }

    #[test]
    fn packaging_is_opt_in() {
        assert!(!Suite::DEFAULT.contains(&Suite::Package));
        assert!(!Suite::DEFAULT.contains(&Suite::Deny));
        let options = parse(&["--suite", "package", "--cache-dir", "ci-cache"]).unwrap();
        assert_eq!(options.suites, [Suite::Package]);
        assert_eq!(options.cache_dir, Some(PathBuf::from("ci-cache")));
        assert!(Suite::Package.needs_node_modules());
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
    fn pins_come_from_the_repository_files() {
        let toolchain = "[toolchain]\nchannel = \"1.100.0\"\ncomponents = [\"clippy\"]\n";
        let package = json!({ "packageManager": "pnpm@12.21.0+sha512.abc" });
        let pins = Pins::from_files(toolchain, "24.18.0\n", &package).unwrap();
        assert_eq!(
            pins,
            Pins {
                rust: "1.100.0".into(),
                node: "24.18.0".into(),
                pnpm: "12.21.0".into()
            }
        );
        assert!(Pins::from_files("[toolchain]\n", "24", &package).is_err());
        assert!(Pins::from_files(toolchain, "24", &json!({})).is_err());
    }
}
