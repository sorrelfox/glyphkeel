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
