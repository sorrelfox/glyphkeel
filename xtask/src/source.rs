// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask source-tarball`: the source of a release as
//! `glyphkeel-<version>.tar.gz`, with every file under `glyphkeel-<version>/`.
//!
//! It holds the files git lists (tracked, plus untracked ones it doesn't
//! ignore), so in a clean checkout of a tag it is the tag's tree. The archive
//! is reproducible: the files are sorted, owned by root and dated to the
//! commit (or `SOURCE_DATE_EPOCH`), and gzip writes no name or time stamp.
//! The Arch package builds from it, and the pre-release attaches it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

const HELP: &str = "\
Usage: cargo xtask source-tarball [--out DIR]

Writes DIR/glyphkeel-<version>.tar.gz (default DIR: target/dist): the files git
lists, under glyphkeel-<version>/, sorted and dated to the last commit (or to
SOURCE_DATE_EPOCH when it is set), so the same tree gives the same archive.
";

/// The workspace version, which names the archive and its folder.
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let mut out = root.join("target/dist");
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                out = PathBuf::from(args.next().context("`--out` needs a folder")?);
            }
            "-h" | "--help" => {
                eprint!("{HELP}");
                return Ok(true);
            }
            other => bail!("unknown option `{other}`\n\n{HELP}"),
        }
    }
    let file = write(root, &out)?;
    eprintln!("source-tarball: {}", file.display());
    Ok(true)
}

/// The archive's file name.
pub(crate) fn file_name() -> String {
    format!("glyphkeel-{VERSION}.tar.gz")
}

/// The folder every file of the archive is in.
fn prefix() -> String {
    format!("glyphkeel-{VERSION}/")
}

/// Writes the archive into `out` and returns its path.
pub(crate) fn write(root: &Path, out: &Path) -> Result<PathBuf> {
    let files = crate::headers::git_files(root)
        .context("the source tarball is made from a git checkout, and git found none here")?;
    fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    let file = out.join(file_name());
    let epoch = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) if value.chars().all(|c| c.is_ascii_digit()) && !value.is_empty() => value,
        _ => commit_time(root)?,
    };
    let target =
        fs::File::create(&file).with_context(|| format!("cannot create {}", file.display()))?;
    let mut gzip = Command::new("gzip")
        .args(["-9", "-n"])
        .stdin(Stdio::piped())
        .stdout(target)
        .spawn()
        .context("cannot run gzip")?;
    let gzip_in = gzip.stdin.take().context("no standard input for gzip")?;
    let mut tar = Command::new("tar")
        .args(tar_args(&epoch))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(gzip_in)
        .spawn()
        .context("cannot run tar")?;
    {
        let mut stdin = tar.stdin.take().context("no standard input for tar")?;
        for name in &files {
            let name = name
                .to_str()
                .with_context(|| format!("{} is not valid UTF-8", name.display()))?;
            stdin.write_all(name.as_bytes())?;
            stdin.write_all(b"\0")?;
        }
    }
    let tar_ok = tar.wait().context("cannot wait for tar")?.success();
    let gzip_ok = gzip.wait().context("cannot wait for gzip")?.success();
    if !tar_ok || !gzip_ok {
        bail!("tar or gzip failed while writing {}", file.display());
    }
    Ok(file)
}

/// The arguments of GNU tar for a reproducible archive of the files given
/// on standard input, NUL-separated.
fn tar_args(epoch: &str) -> Vec<String> {
    [
        "--create",
        "--format=gnu",
        "--null",
        "--no-recursion",
        "--sort=name",
        "--owner=0",
        "--group=0",
        "--numeric-owner",
        "--mode=go-w,u+rw,a+rX",
    ]
    .map(str::to_owned)
    .into_iter()
    .chain([
        format!("--mtime=@{epoch}"),
        format!("--transform=s,^,{},", prefix()),
        "--file=-".to_owned(),
        // Last: tar applies the options before it to the files it lists.
        "--files-from=-".to_owned(),
    ])
    .collect()
}

/// The commit time of HEAD, in seconds since 1970.
fn commit_time(root: &Path) -> Result<String> {
    let out = Command::new("git")
        .args(["log", "-1", "--format=%ct"])
        .current_dir(root)
        .output()
        .context("cannot run git")?;
    let time = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || time.is_empty() || !time.chars().all(|c| c.is_ascii_digit()) {
        bail!("cannot read the time of the last commit");
    }
    Ok(time)
}

#[cfg(test)]
mod tests {
    use super::{file_name, prefix, tar_args};

    #[test]
    fn the_archive_is_named_and_rooted_after_the_version() {
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(file_name(), format!("glyphkeel-{version}.tar.gz"));
        assert_eq!(prefix(), format!("glyphkeel-{version}/"));
    }

    #[test]
    fn the_archive_is_reproducible() {
        let args = tar_args("1700000000");
        for wanted in [
            "--sort=name",
            "--owner=0",
            "--group=0",
            "--numeric-owner",
            "--mtime=@1700000000",
            "--null",
            "--no-recursion",
        ] {
            assert!(args.iter().any(|arg| arg == wanted), "missing {wanted}");
        }
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("--transform=s,^,glyphkeel-"))
        );
    }
}
