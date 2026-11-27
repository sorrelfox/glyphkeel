<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Glyphkeel documentation

Glyphkeel is a task manager for one person on one computer. It keeps your
projects, tasks, wiki pages and files in a folder on your disk, needs no
account and never goes online. You can use it in three ways, and all three see
the same data:

- the **desktop app**, `glyphkeel-app`;
- the **command line**, `glyphkeel`;
- the **MCP server**, `glyphkeel mcp`, for AI agents such as coding assistants.

New here? Start with [Getting started](guide/getting-started.md).

## User guide

How to use the desktop app.

| Page | What it covers |
|---|---|
| [Getting started](guide/getting-started.md) | Installing on Linux and Windows, the first start, the window |
| [Projects and sub-projects](guide/projects.md) | Projects, keys, statuses, task types, labels, parts, archiving |
| [Tasks](guide/tasks.md) | Creating tasks, the task panel, descriptions, checklists, subtasks, links, comments, files |
| [The board](guide/board.md) | Columns, cards, drag and drop, your own layout, WIP limits |
| [The list and saved views](guide/list.md) | The task list, filters, grouping, bulk changes, CSV and JSON export |
| [Calendar and Gantt chart](guide/calendar-and-gantt.md) | Dates on a calendar, the schedule, dependencies and milestones |
| [The wiki](guide/wiki.md) | Pages, links, macros, page templates, export and import |
| [Roadmap and versions](guide/roadmap.md) | Planning tasks for releases |
| [Sprints](guide/sprints.md) | Time-boxed cycles of work, the backlog and the burndown |
| [Custom fields](guide/custom-fields.md) | Your own fields on tasks |
| [Status rules](guide/rules.md) | What happens when a task enters a status, and rules for agents |
| [Automation rules](guide/automation.md) | Rules that act on their own when a task changes, a due date comes, or on a schedule |
| [Recurring tasks and templates](guide/recurring-and-templates.md) | Tasks that repeat, task templates and project templates |
| [Time tracking and reports](guide/time.md) | Logged time, the timer, reminders, the time report and flow reports |
| [Today, search and the palette](guide/search.md) | Finding things and moving around quickly, quick add |
| [Workspaces](guide/workspaces.md) | Keeping separate sets of projects |
| [History, undo and the trash](guide/history-and-trash.md) | Undo, the history of a task, the activity log, revert, the trash |
| [Export and import](guide/export-import.md) | Archives, and tasks from CSV files (Jira and Redmine) |
| [Preferences](guide/preferences.md) | Every page of Preferences |
| [Keyboard shortcuts](guide/keyboard-shortcuts.md) | All default shortcuts |
| [Languages](guide/languages.md) | The interface language, date formats and translations |

## Guides

| Page | What it covers |
|---|---|
| [Backup and restore](backup-and-restore.md) | Daily backups, making and checking one, restoring, moving to another computer |
| [The query language](query-language.md) | Filtering tasks with text such as `is:open due<+7d`, with examples |
| [The command line](cli.md) | Common workflows, `--json`, workspaces, shell completions |
| [Working with AI agents](mcp.md) | What agents can do, profiles, project scope, prompts, safety |
| [Connecting MCP clients](mcp-clients.md) | Set-up for Claude Code, Claude Desktop, Codex, Cursor and VS Code |
| [Data and settings](data-and-settings.md) | Where your data and settings are kept, and how to move them |
| [Glyphkeel on Windows](windows.md) | Installing, updating and removing Glyphkeel on Windows |
| [Security](security.md) | What Glyphkeel protects you against, the local endpoint for agents, reporting a problem |

## Reference

These pages list everything in full. Most of them are generated from the
program itself, so they always match it.

| Page | What it lists |
|---|---|
| [Task queries](reference/queries.md) | Every field and operator of the query language |
| [Links in Markdown](reference/links.md) | Task, comment and wiki links, wiki macros, page templates |
| [Command line](reference/cli.md) | Every `glyphkeel` command, global options and exit codes |
| [Operations](reference/operations.md) | Every operation, with its safety class, MCP tool and command |
| [MCP prompts](reference/mcp-prompts.md) | Each prompt for agents, with its arguments and text |
| [MCP tools](reference/mcp-tools.json) | The MCP tool list, per profile, as agents see it (JSON) |
| [Schemas](reference/catalog.schema.json) | The JSON input and output of every operation |

## For contributors

- [Contributing](../CONTRIBUTING.md): building, the layout of the repository, checks.
- [Making a release](releasing.md): how releases are built and published.
- [Security](../SECURITY.md): how to report a security problem.
