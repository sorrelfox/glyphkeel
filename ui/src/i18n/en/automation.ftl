# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

# Automation rules: the Automation tab of the project settings, its rule
# editor, the dry run and the run log, and how history names a rule.

## Project settings › Automation

project-settings-automation = Automation
automation-intro = When something happens to a task that matches the condition, a rule makes changes of its own. Status rules apply to them, and history names the rule; Undo takes back your change with what the rules did after it.
automation-none = No rules yet.
automation-new = New rule
automation-list = Rules
automation-workspace = Whole workspace
# The accessible name of the switch that turns a rule on or off.
automation-enabled-label = Run { $name }
automation-edit = Edit
# The accessible names of a rule's buttons; $name is the rule's name.
automation-edit-label = Edit { $name }
automation-delete-label = Delete { $name }
automation-runs-label = Runs of { $name }
automation-runs-show = Runs
automation-runs-none = It hasn't run yet.
automation-held = Made by an agent: held to the rules for agents
# $time is a date and time, already formatted.
automation-last-run = Last run { $time }
automation-run-done = Done
automation-run-failed = Failed
automation-run-limited = Didn't run
automation-run-changes = { $count ->
        [0] No changes
        [one] One change
       *[other] { $count } changes
    }
toast-automation-saved = Rule saved
toast-automation-changed = Rule changed
toast-automation-deleted = Rule deleted

## What sets a rule off

automation-trigger-task_created = A task is created
automation-trigger-field_changed = A field changes
automation-trigger-moved_to_status = A task moves to a status
automation-trigger-comment_added = A comment is added
automation-trigger-due_date_reached = The due date comes
automation-trigger-overdue = The due date passes
automation-trigger-schedule = On a schedule
automation-when-any-field = A field changes
# $fields lists the fields, already joined for the language.
automation-when-fields = { $count ->
        [one] { $fields } changes
       *[other] { $fields } change
    }
automation-when-moved-any = A task moves to another status
automation-when-moved = A task moves to { $status }
# $time is a time of day, HH:MM.
automation-when-daily = Every day at { $time }
# $day is a day of the week, already in the language.
automation-when-weekly = Every { $day } at { $time }

## What a rule does

automation-action-set_fields = Set a field
automation-action-add_label = Add a label
automation-action-remove_label = Remove a label
automation-action-move_to_status = Move to a status
automation-action-add_comment = Add a comment
automation-action-create_subtask = Create a subtask
automation-action-create_linked_task = Create a linked task
automation-action-notify = Notify me

## Task fields, as rules name them

automation-field-title = Title
automation-field-description = Description
automation-field-status = Status
automation-field-type = Type
automation-field-priority = Priority
automation-field-labels = Labels
automation-field-start_date = Start date
automation-field-due_date = Due date
automation-field-estimate = Estimate
automation-field-progress = Progress
automation-field-fix_version = Version
automation-field-parent = Parent task

## The rule editor

automation-editor-new = New rule
automation-editor-edit = Edit rule
automation-name = Name
automation-scope = Applies to
# $project is a project's key.
automation-scope-project = This project ({ $project })
automation-scope-workspace = Every project
automation-enabled = Run this rule
automation-trigger = When
automation-trigger-fields = Which fields
automation-trigger-fields-any = Leave them all unticked for any field.
automation-trigger-status = To which status
automation-status-any = Any status
automation-every = How often
automation-every-daily = Every day
automation-every-weekly = Every week
automation-at-time = At
automation-weekday = On
automation-condition = Only for tasks that match
automation-condition-placeholder = Every task
automation-condition-hint = A query, as in the list: for example type:Bug priority:high. Leave it empty for every task; a scheduled rule needs one, such as is:open.
automation-actions = Then
automation-add-action = Add an action
automation-action-kind = Action
# $n counts the actions from 1.
automation-action-label = Action { $n }
automation-action-remove = Remove action { $n }
automation-action-up = Move action { $n } up
automation-action-down = Move action { $n } down
automation-set-field = Field
automation-value = Value
automation-set-clear = Clear it
automation-set-date-hint = A date, or today, +3d, next mon
automation-label = Label
automation-status = Status
automation-text = Text
automation-title = Title of the new task
automation-template = From the template
automation-template-none = No template
automation-link = Link
# $list lists the placeholders, already joined for the language.
automation-placeholders = Texts may use { $list }.
automation-save = Save rule
automation-cancel = Cancel

## The dry run

automation-test = Try it on a task
automation-test-task = Task key
automation-test-run = Try
automation-test-note = Nothing is changed.
automation-test-no-match = The task doesn't match the condition, so the rule wouldn't run.
automation-test-ok = The rule would make these changes:
automation-test-nothing = The rule would change nothing.
# $n counts the actions from 1.
automation-test-failed = Action { $n } would fail:
automation-test-failed-rule = The rule would fail:
# $field is a field's name, $task a task key; $before and $after are values.
automation-test-change = { $task } · { $field }: { $before } → { $after }
automation-test-created = { $task } · { $what } created
automation-test-notice = Notification: { $text }
automation-test-empty = (empty)

## History

activity-actor-automation = Rules
activity-entity-automation_rule = Automation rule
# The chip beside a rule's name where it made a change or wrote a comment.
comment-automation = Automation rule
