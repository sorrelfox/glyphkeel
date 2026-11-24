# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Messages of the engine's search, Today and wiki operations and of dates,
# in English. The engine renders them in English for the command line and
# agents and adds the message's ID and arguments to the error, so the app
# can show its own translation. One message per line; placeables are
# { $name }.

## Lists

list-limit = Ask for 1 to { $max } items at once.
list-cursor = This is not a cursor from an earlier page.

## Search

search-empty = Type something to search for.

## Dates

date-invalid = "{ $text }" is not a date. Write 2026-12-31, or today, fri, next mon, in 3d, +3d.

## Revisions

revision-not-found = There is no revision "{ $revision }" of this text.

## Wiki

wiki-title-empty = A page needs a title.
wiki-title-too-long = The title is { $length } characters long; the limit is { $max }.
wiki-title-reserved = A page title can't contain "{ $char }", because links to pages use it.
wiki-title-taken = Project { $project } already has a page called "{ $title }".
wiki-page-not-found = Project { $project } has no page "{ $page }".
wiki-page-id-not-found = There is no page with the ID { $page }.
wiki-project-needed = Name the project of page "{ $page }", or give the page's ID.
wiki-parent-loop = A page can't be put inside itself or inside a page it holds.
wiki-parent-other-project = The parent page "{ $page }" is in another project.
wiki-too-deep = Pages can be nested at most { $max } deep.
wiki-sibling-not-found = "{ $page }" is not a page next to this one.
wiki-body-and-edits = Give a new body or edits, not both.

## Trash

trash-tasks-count = Name 1 to { $max } tasks.
trash-items-count = Name 1 to { $max } entries of the trash.
trash-item-not-found = There is nothing in the trash with the ID { $item }.
trash-project-needs-full = Deleting a project needs the full profile.
trash-project-count = { $count ->
        [one] { $project } and its sub-projects hold one task, not { $expected }; nothing was deleted.
       *[other] { $project } and its sub-projects hold { $count } tasks, not { $expected }; nothing was deleted.
    }
trash-project-count-hint = Check the project again and repeat with the right count.
trash-call-limit = { $count ->
        [one] This would move one item to the trash; your profile allows { $max } per call.
       *[other] This would move { $count } items to the trash; your profile allows { $max } per call.
    }
trash-call-limit-hint = Delete fewer items per call.
trash-hour-limit = { $count ->
        [one] This session moved one item to the trash in the last hour; your profile allows { $max } an hour.
       *[other] This session moved { $count } items to the trash in the last hour; your profile allows { $max } an hour.
    }
trash-hour-limit-hint = { $seconds ->
        [one] Wait one second before deleting more.
       *[other] Wait { $seconds } seconds before deleting more.
    }
trash-needs-full-hint = Ask the person to do it, or to allow the full profile.
trash-item-not-found-hint = List the trash with list_trash, or `glyphkeel trash list`.
trash-restore-task-first = This can't come back on its own: its task { $container } is in the trash too.
trash-restore-page-first = This page can't come back on its own: the page "{ $container }" that holds it is in the trash too.
trash-restore-project-first = This can't come back on its own: its project { $container } is in the trash too.
trash-restore-owner-first = This file can't come back on its own: what it is attached to is in the trash too.
trash-purge-all-agents = Agents purge only entries they name; the person empties the whole trash.
trash-purge-all-agents-hint = Name the entries to purge in `items`.
trash-purge-all-and-items = Name entries or ask for the whole trash, not both.
trash-restore-first-hint = Restore what holds it first; that brings it back too.

## Backups

backup-storage = The backup could not be made or read: { $error }
backup-check-failed = The new backup failed SQLite's integrity check, so it was not kept.
backup-not-found = There is no backup "{ $name }".
backup-not-found-hint = List the backups with `glyphkeel backup list`.
backup-restore-app-only = { $op } is available in the app only.
backup-restore-app-open = The app has this workspace open, so it can't be restored from the command line.
backup-restore-app-open-hint = Restore from the app, or close it first.
backup-copy-failed = { $path } could not be copied: { $error }
backup-restore-damaged = The backup "{ $name }" is damaged and can't be restored.
backup-restore-missing-files = { $count ->
        [one] The backup "{ $name }" lacks one of its stored files and can't be restored.
       *[other] The backup "{ $name }" lacks { $count } of its stored files and can't be restored.
    }

## Export and import

transfer-rejected-hint = Nothing was imported. Export the data again, or check where the archive came from.
transfer-not-an-archive = This is not a readable archive: { $detail }
transfer-unreadable = The archive could not be read: { $detail }
transfer-encrypted = The archive's entry { $path } is encrypted; archives are never encrypted.
transfer-bad-path = The archive's entry "{ $path }" has an unsafe path.
transfer-link = The archive's entry "{ $path }" is a link or a special file.
transfer-duplicate = The archive holds "{ $path }" twice.
transfer-duplicate-entries = The archive holds two entries with the same name.
transfer-too-many-entries = The archive holds more than { $max } entries.
transfer-too-large = The archive's entries add up to more than { $max } bytes.
transfer-too-many-records = The archive holds more than { $max } records.
transfer-entry-too-large = The archive's entry { $path } is larger than { $max } bytes.
transfer-bomb = The archive's entry { $path } expands far more than any real data does.
transfer-size-mismatch = The archive's entry { $path } doesn't have the size the archive gives for it.
transfer-missing = The archive lacks { $path }.
transfer-unexpected = The archive's entry { $path } is not part of an archive's layout.
transfer-format = This is not an archive of this program (its format is "{ $format }").
transfer-version-newer = The archive is of format version { $version }; this version of the program reads version { $supported }. Update the program to import it.
transfer-version-unknown = The archive's format version { $version } is unknown; this version of the program reads version { $supported }.
transfer-hash = The content of { $path } doesn't match its checksum in the manifest.
transfer-unlisted = The archive's entry { $path } is not listed in the manifest.
transfer-json = { $path }, line { $line }, is not valid: { $detail }
transfer-line-too-long = { $path }, line { $line }, is too long.
transfer-counts = The manifest counts { $said } of "{ $what }", but the archive holds { $found }.
transfer-invalid-manifest-project = { $path }: the project's ID or key differs from the manifest.
transfer-invalid-utf8 = { $path } is not UTF-8 text.
transfer-invalid-stray-description = { $path } is the description of no task in the archive.
transfer-invalid-id = { $path }, line { $line }: "{ $value }" is not an ID.
transfer-invalid-id-twice = { $path }, line { $line }: the ID { $id } is used twice.
transfer-invalid-rank = { $path }, line { $line }: "{ $value }" is not a rank.
transfer-invalid-color = { $path }, line { $line }: "{ $value }" is not a color.
transfer-invalid-icon = { $path }, line { $line }: "{ $value }" is not a project icon.
transfer-invalid-project = { $path }: the project needs a name and a state.
transfer-invalid-no-statuses = { $path }: a project needs statuses and task types.
transfer-invalid-project-parent = { $path }: the project is its own parent.
transfer-invalid-project-nesting = { $path }: sub-projects form a circle.
transfer-invalid-task-project = { $path }, line { $line }: the task belongs to another project.
transfer-invalid-task-number = { $path }, line { $line }: task number { $number } is not unique.
transfer-invalid-task-status = { $path }, line { $line }: the status or type is not one of the project's.
transfer-invalid-task-label = { $path }, line { $line }: the label { $label } is not one of the project's.
transfer-invalid-title = { $path }, line { $line }: the title is empty or too long.
transfer-invalid-description-size = { $path }, line { $line }: the description is larger than 1 MB.
transfer-invalid-link-target = { $path }, line { $line }: a link points to no other task in the archive.
transfer-invalid-link-kind = { $path }, line { $line }: "{ $kind }" is not a kind of link.
transfer-invalid-task-nesting = { $path }, line { $line }: subtasks nest too deep or in a circle.
transfer-invalid-task-parent = { $path }, line { $line }: the parent task is not in the project.
transfer-invalid-comment-task = { $path }, line { $line }: the comment's task is not in the project.
transfer-invalid-comment-reply = { $path }, line { $line }: the comment answers no comment of its task.
transfer-invalid-time-task = { $path }, line { $line }: the time entry's task is not in the project.
transfer-invalid-page-project = { $path }, line { $line }: the page belongs to another project.
transfer-invalid-page-title = { $path }, line { $line }: the page's title is not valid.
transfer-invalid-page-slug = { $path }, line { $line }: the page's slug is empty or too long.
transfer-invalid-page-nesting = { $path }, line { $line }: pages nest too deep or in a circle.
transfer-invalid-two-briefs = { $path }, line { $line }: a project has one page of instructions for agents, not two.
transfer-invalid-page-parent = { $path }, line { $line }: the parent page is not in the project.
transfer-invalid-revision-owner = { $path }, line { $line }: the revision's owner is not in the project.
transfer-invalid-revision-size = { $path }, line { $line }: the revision is larger than 1 MB.
transfer-invalid-file-owner = { $path }, line { $line }: the attachment's owner is not in the project.
transfer-invalid-file-missing = { $path }, line { $line }: the attachment's file is not in the archive.
transfer-invalid-file-name = { $path }, line { $line }: the attachment's name is empty or too long.
transfer-keys-taken = { $count ->
        [one] The project key { $keys } is taken in this workspace.
       *[other] The project keys { $keys } are taken in this workspace.
    }
transfer-keys-taken-hint = Give each a new key in `keys`, as from and to; a dry run lists them.
transfer-key-unknown = The archive has no project with the key { $key }.
transfer-key-invalid = "{ $key }" is not a valid project key.
transfer-agent-path = Agents give a plain file name, which is placed in the workspace's exports folder.
transfer-write-failed = { $path } could not be written: { $error }
transfer-archive-write-failed = The archive could not be written: { $error }
transfer-stored-file-missing = The stored file { $sha256 } is missing from the workspace, so the export stopped.
