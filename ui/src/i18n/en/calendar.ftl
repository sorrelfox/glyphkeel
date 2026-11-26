# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The calendar: a project's tasks by start and due date, a month or a week
# at a time. Dates arrive formatted for the locale.

## Toolbar

calendar-today = Today
calendar-previous-month = Previous month
calendar-next-month = Next month
calendar-previous-week = Previous week
calendar-next-week = Next week
calendar-mode = Calendar range
calendar-mode-month = Month
calendar-mode-week = Week
calendar-query-label = Filter query
calendar-query-placeholder = Filter, for example: is:open label:bug
calendar-include-subprojects = Include sub-projects

## Grid

# The grid's accessible name; $range is the month or the week on screen.
calendar-grid-label = Calendar, { $range }
calendar-keys-hint = Arrow keys move between days, Page Up and Page Down between pages. Enter goes to the tasks of the day, and N adds a task due that day. On a task, Alt and an arrow key moves it to another day.
# A day's accessible name; $date is the full date.
calendar-day-label =
    { $count ->
        [0] { $date }, no tasks
        [one] { $date }, one task
       *[other] { $date }, { $count } tasks
    }
calendar-day-today = { $date }, today
# The tasks of a day that don't fit in the month.
calendar-more =
    { $count ->
       *[other] +{ $count } more
    }
calendar-more-label =
    { $count ->
        [one] One more task on { $date }: show the week
       *[other] { $count } more tasks on { $date }: show the week
    }
# A task's accessible name: one day, or the days it runs.
calendar-task-day = { $key } { $title }, { $date }
calendar-task-days = { $key } { $title }, { $from } to { $to }

## Quick add

# The "+" of a day, and the form it opens; $date is the full date.
calendar-add = Add a task on { $date }
calendar-add-title = New task due { $date }
calendar-add-placeholder = Task title
calendar-add-confirm = Add

## Notes

calendar-moved = { $key } moved to { $date }
calendar-added = { $key } added, due { $date }
calendar-truncated = Showing { $shown } of { $total } tasks of these days. Narrow the query to see the rest.
calendar-error = The calendar's tasks couldn't be read: { $message }
calendar-status =
    { $count ->
        [0] No tasks with dates on screen
        [one] One task on screen
       *[other] { $count } tasks on screen
    }
