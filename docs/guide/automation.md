<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Automation rules

An automation rule makes changes of its own when something happens to a task:
*when* a trigger happens, *if* the task matches a condition, *then* the rule's
actions run. Set rules up in Project settings › **Automation**.

A rule belongs to one project, or to the whole workspace (**Applies to: Every
project**). Rules run in the program that made the change, on your computer;
they never reach the network.

## When: triggers

| Trigger | Sets the rule off when |
|---|---|
| A task is created | a task is created, by you, the command line, an agent, a template or a recurring task |
| A field changes | one of the fields you tick changes (any field when none is ticked): title, description, status, type, priority, labels, dates, estimate, progress, version, parent, or a custom field |
| A task moves to a status | a task moves to the status you choose (any status when none is chosen) |
| A comment is added | someone comments on a task |
| The due date comes | an open task's due date is today: once per task and due date |
| The due date passes | an open task's due date was yesterday or earlier: once per task and due date |
| On a schedule | every day, or every week on one day, at a time of day, for each task the condition matches |

Due dates and schedules are checked every minute while the app (or
`glyphkeel mcp`) runs. A due date more than a week past is no longer acted on,
so a new rule doesn't act on every old task at once. If the app wasn't running
at a scheduled time, the rule runs once when it starts again.

## If: the condition

The condition is a query, written in the same field as the list's query, with
the same colours, completion of fields and values, and the conditions shown as
chips you can remove: for example `type:Bug priority:high` or
`label:urgent is:open`. Leave it empty to act on every task. A scheduled rule needs a condition: it acts on every task the
condition matches, at most 200 at a time.

When you rename a status, a label, a task type, a custom field (or one of its
options) or a project key, the rules that name it follow, as saved views do:
the condition, the trigger's status, and the statuses, labels, types, fields
and options the actions use. The change is part of the rename, so Undo takes
both back.

When you delete a status, a label, a task type, a custom field or an option
that a rule's trigger or actions use, the rule is turned off, and its run log
says which name went away. Change the rule and turn it on again. Conditions
change as saved views do: a deleted label or custom field is taken out of them;
a deleted status or type stays, and the run log shows the mistake when the rule
runs.

## Then: actions

A rule has 1 to 20 actions, done in order:

- **Set a field:** the priority, start or due date (`today`, `+3d`, `next mon`,
  or a date), estimate, progress, type, title, description, version, or a
  custom field; or clear it;
- **Add a label**, **Remove a label**;
- **Move to a status**;
- **Add a comment**;
- **Create a subtask** or **Create a linked task**, with a title, from a task
  template if you choose one; a linked task is linked to the task the rule acts
  on as related, blocking, blocked by, duplicate or copy;
- **Notify me:** a notification, shown with the reminders.

Texts can hold placeholders: `{key}`, `{title}`, `{status}`, `{priority}`,
`{due_date}`, `{project}`, `{rule}`, `{actor}` (who made the change that set
the rule off) and `{today}`. For example: *{key} was triaged by {rule}.*

## Try a rule before you save it

Under **Try it on a task**, enter a task key and choose **Try**. You see
whether the task matches the condition and every change the actions would
make, or why an action would fail. Nothing is changed.

## What a rule's changes are

- **History names the rule.** In a task's history and in Activity, a rule's
  changes show with the rule's name and an *Automation rule* chip. The
  activity filters have a **Rules** choice.
- **Undo takes back the rules too.** A rule's changes belong to the change that
  set it off: one Undo takes back both.
- **Status rules apply.** A rule can't move a task into a status that needs a
  field the task lacks, or change a field that is read-only in its status.
- **Rules for agents apply** to a rule an agent made or changed last, and to
  every rule that follows an agent's change. Once you change an agent's rule,
  it is yours.
- **A failed run changes nothing.** If one action fails, none of the run's
  changes are kept; the change that set the rule off still happens. The run
  log says why.

## Loops and limits

- A rule's own changes never set the same rule off again.
- Rules can set each other off, at most five in a row after one change.
- A rule runs at most 100 times an hour. Past that, it waits for the next hour;
  the run log says so once.

## The run log

Each rule keeps its last 50 runs. Choose **Runs** beside a rule to see when it
ran, on which task, what set it off, how many items it changed, and the error
when it failed or didn't run. The switch beside a rule turns it off and on.

## From the command line and for agents

The same rules can be managed with `glyphkeel automation list`, `show`,
`create`, `edit`, `delete`, `runs` and `test` (see the
[command line reference](../reference/cli.md)), and by agents with the tools
`list_automations`, `get_automation`, `create_automation`,
`update_automation`, `delete_automation`, `get_automation_runs` and
`test_automation` (see [MCP](../mcp.md)).

```sh
glyphkeel automation create --project TM --name "Triage bugs" \
  --trigger '{"on": "task_created"}' --condition 'type:Bug' \
  --actions '[{"kind": "set_fields", "fields": {"priority": "high"}},
              {"kind": "add_label", "label": "triage"}]'
glyphkeel automation test TM-41 --rule "Triage bugs" --project TM
```

## Export and import

An [archive](export-import.md) carries each project's automation rules, and an
export of the whole workspace also carries the workspace's own rules. Imported
rules start afresh: with an empty run log, a scheduled rule waits for its next
time, and a due date rule acts only on what comes due after the import. A
workspace rule whose name is already taken in the workspace you import into is
left out.
