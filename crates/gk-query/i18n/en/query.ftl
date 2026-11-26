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
query-operator = "{ $op }" doesn't apply to { $field }. Use { $usage }.
query-no-range = { $field } takes no ranges. Use { $usage }.
query-range-operator = A range works only with ":", such as { $field }:1h..4h.
query-none-operator = "none" works only with ":" and "!=".
query-equals-text = "=" doesn't apply to { $field }. Use { $field }:word.

## Values

query-bad-date = Dates are written 2026-12-31, or as today, +7d, this-week.
query-bad-duration = "{ $value }" is not a duration. Write it as 90m, 1.5h, 1h30m or 2d.
query-bad-number = "{ $value }" is not a whole number.
query-bad-percent = "{ $value }" is not a percentage from 0 to 100.
query-bad-key = "{ $value }" is not a task key such as TM-41.
query-bad-project = "{ $value }" is not a project key such as TM, or TM/* with its sub-projects.
query-bad-choice = "{ $value }" is not a value of { $field }. Use one of: { $choices }.
query-bad-choice-suggest = "{ $value }" is not a value of { $field }. Did you mean "{ $suggestion }"?
query-during-none = during: needs dates. For tasks without dates, use is:unscheduled.
query-empty-value = { $field } needs a value that is not empty.

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
query-unknown-project = No project "{ $value }". Projects: { $choices }.
query-unknown-status-none = No status "{ $value }": the workspace has no statuses yet.
query-unknown-type-none = No type "{ $value }": the workspace has no types yet.
query-unknown-label-none = No label "{ $value }": the workspace has no labels yet.
query-unknown-version-none = No version "{ $value }": the workspace has no versions yet.
query-unknown-project-none = No project "{ $value }": the workspace has no projects yet.
query-unknown-task = There is no task { $value }.
