# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

# Status rules and rules for agents: the Rules tab of the project settings,
# and what the board, the list and the task panel say about them.

## Project settings › Rules

project-settings-rules = Rules
rules-intro = A status's rules apply to every task that enters it, whoever moves it: you, the command line or an agent.
# The accessible name of one status's rules.
rules-status-label = Rules of { $name }
rules-count = { $count ->
        [0] No rules
        [one] One rule
       *[other] { $count } rules
    }
rules-on-entry = When a task enters this status
rules-start_date-today = Set the start date to today, if it has none
rules-due_date-today = Set the due date to today, if it has none
rules-progress = Progress on entering
rules-progress-keep = Leave the progress as it is
# $value is a percentage, already formatted: "100 %".
rules-progress-to = Set the progress to { $value }
rules-priority = Priority on entering
rules-priority-keep = Leave the priority as it is
# $value is the name of a priority: "High".
rules-priority-to = Set the priority to { $value }
rules-stop-timer = Stop the timer and log its time
rules-release-claim = Release an agent's claim on the task
# $name is the name of a custom field.
rules-custom-set = { $name } on entering
rules-custom-keep = Leave { $name } as it is
# $value is an option of the field, or Yes or No.
rules-custom-set-to = Set { $name } to { $value }
rules-custom-today = Set { $name } to today, if it has none
rules-clear = Clear on entering
rules-required = Must be filled in before a task can enter
rules-read-only = Can't change while a task is in this status
rules-agents-may-not-enter = Agents may not move tasks into this status
rules-agents-heading = Agents
rules-agents-intro = Agents are held to these rules as well as to the status rules. You are not.
rules-agents-read-only = Fields agents may not change
toast-rules-changed = Rules changed

## Task fields, as rules name them

rule-field-title = Title
rule-field-description = Description
rule-field-type = Type
rule-field-priority = Priority
rule-field-start_date = Start date
rule-field-due_date = Due date
rule-field-estimate = Estimate
rule-field-progress = Progress
rule-field-labels = Labels
rule-field-parent = Parent task
rule-field-fix_version = Version
rule-field-custom = A custom field

## The board, the list and the task panel

# $fields lists the missing fields, already joined for the language: "Estimate and Due date".
rule-missing = Fill in first: { $fields }
# On a lane during a drag, when the dragged card can't go there.
lane-rule-closed = { $name }: { $reason }
# Under the task panel's fields; $fields is already joined for the language.
panel-rule-read-only = { $count ->
        [one] In { $status }, this can't change: { $fields }
       *[other] In { $status }, these can't change: { $fields }
    }

## History

history-field-rules = Status rules
history-field-agent_rules = Rules for agents
