<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Links in Markdown

Descriptions, comments and wiki pages are Markdown. Besides ordinary links, the reader
recognises links to tasks, to comments and to wiki pages written as plain text.

## Tasks

A task key on its own links to the task: `TM-41`. A key is a project key (a capital letter,
then 1 to 9 capital letters or digits), a hyphen and the task's number. It must stand on its
own: `xTM-41`, `TM-41x` and `TM-41-b` are not links. Keys inside code, inside other links
and keys of tasks that don't exist stay plain text.

## Comments

A task key followed by `#c-` and the last eight hexadecimal digits of a comment's ID, in
lower case, links to that comment of the task:

```markdown
TM-41#c-1a2b3c4d
```

The link opens the task and scrolls to the comment. Anything else after `#c-` (fewer or more
digits, capital letters) leaves the task key linked on its own and the rest as text. A
comment's ID is listed by `glyphkeel comment list <key>` and by the MCP tool `get_task`
with its comments.

**Quote-reply** writes this form. Quoting a comment in the app puts the selected text of the
comment, or the whole comment, in the reply as a block quote that ends with the link:

```markdown
> The hall is free in May.
>
> — TM-41#c-1a2b3c4d

Let's take the hall.
```

A comment that links to another comment of the same task is sent as an answer to it.

## Wiki pages

`[[Page]]`, `[[Page#Heading]]` and `[[Page#Heading|text]]` link to a wiki page of the
same project.
`[[KEY:Page]]` links to a page of project `KEY`; a page of the same project whose title is
`KEY:Page` wins.

## Wiki macros

A wiki page can hold macros. A macro stands alone on its line, as a paragraph of its own;
anywhere else, in code, or after a backslash (`\{{toc}}`) it is plain text. Macros work in
wiki pages only; in descriptions and comments they stay as written.

| Macro | Shows |
|---|---|
| `{{toc}}` | The page's headings as links, nested by level. |
| `{{children}}` | Links to the pages directly inside the page, in their order. |
| `{{include Page}}` | Another page's text, as it is now. `Page` is a title, slug or `KEY:Page`. |
| `{{tasks <query>}}` | The tasks a query finds, as they are now: key, title and status. |

- **Includes** go at most three levels deep (a page includes a page that includes a page),
  and one page includes at most 20 pages in all. A page is never included inside itself.
  Checkboxes of an included page are shown but can only be ticked on that page.
- **Task lists** use the query language of the task lists and list up to 50 tasks, then say
  how many more match. A query that names no project (`project:KEY`) looks in the page's
  project and the projects below it. The list follows changes to tasks while the page is
  open. An agent limited to one project sees only that project's tasks and pages through a
  macro, as everywhere else.
- **Exports:** a web page export (`text_format: html`) shows the macros expanded, as they
  were at the time of the export. A Markdown export keeps them as written, so the exported
  text works again when it is imported.
- **Agents** read a page's Markdown, with its macros as written; `get_wiki_page` doesn't
  expand them.

## Page templates

Any page can be marked as a template (`glyphkeel wiki put <page> --template true`, MCP
`update_wiki_page` with `template: true`, or the page menu in the app). A new page can start
from a template's text: the app's new-page dialog offers the project's templates,
`glyphkeel wiki new --template <page>` and MCP `create_wiki_page` with `template` take any
page, also `KEY:Page` of another project. The new page is a copy: changing it leaves the
template alone, and it is not a template itself.
