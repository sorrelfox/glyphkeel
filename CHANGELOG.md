# Changelog

All notable changes to Glyphkeel are listed here. The format follows
Conventional Commits, and versions follow Semantic Versioning.

## [Unreleased]

## [0.1.10] - 2026-11-27

### Features

- **Project folders:** group projects in the sidebar in one level of
  folders; drag, menus or Alt+arrows move projects in and out.
- **Encryption:** attachments, thumbnails and exports of an encrypted
  workspace are encrypted too; zip exports open elsewhere with the
  passphrase. Encrypted workspaces also work on Windows.
- **Languages:** the command line, native dialogs and notifications use
  the chosen language (`--lang`, `GLYPHKEEL_LANG`, the app's setting or
  the system's); command-line help is translatable.
- **Agents:** read limits per session, adjustable in Preferences.
- **Queries:** dashboard widgets, colour rules, reports and automation
  use the query field with completion and field help.
- **Dates:** the date picker reads typed dates with the engine's parser.

### Fixes

- Pages use the full width, and down to the 760x520 minimum window no
  screen scrolls sideways; the view tabs fold into a menu and the task
  panel lies over the screen when narrow.
- Task panel values no longer break mid-phrase in a narrow panel.
- The tasks-by-field widget counts open tasks unless its query chooses
  by status.
- The rpm requires libcrypto 3, which Fedora 45 ships separately.

### Performance

- Without a GPU, the web view uses about 140 MB less memory and scrolls
  within the frame budget; lists and boards do less work per row.

## [0.1.9] - 2026-11-27

### Features

- **Dates:** Start and Due open a picker with one text field ("next fri",
  "+3d" or a date), a preview of the day and a calendar.
- **Dashboards:** included in exports and imports; a project's menu lists
  its pinned dashboards.
- **Renames and imports:** workspace rules, dashboard widgets and saved
  views tied to one project follow its renames; an import under a new
  key updates stored queries.
- **Command line:** `glyphkeel board` prints lanes and cards as a table.

### Fixes

- Editor suggestions (`[[`, `#`) are no longer hidden under the status
  bar and look like the app's menus.
- Task-panel popovers such as Repeats fit the window and keep their
  buttons visible; relative dates no longer break mid-phrase.
- An agent limited to some projects no longer sees other projects
  through dashboards or a watched task that moved.
- Menus announce the checked choice; the sprint and version pickers
  name their state.
- Windows: agent folders and backup checks with long path forms, and
  renaming a workspace by case.

## [0.1.8] - 2026-11-27

### Features

- **Sprints:** sprints per project with a goal and dates, a backlog and
  planning view, starting and completing with carry-over, a burndown
  chart, the `sprint:` query field, a board filter and a list column.
- **Automation rules:** a trigger, an optional query and actions (set
  fields, labels or status, comment, create a linked task, notify). Each
  run is recorded as the rule's own change and can be undone; loops and
  runaway rules are stopped. Rules follow renames and travel in exports.
- **Dashboards:** widgets for query counts and lists, tasks by field,
  due and overdue tasks, flow and time reports, sprint burndown, version
  progress, activity, automation failures and notes, arranged by pointer
  or keyboard.
- **Card colours and covers:** colour the card edge by type, priority,
  label or query rules, and show an image from the task as its cover.
- **Encryption and app lock:** encrypt a workspace with a passphrase
  (Linux), with encrypted backups; lock the app when idle or on demand.
- **Agents:** confirmations through the client for destructive or
  beyond-profile changes, subscriptions to tasks, projects and queries,
  and an optional MCP endpoint on 127.0.0.1 with a token.

### Fixes

- Every CSV export shares one guard against spreadsheet formulas.

## [0.1.7] - 2026-11-27

### Features

- **Agents' files:** paths an agent gives for exports, imports and
  attachments are limited to the workspace's exports folder and folders
  you add in Preferences.
- **Accessibility:** every screen checked in light, dark, sand and slate;
  stronger contrast, names that match the visible text, the page's
  language and direction from the locale, reduced motion and forced
  colours, a menu on every resize handle.
- **Column widths:** a saved width can be reset from the handle's menu or
  with a double-click.
- **Task shortcuts:** E edits the description; S, P, L and D open the
  status, priority, labels and due date of the open task.
- **Gantt:** tasks without dates no longer count against the chart's
  limit; a note counts them and shows them in the list.
- **About:** the licence and third-party notices in Preferences.
- **Packages:** an Arch package, signed APT and DNF repositories, and a
  manifest ready for Flathub.
- **Translation:** every message people see, in the app, the engine and
  the command line, comes from the message files.

### Performance

- CSV import of 10,000 rows in about half a second instead of 12.
- Large boards and schedules open in a fraction of a second: the store
  refreshes its statistics when they are stale.

### Fixes

- Several security fixes from a review of the web view, file access,
  diagrams and the MCP server.
- `glyphkeel board` no longer fails on very long table cells.
- `label:none` finds tasks without labels.

### Documentation

- A user guide, the query language, the command line, MCP set-up for
  agents, backup and restore, and the release procedure.

## [0.1.6] - 2026-11-26

### Features

- **Custom fields:** nine kinds per project, in the task panel, on board
  cards, as list and CSV columns, in grouping, bulk changes, search and
  queries (`cf.<name>`, including what a field was and when it changed).
  Required fields are checked on type changes and moves.
- **Versions:** versions per project, a fix version on tasks, a `version:`
  query field, a roadmap page and version markers on the Gantt.
- **Rules:** per status, fields required, read-only, set or cleared on
  entry, the timer stopped and claims released; per project, limits on
  what agents may change. Checked for every change from every client.
- **Recurring tasks and templates:** calendar and after-done repeats with
  skip and stop; task and project templates, a built-in Software project
  template, and a default due date per task type.
- **CSV import:** encoding and separator detection, column mapping to
  built-in and custom fields, saved presets and presets for Jira and
  Redmine exports, a dry run with row errors.
- **Flow reports:** created against closed, cumulative flow and cycle time.
- **Queries and planning:** history predicates (`was:`, `changed:`,
  `changed-by:`), lag on dependencies, a minimum next to the WIP limit,
  kinds of work on time entries, and closing duplicates with the original.
- **Projects and workspaces:** move tasks between projects with a mapping
  and a check, change a project's key with old keys kept, and switch,
  create and rename workspaces from the app, the command line and MCP.
- **MCP:** prompts for agents with argument completion.

## [0.1.5] - 2026-11-26

### Features

- **Windows:** the app and the command line build and run on Windows. An
  unsigned installer installs for the current user, puts `glyphkeel.exe`
  on the user's path, registers `glyphkeel://` links and keeps the data on
  uninstall. A second start hands over to the running app; reminders show
  as Windows notifications that open the task; links, attachments, file
  dialogs and confirmations use the system; the app can start with
  Windows, remembers its window, renders safely on request and uses the
  system title bar, Windows fonts and key names.
- **Folders:** `GLYPHKEEL_DATA_DIR` and `GLYPHKEEL_CONFIG_DIR` move the data
  and the personal settings on every platform.

### Fixes

- Imports and backup copies no longer fail on Windows with access denied.
- Exported file and folder names are valid on every system, including
  names such as `CON` or `a:b`.

## [0.1.4] - 2026-11-26

### Features

- **Wiki:** a page tree per project with a reader, an editor, history with
  word-level differences and restore, and an outline with links in and out.
  Pages can be moved, opened in their own window, given files, marked as a
  home page or a template, and use macros for a table of contents, child
  pages, included pages and live task lists. Checkboxes can be ticked in the
  reader. Pages export as Markdown or web pages, and a folder of Markdown
  files imports as a page tree.
- **Links:** links to wiki pages and tasks in pages, descriptions and
  comments are tracked, and each page and task lists what links to it.
- **Gantt:** a project's schedule as a Gantt chart: move and resize bars,
  draw and remove dependencies, edit dates in the table, drag unscheduled
  tasks onto the timeline, optionally move dependent tasks with an exact
  preview, group rows, list milestones, save views, mark tasks behind
  schedule, and export a picture or print. Works right to left and from the
  keyboard.
- **Schedule:** the schedule with dependency problems through the command
  line and the agent server, and moving dependents in one undoable change.
- **Calendar:** month and week grids with drag or keys to reschedule and
  quick add on a day.
- **Time report:** grouping by day, week, month, person, task or project,
  a person filter and CSV export.
- **Queries:** a `during:` field for tasks whose dates overlap a period.

### Fixes

- Moving a whole bar past its old due date, and undoing it, no longer fails.
- Moving a task whose dependents loop back to it is refused.

## [0.1.0] - 2026-11-24

First public release.

### Features

- **List:** group by status, priority, label, parent, due week or project,
  with an add-task row per group and folded groups remembered; several
  sort keys with Shift-click; columns can be moved and resized and are
  kept per project.
- **Views:** saved views show their descriptions, and renaming a status,
  type, label or project key updates the saved queries that use it.
- **Drafts:** unsent descriptions and comments are kept and offered back
  after a crash or restart.
- **Comments:** link to a comment as `TM-41#c-…` and quote a comment in a
  reply.
- **History:** a task's history can show everyone, only people or only
  agents.
- **Agents:** the limits on each agent session are settings, with no
  daily task limit by default. An agent server started with `--project`
  sees and changes only that project and its sub-projects.
- **Desktop:** start when you log in, and a safe rendering switch for
  graphics drivers that misbehave.
- **Packages:** a Flatpak bundle is built with each release.

### Fixes and changes

- A failed workspace upgrade puts back the backup taken just before it.
- Keyboard: Ctrl+1 and Ctrl+2 switch views, focus stays in the list after
  a query, and cards in long board lanes can be moved with the keyboard.
- Long values in the activity log are cut at 500 characters.
- Memory settings for the allocator and the web view are applied at
  start, without unsafe code.

## [0.0.8] - 2026-11-24

Ready for others: translation, preferences, packages and lower memory use.

### Features

- **Translation:** every message from the app, the engine, the command line
  and the agent server comes from translation files, with plurals and
  placeholders. Layout follows the writing direction.
- **Preferences:** Language, Notifications, a searchable Keyboard shortcuts
  page, and Agents & CLI, which shows whether the command is installed,
  offers to link it, gives set-up snippets for common MCP clients, lists
  agent sessions with a way to revert one, and shows the agent limits.
  The backup folder can be moved. Raising the agent limit asks first.
- **First run:** a short introduction, usable from the keyboard and
  skippable, appears after a workspace is created.
- **Activity:** filters by project, by people or agents, and by date run in
  the engine.
- **Command line:** `glyphkeel workspace` replaces `glyphkeel status`, which
  keeps working for one version.
- **Packages:** the deb and rpm carry the app and the command, shell
  completions, the man page, a desktop file, AppStream metadata and an icon.
  A Flatpak manifest is added.

### Fixes and changes

- Agents may create at most 500 tasks a day per session. An agent change
  touching more than 50 items takes a backup first and is refused if the
  backup fails. Wiki pages can't be changed while the wiki is switched off.
- Search pages no longer skip or repeat results when items change between
  pages.
- Deeply nested Markdown no longer crashes an import.
- Lower memory use: the board fetches closed cards only after "Show older",
  and the app returns freed memory to the system.
- The window no longer stays blank when the system locale is C or POSIX.
- Esc after choosing from a list inside a dialog now closes the dialog.

## [0.0.7] - 2026-11-24

History and safety: a trash, backups, archives and a feed of every change.

### Features

- **Trash:** deleted tasks, comments, files, time entries, wiki pages and
  projects go to the trash, with Undo, and can be restored from the Trash
  screen. Items are purged after the number of days set for the workspace;
  emptying the trash is permanent and asks first. Agents may delete only a
  few items at a time, and only full access may delete projects or purge.
- **Backups:** a daily backup runs in the background, and the command line
  takes one when the last is overdue. Daily, weekly and other backups are
  rotated. In Preferences → Data and backups you can back up now, check a
  backup, restore it or delete it; restoring and deleting ask first, and the
  app reopens the workspace after a restore.
- **Export and import:** export the whole workspace or chosen projects as a
  zip archive or a folder, and import such an archive. Import checks the
  whole archive first, shows what it will add and which task keys are taken,
  lets you choose new keys, and can be undone. Unsafe archives are refused
  before anything is written.
- **Activity:** a screen listing every change in the workspace, by people and
  agents, with filters and revert where possible. The task's History tab
  shows deletions and restores.
- **Command line and agents:** `glyphkeel trash`, `glyphkeel backup`,
  `glyphkeel export` and `glyphkeel import`, and the matching agent tools.

### Fixes

- A description or comment edited in another window or by an agent while you
  were editing it is no longer overwritten when you save.
- A running timer stops, and its time is logged, when its task goes to the
  trash.

## [0.0.6] - 2026-11-23

Today, search across the whole workspace, a command palette and quick add from
anywhere on the desktop.

### Features

- **Today:** a start page with what is overdue, due today and in progress,
  reminders, tasks agents are working on, what changed in the last day and the
  running timer. The sidebar shows how many tasks are due today and open.
- **Global search:** one search across tasks, comments, wiki pages, files and
  projects, ranked, with the matching words highlighted and filters by kind
  and project. Typing in the sidebar's search field suggests tasks, projects
  and actions at once; Enter opens all results.
- **Command palette:** press Ctrl+K to run any action, jump to a task by its
  key or words, or go to a project, all from the keyboard.
- **Quick add:** a small window to capture a task with its project, priority
  and due date in one line (`Renew domain fri #Personal !high`). Open it from
  the tray, the menu, `glyphkeel --quick-add`, or a global keyboard shortcut
  you can turn on in Preferences. The window starts only when first used.
- **Links in the editor:** type `#` to link a task by its title or key, and
  `[[` to link a wiki page or one of its headings, or use the new toolbar
  buttons. Wiki links open the page.
- **Wiki pages from the command line and for agents:** list, search, read,
  create and update wiki pages with `glyphkeel wiki` and through the agent
  server.
- **Dates in words:** start and due dates accept "tomorrow", "next fri",
  "in 3d" or "dec 1" in the task panel, in quick add, on the command line and
  for agents, all read the same way.

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
