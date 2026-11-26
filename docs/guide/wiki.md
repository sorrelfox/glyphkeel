<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The wiki

Each project has a wiki: Markdown pages in a tree. Open it with **Wiki** in the
header bar, or Ctrl+4.

## Pages

- **New page** adds a page at the top of the tree. **New page inside**, in a
  page's menu, adds one inside it. A title is unique in its project.
- Choose **Edit**, or press E, to change a page. Ctrl+Enter saves and Esc
  discards. Unsaved text is kept on this device until you save. When saving,
  you can add a **Note about this change**.
- **Rename** changes the title. Links that use the old title keep finding the
  page.
- **Move to trash** moves the page, with the pages inside it, to the trash.
- **Open in its own window** shows the page in a window of its own.

**Arranging the tree:** drag pages, or use **Move up**, **Move down**, **Move
into the page above** and **Move out of its parent** in the page menu. With the
keyboard: Alt+Up and Alt+Down move a page among its siblings, Alt+Right moves it
into the page above, and Alt+Left moves it out of its parent.

**Home page:** **Make this the home page** makes the wiki open on that page.
Otherwise it opens on its first page.

The side panel, **About this page**, shows the page's outline (**On this
page**), the pages and tasks that link to it (**Linked from**, **Tasks that link
here**) and the links on the page.

## Revisions

**History** lists the page's revisions. Open one to read it, compare it with
another, or **Restore this revision**. If someone else, such as an agent,
changed the page while you edited it, Glyphkeel merges the changes. When it
can't, your text is kept as a conflict copy, which you resolve at the top of the
page.

## Links

| Write | Links to |
|---|---|
| `[[Page]]` | A page of the same project |
| `[[Page#Heading]]` | A heading on a page |
| `[[Page#Heading\|text]]` | The same, shown as *text* |
| `[[KEY:Page]]` | A page of project `KEY` |
| `TM-41` | A task |
| `TM-41#c-1a2b3c4d` | A comment of a task |

The editor completes pages after `[[` and tasks after `#`. A link to a page that
doesn't exist yet is marked; follow it to create the page. Each heading has a
button to copy a link to it. [Links in Markdown](../reference/links.md) has the
details.

## Macros

A macro stands alone on its line. The editor completes macros after `{{`.

| Macro | Shows |
|---|---|
| `{{toc}}` | The page's headings as links |
| `{{children}}` | Links to the pages inside this page |
| `{{include Page}}` | Another page's text, as it is now |
| `{{tasks <query>}}` | A live list of the tasks a query finds |

For example, a release page could hold:

```markdown
## Still open

{{tasks fix-version:"1.2" is:open sort:priority}}
```

Includes go at most three levels deep, and a page includes at most 20 pages. A
task list shows up to 50 tasks. A query without `project:` looks in the page's
project and its sub-projects.

## Page templates

**Use as a template**, in a page's menu, offers the page as a starting point.
When you make a new page, **Start from** lists the project's templates, or
**A blank page**. The new page is a copy; changing it leaves the template alone.

## Export

**Export…**, in a page's menu, exports **This page**, the page **With the pages
inside**, or **The whole wiki**, as:

- **Markdown files**, which read well in other editors, with the attached files.
  Links between the exported pages become relative links. Macros stay as
  written, so the files can be imported again;
- **Web pages**, which open in any browser without the app, with the attached
  files and a list of the pages. Macros show their result at the time of the
  export.

From the command line: `glyphkeel wiki export <page>`.

## Import

**Import a folder of Markdown files…** (in the wiki), or **Import a folder into
this page…** (in a page's menu), makes a page of each Markdown file:

- a sub-folder becomes a page that holds the pages of its files;
- relative links between the files, and `[[…]]` links, become wiki links;
- linked pictures and files are attached to their pages.

Glyphkeel checks the folder first and shows **What the import makes**, including
pages it had to rename. The import is one change, so a single Undo takes it
back.

## The agent brief

**Use as the agent brief** marks a page as the project's instructions for
agents. Agents read it before they work on the project. See
[Working with AI agents](../mcp.md).

## When the wiki is switched off

If a project's **Parts** don't include the wiki, its pages can be read but not
changed. Switch it on in the project's settings.
