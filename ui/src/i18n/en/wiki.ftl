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
wiki-linked-from = Linked from
wiki-no-backlinks = No pages link here.
wiki-tasks-linking = Tasks that link here
wiki-no-task-backlinks = No tasks link here.
wiki-references = Links on this page
wiki-missing = Missing
wiki-link-heading = { $page } › { $heading }
wiki-open-task = Open { $key }: { $title }

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
backlinks-open-page = Open the wiki page { $title }
backlinks-open-task = Open { $key }: { $title }
backlinks-open-comment = Open { $key }, where a comment links here: { $title }
