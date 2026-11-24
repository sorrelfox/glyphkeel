# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The desktop app's own text, shown by Rust: the native confirmation
# dialogs, the file choosers, the tray menu and the autostart entry, in
# English. One message per line; a plural continues on indented lines;
# placeables are { $name }.

## Confirmations of operations that can't be undone

confirm-title = This can't be undone
confirm-cancel = _Cancel
confirm-continue = _Continue
confirm-delete = _Delete for good
confirm-restore = _Restore
confirm-purge-all = Empty the whole trash? Everything in it is deleted for good, with its text in the history.
confirm-purge-items = { $count ->
    [one] Delete one item in the trash for good? Its text is removed from the history too.
   *[other] Delete { $count } items in the trash for good? Their text is removed from the history too.
    }
confirm-backup-delete = Delete the backup “{ $name }” for good? The stored files only it used are deleted with it.
confirm-backup-restore = Replace the whole workspace with the backup “{ $name }”? The current state is backed up first, and the workspace reopens afterwards.
confirm-other = { $summary }
confirm-agent-raise-title = Let agents do more?
confirm-agent-raise = _Allow
confirm-agent-raise-standard = Let agents change tasks and move items to the trash? Every agent connected through MCP may then do this, unless it asks for less.
confirm-agent-raise-full = Let agents do everything, including deleting for good? Every agent connected through MCP may then empty the trash and delete projects, unless it asks for less.
confirm-agent-raise-refused = Agents may do no more than before: the change was not confirmed.
confirm-agent-limits-raise = Raise the limits on agents? Every agent connected through MCP may then make more changes, create more tasks or move more items to the trash than before.
confirm-agent-limits-refused = The limits on agents stay as they were: the change was not confirmed.

## The command-line tool

cli-install-nothing = There is no command-line tool to install: the shell finds it already, or it didn't come with the app.
cli-install-exists = { $path } already exists and is not a link to this app's command-line tool. Remove it first, or keep using it.
cli-install-failed = The link { $path } could not be made: { $reason }

## File choosers

chooser-cancel = _Cancel
chooser-attach-title = Attach files
chooser-attach = _Attach
chooser-save-title = Save a copy
chooser-save = _Save
chooser-export-title = Export to an archive
chooser-export-folder-title = Export to a new folder
chooser-export = _Export
chooser-import-title = Import an archive
chooser-import-folder-title = Import an archive folder
chooser-import = _Choose
chooser-zip-filter = Archives (.zip)
chooser-backup-folder-title = Choose a folder for backups
chooser-backup-folder = _Choose

## The tray icon of background mode

tray-tooltip = Glyphkeel
tray-open = Open Glyphkeel
tray-quick-add = Quick Add…
tray-quit = Quit

## The quick-add shortcut, as the desktop's shortcut settings name it

shortcut-description = Quick add a task

## Starting with the system

autostart-comment = Keeps Glyphkeel running in the background for reminders.
autostart-reason = Start Glyphkeel when you log in, so that reminders come on time.
autostart-no-home = There is no home folder to keep the autostart entry in.
autostart-failed = The autostart entry { $path } could not be changed: { $reason }
autostart-portal-failed = The desktop did not let Glyphkeel start with the system: { $reason }
