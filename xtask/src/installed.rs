// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The suites that check what ships, run from the host by `cargo xtask ci`:
//!
//! - `packages`, `offline` and `perf`: the deb and rpm are built once in the
//!   Ubuntu 22.04 image (the oldest glibc supported), with the seed tool's
//!   data sets. Each chosen distribution then gets a test image: its own
//!   clean base image with the package installed, as a user installs it, plus
//!   the test tools. The checks in `ci/test/` run in that image with no
//!   network (`--network none`, loopback only).
//! - `flatpak`: flatpak-builder on the host, with the manifest in
//!   `crates/gk-app/packaging/flatpak/`.
//!
//! Results and logs land in `<cache>/results/<distro>/<suite>/`; the
//! performance table is `perf.md` there.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::ci::{
    ContainerRun, DATA_DIR, Distro, Engine, Pins, Suite, build_image, prefix, run_in_container,
    status,
};

/// The build caches when `--cache-dir` is not given: the host suites need
/// the packages and data sets on the host, not in named volumes.
pub(crate) const DEFAULT_CACHE: &str = "target/ci-cache";

/// Where the packages are built: the oldest supported glibc.
const BUILD_DISTRO: Distro = Distro::Ubuntu2204;

/// The Flatpak's application ID and manifest.
const APP_ID: &str = "io.github.sorrelfox.Glyphkeel";
const FLATPAK_MANIFEST: &str = "crates/gk-app/packaging/flatpak/io.github.sorrelfox.Glyphkeel.yml";

/// The generated source lists the manifest names, next to it.
const FLATPAK_SOURCES: [&str; 2] = ["cargo-sources.json", "node-sources.json"];

/// The branch the manifest builds (its `default-branch`), the one the
/// published Flatpak repository carries.
const FLATPAK_BRANCH: &str = "stable";

/// Where the runtime comes from when the bundle is installed.
const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// The generated source lists that are not in `folder`.
fn missing_sources(folder: &Path) -> Vec<&'static str> {
    FLATPAK_SOURCES
        .into_iter()
        .filter(|name| !folder.join(name).is_file())
        .collect()
}

pub(crate) struct HostRun<'a> {
    pub(crate) root: &'a Path,
    pub(crate) engine: Engine,
    pub(crate) pins: &'a Pins,
    pub(crate) distros: &'a [Distro],
    pub(crate) suites: &'a [Suite],
    pub(crate) rebuild: bool,
    pub(crate) package_release: Option<u32>,
    pub(crate) perf_enforce: bool,
    pub(crate) cache_dir: PathBuf,
}

/// The packages of one build.
struct Packages {
    version: String,
    deb: PathBuf,
    rpm: PathBuf,
    data: PathBuf,
}

impl HostRun<'_> {
    /// Runs the host suites; returns what failed, as `suite on distro`.
    pub(crate) fn run(&self) -> Result<Vec<String>> {
        let mut failed = Vec::new();
        let on_packages: Vec<Suite> = self
            .suites
            .iter()
            .copied()
            .filter(|suite| matches!(suite, Suite::Packages | Suite::Offline | Suite::Perf))
            .collect();
        if !on_packages.is_empty() {
            match self.build_packages()? {
                Some(packages) => {
                    for &distro in self.distros {
                        failed.extend(self.check_installed(distro, &packages, &on_packages)?);
                    }
                }
                None => failed.push(format!("package build on {}", BUILD_DISTRO.name())),
            }
        }
        if self.suites.contains(&Suite::Flatpak) && !self.flatpak()? {
            failed.push("flatpak on the host".to_owned());
        }
        Ok(failed)
    }

    /// Builds the deb, the rpm and the data sets in the build image.
    fn build_packages(&self) -> Result<Option<Packages>> {
        let started = Instant::now();
        eprintln!(
            "==> packages: building the deb and rpm on {}",
            BUILD_DISTRO.name()
        );
        let built = build_image(
            self.root,
            self.engine,
            BUILD_DISTRO,
            self.pins,
            self.rebuild,
        )? && run_in_container(
            self.root,
            self.engine,
            BUILD_DISTRO,
            &ContainerRun {
                suites: &[Suite::Package, Suite::TestData],
                package_release: self.package_release,
                flows: &[],
                pseudo_locale: "",
                cache_dir: Some(&self.cache_dir),
            },
        )?;
        if !built {
            return Ok(None);
        }
        let target = self
            .cache_dir
            .join(format!("target-{}", BUILD_DISTRO.name()));
        let version = workspace_version(self.root)?;
        let release = self.package_release.unwrap_or(1);
        let bundle = target.join("dist/bundle");
        let packages = Packages {
            deb: bundle.join(format!("deb/Glyphkeel_{version}_amd64.deb")),
            rpm: bundle.join(format!("rpm/Glyphkeel-{version}-{release}.x86_64.rpm")),
            data: target.join(DATA_DIR.trim_start_matches("target/")),
            version,
        };
        for file in [&packages.deb, &packages.rpm] {
            if !file.is_file() {
                bail!("the build made no {}", file.display());
            }
        }
        eprintln!(
            "==> packages: built in {} s: {}, {}",
            started.elapsed().as_secs(),
            packages.deb.display(),
            packages.rpm.display()
        );
        Ok(Some(packages))
    }

    /// Installs the package in a clean image of `distro` and runs the
    /// suites' checks there.
    fn check_installed(
        &self,
        distro: Distro,
        packages: &Packages,
        suites: &[Suite],
    ) -> Result<Vec<String>> {
        let image = format!("{}-test:{}", prefix(), distro.name());
        eprintln!("==> {}: installing the package into {image}", distro.name());
        if !self.build_test_image(distro, packages, &image)? {
            return Ok(vec![format!("package install on {}", distro.name())]);
        }
        let mut failed = Vec::new();
        for &suite in suites {
            let started = Instant::now();
            let ok = self.run_check(distro, packages, suite, &image)?;
            let seconds = started.elapsed().as_secs();
            let out = self.results(distro, suite);
            if ok {
                eprintln!(
                    "==> {}: {} passed in {seconds} s ({})",
                    distro.name(),
                    suite.name(),
                    out.display()
                );
            } else {
                eprintln!(
                    "==> {}: {} FAILED after {seconds} s (logs: {})",
                    distro.name(),
                    suite.name(),
                    out.display()
                );
                failed.push(format!("{} on {}", suite.name(), distro.name()));
            }
        }
        Ok(failed)
    }

    fn results(&self, distro: Distro, suite: Suite) -> PathBuf {
        self.cache_dir
            .join("results")
            .join(distro.name())
            .join(suite.name())
    }

    /// The test image: the distribution's base image with the package and
    /// the test tools. Its build context holds only the package.
    fn build_test_image(&self, distro: Distro, packages: &Packages, image: &str) -> Result<bool> {
        let (package, file, containerfile) = if distro.uses_rpm() {
            (&packages.rpm, "glyphkeel.rpm", "test-rpm.Containerfile")
        } else {
            (&packages.deb, "glyphkeel.deb", "test-deb.Containerfile")
        };
        let context = self.cache_dir.join("stage").join(distro.name());
        if context.exists() {
            fs::remove_dir_all(&context)
                .with_context(|| format!("cannot remove {}", context.display()))?;
        }
        fs::create_dir_all(&context)
            .with_context(|| format!("cannot create {}", context.display()))?;
        fs::copy(package, context.join(file))
            .with_context(|| format!("cannot copy {}", package.display()))?;
        let mut cmd = Command::new(self.engine.program());
        cmd.arg("build")
            .arg("--file")
            .arg(self.root.join("ci/containers").join(containerfile))
            .arg("--tag")
            .arg(image)
            .arg("--build-arg")
            .arg(format!("BASE={}", distro.base_image()));
        if self.rebuild {
            cmd.arg("--no-cache");
        }
        cmd.arg(&context);
        status(&mut cmd)
    }

    /// One suite's script in the test image, with no network.
    fn run_check(
        &self,
        distro: Distro,
        packages: &Packages,
        suite: Suite,
        image: &str,
    ) -> Result<bool> {
        let out = self.results(distro, suite);
        if out.exists() {
            fs::remove_dir_all(&out).with_context(|| format!("cannot remove {}", out.display()))?;
        }
        fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
        let mut cmd = Command::new(self.engine.program());
        cmd.args(check_args(
            &CheckMounts {
                scripts: &self.root.join("ci/test"),
                data: &packages.data,
                out: &out,
            },
            &format!("{}-{}-{}", prefix(), suite.name(), distro.name()),
            &packages.version,
            self.perf_enforce,
        )?)
        .arg(image)
        .args(["sh", &format!("/ci/{}", script(suite))]);
        status(&mut cmd)
    }

    /// Builds the Flatpak with flatpak-builder and exports a bundle file.
    /// Without the generated source lists the suite is skipped, not failed:
    /// they are made from the lock files when a release is built.
    fn flatpak(&self) -> Result<bool> {
        // The offline sources Flathub's build needs, generated from the lock
        // files (see the manifest's header).
        let manifest = self.root.join(FLATPAK_MANIFEST);
        let folder = manifest.parent().context("the manifest has no folder")?;
        let missing = missing_sources(folder);
        if !missing.is_empty() {
            eprintln!(
                "==> flatpak: SKIPPED: {} not found in {}. They are generated from \
                 Cargo.lock and pnpm-lock.yaml with flatpak-builder-tools, as the \
                 manifest's header says; the release build makes them itself.",
                missing.join(" and "),
                folder.display()
            );
            return Ok(true);
        }
        let found = Command::new("flatpak-builder")
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        if !found {
            eprintln!("==> flatpak: flatpak-builder is not installed on this machine");
            return Ok(false);
        }
        let dir = self.cache_dir.join("flatpak");
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let started = Instant::now();
        let built = status(
            Command::new("flatpak-builder")
                .args(["--user", "--force-clean", "--ccache", "--disable-updates"])
                .arg("--install-deps-from=flathub")
                .arg(format!("--state-dir={}", dir.join("state").display()))
                .arg(format!("--repo={}", dir.join("repo").display()))
                .arg(dir.join("build"))
                .arg(&manifest),
        )?;
        let bundle = dir.join("glyphkeel.flatpak");
        let ok = built
            && status(
                Command::new("flatpak")
                    .arg("build-bundle")
                    .arg(format!("--runtime-repo={FLATHUB_REPO}"))
                    .arg(dir.join("repo"))
                    .arg(&bundle)
                    .arg(APP_ID)
                    .arg(FLATPAK_BRANCH),
            )?;
        let seconds = started.elapsed().as_secs();
        if ok {
            eprintln!("==> flatpak: built in {seconds} s: {}", bundle.display());
        } else {
            eprintln!("==> flatpak: FAILED after {seconds} s");
        }
        Ok(ok)
    }
}

/// The script in `ci/test/` that runs a suite.
fn script(suite: Suite) -> &'static str {
    match suite {
        Suite::Offline => "offline.sh",
        Suite::Perf => "perf.sh",
        _ => "smoke.sh",
    }
}

/// The host folders a check sees.
struct CheckMounts<'a> {
    scripts: &'a Path,
    data: &'a Path,
    out: &'a Path,
}

/// The `run` arguments of a check: no network but loopback, the scripts and
/// data read-only, the results folder writable, and room for WebKit's
/// shared memory. strace needs ptrace, which the default seccomp profile of
/// both engines allows for a process's own children.
fn check_args(
    mounts: &CheckMounts<'_>,
    name: &str,
    version: &str,
    perf_enforce: bool,
) -> Result<Vec<String>> {
    let path = |path: &Path| -> Result<String> {
        let path =
            fs::canonicalize(path).with_context(|| format!("cannot resolve {}", path.display()))?;
        path.to_str()
            .map(str::to_owned)
            .context("a path is not valid UTF-8")
    };
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "--init",
        "--network",
        "none",
        "--security-opt",
        "label=disable",
        "--shm-size",
        "1g",
        "--name",
        name,
    ]
    .map(str::to_owned)
    .to_vec();
    for (host, inside, mode) in [
        (mounts.scripts, "/ci", "ro"),
        (mounts.data, "/data", "ro"),
        (mounts.out, "/out", "rw"),
    ] {
        args.push("--volume".to_owned());
        args.push(format!("{}:{inside}:{mode}", path(host)?));
    }
    args.push("--env".to_owned());
    args.push(format!("EXPECTED_VERSION={version}"));
    if perf_enforce {
        args.push("--env".to_owned());
        args.push("PERF_ENFORCE=1".to_owned());
    }
    Ok(args)
}

/// The workspace version from Cargo.toml's `[workspace.package]`.
fn workspace_version(root: &Path) -> Result<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).context("cannot read Cargo.toml")?;
    parse_workspace_version(&text).context("Cargo.toml has no [workspace.package] version")
}

fn parse_workspace_version(text: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[workspace.package]";
        } else if in_section
            && let Some(value) = line
                .strip_prefix("version")
                .and_then(|rest| rest.trim().strip_prefix('='))
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CheckMounts, check_args, parse_workspace_version, script};
    use crate::ci::Suite;

    #[test]
    fn the_version_comes_from_the_workspace_section() {
        let text = "[package]\nversion = \"9.9.9\"\n[workspace.package]\nedition = \"2024\"\nversion = \"0.0.7\"\n[x]\nversion = \"1\"\n";
        assert_eq!(parse_workspace_version(text).as_deref(), Some("0.0.7"));
        assert_eq!(
            parse_workspace_version("[package]\nversion = \"1\"\n"),
            None
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert!(super::workspace_version(&root).is_ok());
    }

    #[test]
    fn checks_run_without_network_and_see_only_their_folders() {
        let dir = std::env::temp_dir();
        let mounts = CheckMounts {
            scripts: &dir,
            data: &dir,
            out: &dir,
        };
        let args = check_args(&mounts, "x-offline-fedora", "0.0.7", false).unwrap();
        let joined = args.join(" ");
        assert!(joined.contains("--network none"));
        assert!(joined.contains(":/ci:ro"));
        assert!(joined.contains(":/data:ro"));
        assert!(joined.contains(":/out:rw"));
        assert!(joined.contains("EXPECTED_VERSION=0.0.7"));
        assert!(!joined.contains("PERF_ENFORCE"));
        assert!(
            check_args(&mounts, "x", "1", true)
                .unwrap()
                .contains(&"PERF_ENFORCE=1".to_owned())
        );
    }

    #[test]
    fn every_check_has_its_script() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ci/test");
        for suite in [Suite::Packages, Suite::Offline, Suite::Perf] {
            assert!(root.join(script(suite)).is_file(), "{}", script(suite));
        }
        for file in ["lib.sh", "gk.py", "connect_check.py", "offline-session.sh"] {
            assert!(root.join(file).is_file(), "{file}");
        }
        let containers = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ci/containers");
        for file in ["test-deb.Containerfile", "test-rpm.Containerfile"] {
            assert!(containers.join(file).is_file(), "{file}");
        }
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(super::FLATPAK_MANIFEST)
                .is_file()
        );
    }

    #[test]
    fn missing_flatpak_sources_are_named() {
        let dir = std::env::temp_dir().join(format!("gk-flatpak-sources-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            super::missing_sources(&dir),
            ["cargo-sources.json", "node-sources.json"]
        );
        std::fs::write(dir.join("cargo-sources.json"), "[]").unwrap();
        assert_eq!(super::missing_sources(&dir), ["node-sources.json"]);
        std::fs::write(dir.join("node-sources.json"), "[]").unwrap();
        assert!(super::missing_sources(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_manifest_builds_the_published_branch() {
        let manifest = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(super::FLATPAK_MANIFEST),
        )
        .unwrap();
        assert!(manifest.contains(&format!("\ndefault-branch: {}\n", super::FLATPAK_BRANCH)));
    }
}
