# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Versions (releases) and the roadmap

view-roadmap = Roadmap

version-status-open = Open
version-status-locked = Locked
version-status-closed = Closed
version-sharing-none = This project only
version-sharing-descendants = This project and its sub-projects

## The task panel's version

prop-version = Version
prop-version-value = Version: { $value }
# The version picker's name when the version isn't open: its name, then its status (Locked, Closed).
prop-version-value-status = Version: { $value } { $status }
prop-version-shared = { $name } (from { $project })
prop-version-loading = Loading versions…
prop-version-none-open = No open versions
prop-version-clear = No version
history-field-fix_version = Version
history-version-gone = a deleted version
activity-entity-version = Version

## The roadmap page

roadmap-title = Roadmap of { $project }
roadmap-new-version = New version
roadmap-show-closed = { $count ->
        [0] Show closed versions
       *[other] Show closed versions ({ $count })
    }
roadmap-unplanned = { $count ->
        [one] One open task is planned for no version.
       *[other] { $count } open tasks are planned for no version.
    }
roadmap-error = The roadmap couldn't be read: { $message }
roadmap-empty = No versions yet. A version groups the tasks of one release, with its dates and progress.
roadmap-all-closed = Every version is closed. Show the closed versions to see them.
roadmap-due = Due { $date }
roadmap-starts = Starts { $date }
roadmap-no-dates = No dates
roadmap-late = Late
roadmap-shared-from = From { $project }
roadmap-shared = Shared with sub-projects
roadmap-version-menu = Version { $name }
roadmap-edit = Edit…
roadmap-reopen = Open again
roadmap-close = Close version
roadmap-delete = Delete version
roadmap-progress-label = Progress of { $name }
roadmap-progress = { $total ->
        [one] { $closed } of one task closed, { $percent } done
       *[other] { $closed } of { $total } tasks closed, { $percent } done
    }
roadmap-tasks = { $count ->
        [one] One task
       *[other] { $count } tasks
    }
roadmap-tasks-of = Tasks of { $name }
roadmap-task-closed = Closed

roadmap-new-title = New version
roadmap-edit-title = Edit { $name }
roadmap-create = Create
roadmap-save = Save
roadmap-field-name = Name
roadmap-field-start = Start date
roadmap-field-due = Due date
roadmap-field-status = Status
roadmap-field-sharing = Used by
roadmap-field-description = Description
roadmap-created = Version { $name } created
roadmap-changed = Version { $name } changed
roadmap-reopened = Version { $name } is open again
roadmap-closed = Version { $name } closed

roadmap-close-title = Close { $name }
roadmap-close-open-tasks = { $count ->
        [0] All its tasks are closed.
        [one] One of its tasks is still open.
       *[other] { $count } of its tasks are still open.
    }
roadmap-close-move = Open tasks go to
roadmap-close-to-none = No version
roadmap-close-keep = Stay in this version

roadmap-delete-title = Delete { $name }?
roadmap-delete-body = { $count ->
        [0] No tasks are planned for it. Undo brings it back.
        [one] Its one task will be planned for no version. Undo brings both back.
       *[other] Its { $count } tasks will be planned for no version. Undo brings them all back.
    }
roadmap-deleted = Version { $name } deleted

## The Gantt chart's version markers

gantt-version-marker = Version { $name }, due { $date }

roadmap-burnup = Burn-up
# $name is the version's name.
roadmap-burnup-named = Burn-up of { $name }
