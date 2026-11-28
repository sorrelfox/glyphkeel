# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Messages about task queries, in English. The engine renders them in English
# for the command line and agents; the app translates them by their ID with
# the same arguments. One message per line; placeables are { $name }.

## Reading the text

query-too-long = The query is longer than { $max } characters.
query-too-deep = The query is nested too deeply.
query-unclosed-quote = The quote that starts here is never closed.
query-unclosed-paren = The "(" here has no matching ")".
query-unexpected-paren = There is no "(" for this ")".
query-empty-group = There is nothing inside the parentheses.
query-condition-after = "{ $keyword }" needs a condition after it.
query-condition-before = "{ $keyword }" needs a condition before it.
query-nothing-to-negate = "-" needs a condition right after it, such as -label:bug.
query-missing-value = "{ $field }{ $op }" needs a value.
query-unexpected-quote = A quote can only start a value or a phrase; put a space before it.
query-text-after-quote = Put a comma or a space after the closing quote.
query-empty-range-end = A range needs both ends, such as 1h..4h.

## Fields and operators

query-unknown-field = Unknown field "{ $field }". Did you mean "{ $suggestion }"?
query-unknown-field-plain = Unknown field "{ $field }". To search for text with a colon, put it in quotes.
query-unknown-custom-field = No custom field "{ $field }". Did you mean "{ $suggestion }"?
query-unknown-custom-field-plain = No custom field "{ $field }". Custom fields: { $choices }.
query-no-custom-fields = No custom field "{ $field }": the workspace has no custom fields yet.
query-operator = "{ $op }" doesn't apply to { $field }. Use { $usage }.
query-no-range = { $field } takes no ranges. Use { $usage }.
query-range-operator = A range works only with ":", such as { $field }:1h..4h.
query-none-operator = "none" works only with ":" and "!=".
query-equals-text = "=" doesn't apply to { $field }. Use { $field }:word.

## Values

query-bad-date = Dates are written 2026-12-31, or as today, +7d, this-week.
query-bad-duration = "{ $value }" is not a duration. Write it as 90m, 1.5h, 1h30m or 2d.
query-bad-number = "{ $value }" is not a whole number.
query-bad-days = "{ $value }" is not a number of days, such as 3d or -1d.
query-bad-decimal = "{ $value }" is not a number: write it with a point, for example 2.5.
query-bad-percent = "{ $value }" is not a percentage from 0 to 100.
query-bad-key = "{ $value }" is not a task key such as TM-41.
query-bad-project = "{ $value }" is not a project key such as TM, or TM/* with its sub-projects.
query-bad-choice = "{ $value }" is not a value of { $field }. Use one of: { $choices }.
query-bad-choice-suggest = "{ $value }" is not a value of { $field }. Did you mean "{ $suggestion }"?
query-was-value = was: needs a field and a value, such as was:status:Review.
query-history-none = "none" can't be looked up in the history. For tasks without a { $field } now, use { $field }:none.
query-history-field = { $history }: doesn't look up "{ $field }". Use one of: { $choices }.
query-history-field-suggest = { $history }: doesn't look up "{ $field }". Did you mean "{ $suggestion }"?
query-during-none = during: needs dates. For tasks without dates, use is:unscheduled.
query-empty-value = { $field } needs a value that is not empty.
query-in-files-or = in:files says where all the query's words are looked for, so it can't be one side of OR.

## Sorting

query-sort-placement = sort: can only appear once, outside parentheses.
query-sort-empty = sort: needs a field, such as sort:due.
query-sort-unknown = Can't sort by "{ $field }". Did you mean "{ $suggestion }"?
query-sort-unknown-plain = Can't sort by "{ $field }". Sortable fields: { $choices }.
query-sort-relevance = Sorting by relevance needs words to search for.
query-sort-negated = sort: can't be negated.

## Names in the workspace

query-unknown-status = No status "{ $value }". Statuses: { $choices }.
query-unknown-type = No type "{ $value }". Types: { $choices }.
query-unknown-label = No label "{ $value }". Labels: { $choices }.
query-unknown-version = No version "{ $value }". Versions: { $choices }.
query-unknown-sprint = No sprint "{ $value }". Sprints: { $choices }.
query-unknown-project = No project "{ $value }". Projects: { $choices }.
query-unknown-status-none = No status "{ $value }": the workspace has no statuses yet.
query-unknown-type-none = No type "{ $value }": the workspace has no types yet.
query-unknown-label-none = No label "{ $value }": the workspace has no labels yet.
query-unknown-version-none = No version "{ $value }": the workspace has no versions yet.
query-unknown-sprint-none = No sprint "{ $value }": the workspace has no sprints yet.
query-unknown-project-none = No project "{ $value }": the workspace has no projects yet.
query-unknown-task = There is no task { $value }.

## The values a field takes, as the query field's completion shows them
# Placeables hold the query's own words and examples, which stay as they are.

query-values-key = a task key such as { $example }
query-values-project = a project key; { $example } adds its sub-projects
query-values-title = words in the title; { $equals } the exact title
query-values-text = words in the title, description, comments or attached files
query-values-description = words in the description
query-values-comment = words in a comment
query-values-status = status names
query-values-category = { $categories }; { $groups }
query-values-type = task type names
query-values-priority = { $order }
query-values-label = label names, or { $none }
query-values-version = version names, or { $none }
query-values-sprint = sprint names, { $active }, or { $none }
query-values-parent = a task key, or { $none }
query-values-under = a task key: its subtasks at any depth
query-values-blocks = a task key, or { $any }
query-values-blocked-by = a task key, or { $any }
query-values-relates = a task key, or { $any }
query-values-duplicates = a task key, or { $any }
query-values-start = a date, { $today }, { $ahead }, { $week }, or { $none }
query-values-due = a date, { $today }, { $ahead }, { $week }, or { $none }
query-values-during = a date, { $week }, or a range such as { $range }: { $start } to { $due } overlaps it
query-values-created = a date, { $today }, { $back }, { $month }
query-values-edited = a date: the task's own fields changed
query-values-updated = a date: anything about the task changed
query-values-closed = a date, or { $none }
query-values-estimate = a duration such as { $examples }, or { $none }
query-values-spent = a duration
query-values-remaining = a duration: estimate minus logged time
query-values-progress = a percentage, { $plain } or { $percent }
query-values-slack = working days of slack on the critical path, such as { $example }; negative when late
query-values-slip = days later than in the newest baseline, or the one { $baseline } names, such as { $late } or { $early }
query-values-baseline = a baseline name of the task's project
query-values-author = { $me }, { $agent }, or an agent's name
query-values-claimed = an agent's name, or { $any }
query-values-comments = a number
query-values-files = a number
query-values-subtasks = a number
query-values-is = a flag
query-values-has = a part of the task
query-values-was = a field and a value the task had at any time: { $example }
query-values-changed = a field that changed; with dates, when: { $example }
query-values-changed-by = { $me }, { $agent }, or an agent's name: who changed the task's fields
query-values-in = where the words are looked for: { $only } only inside attached files, { $never } never there

## The values a custom field takes, by its kind

query-values-custom-text = words in the text; { $equals } the exact text; { $none }
query-values-custom-url = part of the link; { $equals } the exact link; { $none }
query-values-custom-integer = a whole number, a range such as { $range }, or { $none }
query-values-custom-decimal = a number such as { $example }, a range, or { $none }
query-values-custom-date = a date, { $today }, { $ahead }, { $week }, or { $none }
query-values-custom-boolean = { $yes } or { $no }
query-values-custom-select = option names, or { $none }
