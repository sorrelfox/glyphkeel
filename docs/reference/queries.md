<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Task queries

The list, saved views, the Gantt chart, bulk changes, `glyphkeel task list` and the MCP tool
`search_tasks` all take the same query language. A query is a list of conditions, joined by
spaces (all of them must hold) or `OR`, with `-` or `NOT` in front of one that must not
hold, parentheses to group, words and `"phrases"` to search for, and one `sort:` clause:

```text
is:open label:bug,crash -status:Done due<+7d sort:due,-priority
```

A condition is a field, an operator and one or more values separated by commas (any of
them): `label:bug,crash`. The operators are `:` (is, contains, on or within), `=` (exactly),
`!=` (is not; tasks without a value match too), and `<`, `<=`, `>`, `>=` for ordered fields.
Values with spaces go in quotes: `status:"In progress"`. Names (statuses, types, labels,
versions, sprints) are matched without regard to case; an unknown name is an error with the names
that exist.

Dates are written `2026-12-31`, as `today`, `yesterday`, `tomorrow`, as an offset (`-7d`,
`+2w`, `-1m`, `+1y`) or as a period (`this-week`, `last-month`, `next-year`…). With `:` a
period means "within it", `<` "before it starts" and `>` "after it ends". Durations are
written `90m`, `1.5h`, `1h30m`, `1:30`, `2d` or `1w`, in the workspace's working day and week.

The query field completes fields, operators and names as you type, and the CLI and MCP
answer a mistake with its place in the query and a suggestion.

## Fields

| Field | Values |
|---|---|
| `key` | a task key such as `TM-41` |
| `project` | a project key; `TM/*` adds its sub-projects |
| `title` | words in the title; `=` the exact title |
| `text`, `description`, `comment` | words in the title, description, comments or attached files (`text`); the words of text, Markdown and link custom fields count as the description's |
| `status` | status names |
| `category` (`cat`) | `todo`, `in_progress`, `done`, `cancelled`; `open`, `closed` |
| `type` | task type names |
| `priority` (`prio`) | `none` < `low` < `normal` < `high` < `urgent` |
| `label` (`labels`) | label names, or `none` |
| `version` (`fix-version`) | version names, or `none` |
| `sprint` (`sprints`) | sprint names, `active` (the project's running sprint), or `none` (the backlog) |
| `parent`, `under` | a task key (`parent` also `none`); `under` reaches every depth |
| `blocks`, `blocked-by`, `relates`, `duplicates` | a task key, or `any` |
| `start`, `due` | a date, or `none` |
| `during` | a date, period or range: the task's days from start to due overlap it |
| `created`, `edited`, `updated`, `closed` | a date; `updated` is any change, `edited` the task's own fields |
| `estimate`, `spent`, `remaining` | a duration, or `none` |
| `progress` | a percentage, `50` or `50%` |
| `slack` | working days of slack on the critical path, such as `2d`; negative when the end can't be met. Not in automation rule conditions |
| `slip` | days later than in the project's newest baseline, or the one `baseline:` names, such as `3d` or `-1d` |
| `baseline` | a baseline's name: the task is in that baseline of its project |
| `author` (`creator`) | `me`, `agent`, or an agent's name |
| `claimed` | an agent's name, or `any` |
| `comments`, `files`, `subtasks` | a number |
| `is` | `open`, `closed`, `overdue`, `due-soon`, `blocked`, `claimed`, `subtask`, `parent`, `unscheduled`, `milestone`, `over-estimate`, `critical` |
| `has` | `due`, `start`, `estimate`, `description`, `labels`, `version`, `sprint`, `files`, `comments`, `subtasks`, `parent`, `links`, `time`, `reminder`, `attachment-text` (a file whose text search reads) |
| `in` | `files`: `in:files` looks for the query's words only inside attached files, `-in:files` never there |
| `was`, `changed`, `changed-by` | the history: see below |
| `cf.<name>` | a custom field, with the operators of its kind |

## The history

Every change is recorded, so a query can ask what a task was and when it changed.

### `was:` a value the task had

`was:<field>:<values>` finds the tasks that have one of the values now or had it at any time
since they were created:

```text
was:status:Review             tasks that have been in review
was:category:in_progress      tasks that were ever started
-was:priority:urgent          tasks that were never urgent
was:label:regression is:open  open tasks that once had the label
```

`was:` looks up `status`, `category`, `type`, `priority`, `label`, `version` and `sprint`, and every
custom field as `was:cf.<name>`. It takes `:` and `!=` (`was:status!=Review` is "never in
review"); `none` can't be looked up, use `status:none` and the like for a value that is empty
now.

A custom field's values are written as for the field itself: option names for a select field
(any one of a multi-select's options counts), `yes` or `no`, a number, a date, and for text
fields words of the text with `:` or the exact text with `=`. Ranges don't apply here.

```text
was:cf.area:"front end"       tasks that were in the front end at some point
was:cf.story-points:8         estimated at eight points at any time
-was:cf.approved:yes          never approved
```

### `changed:` a field that changed

`changed:<fields>` finds the tasks whose field changed after they were created, at any time;
several fields mean any of them. With an operator and dates after the field, it finds the
changes made then:

```text
changed:status                status changed at least once
changed:due,start             either date was moved
changed:due>-7d               the due date changed in the last week
changed:status:this-week      the status changed this week
changed:priority:2026-11-01..2026-11-15
```

`changed:` looks up `status`, `type`, `priority`, `title`, `description`, `start`, `due`,
`estimate`, `progress`, `parent`, `label`, `version`, `sprint`, `project` and `closed`, and custom
fields as `cf.<name>` (`changed:status,cf.story-points`, `changed:cf.area>-7d`). The dates take
the operators of dates (`:`, `!=`, `<`, `<=`, `>`, `>=` and ranges), in the workspace's time
zone. Creating a task is not a change of its fields.

### `changed-by:` who changed it

`changed-by:me` finds the tasks whose fields the person changed, `changed-by:agent` those an
agent changed, and `changed-by:"Build bot"` those a named agent changed. Like `changed:`, it
leaves out creation; `author:` tells who created a task. Changes of custom fields count like
the others: `changed-by:agent changed:cf.story-points` finds the tasks whose points an agent
changed (and the field changed at some time; the two needn't be the same change).

A renamed custom field keeps its history, and saved views that name it in `was:` or
`changed:` follow the new name; a deleted field leaves those conditions.

The history conditions read the activity log, which keeps reverted changes too: a status
that was undone was still the task's status for a while. Comments, files and logged time are
not changes of the task's fields.
