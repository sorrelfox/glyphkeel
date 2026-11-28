# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## My work: one board for the work of every project, a column for each status category.

view-my-work = My work
action-my-work = My work
# The list of saved My work views under the sidebar's My work entry.
sidebar-my-work-views = My work views
my-work-board-label = My work across projects
my-work-toolbar = My work options
# A column: a status category, whatever each project calls its statuses.
my-work-column = { $category ->
        [todo] To do
        [in_progress] In progress
       *[done] Done
    }
my-work-query-label = Filter My work with a query
my-work-query-placeholder = Filter: project:TM label:bug due<+7d
# The quick toggles, each a condition added to the query.
my-work-toggle-week = Due this week
my-work-toggle-sprint = In an active sprint
my-work-toggle-high = High priority
# The folder choice: which projects the board shows.
my-work-folder-label = Projects: { $folder }
my-work-folder-all = All projects
# The saved views' choice with no view chosen.
my-work-view-none = All my work
my-work-view-default = Show it first when My work opens
# The status chooser when a project has several statuses in the column a card is dropped into.
my-work-choose = Move { $key } to which status of { $project }?
my-work-move-to = Move to
my-work-move-done = { $key } moved to { $status }
my-work-move-refused = { $key } can't go into { $column }: its project has no status there.
my-work-summary = { $total ->
        [one] 1 task
       *[other] { $total } tasks
    }, { $open } open
# Closed tasks of more than two weeks ago, which the Done column leaves out until asked.
my-work-show-older = { $count ->
        [one] Show 1 older closed task
       *[other] Show { $count } older closed tasks
    }
my-work-empty = No work matches. Clear the query or the toggles to see more.
