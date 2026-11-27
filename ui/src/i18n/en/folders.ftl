# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Project folders: one level of groups for top-level projects in the
## sidebar. A folder holds projects, never other folders.

sidebar-new-folder = New folder
activity-entity-project_folder = Folder
history-field-folder = Folder

folder-new-title = New folder
folder-rename-title = Rename folder
folder-name = Name
folder-colour = Colour
folder-no-colour = No colour
folder-create = Create folder
folder-save = Save
# The menu of a folder row in the sidebar.
folder-menu = Folder
folder-rename = Rename…
folder-move-up = Move up
folder-move-down = Move down
folder-delete = Delete folder
# The open tasks of all projects in a folder, read out on its row.
folder-open-tasks = { $count ->
    [one] { $count } open task
   *[other] { $count } open tasks
}

# In the project menu: put a top-level project into a folder, or take it out.
project-move-to-folder = Move to folder
project-move-no-folder = No folder
# In the project menu: one place up or down within its folder (or among the projects in no folder).
project-move-up = Move up
project-move-down = Move down

toast-folder-created = Folder created
toast-folder-changed = Folder changed
toast-folder-moved = Folder moved
toast-folder-deleted = { $count ->
    [0] Folder deleted
    [one] Folder deleted; its project is back at the top level
   *[other] Folder deleted; its { $count } projects are back at the top level
}
toast-project-moved-to-folder = Project moved
