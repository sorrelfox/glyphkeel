# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Messages of the engine's search, Today and wiki operations and of dates,
# in English. The engine renders them in English for the command line and
# agents and adds the message's ID and arguments to the error, so the app
# can show its own translation. One message per line; placeables are
# { $name }.

## Lists

list-limit = Ask for 1 to { $max } items at once.
list-cursor = This is not a cursor from an earlier page.

## Search

search-empty = Type something to search for.

## Dates

date-invalid = "{ $text }" is not a date. Write 2026-12-31, or today, fri, next mon, in 3d, +3d.

## Revisions

revision-not-found = There is no revision "{ $revision }" of this text.

## Wiki

wiki-title-empty = A page needs a title.
wiki-title-too-long = The title is { $length } characters long; the limit is { $max }.
wiki-title-reserved = A page title can't contain "{ $char }", because links to pages use it.
wiki-title-taken = Project { $project } already has a page called "{ $title }".
wiki-page-not-found = Project { $project } has no page "{ $page }".
wiki-page-id-not-found = There is no page with the ID { $page }.
wiki-project-needed = Name the project of page "{ $page }", or give the page's ID.
wiki-parent-loop = A page can't be put inside itself or inside a page it holds.
wiki-parent-other-project = The parent page "{ $page }" is in another project.
wiki-too-deep = Pages can be nested at most { $max } deep.
wiki-sibling-not-found = "{ $page }" is not a page next to this one.
wiki-body-and-edits = Give a new body or edits, not both.
