// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask ci --distro arch`: the Arch Linux package, from the host.
//!
//! Its one suite, `package`:
//! 1. writes the source tarball (`cargo xtask source-tarball`) and the
//!    PKGBUILD of `crates/gk-app/packaging/arch/`, filled in with the
//!    version, the release number and the tarball's checksum;
//! 2. builds it with makepkg in the image of
//!    `ci/containers/arch.Containerfile` (`ci/arch/build.sh`), which also
//!    writes the .SRCINFO and runs namcap on the PKGBUILD and the package;
//! 3. packs the PKGBUILD and .SRCINFO as `glyphkeel-<version>-aur.tar.gz`,
//!    ready for the AUR;
//! 4. installs the package in a clean Arch Linux image
//!    (`ci/containers/test-arch.Containerfile`) and runs the checks of
//!    `ci/test/smoke.sh` and `ci/test/mcp.sh` there with no network.
//!
//! Everything lands in `<cache>/arch/`: `out/` holds the package, the
//! .SRCINFO, namcap's reports and the AUR archive, `results/` the logs of the
//! checks.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::ci::{self, Engine, NO_CORE_DUMPS, Pins};

/// The name `--distro` takes.
pub(crate) const NAME: &str = "arch";

/// The only suite of the Arch Linux build.
pub(crate) const SUITE: &str = "package";

/// The PKGBUILD template, relative to the repository.
pub(crate) const PKGBUILD: &str = "crates/gk-app/packaging/arch/PKGBUILD";

/// The build script, relative to the repository.
const BUILD_SCRIPTS: &str = "ci/arch";

/// The workspace version, which the package carries.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the host run of the Arch Linux build needs.
pub(crate) struct ArchRun<'a> {
    pub(crate) root: &'a Path,
    pub(crate) engine: Engine,
    pub(crate) pins: &'a Pins,
    pub(crate) rebuild: bool,
    /// The package's release number (`pkgrel`); 1 when not given.
    pub(crate) package_release: Option<u32>,
    pub(crate) cache_dir: PathBuf,
}

impl ArchRun<'_> {
    pub(crate) fn run(&self) -> Result<bool> {
        let started = Instant::now();
        let dir = self.cache_dir.join(NAME);
        let input = dir.join("in");
        let out = dir.join("out");
        for folder in [&input, &out] {
            if folder.exists() {
                fs::remove_dir_all(folder)
                    .with_context(|| format!("cannot remove {}", folder.display()))?;
            }
            fs::create_dir_all(folder)
                .with_context(|| format!("cannot create {}", folder.display()))?;
        }

        eprintln!("==> {NAME}: the source tarball and the PKGBUILD");
        let tarball = crate::source::write(self.root, &input)?;
        let sum = sha256(&tarball)?;
        let template = fs::read_to_string(self.root.join(PKGBUILD))
            .with_context(|| format!("cannot read {PKGBUILD}"))?;
        let pkgbuild = fill_pkgbuild(&template, VERSION, self.package_release.unwrap_or(1), &sum)?;
        fs::write(input.join("PKGBUILD"), pkgbuild).context("cannot write the PKGBUILD")?;

        eprintln!("==> {NAME}: building the image {}:{NAME}", ci::prefix());
        if !ci::build_named_image(self.root, self.engine, NAME, self.pins, self.rebuild)? {
            eprintln!("==> {NAME}: FAILED to build the image");
            return Ok(false);
        }
        eprintln!("==> {NAME}: makepkg and namcap");
        if !self.build(&input, &out)? {
            eprintln!(
                "==> {NAME}: the package build FAILED (namcap's reports: {})",
                out.display()
            );
            return Ok(false);
        }
        let package = out.join(package_name(self.package_release.unwrap_or(1)));
        if !package.is_file() {
            bail!("the build made no {}", package.display());
        }
        let aur = aur_archive(&out)?;

        eprintln!("==> {NAME}: installing the package in a clean image");
        let image = format!("{}-test:{NAME}", ci::prefix());
        if !self.build_test_image(&package, &dir.join("stage"), &image)? {
            eprintln!("==> {NAME}: FAILED to install the package");
            return Ok(false);
        }
        let results = dir.join("results");
        let ok = self.check(&image, &dir.join("empty"), &results)?;
        let seconds = started.elapsed().as_secs();
        if ok {
            eprintln!(
                "==> {NAME}: passed in {seconds} s: {}, {}",
                package.display(),
                aur.display()
            );
        } else {
            eprintln!(
                "==> {NAME}: the checks of the installed package FAILED after {seconds} s \
                 (logs: {})",
                results.display()
            );
        }
        Ok(ok)
    }

    /// makepkg in the build image: the PKGBUILD and tarball from `input`,
    /// the results into `out`. The cargo registry, the pnpm store and
    /// pacman's downloads are kept between runs.
    fn build(&self, input: &Path, out: &Path) -> Result<bool> {
        let mut cmd = Command::new(self.engine.program());
        cmd.args(["run", "--rm", "--init"])
            .args(NO_CORE_DUMPS)
            .args(["--security-opt", "label=disable"])
            .args(["--name", &format!("{}-{NAME}-build", ci::prefix())]);
        for (host, inside, mode) in [
            (self.root.join(BUILD_SCRIPTS), "/ci", "ro"),
            (input.to_path_buf(), "/in", "ro"),
            (out.to_path_buf(), "/out", "rw"),
        ] {
            cmd.arg("--volume")
                .arg(format!("{}:{inside}:{mode}", canonical(&host)?));
        }
        for volume in ci::named_volumes(&cache_mounts(), None)? {
            cmd.arg("--volume").arg(volume);
        }
        for name in ["CARGO_BUILD_JOBS", "CARGO_TERM_COLOR"] {
            if std::env::var_os(name).is_some() {
                cmd.args(["--env", name]);
            }
        }
        cmd.arg(format!("{}:{NAME}", ci::prefix()))
            .args(["sh", "/ci/build.sh"]);
        ci::status(&mut cmd)
    }

    /// The clean image with the package installed; its build context holds
    /// only the package.
    fn build_test_image(&self, package: &Path, context: &Path, image: &str) -> Result<bool> {
        if context.exists() {
            fs::remove_dir_all(context)
                .with_context(|| format!("cannot remove {}", context.display()))?;
        }
        fs::create_dir_all(context)
            .with_context(|| format!("cannot create {}", context.display()))?;
        fs::copy(package, context.join("glyphkeel.pkg.tar.zst"))
            .with_context(|| format!("cannot copy {}", package.display()))?;
        let mut cmd = Command::new(self.engine.program());
        cmd.arg("build")
            .args(NO_CORE_DUMPS)
            .arg("--file")
            .arg(self.root.join("ci/containers/test-arch.Containerfile"))
            .arg("--tag")
            .arg(image);
        if self.rebuild {
            cmd.arg("--no-cache");
        }
        cmd.arg(context);
        ci::status(&mut cmd)
    }

    /// The checks of the installed package, with no network.
    fn check(&self, image: &str, empty: &Path, results: &Path) -> Result<bool> {
        for folder in [empty, results] {
            if folder.exists() {
                fs::remove_dir_all(folder)
                    .with_context(|| format!("cannot remove {}", folder.display()))?;
            }
            fs::create_dir_all(folder)
                .with_context(|| format!("cannot create {}", folder.display()))?;
        }
        let mut cmd = Command::new(self.engine.program());
        cmd.args(crate::installed::check_args(
            &crate::installed::CheckMounts {
                scripts: &self.root.join("ci/test"),
                data: empty,
                out: results,
            },
            &format!("{}-{NAME}-check", ci::prefix()),
            VERSION,
            false,
        )?)
        .arg(image)
        .args(["sh", "-c", CHECKS]);
        ci::status(&mut cmd)
    }
}

/// The checks run in the clean image: the files and a command-line session,
/// then an MCP session.
const CHECKS: &str = "sh /ci/smoke.sh && sh /ci/mcp.sh";

/// The caches of the build container: named volumes, so that nothing in
/// them belongs to a host user (the build runs as the container's own user).
fn cache_mounts() -> Vec<(String, &'static str)> {
    vec![
        (
            "arch-cargo-registry".to_owned(),
            "/home/builder/.cargo/registry",
        ),
        (
            "arch-pnpm-store".to_owned(),
            "/home/builder/.local/share/pnpm/store",
        ),
        ("arch-pacman-cache".to_owned(), "/var/cache/pacman/pkg"),
    ]
}

/// The package file makepkg writes.
fn package_name(release: u32) -> String {
    format!("glyphkeel-{VERSION}-{release}-x86_64.pkg.tar.zst")
}

/// The PKGBUILD with this version, release number and source checksum.
/// Each of the three lines must be in the template exactly once.
pub(crate) fn fill_pkgbuild(
    template: &str,
    version: &str,
    release: u32,
    sha256: &str,
) -> Result<String> {
    let mut found = [0_usize; 3];
    let mut lines = Vec::new();
    for line in template.lines() {
        let line = if line.starts_with("pkgver=") {
            found[0] += 1;
            format!("pkgver={version}")
        } else if line.starts_with("pkgrel=") {
            found[1] += 1;
            format!("pkgrel={release}")
        } else if line.starts_with("sha256sums=") {
            found[2] += 1;
            format!("sha256sums=('{sha256}')")
        } else {
            line.to_owned()
        };
        lines.push(line);
    }
    for (count, name) in found.iter().zip(["pkgver", "pkgrel", "sha256sums"]) {
        if *count != 1 {
            bail!("{PKGBUILD} must set {name} once, on a line of its own");
        }
    }
    Ok(lines.join("\n") + "\n")
}

/// The PKGBUILD and .SRCINFO in `out`, packed as an AUR repository's files
/// under `glyphkeel/`.
fn aur_archive(out: &Path) -> Result<PathBuf> {
    let stage = out.join("aur").join("glyphkeel");
    fs::create_dir_all(&stage).with_context(|| format!("cannot create {}", stage.display()))?;
    for name in ["PKGBUILD", ".SRCINFO"] {
        fs::copy(out.join(name), stage.join(name))
            .with_context(|| format!("the build wrote no {name}"))?;
    }
    let file = out.join(format!("glyphkeel-{VERSION}-aur.tar.gz"));
    let ok = ci::status(
        Command::new("tar")
            .args([
                "--sort=name",
                "--owner=0",
                "--group=0",
                "--numeric-owner",
                "-czf",
            ])
            .arg(&file)
            .arg("-C")
            .arg(out.join("aur"))
            .arg("glyphkeel"),
    )?;
    if !ok {
        bail!("cannot write {}", file.display());
    }
    Ok(file)
}

/// The SHA-256 of a file, from coreutils' sha256sum.
fn sha256(file: &Path) -> Result<String> {
    let out = Command::new("sha256sum")
        .arg(file)
        .output()
        .context("cannot run sha256sum")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let sum = text.split_whitespace().next().unwrap_or_default();
    if !out.status.success() || sum.len() != 64 {
        bail!("sha256sum failed for {}", file.display());
    }
    Ok(sum.to_owned())
}

fn canonical(path: &Path) -> Result<String> {
    let path =
        fs::canonicalize(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    path.to_str()
        .map(str::to_owned)
        .context("a path is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{PKGBUILD, fill_pkgbuild, package_name};

    fn template() -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(PKGBUILD),
        )
        .unwrap()
    }

    #[test]
    fn the_pkgbuild_is_filled_in() {
        let sum = "a".repeat(64);
        let filled = fill_pkgbuild(&template(), "9.8.7", 3, &sum).unwrap();
        assert!(filled.contains("\npkgver=9.8.7\n"));
        assert!(filled.contains("\npkgrel=3\n"));
        assert!(filled.contains(&format!("\nsha256sums=('{sum}')\n")));
        assert!(!filled.contains("SKIP"));
    }

    #[test]
    fn a_template_without_the_lines_is_refused() {
        assert!(fill_pkgbuild("pkgname=x\npkgrel=1\n", "1.0.0", 1, "0").is_err());
        assert!(
            fill_pkgbuild("pkgver=1\npkgver=2\npkgrel=1\nsha256sums=()\n", "1", 1, "0").is_err()
        );
    }

    #[test]
    fn the_pkgbuild_builds_the_release_tarball_and_installs_the_legal_files() {
        let text = template();
        assert!(text.contains("releases/download/v$pkgver/$pkgname-$pkgver.tar.gz"));
        assert!(text.contains("license=('GPL-3.0-or-later')"));
        for file in [
            "/usr/share/licenses/$pkgname/LICENSE",
            "/usr/share/licenses/$pkgname/THIRD-PARTY-NOTICES.txt",
            "/usr/bin/glyphkeel-app",
            "/usr/bin/glyphkeel\"",
            "/usr/share/metainfo/io.github.sorrelfox.Glyphkeel.metainfo.xml",
            "/usr/share/applications/Glyphkeel.desktop",
        ] {
            assert!(text.contains(file), "the PKGBUILD installs no {file}");
        }
    }

    #[test]
    fn the_package_is_named_as_makepkg_names_it() {
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            package_name(2),
            format!("glyphkeel-{version}-2-x86_64.pkg.tar.zst")
        );
    }
}
