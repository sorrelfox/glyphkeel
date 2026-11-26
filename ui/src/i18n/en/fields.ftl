# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Custom fields: their values in the task panel and the list, and the
# Fields page of a project's settings. Field names and options are the
# person's own words and are never translated.

## Values

field-yes = Yes
field-no = No
field-not-set = Not set
fields-title = Fields
field-edit = Change { $field }
field-clear = Clear { $field }
field-required-note = { $field } is required
field-choose = Choose an option
field-open-link = Open { $link }

## Kinds

field-kind-text = Text
field-kind-markdown = Formatted text
field-kind-integer = Whole number
field-kind-decimal = Decimal number
field-kind-date = Date
field-kind-boolean = Yes or no
field-kind-select = One option
field-kind-multi_select = Several options
field-kind-url = Link

## The Fields page of project settings

project-settings-fields = Fields
fields-none = No custom fields yet. Add one to give tasks a value of your own, such as story points or a customer.
field-name = Field name
field-kind = Kind
field-scope = Projects
field-scope-project = This project
field-scope-all = All projects
field-shared-note = This field belongs to every project; changes here apply everywhere.
field-is-required = Required
field-options = Options
field-option-name = Option
field-option-add = Add an option
field-option-delete = Remove the option { $option }
field-types = Shown for
field-types-all = All task types
field-default = Default
field-default-none = No default
field-move-up = Move { $field } up
field-move-down = Move { $field } down
field-delete = Delete { $field }
field-delete-confirm = { $count ->
        [0] Delete { $field }
        [one] Delete { $field } and its value on one task
       *[other] Delete { $field } and its values on { $count } tasks
    }
field-add = Add a field
field-add-name = New field name
field-add-kind = Kind of the new field
field-add-all-projects = For all projects
field-changed = Field changed
field-query-name = In queries: { $name }
