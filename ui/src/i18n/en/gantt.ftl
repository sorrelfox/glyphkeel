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

## Table

gantt-label = Gantt chart
gantt-col-key = Key
gantt-col-title = Title
gantt-col-start = Start
gantt-col-due = Due
gantt-col-progress = %
gantt-col-progress-label = Progress
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
gantt-drag-unscheduled = Drag { $key } onto the timeline to schedule it

## Timeline notes

gantt-overdue-by =
    { $days ->
        [one] Overdue by one day
       *[other] Overdue by { $days } days
    }
gantt-no-deadline = No deadline
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
    }{ $overdue ->
        [0] {""}
        [one] , one overdue
       *[other] , { $overdue } overdue
    }
