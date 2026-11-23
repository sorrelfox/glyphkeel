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
| `workspace.get` | read | app, command line, MCP | `get_workspace` | `glyphkeel status` |
| `settings.get` | read | app, command line, MCP | `get_workspace` | `glyphkeel settings get` |
| `settings.update` | write | app, command line, MCP | `update_settings` | `glyphkeel settings set` |
| `project.list` | read | app, command line, MCP | `list_projects` | `glyphkeel project list` |
| `project.get` | read | app, command line, MCP | `get_project` | `glyphkeel project show <project>` |
| `project.create` | write | app, command line, MCP | `create_project` | `glyphkeel project create` |
| `project.update` | write | app, command line, MCP | `update_project` | `glyphkeel project edit <project>` |
| `project.move` | write | app, command line, MCP | `update_project` | `glyphkeel project move <project>` |
| `project.change_key` | write | app, command line, MCP | `update_project` | `glyphkeel project rekey <project>` |
| `project.archive` | write | app, command line, MCP | `update_project` | `glyphkeel project archive <project>` |
| `project.unarchive` | write | app, command line, MCP | `update_project` | `glyphkeel project unarchive <project>` |
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
| `task.get` | read | app, command line, MCP | `get_task` | `glyphkeel task show <keys>` |
| `task.create` | write | app, command line, MCP | `create_task` | `glyphkeel task create` |
| `task.update` | write | app, command line, MCP | `update_task` | `glyphkeel task edit <key>` |
| `task.move` | write | app, command line, MCP | `move_task` | `glyphkeel task move <key>` |
| `board.get` | read | app, command line, MCP | `get_board` | `glyphkeel board <project>` |
| `layout.get` | read | app, command line, MCP | `get_board` | `glyphkeel board layout show <project>` |
| `layout.update` | write | app, command line, MCP | `set_board_layout` | `glyphkeel board layout set <project>` |
| `agent.sessions` | read | app, command line, MCP | `get_activity` | `glyphkeel agents sessions` |
| `agent.policy.update` | app | app, command line | — | `glyphkeel agents policy` |
| `history.feed` | read | app, command line, MCP | `get_activity` | `glyphkeel activity` |
| `history.revert` | write | app, command line, MCP | `revert_changes` | `glyphkeel revert` |
| `history.undo` | app | app | — | — |
| `history.redo` | app | app | — | — |

## `workspace.get`

The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide.

Input: `WorkspaceGetInput`. Output: `WorkspaceInfo`. Schemas: [catalog.schema.json](catalog.schema.json).

## `settings.get`

Read the workspace settings.

Input: `SettingsGetInput`. Output: `Settings`. Schemas: [catalog.schema.json](catalog.schema.json).

## `settings.update`

Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week.

Input: `SettingsPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.list`

The project tree in sidebar order, with keys, names and open-task counts; archived projects on request.

Input: `ProjectListInput`. Output: `ProjectList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.get`

One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts.

Input: `ProjectRef`. Output: `Project`. Schemas: [catalog.schema.json](catalog.schema.json).

## `project.create`

Create a project, or a sub-project that copies its parent's statuses, types and labels.

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

Add a label to a project.

Input: `LabelCreateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `label.update`

Rename or recolour a label.

Input: `LabelUpdateInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `label.delete`

Delete a label and take it off every task. Can't be undone.

Input: `LabelDeleteInput`. Output: `ProjectChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.get`

One to 20 tasks by key, with their fields, description, labels and version.

Input: `TaskGetInput`. Output: `TaskList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.create`

Create a task from a title alone or with its fields; it gets the project's next key.

Input: `TaskCreateInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.update`

Change a task's title, description, type, priority, labels, parent, dates, estimate or progress.

Input: `TaskUpdateInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `task.move`

Move a task to another status or place in its column, or to another project (new key; the old one forwards).

Input: `TaskMoveInput`. Output: `TaskChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `board.get`

A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards.

Input: `BoardGetInput`. Output: `Board`. Schemas: [catalog.schema.json](catalog.schema.json).

## `layout.get`

The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields.

Input: `LayoutGetInput`. Output: `LayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `layout.update`

Hide, show, collapse or reorder board columns, set sort modes, card fields and older closed cards, or reset to the project's layout.

Input: `LayoutUpdateInput`. Output: `LayoutChange`. Schemas: [catalog.schema.json](catalog.schema.json).

## `agent.sessions`

List agent sessions, newest first, with their client, profile, times and counts of reads and writes.

Input: `AgentSessionsInput`. Output: `AgentSessionList`. Schemas: [catalog.schema.json](catalog.schema.json).

## `agent.policy.update`

Set the most an agent may do, whatever profile it asks for.

Input: `AgentPolicyPatch`. Output: `SettingsChange`. Schemas: [catalog.schema.json](catalog.schema.json).

Not available to agents: It sets the rights of agents; an agent that could call it would raise its own rights beyond what the person allowed.

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
