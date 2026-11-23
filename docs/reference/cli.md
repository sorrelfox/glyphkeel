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
| `glyphkeel label add` | `label.create` | Add a label to a project. |
| `glyphkeel label delete` | `label.delete` | Delete a label and take it off every task. Can't be undone. |
| `glyphkeel label edit` | `label.update` | Rename or recolour a label. |
| `glyphkeel project archive <project>` | `project.archive` | Archive a project and its sub-projects: they become read-only and leave the sidebar. |
| `glyphkeel project create` | `project.create` | Create a project, or a sub-project that copies its parent's statuses, types and labels. |
| `glyphkeel project edit <project>` | `project.update` | Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change. |
| `glyphkeel project list` | `project.list` | The project tree in sidebar order, with keys, names and open-task counts; archived projects on request. |
| `glyphkeel project move <project>` | `project.move` | Move a project under another one (at most 3 levels), to the top level, or among its siblings. |
| `glyphkeel project rekey <project>` | `project.change_key` | Give a project a new key; the old key and the old task keys keep working. |
| `glyphkeel project show <project>` | `project.get` | One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts. |
| `glyphkeel project unarchive <project>` | `project.unarchive` | Unarchive a project and its sub-projects. |
| `glyphkeel revert` | `history.revert` | Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed. |
| `glyphkeel settings get` | `settings.get` | Read the workspace settings. |
| `glyphkeel settings set` | `settings.update` | Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week. |
| `glyphkeel status` | `workspace.get` | The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide. |
| `glyphkeel status add` | `status.create` | Add a status (a board column) to a project. |
| `glyphkeel status delete` | `status.delete` | Delete a status; its tasks move to the status named. Can't be undone. |
| `glyphkeel status edit` | `status.update` | Rename or recolour a status, change its category or its WIP limit. |
| `glyphkeel status move` | `status.reorder` | Move a status in the project's board order. |
| `glyphkeel task create` | `task.create` | Create a task from a title alone or with its fields; it gets the project's next key. |
| `glyphkeel task edit <key>` | `task.update` | Change a task's title, description, type, priority, labels, parent, dates, estimate or progress. |
| `glyphkeel task move <key>` | `task.move` | Move a task to another status or place in its column, or to another project (new key; the old one forwards). |
| `glyphkeel task show <keys>` | `task.get` | One to 20 tasks by key, with their fields, description, labels and version. |
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
