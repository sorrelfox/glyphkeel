// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel`: the command line. It reaches the engine through the operation
//! catalog, as the desktop app and the MCP server do.

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// Glyphkeel, a local task manager, from the command line.
#[derive(Debug, Parser)]
#[command(name = "glyphkeel", version, about)]
struct Cli {}

fn main() -> ExitCode {
    let _cli = Cli::parse();
    // No commands exist yet: show what the program is.
    match Cli::command().print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("glyphkeel: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn command_line_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
