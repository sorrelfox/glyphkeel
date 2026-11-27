<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Data and settings

Glyphkeel keeps two kinds of files apart:

- the **data folder**: your workspaces (each one a folder with its database
  and attached files) and their daily backups;
- the **configuration folder**: your personal settings, shared by the app,
  the command line and the MCP server, and the app's own settings.

The app, `glyphkeel` and `glyphkeel mcp` all use the same folders, so they
always see the same workspaces.

## Where they are

| What | Linux | Windows |
|---|---|---|
| Data folder | `$XDG_DATA_HOME/glyphkeel`, normally `~/.local/share/glyphkeel` | `%LOCALAPPDATA%\Glyphkeel` |
| Workspaces | `<data folder>/workspaces/<name>/` | `<data folder>\workspaces\<name>\` |
| Backups | `<data folder>/backups/<name>/` | `<data folder>\backups\<name>\` |
| Configuration folder | `$XDG_CONFIG_HOME/glyphkeel`, normally `~/.config/glyphkeel` | `%APPDATA%\Glyphkeel` |
| Personal settings (time zone, the folders agents may use, the agents' HTTP endpoint and its token, and the like; readable by you only) | `<configuration folder>/settings.json` | `<configuration folder>\settings.json` |
| The app's settings (background mode, notifications, safe rendering, the app lock) | `<configuration folder>/app.json` | `<configuration folder>\app.json` |
| Window size and position | `$XDG_STATE_HOME/glyphkeel/main-window.json`, normally under `~/.local/state` | `<configuration folder>\main-window.json` |
| Starting when you sign in | `~/.config/autostart/io.github.sorrelfox.Glyphkeel.desktop` | the value `Glyphkeel` under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` |

The default workspace is called *Personal* and is made on the first start.
Preferences › Workspace shows the folder of the open workspace.

The Flatpak keeps the same layout inside its own folder,
`~/.var/app/io.github.sorrelfox.Glyphkeel/`. On Windows, the web view keeps
its cache in `%LOCALAPPDATA%\io.github.sorrelfox.Glyphkeel\`; nothing of
yours is stored there.

## Moving them

Four environment variables change where Glyphkeel looks and what it shows. They work the same
on Linux and Windows, for the app, the command line and the MCP server (`GLYPHKEEL_LANG`
only for the command line):

| Variable | What it does |
|---|---|
| `GLYPHKEEL_DATA_DIR` | The data folder (workspaces and backups) |
| `GLYPHKEEL_CONFIG_DIR` | The configuration folder (settings). On every platform the window's size and position then go there too |
| `GLYPHKEEL_WORKSPACE` | The workspace to open: a name in the data folder, or the path of a workspace folder |
| `GLYPHKEEL_LANG` | The language of the command line, such as `de`; it comes before the one chosen in the app's Preferences |

One more gives the passphrase of an encrypted workspace to programs that
can't ask for it, such as agent servers and scripts:
`GLYPHKEEL_PASSPHRASE` ([Encryption and the app lock](guide/encryption-and-lock.md),
with its trade-offs in [Security](security.md#encrypted-workspaces)).

`GLYPHKEEL_DATA_DIR` and `GLYPHKEEL_CONFIG_DIR` take an absolute path; a
relative or empty value is ignored. Set them for every program that should
share the folders, for example for a portable set-up on a USB drive, or to
keep test data away from your own:

```sh
# Linux
export GLYPHKEEL_DATA_DIR="$HOME/sync/glyphkeel-data"
export GLYPHKEEL_CONFIG_DIR="$HOME/sync/glyphkeel-config"
```

```powershell
# Windows, for your account (new programs see it after you sign in again)
[Environment]::SetEnvironmentVariable("GLYPHKEEL_DATA_DIR", "D:\Glyphkeel\data", "User")
[Environment]::SetEnvironmentVariable("GLYPHKEEL_CONFIG_DIR", "D:\Glyphkeel\config", "User")
```

An MCP client passes its own environment to `glyphkeel mcp`; if you moved the
folders, give the client the same variables in its server settings.

Keep a workspace on a local disk. Several Glyphkeel programs may use it at
once, which network and cloud-synced folders don't handle reliably; use
[backups](backup-and-restore.md#moving-to-another-computer) or
[exports](guide/export-import.md) to move data between computers.

## Uninstalling keeps your data

Removing the program, through the package manager, the Flatpak or the
Windows uninstaller, never deletes the data or configuration folder. Delete
them yourself if you want everything gone, after making a backup or an export
you might still need.
