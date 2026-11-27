<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Export and import

## Archives

An archive holds projects as JSON and Markdown files, with their attached files.
It is readable without Glyphkeel, and it is the way to move projects between
workspaces or computers. To copy a whole workspace as it is, use a
[backup](../backup-and-restore.md) instead.

### Exporting

Choose **Export…** in the **Main menu**, in a project's menu, or in Preferences ›
Data and backups › **Export and import**. Then choose:

- **What to export**: **The whole workspace**, or **Chosen projects**. Each
  project is exported with its sub-projects;
- the **Format**: **One .zip file** or **A folder**.

Glyphkeel then shows what it wrote: projects, tasks, comments, files, wiki pages
and time entries. Each project's automation rules and the dashboards pinned to
it go with it, and so does the sidebar folder of a top-level project (an import
puts it in the folder of the same name, made if needed); an export of the whole workspace also takes the project
templates, the workspace's own automation rules and its own dashboards. Rules start afresh when they are imported (see
[Automation rules](automation.md#export-and-import)).

### Importing

Choose **Import…** and pick a .zip file or a folder that Glyphkeel exported. The
archive is checked completely before anything is imported. The projects of the
archive are added as new projects; nothing that is in the workspace already
changes.

If a project key of the archive is taken in this workspace, give that project a
new key under **Taken project keys**. The queries that name the old key follow
the new one: those of saved views, automation rules, board colour rules and
dashboard widgets, in `project:OLD` and in task keys such as `OLD-12`.

From the command line:

```sh
glyphkeel export --path backup-2026-11.zip                # the whole workspace
glyphkeel export --path home.zip --projects HOME
glyphkeel import --dry-run home.zip                       # check, import nothing
glyphkeel import home.zip
```

## Tasks from a CSV file

Glyphkeel imports tasks from CSV files, such as exports of Jira or Redmine.
Choose **Import tasks from a CSV file…** in the **Main menu**, or **Import from
CSV…** in a project's menu.

1. **Choose a CSV file.** Its encoding, separator and date order are detected;
   you can change each.
2. **Preset:** **Detect from the columns**, **Jira**, **Redmine**, or one you
   saved. A preset maps the columns of that tool.
3. **Columns:** for each column, choose where it **Goes to**: title,
   description, status, type, priority, labels, version, start or due date,
   estimate, time spent, comment, a custom field, the key in the other tool, a
   parent task or a link (blocks, relates, duplicates and so on), or **Not
   imported**. For durations, choose the unit.
4. Choose whether to **Create missing statuses, types and labels**, and whether
   to **Leave out rows with errors**.

**What the import makes** shows how many tasks the file gives, the new statuses,
types and labels, and **Rows with problems**. Rows with errors stop the import
until you fix the mapping or leave them out.

The import is one change: a single Undo takes it all back. Parent tasks and
links between the imported rows are kept. Repeating tasks are not read from the
file; each row becomes one task that doesn't repeat.

**Save as preset** keeps your mapping for the next file from the same tool.

From the command line:

```sh
glyphkeel import csv issues.csv --project TM --preset jira --dry-run
glyphkeel import csv issues.csv --project TM --preset jira
glyphkeel csv-preset list
```

## Other exports

- **The list** exports its tasks as CSV or JSON ([The list](list.md#export)).
- **The wiki** exports pages as Markdown files or web pages, and imports a folder
  of Markdown files ([The wiki](wiki.md#export)).
- **The Gantt chart** exports a PNG image or a PDF
  ([Gantt chart](calendar-and-gantt.md#export)).
- **Reports** export CSV ([Time tracking and reports](time.md)).
