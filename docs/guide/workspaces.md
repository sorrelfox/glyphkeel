<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Workspaces

A workspace keeps its own projects, tasks, wiki and files, apart from the
others: for example one for work and one for home. Each workspace is a folder
with its database and its stored files. The first one is called *Personal*.

The app, the command line and agents always work in the same workspace: the one
last opened, unless you choose another.

## In the app

The status bar at the bottom shows the open workspace. Click it to:

- switch to another workspace;
- **New workspace…**: make one and open it in place of this one;
- **Rename this workspace…**. Agents and commands working in the workspace must
  stop first.

Preferences › **Workspace** shows its name, its folder, the program version and
the database version.

## On the command line

```sh
glyphkeel workspace list              # the one commands work in is marked
glyphkeel workspace create Work
glyphkeel workspace open Work         # commands, agents and the app's next start use it
glyphkeel workspace rename Work Office
glyphkeel --workspace Home task list  # one command in another workspace
```

`--workspace` takes a name in the data folder, or the path of a workspace
folder anywhere on the disk. The environment variable `GLYPHKEEL_WORKSPACE`
does the same for every command and for `glyphkeel mcp`. An agent can be given
its own workspace with `glyphkeel mcp --workspace NAME`.

A name has letters, digits, spaces, `-` and `_`, at most 64 characters.

## Where workspaces are kept

Workspaces are in `~/.local/share/glyphkeel/workspaces/` on Linux and in
`%LOCALAPPDATA%\Glyphkeel\workspaces\` on Windows.
[Data and settings](../data-and-settings.md) has every location and explains how
to move them.

Keep a workspace on a local disk. Several Glyphkeel programs may use it at once,
which network and cloud-synced folders don't handle reliably. To move data
between computers, use [backups](../backup-and-restore.md) or
[exports](export-import.md).
