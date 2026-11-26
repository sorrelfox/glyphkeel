## Recurring tasks and templates.

## A rule in words. $interval is a number; $days a list of weekday names;
## $weekday a weekday's name, $month a month's name, $date a month and day.

repeat-daily = { $interval ->
        [one] Every day
       *[other] Every { $interval } days
    }
repeat-weekly = { $interval ->
        [one] Every week on { $days }
       *[other] Every { $interval } weeks on { $days }
    }
repeat-monthly-day = { $interval ->
        [one] Every month on day { $day }
       *[other] Every { $interval } months on day { $day }
    }
repeat-monthly-last-day = { $interval ->
        [one] Every month on the last day
       *[other] Every { $interval } months on the last day
    }
repeat-monthly-weekday = { $interval ->
        [one] Every month on the { $weekday }
       *[other] Every { $interval } months on the { $weekday }
    }
repeat-yearly-day = { $interval ->
        [one] Every year on { $date }
       *[other] Every { $interval } years on { $date }
    }
repeat-yearly-last-day = { $interval ->
        [one] Every year on the last day of { $month }
       *[other] Every { $interval } years on the last day of { $month }
    }
repeat-yearly-weekday = { $interval ->
        [one] Every year on the { $weekday } of { $month }
       *[other] Every { $interval } years on the { $weekday } of { $month }
    }
# $nth is 1 to 5, or -1 for the last.
repeat-nth-weekday = { $nth ->
        [1] first { $weekday }
        [2] second { $weekday }
        [3] third { $weekday }
        [4] fourth { $weekday }
        [5] fifth { $weekday }
       *[other] last { $weekday }
    }
repeat-after-done-days = { $interval ->
        [one] 1 day after it is done
       *[other] { $interval } days after it is done
    }
repeat-after-done-weeks = { $interval ->
        [one] 1 week after it is done
       *[other] { $interval } weeks after it is done
    }
repeat-after-done-months = { $interval ->
        [one] 1 month after it is done
       *[other] { $interval } months after it is done
    }
repeat-after-done-years = { $interval ->
        [one] 1 year after it is done
       *[other] { $interval } years after it is done
    }
repeat-with-until = { $rule }, until { $until }
repeat-with-count = { $count ->
        [one] { $rule }, once
       *[other] { $rule }, { $count } times
    }

## The task panel.

prop-repeats = Repeats
repeat-none = Doesn't repeat
repeat-value = Repeats: { $value }
repeat-title = Repeat this task
repeat-every = Every
repeat-interval = How many
repeat-unit = Unit
repeat-unit-option = { $unit ->
        [day] { $count ->
            [one] day
           *[other] days
        }
        [week] { $count ->
            [one] week
           *[other] weeks
        }
        [month] { $count ->
            [one] month
           *[other] months
        }
       *[year] { $count ->
            [one] year
           *[other] years
        }
    }
repeat-weekdays = On these days
repeat-monthly-by = Falls on
repeat-by-day = A day of the month
repeat-by-last-day = The last day
repeat-by-weekday = A weekday of the month
repeat-day-of-month = Day of the month
repeat-week-of-month = Which one
repeat-nth-option = { $nth ->
        [1] First
        [2] Second
        [3] Third
        [4] Fourth
        [5] Fifth
       *[other] Last
    }
repeat-weekday = Weekday
repeat-month = Month
repeat-ends = Ends
repeat-ends-never = Never
repeat-ends-on = On a date
repeat-ends-after = After a number of times
repeat-until = End date
repeat-count = Number of times
repeat-count-suffix = { $count ->
        [one] time
       *[other] times
    }
repeat-from = Count the next date from
repeat-from-schedule = The schedule
repeat-from-completion = When it is done
repeat-create = Make the next one
repeat-create-on-close = When this one is closed
repeat-create-ahead = Ahead, on its date
repeat-days-ahead = Days ahead
repeat-days-ahead-suffix = { $count ->
        [one] day before its date
       *[other] days before its date
    }
repeat-text = Or type the rule
repeat-text-placeholder = e.g. every 2 weeks on mon and thu
repeat-text-hint = Typed words win over the choices above.
repeat-preview = Next dates
repeat-preview-none = The rule has no dates.
repeat-save = Save
repeat-stop = Stop repeating
repeat-skip = Skip this one
repeat-update-details = Use this task's details for the next ones
repeat-note = Changes to this task apply to this occurrence only. Later ones start from the series' details, unless you use this task's details for them.
repeat-next = Next: { $date }
repeat-next-after-done = The next date is set when this one is done.
repeat-next-none = No more dates after this one.
repeat-made = { $count ->
        [one] 1 occurrence so far
       *[other] { $count } occurrences so far
    }
repeat-series-title = New ones are titled “{ $title }”.
toast-repeat-set = The task repeats.
toast-repeat-stopped = The task no longer repeats.
toast-repeat-skipped = Skipped; the task is now due { $date }.
toast-repeat-details = The next occurrences take this task's details.

# A recurring task's marker in lists, on Today and on the calendar.
recurring-mark = Recurring task
# A row's or a bar's accessible name with its marker; $label is the name without it.
recurring-label = { $label }, recurring

## Templates.

template-picker = Template
template-none = No template
panel-save-template = Save as template…
template-save-title = Save as template
template-name = Name
template-description = What it is for
template-save = Save template
toast-template-saved = Template “{ $name }” saved.
template-left-out = { $count ->
        [one] Left out, as the project doesn't have it: { $names }
       *[other] Left out, as the project doesn't have them: { $names }
    }
template-made = { $key } made from “{ $name }”.
project-template = Start from
project-template-none = An empty project
project-settings-templates = Templates
templates-task = Task templates of this project
templates-project = Project templates
templates-none-task = No task templates yet. Save a task as one from its menu.
templates-none-project = No project templates yet.
# The project template that comes with Glyphkeel.
template-built-in-software = Software project
template-built-in-software-description = Epics, stories, tasks and bugs on a board from the backlog through review to done.
template-built-in-mark = Comes with Glyphkeel
template-contents = Contents of { $name }
template-show-contents = Show what it holds
template-hide-contents = Hide what it holds
# The same button's accessible name: its text, then the template's name.
template-show-contents-of = Show what it holds: { $name }
template-hide-contents-of = Hide what it holds: { $name }
template-contents-loading = Loading…
template-task-title = Title
template-task-type = Type
template-task-priority = Priority
template-task-labels = Labels
template-task-estimate = Estimate
template-task-start = Starts
template-task-due = Due
template-task-in-days = { $days ->
        [0] On the day it is used
        [one] One day after it is used
       *[other] { $days } days after it is used
    }
template-task-version = Version
template-task-fields = Custom fields
template-task-subtasks = Subtasks
template-setup-statuses = Statuses
template-setup-types = Task types
template-setup-labels = Labels
template-setup-fields = Custom fields
template-setup-field-required = { $name } (required)
template-setup-agent-rules = Rules for agents
template-setup-agent-read-only = { $count ->
        [one] Agents may not change one field
       *[other] Agents may not change { $count } fields
    }
template-setup-wip = WIP limit { $limit }
template-setup-pages = Wiki pages
template-setup-tasks = Tasks
template-setup-task-templates = Task templates
template-nothing = None
template-rename = Name of { $name }
template-describe = Description of { $name }
template-delete = Delete { $name }
toast-template-changed = Template changed.
toast-template-deleted = Template “{ $name }” deleted.
template-items-task = { $count ->
        [one] 1 task
       *[other] { $count } tasks with subtasks
    }
template-items-project = { $count ->
        [one] 1 task or page
       *[other] { $count } tasks and pages
    }
template-save-project = Save this project as a template
template-include-tasks = Include its open tasks
template-save-project-button = Save as template

## Task types' default due date.

type-due-offset = Default due date of { $name }, in days after a task is made
type-due-offset-suffix = { $count ->
        [one] day
       *[other] days
    }
type-due-offset-head = Due in
