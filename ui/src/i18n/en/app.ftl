# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The app's own text, in English. Every word the UI shows comes from these
# files. Sentence case for all labels.

app-name = Glyphkeel
# The window title names what the window shows.
window-title = Glyphkeel: { $context }

## Views

view-today = Today
view-all-tasks = All tasks
view-gallery = Themes and components

## Sidebar

sidebar-label = Workspace
sidebar-search = Search
sidebar-projects = Projects
sidebar-new-project = New project
sidebar-no-projects = No projects yet
sidebar-trash = Trash
sidebar-preferences = Preferences
sidebar-resize = Sidebar width

## Header bar

header-new-task = New task
header-main-menu = Main menu
window-controls = Window
window-minimize = Minimize
window-maximize = Maximize
window-close = Close

## Content

content-label = Content
empty-today = Nothing is due today.
empty-all-tasks = No tasks yet.

## Status bar

status-label = Status
status-tasks =
    { $count ->
        [0] No tasks
        [one] One task
       *[other] { $count } tasks
    }
status-workspace = Local workspace
# The open workspace's name.
status-workspace-name = { $name }
status-workspace-unavailable = Workspace unavailable

## Actions: their names in menus, tooltips and the shortcut sheet

action-palette = Search or run a command
action-new-task = New task
action-toggle-sidebar = Show or hide the sidebar
action-next-region = Move focus to the next region
action-previous-region = Move focus to the previous region
action-undo = Undo
action-redo = Redo
action-backup = Back up now
action-import = Import…
action-export = Export…
action-shortcuts = Keyboard shortcuts
action-preferences = Preferences
action-about = About Glyphkeel
action-close-window = Close the window
action-quit = Quit
action-today = Today
action-all-tasks = All tasks
action-gallery = Themes and components
action-leave-gallery = Back to the app

## Preferences

preferences-title = Preferences
preferences-workspace = Workspace
preferences-workspace-name = Name
preferences-program-version = Program version
preferences-schema-version = Database version
preferences-time = Calendar and working time
preferences-week-start = First day of the week
preferences-hours-per-day = Working hours per day
preferences-days-per-week = Working days per week
preferences-reminder-time = Reminders for due dates at
preferences-trash-days = Keep items in the trash for
preferences-days =
    { $count ->
        [one] One day
       *[other] { $count } days
    }
weekday-monday = Monday
weekday-saturday = Saturday
weekday-sunday = Sunday

## Toasts

toast-region = Notifications
toast-undo = Undo
toast-settings-changed = Settings changed
# The app couldn't reach its workspace at all (not an error of an operation).
error-ipc = The workspace could not be reached

## Shortcut sheet

shortcuts-title = Keyboard shortcuts
shortcuts-close = Close
shortcuts-none = No shortcut
shortcut-group-app = Application
shortcut-group-navigation = Navigation
shortcut-group-tasks = Tasks
shortcut-group-window = Window

## Key names in shortcut hints

key-ctrl = Ctrl
key-alt = Alt
key-shift = Shift
key-super = Super
key-escape = Esc
key-delete = Delete
key-enter = Enter
key-space = Space
key-left = ←
key-right = →
key-up = ↑
key-down = ↓

## Appearance

theme-light = Light
theme-dark = Dark
theme-slate = Slate
theme-sand = Sand
theme-contrast = Contrast
theme-note-light = Neutral. Used when the desktop is light.
theme-note-dark = Neutral. Used when the desktop is dark.
theme-note-slate = Colour theme: cool blue-grey, dark.
theme-note-sand = Colour theme: warm paper, light.
theme-note-contrast = Accessibility: strong borders, darkest text.
accent-blue = Blue
accent-teal = Teal
accent-green = Green
accent-amber = Amber
accent-orange = Orange
accent-red = Red
accent-pink = Pink
accent-purple = Purple
accent-slate = Slate
density-regular = Regular
density-compact = Compact

## Start-up failures

startup-failed-title = Glyphkeel could not start
startup-ipc-fallback = The window can't reach the app over its own channel and has fallen back to a slower one, on which larger answers never arrive. The app's security policy is blocking the channel; check the policy's connect sources.

## Views of a project

view-board = Board
view-list = List
view-gantt = Gantt
view-wiki = Wiki
header-views = Project views
# The project button in the header bar; it opens the project menu.
header-project-menu = { $name }: project menu

## Project tree and projects

sidebar-archived = Archived
project-menu = Project
project-open = Open
project-new-subproject = New sub-project…
project-settings = Project settings…
project-archive = Archive
project-unarchive = Unarchive
project-new-title = New project
project-new-subproject-title = New sub-project of { $parent }
project-create = Create project
project-name = Name
project-key = Key
project-colour = Colour
project-description = Description
project-modules = Parts
project-state = State
project-settings-title = { $name }: settings
project-settings-general = General
project-settings-statuses = Statuses
project-settings-types = Task types
project-settings-labels = Labels
module-board = Board
module-list = List
module-gantt = Gantt
module-wiki = Wiki
module-time = Time tracking
toast-project-changed = Project changed
toast-project-archived = Project archived
dialog-cancel = Cancel
settings-add = Add

status-name = Name
status-category = Category
status-colour = Colour
status-wip = WIP limit
status-move-up = Move up
status-move-down = Move down
status-delete = Delete status
status-delete-title = Delete { $name }
status-delete-move = Move the tasks of { $name } to
status-delete-target = Status for its tasks
status-delete-confirm = Delete { $name }
status-add = New status
category-todo = To do
category-in_progress = In progress
category-done = Done
category-cancelled = Cancelled
type-name = Name
type-glyph = Shape
type-default = Default
type-delete = Delete { $name }
type-add = New task type
glyph-task = Task
glyph-bug = Bug
glyph-feature = Feature
glyph-milestone = Milestone
label-name = Name
label-colour = Colour
label-none = No labels yet.
label-delete = Delete { $name }
label-delete-confirm = Delete { $name } and remove it from its tasks
label-add = New label

hue-red = Red
hue-orange = Orange
hue-yellow = Yellow
hue-green = Green
hue-teal = Teal
hue-blue = Blue
hue-purple = Purple
hue-pink = Pink
hue-grey = Grey

## Priorities

priority-none = No priority
priority-low = Low
priority-normal = Normal
priority-high = High
priority-urgent = Urgent

## Board

board-label = Board of { $name }
board-toolbar = Board
board-summary =
    { $total ->
        [0] No tasks
        [one] One task · { $open } open
       *[other] { $total } tasks · { $open } open
    }
board-summary-filtered = { $shown } of { $total } tasks
board-selected = { $key } selected
board-filtered = Filtered: { $shown } of { $total } tasks
board-clear-filter = Clear filter
board-archived = This project is archived. Unarchive it to change it.
board-include-subprojects = Include sub-projects
board-hidden =
    { $columns ->
        [one] 1 hidden column
       *[other] { $columns } hidden columns
    } · { $tasks ->
        [one] 1 task
       *[other] { $tasks } tasks
    }
board-hidden-label = { board-hidden }. Show hidden columns
board-show-column = { $name } ({ $count })
board-reset-layout = Reset to project layout
board-sort-label = Sort: { $mode }
board-options = Board options: card fields, card size, closed cards
board-card-fields = Card fields
board-card-size = Cards
board-hide-done = Hide closed cards after
board-hide-done-never = Never
board-hide-done-days =
    { $days ->
        [one] 1 day
       *[other] { $days } days
    }
card-field-labels = Labels
card-field-due = Due date
card-field-checklist = Checklist
card-field-subtasks = Subtasks
card-field-start = Start date
card-field-estimate = Estimate
card-field-days_in_column = Days in column
card-size-regular = Regular
card-size-compact = Compact
sort-manual = Manual order
sort-priority = Priority
sort-due = Due date
sort-updated = Last updated

filter-label = Filter tasks
filter-placeholder = Filter by title or key
filter-labels = Label
filter-priority = Priority
filter-due = Due
filter-due-overdue = Overdue
filter-due-today = Due today
filter-due-week = Due in the next 7 days
filter-due-none = No due date
filter-recent = Recently updated
filter-recent-short = Recent
filter-active = { $name }: { $count } chosen

lane-label =
    { $count ->
        [one] { $name }, 1 task
       *[other] { $name }, { $count } tasks
    }
lane-label-wip = { $name }, { $count } of { $limit } tasks
lane-wip = { $count } / { $limit }
lane-add = Add task to { $name }
lane-menu = { $name }: column menu
lane-add-task = Add task
lane-hide = Hide
lane-collapse = Collapse
lane-move-left = Move left
lane-move-right = Move right
lane-sort = Sort
lane-show-older = Show older ({ $count })
lane-new-task-title = Title of the new task in { $name }
lane-new-task-placeholder = Task title, Enter to add
lane-collapsed =
    { $count ->
        [one] { $name }, 1 task, collapsed. Expand column
       *[other] { $name }, { $count } tasks, collapsed. Expand column
    }

card-label = { $key }: { $title }
card-checklist = { $done } of { $total } checklist items done
card-subtasks = { $done } of { $total } subtasks done
card-start = Starts { $day }
card-estimate = { $time } estimated
card-days-in-column =
    { $count ->
        [one] 1 day in this column
       *[other] { $count } days in this column
    }
card-menu = Task
card-open = Open
card-move-to = Move to
card-priority = Priority
card-copy-key = Copy key
card-blocked = Blocked
card-claimed = { $agent }
card-claimed-label = Claimed by { $agent }
card-comments =
    { $count ->
        [one] 1 comment
       *[other] { $count } comments
    }
card-files =
    { $count ->
        [one] 1 file
       *[other] { $count } files
    }
card-spent = { $time } spent
due-today = Today

move-done = { $key } moved to { $column }
move-done-position = { $key } moved to position { $position } in { $column }
move-refused = { $key } can't go to { $column }: its project has no such status.
move-sorted = { $column } is sorted; choose Manual order to move cards within it.
task-created = { $key } added to { $column }

drag-instructions = To pick up a task, press Space. Use the arrow keys to move it between and within columns. Press Space again to drop it, or Escape to cancel.
drag-start = Picked up { $key }.
drag-over = { $key } is over { $column }.
drag-end = { $key } dropped in { $column }.
drag-cancel = Moving { $key } was cancelled.

action-card-left = Move the task to the column on the left
action-card-right = Move the task to the column on the right
action-card-up = Move the task up
action-card-down = Move the task down
action-column-left = Move the column left
action-column-right = Move the column right

## Task panel

panel-label = Task { $key }
panel-close = Close panel
panel-copy-key = Copy key
panel-copied = Copied { $key }
panel-title = Title
panel-status = Status: { $name }. Change status
panel-status-menu = Status
panel-description = Description
panel-no-description = No description.
panel-edit = Edit
panel-save = Save
prop-priority = Priority
prop-priority-value = Priority: { $value }
prop-type = Type
prop-type-value = Type: { $value }
prop-start = Start
prop-due = Due
prop-estimate = Estimate
prop-spent = Spent
prop-labels = Labels
prop-labels-edit = Change labels
prop-progress = Progress
prop-none = None
prop-left = { $time } left
prop-over-by = over by { $time }
prop-date-value = { $name }: { $value }
prop-duration-placeholder = e.g. 1h 30m

## The task panel's bar, sections and tabs

panel-own-window = Open in its own window
panel-close-window = Close window
panel-menu = Task menu
panel-duplicate = Duplicate
panel-duplicate-alone = Duplicate without subtasks
panel-duplicated = Task duplicated
panel-release = Release the agent's claim
panel-blocked-by =
    { $count ->
        [0] Blocked
        [one] Blocked by { $keys }
       *[other] Blocked by { $keys }
    }
panel-claimed = Claimed by { $agent } until { $time }
task-window-missing = This task can't be shown.

subtasks-title = Subtasks
subtasks-add = Add subtask
subtasks-new = New subtask
subtasks-new-placeholder = Title of the subtask
subtasks-open = Open { $key }: { $title }
subtasks-close = Close { $key }
subtasks-reopen = Reopen { $key }
subtasks-menu = Subtask { $key }
subtasks-move-up = Move up
subtasks-move-down = Move down

links-title = Links
links-add = Add link
links-none = No links.
links-kind = Kind of link
links-other = Task key
links-other-placeholder = Key, e.g. TM-12
links-open = { $kind } { $key }: { $title }
links-remove = Remove link: { $kind } { $key }
links-removed = Link removed
relation-blocks = Blocks
relation-blocked_by = Blocked by
relation-relates = Relates to
relation-duplicates = Duplicates
relation-duplicated_by = Duplicated by
relation-clones = Copy of
relation-cloned_by = Copied as
relation-subtask_of = Subtask of
relation-parent_of = Parent of

files-title = Files
files-attach = Attach files
files-none = No files. Drop files here to attach them.
files-attached =
    { $count ->
        [one] File attached
       *[other] { $count } files attached
    }
files-attach-failed = { $name } wasn't attached
files-drop-here = Drop to attach to { $key }
files-open = Open
files-open-named = Open { $name }
files-save = Save a copy…
files-remove = Remove
files-removed = Removed { $name }
files-menu = File { $name }

activity-tabs = Comments and history
comments-title = Comments
comments-none = No comments yet.
comment-me = Me
comment-agent = Agent · { $via }
comment-label = Comment by { $author }, { $time }
comment-menu = Comment
comment-edit = Edit
comment-edit-field = Comment text
comment-delete = Move to the trash
comment-trashed = Comment moved to the trash
comment-edited =
    { $count ->
        [one] edited · 1 earlier version
       *[other] edited · { $count } earlier versions
    }
comment-earlier = Earlier versions
comment-replaced = Replaced { $time } by { $actor }
composer-label = Write a comment
composer-placeholder = Write a comment. Ctrl+Enter sends.
composer-send = Comment
via-app = app
via-cli = command line
via-mcp = MCP

history-title = History
history-none = No changes recorded.
history-by = { $actor } · { $via } · { $time }
history-created = Created { $key }
history-changed = { $field }: { $before } → { $after }
history-set = { $field }: { $value }
history-cleared = { $field } cleared
history-description = Description changed
history-moved = Moved in its column
history-claimed = Claimed by { $agent }
history-released = Claim released
history-comment-added = Comment added
history-comment-edited = Comment edited
history-comment-trashed = Comment moved to the trash
history-comment-restored = Comment restored
history-file-attached = File attached: { $name }
history-file-trashed = File removed
history-file-restored = File restored
history-link-added = Link added: { $kind } { $key }
history-link-removed = Link removed: { $kind } { $key }
history-another-task = another task
history-revert = Revert the change of { $time }
history-reverted-toast = Change reverted
history-not-reverted = The change wasn't reverted
history-was-reverted = reverted
history-field-title = Title
history-field-description = Description
history-field-status = Status
history-field-type = Type
history-field-priority = Priority
history-field-labels = Labels
history-field-parent = Parent
history-field-start_date = Start
history-field-due_date = Due
history-field-estimate = Estimate
history-field-progress = Progress
history-field-rank = Place
history-field-project = Project

## Preferences: appearance

preferences-appearance = Appearance
preferences-theme = Theme
preferences-accent = Accent colour
preferences-density = Density
theme-system = System
accent-system = From the desktop
