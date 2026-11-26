<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Time tracking and reports

## Logging time

The **Time** section of a task lists its time entries and their total.

- **Log time** adds an entry: a **Duration** (such as `1h 30m`, `90m`, `1,5h` or
  `1:30`), the **Day** of the work, a **Note** and a **Kind of work**.
- **+30m** logs 30 minutes for today in one click.
- Each entry's menu has **Edit** and **Delete**.

The task's **Spent** field shows the total, and how much is left of the estimate
or how far over it the task is.

## The timer

**Start timer** in a task's Time section, or T, starts the timer on the task.
There is one timer per workspace: starting it on another task stops it on the
first one and logs that time.

- The timer shows in the sidebar while it runs. Click it to open its task.
- **Pause the timer** keeps its time; **Go on with the timer** continues.
- **Stop the timer and log the time** logs an entry on the task. A timer that ran
  under half a minute logs nothing.
- **Start the timer as** a kind of work, so the entry gets it.

A timer still running after 8 hours gets a reminder. Change that in Preferences ›
Time › **Remind about a running timer after**; 0 never reminds. A
[status rule](rules.md) can stop the timer when a task enters a status, such as
*Done*.

The command line shares the same timer: `glyphkeel timer start TM-41`,
`glyphkeel timer status`, `glyphkeel timer stop`.

## Kinds of work

Kinds of work, such as *Development*, *Design* or *Meeting*, mark time entries,
and the time report groups by them. Manage them in Preferences › Time › **Kinds
of work**. A kind that entries use is archived rather than deleted: it is no
longer offered, and its entries keep it.

## Reminders

Set a task's **Reminder** in the task panel:

- **At a time**: a date and time;
- **Before the due date**: a number of days before, at a **Time of day**. Left
  empty, it uses the workspace's reminder time (9:00 unless you change it in
  Preferences › Notifications).

A reminder can have its own text. A reminder on a task without a due date waits
for one.

Reminders show as desktop notifications; **Open** in the notification opens the
task. With Preferences › Notifications › **Show reminders as desktop
notifications** off, they are kept in the app only. Today lists the day's
reminders. If Glyphkeel wasn't running when a reminder was due, it shows the
missed reminder when it starts. Keep it running in the background
([Getting started](getting-started.md#closing-and-running-in-the-background))
so that reminders come on time.

## The time report

**Time report** in the sidebar shows the time logged over a range of days.

- Choose the range with **First day** and **Last day**, or **This week**, **Last
  week**, **This month**, **Last month** or **This year**.
- Narrow it to a **Project** (with **Include sub-projects**) or a **Kind of
  work**.
- **Group by** **Day**, **Week**, **Month**, **Task**, **Project**, **Person** or
  **Kind of work**. Each row shows the time and its **Share of the time**. Time
  logged by agents shows as **Agent**.
- **Export** saves the rows, or the single entries, as CSV.

From the command line: `glyphkeel time report --from 2026-11-01 --to 2026-11-30
--group-by project`.

## Flow reports

**Reports** in the sidebar has three reports, built from the history of your
tasks:

- **Created and closed:** tasks created, closed and reopened in each period, with
  the open tasks at its end;
- **Cumulative flow:** how many tasks were in each status, or each category, at
  the end of each period;
- **Cycle time:** how long tasks took, as **Cycle time** (from the first move into
  progress to the last close) or **Lead time** (from creation to the last
  close), with the median and the 85th and 95th percentiles.

Each report takes a **Period length** (Days, Weeks or Months), a range such as
**Last 30 days**, and filters: **Type**, **Label**, **Version**, a **Query** and
**Done by** (Anyone, People or Agents). **Show as a table** shows the numbers,
and **Export as CSV** saves them.
