# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The command palette, the quick-add window and its desktop-wide shortcut.

## Command palette

palette-title = Command palette
palette-field = Search tasks, projects and actions
palette-placeholder = Search tasks, projects and actions
palette-results = Results
palette-scope = Limit results
palette-scope-all = All
palette-scope-tasks = Tasks
palette-scope-projects = Projects
palette-scope-actions = Actions
palette-group-recent = Recent
palette-group-actions = Actions
palette-group-tasks = Tasks
palette-group-projects = Projects
palette-kind-task = Task
palette-kind-project = Project
palette-go-to-project = Go to { $project }
palette-empty = Nothing matches.
palette-searching = Searching…
palette-hint-move = Move
palette-hint-open = Open
palette-hint-window = Open in a new window
palette-hint-scope = Show only
palette-hint-key = Type a key such as TM-41 to jump
palette-key-ctrl-enter = Ctrl Enter
palette-key-tab = Tab

## Quick add: the window the desktop-wide shortcut opens

action-quick-add = Quick add
quick-add-title = Quick add
quick-add-field = New task
quick-add-placeholder = Add a task. Try: Renew domain fri #Personal !high
quick-add-syntax = #project picks the project, !high the priority, and a date at the end the due date.
quick-add-project = Project
quick-add-due = Due { $words }
quick-add-keep-date = Keep “{ $words }” in the title
quick-add-unknown-project = No project “{ $token }”
quick-add-hint-add = Add
quick-add-hint-close = Close
quick-add-added = { $key } added
quick-add-need-title = Type a title first.
quick-add-no-projects = Make a project first; new tasks go into one.

## Preferences: the quick-add shortcut

preferences-page-quick-add = Quick add
preferences-quick-add = Quick add
preferences-shortcut = Shortcut for quick add
preferences-shortcut-about = Opens the quick-add window from any program while Glyphkeel runs, also in the background.
preferences-shortcut-keys = Keys
preferences-shortcut-first-time = Your desktop may ask you once to allow the shortcut, and may let you choose other keys there.
preferences-shortcut-state-waiting = Waiting for the desktop. If it shows a dialog, answer it there.
preferences-shortcut-state-portal = Registered with the desktop as { $trigger }.
preferences-shortcut-state-grab = Registered as { $keys }.
preferences-shortcut-state-manual = This desktop doesn't let Glyphkeel have the keys itself. Add a shortcut in the desktop's keyboard settings that runs this command:
preferences-shortcut-state-refused = The desktop's request was declined, so the shortcut is off. Turn it on to be asked again, or add a shortcut in the desktop's keyboard settings that runs this command:
preferences-shortcut-unavailable = The shortcut belongs to the desktop app.
preferences-shortcut-try-again = Ask again
