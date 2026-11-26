// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Repository automation, run as `cargo xtask <command>`.

mod bundle;
mod ci;
mod codegen;
mod deps;
mod headers;
mod installed;
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
  gen [--check]       Generate the files that come from the operation
                      catalog: the JSON Schemas, the MCP tool list, the
                      reference pages and the TypeScript client. With
                      --check, write nothing and fail when a file is stale.
                      `codegen` is the same command.
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
