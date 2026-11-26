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
view-time-report = Time report
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

## The workspace switcher, in the status bar

# The menu button's name for screen readers; $name is the open workspace.
workspace-menu = Workspace { $name }: switch to another
workspace-new = New workspace…
workspace-rename = Rename this workspace…
workspace-new-title = New workspace
workspace-rename-title = Rename “{ $name }”
workspace-name = Name
workspace-create = Create and open
workspace-rename-confirm = Rename
workspace-new-note = A workspace keeps its own projects, tasks and wiki, apart from the others. It opens in place of this one; the command line and agents work in it too.
workspace-rename-note = Agents and commands working in this workspace must stop first.
workspace-opened = Opened the workspace “{ $name }”.
workspace-renamed = The workspace is called “{ $name }” now.
workspace-no-data-folder = The data folder could not be found.

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
action-time-report = Time report
action-view-board = Show the project as a board
action-view-list = Show the project as a list
action-view-calendar = Show the project as a calendar
action-previous-period = Previous period
action-next-period = Next period
action-go-to-today = Go to today
action-view-gantt = Show the project as a Gantt chart
action-view-wiki = Show the project's wiki
action-timer-toggle = Start or stop the timer
action-gallery = Themes and components
action-leave-gallery = Back to the app

## Preferences

preferences-title = Preferences
preferences-pages = Preference pages
preferences-page-appearance = Appearance
preferences-page-time = Time
preferences-page-notifications = Notifications
preferences-page-workspace = Workspace
preferences-workspace = Workspace
preferences-workspace-name = Name
preferences-workspace-name-about = The name of the workspace's folder.
preferences-workspace-location = Location
preferences-workspace-location-unknown = Shown in the desktop app
preferences-program-version = Program version
preferences-schema-version = Database version
preferences-zone-group = Time zone
preferences-time-zone = Time zone
preferences-time-zone-about = All times show in this zone; reminders and "today" follow it too. System follows the computer.
preferences-time-zone-system = System
preferences-time-zone-system-named = System ({ $zone })
preferences-time-zone-offset = UTC{ $offset }
preferences-time-zone-list = Show the time zones
preferences-time-zone-none = No time zone matches.
preferences-calendar = Calendar
preferences-week-start = First day of the week
preferences-week-start-about = For calendars, the time report and "this week" in searches.
preferences-working-time = Working time
preferences-hours-per-day = Working hours per day
preferences-hours-per-day-about = How long a day is in estimates and logged time, such as 2d.
preferences-days-per-week = Working days per week
preferences-days-per-week-about = How long a week is, such as 1w.
preferences-timer = Timer
work-kinds-title = Kinds of work
work-kinds-about = Time entries and the timer can be marked with one, and the time report groups by them. A kind that entries use is archived rather than deleted.
work-kinds-name = Name of { $name }
work-kinds-entries =
    { $count ->
        [0] No entries
        [one] { $shown } entry
       *[other] { $shown } entries
    }
work-kinds-archived = Archived
work-kinds-delete = Delete { $name }
work-kinds-deleted = Kind of work { $name } deleted
work-kinds-new = New kind of work
work-kinds-add = Add
preferences-timer-reminder = Remind about a running timer after
preferences-timer-reminder-about = A timer still running after this long gets a reminder. 0 never reminds.
preferences-reminders = Reminders
preferences-reminder-time = Reminders for due dates at
preferences-reminder-time-about = The time of day a reminder on a due date comes, unless it has its own.
preferences-notifications = Show reminders as desktop notifications
preferences-notifications-about = When off, reminders are kept in the app's log only.
preferences-background = Background
preferences-run-in-background = Keep running in the background
preferences-run-in-background-about = Closing the last window keeps Glyphkeel running for reminders and the timer, with an icon in the tray. Quit ends it.
preferences-autostart = Start when you log in
preferences-autostart-about = With background mode on, Glyphkeel starts with only its tray icon, for reminders; otherwise its window opens.
# On Windows.
preferences-autostart-windows = Start when you sign in
preferences-autostart-windows-about = With background mode on, Glyphkeel starts with only its icon in the notification area, for reminders; otherwise its window opens. The Startup apps list in Windows Settings can turn it off too.
preferences-rendering = Rendering
preferences-safe-rendering = Safe rendering
preferences-safe-rendering-about = Draws without the graphics card. Turn it on if windows stay blank, white or flicker. Scrolling is less smooth and Glyphkeel uses more memory.
preferences-safe-rendering-on-next-start = Safe rendering starts the next time Glyphkeel starts.
preferences-safe-rendering-off-next-start = Glyphkeel draws with the graphics card again the next time it starts.
preferences-restart-now = Restart now
preferences-trash = Trash
preferences-trash-days = Keep items in the trash for
preferences-unit-hours =
    { $count ->
        [one] hour
       *[other] hours
    }
preferences-unit-days =
    { $count ->
        [one] day
       *[other] days
    }
# The weekdays' names (weekday-monday…) come with the engine's messages.

## Toasts

toast-region = Notifications
# After a status, type, label or project key was renamed, or a label deleted:
# the saved views whose queries named it were changed to match ($names is a
# list such as "Open bugs and This week").
views-rewritten = { $count ->
    [one] The saved view { $names } was updated to match.
   *[other] The saved views { $names } were updated to match.
}
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
shortcut-group-screen = This view
screen-key-wiki-edit = Edit the page
screen-key-wiki-move-sibling = Move the page up or down in the tree
screen-key-wiki-move-in = Move the page into the page above it
screen-key-wiki-move-out = Move the page out of its parent
screen-key-gantt-move = Move the bar by a day
screen-key-gantt-resize = Make the bar a day shorter or longer
screen-key-gantt-edit-start = Edit the start date
screen-key-gantt-edit-due = Edit the due date
screen-key-gantt-link = Link to another task
screen-key-gantt-zoom-in = Zoom in
screen-key-gantt-zoom-out = Zoom out
screen-key-calendar-open-day = Go into the day's tasks
screen-key-calendar-move-day = Move the task by a day
screen-key-calendar-move-week = Move the task by a week

## Key names in shortcut hints

key-ctrl = Ctrl
key-alt = Alt
key-shift = Shift
key-super = Super
# The Windows logo key, in shortcut hints on Windows.
key-windows = Win
key-escape = Esc
key-delete = Delete
key-enter = Enter
key-space = Space
key-left = ←
key-right = →
key-up = ↑
key-down = ↓
key-page-up = Page Up
key-page-down = Page Down
key-home = Home
key-end = End

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
view-calendar = Calendar
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
project-key-change = Change…
project-key-new = New key
project-key-save = Change key
# $old is the key now. Task keys with it keep working after the change.
project-key-note = 2 to 10 capital letters and digits, starting with a letter. Keys such as { $old }-1 keep working in links, addresses, the command line, agents and queries; saved queries change to the new key.
project-key-old =
    { $count ->
        [one] Earlier key: { $keys }
       *[other] Earlier keys: { $keys }
    }
project-colour = Colour
project-icon = Icon
project-icon-circle = Circle
project-icon-folder = Folder
project-icon-star = Star
project-icon-bug = Bug
project-icon-rocket = Rocket
project-icon-book = Book
project-icon-house = House
project-icon-briefcase = Briefcase
project-icon-code = Code
project-icon-terminal = Terminal
project-icon-globe = Globe
project-icon-heart = Heart
project-icon-flag = Flag
project-icon-zap = Lightning
project-icon-target = Target
project-icon-lightbulb = Light bulb
project-icon-wrench = Wrench
project-icon-hammer = Hammer
project-icon-palette = Palette
project-icon-music = Music
project-icon-camera = Camera
project-icon-graduation-cap = Graduation cap
project-icon-shopping-cart = Shopping cart
project-icon-wallet = Wallet
project-icon-chart-line = Line chart
project-icon-calendar = Calendar
project-icon-users = People
project-icon-building = Building
project-icon-plane = Plane
project-icon-leaf = Leaf
project-icon-coffee = Coffee
project-icon-gamepad = Game controller
project-icon-puzzle = Puzzle
project-icon-shield = Shield
project-icon-flask-conical = Flask
project-icon-layers = Layers
project-icon-package = Package
project-icon-compass = Compass
project-icon-mountain = Mountain
project-icon-sparkles = Sparkles
project-icon-database = Database
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
toast-project-key-changed = Project key changed
toast-project-archived = Project archived
dialog-cancel = Cancel
duplicates-title =
    { $count ->
        [one] Close the duplicate too?
       *[other] Close the duplicates too?
    }
duplicates-body =
    { $count ->
        [one] { $key } is marked as the original of { $keys }, which is still open.
       *[other] { $key } is marked as the original of { $count } open tasks: { $keys }.
    }
duplicates-close =
    { $count ->
        [one] Close it too
       *[other] Close them too
    }
duplicates-keep =
    { $count ->
        [one] Leave it open
       *[other] Leave them open
    }
settings-add = Add

status-name = Name
status-category = Category
status-colour = Colour
status-wip = WIP limit
status-wip-minimum = WIP minimum
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
label-add-colour = Colour of the new label

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
lane-wip-range = { $count } / { $min }–{ $limit }
lane-wip-min = { $count } / { $min }+
lane-label-wip-range =
    { $count ->
        [one] { $name }, 1 task, WIP { $min } to { $limit }
       *[other] { $name }, { $count } tasks, WIP { $min } to { $limit }
    }
lane-label-wip-min =
    { $count ->
        [one] { $name }, 1 task, WIP minimum { $min }
       *[other] { $name }, { $count } tasks, WIP minimum { $min }
    }
lane-label-under =
    { $count ->
        [one] { $name }, 1 task, below its WIP minimum of { $min }
       *[other] { $name }, { $count } tasks, below its WIP minimum of { $min }
    }
lane-under-minimum =
    { $count ->
        [one] 1 task: fewer than the WIP minimum of { $min }
       *[other] { $count } tasks: fewer than the WIP minimum of { $min }
    }
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
# A custom field on a card: its name (the person's own words) and its value.
card-custom-field = { $field }: { $value }
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
label-search = Find or create a label
label-search-placeholder = Find or create…
label-create-named = Create label “{ $name }”
label-create-title = New label
label-create-first = This project has no labels yet. The new one goes on this task.
label-create = Create
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
panel-move = Move to another project…

## Moving tasks to another project

move-to-title =
    { $count ->
        [one] Move to another project
       *[other] Move { $count } tasks to another project
    }
move-to-project = Project
move-to-choose-project = Choose a project
# A project in the list of targets.
move-to-project-option = { $key } · { $name }
move-to-no-project = There is no other active project to move to.
move-to-checking = Checking what the move takes…
move-to-nothing = These tasks are in that project already.
# $subtasks is how many subtasks move with the task, at any depth.
move-to-keys-one =
    { $subtasks ->
        [0] { $from } becomes { $to }.
        [one] { $from } becomes { $to }, and its subtask moves with it.
       *[other] { $from } becomes { $to }, and its { $subtasks } subtasks move with it.
    } Its comments, files, time and links come along, and the old keys still find it.
move-to-keys-many =
    { $count ->
        [one] One task gets a new key in { $project }.
       *[other] { $count } tasks, subtasks included, get new keys in { $project }.
    } Their comments, files, time and links come along, and the old keys still find them.
move-to-all-match = { $project } has a status, type and label of the same name for each of them.
move-to-map-intro = { $project } has nothing of the same name for these. Choose what they become there:
move-to-map-statuses = Statuses
move-to-map-types = Types
move-to-map-labels = Labels
# $name is a status, type or label of the tasks; $count how many of them have it.
move-to-map-row =
    { $count ->
        [one] { $name } (1 task)
       *[other] { $name } ({ $count } tasks)
    }
move-to-map-choice = What “{ $name }” becomes
move-to-leave-off = Leave it off
move-to-fields-left-off = { $project } has no field of the same name and kind for these, so their values are left off:
# $name is a custom field of the tasks; $count how many of them have a value of it.
move-to-fields-row =
    { $count ->
        [one] { $name } (1 task)
       *[other] { $name } ({ $count } tasks)
    }
# The target project requires these custom fields; $project is its key.
move-to-required = { $project } requires these fields; give the moving tasks a value:
move-to-required-defaults = { $project } requires these fields; the moving tasks get their defaults:
move-to-versions-left-off = { $project } can't plan tasks for these versions, so they are taken off:
# $name is a version the tasks are planned for; $count how many of them.
move-to-versions-row =
    { $count ->
        [one] { $name } (1 task)
       *[other] { $name } ({ $count } tasks)
    }
move-to-confirm = Move
move-to-done = Moved: the task is { $key } now.
move-to-done-many =
    { $count ->
        [one] Moved one task to { $project }.
       *[other] Moved { $count } tasks to { $project }.
    }
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
links-lag =
    { $days ->
        [one] +1 day
       *[other] +{ $days } days
    }
links-lag-none = No lag
links-lag-label = Lag in days
links-lag-edit =
    { $days ->
        [0] Lag of the link to { $key }: none. Change it
        [one] Lag of the link to { $key }: 1 day. Change it
       *[other] Lag of the link to { $key }: { $days } days. Change it
    }
links-lag-changed = Lag changed
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
comment-quote = Quote in reply
# The last line of a quote in a reply: where the quoted text comes from. { $link } is the comment's link, such as TM-41#c-1a2b3c4d.
comment-quote-source = — { $link }
comment-link-gone = The linked comment is no longer on { $key }.
comment-draft-kept = Unsaved changes to this comment are kept from earlier.
comment-draft-continue = Continue editing
comment-draft-discard = Discard them
composer-label = Write a comment
composer-placeholder = Write a comment. Ctrl+Enter sends.
composer-send = Comment
via-app = app
via-cli = command line
via-mcp = MCP

history-title = History
history-none = No changes recorded.
history-by = { $actor } · { $via } · { $time }
history-by-reverted = { $actor } · { $via } · { $time } · reverted
history-created = Created { $key }
history-changed = { $field }: { $before } → { $after }
# A long value the activity log keeps only the beginning of; the item itself
# holds the whole text.
history-value-cut = { $value }…
history-set = { $field }: { $value }
history-cleared = { $field } cleared
history-description = Description changed
history-moved = Moved in its column
history-subtask-moved = Moved among its parent's subtasks
history-older = Show older changes
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
history-field-claim = Claim
history-field-reminder = Reminder
history-field-name = Name
history-field-hue = Colour
history-field-icon = Icon
history-field-modules = Parts
history-field-key = Key
history-field-state = State
history-field-category = Category
history-field-wip_limit = WIP limit
history-field-wip_min = WIP minimum
history-field-work_kind = Kind of work
history-work-kind-gone = a deleted kind of work
history-field-glyph = Shape
history-field-is_default = Default
history-field-slug = Address
history-field-body = Text
history-field-date = Day
history-field-minutes = Duration
history-field-note = Note
history-field-agent_brief = Agent brief
history-field-home = Home page
history-field-template = Template

## Preferences: appearance

preferences-theme = Theme
preferences-accent = Accent colour
preferences-density = Density
preferences-accent-about = Used for selection, focus and the main action.
preferences-interface = Interface
preferences-density-about = Compact fits more rows and cards on screen.
preferences-reduce-motion = Reduce motion
preferences-reduce-motion-about = Follows the desktop setting when off here.
theme-system = System
accent-system = From the desktop

## Time: the Time tab, the timer and quick log

time-title = Time
time-total = Total
time-log = Log time
time-log-save = Log
time-none = No time logged yet.
time-entries = Time entries
time-duration = Duration
time-duration-invalid = Type a duration from 1 minute to 24 hours, such as 1h 30m, 90m, 1,5h or 1:30.
time-day = Day
time-note = Note
time-note-placeholder = What was done
time-work-kind = Kind of work
time-no-work-kind = No kind of work
timer-kind = Kind of work for the timer
timer-start-as = Start the timer as
time-entry-label = { $day }, { $duration }, by { $author }
time-entry-menu = Time entry actions
time-edit = Edit
time-edit-label = Edit the entry of { $day }
time-delete = Delete
time-deleted = Time entry deleted
time-quick = +30m
time-quick-label = Log 30 minutes for today
time-logged-quick = 30 minutes logged on { $key }
timer-start = Start timer
timer-stop = Stop the timer and log the time
timer-pause = Pause the timer
timer-resume = Go on with the timer
timer-running-label = Timer running, { $time }. Stop and log the time
timer-paused-label = Timer paused at { $time }. Stop and log the time
timer-logged = { $time } logged on { $key }
timer-nothing-logged = The timer ran under half a minute; no time was logged.
timer-chip-label = Timer
timer-chip-open = Timer running, { $time }, on { $key }: { $title }. Open the task
timer-chip-open-paused = Timer paused at { $time }, on { $key }: { $title }. Open the task
timer-chip-open-long = Timer running for more than { $hours } hours, { $time }, on { $key }: { $title }. Open the task

## Reminders

prop-reminder = Reminder
reminder-title = Reminder
reminder-kind = When it reminds
reminder-at = At a time
reminder-due = Before the due date
reminder-moment = Date and time
reminder-days = Days before the due date
reminder-days-before =
    { $count ->
        [one] day before due, at
       *[other] days before due, at
    }
reminder-before-due =
    { $count ->
        [0] on the due date
        [one] 1 day before due
       *[other] { $count } days before due
    }
reminder-time = Time of day
reminder-time-default = Empty: the workspace's reminder time, { $time }
reminder-no-due = The task has no due date yet; the reminder waits for one.
reminder-note = Reminder text
reminder-note-placeholder = Text of the reminder (optional)
reminder-set = Set reminder
reminder-remove = Remove

## History of time and reminders

history-time-logged = Time logged: { $time }
history-time-edited = Time entry changed
history-time-deleted = Time entry deleted
history-time-restored = Time entry restored
history-reminder-set = Reminder set
history-reminder-cleared = Reminder removed

## The time report

report-from = First day
report-to = Last day
report-this-week = This week
report-last-week = Last week
report-this-month = This month
report-last-month = Last month
report-project = Project
report-all-projects = All projects
report-group-by = Group by
report-this-year = This year
report-subprojects = Include sub-projects
report-by-day = Day
report-by-week = Week
report-by-month = Month
report-by-task = Task
report-by-project = Project
report-by-person = Person
report-by-work_kind = Kind of work
report-work-kind = Kind of work
report-all-work-kinds = All kinds of work
report-no-work-kind = No kind of work
report-agent = Agent
report-export = Export
report-export-groups = Export the rows as CSV
report-export-entries = Export the entries as CSV
report-exported = Time report exported
# The name of a saved file, without its extension; { $from } and { $to } are days such as 2026-11-01.
report-file-groups = time-report-{ $from }-{ $to }
report-file-entries = time-entries-{ $from }-{ $to }
report-entries = Entries
report-time = Time
report-share = Share of the time
report-total = Total
report-none = No time logged in these days.
report-table = Time logged from { $from } to { $to }
