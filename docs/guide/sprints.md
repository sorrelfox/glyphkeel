<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Sprints

A sprint is a short, fixed stretch of time, often two weeks, with a goal and
the tasks the team means to finish in it. The **Sprints** view, in the header
bar, shows a project's running sprint with its burndown, the planned sprints
and the backlog: the open tasks that are in no sprint.

A task is in at most one sprint, of its own project. A project has at most one
active sprint at a time.

## Planning a sprint

Choose **New sprint**. The name is filled in with the next number (*Sprint 4*)
and the dates with the two weeks after the last planned sprint; change them as
you like. A sprint has:

- a **Name**, unique in the project. `active`, `none` and `backlog` can't be
  used: they mean something else in queries;
- a **Start date** and an **End date**, both optional while it is planned;
- a **Goal**: what the sprint is meant to achieve.

## Putting tasks in a sprint

- **Drag** tasks from the backlog to a sprint, between sprints, or back to the
  backlog.
- **Choose** tasks with their checkboxes, in any of the lists, and pick a sprint
  or **Backlog** under **Move to**. This works from the keyboard too.
- In the task panel, set the task's **Sprint** field.

Tasks that move to another project leave their sprint: a task can only be in
a sprint of its own project. The move dialog lists the sprints they leave.

A **duplicate** of a task stays in the task's sprint, unless that sprint is
completed; then the copy goes to the backlog.

A **task template** never names a planned or completed sprint. It may say
`active`: a task made from it then joins the sprint running at that moment,
and goes to the backlog when none is. A template saved from a task in the
running sprint says `active`. A **recurring task** follows the same rule: when
the series was set up on a task in the running sprint, each new occurrence
joins the sprint running when it is made; otherwise new occurrences start in
the backlog.

## Starting and completing

**Start sprint** asks for the dates (from today for two weeks, unless the sprint
has its own) and the goal. A planned sprint can't start while another one runs.

**Complete sprint** says how many of its tasks are closed and still open, and
asks where the open ones go: to a planned sprint or back to the backlog. Closed
tasks stay in the completed sprint, which keeps its burndown. **Show completed
sprints** lists them, newest first.

A planned or completed sprint can be deleted from its menu; its tasks go back to
the backlog. Undo, or revert in the activity log, brings everything back, and the
same goes for starting and completing.

## The burndown

The running sprint shows a burndown chart: for each day from its start, the work
still open at the end of the day, against the ideal line from the work at the
start down to none on the last day. **Tasks** counts tasks; **Estimate** adds up
their estimates. The grey line is the sprint's scope, which rises when tasks are
added after the start.

The chart can be read without seeing it: focus it, and the arrow keys go from
day to day, Home and End to the first and last.

**Burn-up** next to the burndown opens the sprint's burn-up, and **Velocity**
at the top of the page the project's velocity, on the **Reports** page (see
[Planning reports](time.md#planning-reports)). Each planned sprint shows the
work planned for it against the average velocity of the last three sprints:
a guide, not a limit.

## Board, list and queries

- The board's **Sprint** filter shows the cards of one or more sprints, or of
  the backlog.
- The list's **Sprint** menu sets the query's sprint condition, and the
  **Sprint** column shows each task's sprint.
- In queries, `sprint:active` finds the tasks of the project's running sprint,
  `sprint:"Sprint 4"` those of a sprint by name, and `sprint:none` the tasks in
  no sprint. `has:sprint`, `was:sprint:"Sprint 3"` and `changed:sprint` work as
  for other fields, and renaming or deleting a sprint updates saved views.

## From the command line and for agents

```sh
glyphkeel sprint create --project TM --name "Sprint 4" --goal "Ship the importer"
glyphkeel sprint add "Sprint 4" --project TM --tasks TM-41 --tasks TM-42
glyphkeel sprint start "Sprint 4" --project TM --end-date +2w
glyphkeel sprint burndown active --project TM
glyphkeel sprint complete active --project TM --move-open-tasks-to "Sprint 5"
glyphkeel task edit TM-43 --sprint active
```

Agents have the same through MCP: `list_sprints`, `get_sprint`,
`create_sprint`, `update_sprint`, `start_sprint`, `complete_sprint`,
`add_tasks_to_sprint`, `remove_tasks_from_sprint`, `delete_sprint` and
`get_sprint_burndown`, and the `sprint` field of `create_task` and
`update_task`.
