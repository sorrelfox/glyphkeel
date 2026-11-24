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
task-day-limit = { $count ->
        [one] This session created one task in the last 24 hours; an agent may create { $max } a day.
       *[other] This session created { $count } tasks in the last 24 hours; an agent may create { $max } a day.
    }
task-day-limit-hint = { $hours ->
        [one] Wait an hour before creating more, or ask the person to create them.
       *[other] Wait { $hours } hours before creating more, or ask the person to create them.
    }
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
backup-safety-failed = { $count ->
        [one] This change touches one item, and the safety backup made before large changes by agents failed, so nothing was changed: { $error }
       *[other] This change touches { $count } items, and the safety backup made before large changes by agents failed, so nothing was changed: { $error }
    }
backup-safety-failed-hint = Check that the backup folder has room and can be written to, then try again.
backup-safety-pending = { $count ->
        [one] A safety backup is needed before this change to one item.
       *[other] A safety backup is needed before this change to { $count } items.
    }
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
backup-folder-not-absolute = "{ $path }" is not a full path; give one that starts at the root, such as /home/me/Backups.
backup-folder-inside-workspace = { $path } is inside the workspace's own folder; choose a folder outside it.
backup-folder-unwritable = Backups can't be written to { $path }: { $error }

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

## Activity and revert

activity-to-before-from = The last day is before the first.
revert-no-group = There is no such history group.
revert-already = It was already reverted.
revert-imported = It came from an imported archive.
revert-deleted-for-good = The { $entity } it changed was deleted for good.
revert-deleted = A deleted { $entity } can't be brought back by a revert.
revert-not-yet = Changes to { $entity } { $field } can't be reverted yet.
revert-changed-since = { $entity } { $field } was changed again since.
revert-app-only-setting = It changed a setting that only the person changes, in the app.

## The pipeline

op-app-only = { $op } is available in the app only.

## Values

value-day-format = "{ $text }" is not a date: expected YYYY-MM-DD, for example 2026-12-01.
value-day-not-in-calendar = { $text } is not a day in the calendar.
value-day-out-of-range = The date is outside the years 1 to 9999.
value-day-start-after-due = The start date { $start } is after the due date { $due }.
value-duration-empty = Enter a duration, for example 90m, 1.5h, 1:30, 2d or 1w.
value-duration-malformed = "{ $text }" is not a duration: use for example 90m, 1.5h, 1:30, 2d, 1w or 1h 30m.
value-duration-missing-unit = "{ $text }" needs a unit: m, h, d or w (for example { $text }m or { $text }h).
value-duration-repeated-unit = "{ $text }" names the same unit twice.
value-duration-too-long = "{ $text }" is too long a duration.
value-duration-units = A working day has 1 to 24 hours and a week 1 to 7 days, not { $hours } and { $days }.
value-file-name-empty = The file name is empty.
value-file-base64 = The data is not valid Base64 (at character { $position }).
value-id-malformed = "{ $text }" is not an ID: expected 32 hexadecimal digits, optionally hyphenated.
value-id-length = An ID is 16 bytes, not { $length }.
value-key-length = A project key has { $min } to { $max } characters; "{ $key }" has { $length }.
value-key-first-letter = A project key starts with a letter A–Z; "{ $key }" does not.
value-key-character = A project key uses only the capital letters A–Z and the digits 0–9; "{ $key }" contains '{ $char }'.
value-task-key-shape = "{ $key }" is not a task key: expected a project key, a hyphen and a number, like TM-41.
value-task-number = "{ $key }" is not a task number: expected a whole number from 1.
value-not-one-of = { $what ->
        [priority] "{ $value }" is not a priority; expected one of: { $expected }.
        [status-category] "{ $value }" is not a status category; expected one of: { $expected }.
        [kind-of-actor] "{ $value }" is not a kind of actor; expected one of: { $expected }.
        [program] "{ $value }" is not a program; expected one of: { $expected }.
        [colour] "{ $value }" is not a colour; expected one of: { $expected }.
        [type-glyph] "{ $value }" is not a type glyph; expected one of: { $expected }.
        [project-icon] "{ $value }" is not a project icon; expected one of: { $expected }.
       *[other] "{ $value }" is not a valid value; expected one of: { $expected }.
    }
value-edit-empty = Edit { $index } has no text to find.
value-edit-not-found = The text of edit { $index } was not found.
value-edit-ambiguous = The text of edit { $index } occurs { $count } times; add more around it.
value-rank-malformed = "{ $text }" is not a rank: expected 1 to { $max } digits 0-9 and a-z, not all zero.
value-rank-no-room = No rank fits between "{ $lower }" and "{ $upper }"; the list needs re-spreading.
value-text-empty = { $what ->
        [title] The title is empty.
        [name] The name is empty.
        [description] The description is empty.
        [comment] The comment is empty.
        [body] The body is empty.
       *[other] The text is empty.
    }
value-text-too-long = { $what ->
        [title] The title is { $length } characters long; the limit is { $max }.
        [name] The name is { $length } characters long; the limit is { $max }.
        [description] The description is { $length } characters long; the limit is { $max }.
       *[other] The text is { $length } characters long; the limit is { $max }.
    }
value-text-too-long-bytes = { $what ->
        [description] The description is { $length } bytes long; the limit is { $max }.
        [comment] The comment is { $length } bytes long; the limit is { $max }.
        [body] The body is { $length } bytes long; the limit is { $max }.
       *[other] The text is { $length } bytes long; the limit is { $max }.
    }
value-page-title-reserved = A page title can't contain "{ $char }".
value-log-zero = Log at least one minute.
value-log-over-a-day = { $minutes } minutes is more than a day; log the time on each day it was done.
value-log-note-too-long = The note is { $length } characters long; the limit is { $max }.
value-reminder-too-early = A reminder can come at most { $max } days before the due date.

## Looking things up

lookup-project-not-found = There is no project "{ $project }".
lookup-project-not-found-hint = Use a project key from list_projects (`glyphkeel project list`).
lookup-status-not-found = Project { $project } has no status "{ $name }".
lookup-type-not-found = Project { $project } has no task type "{ $name }".
lookup-label-not-found = Project { $project } has no label "{ $name }".
project-archived = Project { $project } is archived, so it can't be changed.
project-archived-hint = Unarchive it first.
project-wiki-off = The wiki is switched off in project { $project }, so its pages can't be changed.
project-wiki-off-hint = Switch the wiki on in the project's settings first.
task-not-found = There is no task "{ $task }".
task-not-found-hint = Task keys look like TM-41.
task-version-changed = { $task } changed since you read it: it is at version { $version }, not { $expected }.
task-version-changed-hint = Read it again and repeat the change if it still applies.

## Placing items in order

place-one-target = Give only one of `before`, `after` and `at`.
place-bad-neighbour = An item can't be placed next to itself or next to one in another list.

## Tasks

task-get-count = Ask for 1 to { $max } tasks at once.
project-no-statuses = Project { $project } has no statuses; add one first.
project-no-types = Project { $project } has no task types; add one first.
task-parent-other-project = { $task }'s parent must be in the same project.
task-new-parent-other-project = The new task's parent must be in the same project.
task-parent-loop = { $task } can't be a subtask of itself or of one of its own subtasks.
task-too-deep = Subtasks go at most { $max } levels deep; this would make { $levels }.
task-not-in-column = { $task } is not in the target column.
task-not-a-subtask = { $task } is not a subtask, so it has no place to change.
task-not-a-subtask-hint = Make it a subtask first: update_task with `parent`.
task-not-a-sibling = { $task } is not another subtask of the same parent.
task-subtask-where = Say where the subtask goes.
task-subtask-where-hint = Pass `before` or `after` with another subtask's key, or `at`.
field-given-and-cleared = { $field } is both given and cleared.
task-description-and-edits = Give either a new description or edits to it, not both.
task-base-revision-alone = base_revision goes with a new description.
task-description-conflict = Someone changed the same lines of the description since your base revision. The description stays as they left it; your text was kept as conflict copy { $copy }.
task-progress-range = Progress is 0 to 100.
task-open-subtasks = { $count ->
        [one] { $task } still has one open subtask.
       *[other] { $task } still has { $count } open subtasks.
    }
task-still-blocked = { $task } is still blocked by { $blockers }.
task-still-blocked-outside = { $task } is still blocked by a task outside the project you work in.
task-label-left-off = Label "{ $label }" doesn't exist in { $project } and was left off.
storage-task-no-project = A task needs a project.
storage-task-no-number = A task needs a number.

## Projects

project-parent-or-top = Give either `parent` or `top_level`.
place-say-where = Say where: `before`, `after` or `at`.
project-key-taken = The key { $key } is already used by a project, now or before.
project-key-taken-hint = Choose another key; old keys stay reserved so links keep working.
project-key-unavailable = The key { $key } can't be used here.
project-key-unavailable-hint = Choose another key.
project-parent-last-level = Projects go at most { $max } levels deep; { $project } is already on the last level.
project-parent-loop = { $project } can't go under itself or one of its own sub-projects.
project-move-too-deep = Projects go at most { $max } levels deep; under { $parent } { $project } would need { $levels }.
project-not-a-sibling = { $sibling } is not a sibling of { $project } at its new place.
project-inside-archived = { $project } is inside an archived project; unarchive that one instead.
project-name-taken = Project { $project } already has "{ $name }".

## Statuses, task types and labels

status-wip-limit = A WIP limit is 1 or more.
status-needs-name = A new status needs a name.
status-last-one = A project needs at least one status.
status-delete-move-to = { $count ->
        [one] "{ $status }" has one task; say which status it moves to.
       *[other] "{ $status }" has { $count } tasks; say which status they move to.
    }
status-move-to-same = The tasks must move to another status.
type-needs-name = A new task type needs a name.
type-delete-default = "{ $type }" is the default type; make another type the default first.
type-delete-move-to = Say which type the tasks get.
type-move-to-same = The tasks must get another type.
label-needs-name = A new label needs a name.

## Bulk changes

bulk-set-and-shift = Give { $field } or a shift of it, not both.
bulk-date-invalid = "{ $text }" is not a date. Write 2026-12-31, or today, fri, next mon, +3d.
bulk-shift-invalid = "{ $text }" is not a shift. Write +2d, -1w, +1m or +1y.
bulk-keys-or-query = Give the tasks' keys or a query.
bulk-keys-and-query = Give keys or a query, not both.
bulk-no-changes = Say what to change.
bulk-too-many = A bulk change can take at most { $max } tasks; this one has { $count }.
bulk-too-many-hint = Narrow the query, or make the change in parts.
bulk-expected-count = Pass expected_count, the count from the preview, to make the change.
bulk-count-changed = { $expected ->
        [one] The change was previewed for one task, but the count is now { $count }.
       *[other] The change was previewed for { $expected } tasks, but the count is now { $count }.
    }
bulk-count-changed-hint = Preview the change again and check the tasks.

## Claims

claim-override-needs-full = Overriding { $agent }'s claim on { $task } needs the full profile.
claim-taken = { $task } is claimed by { $agent } until { $until }.
claim-taken-hint = Pick another task, wait for the claim to end, or ask the person.
claim-person = Only agent sessions claim tasks; the person is never blocked by claims.
claim-status-or-keep = Give either `status` or `keep_status`.
claim-lease-range = A claim lasts 1 to { $max } minutes.

## Links

link-task-gone = One of the linked tasks is gone, so the link can't come back.
link-to-itself = A task can't be linked to itself.
link-to-subtask = { $task } and { $other } are a task and its own subtask; they can't be linked.
link-loop = This link would make a loop of tasks blocking each other.
link-not-a-subtask = { $task } is not a subtask of the other task.
link-not-found = There is no such link between { $task } and { $other }.
link-not-found-hint = get_task with include ["links"] lists a task's links.

## Boards and views

board-column-not-found = The board of { $project } has no column "{ $column }".
board-hide-done-range = Use 0 to { $max } days.
board-card-field = Cards can't show "{ $field }".
view-field-name = Each entry is a field name of 1 to { $max } characters.
list-layout-columns = A list layout names at most { $max } columns.
list-layout-width = A column is { $min } to { $max } pixels wide.
list-layout-grouping = Groups can be folded only for status, priority, label, parent, due week or project.
list-layout-collapsed = A list keeps at most { $max } folded groups per grouping, each name at most { $length } characters long.
view-name-taken = There is already a view named "{ $view }".
view-name-taken-hint = Choose another name, or change that view.
view-not-found = There is no view "{ $view }".
view-not-found-hint = Name a view of the project given with `project`, or use its ID.
view-name-ambiguous = Several views are named "{ $view }", for different screens.
view-name-ambiguous-hint = Use the view's ID from view.list.

## Stored data that can't be read

storage-link-broken = A recorded link is broken.
storage-copy-not-made = The copy was not made.
storage-value-type = A recorded value has the wrong type.
storage-unknown-field = { $entity }.{ $field } is not a recorded field.
storage-status-id = A status ID is broken.
storage-task-id = A task ID is broken.

## Storage

storage-busy = The workspace is busy: another program kept it locked; try again.
storage-busy-hint = Another program is writing to the workspace; try again in a moment.
storage-upgraded = The workspace was upgraded to schema version { $found } while this program (schema version { $expected }) was running; restart it.
storage-upgraded-hint = Restart the program; a newer version is now needed.
storage-replaced = The workspace was restored from a backup while this program was running; restart it.
storage-replaced-hint = Restart the program; the workspace was restored from a backup.
storage-too-new = The workspace has schema version { $found }, but this program supports only up to { $supported }; install a newer Glyphkeel.
storage-too-new-hint = Install the Glyphkeel version that made this workspace, or a newer one.
storage-needs-upgrade = The workspace has schema version { $found } and must be upgraded to { $current } first.
storage-needs-upgrade-hint = Open the app once to upgrade this workspace, or run `glyphkeel migrate`.
storage-invalid-name = "{ $name }" cannot be a workspace name: use letters, digits, spaces, '-' or '_', at most 64 characters.
storage-not-a-workspace = { $path } is not a Glyphkeel workspace database.
storage-no-wal = { $path } is on a file system that does not support the write-ahead log; use a local disk.
storage-no-data-dir = There is no data folder to find the workspace in: no home folder is set.
storage-closed = The workspace is closed.
storage-migration = Upgrading the workspace to schema version { $version } failed: { $error }
storage-corrupt = The workspace database holds invalid data: { $error }
storage-io = { $path } could not be used: { $error }
storage-sqlite = Database error: { $error }
storage-json = Data could not be read or written as JSON: { $error }

## The engine

engine-app-open = Another Glyphkeel app already has this workspace open.
engine-app-open-hint = Switch to the open window.
scope-project-not-found = The project "{ $project }" that this caller is limited to doesn't exist.
scope-project-not-found-hint = Start the agent server with the key of an existing project, or without a project.
scope-outside = The change reaches outside the project { $project }, which this caller is limited to.
scope-outside-hint = Only { $project } and its sub-projects can be changed; ask the person to make this change.
engine-upgrade-failed-restored = Upgrading the workspace failed, so it was put back as it was before, from the backup "{ $name }": { $error }
engine-upgrade-failed-restored-hint = Your data is unchanged. Keep using the older Glyphkeel version and report the problem.
engine-upgrade-failed-not-restored = Upgrading the workspace failed, and it could not be put back from the backup "{ $name }": { $error }
engine-upgrade-failed-not-restored-hint = Don't open the workspace again; restore the backup "{ $name }" from the backups folder.
engine-no-operation = There is no operation "{ $op }".
engine-input-unencodable = The input could not be encoded: { $error }.
engine-result-unreadable = The result of { $op } could not be read: { $error }.
engine-upgraded-meanwhile = A newer Glyphkeel upgraded this workspace while this program was running.
engine-restart-hint = Restart the program.
engine-profile-refused = Your profile ({ $profile }) doesn't allow { $safety } operations such as { $op }.
engine-profile-raise-hint = Ask the person to start the agent server with a higher --profile.
engine-profile-capped-hint = The workspace setting "most an agent may do" doesn't allow it; ask the person.
engine-not-confirmed = { $op } can't be undone and was not confirmed.
engine-not-confirmed-hint = Confirm it in the dialog, or pass --yes on the command line.

## Undo, redo and revert

history-nothing-to-undo = Nothing to undo.
history-nothing-to-redo = Nothing to redo.
history-undo-blocked = Can't undo: { $reason }
history-redo-blocked = Can't redo: { $reason }
history-revert-no-group = The revert recorded no history group.

## Input

input-invalid = Invalid input: { $reason }.
input-invalid-at = Invalid input at `{ $path }`: { $reason }.
input-schema-hint = See the operation's input schema.
input-result-unwritable = The result could not be written as JSON: { $error }.
input-not-an-id = "{ $text }" is not an ID.
input-id-hint = IDs look like 0193b2a4-5c6d-7e8f-9a0b-1c2d3e4f5a6b.
input-limit = limit must be from 1 to { $max }.
input-cursor-unknown = The cursor is not one this list gave.

## Time zones

time-zone-unknown = "{ $name }" is not a known time zone.
time-zone-unknown-hint = Use "system" or an IANA name such as "Europe/Berlin".

## History

history-revert-what = Name the history groups or the session to revert.
history-revert-what-hint = Pass `groups` (from get_activity) or `session`.
history-no-session = You have no agent session.
history-task-holds-subtasks = The task now holds subtasks, so its creation can't be reverted.
history-project-holds-items = The project now holds tasks or sub-projects, so its creation can't be reverted.
history-deleted-not-recreated = A deleted { $entity } can't be created again by a revert.
history-in-use = { $count ->
        [one] The { $entity } is in use by one task, so its creation can't be reverted.
       *[other] The { $entity } is in use by { $count } tasks, so its creation can't be reverted.
    }

## Descriptions, revisions and checklists

description-item-gone = The item is gone.
description-task-or-project = Name either a task or a project.
description-task-or-comment = Name either a task (key) or a comment.
task-project-gone = The task's project is gone.
revision-not-found-page-hint = Read the page again; its revision is the one to pass.
revision-not-found-task-hint = Read the task again; its description_revision is the one to pass.
revision-no-such = There is no revision "{ $revision }".
revision-unreadable = A recorded revision is unreadable.
revision-gone = The recorded revision of the description is gone.
edit-empty = The text to replace is empty.
edit-not-in-page = This text is not in the page (edits apply in order).
edit-not-in-description = This text is not in the description (edits apply in order).
edit-ambiguous-page = This text occurs { $count } times in the page; include more around it.
edit-ambiguous-description = This text occurs { $count } times in the description; include more around it.
edit-page-hint = Read the page again; nothing was changed.
edit-task-hint = Read the task again; nothing was changed.
conflict-not-a-copy = That revision is not a conflict copy.
conflict-resolved-already = That conflict copy was resolved already.
conflict-read-task-hint = Read the task again.
conflict-page-changed = The page changed again since you read it; nothing was resolved.
conflict-description-changed = The description changed again since you read it; nothing was resolved.
conflict-changed-hint = Read the description again and resolve the copy against it.
conflict-use-text-needs-text = use_text needs the text to save.
render-one-source = Give exactly one of source, task, comment, project and revision.
render-count = Render 1 to { $max } texts at once.
checklist-changed = The checklist changed; please tick it again. Item { $number } reads "{ $actual }", not "{ $expected }".
checklist-changed-hint = Read the checklist again (get_task with include ["checklist"]).
checklist-item-needs-text = An item needs text.
checklist-no-item = { $count ->
        [one] There is no item { $number }; the checklist has one.
       *[other] There is no item { $number }; the checklist has { $count }.
    }
checklist-inside-removed = An item can't be added inside an item that is removed.
checklist-nothing-to-change = Nothing to change: give toggle, add or remove.

## Comments

comment-not-found = There is no comment "{ $comment }".
comment-not-found-hint = Comment IDs are listed by get_task with include ["comments"].
comment-task-gone = The comment's task is gone.
comment-not-on-task = There is no comment "{ $comment }" on this task.
comment-reply-other-task = An answer must be on the same task as the comment it answers.
comment-gone = The comment is gone.
comment-version = The comment changed since you read it: it has version { $version }, not { $expected }.
comment-version-hint = Read the comment again and repeat the change.

## Wiki pages

wiki-list-hint = List the pages with list_wiki_pages (`glyphkeel wiki list`).
wiki-project-gone = The page's project is gone.
wiki-no-place = No place was found for the page.
wiki-parent-and-clear = Give parent or clear it, not both.
wiki-page-gone = The page is gone.
wiki-holds-pages = The page now holds other pages, so its creation can't be reverted.

## Task queries

query-task-limit = Ask for 1 to { $max } tasks at once.
list-cursor-hint = Pass the next_cursor of the page before, or leave it out.

## Settings

settings-given-and-cleared = { $name } is both given and cleared.
settings-not-a-time = "{ $time }" is not a time of day; write it as HH:MM, for example 09:00.
settings-out-of-range = { $field } must be from { $min } to { $max }; got { $value }.

## Attached files

file-no-thumbnail = { $name } has no thumbnail: { $error }.
file-no-thumbnail-hint = Thumbnails are made for PNG, JPEG, GIF and WebP images.
file-not-found = There is no attached file "{ $file }".
file-not-found-hint = Attachment IDs are listed by get_task with include ["files"].
file-task-gone = The file's task is gone.
file-unreadable = The stored file can't be read: { $error }.
file-image-too-large = The image is larger than { $max } MB, so its content is not returned.
file-not-text-or-image = This file is neither text nor an image; only its details are returned.
file-too-large = The file is { $size } MB; files can be at most { $max } MB.
file-too-large-hint = The limit is the workspace setting file_max_mb.
file-attach-to-what = Name either the `task` or the `comment` to attach the file to.
file-path-unreadable = Can't read "{ $path }": { $error }.
file-path-not-a-file = "{ $path }" is not a file.
file-path-or-data = Give either `path` or `data_base64` (with `name`).
file-data-needs-name = A file given as `data_base64` needs a `name`.
file-not-stored = The file can't be stored: { $error }.
file-task-or-comment = Name the task or the comment.
file-content-gone = The file's content is no longer stored, so it can't come back.

## Time entries and the timer

time-entry-not-found = There is no time entry "{ $entry }".
time-entry-not-found-hint = Entry IDs are listed by get_task with include ["time"].
time-entry-version = The time entry changed since you read it: it has version { $version }, not { $expected }.
time-entry-version-hint = Read the entry again and repeat the change.
storage-field-unreadable = A recorded { $field } can't be read.
time-report-too-long = A report covers at most { $max } days.
timer-unreadable = A recorded timer can't be read.
timer-task-trashed = The timer's task is in the trash.
timer-task-trashed-hint = Stop the timer with `discard` to throw its time away.
timer-too-short = The timer ran for under half a minute, so nothing was logged.
timer-too-long = The timer ran for { $duration }, more than a day.
timer-too-long-hint = Stop it with `duration` set to the time worked, or with `discard`.
timer-none = No timer is running or paused.
timer-none-hint = Start one on a task with timer.start (`glyphkeel timer start KEY`).

## Reminders

reminder-unreadable = A recorded reminder can't be read.
reminder-time-with-at = `time` goes with `days_before_due`; put the time into `at`.
reminder-not-a-moment = "{ $text }" is not a moment.
reminder-moment-hint = Write it as 2026-11-24 09:00 (local time) or in RFC 3339.
reminder-not-a-time = "{ $time }" is not a time of day; write it as HH:MM.
reminder-at-or-days = Give either `at` or `days_before_due`.
reminder-at-or-days-hint = To remove the reminder, use reminder.clear.
reminder-ack-count = Acknowledge 1 to { $max } reminders at once.

## Export and import

transfer-store-entry-failed = Can't store { $path }: { $error }.
transfer-store-files-failed = Can't store the archive's files: { $error }.

## Thumbnails (reasons inside file-no-thumbnail)

thumb-unsupported = no thumbnails are made for { $mime }
thumb-too-large = the image is too large ({ $width } × { $height } pixels)
thumb-not-cached = the thumbnail can't be cached: { $error }
thumb-unreadable = the stored file can't be read: { $error }

## Drafts

draft-title-long = A draft's title is at most { $max } characters.
