<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Preferences

Open Preferences from the sidebar, the **Main menu**, or with Ctrl+,. Changes
apply at once.

Some preferences are yours on this computer: the appearance, the language and
the time zone. The time zone is shared by the app, the command line and agents.
Others belong to the workspace, such as the working hours, the trash and the
limits on agents, and are kept in the workspace and its backups.

## Appearance

- **Theme:** **System** (Light or Dark, following the desktop), **Light**,
  **Dark**, **Slate** (cool blue-grey, dark), **Sand** (warm paper, light) or
  **Contrast** (strong borders, darkest text).
- **Accent colour:** **From the desktop**, or Blue, Teal, Green, Amber, Orange,
  Red, Pink, Purple or Slate. Used for selection, focus and the main action.
- **Density:** **Regular** or **Compact**, which fits more rows and cards on
  screen.
- **Reduce motion:** follows the desktop setting when off here.
- **Safe rendering:** draws without the graphics card. Turn it on if windows
  stay blank, white or flicker. It applies from the next start; **Restart now**
  restarts Glyphkeel.

## Language

- **Language of the interface:** **System**, or a language Glyphkeel has text
  for. See [Languages](languages.md).
- **Dates and numbers** are written as your system's locale writes them.

## Time

- **First day of the week:** for calendars, the time report and `this-week` in
  queries.
- **Working hours per day** and **Working days per week:** how long `1d` and
  `1w` are in estimates and logged time. The defaults are 8 hours and 5 days.
- **Kinds of work:** the kinds that time entries can be marked with
  ([Time tracking](time.md#kinds-of-work)).
- **Remind about a running timer after:** a number of hours; 0 never reminds.
- **Time zone:** all times show in this zone, and reminders and "today" follow
  it. **System** follows the computer.

## Notifications

- **Reminders for due dates at:** the time of day a reminder on a due date comes,
  unless it has its own. The default is 9:00.
- **Show reminders as desktop notifications:** when off, reminders are kept in
  the app only.
- **Keep running in the background:** closing the last window keeps Glyphkeel
  running for reminders and the timer, with an icon in the tray.
- **Start when you log in** (on Windows: **Start when you sign in**): with
  background mode on, Glyphkeel starts with only its tray icon; otherwise its
  window opens.

## Quick add

**Shortcut for quick add:** the keys that open the quick-add window from any
program ([Quick add](search.md#quick-add)). Preferences says whether the desktop
accepted them.

## Keyboard shortcuts

Every shortcut, with a search field. Changing the keys comes in a later version.
See [Keyboard shortcuts](keyboard-shortcuts.md).

## Agents & CLI

- **Command-line tool:** whether a terminal finds `glyphkeel`. **Install
  command-line tool** adds a link to the copy that came with the app.
- **Connect an agent:** the set-up for common MCP clients, ready to copy, and
  **Only one project** for a repository.
- **What agents may do:** **Most an agent may do** (Read only, Standard or Full)
  and the **Limits on each agent session**.
- **Files agents may use:** the workspace's exports folder, which agents may
  always use, and **Add folder…** for more folders whose files agents may
  attach, import or export to; **Remove** takes one away. The folders are
  yours, for every agent on this computer
  ([Files on disk](../mcp.md#files-on-disk)).
- **Agent sessions:** each session with what it read and changed, and **Revert
  session…**.

See [Working with AI agents](../mcp.md) and
[Connecting MCP clients](../mcp-clients.md).

## Workspace

The workspace's **Name** and **Location**, the **Program version** and the
**Database version**, and **Trash**: **Keep items in the trash for** a number of
days (30 by default).

## Security

- **Encryption:** whether the workspace is encrypted, and **Encrypt…**,
  **Change passphrase…** or **Decrypt…**.
- **App lock:** **Lock when idle** (never, or after one minute to an hour),
  the **Lock passphrase** for a workspace that isn't encrypted, and **Lock
  now**.

See [Encryption and the app lock](encryption-and-lock.md).

## Data and backups

- **Backups:** the **Backup folder**, the **Last backup**, **Back up now**, and
  the **Backups kept**, each with **Check**, **Restore…** and **Delete…**
  ([Backup and restore](../backup-and-restore.md)).
- **Export and import:** archives and CSV files
  ([Export and import](export-import.md)).

## Where preferences are kept

[Data and settings](../data-and-settings.md) lists the files.
