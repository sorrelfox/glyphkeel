# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The task list: its query field, conditions, saved views, table, bulk bar
# and export. The messages about mistakes in a query come from the engine's
# own message file.

## Query field and conditions

list-query-label = Filter query
list-query-placeholder = Filter, for example: is:open due<+7d label:bug sort:due
list-conditions = Conditions
list-add-condition = Add condition
list-advanced = Advanced query · edit as text
list-chip-remove = Remove condition: { $condition }
list-chip-text = Text contains { $values }
list-chip-was = { $field } was { $values }
list-chip-was-not = { $field } was never { $values }
list-chip-changed =
    { $op ->
        [lt] { $field } changed before { $values }
        [le] { $field } changed up to { $values }
        [gt] { $field } changed after { $values }
        [ge] { $field } changed from { $values }
       *[is] { $field } changed on { $values }
    }
list-chip-changed-not =
    { $op ->
        [lt] { $field } didn't change before { $values }
        [le] { $field } didn't change up to { $values }
        [gt] { $field } didn't change after { $values }
        [ge] { $field } didn't change from { $values }
       *[is] { $field } didn't change on { $values }
    }
list-chip-changed-any = Changed: { $values }
list-chip-changed-any-not = Not changed: { $values }
list-chip-changed-by = Changed by { $values }
list-chip-changed-by-not = Not changed by { $values }
list-chip-is = { $field } is { $values }
list-chip-is-not = { $field } is not { $values }
list-chip-lt = { $field } before { $values }
list-chip-le = { $field } up to { $values }
list-chip-gt = { $field } after { $values }
list-chip-ge = { $field } from { $values }
list-chip-not-lt = { $field } not before { $values }
list-chip-not-le = { $field } not up to { $values }
list-chip-not-gt = { $field } not after { $values }
list-chip-not-ge = { $field } not from { $values }

list-field-key = Key
list-field-project = Project
list-field-title = Title
list-field-text = Text
list-field-description = Description
list-field-comment = Comment
list-field-status = Status
list-field-category = Category
list-field-type = Type
list-field-priority = Priority
list-field-label = Label
list-field-parent = Parent
list-field-start = Start
list-field-due = Due
list-field-created = Created
list-field-updated = Updated
list-field-closed = Closed
list-field-estimate = Estimate
list-field-spent = Spent
list-field-remaining = Remaining
list-field-progress = Progress
list-field-is = Task

## Filter banner and status bar

list-filtered = Filtered: { $count } of { $total ->
    [one] { $total } task
   *[other] { $total } tasks
  }
list-status-filtered = { $count } of { $total ->
    [one] { $total } task
   *[other] { $total } tasks
  }
list-clear-filter = Clear filter
list-include-subprojects = Include sub-projects
list-empty = No tasks here yet.
list-empty-filtered = No task matches this filter.

## Saved views

list-views = Saved views
list-view-button = Saved view: { $name }. Change view
list-view-none = All tasks
list-view-default-name = { $name } (default)
list-views-project = This project
list-views-workspace = All projects
list-view-changed = Changed since last save
list-view-save = Save
list-view-save-as = Save as…
list-view-rename = Rename…
list-view-make-default = Make default
list-view-undefault = Stop being the default
list-view-delete = Delete view
list-view-save-title = Save view
list-view-rename-title = Rename view
list-view-save-confirm = Save view
list-view-rename-confirm = Rename
list-view-name = Name
list-view-for = For
list-view-for-project = This project
list-view-for-workspace = All projects
list-view-default = Show it first when the list opens
list-view-created = View “{ $name }” saved
list-view-saved = View “{ $name }” changed
list-view-renamed = View renamed to “{ $name }”
list-view-deleted = View “{ $name }” deleted

## Table

list-table-label = Tasks
list-columns = Columns
list-column-key = Key
list-column-title = Title
list-column-project = Project
list-column-type = Type
list-column-status = Status
list-column-priority = Priority
list-column-labels = Labels
list-column-version = Version
list-column-start = Start
list-column-due = Due
list-column-estimate = Estimate
list-column-spent = Spent
list-column-remaining = Remaining
list-column-progress = Progress
list-column-created = Created
list-column-updated = Updated
list-sort-by = Sort by { $column }
list-sorted-ascending = Sort key { $position }, ascending. Shift-click adds a sort key.
list-sorted-descending = Sort key { $position }, descending. Shift-click adds a sort key.
list-sort-position = { $position }
list-resize-column = Width of { $column }
list-select-all = Select all
list-select-task = Select { $key }
list-blocked = Blocked
list-none = None
list-totals = { $count ->
    [one] { $count } task
   *[other] { $count } tasks
  }, { $open } open
list-edit-title = Title of { $key }
list-edit-status = Status of { $key }: { $value }. Change
list-edit-priority = Priority of { $key }: { $value }. Change
list-edit-due = Due date of { $key }
list-edit-due-value = Due date of { $key }: { $value }. Change

## Groups

list-group = Group
list-group-button = Group by: { $grouping }. Change
list-group-by-none = None
list-group-by-status = Status
list-group-by-priority = Priority
list-group-by-label = Label
list-group-by-parent = Parent
list-group-by-due_week = Due week
list-group-by-project = Project
list-group-none-label = No label
list-group-none-parent = No parent
list-group-none-due_week = No due date
# A parent task or a project: its key, then its title or name.
list-group-titled = { $key } · { $title }
# $day is the week's first day, formatted for the locale.
list-group-week = Week of { $day }
list-group-count = { $count }

## Selection and bulk changes

list-selected = { $count } selected
list-bulk-label = Change the selected tasks
list-bulk-status = Status…
list-bulk-priority = Priority…
list-bulk-add-label = Add label…
list-bulk-remove-label = Remove label…
list-bulk-move = Move to project…
list-bulk-clear = Clear selection
list-bulk-summary-status = Status: { $value }
list-bulk-summary-priority = Priority: { $value }
list-bulk-summary-add-label = Add label { $value }
list-bulk-summary-remove-label = Remove label { $value }
list-bulk-preview = { $change } · { $count ->
    [one] { $count } of { $total } task changes
   *[other] { $count } of { $total } tasks change
  }
list-bulk-apply = { $count ->
    [one] Change { $count } task
   *[other] Change { $count } tasks
  }
list-bulk-done = { $count ->
    [one] { $count } task changed
   *[other] { $count } tasks changed
  }

## Export

list-export = Export
list-export-csv = Export as CSV
list-export-json = Export as JSON
list-exported = { $count ->
    [one] { $count } task exported
   *[other] { $count } tasks exported
  }

# The line at the end of each group of the list that adds a task with the
# group's value (its status, priority, label, parent, due week or project).
list-group-add = Add task
list-group-add-to = Add a task to { $group }
list-group-add-title = Title of the new task
list-group-add-title-in = Title of the new task in { $group }
list-group-added = { $key } added to { $group }
