<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# History, undo and the trash

Glyphkeel records every change, by you, the command line or an agent. Almost
everything can be undone, and deleted items wait in the trash.

## Undo and redo

**Undo** (Ctrl+Z) takes back the last change you made in the app; **Redo**
(Ctrl+Shift+Z) makes it again. Both are also in the **Main menu**. A change that
was several steps at once, such as a bulk change or an import, is undone as one.
In a text field, Ctrl+Z undoes your typing instead.

Some messages after a change, for example after moving a bar in the Gantt chart,
have an **Undo** button too.

## The history of a task

The **History** tab of a task lists its changes, newest first: who made each one
(you or an agent), through which program (app, command line or MCP), and when.
Each change has **Revert**, which undoes that change alone. A change that
something else changed since is not reverted, and Glyphkeel says so.

Descriptions and wiki pages also keep every revision: see
[Tasks](tasks.md#the-description) and [the wiki](wiki.md#revisions).

## The activity log

**Activity** in the sidebar lists the changes to the whole workspace. Filter it
by **Project**, by **Who** (**Everyone**, **People** or **Agents**) and by dates
(**From**, **To**). Each row has **Revert**.

To take back everything an agent did in one session, use **Revert session…** in
Preferences › Agents & CLI ([Working with AI agents](../mcp.md#undo-and-review)).

From the command line:

```sh
glyphkeel activity
glyphkeel revert <group>            # a change, by its group ID
glyphkeel revert --session <id> --dry-run
```

## The trash

Deleting a task, project, wiki page, comment, file or time entry moves it to the
**Trash**, with everything that went with it: a task's subtasks, a project's
tasks, a page's sub-pages.

In **Trash** in the sidebar:

- filter by **Kind of item** and **Project**;
- **Restore** brings the chosen items back, with everything that was deleted
  with them;
- **Delete for good** removes them, and their text from the history. This can't
  be undone, and Glyphkeel asks first;
- **Empty trash** deletes everything in it for good.

Items stay in the trash for 30 days, then they are deleted for good. Change the
number of days in Preferences › Workspace › **Trash**.

From the command line:

```sh
glyphkeel trash list
glyphkeel trash restore <item>
glyphkeel trash purge <item>
```

Agents with the standard profile can move items to the trash but not delete them
for good ([Profiles](../mcp.md#profiles)).
