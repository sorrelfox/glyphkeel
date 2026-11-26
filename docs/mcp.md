<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Working with AI agents

Glyphkeel has an [MCP](https://modelcontextprotocol.io/) server, so AI agents
such as Claude Code, Codex or Cursor can read and change your tasks. The agent's
client starts the server itself, as the command `glyphkeel mcp`, and talks to it
over standard input and output. Nothing listens on the network, and the server
works while the app is closed.

To connect a client, see [Connecting MCP clients](mcp-clients.md), or copy the
set-up from Preferences › Agents & CLI.

## What agents can do

An agent can do what you can do in the app, through MCP tools in these groups:

| Group | Tools for |
|---|---|
| workspace | The workspace and its rules, search, the activity log, revert |
| projects | Projects, custom fields, templates, versions, the agent brief |
| tasks | Finding, creating, changing, moving, linking, duplicating and claiming tasks; checklists; recurrence |
| comments | Adding, editing and deleting comments |
| time | Logging time, the timer, reminders, kinds of work, the time and flow reports |
| views | Today, the board and its layout, saved views |
| wiki | Wiki pages: reading, searching, writing, history, export and import |
| schedule | The Gantt chart's schedule and its problems |
| files | Attaching and reading files |
| admin | The trash, backups, export and import, CSV import, workspace settings |

Every group except **admin** is offered by default. `--toolsets` chooses the
groups, for example `--toolsets tasks,comments,wiki` or, to add admin,
`--toolsets workspace,projects,tasks,comments,time,views,wiki,schedule,files,admin`.
[Operations](reference/operations.md) lists every tool with what it does, and
[the tool list](reference/mcp-tools.json) has each tool's schema.

A good first step for an agent is `get_workspace`, which describes the
workspace, its rules and what the agent may do, and `get_agent_brief` for the
project it works on.

## Profiles

A profile sets how far an agent may go. Each tool has a safety class: it reads,
writes (and can be undone), moves to the trash, or can't be undone.

| Profile | The agent can |
|---|---|
| **Read only** | Read, search and report, but change nothing |
| **Standard** | Create and change tasks, and move items to the trash, but delete nothing for good |
| **Full** | Do everything you can, including emptying the trash and deleting projects |

The client asks for a profile with `--profile read-only|standard|full`; the
default is `standard`. The workspace setting **Most an agent may do**, in
Preferences › Agents & CLI, caps it: an agent never gets more, whatever it asks
for.

Raising that setting, or raising the limits below, asks you in a window of the
desktop app, so an agent can't widen its own rights. Only you, in the app, can
change these settings; agents and the command line can't.

## Limits on each session

Preferences › Agents & CLI › **Limits on each agent session**:

| Limit | Default |
|---|---|
| Changes a minute | 120 |
| New tasks a day | No limit |
| Items to the trash at a time (Standard profile) | 20 |
| Items to the trash an hour (Standard profile) | 50 |

An agent that reaches a limit is told so, and its change is not made.

## One project only

`--project KEY` limits an agent to one project and its sub-projects. It sees
nothing else: not in searches, lists, links or wiki macros. That suits a
repository's own `.mcp.json`:

```json
{
  "mcpServers": {
    "glyphkeel": { "command": "glyphkeel", "args": ["mcp", "--project", "TM"] }
  }
}
```

Preferences › Agents & CLI › **Only one project** gives this set-up for a
project.

## Other options

| Option | What it does |
|---|---|
| `--workspace NAME-OR-PATH` | Another workspace than the one last opened |
| `--agent-name NAME` | The name shown in the history when the client sends none |
| `--tz ZONE` | The time zone of "today", such as `Europe/Berlin` |
| `--log-level LEVEL` | How much to log on standard error: `off`, `error`, `warn` (default), `info`, `debug`, `trace` |

`glyphkeel mcp --help` lists them all.

## Telling agents how to work

- **The agent brief:** mark a wiki page with **Use as the agent brief**, or
  write the project's description, with what the agent should know: the
  project's conventions, what *done* means, whom to ask. Agents read it with
  `get_agent_brief`.
- **Rules for agents** (Project settings › Rules › Agents): statuses agents may
  not move tasks into, and fields they may not change. For example, only you
  move tasks to *Done*. See [Status rules](guide/rules.md#rules-for-agents).
- **Status rules** hold for agents as for you: required fields, read-only
  fields, what happens on entry.

## Claims

Before an agent starts on a task, it can **claim** it for a while
(`claim_task`). Other agents then leave the task alone until the claim ends or
the agent releases it (`release_task`). Claiming moves the task into progress.

The card shows who claimed it, Today lists claims under **Agents at work**, and
`claimed:any` finds them in a query. Claims never block you; **Release the
agent's claim** in the task menu ends one.

## Prompts

The server also offers prompts: ready-made requests that many clients show as
slash commands, such as `plan_my_day`, `weekly_review`, `triage_new_tasks`,
`break_down_task`, `write_status_update`, `write_release_notes`,
`review_schedule` and `summarise_task_history`. The ones that change something
ask you first. A prompt is offered only when the agent may use every tool it
needs.

[Connecting MCP clients](mcp-clients.md#prompts) describes each one briefly, and
[the prompt reference](reference/mcp-prompts.md) has their arguments and text.

## Undo and review

Every change an agent makes is recorded with the agent's name and its session.

- **Activity** in the sidebar, filtered to **Agents**, shows what agents changed.
  Each change has **Revert**.
- Preferences › Agents & CLI › **Agent sessions** lists each session: the
  client, the profile, when it ran, and how much it read and changed. **Revert
  session…** takes back everything that session changed. Changes that something
  else changed since are skipped and listed.
- Before a large change by an agent, Glyphkeel makes a backup, a **Safety copy**
  ([Backup and restore](backup-and-restore.md)).
- Items an agent deletes go to the trash, and a standard agent can't empty it.

From the command line:

```sh
glyphkeel agents sessions
glyphkeel revert --session <id> --dry-run
glyphkeel revert --session <id>
glyphkeel activity --actor-kind agent
```

## Safety in short

- The server is a local process that the client starts. Glyphkeel contains no
  network code.
- An agent works only in the workspace, and with `--project` only in that
  project.
- You choose the most it may do, and only you can raise it.
- Everything it changes can be seen and reverted.
