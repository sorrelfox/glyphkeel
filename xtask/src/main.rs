// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Repository automation, run as `cargo xtask <command>`.

mod arch;
mod bundle;
mod ci;
mod codegen;
mod deps;
mod flathub;
mod headers;
mod installed;
mod notices;
mod release;
mod repos;
mod samples;
mod source;
mod windows;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, bail};

const HELP: &str = "\
Usage: cargo xtask <command> [options]

Commands:
  help                Show this message.
  ci [options]        Run check suites in containers (Podman, or Docker when
                      Podman is missing), one image per distribution, and
                      the Windows build cross-compiled and run under Wine
                      (`--distro windows-cross`). See `cargo xtask ci --help`.
  check-headers       Check that every source file starts with the licence
                      header.
  check-bundle [DIR]  Check that the release UI (default: ui/dist) names no
                      remote address it could load.
  check-deps          Check the dependency rules: the command line and the
                      library crates never link the desktop toolkit or the web
                      view, and the shipped programs link no HTTP client.
  notices [--check]   Write THIRD-PARTY-NOTICES.txt: the licences of every
                      crate, npm package and component that ships, checked
                      against the policy in licences.toml. With --check,
                      write nothing and fail when the file is stale or a
                      licence isn't allowed.
  gen [--check]       Generate the files that come from the operation
                      catalog: the JSON Schemas, the MCP tool list, the
                      reference pages and the TypeScript client. With
                      --check, write nothing and fail when a file is stale.
                      `codegen` is the same command.
  source-tarball [--out DIR]
                      Write glyphkeel-<version>.tar.gz, the source of a
                      release, reproducibly (default DIR: target/dist).
  flathub --tag TAG --commit SHA --out DIR [--sources DIR]
                      Write the Flathub repository's files: the manifest
                      with the release's git source, flathub.json and the
                      generated source lists.
  repos [options]     Build the signed APT and DNF repositories from a deb
                      and an rpm. See `cargo xtask repos --help`.
  migration-samples --check | --binary FILE | --commit REV
                      Check that every schema version a release shipped has
                      a sample workspace for the upgrade tests, or make the
                      sample of a release with its `glyphkeel` program. See
                      `cargo xtask migration-samples --help`.
  check-versions      Check that the version agrees in Cargo.toml, the lock
                      files, ui/package.json, CHANGELOG.md, the AppStream
                      file and the PKGBUILD.
  release-check [options]
                      Run the checks before a release in order, from the
                      static checks to every container run, and write a
                      record of the results. `--dry-run` prints the steps.
                      See `cargo xtask release-check --help`.
";

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatches the command. Returns `Ok(false)` when a check found problems.
fn run() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        eprint!("{HELP}");
        return Ok(true);
    };
    let root = workspace_root();
    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            eprint!("{HELP}");
            Ok(true)
        }
        "ci" => ci::run(&root, rest),
        "check-headers" => {
            no_args(cmd, rest)?;
            headers::run(&root)
        }
        "check-bundle" => bundle::run(&root, rest),
        "check-deps" => {
            no_args(cmd, rest)?;
            deps::run(&root)
        }
        "gen" | "codegen" => codegen::run(&root, rest),
        "notices" => notices::run(&root, rest),
        "source-tarball" => source::run(&root, rest),
        "repos" => repos::run(rest),
        "flathub" => flathub::run(&root, rest),
        "migration-samples" => samples::run(&root, rest),
        "check-versions" => {
            no_args(cmd, rest)?;
            release::check_versions(&root)
        }
        "release-check" => release::run(&root, rest),
        other => {
            eprint!("unknown command `{other}`\n\n{HELP}");
            Ok(false)
        }
    }
}

fn no_args(cmd: &str, rest: &[String]) -> Result<()> {
    if rest.is_empty() {
        Ok(())
    } else {
        bail!("`{cmd}` takes no arguments (got: {})", rest.join(" "))
    }
}

/// The workspace root: the parent of the xtask package directory.
fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}

/// The `cargo` executable to use for subprocesses.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}
