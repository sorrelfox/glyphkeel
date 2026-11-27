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

To move a project under another one, or back to the top level, open its menu
(right-click it in the sidebar, or click its name above the view) and choose
**Move to**: **Top level**, or the project to put it under. Its sub-projects
move with it. Projects that would go deeper than three levels aren't offered.
On the command line: `glyphkeel project move <project> --parent <other>`, or
`glyphkeel project move <project> --top-level`.

## Folders

When you have many projects, group them in folders in the sidebar. A folder
only groups: it changes nothing about its projects, their keys, sub-projects,
queries or tasks. Folders hold projects, never other folders.

- **Make a folder**: choose **New folder** (the folder button next to **New
  project**) in the sidebar, and give it a **Name** and, if you like, a
  **Colour**.
- **Put a project in a folder**: drag it onto the folder, or open its menu and
  choose **Move to folder** and the folder. **No folder** takes it out again.
  Only top-level projects go into folders; a project's sub-projects come along
  with it. A project that you move under another project leaves its folder.
- **Make a project in a folder**: click the folder (or move the focus to it
  with the keyboard) to select it, then choose **New project** (**+**); or
  choose **New project in this folder** from the folder's menu. The dialog's
  **Folder** field starts with the selected folder, or with the folder of the
  open project, and you can change it there. With no folder selected the
  project goes to the top level. A sub-project has no folder of its own: it
  follows its parent.
- **Order**: drag a project before or after another one, or a folder before or
  after another folder. With the keyboard, focus a project or a folder in the
  sidebar and press **Alt+↑** or **Alt+↓**; the menus have **Move up** and
  **Move down** too.
- **Open and close a folder** with its arrow (clicking its name selects it),
  or with **←** and **→** on the keyboard. Glyphkeel remembers which folders
  you closed.
- **Rename** a folder or change its colour from its menu (right-click it, or
  press **Shift+F10**). **Delete folder** keeps its projects: they go back to
  the top level. Undo brings the folder back with its projects.

The folder's number is the open tasks of all its projects. Folders come first
in the sidebar, then the projects in no folder.

On the command line: `glyphkeel folder list`, `glyphkeel folder create --name
Clients`, `glyphkeel project create --name Garden --key GA --folder Clients`,
`glyphkeel folder move <project> --folder Clients` (or `--none true`
to take it out), `glyphkeel folder rename <folder> <name>` and `glyphkeel folder
delete <folder>`. Agents have the matching tools, such as `list_folders` and
`move_project_to_folder`. An export takes each project's folder along, and an
import puts the project in the folder of the same name, making it if needed.

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

Renaming a status, type or label updates the saved views that name it, and
the workspace's saved views, dashboard widgets and automation rules whose query
keeps them to the project (`project:TM`).

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
