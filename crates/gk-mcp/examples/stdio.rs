// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The MCP server as a program of its own, as `glyphkeel mcp` runs it:
//! `cargo run -p gk-mcp --example stdio -- --workspace <path>`. Useful for
//! trying MCP clients and timing start-up without the command line.

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    gk_mcp::run_stdio(&args)
}
