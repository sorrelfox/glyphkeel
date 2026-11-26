<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Generated from the operation catalog by `cargo xtask gen`. Do not edit. -->

# Operations

Everything Glyphkeel can do is an operation of one catalog. The desktop app, the `glyphkeel` command and the MCP server for agents are three views of it, so they offer the same operations with the same rules.

The safety class decides who may run an operation:

- `read` changes nothing;
- `write` creates or changes data and can be undone;
- `trash` moves data to the trash, from where it can be restored;
- `permanent` can't be undone in the app and is confirmed first;
- `app` changes the application itself and is never offered to agents.

| Operation | Class | Reachable from | MCP tool | Command |
|---|---|---|---|---|
| `workspace.get` | read | app, command line, MCP | `get_workspace` | `glyphkeel workspace` |
| `settings.get` | read | app, command line, MCP | `get_workspace` | `glyphkeel settings get` |
| `settings.update` | write | app, command line, MCP | `update_settings` | `glyphkeel settings set` |
| `search.query` | read | app, command line, MCP | `search` | `glyphkeel search <text>` |
| `project.list` | read | app, command line, MCP | `list_projects` | `glyphkeel project list` |
| `project.get` | read | app, command line, MCP | `get_project` | `glyphkeel project show <project>` |
| `project.create` | write | app, command line, MCP | `create_project` | `glyphkeel project create` |
| `project.update` | write | app, command line, MCP | `update_project` | `glyphkeel project edit <project>` |
| `project.move` | write | app, command line, MCP | `update_project` | `glyphkeel project move <project>` |
| `project.change_key` | write | app, command line, MCP | `update_project` | `glyphkeel project rekey <project>` |
| `project.archive` | write | app, command line, MCP | `update_project` | `glyphkeel project archive <project>` |
| `project.unarchive` | write | app, command line, MCP | `update_project` | `glyphkeel project unarchive <project>` |
| `project.delete` | trash | app, command line, MCP | `delete_project` | `glyphkeel project delete <project>` |
| `project.brief` | read | app, command line, MCP | `get_agent_brief` | `glyphkeel project brief <project>` |
| `status.create` | write | app, command line, MCP | `update_project` | `glyphkeel status add` |
| `status.update` | write | app, command line, MCP | `update_project` | `glyphkeel status edit` |
| `status.reorder` | write | app, command line, MCP | `update_project` | `glyphkeel status move` |
| `status.delete` | permanent | app, command line, MCP | `update_project` | `glyphkeel status delete` |
| `type.create` | write | app, command line, MCP | `update_project` | `glyphkeel type add` |
| `type.update` | write | app, command line, MCP | `update_project` | `glyphkeel type edit` |
| `type.delete` | permanent | app, command line, MCP | `update_project` | `glyphkeel type delete` |
| `label.create` | write | app, command line, MCP | `update_project` | `glyphkeel label add` |
| `label.update` | write | app, command line, MCP | `update_project` | `glyphkeel label edit` |
| `label.delete` | permanent | app, command line, MCP | `update_project` | `glyphkeel label delete` |
| `task.search` | read | app, command line, MCP | `search_tasks` | `glyphkeel task list` |
| `task.get` | read | app, command line, MCP | `get_task` | `glyphkeel task show <keys>` |
| `task.create` | write | app, command line, MCP | `create_task` | `glyphkeel task create` |
| `task.update` | write | app, command line, MCP | `update_task` | `glyphkeel task edit <key>` |
| `task.move` | write | app, command line, MCP | `move_task` | `glyphkeel task move <key>` |
| `task.delete` | trash | app, command line, MCP | `delete_tasks` | `glyphkeel task delete <keys>` |
| `task.bulk_update` | write | app, command line, MCP | `bulk_update_tasks` | `glyphkeel task bulk` |
| `task.reorder_subtask` | write | app, command line, MCP | `reorder_subtask` | `glyphkeel task reorder-subtask <key>` |
| `task.resolve_conflict` | write | app, command line, MCP | `update_task` | `glyphkeel task resolve <key>` |
| `checklist.update` | write | app, command line, MCP | `update_checklist` | `glyphkeel task check <key>` |
| `task.duplicate` | write | app, command line, MCP | `duplicate_task` | `glyphkeel task duplicate <key>` |
| `task.claim` | write | app, command line, MCP | `claim_task` | `glyphkeel task claim <key>` |
| `task.release` | write | app, command line, MCP | `release_task` | `glyphkeel task release <key>` |
| `relation.add` | write | app, command line, MCP | `link_tasks` | `glyphkeel task link <key>` |
| `relation.remove` | write | app, command line, MCP | `unlink_tasks` | `glyphkeel task unlink <key>` |
| `description.history` | read | app, command line, MCP | `get_task` | `glyphkeel description history` |
| `description.revision` | read | app, command line, MCP | `get_description_revision` | `glyphkeel description show <revision>` |
| `markdown.render` | read | app | — | — |
| `comment.list` | read | app, command line, MCP | `get_task` | `glyphkeel comment list <key>` |
| `comment.add` | write | app, command line, MCP | `add_comment` | `glyphkeel comment add <key>` |
| `comment.update` | write | app, command line, MCP | `update_comment` | `glyphkeel comment edit <comment>` |
| `comment.delete` | trash | app, command line, MCP | `delete_comment` | `glyphkeel comment delete <comment>` |
| `draft.get` | read | app | — | — |
| `draft.list` | read | app | — | — |
| `draft.save` | app | app | — | — |
| `draft.discard` | app | app | — | — |
| `time.log` | write | app, command line, MCP | `log_time` | `glyphkeel time log <key>` |
| `time.update` | write | app, command line, MCP | `update_time_entry` | `glyphkeel time edit <entry>` |
| `time.delete` | trash | app, command line, MCP | `delete_time_entry` | `glyphkeel time delete <entry>` |
| `time.report` | read | app, command line, MCP | `get_time_report` | `glyphkeel time report` |
| `timer.get` | read | app, command line, MCP | `get_workspace` | `glyphkeel timer status` |
| `timer.start` | write | app, command line, MCP | `start_timer` | `glyphkeel timer start <key>` |
| `timer.pause` | write | app, command line, MCP | `pause_timer` | `glyphkeel timer pause` |
| `timer.stop` | write | app, command line, MCP | `stop_timer` | `glyphkeel timer stop` |
| `reminder.set` | write | app, command line, MCP | `set_reminder` | `glyphkeel reminder set <key>` |
| `reminder.clear` | write | app, command line, MCP | `clear_reminder` | `glyphkeel reminder clear <key>` |
| `reminder.due` | read | app, command line, MCP | `get_reminders` | `glyphkeel reminder due` |
| `reminder.acknowledge` | write | app, command line, MCP | `acknowledge_reminders` | `glyphkeel reminder ack <ids>` |
| `today.get` | read | app, command line, MCP | `get_today` | `glyphkeel today` |
| `board.get` | read | app, command line, MCP | `get_board` | `glyphkeel board <project>` |
| `layout.get` | read | app, command line, MCP | `get_board` | `glyphkeel board layout show <project>` |
| `layout.update` | write | app, command line, MCP | `set_board_layout` | `glyphkeel board layout set <project>` |
| `view.list` | read | app, command line, MCP | `list_views` | `glyphkeel view list` |
| `view.create` | write | app, command line, MCP | `create_view` | `glyphkeel view create` |
| `view.update` | write | app, command line, MCP | `update_view` | `glyphkeel view edit <view>` |
| `view.delete` | write | app, command line, MCP | `delete_view` | `glyphkeel view delete <view>` |
| `list.layout.get` | read | app | — | — |
| `list.layout.update` | write | app | — | — |
| `query.parse` | read | app | — | — |
| `schedule.get` | read | app, command line, MCP | `get_schedule` | `glyphkeel schedule` |
| `schedule.preview_move` | read | app | — | — |
| `gantt.layout.get` | read | app | — | — |
| `gantt.layout.update` | write | app | — | — |
| `wiki.list` | read | app, command line, MCP | `list_wiki_pages` | `glyphkeel wiki list` |
| `wiki.search` | read | app, command line, MCP | `search_wiki_pages` | `glyphkeel wiki search <text>` |
| `wiki.get` | read | app, command line, MCP | `get_wiki_page` | `glyphkeel wiki get <page>` |
| `wiki.create` | write | app, command line, MCP | `create_wiki_page` | `glyphkeel wiki new` |
| `wiki.update` | write | app, command line, MCP | `update_wiki_page` | `glyphkeel wiki put <page>` |
| `wiki.move` | write | app, command line, MCP | `update_wiki_page` | `glyphkeel wiki move <page>` |
| `wiki.delete` | trash | app, command line, MCP | `delete_wiki_page` | `glyphkeel wiki delete <page>` |
| `wiki.history` | read | app, command line, MCP | `get_wiki_history` | `glyphkeel wiki history <page>` |
| `wiki.export` | write | app, command line, MCP | `export_wiki_page` | `glyphkeel wiki export <page>` |
| `wiki.import` | write | app, command line, MCP | `import_wiki_pages` | `glyphkeel wiki import <path>` |
| `wiki.backlinks` | read | app, command line, MCP | `get_wiki_page` | `glyphkeel wiki backlinks <page>` |
| `wiki.references` | read | app, command line, MCP | `get_wiki_page` | `glyphkeel wiki references <page>` |
| `task.backlinks` | read | app, command line, MCP | `get_task` | `glyphkeel task backlinks <key>` |
| `task.references` | read | app, command line, MCP | `get_task` | `glyphkeel task references <key>` |
| `file.get` | read | app, command line, MCP | `read_attachment` | `glyphkeel file show <attachment>` |
| `file.read` | read | app, command line, MCP | `read_attachment` | `glyphkeel file cat <attachment>` |
| `file.thumbnail` | read | app | — | — |
| `file.attach` | write | app, command line, MCP | `attach_file` | `glyphkeel file attach` |
| `file.delete` | trash | app, command line, MCP | `delete_attachment` | `glyphkeel file delete <attachment>` |
| `agent.sessions` | read | app, command line, MCP | `get_activity` | `glyphkeel agents sessions` |
| `agent.policy.update` | app | app, command line | — | `glyphkeel agents policy` |
| `agent.limits.update` | app | app | — | — |
| `history.feed` | read | app, command line, MCP | `get_activity` | `glyphkeel activity` |
| `history.revert` | write | app, command line, MCP | `revert_changes` | `glyphkeel revert` |
| `history.undo` | app | app | — | — |
| `history.redo` | app | app | — | — |
| `trash.list` | read | app, command line, MCP | `list_trash` | `glyphkeel trash list` |
| `trash.restore` | write | app, command line, MCP | `restore_from_trash` | `glyphkeel trash restore <items>` |
| `trash.purge` | permanent | app, command line, MCP | `purge_trash` | `glyphkeel trash purge <items>` |
| `backup.create` | write | app, command line, MCP | `create_backup` | `glyphkeel backup create` |
| `backup.list` | read | app, command line, MCP | `list_backups` | `glyphkeel backup list` |
| `backup.verify` | read | app, command line, MCP | `verify_backup` | `glyphkeel backup verify <name>` |
| `backup.restore` | app | app, command line | — | `glyphkeel backup restore <name>` |
| `backup.delete` | permanent | app, command line | — | `glyphkeel backup delete <name>` |
| `backup.folder.update` | app | app | — | — |
| `transfer.export` | write | app, command line, MCP | `export_data` | `glyphkeel export` |
| `transfer.import` | write | app, command line, MCP | `import_data` | `glyphkeel import <path>` |

## `workspace.get`

The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings, the limits on agents and a short guide.

Input: `WorkspaceGetInput`. Output: `WorkspaceInfo`. Schemas: [catalog.schema.json](catalog.schema.json).

## `settings.get`

Read the workspace settings.

Input: `SettingsGetInput`. Output: `Settings`. Schemas: [catalog.schema.json](catalog.schema.json).

## `settings.update`

Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder.

Input: `SettingsPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `search.query`

Search task titles and descriptions, comments, wiki pages and file names for words and fragments, ranked, with the matches marked; a task key finds that task.

Input: `SearchInput`. Output: `SearchResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.list`

The project tree in sidebar order, with keys, names and open-task counts; archived projects on request.

Input: `ProjectListInput`. Output: `ProjectList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.get`

One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts.

Input: `ProjectRef`. Output: `Project`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.create`

Create a project, or a sub-project that copies its parent's statuses, types and labels; without a colour it gets one its sibling projects don't use yet.

Input: `ProjectCreateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.update`

Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change.

Input: `ProjectUpdateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.move`

Move a project under another one (at most 3 levels), to the top level, or among its siblings.

Input: `ProjectMoveInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.change_key`

Give a project a new key; the old key and the old task keys keep working.

Input: `ProjectRekeyInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.archive`

Archive a project and its sub-projects: they become read-only and leave the sidebar.

Input: `ProjectRef`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.unarchive`

Unarchive a project and its sub-projects.

Input: `ProjectRef`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.delete`

Move a project and its sub-projects, with everything in them, to the trash. Agents need the full profile.

Input: `ProjectDeleteInput`. Output: `ProjectDeleted`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.brief`

The project's instructions for agents: the wiki page marked as the brief, else the project's description.

Input: `ProjectRef`. Output: `AgentBrief`. Schemas: [catalog.schema.json](catalog.schema.json).

## `status.create`

Add a status (a board column) to a project.

Input: `StatusCreateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `status.update`

Rename or recolour a status, change its category or its WIP limit.

Input: `StatusUpdateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `status.reorder`

Move a status in the project's board order.

Input: `StatusReorderInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `status.delete`

Delete a status; its tasks move to the status named. Can't be undone.

Input: `StatusDeleteInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `type.create`

Add a task type to a project.

Input: `TypeCreateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `type.update`

Rename, reshape, recolour or reorder a task type, or make it the default.

Input: `TypeUpdateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `type.delete`

Delete a task type; its tasks get another type. Can't be undone.

Input: `TypeDeleteInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `label.create`

Add a label to a project; without a colour it gets the first palette colour the project's labels don't use yet.

Input: `LabelCreateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `label.update`

Rename or recolour a label.

Input: `LabelUpdateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `label.delete`

Delete a label and take it off every task. Can't be undone.

Input: `LabelDeleteInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.search`

List the tasks matching a query (is:open due<+7d label:bug sort:due), a page at a time, with totals of estimates and logged time.

Input: `TaskSearchInput`. Output: `TaskSearchResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.get`

One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request.

Input: `TaskGetInput`. Output: `TaskList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.create`

Create a task from a title alone or with its fields; it gets the project's next key.

Input: `TaskCreateInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.update`

Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress.

Input: `TaskUpdateInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.move`

Move a task to another status or place in its column, or to another project (new key; the old one forwards).

Input: `TaskMoveInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.delete`

Move tasks to the trash, each with its subtasks; they can be restored from there.

Input: `TaskDeleteInput`. Output: `TrashChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.bulk_update`

Make the same change to many tasks chosen by key or query (status, type, priority, labels, parent, dates set or shifted, estimate, project), previewed first, as one change for Undo and revert.

Input: `BulkUpdateInput`. Output: `BulkResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.reorder_subtask`

Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order.

Input: `SubtaskReorderInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.resolve_conflict`

Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text.

Input: `TaskResolveInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `checklist.update`

Tick, clear, add or remove checklist items in a task's description, a comment or a wiki page, by number plus expected text.

Input: `ChecklistUpdateInput`. Output: `ChecklistChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.duplicate`

Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone.

Input: `TaskDuplicateInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.claim`

Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress.

Input: `TaskClaimInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.release`

Give up a claim on a task, optionally moving it and adding a comment.

Input: `TaskReleaseInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `relation.add`

Link two tasks (blocks, relates, duplicates, clones, or make one a subtask of the other).

Input: `RelationInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `relation.remove`

Remove a link between two tasks, or a subtask from its parent.

Input: `RelationInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `description.history`

List the revisions of a task's or project's description, newest first, conflict copies included.

Input: `DescriptionHistoryInput`. Output: `RevisionList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `description.revision`

One description revision with its text, and the differences from another revision on request.

Input: `DescriptionRevisionInput`. Output: `RevisionDetail`. Schemas: [catalog.schema.json](catalog.schema.json).

## `markdown.render`

Render descriptions, comments or editor text to the reader's sanitized HTML, with numbered checkboxes and linked task keys.

Input: `RenderInput`. Output: `RenderOutput`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It produces HTML for the app's reader; agents and the command line read the Markdown itself.

## `comment.list`

A task's comments, oldest first, optionally with their earlier texts.

Input: `CommentListInput`. Output: `CommentList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `comment.add`

Add a Markdown comment to a task, optionally answering another.

Input: `CommentAddInput`. Output: `CommentChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `comment.update`

Change a comment's text; the earlier text is kept and can be viewed.

Input: `CommentUpdateInput`. Output: `CommentChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `comment.delete`

Move a comment to the trash.

Input: `CommentRef`. Output: `CommentChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `draft.get`

The person's unsaved text of one editor (a description, a comment, a new comment or a wiki page), kept from earlier.

Input: `DraftRef`. Output: `DraftOutput`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: A draft is the person's own unsaved text in an editor of the app, not the workspace's data; agents and scripts save text through the operations that change it.

## `draft.list`

The person's kept drafts, most recently saved first; or only those about one task.

Input: `DraftListInput`. Output: `DraftList`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: A draft is the person's own unsaved text in an editor of the app, not the workspace's data; agents and scripts save text through the operations that change it.

## `draft.save`

Keep the unsaved text of an editor, replacing the person's earlier draft of it. Drafts are not history and expire after 30 days.

Input: `DraftSaveInput`. Output: `DraftOutput`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: A draft is the person's own unsaved text in an editor of the app, not the workspace's data; agents and scripts save text through the operations that change it.

## `draft.discard`

Forget the person's unsaved text of an editor.

Input: `DraftRef`. Output: `DraftDiscarded`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: A draft is the person's own unsaved text in an editor of the app, not the workspace's data; agents and scripts save text through the operations that change it.

## `time.log`

Log time on a task: a duration, the day of the work (default today) and a note.

Input: `TimeLogInput`. Output: `TimeEntryChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `time.update`

Change a time entry's duration, day or note.

Input: `TimeUpdateInput`. Output: `TimeEntryChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `time.delete`

Move a time entry to the trash.

Input: `TimeEntryRef`. Output: `TimeEntryChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `time.report`

Logged time over a range of days, grouped by day, week, month, task, project or person, with totals in the workspace's units and optional CSV.

Input: `TimeReportInput`. Output: `TimeReport`. Schemas: [catalog.schema.json](catalog.schema.json).

## `timer.get`

The workspace's timer: its task, whether it runs and the time so far.

Input: `TimerGetInput`. Output: `TimerStatus`. Schemas: [catalog.schema.json](catalog.schema.json).

## `timer.start`

Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged.

Input: `TimerStartInput`. Output: `TimerChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `timer.pause`

Pause the running timer; it keeps its time.

Input: `TimerPauseInput`. Output: `TimerChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `timer.stop`

Stop the timer and log its time as an entry on its task.

Input: `TimerStopInput`. Output: `TimerChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `reminder.set`

Set a task's personal reminder: a moment, or a number of days before its due date.

Input: `ReminderSetInput`. Output: `ReminderChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `reminder.clear`

Remove a task's personal reminder.

Input: `ReminderClearInput`. Output: `ReminderChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `reminder.due`

The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes.

Input: `ReminderDueInput`. Output: `DueReminders`. Schemas: [catalog.schema.json](catalog.schema.json).

## `reminder.acknowledge`

Acknowledge reminders that were shown, so they don't come due again.

Input: `ReminderAckInput`. Output: `ReminderAcknowledged`. Schemas: [catalog.schema.json](catalog.schema.json).

## `today.get`

What needs attention today across projects: overdue tasks, tasks due today, tasks in progress, today's reminders, agents' claims, recent changes and the timer, with counts for the sidebar.

Input: `TodayInput`. Output: `Today`. Schemas: [catalog.schema.json](catalog.schema.json).

## `board.get`

A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards.

Input: `BoardGetInput`. Output: `Board`. Schemas: [catalog.schema.json](catalog.schema.json).

## `layout.get`

The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields.

Input: `LayoutGetInput`. Output: `LayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `layout.update`

Hide, show, collapse or reorder board columns, set sort modes, card fields and older closed cards, or reset to the project's layout.

Input: `LayoutUpdateInput`. Output: `LayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `view.list`

The saved views of a project or of the whole workspace: name, query, columns, order, grouping, totals and which is the default.

Input: `ViewListInput`. Output: `ViewList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `view.create`

Save a query as a named view of a project or of the whole workspace, with its columns, order, grouping and totals.

Input: `ViewCreateInput`. Output: `ViewChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `view.update`

Rename a saved view, change its query, columns, order, grouping or totals, or make it the default.

Input: `ViewUpdateInput`. Output: `ViewChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `view.delete`

Delete a saved view; it can be brought back with revert.

Input: `ViewDeleteInput`. Output: `ViewChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `list.layout.get`

The person's layout of a project's list, or of the whole workspace's: column order and widths, and the folded groups.

Input: `ListLayoutGetInput`. Output: `ListLayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It holds how the person's own list screen looks (the order and widths of its columns), not the workspace's data; agents and scripts choose columns in saved views.

## `list.layout.update`

Reorder the columns of a list, change their widths, fold or unfold its groups, or go back to the default layout.

Input: `ListLayoutUpdateInput`. Output: `ListLayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It changes how the person's own list screen looks (the order and widths of its columns), not the workspace's data; agents and scripts choose columns in saved views.

## `query.parse`

Read a task query for the query field: highlighted pieces, the chips of the condition builder, the normal form and the first mistake with its place.

Input: `QueryParseInput`. Output: `ParsedQuery`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It serves the app's query field as the person types; agents and scripts run queries with task.search, which reports the same mistakes.

## `schedule.get`

The schedule of a project or a query for the Gantt chart: dated tasks as a tree with summary ranges for parents, milestones, the tasks without dates, blocks links as finish-to-start dependencies, and problems such as a task starting before its blocker ends.

Input: `ScheduleInput`. Output: `Schedule`. Schemas: [catalog.schema.json](catalog.schema.json).

## `schedule.preview_move`

The tasks that "also move dependents" would move, and their new dates, if a task got the given dates; nothing is changed.

Input: `SchedulePreviewInput`. Output: `SchedulePreview`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It previews a drag in the Gantt chart while the pointer moves; agents and scripts change dates with update_task and move_dependents, whose answer lists the tasks that moved.

## `gantt.layout.get`

The person's layout of a project's Gantt chart: zoom level, table and column widths, grouping, and whether moving a task also moves its dependents.

Input: `GanttLayoutGetInput`. Output: `GanttLayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It holds how the person's own Gantt chart looks, not the workspace's data; agents and scripts read the schedule itself.

## `gantt.layout.update`

Change the zoom level, table or column widths, grouping or the "also move dependents" switch of a project's Gantt chart, or go back to the default layout.

Input: `GanttLayoutUpdateInput`. Output: `GanttLayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It changes how the person's own Gantt chart looks, not the workspace's data; agents and scripts read the schedule itself.

## `wiki.list`

A project's wiki pages as a tree: titles, slugs, nesting and when each changed.

Input: `WikiListInput`. Output: `WikiTree`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.search`

Find wiki pages by title, heading or text, for example to link one; `Page#Heading` lists a page's headings.

Input: `WikiSearchInput`. Output: `WikiSearchResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.get`

One wiki page with its text, revision, headings, place in the tree and, on request, what links to it; or an older revision of it.

Input: `WikiGetInput`. Output: `WikiPage`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.create`

Create a wiki page with a title unique in its project, optionally inside another page and with its text.

Input: `WikiCreateInput`. Output: `WikiChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.update`

Change a wiki page: its text (whole, with a base revision, or by targeted edits), title, parent or place, whether it is the home page; or resolve a conflict copy.

Input: `WikiUpdateInput`. Output: `WikiChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.move`

Move a wiki page, with the pages inside it, into another parent page or to the top, and to a place among its siblings; refuses moves into itself; links to it keep working.

Input: `WikiMoveInput`. Output: `WikiChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.delete`

Move a wiki page, with the pages inside it, to the trash; links to it stop resolving until it is restored.

Input: `WikiDeleteInput`. Output: `TrashChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.history`

List the revisions of a wiki page, newest first, conflict copies included; read one with wiki.get and its revision.

Input: `WikiHistoryInput`. Output: `RevisionList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.export`

Export a wiki page, optionally with the pages inside it, or a project's whole wiki, as Markdown files or web pages with their attachments, in a .zip file or a folder; links between the exported pages and to the attachments become relative links.

Input: `WikiExportInput`. Output: `WikiExportResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.import`

Import a folder of Markdown files as wiki pages: sub-folders become pages inside pages, relative links between the files and `[[…]]` links become wiki links, and linked images and files are attached to their pages; one change that Undo takes back.

Input: `WikiImportInput`. Output: `WikiImportResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.backlinks`

The wiki pages, task descriptions and comments that link to a wiki page, by its title, slug, an old slug or its ID.

Input: `WikiLinksInput`. Output: `BacklinkList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `wiki.references`

The pages and tasks a wiki page's text links to, in order, each resolved now; links that find nothing are marked.

Input: `WikiLinksInput`. Output: `ReferenceList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.backlinks`

The wiki pages, other tasks' descriptions and comments that mention a task by its key (current or old).

Input: `TaskLinksInput`. Output: `BacklinkList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.references`

The pages and tasks a task's description and comments link to, in order, each resolved now; links that find nothing are marked.

Input: `TaskLinksInput`. Output: `ReferenceList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `file.get`

An attached file's name, type, size and owner.

Input: `FileRef`. Output: `Attachment`. Schemas: [catalog.schema.json](catalog.schema.json).

## `file.read`

An attached file's content: text for text files, Base64 for images, otherwise only its details.

Input: `FileRef`. Output: `FileContent`. Schemas: [catalog.schema.json](catalog.schema.json).

## `file.thumbnail`

A small PNG of an attached PNG, JPEG, GIF or WebP image, at most 256 pixels wide and high, from the workspace's cache; made when the image is attached, and again when the cache has lost it.

Input: `FileRef`. Output: `Thumbnail`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It gives a path in the workspace's cache, which the app serves to its pages; agents and the command line read images with file.read.

## `file.attach`

Attach a local file, or Base64 data with a name, to a task, a comment or a wiki page.

Input: `FileAttachInput`. Output: `FileChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `file.delete`

Move an attached file to the trash.

Input: `FileRef`. Output: `FileChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `agent.sessions`

List agent sessions, newest first, with their client, profile, times and counts of reads and writes.

Input: `AgentSessionsInput`. Output: `AgentSessionList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `agent.policy.update`

Set the most an agent may do, whatever profile it asks for.

Input: `AgentPolicyPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It sets the rights of agents; an agent that could call it would raise its own rights beyond what the person allowed.

## `agent.limits.update`

Change the limits on agent sessions: changes a minute, new tasks a day, and items moved to the trash per call and per hour. Each is a number or no limit.

Input: `AgentLimitsPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It sets how much agents may do; an agent that could call it would raise its own limits beyond what the person allowed.

## `history.feed`

The activity log, newest first: who changed what, when and through which program; optionally the agent sessions.

Input: `ActivityQuery`. Output: `ActivityPage`. Schemas: [catalog.schema.json](catalog.schema.json).

## `history.revert`

Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed.

Input: `RevertInput`. Output: `RevertResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `history.undo`

Undo the last change made in this run of the app.

Input: `UndoInput`. Output: `UndoResult`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: Undo walks the app's own stack of changes made in this run; agents and the command line revert named groups instead.

## `history.redo`

Redo the change the last Undo took back.

Input: `UndoInput`. Output: `UndoResult`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: Redo walks the app's own stack of changes made in this run; agents and the command line revert named groups instead.

## `trash.list`

The trash: deleted items with what went with them, who deleted them and when they will be purged, newest first.

Input: `TrashListInput`. Output: `TrashList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `trash.restore`

Bring items back out of the trash, with everything that was deleted with them.

Input: `TrashRestoreInput`. Output: `TrashChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `trash.purge`

Delete items in the trash for good, and their text from the history. Can't be undone.

Input: `TrashPurgeInput`. Output: `TrashChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `backup.create`

Make a backup of the workspace now: a checked copy of the database and its stored files.

Input: `BackupCreateInput`. Output: `Backup`. Schemas: [catalog.schema.json](catalog.schema.json).

## `backup.list`

The workspace's backups, newest first, with their kind and size.

Input: `BackupListInput`. Output: `BackupList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `backup.verify`

Check a backup: the database copy's integrity and every stored file it needs.

Input: `BackupRef`. Output: `BackupCheck`. Schemas: [catalog.schema.json](catalog.schema.json).

## `backup.restore`

Replace the workspace with a backup, after backing up the current state. Other programs with the workspace open must restart.

Input: `BackupRef`. Output: `BackupRestored`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: it replaces the whole workspace, including every agent's own work, so only the person may do it

## `backup.delete`

Delete a backup and the stored files only it used.

Input: `BackupRef`. Output: `BackupDeleted`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: backups are the person's safety net outside the workspace, so agents never remove them

## `backup.folder.update`

Move the backup folder to another place, or back to the default; backups already made stay where they are.

Input: `BackupFolderPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: it decides where copies of all the person's data are written, so only the person may set it, in the app

## `transfer.export`

Export projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments.

Input: `ExportInput`. Output: `ExportResult`. Schemas: [catalog.schema.json](catalog.schema.json).

## `transfer.import`

Import an archive as new projects; checks it completely first and never changes existing data.

Input: `ImportInput`. Output: `ImportResult`. Schemas: [catalog.schema.json](catalog.schema.json).
