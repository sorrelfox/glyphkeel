# Changelog

All notable changes to Glyphkeel are listed here. The format follows
Conventional Commits, and versions follow Semantic Versioning.

## [Unreleased]

## [0.0.5] - 2026-11-23

A task list with a query language, and a set of smaller improvements.

### Features

- **Task list:** every task of a project, or of all projects, in a fast table
  with sortable columns, a column chooser, totals for estimates and time spent,
  and inline editing of title, status, priority and due date.
- **Query language:** a one-line query field with highlighting, completion and
  clear error messages, natural dates such as "next fri", and saved views.
  The same queries work in `glyphkeel task list --query` and for agents.
- **Bulk changes:** change the status, priority or labels of many tasks at
  once, or move them, with a preview and a single Undo.
- **Export:** save the current list as CSV or JSON.
- **Labels:** new labels get an unused colour, and you can create a label
  straight from a task.
- **Projects:** choose a project's colour and an icon to show next to its name.
- **Time zone:** pick the time zone in which all times are shown; the app,
  the command line and agents use the same setting.
- **Editor toolbar:** bold, italic, links, lists, checklists and more while
  you write descriptions and comments.

### Fixes

- Task windows open in the installed app.
- Preferences rows stay inside their cards, and drop-down lists follow the
  current theme.

## [0.0.4] - 2026-11-23

The first test build: projects, tasks and the board, task details with a
Markdown editor, and time tracking with reminders. Data stays in one folder on
your disk.

### Features

- **Projects and the board:** a project tree with sub-projects, project
  settings with statuses and labels, and a board of task cards that you can
  drag between columns.
- **Tasks:** a task panel and a separate task window with subtasks (which you
  can reorder), links between tasks, attached files with thumbnails (including
  pasted images), comments and the full history of every change.
- **Descriptions:** a Markdown reader and an editor with a live preview,
  checklists you can tick in place, and spell checking. Each edit is kept as a
  revision; when two edits clash, the other version is kept as a copy instead
  of being lost.
- **Time:** a time tab with time entries, a running timer with its own
  keyboard shortcut, and a time report. Reminders appear as desktop
  notifications, and reminders that fell due while the app was closed are
  shown as missed at the next start. The app can keep running in the
  background with a tray icon.
- **Preferences:** preference pages and a settings file.
- **The app shell:** themes, actions and a keymap.

### Agents and the command line

- The `glyphkeel` command line: work with tasks from the terminal, open a task
  in the app by its key (for example `glyphkeel TM-41`), shell completions and
  a man page.
- `glyphkeel mcp` starts the built-in MCP server over standard input and
  output, so MCP clients can use the same operations as the app.

### Fixes

- Times are right on systems without a time zone database: a copy of the
  database is built in and used only when the system has none, instead of
  falling back to UTC.
- Links to attachments keep working in the reader.

### Packages

- The deb and rpm packages install both programs: the desktop app
  (`glyphkeel-app`) and the command line (`glyphkeel`).

### Other

- The repository's foundations: the Rust workspace, the desktop app's window,
  the UI's toolchain and the container checks, which now include the licence
  and advisory check of the dependencies.
