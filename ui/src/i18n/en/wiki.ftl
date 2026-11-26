# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

# The wiki: the page tree, a page with its outline and links, its editor
# and history, wiki windows, and what links to a task.

## The page tree

wiki-pages = Pages
wiki-new-page = New page
wiki-no-pages = No pages yet.
wiki-page-count =
    { $count ->
        [one] { $count } page
       *[other] { $count } pages
    }
wiki-page-menu = Page menu
wiki-open = Open
wiki-open-window = Open in its own window
wiki-open-in-wiki = Open in the wiki
wiki-new-child = New page inside
wiki-rename = Rename
wiki-delete = Move to trash
wiki-brief-set = Use as the agent brief
wiki-brief-unset = Stop using as the agent brief
wiki-brief-set-done = Agents now get this page as the project's brief.
wiki-brief-unset-done = This page no longer briefs agents.
wiki-home = Home page
wiki-home-set = Make this the home page
wiki-home-unset = Stop using as the home page
wiki-home-set-done = The wiki now opens on this page.
wiki-home-unset-done = The wiki opens on its first page again.
# A page marked as a template is offered when a new page is made.
wiki-template = Template
wiki-template-set = Use as a template
wiki-template-unset = Stop using as a template
wiki-template-set-done = New pages can now start from this page.
wiki-template-unset-done = This page is no longer offered as a template.
# The field of the new-page dialog that picks a template.
wiki-template-field = Start from
wiki-template-blank = A blank page
wiki-export = Export…
wiki-import = Import a folder of Markdown files…
wiki-import-into = Import a folder into this page…
# The name the save dialog suggests for a whole wiki's export, without ".zip".
wiki-export-file = { $project }-wiki
wiki-exported =
    { $count ->
        [one] Page exported as Markdown
       *[other] { $count } pages exported as Markdown
    }
wiki-exported-html =
    { $count ->
        [one] Page exported as a web page
       *[other] { $count } pages exported as web pages
    }
wiki-deleted = Page moved to the trash
wiki-move-up = Move up
wiki-move-down = Move down
wiki-move-in = Move into the page above
wiki-move-out = Move out of its parent
wiki-drag = Drag { $page }
wiki-expand = Show the pages inside { $page }
wiki-collapse = Hide the pages inside { $page }
wiki-agent-brief = Agent brief
wiki-empty-text = This project has no wiki pages yet.
wiki-empty-off = This project has no wiki pages, and its wiki is switched off.
wiki-files-drop-here = Drop to attach to { $page }

## A page

wiki-page-label = Wiki page { $page }
wiki-window-title = { $page } · { $project }
wiki-breadcrumbs = Place in the wiki
wiki-changed = Changed { $time }
wiki-history = History
wiki-revisions = Revisions
wiki-edit = Edit
wiki-read-only-note = The wiki is switched off in this project: its pages can be read, not changed. Switch it on in the project's settings.
wiki-draft-kept = Unsaved changes to this page are kept from earlier.
wiki-draft-note = Unsaved text is kept on this device until you save · Ctrl+Enter saves · Esc discards
wiki-discard = Discard
wiki-save-message = Note about this change
wiki-save-message-hint = What changed (optional)
wiki-merged = Saved, merged with a change made meanwhile.
wiki-conflict-saved = Your edit clashed with another change.
wiki-conflict-detail = It is kept as a conflict copy; resolve it at the top of the page.
wiki-conflict =
    { $count ->
        [one] An edit of this page clashed with another change and is kept as a conflict copy.
       *[other] { $count } edits of this page clashed with other changes and are kept as conflict copies.
    }
wiki-restore = Restore this revision
wiki-restored = The revision is restored as the page's newest.
wiki-missing-page = There is no page “{ $page }” yet.

## The aside

wiki-aside = About this page
wiki-outline = On this page
wiki-copy-heading-link = Copy a link to { $heading }
wiki-heading-link-copied = Link to the heading copied
wiki-linked-from = Linked from
wiki-no-backlinks = No pages link here.
wiki-tasks-linking = Tasks that link here
wiki-no-task-backlinks = No tasks link here.
wiki-references = Links on this page
wiki-missing = Missing
wiki-link-heading = { $page } › { $heading }

## New page and rename

wiki-title = Title
wiki-create = Create
wiki-new-child-title = New page in { $parent }
wiki-new-in = It goes inside { $parent }.
wiki-new-at-top = It goes at the top of the tree.
wiki-rename-title = Rename { $page }
wiki-rename-note = Links that use the old title keep finding the page.

## What links to a task (the task panel)

backlinks-title = Linked from
backlinks-none = No pages, tasks or comments link here.
backlinks-kind-page = Wiki page
backlinks-kind-task = Description
backlinks-kind-comment = Comment

## Export and folder import

wiki-export-title = Export { $page }
wiki-export-scope = What to export
wiki-export-scope-page = This page
wiki-export-scope-subpages = With the pages inside
wiki-export-scope-wiki = The whole wiki
wiki-export-format = Export as
wiki-export-format-markdown = Markdown files
wiki-export-format-html = Web pages
wiki-export-markdown-about = Markdown files that read well in other editors, with the attached files. Links between the exported pages become relative links.
wiki-export-html-about = Web pages that open in any browser without the app, with the attached files and a list of the pages. Links between the exported pages keep working.
wiki-import-title = Import a folder of Markdown files
wiki-import-into-title = Import a folder into { $page }
wiki-import-about = Each Markdown file becomes a page, and each sub-folder a page that holds the pages of its files. Links between the files become wiki links, and the pictures and files they link to are attached to their pages. Existing pages stay as they are.
wiki-import-choose = Choose a folder…
# In a browser without the app: a folder in the workspace's exports folder.
wiki-import-name = Folder in the exports folder
wiki-import-checking = Checking the folder…
wiki-importing = Importing…
wiki-import-summary = What the import makes
wiki-import-makes =
    { $pages ->
        [one] One page
       *[other] { $pages } pages
    } { $files ->
        [0] and no attached files.
        [one] and one attached file.
       *[other] and { $files } attached files.
    }
wiki-import-start =
    { $count ->
        [one] Import one page
       *[other] Import { $count } pages
    }
wiki-import-renamed =
    { $count ->
        [one] One title is taken already, so the page gets a number:
       *[other] { $count } titles are taken already, so their pages get a number:
    }
wiki-import-renamed-one = { $path } → { $title }
wiki-import-skipped =
    { $count ->
        [one] One file is left out: it is neither Markdown nor linked from a page.
       *[other] { $count } files are left out: they are neither Markdown nor linked from a page.
    }
wiki-imported =
    { $count ->
        [one] One page imported
       *[other] { $count } pages imported
    }
