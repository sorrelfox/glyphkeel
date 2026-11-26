<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Roadmap and versions

A version groups the tasks of one release, with its dates and progress. The
**Roadmap** view, in the header bar, shows a project's versions in order, each
with its progress bar and its tasks.

## Making a version

Choose **New version** on the roadmap. A version has:

- a **Name**, such as `1.2`;
- a **Start date** and a **Due date**, both optional;
- a **Status**: **Open**, **Locked** or **Closed**. No tasks can be added to a
  locked version;
- **Used by**: **This project only**, or **This project and its sub-projects**.
  A shared version can be chosen in the sub-projects' tasks too, and shows
  *From …* there;
- a **Description**.

## Planning tasks

Set a task's **Version** field in the task panel. On the roadmap, the tasks that have no version are
counted apart.

In queries, `version:"1.2"` (or `fix-version:`) finds a version's tasks, and
`version:none` the unplanned ones.

## Progress and dates

The progress bar counts the version's closed tasks against all of them. A
version that is past its due date is marked **Late**. In the Gantt chart,
a version's due date shows as a marker on the timeline.

## Closing a version

**Close version**, in the version's menu, asks where its open tasks go: to
another version, to **No version**, or **Stay in this version**. **Open again**
reopens it. Closed versions are hidden until you choose to show them.

**Delete version** removes it, and its tasks are planned for no version. Revert
in the activity log brings both back.

## From the command line

```sh
glyphkeel version create --project TM --name 1.2 --due-date 2027-03-31
glyphkeel version list TM
glyphkeel task edit TM-41 --fix-version 1.2
```
