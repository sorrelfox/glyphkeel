<div align="center">

# Glyphkeel

**A local-only desktop task manager with boards, lists, a Gantt chart, a wiki,
a command line and a built-in MCP server.**

[![Licence: GPL-3.0-or-later](https://img.shields.io/badge/licence-GPL--3.0--or--later-blue)](LICENSE)
[![Platforms: Linux, Windows](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows-informational)](#status-and-roadmap)
[![Status: early development](https://img.shields.io/badge/status-early%20development-orange)](#status-and-roadmap)

<img src="docs/screenshots/board.png" alt="The Glyphkeel board in the light theme: a sidebar with Today, All tasks and three projects, a filter bar, and five columns from Backlog to Done with task cards showing labels, due dates, checklist progress, comments and time spent" width="900">

<sub>The board, with a running timer in the sidebar.</sub>

</div>

> [!IMPORTANT]
> Glyphkeel is in early development. It works, but it changes quickly and has
> few users so far: keep backups of anything that matters. The pictures in this
> README are screenshots of the app, except the two marked as design previews.

## Why

Most project trackers in the style of Jira or Redmine assume a server, an
account and a network connection. Glyphkeel is for one person on one computer:
it keeps every task, wiki page and attachment in a single folder on your disk,
needs no account, and contains no network code at all.

It is also built for working alongside coding agents. The desktop app, the
`glyphkeel` command line and the MCP server are three front ends over one
engine and one set of operations, so an agent can do the same things you can,
under rules you set, and every change it makes is recorded and can be undone.

## Features

**Projects and tasks**
- Projects with sub-projects, each with its own key, statuses, types and labels.
  Keys can be changed, and tasks moved between projects.
- Tasks with Markdown descriptions, checklists, subtasks, links, comments,
  attachments, estimates and due dates.
- [Custom fields](docs/guide/custom-fields.md) of nine kinds per project, on the
  task panel, board cards, list columns, in grouping and in queries.
- [Versions](docs/guide/roadmap.md) with a roadmap page, and
  [status rules](docs/guide/rules.md): fields a status needs, sets or protects,
  and limits on what agents may change.
- [Recurring tasks and templates](docs/guide/recurring-and-templates.md) for
  tasks and whole projects.
- [History](docs/guide/history-and-trash.md) for every change, undo, revert, and
  a trash for deleted items.

**Views**
- [Kanban board](docs/guide/board.md) with a personal column layout: reorder,
  hide or collapse columns without changing the project.
- [Task list](docs/guide/list.md) with filters, grouping, bulk changes, saved
  views and export.
- [Gantt chart and calendar](docs/guide/calendar-and-gantt.md) with
  dependencies, lag and milestones.
- [Today view, search and a command palette](docs/guide/search.md), with quick
  add.
- [A wiki](docs/guide/wiki.md) per project, with a page tree, revisions, macros,
  page templates and links to tasks.

**Query language**
- One text form for every filter, such as
  `status:open label:linux due<+7d sort:due,-priority`. It works the same in
  the filter bar, the search box, the command line and the MCP server
  ([the query language](docs/query-language.md)).

**Time and reports**
- [Time entries](docs/guide/time.md), a running timer, remaining estimates and
  reminders as desktop notifications.
- A time report, and flow reports: created against closed, cumulative flow and
  cycle time.

**Data**
- One SQLite database and a folder of attached files. Daily backups, restore,
  and export to plain JSON and Markdown
  ([backup and restore](docs/backup-and-restore.md)).
- [CSV import](docs/guide/export-import.md) with column mapping and presets for
  Jira and Redmine exports.
- Never online: no account, no telemetry, no update checks.

**Desktop**
- Keyboard-first ([keyboard shortcuts](docs/guide/keyboard-shortcuts.md)), with
  five themes (Light, Dark, Slate, Sand and a high-contrast theme), nine accent
  colours and a compact density.
- Ready for translation: every text comes from translation files, and dates and
  numbers follow your locale ([languages](docs/guide/languages.md)).

## Working with AI agents (MCP)

Glyphkeel includes an [MCP](https://modelcontextprotocol.io/) server, so MCP
clients such as coding agents can read and update your tasks. The client starts
it as a local process, `glyphkeel mcp`, and talks to it over standard input and
output; nothing listens on the network.

- Every data operation available in the app is also available as an MCP tool.
- You choose what an agent may do: read only, standard, or full access, and
  optionally limit it to one project.
- Each change records which agent made it and in which session. An agent's
  whole session can be reviewed and reverted.
- Agents can claim a task for a while, so two agents don't pick up the same
  work.
- Prompts such as "Plan my day", "Weekly review" and "Write release notes"
  give agents a ready-made starting point, with completion for their
  arguments.

The MCP server works while the app is closed. It is part of the command line,
not a separate service.
[Connecting MCP clients](docs/mcp-clients.md) has the set-up for common
clients, on Linux and on Windows (`glyphkeel.exe mcp`). Where your workspaces
and settings are kept, and how to move them, is in
[Data and settings](docs/data-and-settings.md).

## Screenshots

Screenshots of the app, taken with a sample workspace.

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/list.png" alt="The task list with the saved view Open work by status: a query in the filter bar, a condition chip, tasks grouped by status with columns for priority, labels, due date, estimate and time spent, and totals"><br><sub>Task list with a saved view, a query and grouping</sub></td>
    <td width="50%"><img src="docs/screenshots/task.png" alt="The board in the dark theme with the task panel open: status, a running timer, priority, dates, labels, a description with a checklist, subtasks and links to other tasks"><br><sub>Task panel with checklist, subtasks, links and a running timer (dark theme)</sub></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/today.png" alt="The command palette open over the Today view in the dark theme, listing matching actions, tasks and projects"><br><sub>Command palette over the Today view (dark theme)</sub></td>
    <td width="50%"><img src="docs/screenshots/board-sand.png" alt="The board in the warm, light Sand theme"><br><sub>The board in the Sand theme</sub></td>
  </tr>
</table>

<details>
<summary>More: the Slate theme and all five themes side by side</summary>

<br>

<img src="docs/screenshots/board-slate.png" alt="The board in the cool, dark Slate theme" width="900">

<sub>The board in the Slate theme</sub>

<img src="docs/screenshots/themes.png" alt="The board cut into five vertical slices, each in one theme: Light, Sand, Slate, Dark and Contrast" width="900">

<sub>The board in the Light, Sand, Slate, Dark and Contrast themes</sub>

</details>

### Design previews

The Gantt chart and the wiki are part of the app. These two pictures, though,
are design previews rendered from the UI design, not screenshots of the app,
so the app looks a little different.

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/gantt.png" alt="Design preview: Gantt chart with task bars, a dependency, a milestone and unscheduled tasks"><br><sub>Design preview: Gantt chart with dependencies, a milestone and unscheduled tasks</sub></td>
    <td width="50%"><img src="docs/screenshots/wiki.png" alt="Design preview: a wiki page about the query language, with a page tree and an outline"><br><sub>Design preview: wiki page with a page tree, outline and links to tasks</sub></td>
  </tr>
</table>

## Status and roadmap

Glyphkeel is being built in the open, in vertical slices: each slice adds a
feature to the engine, the MCP server, the command line and the app together.
The [changelog](CHANGELOG.md) lists what each version added.

- **Now:** everything in [Features](#features) works in the app, on the
  command line and through the MCP server.
- **Next:** polish, accessibility, security review and packaging, based on
  feedback.
- **Platforms:** Linux, packaged as deb, rpm and Flatpak, and Windows 10
  and 11, with a per-user installer ([Glyphkeel on Windows](docs/windows.md)).
  macOS is not planned at present.

Releases are published on the [releases page](https://github.com/sorrelfox/glyphkeel/releases).

## Documentation

The [documentation](docs/README.md) has a user guide for the app, and guides to
[backup and restore](docs/backup-and-restore.md),
[the query language](docs/query-language.md),
[the command line](docs/cli.md) and
[working with AI agents](docs/mcp.md).

## Building from source

You need:

- **Rust** through [rustup](https://rustup.rs/). The toolchain is pinned in
  `rust-toolchain.toml`, and rustup installs it on first use.
- **Node** 24 (the exact version is in `.node-version`) with Corepack, which
  provides the pinned pnpm.
- **WebKitGTK 4.1, GTK 3 and libsoup 3** with their development files:
  - Debian and Ubuntu: `sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev`
  - Fedora: `sudo dnf install gcc gcc-c++ pkgconf-pkg-config webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel`

Then:

```sh
git clone https://github.com/sorrelfox/glyphkeel.git
cd glyphkeel
corepack enable                  # provides the pinned pnpm
pnpm install --frozen-lockfile   # the UI's build tools
pnpm build                       # the UI, into ui/dist
cargo build -p gk-app --features tauri/custom-protocol
./target/debug/glyphkeel-app     # the desktop app, with the UI embedded
cargo run -p gk-cli -- --help    # the command line, `glyphkeel`
```

For UI work, `pnpm tauri dev` runs the app against the UI's development
server. [CONTRIBUTING.md](CONTRIBUTING.md) describes the layout of the
repository, the checks and the code requirements.

## Contributing

The project is young and changes quickly, so please open an issue to discuss a
change before you spend time on a pull request. Read
[CONTRIBUTING.md](CONTRIBUTING.md) first: commits follow Conventional Commits
and carry a `Signed-off-by` line (Developer Certificate of Origin), and pull
requests go to the `dev` branch.

Please report security problems privately, as described in
[SECURITY.md](SECURITY.md).

## The name

A keel keeps a boat on its course. Glyphkeel is a sibling of
[Glyphvane](https://github.com/sorrelfox/glyphvane), a Markdown editor by the
same author: the vane reads the wind, the keel holds the course.

## Licence

Glyphkeel is free software, licensed under the
[GNU General Public License, version 3 or later](LICENSE). The licences of the
third-party software it contains are in
[THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt), which every package
installs and the app shows under Preferences > About.
