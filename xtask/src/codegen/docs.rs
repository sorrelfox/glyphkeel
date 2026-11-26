// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The reference pages generated from the catalog.

use std::fmt::Write;

use gk_api::mcp::{PARITY_EXCEPTIONS, tool_for};
use gk_api::{CATALOG, Exposure, OpDescriptor};

const HEADER: &str = "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->\n\
<!-- Generated from the operation catalog by `cargo xtask gen`. Do not edit. -->\n\n";

fn reach(d: &OpDescriptor) -> &'static str {
    match (d.is_app_only(), d.cli.is_some()) {
        (false, _) => "app, command line, MCP",
        (true, true) => "app, command line",
        (true, false) => "app",
    }
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// `docs/reference/operations.md`.
pub(super) fn operations() -> String {
    let mut s = String::from(HEADER);
    s += "# Operations\n\n";
    s += "Everything Glyphkeel can do is an operation of one catalog. The desktop app, the \
`glyphkeel` command and the MCP server for agents are three views of it, so they offer the \
same operations with the same rules.\n\n";
    s += "The safety class decides who may run an operation:\n\n";
    s += "- `read` changes nothing;\n";
    s += "- `write` creates or changes data and can be undone;\n";
    s += "- `trash` moves data to the trash, from where it can be restored;\n";
    s += "- `permanent` can't be undone in the app and is confirmed first;\n";
    s += "- `app` changes the application itself and is never offered to agents.\n\n";
    s += "| Operation | Class | Reachable from | MCP tool | Command |\n";
    s += "|---|---|---|---|---|\n";
    for d in CATALOG {
        let tool = tool_for(d.id).map_or_else(|| "—".to_owned(), |t| format!("`{}`", t.name));
        let cli = d
            .cli
            .map_or_else(|| "—".to_owned(), |c| format!("`glyphkeel {c}`"));
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {tool} | {cli} |",
            d.id,
            d.safety.as_str(),
            reach(d)
        );
    }
    s += "\n";
    for d in CATALOG {
        let _ = writeln!(s, "## `{}`\n\n{}\n", d.id, d.summary);
        let _ = writeln!(
            s,
            "Input: `{}`. Output: `{}`. Schemas: [catalog.schema.json](catalog.schema.json).\n",
            d.input_name, d.output_name
        );
        if let Exposure::App { reason } = d.exposure {
            let _ = writeln!(s, "Not available to agents: {reason}\n");
        }
    }
    if !PARITY_EXCEPTIONS.is_empty() {
        s += "## Covered differently for agents\n\n";
        for (id, how) in PARITY_EXCEPTIONS {
            let _ = writeln!(s, "- `{id}`: {how}");
        }
        s += "\n";
    }
    s.truncate(s.trim_end().len());
    s.push('\n');
    s
}

/// `docs/reference/cli.md`.
pub(super) fn cli() -> String {
    let mut s = String::from(HEADER);
    s += "# Command line\n\n";
    s += "`glyphkeel <area> <verb> [arguments] [--options]`. Every command runs one operation \
of the catalog ([operations.md](operations.md)). Global options: `--workspace`, `--json`, \
`--quiet`, `--no-color`.\n\n";
    s += "With `--json` a command prints exactly the operation's output, as described in \
[catalog.schema.json](catalog.schema.json).\n\n";
    s += "| Command | Operation | What it does |\n|---|---|---|\n";
    let mut commands: Vec<&OpDescriptor> = CATALOG.iter().filter(|d| d.cli.is_some()).collect();
    commands.sort_by_key(|d| d.cli);
    for d in commands {
        let _ = writeln!(
            s,
            "| `glyphkeel {}` | `{}` | {} |",
            d.cli.unwrap_or_default(),
            d.id,
            cell(d.summary)
        );
    }
    s += "\n## Workspaces\n\n";
    s += "A command works in the workspace `--workspace` names (a name in the data folder, or \
a folder path), else the one `GLYPHKEEL_WORKSPACE` names, else the one last opened in the app or \
with `glyphkeel workspace open`, else `Personal`. `glyphkeel mcp` chooses the same way. These \
commands work on the workspaces of the data folder rather than inside one:\n\n";
    s += "| Command | What it does |\n|---|---|\n";
    s += "| `glyphkeel workspace list` | List the workspaces; the one commands work in is marked |\n";
    s += "| `glyphkeel workspace create <name>` | Make a new, empty workspace |\n";
    s += "| `glyphkeel workspace rename <old> <new>` | Rename a workspace that nobody has open |\n";
    s += "| `glyphkeel workspace open <name or path>` | Work in this workspace from now on: \
commands, agents and the app's next start use it |\n";
    s += "\n## Exit codes\n\n";
    s += "| Code | Meaning |\n|---|---|\n";
    s += "| 0 | Success |\n";
    s += "| 1 | Usage error or invalid input (`invalid`) |\n";
    s += "| 2 | Not found (`not_found`) |\n";
    s += "| 3 | Change rejected (`conflict`, `forbidden`, `limit`) |\n";
    s += "| 4 | Workspace or storage problem (`busy`, `workspace_upgraded`, `workspace_too_new`, `storage`) |\n";
    s
}
