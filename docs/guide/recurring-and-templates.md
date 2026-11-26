<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Recurring tasks and templates

## Recurring tasks

A recurring task comes back on a schedule: each time, a new occurrence is made
with the series' details and the next date. Open a task, choose **Repeats**, and
set the rule in **Repeat this task**:

- **Every** so many days, weeks, months or years;
- for weeks, **On these days**;
- for months and years, it **Falls on** a day of the month, the last day, or a
  weekday of the month, such as the second Tuesday;
- **Ends**: **Never**, **On a date**, or **After a number of times**;
- **Count the next date from**: **The schedule** (fixed dates), or **When it is
  done** (for example two weeks after you last watered the plants);
- **Make the next one**: **When this one is closed**, or **Ahead, on its date**,
  a number of days before it is due.

You can also type the rule in **Or type the rule**, for example
`every 2 weeks on mon and thu`, `every last friday` or `every month on the 15th`.
Typed words win over the choices above. **Next dates** shows the dates the rule
gives.

A recurring task is marked on its card. In the task panel:

- **Skip this one** moves the task to the next date of the rule;
- **Stop repeating** keeps the task and makes no more occurrences;
- **Use this task's details for the next ones**: changes to a task apply to
  that occurrence only, unless you use this.

From the command line:

```sh
glyphkeel recurrence set TM-12 --text "every 2 weeks on sat"
glyphkeel recurrence preview --text "every last friday"
glyphkeel recurrence skip TM-12
```

## Templates

### Task templates

A task template makes a task with its fields and subtasks.

- **Save as template…** in a task's menu saves the task, with **What it is for**,
  as a template of its project. Its dates are kept as days after the task is
  made.
- When you make a task, choose a **Template**. The new task gets the template's
  title, type, priority, labels, estimate, dates, version, custom fields and
  subtasks.
- Project settings › **Templates** lists the project's task templates. Rename,
  describe or delete them there, and **Show what it holds**.

A task type can also give new tasks a due date: **Due in** a number of days,
in Project settings › Task types.

### Project templates

A project template makes a project with its whole set-up: statuses, task types,
labels, custom fields, rules for agents, WIP limits, wiki pages, tasks and task
templates.

- When you make a project, **Start from** lists the project templates.
- **Software project** comes with Glyphkeel: epics, stories, tasks and bugs on a
  board from the backlog through review to done.
- **Save this project as a template**, in Project settings › Templates, saves a
  project of your own. **Include its open tasks** adds them to the template.

From the command line:

```sh
glyphkeel template list
glyphkeel template create "Weekly report" --from-task TM-12
glyphkeel template use "Weekly report" --project TM
```

What was made from a template stays when the template is deleted.
