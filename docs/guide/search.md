<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Today, search and the palette

## Today

**Today** in the sidebar shows what needs attention across all projects:

- **Overdue** tasks and tasks **Due today**;
- tasks **In progress**;
- today's **Reminders**;
- **Agents at work**: tasks that agents have claimed, and until when;
- tasks **Changed in the last day**, and by whom;
- the **Timer**, when one runs.

The sidebar shows the counts next to **Today**. From the command line:
`glyphkeel today`.

## Search

The **Search** field at the top of the sidebar suggests tasks, pages and
projects as you type. Enter, or **Search everything for …**, opens the search
view with all results.

The search finds words and parts of words in task titles and descriptions,
comments, wiki pages, file names and projects, and marks the matches. Use
`"a phrase"` for words in order, or a key such as `TM-41` to go to that task.
Narrow the results by **What to search** (Tasks, Comments, Wiki pages, Files or
Projects) and by **Project**.

From the command line: `glyphkeel search "release notes"`.

Search finds text. To filter tasks by their fields, use a
[query](../query-language.md) in the list.

## The command palette

Ctrl+K opens the palette. It searches tasks, projects and actions at once:

- type words to find a task, a project or an action such as *Back up now*;
- type a key such as `TM-41` to jump to the task;
- Tab limits the results to **Tasks**, **Projects** or **Actions**;
- the up and down arrow keys move, Enter opens, and Ctrl+Enter opens a task in a
  new window.

With an empty field, the palette shows what you opened recently.

<img src="../screenshots/today.png" alt="The command palette open over the Today view in the dark theme, listing matching actions, tasks and projects" width="900">

## Quick add

Quick add captures a task without leaving what you are doing. Open it from the
**Main menu**, from the tray icon (**Quick Add…**), or with its global shortcut,
**Ctrl+Alt+Space** by default, from any program while Glyphkeel runs.

Type the title, and add to it:

- `#Project` picks the project, by its key or by its name without spaces
  (`#TM`, `#Personal`, `#homelab` for *Home lab*);
- `!high` sets the priority: `!urgent`, `!high`, `!normal` or `!low`, or `!1`
  (urgent) to `!4` (low);
- a date at the end sets the due date, such as `fri`, `tomorrow` or `dec 1`.

For example: `Renew domain fri #Personal !high`. The window shows what it
understood before you press Enter. If a date is meant to stay in the title,
choose **Keep “…” in the title**.

Change the shortcut in Preferences › **Quick add**. On some Linux desktops the
desktop asks once to allow it, or Glyphkeel can't hold the keys itself; then
Preferences shows the command to bind in the desktop's keyboard settings:
`glyphkeel-app --quick-add`.
