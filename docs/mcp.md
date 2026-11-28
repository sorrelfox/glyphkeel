<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Working with AI agents

Glyphkeel has an [MCP](https://modelcontextprotocol.io/) server, so AI agents
such as Claude Code, Codex or Cursor can read and change your tasks. The agent's
client starts the server itself, as the command `glyphkeel mcp`, and talks to it
over standard input and output. Nothing listens on the network, and the server
works while the app is closed. For clients that can only connect to an
address, the app can also serve agents on 127.0.0.1
([Local HTTP endpoint](#local-http-endpoint)).

To connect a client, see [Connecting MCP clients](mcp-clients.md), or copy the
set-up from Preferences › Agents & CLI.

## What agents can do

An agent can do what you can do in the app, through MCP tools in these groups:

| Group | Tools for |
|---|---|
| workspace | The workspace and its rules, search, the activity log, revert |
| projects | Projects, project folders, custom fields, templates, versions, sprints, the agent brief |
| tasks | Finding, creating, changing, moving, linking, duplicating and claiming tasks; checklists; recurrence |
| comments | Adding, editing and deleting comments |
| time | Logging time, the timer, reminders, kinds of work, the time and flow reports |
| views | Today, the board and its layout, saved views, dashboards |
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

To catch up at the start of a session, an agent calls `get_digest`: what the
person, other agents, automation rules and the command line changed since the
end of its previous session of the same name (or since `since`, or else in the
last day), grouped like the digest on Today, with counts. An agent limited to a
project sees only that project's part. Agents can read the digest but not mark
it as seen: that is the person's.

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

## Asking you first

When an agent's client can show you a question (MCP's *elicitation*), the
agent sees every tool, and two kinds of change ask you before they run:

- a change its profile doesn't allow, such as a read-only agent creating a
  task;
- a change that deletes a lot: deleting a project, purging items from the
  trash, or moving more tasks to the trash than the limit for one call.

The client shows the question, for example "Claude Code wants to delete the
project Home lab with 42 tasks for good. Allow?". If you allow it, that one
change runs, as if the agent had the full profile, and the history shows it
with the agent's name and **You allowed this when the agent asked**. If you
decline, or don't answer within five minutes, the agent is told it wasn't
allowed, as it would be without asking.

Clients that can't show questions work as before: the profile decides. To
turn the questions off, switch off **Agents may ask you** in Preferences ›
Agents & CLI; turning it on again asks you in a window of the desktop.

## Limits on each session

Preferences › Agents & CLI › **Limits on each agent session**:

| Limit | Default |
|---|---|
| Changes a minute | 120 |
| New tasks a day | No limit |
| Items to the trash at a time (Standard profile) | 20 |
| Items to the trash an hour (Standard profile) | 50 |
| Reads at once | 60 |
| Reads a minute | 600 (10 a second) |
| Data read a minute | 8 MB |

An agent that reaches a limit on changes is told so, and its change is not
made.

The read limits work like a bucket: an agent may read 60 times in a row,
then as fast as the bucket refills, ten a second. Tools that read, reading a
resource and getting a prompt each count as one read; an update to a
subscription counts a tenth, and waits when the bucket is empty. The data
limit counts the size of the results, so an agent that pages through the
whole workspace slows down while one that works on a few tasks never notices.
An agent with the read-only profile may read twice as much. Lifting **Reads a
minute** lifts the limit on reads; lifting **Reads at once** leaves a minute's
worth.

An agent that reads faster gets a `rate_limited` error. Its text starts with
`Error (rate_limited)` and has a `Retry after (s): N` line; over JSON-RPC
(resources and prompts) the error's data has `"code": "rate_limited"` and
`"retry_after": N`. The agent should wait that many seconds, then read fewer
or smaller results. Refused reads are counted in the session and shown in
**Agent sessions**. Only agents are limited: your own use in the app and with
`glyphkeel` is not, and changes keep their own limit.

The limits hold per session, the same over standard input and output and
over HTTP. Each `glyphkeel mcp` process is one session, and each client of
the HTTP endpoint has its own; they don't share a bucket, and a new session
starts with a full one. A change to the limits counts at once, also in
sessions already running.

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

## Files on disk

Some tools name a file or a folder on your computer: `attach_file` (by
`path`), `import_data`, `import_csv` and `import_wiki_pages`, and
`export_data` and `export_wiki_page`. Agents may use only the files in the
**folders agents may use**:

- the workspace's `exports` folder, always. A plain file name, such as
  `report.pdf` or `notes/plan.md`, means a file in it;
- the folders you add in Preferences › Agents & CLI › **Files agents may use**
  (**Add folder…**). The agent then gives a full path inside one of them.

Everything else is refused, with a message that lists the folders:

- any other absolute path, such as `/home/me/.ssh/id_ed25519` or
  `C:\Users\me\Documents\secret.pdf`;
- a path that goes up a folder with `..`;
- hidden files and folders (a name that starts with `.`), also inside a
  folder you added;
- a link (symbolic link or junction) that leads outside the folders. Links
  are followed, and the place they lead to is what counts; an export never
  writes onto a link;
- the workspace's own folder and its backups, even inside a folder you added;
- names that Windows reserves, such as `CON` or `file.txt:stream`.

Paths on Windows are compared without regard to case, in every form
(`C:\…`, `C:/…`, `\\?\C:\…`, `\\server\share\…`).

`get_workspace` lists the folders, as `agent_folders`, so an agent knows
where to put a file before it asks you to attach it.

The setting is personal: it is kept in your personal settings
(`agent_folders` in `settings.json`, see [Data and settings](data-and-settings.md))
and applies to every agent on this computer, in every workspace. The server
reads it again on every call, so a folder you add or remove counts at once.
Only you can change it: no tool changes it, and the app adds a folder only
from the folder chooser. Your own commands, in the app and with `glyphkeel`,
are not limited.

## Watching for changes

Agents can read a task, a project or the result of a query as a resource,
and subscribe to hear when it changes, including changes you make in the app:

| Resource | What it is |
|---|---|
| `glyphkeel://tasks/TM-41` | A task, as `get_task` returns it |
| `glyphkeel://projects/TM` | A project; it changes when anything in it does |
| `glyphkeel://queries?query=status%3Aopen%20label%3Aui` | The first 50 tasks a [query](query-language.md) finds; it changes when the result does |

Clients of the 2026-07-28 protocol subscribe with `subscriptions/listen`,
older ones with `resources/subscribe`. Notifications come at most once a
second for each resource, and an agent limited to one project can watch only
what is in it.

## Local HTTP endpoint

Some clients connect to a URL instead of starting a program. For them,
switch on **Serve agents over HTTP** in Preferences › Agents & CLI › **Local
HTTP endpoint**. The app asks you to confirm, then serves agents at
`http://127.0.0.1:<port>/mcp` while it runs, and follows the workspace you
have open. Preferences shows the address, the token (hidden until you show
it), a **New token** button and the set-up to copy:

```json
{
  "mcpServers": {
    "glyphkeel": {
      "type": "http",
      "url": "http://127.0.0.1:41000/mcp",
      "headers": { "Authorization": "Bearer <token>" }
    }
  }
}
```

- The port is a free one each time the app starts, unless you choose one
  (1024 to 65535); choose one if your client's set-up should stay the same.
- `/mcp` serves the standard profile, `/mcp/read-only` and `/mcp/full` those
  profiles; **Most an agent may do** still caps them.
- Agents get the same tools, limits and history as over standard input and
  output. Each client's work is one agent session.

Without the app, `glyphkeel mcp --http` serves the same endpoint for the
workspace it opens, with the same token, until you press Ctrl+C. `--port`
chooses the port, and `--profile`, `--project` and `--toolsets` apply to
`/mcp`. The first run without a token makes one and says where it is kept.
[Security](security.md#the-local-http-endpoint-for-agents) explains how the
endpoint is protected.

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

- **While you were away** at the top of Today sums up what agents changed since
  you last marked it as seen: each session with its client, its number of
  changes and the tasks it touched, with **Revert session** and **Open in
  Activity** ([Today](guide/search.md#while-you-were-away)). **Inbox** in the
  sidebar counts its entries.
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
glyphkeel digest show
glyphkeel agents sessions
glyphkeel revert --session <id> --dry-run
glyphkeel revert --session <id>
glyphkeel activity --actor-kind agent
```

## Safety in short

- The server is a local process that the client starts. Glyphkeel makes no
  connection to the network; the optional HTTP endpoint listens on 127.0.0.1
  only, with a token, and is off unless you turn it on.
- An agent works only in the workspace, and with `--project` only in that
  project.
- You choose the most it may do, and only you can raise it. A change beyond
  it, or one that deletes a lot, runs only when you allow it.
- Everything it changes can be seen and reverted.
- It reads and writes files only in the [folders agents may use](#files-on-disk).
