// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask ci --distro windows-cross`: the Windows build, cross-compiled
//! on Linux with cargo-xwin and run under Wine, in the image of
//! `ci/containers/windows-cross.Containerfile`.
//!
//! Its suites build both programs, run clippy for the Windows target, run
//! the crates' tests under Wine, take the command line and its MCP server
//! through a short session under Wine, make the NSIS installer with the
//! Tauri bundler and install and uninstall it under Wine. Wine runs without a
//! display, so nothing ever opens a window.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::ci::{self, Engine, Pins};

/// The name `--distro` takes.
pub(crate) const NAME: &str = "windows-cross";

/// The Rust target of the Windows build.
pub(crate) const TARGET: &str = "x86_64-pc-windows-msvc";

/// The workspace version, which the installer's name carries.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Tauri configuration the `nsis` suite adds: everyday builds never
/// bundle (`bundle.active` is off in tauri.conf.json).
const BUNDLE_CONFIG: &str = r#"{"bundle":{"active":true}}"#;

/// The Cargo profile of the installer. The command line and the app must
/// both be built with it, for the same target and into the same target
/// folder: the installer takes `glyphkeel.exe` from beside the app.
const INSTALLER_PROFILE: &str = "dist";

/// Where the release directory is mounted in the container.
const RELEASE_MOUNT: &str = "/release";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WinSuite {
    /// The command line and the desktop app, with the UI embedded.
    Build,
    /// clippy for the Windows target, every crate and target.
    Clippy,
    /// The crates' unit and integration tests under Wine.
    Test,
    /// A command-line and MCP session under Wine.
    Smoke,
    /// The NSIS installer, from the `dist` profile, unsigned.
    Nsis,
    /// The installer installed and uninstalled silently under Wine.
    Install,
}

impl WinSuite {
    pub(crate) const ALL: [Self; 6] = [
        Self::Build,
        Self::Clippy,
        Self::Test,
        Self::Smoke,
        Self::Nsis,
        Self::Install,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Clippy => "clippy",
            Self::Test => "test",
            Self::Smoke => "smoke",
            Self::Nsis => "nsis",
            Self::Install => "install",
        }
    }

    pub(crate) fn about(self) -> &'static str {
        match self {
            Self::Build => "glyphkeel.exe and glyphkeel-app.exe with the UI embedded",
            Self::Clippy => "clippy for the Windows target, every crate and target",
            Self::Test => "the crates' unit and integration tests under Wine",
            Self::Smoke => "a command-line and MCP session under Wine (ci/test/wine-smoke.sh)",
            Self::Nsis => "the NSIS installer (`dist` profile, unsigned)",
            Self::Install => {
                "silent install and uninstall of the installer under Wine \
                 (ci/test/wine-install.sh)"
            }
        }
    }

    pub(crate) fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|suite| suite.name() == name)
            .with_context(|| {
                format!("unknown suite `{name}` for `{NAME}` (see `cargo xtask ci --list`)")
            })
    }

    fn needs_node_modules(self) -> bool {
        matches!(self, Self::Build | Self::Nsis)
    }

    /// The commands of this suite, in order. `release_dir` is where the
    /// `nsis` suite copies the installer.
    fn steps(self, release_dir: Option<&Path>) -> Vec<WinStep> {
        match self {
            Self::Build => vec![
                WinStep::pnpm(&["build"]),
                WinStep::xwin(&["build", "--locked", "--package", "gk-cli"]),
                WinStep::xwin(&[
                    "build",
                    "--locked",
                    "--package",
                    "gk-app",
                    "--features",
                    "tauri/custom-protocol",
                ]),
            ],
            Self::Clippy => vec![WinStep::xwin(&[
                "clippy",
                "--locked",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ])],
            Self::Test => vec![
                WinStep::FreshPrefix("test"),
                WinStep::Tests(
                    ["test", "--locked", "--no-fail-fast"]
                        .map(str::to_owned)
                        .to_vec(),
                ),
            ],
            Self::Smoke => vec![
                WinStep::xwin(&["build", "--locked", "--package", "gk-cli"]),
                WinStep::Script {
                    script: SMOKE_SCRIPT,
                    program: cli_debug(),
                },
            ],
            Self::Nsis => {
                let mut steps = vec![
                    // The command line of the same profile, target and
                    // target folder, for the installer to carry next to
                    // the app.
                    WinStep::xwin(&[
                        "build",
                        "--locked",
                        "--package",
                        "gk-cli",
                        "--profile",
                        INSTALLER_PROFILE,
                    ]),
                    WinStep::Bundle,
                ];
                if let Some(dir) = release_dir {
                    steps.push(WinStep::Publish(dir.to_path_buf()));
                }
                steps
            }
            Self::Install => vec![WinStep::Script {
                script: INSTALL_SCRIPT,
                program: installer(),
            }],
        }
    }
}

/// The command-line and MCP session, relative to the repository.
pub(crate) const SMOKE_SCRIPT: &str = "ci/test/wine-smoke.sh";

/// The install and uninstall check, relative to the repository.
pub(crate) const INSTALL_SCRIPT: &str = "ci/test/wine-install.sh";

/// The debug command line, relative to the repository.
fn cli_debug() -> PathBuf {
    Path::new("target").join(TARGET).join("debug/glyphkeel.exe")
}

/// The installer the `nsis` suite makes, relative to the repository.
pub(crate) fn installer() -> PathBuf {
    Path::new("target")
        .join(TARGET)
        .join(INSTALLER_PROFILE)
        .join("bundle/nsis")
        .join(installer_name())
}

/// The installer's file name, as the Tauri bundler names it.
pub(crate) fn installer_name() -> String {
    format!("Glyphkeel_{VERSION}_x64-setup.exe")
}

/// Where `--release-dir DIR` puts the installer.
pub(crate) fn release_folder(dir: &Path) -> PathBuf {
    dir.join(VERSION).join("windows")
}

/// A path as Wine shows it to Windows programs: the Linux root is drive Z:.
pub(crate) fn wine_path(path: &Path) -> String {
    format!("Z:{}", path.display()).replace('/', "\\")
}

#[derive(Debug, PartialEq, Eq)]
enum WinStep {
    Pnpm(Vec<String>),
    /// `cargo xwin <args>` for the Windows target.
    Xwin(Vec<String>),
    /// A new, empty Wine prefix of this name, booted.
    FreshPrefix(&'static str),
    /// `cargo xwin <args>` with the test programs run under Wine, in the
    /// prefix of the `test` suite.
    Tests(Vec<String>),
    /// A check script, given a program relative to the repository.
    Script {
        script: &'static str,
        program: PathBuf,
    },
    /// The NSIS installer, made by the Tauri bundler through cargo-xwin.
    Bundle,
    /// The installer copied to `<dir>/<version>/windows/`.
    Publish(PathBuf),
}

impl WinStep {
    fn pnpm(args: &[&str]) -> Self {
        Self::Pnpm(args.iter().map(|&arg| arg.to_owned()).collect())
    }

    fn xwin(args: &[&str]) -> Self {
        Self::Xwin(args.iter().map(|&arg| arg.to_owned()).collect())
    }

    /// `cargo xwin <command> --target <TARGET> <rest>`.
    fn xwin_args(args: &[String]) -> Vec<String> {
        let mut all = vec!["xwin".to_owned()];
        if let Some((command, rest)) = args.split_first() {
            all.push(command.clone());
            all.extend(["--target".to_owned(), TARGET.to_owned()]);
            all.extend(rest.iter().cloned());
        }
        all
    }

    /// The arguments of `pnpm tauri build` for the installer.
    fn bundle_args() -> Vec<String> {
        [
            "tauri",
            "build",
            "--ci",
            "--runner",
            "cargo-xwin",
            "--target",
            TARGET,
            "--bundles",
            "nsis",
            "--config",
            BUNDLE_CONFIG,
            "--",
            "--profile",
            INSTALLER_PROFILE,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    fn run(&self, root: &Path) -> Result<bool> {
        match self {
            Self::Pnpm(args) => ci::status(Command::new("pnpm").args(args).current_dir(root)),
            Self::Xwin(args) => ci::status(
                Command::new(crate::cargo())
                    .args(Self::xwin_args(args))
                    .current_dir(root),
            ),
            Self::FreshPrefix(name) => fresh_prefix(&prefix_dir(name)),
            Self::Tests(args) => {
                let prefix = prefix_dir("test");
                let data = prefix_dir("test-data");
                fs::create_dir_all(&data)
                    .with_context(|| format!("cannot create {}", data.display()))?;
                let mut cmd = Command::new(crate::cargo());
                cmd.args(Self::xwin_args(args)).current_dir(root);
                for (name, value) in test_env(root, &prefix, &data) {
                    cmd.env(name, value);
                }
                ci::status(&mut cmd)
            }
            Self::Script { script, program } => {
                let program = root.join(program);
                if !program.is_file() {
                    eprintln!(
                        "{} is missing; build it first (the `nsis` suite makes the installer)",
                        program.display()
                    );
                    return Ok(false);
                }
                ci::status(
                    Command::new("sh")
                        .arg(root.join(script))
                        .arg(&program)
                        .env("EXPECTED_VERSION", VERSION)
                        .env("GK_WINE_ROOT", prefix_dir(""))
                        .current_dir(root),
                )
            }
            Self::Bundle => ci::status(
                Command::new("pnpm")
                    .args(Self::bundle_args())
                    // The Tauri CLI reads CI as a boolean and refuses `1`.
                    .env("CI", "true")
                    .current_dir(root),
            ),
            Self::Publish(dir) => publish(root, dir),
        }
    }
}

impl std::fmt::Display for WinStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pnpm(args) => write!(f, "pnpm {}", args.join(" ")),
            Self::Xwin(args) => write!(f, "cargo {}", Self::xwin_args(args).join(" ")),
            Self::FreshPrefix(name) => write!(f, "a new Wine prefix `{name}`"),
            Self::Tests(args) => {
                write!(f, "cargo {} (under Wine)", Self::xwin_args(args).join(" "))
            }
            Self::Script { script, program } => {
                write!(f, "sh {script} {}", program.display())
            }
            Self::Bundle => write!(f, "pnpm {}", Self::bundle_args().join(" ")),
            Self::Publish(dir) => {
                write!(
                    f,
                    "copy {} to {}",
                    installer_name(),
                    release_folder(dir).display()
                )
            }
        }
    }
}

/// The folder of the Wine prefix `name`, in the temporary folder. An empty
/// name gives the folder that holds them.
fn prefix_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join("gk-wine").join(name)
}

/// The environment of the tests under Wine: the runner, the prefix, the
/// data and settings folders kept apart from the prefix's user profile, and
/// insta's workspace as a Windows path (the compiled-in Linux path means
/// nothing to the test program).
fn test_env(root: &Path, prefix: &Path, data: &Path) -> Vec<(&'static str, String)> {
    vec![
        (
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER",
            "wine".to_owned(),
        ),
        ("WINEPREFIX", prefix.display().to_string()),
        ("WINEDEBUG", "-all".to_owned()),
        ("INSTA_WORKSPACE_ROOT", wine_path(root)),
        ("GLYPHKEEL_DATA_DIR", wine_path(&data.join("data"))),
        ("GLYPHKEEL_CONFIG_DIR", wine_path(&data.join("config"))),
    ]
}

/// Removes the prefix and boots a new one, so that the first test doesn't
/// pay for it (or time out on it).
fn fresh_prefix(dir: &Path) -> Result<bool> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("cannot remove {}", dir.display()))?;
    }
    let parent = dir.parent().context("the Wine prefix has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    Ok(
        ci::status(Command::new("wineboot").arg("-i").env("WINEPREFIX", dir))?
            && ci::status(Command::new("wineserver").arg("-w").env("WINEPREFIX", dir))?,
    )
}

/// Copies the installer to `<dir>/<version>/windows/`.
fn publish(root: &Path, dir: &Path) -> Result<bool> {
    let from = root.join(installer());
    let folder = release_folder(dir);
    fs::create_dir_all(&folder).with_context(|| format!("cannot create {}", folder.display()))?;
    let to = folder.join(installer_name());
    fs::copy(&from, &to)
        .with_context(|| format!("cannot copy {} to {}", from.display(), to.display()))?;
    eprintln!("==> [{NAME}] installer: {}", to.display());
    Ok(true)
}

/// Runs the suites on this machine, stopping at the first failure. This is
/// what runs inside the container.
pub(crate) fn run_here(
    root: &Path,
    suites: &[WinSuite],
    release_dir: Option<&Path>,
) -> Result<bool> {
    if suites.iter().any(|suite| suite.needs_node_modules()) {
        eprintln!("==> pnpm install --frozen-lockfile");
        if !WinStep::pnpm(&["install", "--frozen-lockfile"]).run(root)? {
            return Ok(false);
        }
    }
    let mut times = Vec::new();
    for &suite in suites {
        let started = Instant::now();
        for step in suite.steps(release_dir) {
            eprintln!("==> [{NAME}/{}] {step}", suite.name());
            let step_started = Instant::now();
            let ok = step.run(root)?;
            eprintln!(
                "==> [{NAME}/{}] {} in {} s",
                suite.name(),
                if ok { "done" } else { "FAILED" },
                step_started.elapsed().as_secs()
            );
            if !ok {
                eprintln!("==> FAILED: [{NAME}/{}] {step}", suite.name());
                return Ok(false);
            }
        }
        times.push((suite, started.elapsed().as_secs()));
    }
    eprintln!("==> [{NAME}] passed:");
    for (suite, seconds) in times {
        eprintln!("      {:<8} {seconds:>5} s", suite.name());
    }
    Ok(true)
}

/// What the host run does.
pub(crate) struct HostOptions<'a> {
    pub(crate) suites: &'a [WinSuite],
    pub(crate) rebuild: bool,
    pub(crate) cache_dir: Option<&'a Path>,
    pub(crate) release_dir: Option<&'a Path>,
}

/// Builds the image and runs the suites in a fresh container.
pub(crate) fn run_in_container(
    root: &Path,
    engine: Engine,
    pins: &Pins,
    options: &HostOptions<'_>,
) -> Result<bool> {
    let started = Instant::now();
    eprintln!("==> {NAME}: building the image {}:{NAME}", ci::prefix());
    if !ci::build_named_image(root, engine, NAME, pins, options.rebuild)? {
        eprintln!("==> {NAME}: FAILED to build the image");
        return Ok(false);
    }
    let files = crate::headers::git_files(root);
    let source = root
        .to_str()
        .context("the repository path is not valid UTF-8")?;
    let mut cmd = ci::container_command(engine, source);
    for volume in ci::named_volumes(&cache_mounts(), options.cache_dir)? {
        cmd.arg("--volume").arg(volume);
    }
    if let Some(dir) = options.release_dir {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let dir =
            fs::canonicalize(dir).with_context(|| format!("cannot resolve {}", dir.display()))?;
        let dir = dir
            .to_str()
            .context("the release path is not valid UTF-8")?;
        cmd.arg("--volume").arg(format!("{dir}:{RELEASE_MOUNT}"));
    }
    let names: Vec<&str> = options.suites.iter().map(|suite| suite.name()).collect();
    let script = container_script(files.is_some(), &names, options.release_dir.is_some());
    cmd.arg(format!("{}:{NAME}", ci::prefix()))
        .args(["sh", "-c", &script]);
    let ok = ci::run_with_files(cmd, files.as_deref())?;
    let seconds = started.elapsed().as_secs();
    if ok {
        eprintln!("==> {NAME}: passed in {seconds} s");
        if let Some(dir) = options.release_dir {
            eprintln!(
                "==> {NAME}: installer in {}",
                release_folder(dir).join(installer_name()).display()
            );
        }
    } else {
        eprintln!("==> {NAME}: FAILED after {seconds} s");
    }
    Ok(ok)
}

/// The caches of the Windows build: its own target folder, the shared cargo
/// registry and pnpm store, and the Tauri bundler's downloads (the NSIS
/// plug-ins and the WebView2 bootstrapper).
fn cache_mounts() -> Vec<(String, &'static str)> {
    vec![
        (format!("target-{NAME}"), "/work/target"),
        ("cargo-registry".to_owned(), "/usr/local/cargo/registry"),
        ("pnpm-store".to_owned(), "/pnpm-store"),
        (format!("tauri-cache-{NAME}"), "/root/.cache/tauri"),
    ]
}

/// The shell script run in the container: copy the source into /work, then
/// run the suites there.
fn container_script(from_list: bool, suites: &[&str], release: bool) -> String {
    let release = if release {
        format!(" --release-dir {RELEASE_MOUNT}")
    } else {
        String::new()
    };
    format!(
        "set -e; {}; exec cargo xtask ci --here --distro {NAME} --suite {}{release} </dev/null",
        ci::copy_command(from_list),
        suites.join(",")
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        INSTALL_SCRIPT, NAME, SMOKE_SCRIPT, TARGET, VERSION, WinStep, WinSuite, cache_mounts,
        container_script, installer, installer_name, release_folder, test_env, wine_path,
    };

    fn steps(suite: WinSuite, release: Option<&Path>) -> Vec<String> {
        suite
            .steps(release)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn suites_parse_by_name_and_run_in_order() {
        for suite in WinSuite::ALL {
            assert_eq!(WinSuite::parse(suite.name()).unwrap(), suite);
        }
        assert!(WinSuite::parse("rust").is_err());
        let names: Vec<&str> = WinSuite::ALL.iter().map(|suite| suite.name()).collect();
        assert_eq!(
            names,
            ["build", "clippy", "test", "smoke", "nsis", "install"],
            "the installer is made before it is installed"
        );
    }

    #[test]
    fn every_cargo_step_targets_windows() {
        for suite in WinSuite::ALL {
            for step in steps(suite, None) {
                if step.starts_with("cargo ") {
                    assert!(
                        step.starts_with("cargo xwin "),
                        "{step} goes through cargo-xwin"
                    );
                    assert!(step.contains(&format!("--target {TARGET}")), "{step}");
                    assert!(step.contains("--locked"), "{step}");
                }
            }
        }
        assert_eq!(
            steps(WinSuite::Clippy, None),
            [format!(
                "cargo xwin clippy --target {TARGET} --locked --workspace --all-targets \
                 -- -D warnings"
            )]
        );
    }

    #[test]
    fn the_build_embeds_the_ui() {
        let build = steps(WinSuite::Build, None);
        assert_eq!(build[0], "pnpm build");
        assert!(build[1].ends_with("--package gk-cli"));
        assert!(build[2].ends_with("--package gk-app --features tauri/custom-protocol"));
        assert!(WinSuite::Build.needs_node_modules());
        assert!(!WinSuite::Test.needs_node_modules());
    }

    #[test]
    fn the_tests_run_under_wine_in_a_fresh_prefix() {
        let test = WinSuite::Test.steps(None);
        assert_eq!(test[0], WinStep::FreshPrefix("test"));
        let shown = test[1].to_string();
        assert!(shown.starts_with(&format!("cargo xwin test --target {TARGET} --locked")));
        assert!(shown.contains("--no-fail-fast"));
        assert!(!shown.contains("--workspace"), "the crates, not the tools");
        let env = test_env(
            Path::new("/work"),
            Path::new("/tmp/gk-wine/test"),
            Path::new("/tmp/gk-wine/test-data"),
        );
        let get = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.as_str())
                .unwrap()
        };
        assert_eq!(get("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER"), "wine");
        assert_eq!(get("WINEPREFIX"), "/tmp/gk-wine/test");
        assert_eq!(get("INSTA_WORKSPACE_ROOT"), r"Z:\work");
        assert_eq!(get("GLYPHKEEL_DATA_DIR"), r"Z:\tmp\gk-wine\test-data\data");
        assert_eq!(
            get("GLYPHKEEL_CONFIG_DIR"),
            r"Z:\tmp\gk-wine\test-data\config"
        );
    }

    #[test]
    fn linux_paths_become_drive_z() {
        assert_eq!(wine_path(Path::new("/work")), r"Z:\work");
        assert_eq!(wine_path(Path::new("/a/b c/d.exe")), r"Z:\a\b c\d.exe");
    }

    #[test]
    fn the_smoke_and_install_checks_run_their_scripts() {
        assert_eq!(
            steps(WinSuite::Smoke, None)[1],
            format!("sh {SMOKE_SCRIPT} target/{TARGET}/debug/glyphkeel.exe")
        );
        assert_eq!(
            steps(WinSuite::Install, None),
            [format!("sh {INSTALL_SCRIPT} {}", installer().display())]
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for script in [SMOKE_SCRIPT, INSTALL_SCRIPT] {
            let text = std::fs::read_to_string(root.join(script)).unwrap();
            assert!(text.starts_with("#!/bin/sh\n"), "{script}");
            assert!(
                text.contains("wine-lib.sh"),
                "{script} uses the shared set-up"
            );
        }
        assert!(root.join("ci/test/wine-lib.sh").is_file());
    }

    #[test]
    fn the_installer_comes_from_the_dist_profile() {
        let nsis = steps(WinSuite::Nsis, None);
        assert_eq!(nsis.len(), 2, "nothing is copied without --release-dir");
        assert!(nsis[0].ends_with("--package gk-cli --profile dist"));
        assert!(nsis[1].starts_with("pnpm tauri build --ci --runner cargo-xwin"));
        assert!(nsis[1].contains(&format!("--target {TARGET} --bundles nsis")));
        assert!(nsis[1].contains(r#"--config {"bundle":{"active":true}}"#));
        assert!(nsis[1].ends_with("-- --profile dist"));
        assert_eq!(
            installer(),
            PathBuf::from(format!(
                "target/{TARGET}/dist/bundle/nsis/Glyphkeel_{VERSION}_x64-setup.exe"
            ))
        );
        let nsis = steps(WinSuite::Nsis, Some(Path::new("/release")));
        assert_eq!(
            nsis[2],
            format!("copy {} to /release/{VERSION}/windows", installer_name())
        );
        assert_eq!(
            release_folder(Path::new("/r")),
            PathBuf::from(format!("/r/{VERSION}/windows"))
        );
    }

    #[test]
    fn the_container_runs_the_chosen_suites_here() {
        let script = container_script(true, &["build", "smoke"], false);
        assert!(script.contains("--null --files-from=-"));
        assert!(script.ends_with(&format!(
            "exec cargo xtask ci --here --distro {NAME} --suite build,smoke </dev/null"
        )));
        let script = container_script(false, &["nsis"], true);
        assert!(script.contains("--exclude=./target"));
        assert!(script.ends_with("--suite nsis --release-dir /release </dev/null"));
        let mounts = cache_mounts();
        assert_eq!(mounts[0], (format!("target-{NAME}"), "/work/target"));
        assert!(mounts.iter().any(|(_, path)| *path == "/root/.cache/tauri"));
    }

    #[test]
    fn the_image_has_a_containerfile() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let file = root.join(format!("ci/containers/{NAME}.Containerfile"));
        let text = std::fs::read_to_string(&file).unwrap();
        for tool in ["cargo-xwin", "wine", "nsis", TARGET] {
            assert!(text.contains(tool), "{} installs {tool}", file.display());
        }
    }
}
