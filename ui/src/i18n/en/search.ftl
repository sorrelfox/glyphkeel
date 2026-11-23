# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The search page, the sidebar's search field, and wiki pages opened read
# only from a link or a search result.

view-search = Search
sidebar-search-label = Search the workspace
sidebar-search-results = Suggestions
sidebar-search-everything = Search everything for “{ $query }”

## The search page

search-field = Search
search-placeholder = Words, "a phrase", or a key such as TM-41
search-kinds = What to search
search-kind-all = All
search-kind-task = Tasks
search-kind-comment = Comments
search-kind-wiki = Wiki pages
search-kind-file = Files
search-kind-project = Projects
search-kind-one-task = Task
search-kind-one-comment = Comment
search-kind-one-wiki = Wiki page
search-kind-one-file = File
search-kind-one-project = Project
search-project = Project
search-all-projects = All projects
search-hint = Type words to search tasks, comments, wiki pages, files and projects.
search-nothing = Nothing found for “{ $text }”.
search-count = { $count ->
    [one] { $count } result
   *[other] { $count } results
  }
search-results = Search results
search-more = Show more
search-where = { $project } · { $status }
search-in-comment = Comment:

## A wiki page, read only

wiki-page = Wiki page
wiki-loading = Loading the page…
wiki-not-found = There is no wiki page “{ $page }”.
wiki-updated = Changed { $time }
wiki-empty = This page has no text yet.
wiki-text = Text of { $page }
wiki-read-only = Wiki pages open read only here; editing them comes with the wiki’s own screens.
