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
`--quiet`, `--no-color`, `--tz`, `--lang`.\n\n";
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
    s += "\n## Options with several values\n\n";
    s += "An option that takes a list (`--labels`, `--options`, `--include`…) takes its values \
either repeated or separated by commas: `--labels bug --labels ui` and `--labels bug,ui` are the \
same. Spaces around the values are dropped. Write `\\,` for a comma inside a value \
(`--labels 'needs\\, maybe'`), or give the input as JSON with `--input`, which is always taken \
exactly. The same goes for arguments that take several values (`glyphkeel task show TM-1,TM-2`).\n";
    s += "\n## Workspaces\n\n";
    s += "A command works in the workspace `--workspace` names (a name in the data folder, or \
a folder path), else the one `GLYPHKEEL_WORKSPACE` names, else the one last opened in the app or \
with `glyphkeel workspace open`, else `Personal`. `glyphkeel mcp` chooses the same way. These \
commands work on the workspaces of the data folder rather than inside one:\n\n";
    s += "| Command | What it does |\n|---|---|\n";
    s += "| `glyphkeel workspace list` | List the workspaces; the one commands work in is marked |\n";
    s += "| `glyphkeel workspace create <name>` | Make a new, empty workspace |\n";
    s += "| `glyphkeel workspace rename <old> <new>` | Rename a workspace that nobody has open |\n";
    s += "| `glyphkeel workspace encrypt` | Encrypt the workspace with a passphrase, or give it a new one; nobody may have it open |\n";
    s += "| `glyphkeel workspace decrypt` | Store the workspace unencrypted again; nobody may have it open |\n";
    s += "| `glyphkeel workspace open <name or path>` | Work in this workspace from now on: \
commands, agents and the app's next start use it |\n";
    s += "\n## Language\n\n";
    s += "Help, output for people and error messages are in the language `--lang` names (a \
language tag such as `de` or `pt-BR`), else the one `GLYPHKEEL_LANG` names, else the language \
chosen in the app's Preferences, else the system's (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`; \
on Windows the display language), else English. A message not yet translated shows in English. \
Numbers, dates and times in the output for people follow the language; times are shown in the \
time zone `--tz` names, else the personal setting, else the system's. `en-XA` is a pseudo-language for \
testing translations. Machine text stays English: `--json` output (an error's `message`, with \
its `message_id` and `args` for translating it), command and option names, and everything \
`glyphkeel mcp` serves to agents.\n";
    s += "\n## Exit codes\n\n";
    s += "| Code | Meaning |\n|---|---|\n";
    s += "| 0 | Success |\n";
    s += "| 1 | Usage error or invalid input (`invalid`) |\n";
    s += "| 2 | Not found (`not_found`) |\n";
    s += "| 3 | Change rejected (`conflict`, `forbidden`, `limit`) |\n";
    s += "| 4 | Workspace or storage problem (`busy`, `workspace_upgraded`, `workspace_too_new`, `storage`) |\n";
    s
}

/// `docs/reference/mcp-prompts.md`.
pub(super) fn prompts() -> String {
    use gk_api::prompts::{ArgumentKind, prompts};
    use gk_api::{Profile, Toolset};

    let mut s = String::from(HEADER);
    s += "# MCP prompts\n\n";
    s += "The MCP server offers prompts: ready-made requests that an agent's client shows the \
person, often as slash commands. Each one tells the agent which tools to use and what to \
deliver; none calls a model itself, and the ones that change something ask the person first.\n\n";
    s += "A prompt is listed only when every tool it uses is listed for the agent, so it follows \
the profile, the workspace's cap and the toolsets. When the cap changes while the server runs, \
clients of the `initialize` era get `notifications/prompts/list_changed`; newer clients see the \
list's short cache time. In a session limited to a project (`--project`), a `project` argument \
left out means that project, and other projects can't be named.\n\n";
    s += "Arguments complete with `completion/complete`: projects by key, versions by name \
(within the `project` argument), and tasks by key or by words of the title. \
[mcp-prompts.json](mcp-prompts.json) is the golden list.\n\n";
    let defaults = Toolset::defaults();
    s += "| Prompt | Arguments | Tools | Profiles |\n|---|---|---|---|\n";
    for p in prompts() {
        let arguments: Vec<String> = p
            .arguments
            .iter()
            .map(|a| {
                if a.required {
                    format!("`{}`", a.name)
                } else {
                    format!("`{}` (optional)", a.name)
                }
            })
            .collect();
        let tools: Vec<String> = p.tools.iter().map(|t| format!("`{t}`")).collect();
        let profiles: Vec<&str> = Profile::ALL
            .iter()
            .filter(|profile| {
                gk_api::prompts::listed(**profile, &defaults).any(|q| q.name == p.name)
            })
            .map(|profile| profile.as_str())
            .collect();
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {} |",
            p.name,
            if arguments.is_empty() {
                "—".to_owned()
            } else {
                arguments.join(", ")
            },
            tools.join(", "),
            profiles.join(", ")
        );
    }
    s += "\n";
    for p in prompts() {
        let _ = writeln!(
            s,
            "## `{}`: {}\n\n{}\n",
            p.name,
            p.title,
            cell(&p.description)
        );
        for a in &p.arguments {
            let complete = a.complete.map_or("", |k| match k {
                ArgumentKind::Project => " Completes project keys.",
                ArgumentKind::Version => " Completes the project's version names.",
                ArgumentKind::Task => " Completes task keys.",
            });
            let _ = writeln!(
                s,
                "- `{}`{}: {}{complete}",
                a.name,
                if a.required { "" } else { " (optional)" },
                a.description
            );
        }
        let example: std::collections::BTreeMap<String, String> = p
            .arguments
            .iter()
            .filter(|a| a.required)
            .map(|a| {
                let v = match a.complete {
                    Some(ArgumentKind::Task) => "TM-41",
                    Some(ArgumentKind::Version) => "1.2",
                    _ => "TM",
                };
                (a.name.clone(), v.to_owned())
            })
            .collect();
        let shown: Vec<String> = example
            .iter()
            .map(|(k, v)| format!("`{k}` = `{v}`"))
            .collect();
        if shown.is_empty() {
            s += "\nThe text the agent receives:\n\n";
        } else {
            let _ = writeln!(
                s,
                "\nThe text the agent receives, with {}:\n",
                shown.join(", ")
            );
        }
        s += "```text\n";
        s += &p.render(&example);
        s += "\n```\n\n";
    }
    s
}
