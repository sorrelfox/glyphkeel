<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Generated from the operation catalog by `cargo xtask gen`. Do not edit. -->

# Command line

`glyphkeel <area> <verb> [arguments] [--options]`. Every command runs one operation of the catalog ([operations.md](operations.md)). Global options: `--workspace`, `--json`, `--quiet`, `--no-color`.

With `--json` a command prints exactly the operation's output, as described in [catalog.schema.json](catalog.schema.json).

| Command | Operation | What it does |
|---|---|---|
| `glyphkeel activity` | `history.feed` | The activity log, newest first: who changed what, when and through which program; optionally the agent sessions. |
| `glyphkeel agents policy` | `agent.policy.update` | Set the most an agent may do, whatever profile it asks for. |
| `glyphkeel agents sessions` | `agent.sessions` | List agent sessions, newest first, with their client, profile, times and counts of reads and writes. |
| `glyphkeel board <project>` | `board.get` | A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards. |
| `glyphkeel board layout set <project>` | `layout.update` | Hide, show, collapse or reorder board columns, set sort modes, card fields and older closed cards, or reset to the project's layout. |
| `glyphkeel board layout show <project>` | `layout.get` | The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields. |
| `glyphkeel comment add <key>` | `comment.add` | Add a Markdown comment to a task, optionally answering another. |
| `glyphkeel comment delete <comment>` | `comment.delete` | Move a comment to the trash. |
| `glyphkeel comment edit <comment>` | `comment.update` | Change a comment's text; the earlier text is kept and can be viewed. |
| `glyphkeel comment list <key>` | `comment.list` | A task's comments, oldest first, optionally with their earlier texts. |
| `glyphkeel description history` | `description.history` | List the revisions of a task's or project's description, newest first, conflict copies included. |
| `glyphkeel description show <revision>` | `description.revision` | One description revision with its text, and the differences from another revision on request. |
| `glyphkeel file attach` | `file.attach` | Attach a local file, or Base64 data with a name, to a task or a comment. |
| `glyphkeel file cat <attachment>` | `file.read` | An attached file's content: text for text files, Base64 for images, otherwise only its details. |
| `glyphkeel file delete <attachment>` | `file.delete` | Move an attached file to the trash. |
| `glyphkeel file show <attachment>` | `file.get` | An attached file's name, type, size and owner. |
| `glyphkeel label add` | `label.create` | Add a label to a project. |
| `glyphkeel label delete` | `label.delete` | Delete a label and take it off every task. Can't be undone. |
| `glyphkeel label edit` | `label.update` | Rename or recolour a label. |
| `glyphkeel project archive <project>` | `project.archive` | Archive a project and its sub-projects: they become read-only and leave the sidebar. |
| `glyphkeel project brief <project>` | `project.brief` | The project's instructions for agents: the wiki page marked as the brief, else the project's description. |
| `glyphkeel project create` | `project.create` | Create a project, or a sub-project that copies its parent's statuses, types and labels. |
| `glyphkeel project delete <project>` | `project.delete` | Delete a project and its sub-projects with every task, comment and file in them. Can't be undone. |
| `glyphkeel project edit <project>` | `project.update` | Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change. |
| `glyphkeel project list` | `project.list` | The project tree in sidebar order, with keys, names and open-task counts; archived projects on request. |
| `glyphkeel project move <project>` | `project.move` | Move a project under another one (at most 3 levels), to the top level, or among its siblings. |
| `glyphkeel project rekey <project>` | `project.change_key` | Give a project a new key; the old key and the old task keys keep working. |
| `glyphkeel project show <project>` | `project.get` | One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts. |
| `glyphkeel project unarchive <project>` | `project.unarchive` | Unarchive a project and its sub-projects. |
| `glyphkeel reminder ack <ids>` | `reminder.acknowledge` | Acknowledge reminders that were shown, so they don't come due again. |
| `glyphkeel reminder clear <key>` | `reminder.clear` | Remove a task's personal reminder. |
| `glyphkeel reminder due` | `reminder.due` | The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes. |
| `glyphkeel reminder set <key>` | `reminder.set` | Set a task's personal reminder: a moment, or a number of days before its due date. |
| `glyphkeel revert` | `history.revert` | Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed. |
| `glyphkeel settings get` | `settings.get` | Read the workspace settings. |
| `glyphkeel settings set` | `settings.update` | Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder. |
| `glyphkeel status` | `workspace.get` | The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide. |
| `glyphkeel status add` | `status.create` | Add a status (a board column) to a project. |
| `glyphkeel status delete` | `status.delete` | Delete a status; its tasks move to the status named. Can't be undone. |
| `glyphkeel status edit` | `status.update` | Rename or recolour a status, change its category or its WIP limit. |
| `glyphkeel status move` | `status.reorder` | Move a status in the project's board order. |
| `glyphkeel task check <key>` | `checklist.update` | Tick, clear, add or remove checklist items in a task's description or a comment, by number plus expected text. |
| `glyphkeel task claim <key>` | `task.claim` | Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress. |
| `glyphkeel task create` | `task.create` | Create a task from a title alone or with its fields; it gets the project's next key. |
| `glyphkeel task duplicate <key>` | `task.duplicate` | Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone. |
| `glyphkeel task edit <key>` | `task.update` | Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress. |
| `glyphkeel task link <key>` | `relation.add` | Link two tasks (blocks, relates, duplicates, clones, or make one a subtask of the other). |
| `glyphkeel task move <key>` | `task.move` | Move a task to another status or place in its column, or to another project (new key; the old one forwards). |
| `glyphkeel task release <key>` | `task.release` | Give up a claim on a task, optionally moving it and adding a comment. |
| `glyphkeel task reorder-subtask <key>` | `task.reorder_subtask` | Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order. |
| `glyphkeel task resolve <key>` | `task.resolve_conflict` | Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text. |
| `glyphkeel task show <keys>` | `task.get` | One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request. |
| `glyphkeel task unlink <key>` | `relation.remove` | Remove a link between two tasks, or a subtask from its parent. |
| `glyphkeel time delete <entry>` | `time.delete` | Move a time entry to the trash. |
| `glyphkeel time edit <entry>` | `time.update` | Change a time entry's duration, day or note. |
| `glyphkeel time log <key>` | `time.log` | Log time on a task: a duration, the day of the work (default today) and a note. |
| `glyphkeel time report` | `time.report` | Logged time over a range of days, grouped by day, task or project, with totals in the workspace's units. |
| `glyphkeel timer pause` | `timer.pause` | Pause the running timer; it keeps its time. |
| `glyphkeel timer start <key>` | `timer.start` | Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged. |
| `glyphkeel timer status` | `timer.get` | The workspace's timer: its task, whether it runs and the time so far. |
| `glyphkeel timer stop` | `timer.stop` | Stop the timer and log its time as an entry on its task. |
| `glyphkeel type add` | `type.create` | Add a task type to a project. |
| `glyphkeel type delete` | `type.delete` | Delete a task type; its tasks get another type. Can't be undone. |
| `glyphkeel type edit` | `type.update` | Rename, reshape, recolour or reorder a task type, or make it the default. |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error or invalid input (`invalid`) |
| 2 | Not found (`not_found`) |
| 3 | Change rejected (`conflict`, `forbidden`, `limit`) |
| 4 | Workspace or storage problem (`busy`, `workspace_upgraded`, `workspace_too_new`, `storage`) |
