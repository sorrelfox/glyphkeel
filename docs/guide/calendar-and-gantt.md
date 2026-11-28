<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Calendar and Gantt chart

Both views show a project's tasks by their dates. Both have a **Filter query**
field that takes the [query language](../query-language.md), and **Include
sub-projects**.

## The calendar

Open it with **Calendar** in the header bar, or Ctrl+5. It shows a **Month** or
a **Week**. A task shows on its due date, or across its days from start to due.

- **Moving between pages:** the arrows next to **Today**, or Alt+Page Up and
  Alt+Page Down. Alt+Home goes back to today.
- **Changing a date:** drag a task to another day. With the keyboard, Alt and
  the left or right arrow key move it by a day, and Alt and up or down by a
  week.
- **Adding a task:** choose **Add a task on …** in a day, or press N on a day.
  The new task is due that day.
- In the grid, the arrow keys move between days, Page Up and Page Down between
  pages, and Enter goes to the tasks of the day.

The first day of the week comes from Preferences › Time.

## The Gantt chart

Open it with **Gantt** in the header bar, or Ctrl+3. On the left is a table of
tasks (Key, Title, Start, Due, progress and Claimed by), and on the right their
bars on a timeline. Parent tasks show the range of their subtasks.

### Moving around

- **Zoom:** Day, Week, Month or Quarter, or `+` and `-`.
- **Today**, **Earlier** and **Later** move the timeline. Alt+Page Up, Alt+Page
  Down and Alt+Home work too.
- **Group** groups the rows by **Parent**, **Label**, **Status** or **Claimed
  by**, or **None**.
- Up and Down move between tasks, and Left and Right fold and unfold a parent.

### Changing dates

- Drag a bar to move it, or drag its ends to change the start or due date.
- With the keyboard: Alt and the left or right arrow key move the bar by a day.
  With Shift as well, they make it a day shorter or longer. F2 edits the start
  date and Shift+F2 the due date.
- Tasks without dates are listed apart. Drag one onto the timeline to schedule
  it.
- **Also move dependents** moves the tasks that wait for a task along with it.

### Dependencies

A *blocks* link between two tasks is a dependency: the blocked task may start
only after the other one ends. Draw one by dragging from the small circle at
the end of a bar to another bar. With the keyboard, select a task, press L, go
to the blocked task and press Enter; doing the same on an existing dependency
removes it. Click a dependency to give it a **lag in days**, or to remove it.
Enter opens the selected task.

The chart lists problems, such as a task that starts before its blocker ends.
A task whose progress is behind its dates is marked **behind schedule**, and an
overdue task shows how late it is.

### Critical path

**Critical path** highlights the tasks that decide when the project ends: the
longest chain of dependencies, where any delay moves the end. Their bars are
striped and outlined in the alert colour, and the dependencies between them
are drawn thicker. A **Slack** column shows, for every task, how many working
days it can slip without moving the end; a bar's tooltip says so too. A note
above the chart says how many tasks are critical, the earliest the plan can
finish, and how many tasks are left out because they have neither dates nor an
estimate.

The path follows the dependencies and their lags. A task with dates takes as
long as its dates say and doesn't start before its start date; a task without
dates takes its estimate. A parent counts as critical when one of its subtasks
is. Working days are the first days of the week from Monday, as many as the
workspace's **days per week** (five: Monday to Friday). A working week that
starts on another day, such as Sunday to Thursday, can't be set yet.

The chart remembers the switch and the baseline shown (below) for each
project, as your own setting.

### Baselines

A baseline is a saved copy of the plan: every dated task's start and due date
at the moment you save it. Save one before work starts, and see later how far
the plan has moved.

- **Baseline** › **Save baseline…** saves the project's plan under a name.
  A project can have several baselines.
- **Baseline** › **Show** chooses the baseline to compare with, or **None**.
- **Rename shown baseline…** and **Delete shown baseline…** change the one
  shown; deleting asks first, and leaves the tasks as they are.
- With a baseline shown, a thin grey bar under each task bar shows its
  baseline dates, and a **Slip** column says how many days later (`+3d`) or
  earlier (`-1d`) it now ends. Tasks added since are marked **New**, and the
  tasks the baseline has that the plan no longer shows (deleted, or without
  dates now) are listed above the chart.
- Saving and deleting a baseline can be undone, like any other change.
  Baselines travel in archives ([Export and import](export-import.md)).

In queries, `slip>3d` finds the tasks that slipped more than three days against
the newest baseline, and `baseline:"Kick-off" slip>3d` against a chosen one
([The query language](../query-language.md#the-critical-path-and-baselines)).

### Milestones

A task of a type with the Milestone shape is a milestone: a diamond on its due
date. **Milestones** lists them. A version's due date shows as a marker too
([Roadmap](roadmap.md)).

### Export

**Export** saves the chart as a **PNG image**, or opens **Print or save as PDF**.

Like the list, the chart has **saved views** of its query
([Saved views](list.md#saved-views)); a view can be marked **Show it first when
the chart opens**. The chart remembers your zoom, grouping, column widths and
**Also move dependents**.

On touch screens the chart is read-only; change dates in the task.

## From the command line

`glyphkeel schedule --project TM` prints the same schedule: dated tasks as a
tree, milestones, tasks without dates, dependencies and problems, with each
task's slack. `--critical` keeps only the critical tasks; `--target` counts
back from a date, a milestone's key or a version's due date instead of the
latest finish. `--baseline "Kick-off"` adds each task's baseline dates and slip.

`glyphkeel baseline create TM --name "Kick-off"` saves a baseline;
`glyphkeel baseline list TM`, `baseline rename` and `baseline delete` manage
them.
