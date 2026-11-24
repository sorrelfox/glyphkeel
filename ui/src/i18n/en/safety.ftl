# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# History and data safety: the activity feed, the trash, backups, export
# and import, and moving things to the trash.

## Screens and actions

view-trash = Trash
view-activity = Activity
action-trash = Trash
action-activity = Activity
preferences-page-data = Data and backups

## Moving to the trash

trash-move = { $count ->
    [one] Move to trash
   *[other] Move { $count } tasks to trash
}
trash-tasks-moved = { $count ->
    [one] { $total ->
        [one] Moved one task to the trash
       *[other] Moved a task and its subtasks to the trash ({ $total } in all)
    }
   *[other] Moved { $count } tasks to the trash
}
history-task-trashed = Moved to the trash
history-task-restored = Restored from the trash

## The trash

trash-toolbar = Trash actions
trash-filter-kind = Kind of item
trash-filter-project = Project
trash-kind-all = Everything
trash-kind-task = Task
trash-kind-project = Project
trash-kind-wiki_page = Wiki page
trash-kind-comment = Comment
trash-kind-file = File
trash-kind-time_entry = Time entry
trash-restore = Restore
trash-purge = Delete for good
trash-empty = Empty trash
trash-about = { $days ->
    [one] Items stay in the trash for one day; then they are deleted for good.
   *[other] Items stay in the trash for { $days } days; then they are deleted for good.
}
trash-none = The trash is empty.
trash-more = Show more
trash-select-all = Select everything shown
trash-select = Select { $title }
trash-untitled = (untitled)
trash-on-task = on { $key }
trash-contains = { $count ->
    [one] with one more item
   *[other] with { $count } more items
}
trash-column-item = Item
trash-column-kind = Kind
trash-column-project = Project
trash-column-deleted = Deleted
trash-column-purged = Deleted for good
trash-deleted-by = { $actor } · { $time }
trash-days-left = { $days ->
    [0] today
    [one] in one day
   *[other] in { $days } days
}
trash-restored = { $total ->
    [one] Restored one item
   *[other] Restored { $total } items
}
trash-purged = { $total ->
    [one] Deleted one item for good
   *[other] Deleted { $total } items for good
}

## The activity feed

activity-filters = Filters
activity-filter-project = Project
activity-filter-actor = Who
activity-actor-all = Everyone
activity-actor-user = People
activity-actor-agent = Agents
activity-from = From
activity-to = To
activity-clear-filters = Clear filters
activity-count = { $shown } of { $total }
activity-none = No changes recorded yet.
activity-none-matching = No changes match the filters.
activity-loading = Loading older changes…
activity-column-when = When
activity-column-who = Who
activity-column-what = What
activity-column-project = Project
activity-column-revert = Revert
activity-open-task = Open the task
activity-created = { $entity } created
activity-changed = { $entity } changed
activity-trashed = { $entity } moved to the trash
activity-restored = { $entity } restored
activity-purged = { $entity } deleted for good
activity-entity-task = Task
activity-entity-project = Project
activity-entity-comment = Comment
activity-entity-attachment = File
activity-entity-time_entry = Time entry
activity-entity-relation = Link
activity-entity-status = Status
activity-entity-task_type = Task type
activity-entity-label = Label
activity-entity-layout = Board layout
activity-entity-list_layout = List layout
activity-entity-view = Saved view
activity-entity-wiki_page = Wiki page
activity-entity-settings = Settings
activity-entity-timer = Timer
activity-entity-description = Description
activity-entity-other = Item

## Backups

backups-title = Backups
backups-about = A backup is a checked copy of the database and the stored files. The app makes one every day and keeps the recent ones.
backups-folder = Backup folder
backups-folder-about = Where backups are made from now on. Backups made before stay where they are.
backups-folder-change = Change…
backups-folder-default = Use default
backups-folder-changed = Backups go to the new folder from now on.
backups-folder-old-stay = Backups made before stay in the old folder.
backups-last = Last backup
backups-never = None yet
backups-now-about = Make a backup of the workspace now.
backups-making = Backing up…
backups-list = Backups kept
backups-column-made = Made
backups-column-kind = Kind
backups-column-size = Size
backups-column-check = Check
backups-column-actions = Actions
backups-files = { $count ->
    [one] one stored file
   *[other] { $count } stored files
}
backups-unchecked = Not checked
backups-checking = Checking…
backups-check-ok = { $count ->
    [one] Good; one file checked
   *[other] Good; { $count } files checked
}
backups-check-problems = { $count ->
    [one] One problem
   *[other] { $count } problems
}
backups-verify = Check
backups-restore = Restore…
backups-delete = Delete…
backup-kind-daily = Daily
backup-kind-weekly = Weekly
backup-kind-manual = Made by hand
backup-kind-pre_migration = Before an upgrade
backup-kind-safety = Safety copy
backup-kind-pre_restore = Before a restore
backup-made = Backup made: { $name }
backup-deleted = Backup deleted: { $name }
backup-restored = Workspace restored from { $name }
backup-restored-previous = The state before is kept as the backup { $name }.

## Export and import

transfer-title = Export and import
transfer-export-about = Write projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments.
transfer-import-about = Add the projects of an archive to this workspace. Nothing that is here already changes.
transfer-export-title = Export
transfer-export-scope = What to export
transfer-scope-all = The whole workspace
transfer-scope-some = Chosen projects
transfer-export-projects = Projects to export
transfer-export-subprojects = Each project is exported with its sub-projects.
transfer-format-label = Format
transfer-format-zip = One .zip file
transfer-format-folder = A folder
transfer-export-choose = Choose where…
transfer-export-start = Export
transfer-export-browser = The archive goes to the workspace's exports folder.
# The archive's file name: a .zip file, or a folder without an extension.
transfer-export-name = { $format ->
    [zip] glyphkeel-export-{ $date }.zip
   *[folder] glyphkeel-export-{ $date }
}
transfer-exporting = Exporting…
transfer-exported = { $count ->
    [one] Exported one project
   *[other] Exported { $count } projects
}
transfer-done = Done
transfer-count-projects = Projects
transfer-count-tasks = Tasks
transfer-count-comments = Comments
transfer-count-attachments = Files
transfer-count-wiki_pages = Wiki pages
transfer-count-time_entries = Time entries
transfer-import-title = Import
transfer-import-choose-about = Choose an archive: a .zip file or a folder that this program exported. It is checked completely before anything is imported.
transfer-import-zip = Choose a .zip file…
transfer-import-folder = Choose a folder…
transfer-import-name = Archive in the exports folder
transfer-import-check = Check
transfer-checking = Checking the archive…
transfer-importing = Importing…
transfer-check-ok = { $name } can be imported. It was written by version { $version }.
transfer-taken-title = Taken project keys
transfer-taken-about = { $count ->
    [one] One project key is taken here. Give the project a new key:
   *[other] { $count } project keys are taken here. Give each project a new key:
}
transfer-new-key = New key for { $key }
transfer-import-start = { $count ->
    [one] Import one project
   *[other] Import { $count } projects
}
transfer-imported = { $count ->
    [one] Imported one project
   *[other] Imported { $count } projects
}
transfer-imported-projects = Imported projects

## Deleting a project

project-delete = Delete
project-delete-start = Move to trash…
project-delete-confirm = { $subprojects ->
    [0] Move { $name } and its { $tasks ->
        [one] task
       *[other] { $tasks } tasks
    } to the trash?
   *[other] Move { $name }, its { $subprojects ->
        [one] sub-project
       *[other] { $subprojects } sub-projects
    } and everything in them to the trash?
} It can be restored from there.
project-delete-confirm-button = Move to trash
project-deleted = { $subprojects ->
    [0] { $name } moved to the trash, with { $tasks ->
        [one] one task
       *[other] { $tasks } tasks
    }
   *[other] { $name } and { $subprojects } sub-projects moved to the trash, with { $tasks ->
        [one] one task
       *[other] { $tasks } tasks
    }
}
project-delete-title = Delete { $name }
project-export = Export…
