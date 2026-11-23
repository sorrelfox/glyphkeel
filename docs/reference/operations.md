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
