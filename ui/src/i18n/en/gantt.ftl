# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Gantt chart: a project's tasks as bars on a timeline, with a table of
# their dates beside it. Dates arrive formatted for the locale.

## Toolbar

gantt-zoom = Zoom
gantt-zoom-day = Day
gantt-zoom-week = Week
gantt-zoom-month = Month
gantt-zoom-quarter = Quarter
gantt-today = Today
gantt-earlier = Earlier
gantt-later = Later
gantt-query-label = Filter query
gantt-query-placeholder = Filter, for example: is:open label:release
gantt-include-subprojects = Include sub-projects
gantt-move-dependents = Also move dependents
gantt-group = Group
gantt-group-button = Group: { $grouping }
gantt-group-by-parent = Parent
gantt-group-by-label = Label
gantt-group-by-status = Status
gantt-group-by-claim = Claimed by
gantt-group-by-none = None
gantt-export = Export
gantt-export-png = PNG image
gantt-export-print = Print or save as PDF
# In the Save view dialog, when the view belongs to the Gantt chart.
gantt-view-default = Show it first when the chart opens

## Milestones: the project's list of them, beside the chart

gantt-milestones = Milestones
gantt-milestones-none = This project has no milestones yet. A task of a milestone type is one.
gantt-milestone-no-date = No date

## Table

gantt-label = Gantt chart
gantt-col-key = Key
gantt-col-title = Title
gantt-col-start = Start
gantt-col-due = Due
gantt-col-progress = %
gantt-col-progress-label = Progress
# The agent session working on the task, while its claim lasts.
gantt-col-claim = Claimed by
gantt-claim-until = Claimed by { $agent } until { $until }
# The handle between the table and the timeline, and between two columns.
gantt-resize-table = Width of the table
gantt-resize-column = Width of the column { $column }
gantt-unscheduled =
    { $count ->
        [one] Unscheduled: one task
       *[other] Unscheduled: { $count } tasks
    }
gantt-keys-hint = Up and Down move between tasks, Left and Right fold and unfold. Alt and an arrow key moves the task by a day; with Shift it changes the due date. F2 edits the start date and Shift F2 the due date. L starts a dependency and Enter on another task ends it, or removes it when it exists. Enter opens the task. Plus and minus zoom.
gantt-expand = Unfold { $key }
gantt-collapse = Fold { $key }
gantt-edit-start = Start date of { $key }
gantt-edit-due = Due date of { $key }
gantt-edit-start-value = Start date of { $key }: { $value }
gantt-edit-due-value = Due date of { $key }: { $value }
gantt-none = None
gantt-progress = { NUMBER($percent, style: "percent") }
# A task's row as screen readers read it.
gantt-row-dates = { $key } { $title }, { $from } to { $to }, { $progress } done
gantt-row-due = { $key } { $title }, due { $due }, { $progress } done
gantt-row-start = { $key } { $title }, starts { $start }, no due date
gantt-row-unscheduled = { $key } { $title }, unscheduled
gantt-row-closed = { $row }, closed
gantt-row-claimed = { $row }, claimed by { $agent }
gantt-drag-unscheduled = Drag { $key } onto the timeline to schedule it
gantt-row-behind = { $row }, behind schedule
gantt-row-read-only =
    { $reason ->
       *[archived_project] { $row }, read only because its project is archived
    }
# Group rows when the chart is grouped by label or by status.
gantt-group-label =
    { $count ->
        [one] { $name }: one task
       *[other] { $name }: { $count } tasks
    }
gantt-group-no-label =
    { $count ->
        [one] No label: one task
       *[other] No label: { $count } tasks
    }
gantt-group-status =
    { $count ->
        [one] { $name }: one task
       *[other] { $name }: { $count } tasks
    }
# Group rows when the chart is grouped by the agent session that claimed the tasks.
gantt-group-claim =
    { $count ->
        [one] { $agent }: one task
       *[other] { $agent }: { $count } tasks
    }
gantt-group-unclaimed =
    { $count ->
        [one] Not claimed: one task
       *[other] Not claimed: { $count } tasks
    }

## Timeline notes

gantt-overdue-by =
    { $days ->
        [one] Overdue by one day
       *[other] Overdue by { $days } days
    }
gantt-no-deadline = No deadline
# A bar whose time has passed further than its work: both are percentages.
gantt-behind = Behind schedule: { $done } done, { $expected } of the time gone
gantt-problems =
    { $count ->
        [one] One problem
       *[other] { $count } problems
    }
gantt-remove-dependency = Remove the dependency { $from } to { $to }
# The tooltip while dragging: the new dates.
gantt-drag-dates = { $from } to { $to }
gantt-drag-day = { $date }
# With "Also move dependents" on, how many tasks a drop may move later at most.
gantt-drag-dependents =
    { $count ->
        [one] May move one dependent task
       *[other] May move up to { $count } dependent tasks
    }
# With "Also move dependents" on, the tasks the engine says a drop would move.
gantt-drag-will-move =
    { $count ->
        [0] Moves no dependent tasks
        [one] Moves one dependent task
       *[other] Moves { $count } dependent tasks
    }
gantt-quarter = Q{ $quarter }

## Announcements and notes

gantt-moved-dates = { $key } now runs { $from } to { $to }
gantt-moved-day = { $key } now on { $date }
gantt-moved-with-dependents =
    { $count ->
        [one] { $key } and one dependent task moved
       *[other] { $key } and { $count } dependent tasks moved
    }
gantt-scheduled = { $key } scheduled on { $date }
gantt-link-start = Dependency from { $key }: go to the blocked task and press Enter. Escape cancels.
gantt-link-made = { $from } now blocks { $to }
gantt-link-removed = { $from } no longer blocks { $to }
gantt-link-cancelled = No dependency made
gantt-zoomed =
    { $zoom ->
        [day] Zoom: days
        [week] Zoom: weeks
        [month] Zoom: months
       *[quarter] Zoom: quarters
    }
gantt-error = The schedule couldn't be read: { $message }
gantt-empty = No tasks with dates or without them match.
gantt-status =
    { $scheduled ->
        [one] One scheduled
       *[other] { $scheduled } scheduled
    }, { $unscheduled ->
        [one] one unscheduled
       *[other] { $unscheduled } unscheduled
    }
gantt-status-overdue =
    { $count ->
        [one] One overdue
       *[other] { $count } overdue
    }
gantt-touch-read-only = Read only on touch screens: change dates in the task.
gantt-read-only =
    { $reason ->
       *[archived_project] { $key } can't be changed: its project is archived.
    }

## Export

# The heading of a printed chart or a picture of it.
gantt-export-title = { $project }: { $range }
# The suggested file name of a picture of the chart; keep the extension .png.
gantt-export-file = { $project } Gantt chart { $date }.png
gantt-exported = Saved the chart as { $file }
gantt-export-failed = The picture of the chart couldn't be made; it may be too large. Try a coarser zoom.
