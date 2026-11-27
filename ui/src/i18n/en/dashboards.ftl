# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Dashboards: named pages of widgets, for the whole workspace or pinned to a
## project. Each widget shows a count, a list, a chart from the reports, a
## note or the latest changes.

view-dashboards = Dashboards
action-dashboards = Dashboards
activity-entity-dashboard = Dashboard

dashboards-choose = Dashboard
# A dashboard pinned to a project, in the choice of dashboards: its name and the project's key.
dashboards-option-project = { $name } ({ $project })
dashboards-empty = No dashboards yet. A starter dashboard shows the open, overdue and soon due tasks, open tasks by status, tasks created against closed and the latest changes.
dashboards-create-starter = Create a starter dashboard
dashboards-create-empty = Create an empty one
# The name the starter dashboard gets.
dashboards-starter-name = Overview
dashboards-new = New dashboard
dashboards-new-title = New dashboard
dashboards-edit-title = Change "{ $name }"
dashboards-field-name = Name
dashboards-field-description = Description
dashboards-field-project = Pinned to
dashboards-whole-workspace = The whole workspace
dashboards-field-starter = Start with the starter widgets
dashboards-create = Create
dashboards-save = Save
dashboards-created = Dashboard "{ $name }" created.
dashboards-changed = Dashboard "{ $name }" changed.
dashboards-deleted = Dashboard "{ $name }" deleted.
dashboards-menu = Dashboard menu
dashboards-rename = Rename or describe…
# In the dashboard menu, when the page was opened from a project: pins the dashboard shown to it.
dashboards-pin-here = Pin to { $project }
dashboards-pinned = Dashboard "{ $name }" pinned to { $project }.
dashboards-delete = Delete dashboard
dashboards-delete-confirm = Delete the dashboard "{ $name }"? What its widgets show stays as it is; Undo brings the dashboard back.
dashboards-no-widgets = This dashboard has no widgets yet.
dashboards-add-widget = Add widget
dashboards-grid = Widgets

## A widget's frame: its title, its menu, moving and resizing it

widget-menu = Menu of { $title }
widget-edit = Edit…
widget-wider = Make wider
widget-narrower = Make narrower
widget-earlier = Move earlier
widget-later = Move later
widget-remove = Remove
widget-added = Widget "{ $title }" added.
widget-changed = Widget "{ $title }" changed.
widget-removed = Widget "{ $title }" removed.
# Read out after a widget moved.
widget-moved = "{ $title }" is now at place { $place } of { $count }.
widget-move-handle = Move { $title }
widget-move-keys = Drag to move it, or press the arrow keys.
# Read out while a widget is dragged with the pointer.
widget-drag-start = Moving "{ $title }".
widget-drag-over = Over place { $place } of { $count }.
widget-drag-end = "{ $title }" dropped at place { $place } of { $count }.
widget-drag-cancel = Moving "{ $title }" was cancelled.
widget-width = Width of { $title }
widget-width-value = { $width ->
        [one] One column
       *[other] { $width } columns
    }
widget-open-list = Open as a list
widget-as-table = Table
widget-error = This widget can't be shown: { $message }

## What widgets show

widget-tasks = { $count ->
        [one] 1 task
       *[other] { $count } tasks
    }
# Under a list cut short: how many more tasks match.
widget-more = { $count ->
        [one] and 1 more
       *[other] and { $count } more
    }
widget-no-tasks = No tasks.
widget-due = Due { $date }
# A bar of the tasks without a value for the field.
widget-no-value = None
widget-time-total = { $total } logged, { $period }
widget-no-time = No time logged in this period.
widget-no-versions = No open versions.
widget-version-progress = { $closed } of { $total } closed ({ $percent })
widget-version-label = Progress of { $name }
widget-no-activity = No changes.
widget-no-failures = No failed runs.
# A failed automation run: the rule's name and why.
widget-run = { $rule }: { $message }

## The kinds of widget: their own titles and what they show

widget-kind-query_count = Matching tasks
widget-kind-query_list = Task list
widget-kind-tasks_by_field = Tasks by field
widget-kind-due_soon = Due soon
widget-kind-overdue = Overdue
widget-kind-created_closed = Created and closed
widget-kind-cumulative_flow = Cumulative flow
widget-kind-cycle_time = Cycle time
widget-kind-time_report = Time logged
widget-kind-burndown = Sprint burndown
widget-kind-version_progress = Version progress
widget-kind-activity = Recent activity
widget-kind-automation_failures = Automation failures
widget-kind-note = Note
# The title of a widget that counts the open tasks (query is:open), as the starter dashboard has.
widget-open-tasks = Open tasks
# The titles of a widget that counts tasks by a field; a custom field by its name.
widget-tasks-by = Tasks by { $field }
widget-tasks-by-status = Tasks by status
widget-tasks-by-priority = Tasks by priority
widget-tasks-by-type = Tasks by type
widget-tasks-by-label = Tasks by label
# The fields tasks can be counted by, in the widget's settings.
widget-count-by-status = Status
widget-count-by-priority = Priority
widget-count-by-type = Type
widget-count-by-label = Label
widget-help-query_count = How many tasks match a query.
widget-help-query_list = The first tasks matching a query.
widget-help-tasks_by_field = Open tasks counted by status, priority, type, label or a custom field, as bars. A query narrows them; to count other tasks, choose their state in it, such as is:closed, category:done or status:Done.
widget-help-due_soon = Open tasks due in the next days.
widget-help-overdue = Open tasks past their due date.
widget-help-created_closed = Tasks created against tasks closed, per period.
widget-help-cumulative_flow = The tasks in each status, per period.
widget-help-cycle_time = How long tasks took from start to close.
widget-help-time_report = The time logged in the last days.
widget-help-burndown = The work still open in a project's active sprint.
widget-help-version_progress = How far a project's open versions have come.
widget-help-activity = The latest changes.
widget-help-automation_failures = Automation rules' runs that failed.
widget-help-note = A note in Markdown.

## Adding and editing a widget

widget-new-title = Add a widget
widget-edit-title = Edit "{ $title }"
widget-field-kind = Shows
widget-field-title = Title
widget-field-query = Query
# What an empty query field shows: an example query, in the query language (not translated).
widget-query-placeholder = is:open label:bug
# What the empty fields show: the default number of days and of rows.
widget-days-placeholder = 7
widget-limit-placeholder = 10
widget-field-project = Project
widget-any-project = Every project
widget-dashboard-project = The dashboard's project
widget-field-field = Count by
widget-field-limit = Rows
widget-field-days = Days
widget-field-text = Note
widget-field-width = Width
widget-add = Add
widget-save = Save

## The project menu in the header: the dashboards pinned to the project

project-dashboards = Dashboards
# Opens the dashboards page from a project, to pin a dashboard to it, when none is pinned yet.
project-dashboards-pin = Pin a dashboard here…
project-dashboards-all = All dashboards…
