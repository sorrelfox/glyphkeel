<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Backup and restore

A backup is a checked copy of a workspace: its database and its stored files.
Glyphkeel makes one every day by itself, and you can make one at any time.

## What Glyphkeel does by itself

- **Every day** the workspace is used, the first Glyphkeel program to open it
  (the app, a command or an agent) makes a **Daily** backup.
- **Before an upgrade** of the database to a new version, it makes a backup of
  that kind.
- **Before a restore**, it backs up the current state, so a restore can be taken
  back.
- Before a large change by an agent, it makes a **Safety copy**.

Each backup is written under a temporary name, checked with SQLite's integrity
check, and only then kept. Stored files are copied once and shared by all
backups, so a new backup takes little room.

### What is kept

| Kind | Kept |
|---|---|
| Daily | The last 7 |
| Weekly | The last 4. A daily backup that ages out becomes its week's weekly one |
| Before an upgrade, Before a restore, Safety copy | The last 3 of each |
| Made by hand | All, until you delete them |

Stored files that no backup needs any more are removed when old backups go.

## Where backups are kept

| Platform | Folder |
|---|---|
| Linux | `~/.local/share/glyphkeel/backups/<workspace>/` |
| Linux, Flatpak | `~/.var/app/io.github.sorrelfox.Glyphkeel/data/glyphkeel/backups/<workspace>/` |
| Windows | `%LOCALAPPDATA%\Glyphkeel\backups\<workspace>\` |

To keep backups elsewhere, for example on another disk, choose Preferences ›
Data and backups › **Backup folder** › **Change…**. New backups go there; the
ones made before stay where they are. **Use default** goes back.

Backups on the same disk protect against mistakes, not against a broken disk.
Copy the backup folder to another disk or computer from time to time, or set the
backup folder to one.

## Making a backup

- In the app: **Back up now** in the **Main menu**, in the command palette, or in
  Preferences › Data and backups.
- On the command line: `glyphkeel backup create`.

## Checking a backup

In Preferences › Data and backups, **Check** next to a backup reads the
database copy and every stored file it needs, and reports any problem. On the
command line:

```sh
glyphkeel backup list
glyphkeel backup verify 2026-11-26T091200Z
```

## Restoring

A restore replaces the **whole workspace** with the backup. To get back only one
deleted task or page, look in the [trash](guide/history-and-trash.md#the-trash)
first; to undo a change, use [the history](guide/history-and-trash.md).

1. Quit the MCP clients and close the terminals that use the workspace. Other
   programs that have it open must restart after the restore.
2. In Preferences › Data and backups, choose **Restore…** next to the backup,
   and confirm.
3. Glyphkeel first backs up the current state, as **Before a restore**, then
   restores and reopens the workspace.

On the command line:

```sh
glyphkeel backup restore 2026-11-26T091200Z
```

It asks before it replaces anything; `--yes` confirms in advance, for scripts.

If the restore was a mistake, restore the **Before a restore** backup it made.

## Deleting a backup

**Delete…** next to a backup, or `glyphkeel backup delete <name>`, deletes it
for good, with the stored files that only it used.

## Moving to another computer

There are two ways:

- **An export** moves projects. Export the whole workspace to an archive and
  import it on the other computer, where its projects are added to the
  workspace there ([Export and import](guide/export-import.md)).
- **A backup** moves the workspace exactly as it was. On the new computer,
  install Glyphkeel and start it once, so that it makes the workspace. Quit it,
  copy the old computer's backup folder of the workspace over the new one (this
  replaces the new computer's list of backups), and restore the backup you want.

Don't copy a workspace folder while a Glyphkeel program has it open, and don't
keep a workspace in a cloud-synced folder; see
[Data and settings](data-and-settings.md).

## Before uninstalling

Removing Glyphkeel never deletes your workspaces or backups. If you mean to
delete the data folder too, make an export first if you might want your tasks
again.
