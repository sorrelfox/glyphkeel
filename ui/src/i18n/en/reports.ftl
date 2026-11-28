# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## The flow reports: tasks created against closed, the cumulative flow and
## cycle time, read from the history.

view-reports = Reports
action-reports = Reports

flow-tabs = Report
flow-tab-created-closed = Created and closed
flow-tab-cumulative = Cumulative flow
flow-tab-cycle = Cycle time

## The controls

flow-bucket = Period length
flow-bucket-day = Days
flow-bucket-week = Weeks
flow-bucket-month = Months
flow-last-30-days = Last 30 days
flow-last-12-weeks = Last 12 weeks
flow-last-12-months = Last 12 months
flow-type = Type
flow-any-type = Any type
flow-label = Label
flow-any-label = Any label
flow-version = Version
flow-any-version = Any version
flow-query = Query
flow-query-placeholder = Narrow with a query, then press Enter
flow-actor = Done by
flow-actor-anyone = Anyone
flow-actor-people = People
flow-actor-agents = Agents
flow-group-by = Bands
flow-by-status = By status
flow-by-category = By category
flow-metric = Time shown
flow-as-table = Show as a table
flow-export = Export as CSV
flow-exported = Report exported
# Names of saved files, without their extension; { $from } and { $to } are days such as 2026-11-01.
flow-file-created-closed = created-closed-{ $from }-{ $to }
flow-file-cumulative = cumulative-flow-{ $from }-{ $to }
flow-file-cycle = cycle-time-{ $from }-{ $to }

## Categories of statuses, as bands of the cumulative flow

flow-category-todo = To do
flow-category-in-progress = In progress
flow-category-done = Done
flow-category-cancelled = Cancelled

## Series, columns and figures

flow-period = Period
flow-created = Created
flow-closed = Closed
flow-reopened = Reopened
flow-open = Open at the end
flow-total = Total
flow-cycle-time = Cycle time
flow-lead-time = Lead time
flow-cycle-help = From the first move into progress to the last close.
flow-lead-help = From creation to the last close.
flow-stat-count = Tasks
flow-stat-mean = Mean
flow-stat-median = Median
flow-stat-p85 = 85th percentile
flow-stat-p95 = 95th percentile
flow-stat-min = Shortest
flow-stat-max = Longest
flow-task = Task
flow-closed-on = Closed
flow-not-started = Never in progress
flow-no-value = –
flow-tasks-shown = { $total ->
    [one] The one task closed in these days.
   *[other] The latest { $shown } of { $total } tasks closed in these days; the export has all of them.
}
flow-none = No tasks in these days.
flow-none-closed = No tasks closed in these days.
flow-before = { $count ->
    [one] One task was open before the first day.
   *[other] { $count } tasks were open before the first day.
}

## The charts and what they say to screen readers

flow-chart-created-closed = Tasks created and closed, from { $from } to { $to }
flow-chart-cumulative = Tasks in each status at the end of each period, from { $from } to { $to }
flow-chart-cycle = Time each task took until it closed, from { $from } to { $to }
flow-chart-keys = Use the left and right arrow keys to read the periods one by one; Home and End go to the first and last.
flow-summary-created-closed = { $created ->
    [one] One task created
   *[other] { $created } tasks created
}, { $closed ->
    [one] one close
   *[other] { $closed } closes
}, { $open ->
    [one] one task open at the end.
   *[other] { $open } tasks open at the end.
}
flow-summary-cumulative = { $count ->
    [one] One task at the end of the last period.
   *[other] { $count } tasks at the end of the last period.
}
flow-summary-cycle = { $count ->
    [0] No tasks closed.
    [one] One task closed; median cycle time { $median }.
   *[other] { $count } tasks closed; median cycle time { $median }.
}
flow-point-created-closed = { $period }: { $created } created, { $closed } closed, { $reopened } reopened, { $open } open
flow-point-cumulative = { $period }: { $count ->
    [one] one task
   *[other] { $count } tasks
}
flow-point-band = { $name }: { $count }
flow-point-cycle = { $period }: { $count ->
    [0] no tasks closed
    [one] one task closed, cycle time { $cycle }, lead time { $lead }
   *[other] { $count } tasks closed, median cycle time { $cycle }, median lead time { $lead }
}
flow-line-median = Median
flow-line-p85 = 85th percentile

## The planning reports: sprint velocity, burn-up and estimate accuracy

plan-tab-velocity = Velocity
plan-tab-burnup = Burn-up
plan-tab-estimates = Estimate accuracy
plan-measure = Measure
plan-measure-tasks = Tasks
plan-measure-estimate = Estimated hours
plan-measure-field = Number field
plan-field = Field
plan-target = Version or sprint
# $name is a version's name.
plan-target-version = Version { $name }
# $name is a sprint's name.
plan-target-sprint = Sprint { $name }
# Days counted back from today, a quick choice of range.
plan-last-days = Last { $days } days
# Names of saved files, without their extension.
plan-file-velocity = velocity-{ $project }
plan-file-burnup = burn-up-{ $name }
plan-file-estimates = estimates-{ $from }-{ $to }

plan-sprint = Sprint
plan-committed = Committed
plan-completed = Completed
plan-average = Average of the last 3
plan-tasks-done = Tasks completed
# $done of $total tasks, such as "4 of 6".
plan-of = { $done } of { $total }
plan-scope = Scope
plan-forecast-line = Forecast
# $date is a formatted day.
plan-forecast = At this pace: { $date }
plan-velocity-none = No completed sprints yet.
plan-burnup-nothing = This project has no versions and no started sprints.
plan-burnup-empty = No days to show yet.
# $sprint is the next sprint's name; $planned and $average are amounts of work, formatted.
plan-hint = Planned for { $sprint }: { $planned } · average velocity: { $average }

plan-chart-velocity = Work committed and completed per sprint of { $project }
plan-chart-burnup = Scope and completed work of { $name }, day by day
plan-chart-estimates = Closed tasks by time spent against their estimate, from { $from } to { $to }
plan-summary-velocity = Average velocity { $average }, over { $count ->
    [one] the last sprint.
   *[other] the last { $count } sprints.
}
plan-summary-burnup = { $completed } of { $scope } completed.
plan-summary-burnup-forecast = { $completed } of { $scope } completed; at this pace, done on { $date }.
plan-summary-estimates = { $count ->
    [one] One task compared
   *[other] { $count } tasks compared
}: median ratio { $median }, { $within } within 20%.
plan-point-velocity = { $sprint }: { $committed } committed, { $completed } completed, average { $average }
plan-point-burnup = { $date }: { $completed } of { $scope } completed
plan-point-burnup-ahead = { $date }: still to come
plan-point-bin = { $range }: { $count ->
    [one] one task
   *[other] { $count } tasks
}

## Estimate accuracy

# A ratio of time spent to time estimated, such as 1.4×.
plan-ratio = { $ratio }×
plan-ratio-column = Spent ÷ estimate
plan-ratio-range = Time spent against the estimate
plan-bin-1 = Half or less
plan-bin-2 = Under by more than 20%
plan-bin-3 = Within 20%
plan-bin-4 = Over by 20 to 50%
plan-bin-5 = Over by 50 to 100%
plan-bin-6 = More than twice
plan-on-target = Within 20%
plan-off-target = Further off
plan-stat-tasks = Tasks
plan-stat-median = Median ratio
plan-stat-within = Within 20%
plan-stat-over = Over by more
plan-stat-under = Under by more
plan-stat-estimated = Estimated
plan-stat-spent = Spent
plan-outliers = Furthest off
plan-type = Type
plan-estimates-none = No closed tasks with an estimate and logged time in these days.
plan-excluded = Left out: { $estimate ->
    [one] one task without an estimate
   *[other] { $estimate } tasks without an estimate
}, { $time ->
    [one] one without logged time.
   *[other] { $time } without logged time.
}
# The grouping, and the column of the groups' names.
plan-group-none = No grouping
plan-group-type = Type
plan-group-label = Label
plan-group-project = Project
plan-group-work_kind = Kind of work
plan-group-month = Month closed
# The group of tasks that have no value.
plan-group-none-none = All tasks
plan-group-none-type = No type
plan-group-none-label = No label
plan-group-none-project = No project
plan-group-none-work_kind = No kind of work
plan-group-none-month = No month
# $by is the grouping, such as "Type".
plan-groups = Estimate accuracy by { $by }

## The hint in the task panel's estimate field, when switched on in the preferences.
## $type is a task type's name, $ratio a formatted ratio (such as 1.4×), $count the closed tasks it comes from.

estimate-hint-over = Your { $type } estimates usually run { $ratio } over ({ $count } closed tasks).
estimate-hint-under = Your { $type } tasks usually take { $ratio } of their estimate ({ $count } closed tasks).
