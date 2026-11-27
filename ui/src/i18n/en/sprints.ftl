# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Sprints: the sprints page, the task panel's sprint and the burndown

view-sprints = Sprints

sprint-state-planned = Planned
sprint-state-active = Active
sprint-state-closed = Completed

## The task panel's sprint

prop-sprint = Sprint
prop-sprint-value = Sprint: { $value }
# The sprint picker's name when the sprint runs or has ended: its name, then its state (Active, Completed).
prop-sprint-value-state = Sprint: { $value } { $state }
prop-sprint-active = { $name } (active)
prop-sprint-loading = Loading sprints…
prop-sprint-none-open = No planned or active sprints
prop-sprint-clear = Backlog
history-field-sprint = Sprint
history-sprint-gone = a deleted sprint
activity-entity-sprint = Sprint

## The board's and the list's sprint filters

filter-sprint = Sprint
filter-sprint-backlog = Backlog (no sprint)
list-field-sprint = Sprint
list-sprint = Sprint
list-sprint-button = Sprint: { $sprint }
list-sprint-active = Active sprint
list-sprint-backlog = Backlog
list-sprint-any = Any sprint

## The sprints page

sprints-title = Sprints of { $project }
sprints-new = New sprint
sprints-default-name = Sprint { $number }
sprints-show-closed = { $count ->
        [0] Show completed sprints
       *[other] Show completed sprints ({ $count })
    }
sprints-error = The sprints couldn't be read: { $message }
sprints-empty = No sprints yet. A sprint is a short, fixed stretch of time with a goal: plan one, put tasks from the backlog in it, and start it.
sprints-backlog = Backlog
sprints-backlog-count = { $count ->
        [0] No open tasks outside a sprint
        [one] One open task outside a sprint
       *[other] { $count } open tasks outside a sprint
    }
sprints-tasks-of = Tasks of { $name }
sprints-drop-here = No tasks. Drag tasks here, or choose some and use Move to.
sprints-chosen = { $count ->
        [one] One task chosen
       *[other] { $count } tasks chosen
    }
sprints-move-to = Move to
sprints-clear-choice = Clear the choice
sprints-moved = { $count ->
        [one] One task moved to { $name }.
       *[other] { $count } tasks moved to { $name }.
    }
sprints-moved-backlog = { $count ->
        [one] One task moved to the backlog.
       *[other] { $count } tasks moved to the backlog.
    }
sprints-starts = Starts { $date }
sprints-ends = Ends { $date }
sprints-no-dates = No dates
sprints-size = { $count ->
        [one] One task, { $estimate } estimated
       *[other] { $count } tasks, { $estimate } estimated
    }
sprints-progress-label = Progress of { $name }
sprints-progress = { $closed } of { $total } closed ({ $percent })
sprints-details = { $count ->
        [one] Burndown and one task
       *[other] Burndown and { $count } tasks
    }
sprints-menu = More for { $name }
sprints-edit = Edit
sprints-delete = Delete sprint
sprints-start = Start sprint
sprints-start-named = Start { $name }
sprints-start-blocked = Start { $name }: complete the active sprint first
sprints-complete = Complete sprint
sprints-complete-named = Complete { $name }

## Sprint dialogs

sprints-new-title = New sprint
sprints-edit-title = Edit { $name }
sprints-field-name = Name
sprints-field-goal = Goal
sprints-field-start = Start date
sprints-field-end = End date
sprints-create = Create sprint
sprints-save = Save
sprints-created = Sprint { $name } planned.
sprints-changed = Sprint { $name } changed.
sprints-start-title = Start { $name }
sprints-start-body = { $count ->
        [0] The sprint has no tasks yet; you can add them once it runs.
        [one] The sprint starts with one task.
       *[other] The sprint starts with { $count } tasks.
    }
sprints-started = Sprint { $name } started.
sprints-complete-title = Complete { $name }
sprints-complete-body = { $open ->
        [0] Every task in the sprint is closed ({ $closed }).
        [one] One task is still open; { $closed } closed.
       *[other] { $open } tasks are still open; { $closed } closed.
    }
sprints-complete-move = Move the open tasks to
sprints-completed = Sprint { $name } completed.
sprints-delete-title = Delete { $name }?
sprints-delete-body = { $count ->
        [0] The sprint has no tasks. Undo brings it back.
        [one] Its one task goes back to the backlog. Undo brings both back.
       *[other] Its { $count } tasks go back to the backlog. Undo brings them back.
    }
sprints-deleted = Sprint { $name } deleted.

## The burndown

burndown-heading = Burndown
burndown-title = Burndown of { $name }
burndown-unit = Measure
burndown-unit-tasks = Tasks
burndown-unit-estimate = Estimate
burndown-error = The burndown couldn't be read: { $message }
burndown-empty = Nothing to show yet.
burndown-summary-active = { $open } of { $total } still open; { $days ->
        [0] the sprint ends today or has ended.
        [one] one day to go.
       *[other] { $days } days to go.
    }
burndown-summary-closed = { $open } of { $total } were still open when the sprint was completed.
burndown-day = { $date }: { $open } of { $total } open; ideal { $ideal }.
burndown-day-ahead = { $date }: still to come; ideal { $ideal }.
burndown-legend-remaining = Still open
burndown-legend-ideal = Ideal
burndown-legend-scope = All work in the sprint
# The column of days in the burndown's table.
burndown-day-column = Day
