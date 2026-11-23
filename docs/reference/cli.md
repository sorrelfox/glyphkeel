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
| `glyphkeel revert` | `history.revert` | Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed. |
| `glyphkeel settings get` | `settings.get` | Read the workspace settings. |
| `glyphkeel settings set` | `settings.update` | Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week. |
| `glyphkeel status` | `workspace.get` | The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings and a short guide. |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error or invalid input (`invalid`) |
| 2 | Not found (`not_found`) |
| 3 | Change rejected (`conflict`, `forbidden`, `limit`) |
| 4 | Workspace or storage problem (`busy`, `workspace_upgraded`, `workspace_too_new`, `storage`) |
