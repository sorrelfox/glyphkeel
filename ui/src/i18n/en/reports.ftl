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
