<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Getting started

Glyphkeel runs on 64-bit Linux and on Windows 10 and 11. Download it from the
[releases page](https://github.com/sorrelfox/glyphkeel/releases).

## Installing on Linux

Each release has a deb package, an rpm package and a Flatpak. The deb and rpm
packages install both programs, the app `glyphkeel-app` and the command line
`glyphkeel`, with shell completions and a man page.

**Debian and Ubuntu:**

```sh
sudo apt install ./glyphkeel_<version>_amd64.deb
```

**Fedora:**

```sh
sudo dnf install ./glyphkeel-<version>-1.x86_64.rpm
```

**Flatpak:** install it from the project's Flatpak repository, which also
brings updates through `flatpak update`:

```sh
flatpak install --user https://sorrelfox.github.io/glyphkeel/flatpak/io.github.sorrelfox.Glyphkeel.flatpakref
```

The `.flatpak` file on the releases page installs without the repository
(`flatpak install --user ./glyphkeel-<version>-x86_64.flatpak`), but it doesn't
update itself. In the Flatpak, the command line runs as
`flatpak run --command=glyphkeel io.github.sorrelfox.Glyphkeel`.

Check a download against the release's `SHA256SUMS` file before you install it:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

## Installing on Windows

Run `glyphkeel-<version>-x64-setup.exe`. It installs for your account only and
needs no administrator rights. The installers are not code-signed yet, so
Windows shows a SmartScreen warning first.
[Glyphkeel on Windows](../windows.md) explains the warning, WebView2, updating
and uninstalling.

## The first start

The first time Glyphkeel starts, it makes a workspace called *Personal* and
shows **Set up Glyphkeel**, four short steps:

1. **Welcome to Glyphkeel.** Your tasks are kept on this computer, and nothing
   is sent anywhere.
2. **Choose a look.** Pick a theme. Accent colours and more are in
   Preferences › Appearance.
3. **Start a project.** Give your first project a name. Its key, shown before
   task numbers, comes from the name. You can change it later.
4. **Work with agents.** Agents such as Claude Code, Codex or Cursor can read and
   change your tasks over MCP. **Set up agents** opens Preferences › Agents &
   CLI.

**Skip set-up** goes straight to the app. Everything in these steps can be
changed later.

## The window

- **The sidebar** on the left has **Search**, the views **Today**, **All
  tasks**, **Time report**, **Reports** and **Activity**, your **Projects**,
  **Trash** and **Preferences**. F9 shows or hides it.
- **The header bar** has the project's views (**Board**, **List**,
  **Calendar**, **Gantt**, **Roadmap** and **Wiki**), the **New task** button and
  the **Main menu**: Undo, Redo, Quick add, Back up now, Import…, Import tasks
  from a CSV file…, Export…, Keyboard shortcuts and Preferences.
- **The status bar** at the bottom shows the open workspace. Click it to
  switch to another one ([Workspaces](workspaces.md)).
- A running timer shows in the sidebar ([Time tracking](time.md)).

Press Ctrl+K for the command palette, and `?` for the list of keyboard
shortcuts.

## Closing and running in the background

Closing the last window keeps Glyphkeel running, with an icon in the tray, so
that reminders and the timer go on. **Quit** in the tray menu or in the palette
(Ctrl+Q) ends it. To make closing the window quit, turn off Preferences ›
Notifications › **Keep running in the background**. **Start when you log in**
starts Glyphkeel with the computer.

## If the window stays blank

If the window stays blank, white or flickers, turn on Preferences › Appearance
› **Safe rendering**, which draws without the graphics card. If you can't reach
Preferences, start the app once with `glyphkeel-app --safe-graphics`.

## Next steps

- [Projects and sub-projects](projects.md)
- [Tasks](tasks.md)
- [The board](board.md)
- [Working with AI agents](../mcp.md)
