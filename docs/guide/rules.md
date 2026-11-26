<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Status rules

A status's rules apply to every task that enters it, whoever moves it: you, the
command line or an agent. Set them in Project settings › **Rules**.

## When a task enters a status

For each status you can choose what happens when a task enters it:

- **Set the start date to today, if it has none**;
- **Set the due date to today, if it has none**;
- **Progress on entering:** leave it, or set it, for example to 100 %;
- **Priority on entering:** leave it, or set it;
- set a custom field to a value, or a date field to today;
- **Stop the timer and log its time**;
- **Release an agent's claim on the task**;
- **Clear on entering:** fields to empty.

## Fields a status needs or protects

- **Must be filled in before a task can enter:** a task without these fields
  can't move into the status. The board and the list say what is missing:
  *Fill in first: …*.
- **Can't change while a task is in this status:** these fields are read-only
  while the task stays there.

The fields can be the title, description, type, priority, start and due date,
estimate, progress, labels, parent task, version, and custom fields.

## Rules for agents

Under **Agents**, rules hold only for agents, as well as the status rules. You
are not held to them.

- **Agents may not move tasks into this status**, for example *Done*, so that
  you close tasks yourself after a review.
- **Fields agents may not change**, for example the priority or the version.

An agent that breaks a rule gets an answer that explains why, and changes
nothing.

## Examples

| Status | Rule | Why |
|---|---|---|
| In progress | Set the start date to today | The Gantt chart shows when work began |
| Review | Must be filled in: Estimate | Every reviewed task has an estimate |
| Done | Progress on entering: 100 %; Stop the timer and log its time | Closing a task finishes it |
| Done | Agents may not move tasks into this status | You close tasks yourself |

## From the command line

`glyphkeel project rules <project>` changes the rules; see
`glyphkeel project rules --help` and [the command reference](../reference/cli.md).
