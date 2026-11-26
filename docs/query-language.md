<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The query language

A query is a line of text that picks tasks, such as:

```text
is:open label:bug due<+7d sort:due
```

"Open tasks with the label *bug*, due within a week, soonest first." The same
query works everywhere tasks are filtered:

- the list, saved views, the calendar and the Gantt chart;
- bulk changes and flow reports;
- `{{tasks …}}` lists in wiki pages;
- `glyphkeel task list --query …` on the command line;
- the MCP tool `search_tasks` for agents.

This page explains the language with examples. [Task queries](reference/queries.md)
lists every field and operator.

## Conditions

A condition is a field, an operator and a value: `status:Review`,
`priority>=high`, `due<today`.

| Operator | Means | Example |
|---|---|---|
| `:` | is, contains, on or within | `label:bug`, `title:login`, `due:this-week` |
| `=` | exactly | `title="Fix the login"` |
| `!=` | is not (tasks with no value match too) | `status!=Done` |
| `<` `<=` `>` `>=` | before, up to, after, from | `due<today`, `estimate>=1d`, `priority>normal` |

- **Several values** separated by commas mean *any of them*:
  `label:bug,crash`.
- **Quotes** hold values with spaces: `status:"In progress"`.
- Names of statuses, types, labels and versions are matched without regard to
  case. A name that doesn't exist is an error, which lists the names that do.

## Combining conditions

| Write | Means |
|---|---|
| `a b` | both |
| `a OR b` | either |
| `-a` or `NOT a` | not |
| `( … )` | a group |

```text
is:open (priority:urgent OR label:security)
is:open -label:waiting
project:TM/* NOT category:done
```

## Words

Words without a field search the title, the description and the comments:

```text
login timeout is:open
"release notes"
```

## Sorting

One `sort:` clause sets the order. `-` in front reverses a key:

```text
is:open sort:due,-priority
sort:-updated
```

## Dates

| Form | Example |
|---|---|
| A day | `2026-12-31` |
| Words | `today`, `yesterday`, `tomorrow` |
| An offset from today | `-7d`, `+2w`, `-1m`, `+1y` |
| A period | `this-week`, `last-month`, `next-year` |
| A range | `2026-12-01..2026-12-31` |

With `:` a period means *within it*, `<` *before it starts* and `>` *after it
ends*. Weeks start on the day set in Preferences › Time.

```text
due<today is:open          overdue
due:this-week
due<+7d                    due within a week (and overdue)
created>-30d
closed:last-month
during:next-month          the task's days from start to due overlap next month
due:none                   no due date
```

## Durations and numbers

Durations are written `90m`, `1.5h`, `1h30m`, `1:30`, `2d` or `1w`, in the
workspace's working day and week (8 hours and 5 days unless changed).

```text
estimate>1d
spent>1h
remaining:none
progress>=50
subtasks>0
```

## Shortcuts with `is:` and `has:`

```text
is:open            is:closed           is:overdue         is:due-soon
is:blocked         is:claimed          is:subtask         is:parent
is:unscheduled     is:milestone        is:over-estimate

has:due            has:estimate        has:description    has:labels
has:version        has:files           has:comments       has:subtasks
has:parent         has:links           has:time           has:reminder
```

Combine them with `-`: `is:open -has:labels` finds open tasks without labels.

## Projects, people and links

```text
project:TM                 one project
project:TM/*               a project and its sub-projects
author:me                  tasks I created
author:agent               tasks an agent created
claimed:any                tasks an agent has claimed
under:TM-12                everything below TM-12, at any depth
parent:none                top-level tasks
blocked-by:any             tasks with a blocker
version:"1.2" is:open
```

## Custom fields

Each custom field has a query name, shown in Project settings › Fields as
**In queries:**. Use it with `cf.`:

```text
cf.story-points>=5
cf.customer:"Acme"
cf.approved:no
```

## The history

Glyphkeel records every change, so a query can ask what a task was and when it
changed:

```text
was:status:Review                  tasks that have been in review
-was:priority:urgent               never urgent
changed:due>-7d                    the due date moved in the last week
changed:status:this-week
changed-by:agent                   an agent changed the task's fields
changed-by:agent changed:cf.story-points
```

## Examples

| Query | Finds |
|---|---|
| `is:open due<today sort:due` | Overdue tasks, oldest first |
| `is:open priority>=high -is:blocked sort:-priority` | Important work that can start now |
| `status:"In progress" updated<-14d` | Work that has stalled |
| `type:Bug is:open created>-7d` | New bugs this week |
| `is:closed closed:last-week sort:-closed` | What was finished last week |
| `is:open has:estimate is:over-estimate` | Tasks over their estimate |
| `is:open -has:due project:TM/*` | Open tasks without a due date in TM and below |
| `author:agent created:today` | What agents created today |
| `is:open version:none` | Open tasks not planned for a version |

## Mistakes

The query field completes fields, operators and names as you type. A mistake is
pointed out where it is, with a suggestion: `stauts:open` gives *Unknown field
"stauts". Did you mean "status"?* The command line and agents get the same
answers.
