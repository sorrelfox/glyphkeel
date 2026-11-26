<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Connecting MCP clients

An MCP client, such as a coding agent, starts Glyphkeel's server itself, as
the command `glyphkeel mcp`, and talks to it over standard input and output.
Nothing listens on the network, and the server works while the app is
closed.

The quickest way is Preferences › Agents & CLI in the app: pick a client and
copy its set-up, with the full path of the command already filled in. The
examples below do the same by hand. What agents can do, and how to keep them
within limits, is in [Working with AI agents](mcp.md).

## The command

| Platform | Command |
|---|---|
| Linux, deb or rpm | `glyphkeel mcp` |
| Linux, Flatpak | `flatpak run --command=glyphkeel io.github.sorrelfox.Glyphkeel mcp` (Preferences shows the exact command) |
| Windows | `glyphkeel.exe mcp`, installed at `%LOCALAPPDATA%\Programs\Glyphkeel\glyphkeel.exe` |

Options come after `mcp`:

- `--project TM` limits the agent to one project and its sub-projects;
- `--workspace NAME-OR-PATH` picks another workspace than *Personal*;
- `--profile read-only|standard|full` lowers what the agent may do; the
  workspace setting "Most an agent may do" caps it.

`glyphkeel mcp --help` lists them all.

On Windows a client finds `glyphkeel` by name once the installer has put its
folder on your `PATH` and the client was started afterwards. If it doesn't,
use the full path. In JSON files, write each backslash twice.

## Claude Code

```sh
claude mcp add --scope user glyphkeel -- glyphkeel mcp
```

On Windows, in PowerShell, with the full path:

```powershell
claude mcp add --scope user glyphkeel -- "$env:LOCALAPPDATA\Programs\Glyphkeel\glyphkeel.exe" mcp
```

## Claude Desktop

Add this to `claude_desktop_config.json` (Settings › Developer › Edit config;
on Windows the file is in `%APPDATA%\Claude\`):

```json
{
  "mcpServers": {
    "glyphkeel": {
      "command": "C:\\Users\\you\\AppData\\Local\\Programs\\Glyphkeel\\glyphkeel.exe",
      "args": ["mcp"]
    }
  }
}
```

On Linux, `"command": "glyphkeel"` is enough.

## Codex

In `~/.codex/config.toml` (Windows: `%USERPROFILE%\.codex\config.toml`):

```toml
[mcp_servers.glyphkeel]
command = "glyphkeel"
args = ["mcp"]
```

## Cursor and VS Code

Cursor reads `~/.cursor/mcp.json` (Windows: `%USERPROFILE%\.cursor\mcp.json`)
in the `mcpServers` layout shown for Claude Desktop. VS Code reads
`.vscode/mcp.json` in a repository:

```json
{
  "servers": {
    "glyphkeel": { "type": "stdio", "command": "glyphkeel", "args": ["mcp"] }
  }
}
```

## One project per repository

A repository's `.mcp.json` can give the agent only that repository's
project:

```json
{
  "mcpServers": {
    "glyphkeel": { "command": "glyphkeel", "args": ["mcp", "--project", "TM"] }
  }
}
```

## Prompts

The server also offers prompts: ready-made requests that many clients show
as slash commands or in a prompt menu. Each one tells the agent which tools
to use and what to deliver, and the ones that change something ask you
first.

| Prompt | What it does |
|---|---|
| `plan_my_day` | Proposes a plan for today from what is overdue, due and in progress |
| `weekly_review` | Reviews the last seven days: finished, stuck, coming up, time logged |
| `triage_new_tasks` | Proposes a type, priority, labels and version for new tasks, then applies them |
| `break_down_task` | Splits a task into subtasks of a day or less, then creates them |
| `write_status_update` | Writes a short status update for a project |
| `write_release_notes` | Writes release notes for a version of a project |
| `review_schedule` | Goes through a project's schedule problems and proposes fixes |
| `summarise_task_history` | Summarises how a task came to where it is |

The arguments (a project, a task, a version) complete as you type. A
prompt is offered only when the agent may use every tool it needs: a
read-only agent gets no prompt that changes something. With `--project`, a
prompt's project is that project when you leave it out.
[The prompt reference](reference/mcp-prompts.md) has each prompt's
arguments and full text.

Every change an agent makes is recorded with its session, and Preferences ›
Agents & CLI lists the sessions, each of which can be reverted
([Undo and review](mcp.md#undo-and-review)). Client
formats change from time to time; if one of these no longer works, the
client's own documentation has the current form.
