// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask repos`: the signed APT and DNF repositories of a release,
//! made from its deb and rpm, as files ready to be served from any static
//! web host (the release workflow puts them on GitHub Pages).
//!
//! ```text
//! OUT/apt/                       an APT repository, suite `stable`, component `main`
//!   dists/stable/InRelease       the Release file, signed inline
//!   dists/stable/Release(.gpg)   the Release file and its detached signature
//!   dists/stable/main/binary-amd64/Packages(.gz)
//!   pool/main/g/glyphkeel/glyphkeel_<version>_amd64.deb
//!   glyphkeel.gpg                the public key, for `Signed-By`
//!   glyphkeel.sources            the source entry for /etc/apt/sources.list.d/
//! OUT/rpm/                       a DNF repository
//!   glyphkeel-<version>-<release>.x86_64.rpm   signed with the same key
//!   repodata/repomd.xml(.asc)    the metadata and its detached signature
//!   glyphkeel.asc                the public key, ASCII-armoured
//!   glyphkeel.repo               the entry for /etc/yum.repos.d/
//! ```
//!
//! The tools come from the system: apt-ftparchive (apt-utils), dpkg-deb,
//! createrepo_c, rpm and rpmsign, gpg and gzip. The key is the one in gpg's
//! keyring (`GNUPGHOME`) that `--key` or `GLYPHKEEL_REPO_KEY` names, or the
//! only secret key there; it must sign without a passphrase. The rpm is
//! signed in the repository's copy only: the release's own file stays as it
//! was built.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::ci::{self, Distro, Engine, NO_CORE_DUMPS, Pins};

const HELP: &str = "\
Usage: cargo xtask repos --deb FILE --rpm FILE --out DIR [--key KEY] [--site-url URL]

Builds the signed APT repository in DIR/apt and the signed DNF repository in
DIR/rpm from one deb and one rpm, replacing what was there.

Options:
  --deb FILE      The deb package.
  --rpm FILE      The rpm package. Its copy in the repository is signed.
  --out DIR       Where the repositories are written.
  --key KEY       The signing key in gpg's keyring (GNUPGHOME): a fingerprint,
                  ID or user ID. Default: GLYPHKEEL_REPO_KEY, else the only
                  secret key in the keyring. It must have no passphrase.
  --site-url URL  Where DIR is served, for the source and .repo files.
                  Default: https://sorrelfox.github.io/glyphkeel

Needs apt-ftparchive, dpkg-deb, createrepo_c, rpm, rpmsign, gpg and gzip.
";

/// Where the release workflow publishes the repositories.
const SITE_URL: &str = "https://sorrelfox.github.io/glyphkeel";

/// The APT suite and component; one of each, for every Debian and Ubuntu
/// release the deb supports.
const SUITE: &str = "stable";
const COMPONENT: &str = "main";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    deb: PathBuf,
    rpm: PathBuf,
    out: PathBuf,
    key: Option<String>,
    site_url: String,
}

impl Options {
    /// The options, or `None` for `--help`.
    fn parse(args: &[String]) -> Result<Option<Self>> {
        let (mut deb, mut rpm, mut out, mut key) = (None, None, None, None);
        let mut site_url = SITE_URL.to_owned();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .cloned()
                    .with_context(|| format!("`{arg}` needs a value"))
            };
            match arg.as_str() {
                "--deb" => deb = Some(PathBuf::from(value()?)),
                "--rpm" => rpm = Some(PathBuf::from(value()?)),
                "--out" => out = Some(PathBuf::from(value()?)),
                "--key" => key = Some(value()?),
                "--site-url" => site_url = value()?.trim_end_matches('/').to_owned(),
                "-h" | "--help" => return Ok(None),
                other => bail!("unknown option `{other}`\n\n{HELP}"),
            }
        }
        let key = key.or_else(|| {
            std::env::var("GLYPHKEEL_REPO_KEY")
                .ok()
                .filter(|key| !key.is_empty())
        });
        Ok(Some(Self {
            deb: deb.context("`--deb` is required")?,
            rpm: rpm.context("`--rpm` is required")?,
            out: out.context("`--out` is required")?,
            key,
            site_url,
        }))
    }
}

pub(crate) fn run(args: &[String]) -> Result<bool> {
    let Some(options) = Options::parse(args)? else {
        eprint!("{HELP}");
        return Ok(true);
    };
    for tool in [
        "apt-ftparchive",
        "dpkg-deb",
        "createrepo_c",
        "rpm",
        "rpmsign",
        "gpg",
        "gzip",
    ] {
        if !found(tool) {
            bail!("{tool} is not installed; see `cargo xtask repos --help`");
        }
    }
    let key = signing_key(options.key.as_deref())?;
    eprintln!("repos: signing with {key}");
    for dir in ["apt", "rpm"] {
        let dir = options.out.join(dir);
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("cannot remove {}", dir.display()))?;
        }
    }
    let apt = options.out.join("apt");
    let rpm = options.out.join("rpm");
    build_apt(&options.deb, &apt, &key, &options.site_url)?;
    build_rpm(&options.rpm, &rpm, &key, &options.site_url)?;
    eprintln!("repos: {} and {}", apt.display(), rpm.display());
    Ok(true)
}

fn found(tool: &str) -> bool {
    tool_path(tool).is_ok()
}

/// Where a tool is on the `PATH`.
fn tool_path(tool: &str) -> Result<String> {
    let path = output(Command::new("sh").args(["-c", &format!("command -v {tool}")]))?;
    let path = path.trim();
    if path.is_empty() {
        bail!("{tool} is not on the PATH");
    }
    Ok(path.to_owned())
}

/// The fingerprint of the signing key: the one named, or the only secret
/// key. It is checked to sign without asking for a passphrase.
fn signing_key(named: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("gpg");
    cmd.args(["--batch", "--with-colons", "--list-secret-keys"]);
    if let Some(key) = named {
        cmd.arg(key);
    }
    let listing = output(&mut cmd).context("gpg found no secret key to sign with")?;
    let keys = fingerprints(&listing);
    let key = match keys.as_slice() {
        [key] => key.clone(),
        [] => bail!("gpg's keyring has no secret key; import the signing key first"),
        _ => bail!(
            "gpg's keyring has {} secret keys; name one with --key or GLYPHKEEL_REPO_KEY",
            keys.len()
        ),
    };
    let signs = Command::new("gpg")
        .args([
            "--batch",
            "--yes",
            "--pinentry-mode",
            "error",
            "--local-user",
        ])
        .arg(&key)
        .args(["--output", "/dev/null", "--sign", "/dev/null"])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !signs {
        bail!("the key {key} can't sign without a passphrase; use a copy without one");
    }
    Ok(key)
}

/// The primary keys' fingerprints in `gpg --with-colons` output: the `fpr`
/// record right after each `sec` record.
fn fingerprints(listing: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut after_sec = false;
    for line in listing.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first() {
            Some(&"sec") => after_sec = true,
            Some(&"fpr") if after_sec => {
                if let Some(fpr) = fields.get(9).filter(|fpr| !fpr.is_empty()) {
                    keys.push((*fpr).to_owned());
                }
                after_sec = false;
            }
            _ => {}
        }
    }
    keys
}

fn build_apt(deb: &Path, apt: &Path, key: &str, site_url: &str) -> Result<()> {
    let field = |name: &str| -> Result<String> {
        Ok(
            output(Command::new("dpkg-deb").arg("--field").arg(deb).arg(name))?
                .trim()
                .to_owned(),
        )
    };
    let (name, version, arch) = (field("Package")?, field("Version")?, field("Architecture")?);
    let pool = apt
        .join("pool")
        .join(COMPONENT)
        .join(pool_letter(&name))
        .join(&name);
    fs::create_dir_all(&pool).with_context(|| format!("cannot create {}", pool.display()))?;
    fs::copy(deb, pool.join(format!("{name}_{version}_{arch}.deb")))
        .with_context(|| format!("cannot copy {}", deb.display()))?;

    let dist = apt.join("dists").join(SUITE);
    let binary = dist.join(COMPONENT).join(format!("binary-{arch}"));
    fs::create_dir_all(&binary).with_context(|| format!("cannot create {}", binary.display()))?;
    // Paths in Packages are relative to the repository's root.
    let packages = output(
        Command::new("apt-ftparchive")
            .args(["packages", "pool"])
            .current_dir(apt),
    )?;
    fs::write(binary.join("Packages"), &packages).context("cannot write Packages")?;
    checked(
        Command::new("gzip")
            .args(["-9", "-n", "-k", "-f", "Packages"])
            .current_dir(&binary),
    )?;

    let mut release = Command::new("apt-ftparchive");
    for (field, value) in release_fields(&arch) {
        release
            .arg("-o")
            .arg(format!("APT::FTPArchive::Release::{field}={value}"));
    }
    let release = output(release.arg("release").arg(".").current_dir(&dist))?;
    fs::write(dist.join("Release"), release).context("cannot write Release")?;
    gpg_sign(
        key,
        &["--clearsign"],
        &dist.join("Release"),
        &dist.join("InRelease"),
    )?;
    gpg_sign(
        key,
        &["--armor", "--detach-sign"],
        &dist.join("Release"),
        &dist.join("Release.gpg"),
    )?;

    export_key(key, false, &apt.join("glyphkeel.gpg"))?;
    fs::write(apt.join("glyphkeel.sources"), apt_sources(site_url, &arch))
        .context("cannot write glyphkeel.sources")?;
    Ok(())
}

/// The pool folder of a source package: `libfoo` under `libf`, else its
/// first letter, as Debian lays out its archive.
fn pool_letter(name: &str) -> String {
    if name.starts_with("lib") && name.len() > 3 {
        name[..4].to_owned()
    } else {
        name.chars().next().map(String::from).unwrap_or_default()
    }
}

/// The Release file's own fields; apt-ftparchive adds the date and the
/// checksums.
fn release_fields(arch: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Origin", "Glyphkeel".to_owned()),
        ("Label", "Glyphkeel".to_owned()),
        ("Suite", SUITE.to_owned()),
        ("Codename", SUITE.to_owned()),
        ("Architectures", arch.to_owned()),
        ("Components", COMPONENT.to_owned()),
        ("Description", "Glyphkeel, the latest release".to_owned()),
    ]
}

/// The deb822 source entry users put in /etc/apt/sources.list.d/, with the
/// key from glyphkeel.gpg in /etc/apt/keyrings/.
fn apt_sources(site_url: &str, arch: &str) -> String {
    format!(
        "Types: deb\nURIs: {site_url}/apt/\nSuites: {SUITE}\nComponents: {COMPONENT}\n\
         Architectures: {arch}\nSigned-By: /etc/apt/keyrings/glyphkeel.gpg\n"
    )
}

fn build_rpm(rpm: &Path, dir: &Path, key: &str, site_url: &str) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let name = output(
        Command::new("rpm")
            .args([
                "-qp",
                "--queryformat",
                "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}.rpm",
            ])
            .arg(rpm),
    )?;
    let copy = dir.join(name.trim());
    fs::copy(rpm, &copy).with_context(|| format!("cannot copy {}", rpm.display()))?;
    // rpm's own default for gpg differs between distributions (gpg2 on
    // some), so it is given the one on the PATH.
    checked(
        Command::new("rpmsign")
            .arg("--addsign")
            .arg("--define")
            .arg(format!("__gpg {}", tool_path("gpg")?))
            .arg("--define")
            .arg(format!("_gpg_name {key}"))
            .arg("--define")
            .arg("_gpg_sign_cmd_extra_args --batch --pinentry-mode error")
            .arg(&copy),
    )?;
    checked(
        Command::new("createrepo_c")
            .args(["--no-database", "--simple-md-filenames"])
            .arg(dir),
    )?;
    let repomd = dir.join("repodata/repomd.xml");
    gpg_sign(
        key,
        &["--armor", "--detach-sign"],
        &repomd,
        &dir.join("repodata/repomd.xml.asc"),
    )?;
    export_key(key, true, &dir.join("glyphkeel.asc"))?;
    fs::write(dir.join("glyphkeel.repo"), dnf_repo(site_url))
        .context("cannot write glyphkeel.repo")?;
    Ok(())
}

/// The entry users put in /etc/yum.repos.d/: the packages and the metadata
/// are both checked against the key.
fn dnf_repo(site_url: &str) -> String {
    format!(
        "[glyphkeel]\nname=Glyphkeel\nbaseurl={site_url}/rpm/\nenabled=1\ngpgcheck=1\n\
         repo_gpgcheck=1\ngpgkey={site_url}/rpm/glyphkeel.asc\n"
    )
}

fn gpg_sign(key: &str, how: &[&str], input: &Path, output_file: &Path) -> Result<()> {
    checked(
        Command::new("gpg")
            .args([
                "--batch",
                "--yes",
                "--pinentry-mode",
                "error",
                "--digest-algo",
                "SHA512",
            ])
            .arg("--local-user")
            .arg(key)
            .args(how)
            .arg("--output")
            .arg(output_file)
            .arg(input),
    )
}

fn export_key(key: &str, armor: bool, file: &Path) -> Result<()> {
    let mut cmd = Command::new("gpg");
    cmd.args(["--batch", "--yes", "--export-options", "export-minimal"]);
    if armor {
        cmd.arg("--armor");
    }
    checked(cmd.arg("--output").arg(file).arg("--export").arg(key))
}

fn checked(cmd: &mut Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let status = cmd
        .status()
        .with_context(|| format!("cannot run {shown}"))?;
    if !status.success() {
        bail!("{shown} failed");
    }
    Ok(())
}

fn output(cmd: &mut Command) -> Result<String> {
    let shown = format!("{cmd:?}");
    let out = cmd
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("cannot run {shown}"))?;
    if !out.status.success() {
        bail!("{shown} failed");
    }
    String::from_utf8(out.stdout).with_context(|| format!("{shown} wrote text that isn't UTF-8"))
}

/// The one deb and the one rpm in `dir`.
pub(crate) fn packages_in(dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let mut debs = Vec::new();
    let mut rpms = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("cannot list {}", dir.display()))? {
        let path = entry?.path();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("deb") => debs.push(path),
            Some("rpm") => rpms.push(path),
            _ => {}
        }
    }
    match (debs.as_slice(), rpms.as_slice()) {
        ([deb], [rpm]) => Ok((deb.clone(), rpm.clone())),
        _ => bail!(
            "{} must hold one deb and one rpm (found {} and {})",
            dir.display(),
            debs.len(),
            rpms.len()
        ),
    }
}

/// The `repos` suite of `cargo xtask ci`, from the host: the repositories
/// built in the image of `ci/containers/repos.Containerfile` with a
/// throwaway key, then added and installed from in a clean container of each
/// distribution (`ci/test/repo-apt.sh`, `ci/test/repo-dnf.sh`). Those
/// containers have the network, for the package's dependencies.
pub(crate) struct Check<'a> {
    pub(crate) root: &'a Path,
    pub(crate) engine: Engine,
    pub(crate) pins: &'a Pins,
    pub(crate) distros: &'a [Distro],
    pub(crate) rebuild: bool,
    pub(crate) cache_dir: &'a Path,
    pub(crate) deb: &'a Path,
    pub(crate) rpm: &'a Path,
}

/// The image's name, under the CI prefix.
const IMAGE: &str = "repos";

impl Check<'_> {
    /// Runs the suite; returns what failed.
    pub(crate) fn run(&self) -> Result<Vec<String>> {
        let dir = self.cache_dir.join("repos");
        let input = dir.join("packages");
        let site = dir.join("site");
        for folder in [&input, &site] {
            if folder.exists() {
                fs::remove_dir_all(folder)
                    .with_context(|| format!("cannot remove {}", folder.display()))?;
            }
            fs::create_dir_all(folder)
                .with_context(|| format!("cannot create {}", folder.display()))?;
        }
        fs::copy(self.deb, input.join("glyphkeel.deb"))
            .with_context(|| format!("cannot copy {}", self.deb.display()))?;
        fs::copy(self.rpm, input.join("glyphkeel.rpm"))
            .with_context(|| format!("cannot copy {}", self.rpm.display()))?;

        eprintln!("==> repos: building the image {}:{IMAGE}", ci::prefix());
        if !ci::build_named_image(self.root, self.engine, IMAGE, self.pins, self.rebuild)? {
            return Ok(vec!["repos: the image".to_owned()]);
        }
        eprintln!("==> repos: the repositories, signed with a throwaway key");
        if !self.build(&input, &site)? {
            return Ok(vec!["repos: building the repositories".to_owned()]);
        }
        let mut failed = Vec::new();
        for &distro in self.distros {
            let started = Instant::now();
            let out = dir.join("results").join(distro.name());
            if out.exists() {
                fs::remove_dir_all(&out)
                    .with_context(|| format!("cannot remove {}", out.display()))?;
            }
            fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
            let script = if distro.uses_rpm() {
                "repo-dnf.sh"
            } else {
                "repo-apt.sh"
            };
            let mut cmd = Command::new(self.engine.program());
            cmd.args(["run", "--rm", "--init"])
                .args(NO_CORE_DUMPS)
                .args(["--security-opt", "label=disable"])
                .args([
                    "--name",
                    &format!("{}-repos-{}", ci::prefix(), distro.name()),
                ]);
            for (host, inside, mode) in [
                (self.root.join("ci/test"), "/ci", "ro"),
                (site.clone(), "/repo", "ro"),
                (out.clone(), "/out", "rw"),
            ] {
                cmd.arg("--volume")
                    .arg(format!("{}:{inside}:{mode}", canonical(&host)?));
            }
            cmd.arg(distro.base_image())
                .args(["sh", &format!("/ci/{script}")]);
            let ok = ci::status(&mut cmd)?;
            let seconds = started.elapsed().as_secs();
            if ok {
                eprintln!("==> {}: repos passed in {seconds} s", distro.name());
            } else {
                eprintln!(
                    "==> {}: repos FAILED after {seconds} s (logs: {})",
                    distro.name(),
                    out.display()
                );
                failed.push(format!("repos on {}", distro.name()));
            }
        }
        Ok(failed)
    }

    /// `cargo xtask repos` in the image, with a key made for this run only.
    fn build(&self, input: &Path, site: &Path) -> Result<bool> {
        let files = crate::headers::git_files(self.root);
        let source = self
            .root
            .to_str()
            .context("the repository path is not valid UTF-8")?;
        let mut cmd = ci::container_command(self.engine, source);
        cmd.arg("--volume")
            .arg(format!("{}:/packages:ro", canonical(input)?))
            .arg("--volume")
            .arg(format!("{}:/site", canonical(site)?));
        for volume in ci::named_volumes(
            &[
                ("target-repos".to_owned(), "/work/target"),
                ("cargo-registry".to_owned(), "/usr/local/cargo/registry"),
            ],
            // Named volumes: the build output is not needed on the host.
            None,
        )? {
            cmd.arg("--volume").arg(volume);
        }
        let script = format!(
            "set -e; {}; {BUILD_SCRIPT}",
            ci::copy_command(files.is_some())
        );
        cmd.arg(format!("{}:{IMAGE}", ci::prefix()))
            .args(["sh", "-c", &script]);
        ci::run_with_files(cmd, files.as_deref())
    }
}

/// Makes a throwaway signing key and builds the repositories for
/// `file:///repo`, where the checks mount them.
const BUILD_SCRIPT: &str = "export GNUPGHOME=/tmp/gnupg; mkdir -m 700 \"$GNUPGHOME\"; \
     gpg --batch --passphrase '' --quick-generate-key \
       'Glyphkeel test repository <test@example.invalid>' ed25519 sign 1d; \
     exec cargo run --locked --quiet --package xtask -- repos \
       --deb /packages/glyphkeel.deb --rpm /packages/glyphkeel.rpm \
       --out /site --site-url file:///repo </dev/null";

fn canonical(path: &Path) -> Result<String> {
    let path =
        fs::canonicalize(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    path.to_str()
        .map(str::to_owned)
        .context("a path is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Options, apt_sources, dnf_repo, fingerprints, pool_letter, release_fields};

    fn parse(args: &[&str]) -> anyhow::Result<Option<Options>> {
        let args: Vec<String> = args.iter().map(|&arg| arg.to_owned()).collect();
        Options::parse(&args)
    }

    #[test]
    fn the_packages_and_the_folder_are_required() {
        let options = parse(&[
            "--deb",
            "a.deb",
            "--rpm",
            "b.rpm",
            "--out",
            "site",
            "--key",
            "ABC",
            "--site-url",
            "https://example.org/x/",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(options.deb, PathBuf::from("a.deb"));
        assert_eq!(options.key.as_deref(), Some("ABC"));
        assert_eq!(options.site_url, "https://example.org/x");
        assert!(parse(&["--deb", "a.deb", "--out", "site"]).is_err());
        assert!(parse(&["--rpm", "b.rpm", "--out", "site"]).is_err());
        assert!(parse(&["--deb", "a.deb", "--rpm", "b.rpm"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(parse(&["--help"]).unwrap().is_none());
    }

    #[test]
    fn the_signing_key_is_the_primary_key_of_each_secret_key() {
        let listing = "sec:u:255:22:AAAA:1:::u:::scESC:::+:::ed25519:::0:\n\
                       fpr:::::::::0123456789ABCDEF0123456789ABCDEF01234567:\n\
                       grp:::::::::X:\n\
                       uid:u::::1::Y::Glyphkeel test::::::::::0:\n\
                       ssb:u:255:18:BBBB:1::::::e:::+:::cv25519::\n\
                       fpr:::::::::FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:\n";
        assert_eq!(
            fingerprints(listing),
            ["0123456789ABCDEF0123456789ABCDEF01234567"]
        );
        assert!(fingerprints("").is_empty());
    }

    #[test]
    fn packages_go_in_the_pool_as_in_debian() {
        assert_eq!(pool_letter("glyphkeel"), "g");
        assert_eq!(pool_letter("libfoo"), "libf");
    }

    #[test]
    fn the_client_files_name_the_site_and_check_signatures() {
        let sources = apt_sources("https://example.org/gk", "amd64");
        assert!(sources.contains("URIs: https://example.org/gk/apt/\n"));
        assert!(sources.contains("Suites: stable\n"));
        assert!(sources.contains("Signed-By: /etc/apt/keyrings/glyphkeel.gpg\n"));
        let repo = dnf_repo("https://example.org/gk");
        assert!(repo.contains("baseurl=https://example.org/gk/rpm/\n"));
        assert!(repo.contains("gpgcheck=1\n"));
        assert!(repo.contains("repo_gpgcheck=1\n"));
        assert!(repo.contains("gpgkey=https://example.org/gk/rpm/glyphkeel.asc\n"));
        let fields = release_fields("amd64");
        assert!(fields.contains(&("Suite", "stable".to_owned())));
        assert!(fields.contains(&("Components", "main".to_owned())));
    }
}
