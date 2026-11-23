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
