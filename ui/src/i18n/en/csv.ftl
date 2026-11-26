# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Importing tasks from a CSV file

action-import-csv = Import tasks from a CSV file…
project-import-csv = Import from CSV…
csv-import-about = Add the tasks of a CSV file to a project, such as an export of Jira or Redmine. Nothing that is here already changes.
csv-title = Import from CSV
csv-title-into = Import from CSV into { $project }
csv-choose-about = Choose a CSV file. Its encoding, separator and columns are detected; you can change each before anything is imported.
csv-choose = Choose a CSV file…
csv-file-name = CSV file in the exports folder
csv-check = Check
csv-checking = Reading the file…
csv-importing = Importing…
csv-file = File: { $name }
csv-project = Project
csv-project-option = { $key } · { $name }
csv-preset = Preset
csv-preset-auto = Detect from the columns
csv-preset-detected = Detected: { $name }
csv-preset-jira = Jira
csv-preset-redmine = Redmine
csv-encoding = Encoding
csv-encoding-auto = Detect ({ $found })
csv-separator = Separator
csv-separator-auto = Detect ({ $found })
csv-separator-comma = Comma
csv-separator-semicolon = Semicolon
csv-separator-tab = Tab
csv-separator-pipe = Vertical bar
csv-date-order = Dates
csv-date-auto = Detect
csv-date-auto-found = Detect ({ $found })
csv-date-dmy = Day, month, year
csv-date-mdy = Month, day, year
csv-date-ymd = Year, month, day
csv-columns = Columns
csv-column = Column
csv-column-samples = Examples
csv-column-target = Goes to
csv-column-unit = Unit
csv-column-target-of = Where “{ $column }” goes
csv-column-unit-of = The unit of “{ $column }”
csv-target-ignore = Not imported
csv-target-title = Title
csv-target-description = Description
csv-target-status = Status
csv-target-type = Type
csv-target-priority = Priority
csv-target-labels = Labels
csv-target-fix_version = Version
csv-target-start_date = Start date
csv-target-due_date = Due date
csv-target-estimate = Estimate
csv-target-spent = Time spent
csv-target-comment = Comment
csv-target-key = Key in the other tool
csv-target-parent = Parent task
csv-target-blocks = Blocks
csv-target-blocked_by = Blocked by
csv-target-relates = Relates to
csv-target-duplicates = Duplicates
csv-target-duplicated_by = Duplicated by
csv-target-clones = Clones
csv-target-cloned_by = Cloned by
csv-target-relations = Links, as Redmine writes them
csv-target-field = Field: { $name }
csv-unit-seconds = Seconds
csv-unit-minutes = Minutes
csv-unit-hours = Hours
csv-unit-days = Days
csv-unit-text = Written (1h 30m)
csv-create-missing = Create missing statuses, types and labels
csv-skip-invalid = Leave out rows with errors
csv-summary = What the import makes
csv-summary-rows = { $rows ->
        [one] One row
       *[other] { $rows } rows
    }
csv-summary-created = { $count ->
        [one] One new task
       *[other] { $count } new tasks
    }
csv-summary-skipped = { $count ->
        [0] No row is left out
        [one] One row is left out
       *[other] { $count } rows are left out
    }
csv-summary-warnings = { $count ->
        [0] No row has warnings
        [one] One row has warnings
       *[other] { $count } rows have warnings
    }
csv-summary-links = { $parents ->
        [one] One subtask
       *[other] { $parents } subtasks
    }, { $links ->
        [one] one link
       *[other] { $links } links
    }, { $entries ->
        [one] one time entry
       *[other] { $entries } time entries
    }, { $comments ->
        [one] one comment
       *[other] { $comments } comments
    }
csv-new-statuses = New statuses: { $names }
csv-new-types = New types: { $names }
csv-new-labels = New labels: { $names }
csv-no-recurrence = Repeating tasks are not read from the file: each row becomes one task that doesn't repeat.
csv-blocked = Rows with errors stop the import. Fix the mapping, or leave those rows out.
csv-problems = Rows with problems
csv-problems-none = No row has a problem.
csv-problems-more = { $count ->
        [one] One more row has problems.
       *[other] { $count } more rows have problems.
    }
csv-problem-line = Line
csv-problem-row = Row
csv-problem-column = Column
csv-problem-what = Problem
csv-problem-error = Error
csv-problem-warning = Warning
csv-save-preset = Save as preset
csv-save-preset-name = Preset name
csv-preset-saved = Saved the preset { $name }.
csv-import-start = { $count ->
        [one] Import one task
       *[other] Import { $count } tasks
    }
csv-imported = { $count ->
        [one] Imported one task into { $project }.
       *[other] Imported { $count } tasks into { $project }.
    }
