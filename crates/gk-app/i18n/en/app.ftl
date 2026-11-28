# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The desktop app's own text, shown by Rust: the native confirmation
# dialogs, the file choosers, the tray menu, the autostart entry, the
# reminders' notifications and the errors of the app's own commands, in
# English. The page loads this file too, so that it shows these errors in
# the person's language. One message per line; a plural continues on indented lines;
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
confirm-agent-confirmations-on = Let agents ask you to allow a change their profile doesn't allow, or one that deletes a lot? Each time, their MCP client shows you the question, and only what you allow there runs.
confirm-agent-http-on = Let agents connect to the app over local HTTP? Any program on this computer that has the token may then work in the open workspace as an agent.
confirm-agent-http-refused = The HTTP endpoint for agents stays off: the change was not confirmed.
confirm-agent-limits-refused = The limits on agents stay as they were: the change was not confirmed.

## Notifications of reminders

notification-open = Open
# $key is a task key, such as TM-41.
notification-reminder = Reminder: { $key }
# $day is the due date as the task has it, such as 2026-11-24.
notification-due = { $key } is due { $day }
notification-due-soon = { $key } is due soon
notification-timer = The timer is still running on { $key }
# A notification an automation rule left about a task; its text is the body.
notification-automation = Automation: { $key }
# A reminder that came due while the app wasn't running. $what is one of the
# notification titles above.
notification-missed = Missed: { $what }
# The text under a notification: the task's title, then the reminder's own note.
notification-body-with-note = { $title }
    { $note }
notification-missed-many = { $count ->
    [one] One missed reminder
   *[other] { $count } missed reminders
    }
# $keys is a list of task keys; $more is how many more are left out (0 when none).
notification-missed-many-body = { $more ->
    [0] They came due while Glyphkeel was closed: { $keys }
   *[other] They came due while Glyphkeel was closed: { $keys }, …
    }

## The notification of the digest at the start

# Shown once when the app starts, if agents changed something since the
# person last dismissed the digest on Today.
notification-digest = { $sessions ->
    [one] An agent changed things while you were away
   *[other] { $sessions } agents changed things while you were away
    }
# $changes counts the items agents changed; the title names the sessions.
notification-digest-body = { $changes ->
    [one] One change. Today shows what it was.
   *[other] { $changes } changes. Today shows what they were.
    }

## The command-line tool

cli-install-nothing = There is no command-line tool to install: the shell finds it already, or it didn't come with the app.
cli-install-exists = { $path } already exists and is not a link to this app's command-line tool. Remove it first, or keep using it.
cli-install-failed = The link { $path } could not be made: { $reason }

## Preferences › About

# $file is a file name, such as THIRD-PARTY-NOTICES.txt.
legal-text-missing = { $file } did not come with this copy of the app. It is also in the source code of Glyphkeel.

## File choosers

chooser-cancel = _Cancel
# The page asked for an operation with a file or folder it named itself; only the system's chooser may name one.
page-path-refused = { $op } takes its { $field } from the system's chooser, not from the page.
chooser-attach-title = Attach files
chooser-attach = _Attach
chooser-save-title = Save a copy
chooser-save = _Save
chooser-export-title = Export to an archive
chooser-export-folder-title = Export to a new folder
chooser-export = _Export
chooser-import-title = Import an archive
chooser-import-folder-title = Import an archive folder
chooser-csv-title = Import tasks from a CSV file
csv-input-not-an-object = The import's settings are not an object.
chooser-import = _Choose
chooser-zip-filter = Archives (.zip)
chooser-backup-folder-title = Choose a folder for backups
chooser-backup-folder = _Choose
chooser-wiki-folder-title = Import a folder of Markdown files into the wiki
chooser-agent-folder-title = Choose a folder agents may use
agent-folders-not-stored = Can't store the folders agents may use in { $file }: { $error }.

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
autostart-run-key-failed = Glyphkeel could not change its place among the apps that start when you sign in: { $reason }

## Errors of the app's own commands. $error is the system's own words.

preference-bad-name = "{ $name }" is not a preference name.
preference-too-large = The preference "{ $name }" is too large.
preferences-too-many = There are too many preferences.
time-zone-not-stored = Can't store the time zone in { $file }: { $error }.
workspace-closed = The workspace is closed.
workspace-closed-quitting-hint = The app is quitting.
workspace-no-data-folder = The data folder could not be found.
workspace-no-data-folder-hint = Set XDG_DATA_HOME or HOME.
workspace-no-data-folder-env-hint = Set XDG_DATA_HOME or HOME, or name a folder in GLYPHKEEL_WORKSPACE.
operation-stopped = The operation stopped unexpectedly: { $error }.
command-stopped = The command stopped unexpectedly: { $error }.
app-quitting = The app is quitting.
file-details-unreadable = The file's details can't be read: { $error }.
file-content-missing = The stored content of { $name } is missing.
file-content-missing-hint = Restore the workspace from a backup.
file-thumbnail-unreadable = The thumbnail can't be read: { $error }.
file-attach-owner = Name either the task or the wiki page to attach the files to.
file-path-not-text = The path of { $name } can't be read as text.
file-drop-unknown = Those dropped files are no longer known.
file-drop-unknown-hint = Drop them again.
file-runs-programs = { $name } can run programs, so it isn't opened.
file-runs-programs-hint = Save it, and look at it first.
file-copy-failed = { $name } can't be copied to open it: { $error }.
file-save-failed = { $name } can't be saved there: { $error }.
file-open-name-failed = The file can't be named for opening: { $error }.
file-open-failed = The file can't be opened: { $error }.
file-no-app = No app opened the file: { $error }.
file-no-app-hint = Save it and open it yourself.
file-open-unsupported = Files can't be opened on this platform yet.
export-too-large = The export is too large to save.
export-unreadable = The export can't be read.
chooser-failed = The file chooser can't be shown: { $error }.
chooser-unsupported = Files can't be chosen on this platform yet.
archive-unknown = The chosen archive is no longer known.
archive-unknown-hint = Choose the archive again.
link-too-long = The address is too long to open.
link-not-an-address = That isn't an address: { $error }.
link-not-external = Only web (http, https) and mail addresses are opened outside the app.
link-open-failed = The link can't be opened: { $error }.
link-no-program = No program opened the link: { $error }.
link-unsupported = Links can't be opened on this platform yet.
links-main-window-only = Only the main window opens links.
quick-add-hide-only = Only the quick-add window hides itself.
quick-add-listen-only = Only the quick-add window listens here.
task-window-bad-key = { $key } isn't a task key.
task-window-failed = The task window can't be opened: { $error }.
wiki-window-bad-id = { $id } isn't a page ID.
wiki-window-failed = The wiki window can't be opened: { $error }.
shortcut-not-a-choice = "{ $keys }" is not one of the shortcuts to choose from.

## The agents' HTTP endpoint

agent-http-no-settings-file = The HTTP endpoint's settings can't be stored: there is no home folder for the personal settings.
agent-http-not-stored = The HTTP endpoint's settings couldn't be stored in { $file }: { $error }
agent-http-port-range = Port { $port } can't be used: choose one from 1024 to 65535, or let the app choose a free one.

## The app lock and the workspace's encryption

app-locked = Glyphkeel is locked.
lock-notice-title = Glyphkeel reminder
lock-notice-body = A reminder is due. Unlock Glyphkeel to see it.
lock-passphrase-empty = Enter the passphrase.
lock-passphrase-wrong = That passphrase is not right.
lock-not-possible = There is no passphrase to unlock with.
lock-not-possible-hint = Encrypt the workspace, or set a passphrase for the lock in Preferences, Security.
