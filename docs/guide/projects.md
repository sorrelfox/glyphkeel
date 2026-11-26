<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Projects and sub-projects

A project holds tasks, a wiki, versions and its own set-up: statuses, task
types, labels, custom fields and rules. Every project has a **key**, such as
`TM`, and its tasks are numbered with it: `TM-1`, `TM-2` and so on.

## Making a project

Choose **New project** in the sidebar. Give it a **Name**, and if you like a
**Key**, a **Colour**, an **Icon** and a **Description**. Under **Start from**
you can pick **An empty project** or a project template, such as the built-in
**Software project** ([templates](recurring-and-templates.md#project-templates)).

A new empty project has the statuses *Backlog*, *To do*, *In progress*,
*Review* and *Done*, and the task types *Task*, *Bug*, *Feature* and
*Milestone*. Change them in the project's settings.

## Sub-projects

A project can hold sub-projects, up to three levels deep. Choose **New
sub-project…** in a project's menu (right-click it in the sidebar, or use the
project button in the header bar). A new sub-project copies its parent's
statuses, task types and labels.

The board, the list, the calendar, the Gantt chart and the time report each
have **Include sub-projects**, to show the tasks of the sub-projects too. In a
query, `project:TM/*` means the project and everything below it.

To move a project under another one, or back to the top level, use the command
line: `glyphkeel project move <project> --parent <other>`, or
`glyphkeel project move <project> --top-level`.

## The project key

A key has 2 to 10 capital letters and digits, and starts with a letter. To
change it, open the project's settings and choose **Change…** next to the key.
The old key keeps working: `TM-1` still finds the task in links, addresses, the
command line, agents and queries, and saved queries change to the new key.

## Project settings

Choose **Project settings…** in the project's menu. The settings have these
pages:

- **General:** name, key, colour, icon, description, **Parts** and
  **State**. Parts are the views the project uses: Board, List, Gantt, Wiki and
  Time tracking. With the wiki switched off, its pages can be read but not
  changed.
- **Statuses:** the board's columns, in order. Each status has a **Name**, a
  **Category** (To do, In progress, Done or Cancelled), a **Colour**, a **WIP
  limit** and a **WIP minimum**. The category tells Glyphkeel whether a task is
  open or closed. Deleting a status moves its tasks to another one.
- **Task types:** each type has a **Name** and a **Shape** (Task, Bug,
  Feature, Milestone, Epic or Story). One type is the **Default**. A task of
  the Milestone shape is a milestone in the Gantt chart. A type can also give
  new tasks a default due date, a number of days after they are made.
- **Labels:** names with a colour. Deleting a label takes it off every task.
- **Fields:** custom fields ([Custom fields](custom-fields.md)).
- **Rules:** status rules and rules for agents ([Status rules](rules.md)).
- **Templates:** the project's task templates, the project templates, and
  **Save this project as a template**
  ([Templates](recurring-and-templates.md#templates)).

Renaming a status, type or label updates the saved views that name it.

## Archiving

**Archive** in the project's menu makes a project and its sub-projects
read-only, and moves them to **Archived** at the bottom of the sidebar.
**Unarchive** brings them back.

## Deleting

**Delete** in the project's menu, then **Move to trash**, moves the project,
its sub-projects and everything in them to the trash. You can restore them from
there until the trash is emptied ([The trash](history-and-trash.md#the-trash)).

## Instructions for agents

Agents ask a project for its brief before they work on it. The brief is the
wiki page you mark with **Use as the agent brief** in the page's menu, or else
the project's description. See [Working with AI agents](../mcp.md).
