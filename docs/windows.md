<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Glyphkeel on Windows

Glyphkeel runs on 64-bit Windows 10 (22H2) and Windows 11. It comes as one
installer, `glyphkeel-<version>-x64-setup.exe`, on the
[releases page](https://github.com/sorrelfox/glyphkeel/releases). Windows
builds are new: until a release says otherwise, treat them as pre-releases.

## Installing

The installer works for your account only and needs no administrator rights.
It puts:

- the app, `glyphkeel-app.exe`, and the command line, `glyphkeel.exe`, in
  `%LOCALAPPDATA%\Programs\Glyphkeel`;
- that folder on your user `PATH`, so a new terminal finds `glyphkeel`;
- a Start-menu entry (no desktop shortcut unless you tick the box at the end);
- the handler for `glyphkeel://` links, so a link such as
  `glyphkeel://open/TM-41` opens the task in the app.

It changes nothing for other accounts, and nothing outside your profile
except the Start menu.

### The SmartScreen warning

Glyphkeel's installers are not code-signed yet. Windows therefore shows
**"Windows protected your PC"** when you start the installer, and your
browser may say the download "isn't commonly downloaded". To go on:

1. Check the file first: compare its checksum with the release's
   `SHA256SUMS` file. In PowerShell, with the name of the file you
   downloaded:

   ```powershell
   Get-FileHash .\glyphkeel-0.3.0-x64-setup.exe -Algorithm SHA256
   ```

   The hash must match the line for the installer in `SHA256SUMS`. Each
   release also has a build-provenance attestation, which
   `gh attestation verify <file> --repo sorrelfox/glyphkeel` checks.
2. In the SmartScreen window choose **More info**, then **Run anyway**.

If the hash doesn't match, don't run the file; download it again from the
releases page.

### WebView2

The app draws its window with Microsoft Edge WebView2, which Windows 11 and
up-to-date Windows 10 already include. If it is missing, the installer
installs it, which needs an internet connection once. Without one, install
the "Evergreen Standalone Installer" of the WebView2 Runtime from Microsoft
first, then run Glyphkeel's installer again.

WebView2 is part of Windows, not of Glyphkeel, and Windows keeps it up to
date. Glyphkeel itself never goes online.

## Using it

- **Window:** Glyphkeel uses the Windows title bar, with its own buttons and
  Snap layouts.
- **Spelling:** the editor checks spelling with WebView2's own spell checker,
  in the languages you have added in Windows Settings › Time & language ›
  Language & region. Right-click a marked word for suggestions.
- **Shortcuts:** shortcut hints say Ctrl, Alt and Shift; the Windows logo key
  is shown as Win.
- **Starting with Windows:** Preferences › Notifications › *Start when you
  sign in*. It adds Glyphkeel to the apps Windows starts when you sign in;
  with background mode on it starts with only its icon in the notification
  area. Settings › Apps › Startup and Task Manager can switch it off too, and
  Preferences shows that.
- **Quick add:** the shortcut in Preferences › Quick add works from any
  program while Glyphkeel runs. If another program already holds the keys,
  Preferences says so; choose other keys.
- **File names in exports:** exports, saved files and opened attachments use
  names that Windows accepts, the same on every system: characters such as
  `:` and `?` become `_`, trailing dots and spaces go, and a name Windows
  keeps for a device gets a `_` in front (a wiki page called *CON* is saved
  as `_con.md`). Importing such a folder of Markdown files takes the `_` off
  again.
- **If the window stays blank or flickers:** start it once with
  `glyphkeel-app.exe --safe-graphics`, or turn on Preferences › Appearance ›
  Safe rendering, which draws without the graphics card.

## The command line and agents

Open a **new** terminal after installing (Command Prompt, PowerShell or
Windows Terminal), so it sees the new `PATH`:

```powershell
glyphkeel --version
glyphkeel task list
glyphkeel completions powershell | Out-String | Invoke-Expression
```

Preferences › Agents & CLI says whether a terminal finds `glyphkeel`. If the
installer couldn't add its folder to your `PATH` (it leaves a very long
`PATH` alone rather than cut it), add `%LOCALAPPDATA%\Programs\Glyphkeel` to
your user `PATH` in Windows' environment variables.

MCP clients start the server as `glyphkeel.exe mcp`. Preferences › Agents &
CLI has the set-up for common clients, with the full path filled in; see
[MCP clients](mcp-clients.md) for the details.

## Where your data is

| What | Where |
|---|---|
| Workspaces | `%LOCALAPPDATA%\Glyphkeel\workspaces\` |
| Backups | `%LOCALAPPDATA%\Glyphkeel\backups\` |
| Settings | `%APPDATA%\Glyphkeel\` |
| The web view's cache | `%LOCALAPPDATA%\io.github.sorrelfox.Glyphkeel\` |

The program folder (`%LOCALAPPDATA%\Programs\Glyphkeel`) holds no data. All
locations, and how to move them, are in
[Data and settings](data-and-settings.md).

## Updating

Run the new version's installer. It replaces the program and keeps your data
and settings. Quit Glyphkeel first, and close MCP clients that are running
`glyphkeel.exe mcp`, so that no program file is in use.

## Uninstalling

Settings › Apps › Installed apps › Glyphkeel › Uninstall. This removes the
program, the Start-menu entry, the `PATH` entry, the `glyphkeel://` handler
and the entry that starts Glyphkeel when you sign in.

**Your workspaces, backups and settings stay.** The uninstaller's option to
delete the application data removes only the web view's cache. To remove
everything, delete these two folders yourself after uninstalling:

```powershell
Remove-Item -Recurse "$env:LOCALAPPDATA\Glyphkeel"   # workspaces and backups
Remove-Item -Recurse "$env:APPDATA\Glyphkeel"        # settings
```

Make a backup or an export first if you might want your tasks again.
