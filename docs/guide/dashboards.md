<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Dashboards

A dashboard is a page of widgets: how many tasks match a query, the first of
them, open tasks by status, what is overdue or due soon, the flow reports, the
time logged, a sprint's burndown, the progress of versions, the latest changes,
automation rules that failed, and notes. **Dashboards**, in the sidebar or the
command palette, shows them.

A dashboard belongs to the whole workspace, or is pinned to one project. On a
project's dashboard, every widget looks at that project and its sub-projects
unless it names another.

Inside a project, its menu (the project's name in the header) lists the
dashboards pinned to it under **Dashboards**; choosing one opens it. **Pin a
dashboard here…** (or **All dashboards…**) opens the Dashboards page from the
project, and the dashboard menu (**⋯**) then has **Pin to** and the project's
key, which pins the dashboard shown to it. **Rename or describe…** also changes
what a dashboard is pinned to, or makes it the whole workspace's again.

## Your first dashboard

With no dashboard yet, the page offers **Create a starter dashboard**: open
tasks, overdue and due soon, open tasks by status, tasks created against
closed, and the latest changes. **Create an empty one** starts with nothing.

The dashboard menu (**⋯**) makes another dashboard, renames or describes the one
shown, or deletes it. Undo, or revert in the activity log, brings a deleted
dashboard back, and the same goes for every change to its widgets.

## Widgets

| Widget | Shows | Settings |
|---|---|---|
| Matching tasks | How many tasks match a query; **Open as a list** opens the list with it | query, project |
| Task list | The first tasks matching a query | query, project, rows |
| Tasks by field | Open tasks counted by status, priority, type, label or a custom select or yes/no field, as bars; each bar opens its tasks. A query narrows the open tasks; one that chooses a state itself (`is:closed`, `category:done`, `status:Done`) counts those instead | field, query, project |
| Due soon | Open tasks due in the next days (7 unless set) | query, project, days, rows |
| Overdue | Open tasks past their due date, oldest first | query, project, rows |
| Created and closed | The flow report of tasks created against closed | query, project, periods |
| Cumulative flow | The tasks in each status, per period | query, project, periods |
| Cycle time | How long tasks took from start to close | query, project, periods |
| Time logged | The time logged in the last days (7 unless set), grouped | project, days, group by |
| Sprint burndown | The burndown of the project's running sprint | project |
| Version progress | How far the project's open versions have come | project |
| Recent activity | The latest changes, by everyone, people, agents or rules | project, who, days, rows |
| Automation failures | Rules' runs that failed or were limited | project, rows |
| Note | A note in Markdown | text |

The query is the [task query language](../query-language.md): `is:open
label:bug`, `due:this-week`, `sort:-priority`. Relative dates in it are worked
out each time the widget shows. Renaming a status, type, label or sprint
updates the queries of the widgets that look at its project, and of the
workspace's widgets whose query keeps them to that project (`project:TM`).
Renaming a project key updates every widget's query.

Periods are days (the last 30), weeks (the last 12) or months (the last 12).
A chart reads without seeing it, as on the **Reports** page: focus it and the
arrow keys go from period to period; **Table** shows the same figures as a
table.

A widget that can't be shown says why, for example a burndown when the project
has no running sprint, or a query that names a project that was deleted; the
other widgets still show.

## Arranging widgets

Widgets sit in a grid of three columns, each one to three columns wide; a
narrow window shows one column.

- **Move** a widget by dragging its handle (**⠿**), or focus the handle and use
  the arrow keys.
- **Resize** it by dragging its end edge, or focus the edge and use the arrow
  keys, Home and End.
- The widget menu (**⋯**) has **Move earlier**, **Move later**, **Make wider**,
  **Make narrower**, **Edit…** and **Remove**.
- **Add widget** chooses what a new widget shows, its title, width and
  settings.

What widgets show is worked out by Glyphkeel's engine, one widget at a time, and
follows every change as it happens, from the app, the command line or an agent.

## Export and import

An [archive](export-import.md) carries the dashboards pinned to each project
with the project, and an export of the whole workspace also carries the
workspace's own dashboards. On import, the projects widgets name are the
imported projects, and a project that comes in under a new key is named by
that key in the widgets' queries. A workspace dashboard whose name is already
taken in the workspace you import into is left out.

## From the command line and for agents

```sh
glyphkeel dashboard create --name Team --project TM --starter true
glyphkeel dashboard add-widget Team --project TM \
  --widget '{"kind": "query_count", "title": "Open bugs", "settings": {"query": "is:open label:bug"}}'
glyphkeel dashboard edit-widget Team 1 --project TM --width 2 --position 3
glyphkeel dashboard edit Team --project TM --pin-to ZZ    # pin it to another project
glyphkeel dashboard edit Team --project ZZ --clear pin_to # the whole workspace's again
glyphkeel dashboard show Team --project TM
glyphkeel dashboard show Team --project TM --json
glyphkeel dashboard widget Team 2 --project TM
glyphkeel dashboard list --all true
```

`show` prints each widget with its figures; with `--json` it prints everything
the widgets show. A widget is named by its ID or its place in the grid (`1` is
the first).

Agents read what widgets show with `list_dashboards`, `get_dashboard` and
`get_dashboard_widget`. With the standard profile they also arrange dashboards:
`create_dashboard`, `update_dashboard`, `delete_dashboard`,
`add_dashboard_widget`, `update_dashboard_widget` and
`remove_dashboard_widget`.
