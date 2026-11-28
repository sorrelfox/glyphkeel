# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Generated from the operation catalog by `cargo xtask gen`. Do not edit:
# change the operation's summary or its field's description instead.
#
# The help of the command line's commands (op-<operation>) and of their
# options (op-<operation>--<option>). Words in backquotes are commands,
# options, values or query syntax: keep them as they are.

## glyphkeel workspace

op-workspace-get = The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings, the limits on agents and a short guide.

## glyphkeel settings get

op-settings-get = Read the workspace settings.

## glyphkeel settings set

op-settings-update = Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder.
op-settings-update--clear = Settings to set back to their default.
op-settings-update--days-per-week = Working days in a week, 1 to 7.
op-settings-update--file-max-mb = The largest file that can be attached, in megabytes, 1 to 1024.
op-settings-update--hours-per-day = Working hours in a day, 1 to 24.
op-settings-update--reminder-time = Reminder time, `HH:MM` (24-hour).
op-settings-update--timer-reminder-hours = Hours a timer runs before it reminds that it is still running, 0
    (never) to 24.
op-settings-update--trash-days = Days in the trash before purging, 1 to 3650.
op-settings-update--week-start = First day of the week.

## glyphkeel search <text>

op-search-query = Search task titles and descriptions, comments, wiki pages, file names and the text inside attached files for words and fragments, ranked, with the matches marked; a task key finds that task.
op-search-query--cursor = The `next_cursor` of the page before.
op-search-query--include-subprojects = With `project`: its sub-projects too.
op-search-query--kinds = Only these kinds; default: all.
op-search-query--limit = Results per page, 1 to 200; default 50.
op-search-query--project = Only this project (key or ID).
op-search-query--query = Only tasks matching this task query (`is:open label:bug`), with
    their comments and files; wiki pages are left out.
op-search-query--text = Words (each found as a whole word or a word's beginning) and
    "quoted phrases"; all must occur. A task key such as `TM-41` also
    finds that task.

## glyphkeel project list

op-project-list = The project tree in sidebar order, with keys, names and open-task counts; archived projects on request.
op-project-list--include-archived = Also list archived projects.

## glyphkeel project show <project>

op-project-get = One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts.
op-project-get--project = The project: its key (current or old) or ID.

## glyphkeel project create

op-project-create = Create a project, optionally in a folder, or a sub-project that copies its parent's statuses, types and labels; without a colour it gets one its sibling projects don't use yet.
op-project-create--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of creating a second project.
op-project-create--description = The description (Markdown).
op-project-create--folder = The folder to put the new top-level project in (ID or name), as
    its last project; default: no folder. Not with `parent`: a
    sub-project follows its parent.
op-project-create--hue = The colour; default: the first palette colour that no sibling
    project uses yet (the least-used one when all are taken).
op-project-create--icon = An icon shown instead of the circle; default: none (the circle).
op-project-create--key = The key: 2 to 10 capital letters and digits, starting with a letter
    (`TM`). Task keys start with it.
op-project-create--name = The name, 1 to 100 characters.
op-project-create--parent = The parent, for a sub-project (at most 3 levels). It copies the
    parent's statuses, types and labels.

## glyphkeel project edit <project>

op-project-update = Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change.
op-project-update--after = Place it after this sibling project.
op-project-update--archived = Archive (`true`) or unarchive (`false`) it with its sub-projects.
op-project-update--at = Place it first or last among its siblings.
op-project-update--before = Place it before this sibling project.
op-project-update--clear = Fields to clear.
op-project-update--description = A new description (Markdown).
op-project-update--hue = A new colour.
op-project-update--icon = A new icon, shown instead of the circle. To go back to the circle,
    list `icon` in `clear`.
op-project-update--key = A new key; the old one keeps finding the project and its tasks.
op-project-update--labels = Labels to add, change or remove, in order.
op-project-update--modules = Modules to switch on or off.
op-project-update--name = A new name, 1 to 100 characters.
op-project-update--parent = Move it under this project (at most 3 levels, never under its own
    sub-project). To make it a top-level project, list `parent` in
    `clear`.
op-project-update--project = The project: its key (current or old) or ID.
op-project-update--statuses = Statuses to add, change, reorder or remove, in order.
op-project-update--types = Task types to add, change, reorder or remove, in order.

## glyphkeel project move <project>

op-project-move = Move a project under another one (at most 3 levels), to the top level, or among its siblings.
op-project-move--after = Place it after this sibling project.
op-project-move--at = Place it first or last among its siblings.
op-project-move--before = Place it before this sibling project.
op-project-move--parent = The new parent; with `top_level`, none.
op-project-move--project = The project: its key (current or old) or ID.
op-project-move--top-level = Make it a top-level project.

## glyphkeel project rekey <project>

op-project-change-key = Give a project a new key; the old key and the old task keys keep working.
op-project-change-key--key = The new key: 2 to 10 capital letters and digits, starting with a
    letter. The old key keeps finding the project and its tasks.
op-project-change-key--project = The project: its key (current or old) or ID.

## glyphkeel project archive <project>

op-project-archive = Archive a project and its sub-projects: they become read-only and leave the sidebar.
op-project-archive--project = The project: its key (current or old) or ID.

## glyphkeel project unarchive <project>

op-project-unarchive = Unarchive a project and its sub-projects.
op-project-unarchive--project = The project: its key (current or old) or ID.

## glyphkeel project delete <project>

op-project-delete = Move a project and its sub-projects, with everything in them, to the trash. Agents need the full profile.
op-project-delete--expected-task-count = The number of tasks you expect it and its sub-projects to hold,
    trashed ones included; the deletion stops if the number differs.
op-project-delete--project = The project: its key (current or old) or ID.

## glyphkeel project brief <project>

op-project-brief = The project's instructions for agents: the wiki page marked as the brief, else the project's description.
op-project-brief--project = The project: its key (current or old) or ID.

## glyphkeel folder list

op-folder-list = The project folders in sidebar order, each with its projects and their open tasks.

## glyphkeel folder create

op-folder-create = Make a folder to group top-level projects in the sidebar.
op-folder-create--after = Place it after this folder (ID or name).
op-folder-create--at = Place it first or last; default: last.
op-folder-create--before = Place it before this folder (ID or name).
op-folder-create--hue = Its colour; left out: none.
op-folder-create--name = Its name, unique among the folders.

## glyphkeel folder edit <folder>

op-folder-update = Rename a folder, change its colour or move it among the folders.
op-folder-update--after = Move it after this folder (ID or name).
op-folder-update--at = Move it first or last among the folders.
op-folder-update--before = Move it before this folder (ID or name).
op-folder-update--clear = Fields to clear.
op-folder-update--folder = The folder: its ID or its name.
op-folder-update--hue = A new colour.
op-folder-update--name = A new name.

## glyphkeel folder rename <folder> <name>

op-folder-rename = Rename a folder.
op-folder-rename--folder = The folder: its ID or its name.
op-folder-rename--name = Its new name.

## glyphkeel folder delete <folder>

op-folder-delete = Delete a folder; its projects stay and go back to the top level. Revert brings it back.
op-folder-delete--folder = The folder: its ID or its name.

## glyphkeel folder move <project>

op-folder-move = Put a top-level project into a folder or take it out, or move it within its folder.
op-folder-move--after = Place it after this project of the same folder.
op-folder-move--at = Place it first or last in the folder (or among the projects in no
    folder); default when it changes folder: last.
op-folder-move--before = Place it before this project of the same folder.
op-folder-move--folder = The folder to put it in (ID or name); left out with `none`: out of
    any folder, back at the top level; left out without `none`: it
    stays in its folder and only moves within it.
op-folder-move--none = Take it out of its folder.
op-folder-move--project = The project: its key (current or old) or ID. Only top-level
    projects go into folders; their sub-projects come along.

## glyphkeel status add

op-status-create = Add a status (a board column) to a project.
op-status-create--after = Place it after this status.
op-status-create--at = Place it first or last; default: last.
op-status-create--before = Place it before this status.
op-status-create--category = Its category; default: to do.
op-status-create--hue = Its colour; default: grey.
op-status-create--name = The name, 1 to 50 characters, unique in the project ignoring case.
op-status-create--project = The project: its key (current or old) or ID.
op-status-create--wip-limit = Its WIP limit, 1 or more.
op-status-create--wip-min = Its WIP minimum, 1 or more and not above the limit; the count shows
    a warning while the column holds fewer tasks.

## glyphkeel status edit

op-status-update = Rename or recolour a status, change its category or its WIP limit.
op-status-update--category = A new category. Tasks in it close or reopen with it.
op-status-update--clear = Fields to clear.
op-status-update--hue = A new colour.
op-status-update--name = A new name.
op-status-update--project = The project: its key (current or old) or ID.
op-status-update--status = The status: its name or ID.
op-status-update--wip-limit = A new WIP limit, 1 or more.
op-status-update--wip-min = A new WIP minimum, 1 or more and not above the limit.

## glyphkeel status move

op-status-reorder = Move a status in the project's board order.
op-status-reorder--after = Place it after this status.
op-status-reorder--at = Place it first or last.
op-status-reorder--before = Place it before this status.
op-status-reorder--project = The project: its key (current or old) or ID.
op-status-reorder--status = The status: its name or ID.

## glyphkeel status delete

op-status-delete = Delete a status; its tasks move to the status named. Can't be undone.
op-status-delete--move-tasks-to = The status its tasks move to; needed when it has tasks.
op-status-delete--project = The project: its key (current or old) or ID.
op-status-delete--status = The status: its name or ID.

## glyphkeel type add

op-type-create = Add a task type to a project.
op-type-create--due-offset-days = New tasks of this type are due this many days (0 to 3650) after
    the day they are made, unless given a due date.
op-type-create--glyph = Its shape; default: task.
op-type-create--hue = Its colour; default: grey.
op-type-create--make-default = Make it the type new tasks get.
op-type-create--name = The name, 1 to 50 characters, unique in the project ignoring case.
op-type-create--project = The project: its key (current or old) or ID.

## glyphkeel type edit

op-type-update = Rename, reshape, recolour or reorder a task type, or make it the default.
op-type-update--after = Place it after this type.
op-type-update--at = Place it first or last.
op-type-update--before = Place it before this type.
op-type-update--clear-due-offset = New tasks of this type get no due date of their own.
op-type-update--due-offset-days = New tasks of this type are due this many days (0 to 3650) after
    the day they are made, unless given a due date.
op-type-update--glyph = A new shape.
op-type-update--hue = A new colour.
op-type-update--make-default = Make it the type new tasks get.
op-type-update--name = A new name.
op-type-update--project = The project: its key (current or old) or ID.
op-type-update--type = The type: its name or ID.

## glyphkeel type delete

op-type-delete = Delete a task type; its tasks get another type. Can't be undone.
op-type-delete--move-tasks-to = The type its tasks get instead; default: the default type.
op-type-delete--project = The project: its key (current or old) or ID.
op-type-delete--type = The type: its name or ID. The default type can't be deleted.

## glyphkeel label add

op-label-create = Add a label to a project; without a colour it gets the first palette colour the project's labels don't use yet.
op-label-create--hue = Its colour; default: the first palette colour no label of the
    project uses yet, else the least-used one (the project's
    `next_label_hue`).
op-label-create--name = The name, 1 to 50 characters, unique in the project ignoring case.
op-label-create--project = The project: its key (current or old) or ID.

## glyphkeel label edit

op-label-update = Rename or recolour a label.
op-label-update--hue = A new colour.
op-label-update--label = The label: its name or ID.
op-label-update--name = A new name.
op-label-update--project = The project: its key (current or old) or ID.

## glyphkeel label delete

op-label-delete = Delete a label and take it off every task. Can't be undone.
op-label-delete--label = The label: its name or ID. It is taken off every task.
op-label-delete--project = The project: its key (current or old) or ID.

## glyphkeel version list <project>

op-version-list = The versions of a project and those its ancestors share with it, with status, dates and progress; with their tasks for the roadmap.
op-version-list--include-tasks = Include each version's tasks, for the roadmap.
op-version-list--own-only = Leave out the versions ancestors share with the project.
op-version-list--project = The project (key or ID): its own versions and those its ancestors
    share with it.
op-version-list--status = Only versions with these statuses; default: all.

## glyphkeel version show <version>

op-version-get = One version with its progress and its tasks.
op-version-get--project = With a name: the project (key or ID); its own versions come first,
    then those shared with it.
op-version-get--version = The version: its ID, or its name.

## glyphkeel version create

op-version-create = Create a version of a project: name, description, status, start and due dates, and whether its sub-projects can use it.
op-version-create--description = What it is for: plain text, up to 4,000 characters.
op-version-create--due-date = When it is due: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 1`.
op-version-create--name = Its name, up to 100 characters, unique in the project.
op-version-create--project = The project (key or ID).
op-version-create--sharing = Which projects can plan tasks for it; default: only its own.
op-version-create--start-date = When work starts: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 1`.
op-version-create--status = Where it stands; default: open.

## glyphkeel version edit <version>

op-version-update = Rename a version, change its description, dates, sharing or status, or close it and move its open tasks to another version.
op-version-update--clear = Fields to clear.
op-version-update--description = A new description.
op-version-update--due-date = A new due date.
op-version-update--move-open-tasks-to = When closing it: plan its open tasks for this other open version (its
    name or ID) instead, or `none` to plan them for no version. Left
    out, open tasks stay in the closed version.
op-version-update--name = A new name; saved views that name the old one are rewritten.
op-version-update--project = With a name: the version's project (key or ID).
op-version-update--sharing = New sharing. Tasks of projects that no longer see the version lose
    it.
op-version-update--start-date = A new start date.
op-version-update--status = A new status.
op-version-update--version = The version: its ID, or its name.

## glyphkeel version delete <version>

op-version-delete = Delete a version; its tasks are planned for no version. Revert brings both back.
op-version-delete--project = With a name: the version's project (key or ID).
op-version-delete--version = The version: its ID, or its name.

## glyphkeel sprint list <project>

op-sprint-list = The sprints of a project with their state, dates and progress; with their tasks and the backlog for the sprints page.
op-sprint-list--include-tasks = Include each sprint's tasks and the backlog, for the sprints page.
op-sprint-list--project = The project (key or ID).
op-sprint-list--state = Only sprints in these states; default: all.

## glyphkeel sprint show <sprint>

op-sprint-get = One sprint with its progress and its tasks.
op-sprint-get--project = With a name or `active`: the project (key or ID).
op-sprint-get--sprint = The sprint: its ID, its name, or `active` for the project's active
    sprint.

## glyphkeel sprint create

op-sprint-create = Plan a sprint in a project: name, goal, start and end dates, and tasks to put in it.
op-sprint-create--end-date = Its last day: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 12`.
op-sprint-create--goal = What it is meant to achieve: plain text, up to 4,000 characters.
op-sprint-create--name = Its name, up to 100 characters, unique in the project; not `active`,
    `none` or `backlog`.
op-sprint-create--project = The project (key or ID).
op-sprint-create--start-date = Its first day: `YYYY-MM-DD`, or `today`, `mon`, `+1w`, `dec 1`.
op-sprint-create--tasks = Tasks to put in it straight away (keys or IDs, of the project).

## glyphkeel sprint edit <sprint>

op-sprint-update = Rename a sprint or change its goal or dates.
op-sprint-update--clear = Fields to clear.
op-sprint-update--end-date = A new end date.
op-sprint-update--goal = A new goal.
op-sprint-update--name = A new name; saved views that name the old one are rewritten.
op-sprint-update--project = With a name or `active`: the sprint's project (key or ID).
op-sprint-update--sprint = The sprint: its ID, its name, or `active`.
op-sprint-update--start-date = A new start date.

## glyphkeel sprint delete <sprint>

op-sprint-delete = Delete a planned or closed sprint; its tasks go back to the backlog. Revert brings both back.
op-sprint-delete--project = With a name: the sprint's project (key or ID).
op-sprint-delete--sprint = The sprint: its ID or its name. An active sprint is completed
    first, not deleted.

## glyphkeel sprint start <sprint>

op-sprint-start = Start a planned sprint, with its dates; a project has one active sprint at a time.
op-sprint-start--end-date = Its last day; default: its end date, or two weeks after the start.
op-sprint-start--goal = A new goal.
op-sprint-start--project = With a name: the sprint's project (key or ID).
op-sprint-start--sprint = The sprint (planned): its ID or its name.
op-sprint-start--start-date = Its first day; default: its start date, or today.

## glyphkeel sprint complete <sprint>

op-sprint-complete = Complete the active sprint and move its open tasks to a planned sprint or back to the backlog.
op-sprint-complete--move-open-tasks-to = Where its open tasks go: a planned sprint of the project (name or
    ID), or `backlog` (the default) for no sprint. Closed tasks stay in
    the completed sprint.
op-sprint-complete--project = With a name or `active`: the sprint's project (key or ID).
op-sprint-complete--sprint = The sprint (active): its ID, its name, or `active`.

## glyphkeel sprint add <sprint>

op-sprint-add-tasks = Put tasks in a planned or active sprint of their project, moving them from any other sprint.
op-sprint-add-tasks--project = With a name or `active`: the sprint's project (key or ID).
op-sprint-add-tasks--sprint = The sprint (planned or active): its ID, its name, or `active`.
op-sprint-add-tasks--tasks = The tasks (keys or IDs), of the sprint's project; up to 500. A task
    in another sprint moves to this one.

## glyphkeel sprint remove

op-sprint-remove-tasks = Take tasks out of their sprint, back to the backlog.
op-sprint-remove-tasks--tasks = The tasks (keys or IDs); up to 500. Tasks in no sprint are left
    as they are.

## glyphkeel sprint burndown <sprint>

op-sprint-burndown = The burndown of an active or closed sprint: its open and total tasks and estimates at the end of each day.
op-sprint-burndown--project = With a name or `active`: the sprint's project (key or ID).
op-sprint-burndown--sprint = The sprint (active or closed): its ID, its name, or `active`.

## glyphkeel field list

op-field-list = The custom fields of a project (its own and those of every project) or of the whole workspace: name, query name, kind, options, default, task types, required, and how many tasks have a value.
op-field-list--project = The project: its key (current or old) or ID. Lists the fields its
    tasks have: its own and those of every project. Left out: every
    field of the workspace.

## glyphkeel field add

op-field-create = Add a custom field to a project, or to every project: text, Markdown, whole number, decimal, date, yes/no, select, multi-select or link; with options, a default, the task types it shows for, and whether it is required.
op-field-create--after = Place it after this field.
op-field-create--at = Place it first or last; default: last.
op-field-create--before = Place it before this field.
op-field-create--default = The value new tasks get unless told otherwise.
op-field-create--kind = What it holds.
op-field-create--name = The name, 1 to 50 characters with at least one letter or digit;
    unique among the fields the project's tasks have, ignoring case. A
    field of the same name in another project must hold the same kind.
op-field-create--options = The options of a select or multi-select field, in order (1 to 200,
    each 1 to 100 characters, unique ignoring case).
op-field-create--project = The project: its key (current or old) or ID. Left out: a field of
    every project. A project's sub-projects created later get a copy of
    its own fields.
op-field-create--required = Tasks it is shown for must have a value: a new task needs one (or
    the default), and a value can't be cleared.
op-field-create--types = Show it only for tasks of these types, by name; default: every type.

## glyphkeel field edit

op-field-update = Rename a custom field, rename, add, reorder or remove its options, change its default, task types or whether it is required, or move it; saved queries keep up with renames.
op-field-update--after = Move it after this field.
op-field-update--at = Move it first or last.
op-field-update--before = Move it before this field.
op-field-update--clear = Settings to clear.
op-field-update--default = A new default value.
op-field-update--field = The field: its name or ID.
op-field-update--name = A new name. Saved queries that use the field are rewritten.
op-field-update--options = The whole list of options after the renames, in order. Options left
    out are taken off every task.
op-field-update--project = The project whose fields to look in: its key (current or old) or
    ID; left out: the fields of every project.
op-field-update--rename-options = Options to rename; tasks and saved queries keep up.
op-field-update--required = Whether tasks must have a value.
op-field-update--types = Show it only for tasks of these types, by name.

## glyphkeel field delete

op-field-delete = Delete a custom field with its values on every task; saved queries lose their conditions on it. Can't be undone.
op-field-delete--expected-task-count = The number of tasks you expect to have a value for it; the deletion
    stops if the number differs.
op-field-delete--field = The field: its name or ID. Its values leave every task, and saved
    queries that use it lose those conditions.
op-field-delete--project = The project whose fields to look in: its key (current or old) or
    ID; left out: the fields of every project.

## glyphkeel task list

op-task-search = List the tasks matching a query (is:open due<+7d label:bug sort:due), a page at a time, with totals of estimates and logged time.
op-task-search--cursor = The `next_cursor` of the page before.
op-task-search--group-by = Groups the rows: they come group by group (each group sorted by the
    query's order), and the result names the groups with their counts.
op-task-search--group-by-field = Groups the rows by a custom field instead (`cf.<name>`): a select
    field by its option, in the field's order (a multi-select by its
    first option), a yes/no field by `yes` and `no` (no value counts as
    no). Not with `group_by`.
op-task-search--include-subprojects = With `project`: its sub-projects' tasks too.
op-task-search--limit = Tasks per page, 1 to 200; default 50.
op-task-search--project = Only this project (key or ID).
op-task-search--query = The query, for example `is:open due<+7d sort:due`; empty lists every
    open and closed task of active projects.
op-task-search--sort = The order when the query has no `sort:`, written like its keys
    (`due,-priority`). Default: by relevance for text searches, else by
    status, then place on the board.

## glyphkeel task show <keys>

op-task-get = One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request.
op-task-get--include = What to add to each task: `subtasks`, `links`, `comments`, `files`,
    `checklist`, `revisions`, `time`, `reminder`, `backlinks`,
    `references`.
op-task-get--keys = One to 20 task keys or IDs.

## glyphkeel task context <key>

op-task-context = Everything needed to work on one task, in one read within a size: the task with its checklist, fields, sprint and version, its parents, subtasks and links, the recent comments and changes, the status rules, the project's agent brief, the wiki pages it links to, and waiting proposals; says what was trimmed.
op-task-context--depth = Levels of subtasks to list, 0 to 4: 1 (the default) lists the
    task's own subtasks, 2 also theirs; 0 lists none.
op-task-context--key = The task: its key (current or old) or ID.
op-task-context--max-bytes = The most bytes of JSON to return, 4096 to 262144; default 32768.
    Over it, the oldest comments go first, then the wiki pages'
    openings, the oldest changes and proposals, and then the brief and
    the description are cut; `trimmed` says what went.

## glyphkeel task create

op-task-create = Create a task from a title alone or with its fields; it gets the project's next key.
op-task-create--after = Place it after this task of the same status.
op-task-create--at = Place it first or last in its status; default: last.
op-task-create--before = Place it before this task of the same status.
op-task-create--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of creating a second task.
op-task-create--description = The description (Markdown).
op-task-create--due-date = The due date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`,
    `in 2 weeks`, `dec 1`, in the workspace's time zone.
op-task-create--estimate = The estimate, as a duration (`1d 4h`, `90m`, `1.5h`).
op-task-create--fields = Custom field values, by field name, query name or ID (see
    `field.list`): a string, a number, `true`/`false`, or a list of
    options for a multi-select field. Fields with a default get it
    unless given here; a required field needs a value.
op-task-create--fix-version = The version it is planned for (name or ID): an open version of the
    project or one shared with it.
op-task-create--labels = Labels, by name or ID.
op-task-create--parent = The parent task, for a subtask (same project, at most 5 levels).
op-task-create--priority = The priority; default: none.
op-task-create--project = The project: its key (current or old) or ID.
op-task-create--sprint = The sprint it goes in (name or ID, or `active`): a planned or active
    sprint of the project.
op-task-create--start-date = The start date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`,
    `in 2 weeks`, `dec 1`, in the workspace's time zone.
op-task-create--status = The status; default: the project's first to-do status.
op-task-create--title = The title, 1 to 500 characters; tabs and line breaks become spaces.
op-task-create--type = The type; default: the project's default type.

## glyphkeel task edit <key>

op-task-update = Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress.
op-task-update--add-labels = Labels to add.
op-task-update--base-revision = The description revision your new description was edited from. If
    somebody saved in between, the two edits are merged; when they
    changed the same lines, yours is kept as a conflict copy and the
    description stays as it is.
op-task-update--clear = Fields to clear.
op-task-update--description = A new description (Markdown), replacing the old one.
op-task-update--description-edits = Targeted changes to the description instead of a new one: each
    `old_text` must occur exactly once.
op-task-update--due-date = A new due date: `YYYY-MM-DD`, or a natural date such as `next fri`
    or `+3d`.
op-task-update--estimate = A new estimate, as a duration.
op-task-update--expected-version = Refuse the change unless the task still has this version.
op-task-update--fields = Custom field values to set, by field name, query name or ID: a
    string, a number, `true`/`false`, a list of options for a
    multi-select field, or `null` to clear the value.
op-task-update--fix-version = Plan it for this version (name or ID): an open version of the
    project or one shared with it.
op-task-update--key = The task: its key or ID.
op-task-update--labels = Replace the labels with these.
op-task-update--move-dependents = When the dates change, also move the tasks that depend on this one
    (through `blocks` links, and theirs in turn) later, as far as each
    must to begin after its blockers end. Their lengths stay, nothing
    moves earlier, and closed tasks stay where they are. One change in
    history with this one.
op-task-update--override-claim = Change the task although another agent claimed it (full profile
    only).
op-task-update--parent = A new parent task (same project, at most 5 levels, no loops).
op-task-update--priority = A new priority.
op-task-update--progress = Progress set by hand, 0 to 100.
op-task-update--remove-labels = Labels to take off.
op-task-update--resolve-conflict = Resolve a conflict copy of the description.
op-task-update--sprint = Put it in this sprint (name or ID, or `active`): a planned or active
    sprint of the project.
op-task-update--start-date = A new start date: `YYYY-MM-DD`, or a natural date such as `fri` or
    `in 3d`.
op-task-update--title = A new title.
op-task-update--type = A new type.

## glyphkeel task move <key>

op-task-move = Move a task to another status or place in its column, or to another project (new key; the old one forwards); closing it can close its duplicates too.
op-task-move--after = Place it after this task of the target status.
op-task-move--at = Place it first or last in the target status; default when the status
    changes: last.
op-task-move--before = Place it before this task of the target status.
op-task-move--changes = Changes to the task's own fields made with the move, in the same
    change, as `task.update` makes them: what a card dropped into a
    board swimlane takes from it. Not with `project`.
op-task-move--close-duplicates = When the move closes the task: `true` closes the open tasks marked
    as its duplicates too, in the same change (each into the status of
    the same name in its project, else the first one of the same
    category); `false` leaves them open without a warning. Left out, they
    stay open and a warning names them.
op-task-move--expected-version = Refuse the move unless the task still has this version.
op-task-move--fields = With `project`: custom field values the task and its subtasks get in
    that project (each where its type shows the field), by field name,
    query name or ID, as for `task.update`; for a required field they
    would otherwise have no value of.
op-task-move--key = The task: its key or ID.
op-task-move--mapping = With `project`: which status, type or label of that project stands
    in for one it has no match for.
op-task-move--override-claim = Move the task although another agent claimed it (full profile only).
op-task-move--project = Move it to another project: it gets the next key there, and the old
    key keeps finding it. Its subtasks move with it.
op-task-move--status = The new status, by name or ID.

## glyphkeel task move-check <keys>

op-task-move-check = What moving tasks to another project would do: their new keys, and which of their statuses, types and labels the target project has no match for.
op-task-move-check--keys = The tasks: keys or IDs, 1 to 500. Their subtasks would move with them.
op-task-move-check--project = The project they would move to.

## glyphkeel task delete <keys>

op-task-delete = Move tasks to the trash, each with its subtasks; they can be restored from there.
op-task-delete--keys = The tasks: keys or IDs, 1 to 500. Their subtasks go with them.

## glyphkeel task bulk

op-task-bulk-update = Make the same change to many tasks chosen by key or query (status, type, priority, labels, parent, dates set or shifted, estimate, custom fields, project), previewed first, as one change for Undo and revert.
op-task-bulk-update--changes = The change.
op-task-bulk-update--dry-run = Only show what would change; default true. Turn it off to make the
    change.
op-task-bulk-update--expected-count = How many tasks the change is for, from the preview; needed to make
    the change, which stops when the tasks now number otherwise.
op-task-bulk-update--keys = The tasks, by key or ID. Give these or `query`.
op-task-bulk-update--query = A query choosing the tasks (`project:TM label:old is:open`).

## glyphkeel task reorder-subtask <key>

op-task-reorder-subtask = Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order.
op-task-reorder-subtask--after = Place it after this subtask of the same parent.
op-task-reorder-subtask--at = Place it first or last among the parent's subtasks.
op-task-reorder-subtask--before = Place it before this subtask of the same parent.
op-task-reorder-subtask--expected-version = Refuse the change unless the subtask still has this version.
op-task-reorder-subtask--key = The subtask: its key or ID.
op-task-reorder-subtask--override-claim = Reorder it although another agent claimed it (full profile only).

## glyphkeel task resolve <key>

op-task-resolve-conflict = Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text.
op-task-resolve-conflict--action = What to do with it.
op-task-resolve-conflict--base-revision = Refuse unless the description is still at this revision.
op-task-resolve-conflict--copy = The conflict copy's revision ID.
op-task-resolve-conflict--key = The task: its key or ID.
op-task-resolve-conflict--text = The text to save, for `use_text`.

## glyphkeel task check <key>

op-checklist-update = Tick, clear, add or remove checklist items in a task's description, a comment or a wiki page, by number plus expected text.
op-checklist-update--add = Items to add.
op-checklist-update--comment = Or the comment that holds it: its ID.
op-checklist-update--key = The task whose description holds the checklist: its key or ID.
op-checklist-update--page = Or the wiki page that holds it: its title, slug, old slug or ID. A
    change to a page is saved as a new revision of it.
op-checklist-update--project = The project (key or ID) whose wiki holds `page`; needed when the page
    is named by title or slug.
op-checklist-update--remove = Items to remove.
op-checklist-update--toggle = Items to tick or clear.

## glyphkeel task duplicate <key>

op-task-duplicate = Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone.
op-task-duplicate--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of making a second copy.
op-task-duplicate--key = The task to copy: its key or ID.
op-task-duplicate--status = The copy's status; default: the project's first to-do status.
op-task-duplicate--title = The copy's title; default: the same title.
op-task-duplicate--without-subtasks = Leave the subtasks out; by default they are copied too, in order.

## glyphkeel task claim <key>

op-task-claim = Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress.
op-task-claim--keep-status = Leave the status as it is.
op-task-claim--key = The task: its key or ID.
op-task-claim--lease-minutes = How long the claim lasts unless renewed, 1 to 480 minutes; default
    60. Every change you make to the task renews it.
op-task-claim--override-claim = Take the claim over from another agent (full profile only).
op-task-claim--status = The status to move it to; default: the first in-progress status,
    unless it is in progress already.

## glyphkeel task release <key>

op-task-release = Give up a claim on a task, optionally moving it and adding a comment.
op-task-release--comment = A comment to add (Markdown), for example what was done.
op-task-release--key = The task: its key or ID.
op-task-release--override-claim = Release another agent's claim (full profile only; the person can
    always).
op-task-release--status = A status to move it to, for example Review.

## glyphkeel task link <key>

op-relation-add = Link two tasks (blocks, with an optional lag in days; relates, duplicates, clones; or make one a subtask of the other).
op-relation-add--key = The task: its key or ID.
op-relation-add--kind = How the link reads from this task. `subtask_of` and `parent_of`
    change the parent instead of adding a link.
op-relation-add--lag-days = For `blocks` and `blocked_by` with `relation.add`: the days that must
    pass after the blocking task ends before the blocked one may begin
    (finish-to-start plus a lag), 0 to 365. On an existing link it
    changes the lag; 0 removes it.
op-relation-add--other = The other task: its key or ID.

## glyphkeel task unlink <key>

op-relation-remove = Remove a link between two tasks, or a subtask from its parent.
op-relation-remove--key = The task: its key or ID.
op-relation-remove--kind = How the link reads from this task. `subtask_of` and `parent_of`
    change the parent instead of adding a link.
op-relation-remove--lag-days = For `blocks` and `blocked_by` with `relation.add`: the days that must
    pass after the blocking task ends before the blocked one may begin
    (finish-to-start plus a lag), 0 to 365. On an existing link it
    changes the lag; 0 removes it.
op-relation-remove--other = The other task: its key or ID.

## glyphkeel recurrence set <key>

op-recurrence-set = Make a task recurring, change its rule, or (`update_details`) make its current details those of the occurrences to come.
op-recurrence-set--count = At most this many occurrences, 1 to 10000.
op-recurrence-set--create = When the next occurrence is made; default when this one is closed.
op-recurrence-set--days-ahead = With `create: ahead`: this many days before its date, 0 to 365.
op-recurrence-set--every = The unit.
op-recurrence-set--from = From the calendar (default) or from completion.
op-recurrence-set--interval = Every this many units, 1 to 999; default 1.
op-recurrence-set--key = The task: its key or ID.
op-recurrence-set--month = Yearly: the month, 1 to 12.
op-recurrence-set--month-day = Monthly and yearly: the day of the month, 1 to 31, or -1 for the
    last.
op-recurrence-set--starts = The first possible day; default the task's due date, else its start
    date, else today.
op-recurrence-set--text = The rule in words (`every monday`); fields given as well override
    it. Leave the rule out to keep the series' rule.
op-recurrence-set--until = The last possible day.
op-recurrence-set--update-details = Make this task's current details (title, description, type,
    priority, labels, estimate, subtasks, dates) those of the
    occurrences still to come. A new series always starts from them.
op-recurrence-set--week-of-month = Monthly and yearly: 1 to 5 or -1 (last), with `weekday`.
op-recurrence-set--weekday = With `week_of_month`: the weekday.
op-recurrence-set--weekdays = Weekly: the weekdays; default the start's.

## glyphkeel recurrence clear <key>

op-recurrence-clear = Stop a task repeating: it stays, and no more occurrences are made.
op-recurrence-clear--key = The task: its key or ID.

## glyphkeel recurrence skip <key>

op-recurrence-skip = Pass over a recurring task's current occurrence: its dates move to the next date of the rule.
op-recurrence-skip--key = The task: its key or ID.

## glyphkeel recurrence preview

op-recurrence-preview = Read a recurrence rule, in words or by fields, and list its next dates.
op-recurrence-preview--count = At most this many occurrences, 1 to 10000.
op-recurrence-preview--create = When the next occurrence is made; default when the current one is
    closed.
op-recurrence-preview--days-ahead = With `create: ahead`: this many days before its date, 0 to 365.
op-recurrence-preview--every = The unit; needed without `text`.
op-recurrence-preview--from = From the calendar (default) or from completion.
op-recurrence-preview--interval = Every this many units, 1 to 999; default 1.
op-recurrence-preview--limit = How many dates to show, 1 to 50; default 5.
op-recurrence-preview--month = Yearly: the month, 1 to 12.
op-recurrence-preview--month-day = Monthly and yearly: the day of the month, 1 to 31, or -1 for the
    last.
op-recurrence-preview--starts = The first possible day (a date: `2026-12-01`, `today`, `next mon`);
    default the task's due date, else its start date, else today.
op-recurrence-preview--text = The rule in words (`every 2 weeks on mon and thu`); fields given as
    well override it.
op-recurrence-preview--until = The last possible day.
op-recurrence-preview--week-of-month = Monthly and yearly: 1 to 5 or -1 (last), with `weekday`.
op-recurrence-preview--weekday = With `week_of_month`: the weekday.
op-recurrence-preview--weekdays = Weekly: the weekdays; default the start's.

## glyphkeel recurrence run

op-recurrence-run = Make the occurrences of series that are made ahead whose day has come; the app does this by itself.

## glyphkeel template list

op-template-list = The project templates and the task templates of a project or of every project.
op-template-list--kind = Only templates of this kind.
op-template-list--project = Task templates of this project (key or ID); left out: of every
    project.

## glyphkeel template show <template>

op-template-get = One template with its content.
op-template-get--project = For a task template named by name: its project (key or ID).
op-template-get--template = The template: its name or ID.

## glyphkeel template create <name>

op-template-create = Make a task template (from a task, or given) or a project template (from a project, or given).
op-template-create--description = What it is for, at most 1000 characters.
op-template-create--from-project = Make a project template from this project (key or ID): its
    statuses, types, labels, modules, description, wiki pages and task
    templates.
op-template-create--from-task = Make it from this task (key or ID), with its subtasks; dates become
    days after the day it was created.
op-template-create--include-tasks = With `from_project`: also its open top-level tasks with their
    subtasks (at most 500 tasks).
op-template-create--kind = What it makes; default: a task, or what `from_task`/`from_project`
    names.
op-template-create--name = The name, 1 to 100 characters, unique among task templates of the
    project or among project templates, ignoring case.
op-template-create--project = For a task template: its project (key or ID); default the project
    of `from_task`.
op-template-create--setup = The set-up, for a project template not made from a project.
op-template-create--task = The task, for a task template not made from a task.

## glyphkeel template edit <template>

op-template-update = Change a template's name, description or content.
op-template-update--description = A new description; empty removes it.
op-template-update--name = A new name.
op-template-update--project = For a task template named by name: its project.
op-template-update--setup = A project template's new set-up.
op-template-update--task = A task template's new task.
op-template-update--template = The template: its name or ID.

## glyphkeel template delete <template>

op-template-delete = Delete a template; it can be brought back with revert. What was made from it stays.
op-template-delete--project = For a task template named by name: its project (key or ID).
op-template-delete--template = The template: its name or ID.

## glyphkeel template use <template>

op-template-apply = Use a template: a task template makes a task with its subtasks; a project template makes a project with its set-up, pages and tasks.
op-template-apply--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of using the template twice.
op-template-apply--date = The day the template's dates count from (a date: `2026-12-01`,
    `next mon`); default today.
op-template-apply--due-date = With a task template: the new task's due date instead of the
    template's (a date: `2026-12-01`, or `fri`, `next mon`, `+3d`).
op-template-apply--folder = A project template: the folder to put the new top-level project in
    (ID or name); not with `parent_project`.
op-template-apply--hue = A project template: its colour.
op-template-apply--key = A project template: the new project's key.
op-template-apply--name = A project template: the new project's name.
op-template-apply--parent = A task template: make it a subtask of this task.
op-template-apply--parent-project = A project template: the parent project, for a sub-project.
op-template-apply--priority = With a task template: the new task's priority instead of the
    template's.
op-template-apply--project = A task template: the project the task goes to (default the
    template's), and where a template named by name is looked up.
op-template-apply--status = A task template: the status; default the project's first.
op-template-apply--template = The template: its name or ID.
op-template-apply--title = A task template: the title instead of the template's.

## glyphkeel description history

op-description-history = List the revisions of a task's or project's description, newest first, conflict copies included.
op-description-history--project = The project: its key or ID.
op-description-history--task = The task: its key or ID.

## glyphkeel description show <revision>

op-description-revision = One description revision with its text, and the differences from another revision on request.
op-description-revision--diff-from = Another revision of the same description: adds the differences
    from it to this one.
op-description-revision--revision = The revision's ID.

## glyphkeel comment list <key>

op-comment-list = A task's comments, oldest first, optionally with their earlier texts.
op-comment-list--key = The task: its key or ID.
op-comment-list--revisions = Add each comment's earlier texts.

## glyphkeel comment add <key>

op-comment-add = Add a Markdown comment to a task, optionally answering another.
op-comment-add--body = The text (Markdown), at most 256 KB.
op-comment-add--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of adding a second comment.
op-comment-add--key = The task: its key or ID.
op-comment-add--reply-to = The ID of the comment this answers (same task).

## glyphkeel comment edit <comment>

op-comment-update = Change a comment's text; the earlier text is kept and can be viewed.
op-comment-update--body = The new text (Markdown); the earlier one is kept.
op-comment-update--comment = The comment's ID.
op-comment-update--expected-version = Refuse the change unless the comment still has this version.

## glyphkeel comment delete <comment>

op-comment-delete = Move a comment to the trash.
op-comment-delete--comment = The comment's ID.

## glyphkeel time log <key>

op-time-log = Log time on a task: a duration, the day of the work (default today), a note and a kind of work.
op-time-log--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of logging the time twice.
op-time-log--date = The day of the work: `YYYY-MM-DD`, `today` or `yesterday`; default
    today.
op-time-log--duration = How long: `1h 30m`, `90m`, `1.5h`, `1:30`, `+30m`; at most 24 hours.
op-time-log--key = The task: its key or ID.
op-time-log--note = What was done, at most 2000 characters.
op-time-log--work-kind = The kind of work: its name (any case) or ID, from `work_kind.list`.

## glyphkeel time edit <entry>

op-time-update = Change a time entry's duration, day, note or kind of work.
op-time-update--date = The new day: `YYYY-MM-DD`, `today` or `yesterday`.
op-time-update--duration = The new duration.
op-time-update--entry = The entry's ID.
op-time-update--expected-version = Refuse the change unless the entry still has this version.
op-time-update--note = The new note; an empty text removes it.
op-time-update--work-kind = The new kind of work (name or ID); an empty text removes it.

## glyphkeel time delete <entry>

op-time-delete = Move a time entry to the trash.
op-time-delete--entry = The entry's ID.

## glyphkeel time report

op-time-report = Logged time over a range of days, grouped by day, week, month, task, project, person or kind of work, with totals in the workspace's units and optional CSV.
op-time-report--csv = Also return the report as CSV text: `groups` or `entries`.
op-time-report--entries = Also list the entries, oldest first.
op-time-report--from = The first day: `YYYY-MM-DD`, `today` or `yesterday`; default the
    first day of the current week.
op-time-report--group-by = How to group: `day`, `week`, `month`, `task` (default), `project`,
    `person` or `work_kind`.
op-time-report--include-subprojects = With `project`: its sub-projects' time too; default true.
op-time-report--person = Only the time logged by `me` (the local user), `agent` (any agent)
    or the person or agent of that name.
op-time-report--project = Only this project: a key or ID.
op-time-report--task = Only this task and its subtasks: a key or ID.
op-time-report--to = The last day, included; default six days after `from`. At most 366
    days after `from`.
op-time-report--work-kind = Only the time of this kind of work (name or ID), or `none` for time
    without one.

## glyphkeel work-kind list

op-work-kind-list = The workspace's kinds of work for logged time (Development, Design, Meeting…), by name, with how many entries use each.
op-work-kind-list--include-archived = Also the archived ones.

## glyphkeel work-kind create

op-work-kind-create = Add a kind of work that time entries can be marked with.
op-work-kind-create--name = Its name, 1 to 60 characters, unique in the workspace (any case).

## glyphkeel work-kind edit <kind>

op-work-kind-update = Rename a kind of work, or archive it so it is no longer offered while its entries keep it.
op-work-kind-update--archived = Archive it (`true`) or offer it again (`false`).
op-work-kind-update--kind = The kind: its name (any case) or ID.
op-work-kind-update--name = A new name.

## glyphkeel work-kind delete <kind>

op-work-kind-delete = Delete a kind of work that no time entry names; archive one that is in use.
op-work-kind-delete--kind = The kind: its name (any case) or ID.

## glyphkeel report created-closed

op-report-created-closed = Tasks created against tasks closed and reopened per day, week or month, with the open tasks at the end of each period; from the history, with optional CSV.
op-report-created-closed--actor = Created against closed and cycle time only: count only what `me`
    (the local user), `agent` (any agent) or the person or agent of that
    name did: creating a task, closing or reopening it.
op-report-created-closed--bucket = The length of a period: `day`, `week` (default) or `month`. At most
    1,000 periods.
op-report-created-closed--csv = Also return the report as CSV text (comma-separated, a header line,
    CRLF line ends).
op-report-created-closed--fix-version = Only tasks planned for this version: a name (every version of that
    name) or an ID.
op-report-created-closed--from = The first day: `YYYY-MM-DD`, `today` or `yesterday`; default twelve
    periods before `to` (30 days, 12 weeks or 12 months).
op-report-created-closed--group-by = Cumulative flow only: bands by `status` (default) or `category`.
op-report-created-closed--include-subprojects = With `project`: its sub-projects' tasks too; default true.
op-report-created-closed--label = Only tasks with this label, by name.
op-report-created-closed--project = Only this project: a key or ID.
op-report-created-closed--query = Only tasks that match this query (the task query language), as they
    are now.
op-report-created-closed--tasks = Cycle time only: also list every task closed in the period.
op-report-created-closed--to = The last day, included; default today.
op-report-created-closed--type = Only tasks of this type, by name.

## glyphkeel report flow

op-report-cumulative-flow = How many tasks were in each status (or category) at the end of each day, week or month; from the history, with optional CSV.
op-report-cumulative-flow--actor = Created against closed and cycle time only: count only what `me`
    (the local user), `agent` (any agent) or the person or agent of that
    name did: creating a task, closing or reopening it.
op-report-cumulative-flow--bucket = The length of a period: `day`, `week` (default) or `month`. At most
    1,000 periods.
op-report-cumulative-flow--csv = Also return the report as CSV text (comma-separated, a header line,
    CRLF line ends).
op-report-cumulative-flow--fix-version = Only tasks planned for this version: a name (every version of that
    name) or an ID.
op-report-cumulative-flow--from = The first day: `YYYY-MM-DD`, `today` or `yesterday`; default twelve
    periods before `to` (30 days, 12 weeks or 12 months).
op-report-cumulative-flow--group-by = Cumulative flow only: bands by `status` (default) or `category`.
op-report-cumulative-flow--include-subprojects = With `project`: its sub-projects' tasks too; default true.
op-report-cumulative-flow--label = Only tasks with this label, by name.
op-report-cumulative-flow--project = Only this project: a key or ID.
op-report-cumulative-flow--query = Only tasks that match this query (the task query language), as they
    are now.
op-report-cumulative-flow--tasks = Cycle time only: also list every task closed in the period.
op-report-cumulative-flow--to = The last day, included; default today.
op-report-cumulative-flow--type = Only tasks of this type, by name.

## glyphkeel report cycle-time

op-report-cycle-time = Cycle time (first move into progress to the last close) and lead time (creation to the last close) of the tasks closed over a range of days, with percentiles per period; optional task list and CSV.
op-report-cycle-time--actor = Created against closed and cycle time only: count only what `me`
    (the local user), `agent` (any agent) or the person or agent of that
    name did: creating a task, closing or reopening it.
op-report-cycle-time--bucket = The length of a period: `day`, `week` (default) or `month`. At most
    1,000 periods.
op-report-cycle-time--csv = Also return the report as CSV text (comma-separated, a header line,
    CRLF line ends).
op-report-cycle-time--fix-version = Only tasks planned for this version: a name (every version of that
    name) or an ID.
op-report-cycle-time--from = The first day: `YYYY-MM-DD`, `today` or `yesterday`; default twelve
    periods before `to` (30 days, 12 weeks or 12 months).
op-report-cycle-time--group-by = Cumulative flow only: bands by `status` (default) or `category`.
op-report-cycle-time--include-subprojects = With `project`: its sub-projects' tasks too; default true.
op-report-cycle-time--label = Only tasks with this label, by name.
op-report-cycle-time--project = Only this project: a key or ID.
op-report-cycle-time--query = Only tasks that match this query (the task query language), as they
    are now.
op-report-cycle-time--tasks = Cycle time only: also list every task closed in the period.
op-report-cycle-time--to = The last day, included; default today.
op-report-cycle-time--type = Only tasks of this type, by name.

## glyphkeel report velocity <project>

op-report-velocity = The velocity of a project's completed sprints: the work each took on at its start against the work it completed, in tasks, estimated hours or a number field, with the average of the last three and the work planned for the sprints to come.
op-report-velocity--csv = Also return the report as CSV text (comma-separated, a header line,
    CRLF line ends).
op-report-velocity--field = With `measure: field`: a whole-number or decimal custom field (its
    name or `cf.<name>`).
op-report-velocity--last = How many of the latest completed sprints to show, 1 to 50; default
    10.
op-report-velocity--measure = `tasks` (default), `estimate` (hours) or `field`.
op-report-velocity--project = The project: a key or ID.

## glyphkeel report burnup

op-report-burnup = A burn-up of a version or a sprint: the work planned for it and the part completed at the end of each day, from the history, with a forecast of when the two meet.
op-report-burnup--csv = Also return the days as CSV text.
op-report-burnup--field = With `measure: field`: a whole-number or decimal custom field.
op-report-burnup--forecast = Add the forecast; default true.
op-report-burnup--measure = `tasks` (default), `estimate` (hours) or `field`.
op-report-burnup--project = With a name or `active`: the project (key or ID).
op-report-burnup--sprint = The sprint (active or closed): its ID, its name, or `active`.
op-report-burnup--version = The version: its ID or name. Give this or `sprint`.

## glyphkeel report estimates

op-report-estimates = Estimate accuracy: for tasks closed over a range of days with an estimate and logged time, the median ratio of spent to estimated time, the share within 20 %, and the outliers, overall and by type, label, project, kind of work or month.
op-report-estimates--csv = Also return the tasks as CSV text.
op-report-estimates--from = The first day tasks were closed on: `YYYY-MM-DD`, `today` or
    `yesterday`; default 89 days before `to`.
op-report-estimates--group-by = `none` (default), `type`, `label`, `project`, `work_kind` or
    `month`.
op-report-estimates--project = Only this project (key or ID) and its sub-projects.
op-report-estimates--query = Only tasks that match this query, as they are now.
op-report-estimates--tasks = Also list every task compared.
op-report-estimates--to = The last day, included; default today.

## glyphkeel timer status

op-timer-get = The workspace's timer: its task, whether it runs and the time so far.

## glyphkeel timer start <key>

op-timer-start = Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged.
op-timer-start--key = The task: its key or ID. Left out, the paused timer runs again. A
    timer on another task is stopped first and its time logged.
op-timer-start--note = The note the entry will get, at most 2000 characters.
op-timer-start--work-kind = The kind of work the entry will get (name or ID); an empty text
    removes it.

## glyphkeel timer pause

op-timer-pause = Pause the running timer; it keeps its time.

## glyphkeel timer stop

op-timer-stop = Stop the timer and log its time as an entry on its task.
op-timer-stop--discard = Throw the time away instead of logging it.
op-timer-stop--duration = Log this duration instead of the time measured, for example when the
    timer ran on by mistake.
op-timer-stop--note = The entry's note; default the timer's own note.
op-timer-stop--work-kind = The entry's kind of work (name or ID); default the timer's own; an
    empty text logs none.

## glyphkeel reminder set <key>

op-reminder-set = Set a task's personal reminder: a moment, or a number of days before its due date.
op-reminder-set--at = When: RFC 3339, or a local `YYYY-MM-DD HH:MM`. Give this or
    `days_before_due`.
op-reminder-set--days-before-due = Fire this many days (0 to 365) before the due date, and again when
    the due date changes.
op-reminder-set--key = The task: its key or ID.
op-reminder-set--note = Its text, at most 500 characters.
op-reminder-set--time = With `days_before_due`: the time of day, `HH:MM`; default the
    workspace's reminder time.

## glyphkeel reminder clear <key>

op-reminder-clear = Remove a task's personal reminder.
op-reminder-clear--key = The task: its key or ID.

## glyphkeel reminder due

op-reminder-due = The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes.
op-reminder-due--limit = At most this many, 1 to 500; default 100.
op-reminder-due--since = Reminders due before this moment are marked `missed`: pass the time
    your program started, so what came due while it was closed shows as
    missed. Default: due more than 5 minutes ago.
op-reminder-due--until = Reminders due up to this moment (RFC 3339 or local `YYYY-MM-DD
    HH:MM`); default now. A later moment lists what is coming.

## glyphkeel reminder ack <ids>

op-reminder-acknowledge = Acknowledge reminders that were shown, so they don't come due again.
op-reminder-acknowledge--ids = The `id`s of due reminders, as the list of due reminders gives them.

## glyphkeel today

op-today-get = What needs attention today across projects: overdue tasks, tasks due today, tasks in progress, today's reminders, agents' claims, recent changes and the timer, with counts for the sidebar.
op-today-get--limit = At most this many tasks per list, 1 to 200; default 50. The counts
    are always complete.
op-today-get--project = Only this project (key or ID), with its sub-projects; default: every
    active project.

## glyphkeel digest show

op-digest-get = What changed without the person since a moment (by default since they last looked): agent sessions with their changes and tasks, automation rules with their runs and failures, reminders and tasks that came due, and changes through the command line (for an agent, also the person's changes in the app), with counts.
op-digest-get--limit = At most this many items in each list, 1 to 100; default 20. The
    counts are always complete.
op-digest-get--project = Only this project (key or ID), with its sub-projects; default: every
    project.
op-digest-get--since = From when: a moment (RFC 3339) or a day in words (`yesterday`,
    `-3d`, `2026-11-20`: from its start, in the caller's time zone).
    Left out: for the person, the moment they last dismissed the
    digest; for an agent, the end of its previous session of the same
    name; without either, the last 24 hours.

## glyphkeel digest dismiss

op-digest-dismiss = Mark the digest as seen up to a moment, so that the next one starts there.
op-digest-dismiss--at = The `until` of the digest the person saw (RFC 3339); default: now.
    The marker never moves back.

## glyphkeel board <project>

op-board-get = A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state, compact cards and the swimlanes with their counts.
op-board-get--include-subprojects = Show the sub-projects' tasks too; default: as the layout says.
op-board-get--lane-field = With `lanes: field`, the select custom field (name or `cf.<query
    name>`); default: the layout's.
op-board-get--lanes = Group the cards into swimlanes by this; default: as the layout says.
op-board-get--project = The project: its key (current or old) or ID.
op-board-get--show-older = Show every closed card, however old.

## glyphkeel board all

op-board-get-across = The open work of every active project on one board: a column for each status category (to do, in progress, recently done), cards with their project and real status, filtered by a query or a folder, in swimlanes by project by default.
op-board-get-across--folder = Only the projects of this folder (ID or name), with their
    sub-projects.
op-board-get-across--lanes = Group the cards into swimlanes by this; default: `project`.
op-board-get-across--query = Only the tasks this query matches, such as `label:bug due<+7d`;
    empty: every task.
op-board-get-across--show-older = Show every closed card, however old.
op-board-get-across--view = A saved view of the board across projects (ID or name): its query,
    folder and swimlanes, which the fields below override.

## glyphkeel board layout show <project>

op-layout-get = The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields, card colours, covers and swimlanes.
op-layout-get--project = The project: its key (current or old) or ID.

## glyphkeel board layout set <project>

op-layout-update = Hide, show, collapse or reorder board columns, set sort modes, card fields, card colours (by type, priority, label or ordered query rules), covers, swimlanes and older closed cards, or reset to the project's layout.
op-layout-update--card-fields = The fields shown on cards: `type`, `priority`, `due`, `start`,
    `labels`, `checklist`, `subtasks`, `estimate`, `days_in_column`,
    `updated`, `key`, and custom fields of the project as
    `cf.<query name>`.
op-layout-update--collapse = Columns to collapse.
op-layout-update--colour-by = What colours the edge of the cards: `none`, `type`, `priority`,
    `label` or `query` (the colour rules).
op-layout-update--colour-rules = The colour rules, in order, replacing the old ones: each a query and
    a colour, and an optional name. At most 20.
op-layout-update--column-sort = Sort modes of single columns, by status name or ID; `manual` columns
    can be reordered by hand.
op-layout-update--covers = Which image the cards show as their cover: `off`, `chosen` or
    `first_image`.
op-layout-update--expand = Columns to expand again.
op-layout-update--hide = Columns to hide, by status name or ID.
op-layout-update--hide-done-after-days = Hide cards closed more than this many days ago; 0 shows all.
op-layout-update--include-subprojects = Show the sub-projects' tasks too.
op-layout-update--lane-field = The select custom field the lanes group by, by name or as
    `cf.<query name>`.
op-layout-update--lanes = What the swimlanes group the cards by: `none`, `parent`, `priority`,
    `label`, `sprint`, `type`, `project` or `field` (with `lane_field`).
op-layout-update--order = The column order, by status name or ID; columns left out follow in
    project order.
op-layout-update--project = The project: its key (current or old) or ID.
op-layout-update--reset = Go back to the project's layout first: every column shown and
    expanded, in project order, sorted by hand.
op-layout-update--show = Columns to show again.
op-layout-update--sort-mode = The sort mode of every column without its own.

## glyphkeel view list

op-view-list = The saved views of a project or of the whole workspace: name, query, columns, order, grouping, totals and which is the default.
op-view-list--all = Every view, of every project and the workspace.
op-view-list--kind = Only views for this screen.
op-view-list--project = The views of this project (key or ID); left out: the views of the
    whole workspace.

## glyphkeel view create

op-view-create = Save a query as a named view of a project or of the whole workspace, with its columns, order, grouping and totals.
op-view-create--columns = The columns shown, in order.
op-view-create--description = What it is for.
op-view-create--folder = For a board across projects: only the projects of this folder (ID
    or name).
op-view-create--group-by = The field rows are grouped by; for a board across projects, what
    its swimlanes group by (`project`, `priority`, `label`, `sprint`,
    `type`, `parent` or `none`).
op-view-create--is-default = Make it the screen's default view.
op-view-create--kind = The screen; default: the list.
op-view-create--name = Its name, unique among the views of its project and screen.
op-view-create--project = The project (key or ID); left out: the whole workspace.
op-view-create--query = The query text; it must be valid.
op-view-create--sort = The order when the query has no `sort:`.
op-view-create--totals = The columns totalled.

## glyphkeel view edit <view>

op-view-update = Rename a saved view, change its query, columns, order, grouping or totals, or make it the default.
op-view-update--clear = Fields to clear.
op-view-update--columns = New columns.
op-view-update--description = A new description.
op-view-update--folder = For a board across projects: another folder (ID or name).
op-view-update--group-by = A new grouping.
op-view-update--is-default = Make it, or stop it being, the screen's default view.
op-view-update--name = A new name.
op-view-update--project = With a name: the view's project (key or ID).
op-view-update--query = A new query.
op-view-update--sort = A new order.
op-view-update--totals = New totals.
op-view-update--view = The view: its ID, or its name.

## glyphkeel view delete <view>

op-view-delete = Delete a saved view; it can be brought back with revert.
op-view-delete--project = With a name: the view's project (key or ID).
op-view-delete--view = The view: its ID, or its name.

## glyphkeel dashboard list

op-dashboard-list = The dashboards of the whole workspace or of a project, with their widgets and settings.
op-dashboard-list--all = Every dashboard, of the workspace and of every project.
op-dashboard-list--project = The dashboards pinned to this project (key or ID); left out: the
    dashboards of the whole workspace.

## glyphkeel dashboard show <dashboard>

op-dashboard-show = One dashboard with what each of its widgets shows now: counts, tasks, report figures, burndown, versions, changes, failed automation runs and notes.
op-dashboard-show--dashboard = The dashboard: its ID, or its name.
op-dashboard-show--project = With a name: the project it is pinned to (key or ID); left out for
    the workspace's dashboards.

## glyphkeel dashboard widget <dashboard> <widget>

op-dashboard-widget = What one widget of a dashboard shows now.
op-dashboard-widget--dashboard = The dashboard: its ID, or its name.
op-dashboard-widget--project = With a name: the project it is pinned to (key or ID).
op-dashboard-widget--widget = The widget: its ID, or its place in the grid (`1` is the first).

## glyphkeel dashboard create

op-dashboard-create = Make a dashboard for the whole workspace or pinned to a project, empty, with the starter widgets or with the widgets given.
op-dashboard-create--description = What it is for.
op-dashboard-create--name = Its name, unique among the dashboards of its project (or of the
    workspace).
op-dashboard-create--project = The project to pin it to (key or ID); left out: the whole
    workspace.
op-dashboard-create--starter = Start with the starter widgets: open tasks, overdue and due soon,
    open tasks by status, created against closed and the latest
    changes. Widgets given as well come after them.
op-dashboard-create--widgets = Its widgets, in order; at most 40.

## glyphkeel dashboard edit <dashboard>

op-dashboard-update = Rename a dashboard or change its description.
op-dashboard-update--clear = Fields to clear.
op-dashboard-update--dashboard = The dashboard: its ID, or its name.
op-dashboard-update--description = A new description.
op-dashboard-update--name = A new name.
op-dashboard-update--pin-to = Pins it to this project (key or ID); its widgets that name no
    project then look at that project.
op-dashboard-update--project = With a name: the project it is pinned to (key or ID).

## glyphkeel dashboard delete <dashboard>

op-dashboard-delete = Delete a dashboard; it can be brought back with revert.
op-dashboard-delete--dashboard = The dashboard: its ID, or its name.
op-dashboard-delete--project = With a name: the project it is pinned to (key or ID); left out for
    the workspace's dashboards.

## glyphkeel dashboard add-widget <dashboard>

op-dashboard-add-widget = Add a widget to a dashboard, at the end or at a given place.
op-dashboard-add-widget--dashboard = The dashboard: its ID, or its name.
op-dashboard-add-widget--position = Its place in the grid (`1` is the first); default: the end.
op-dashboard-add-widget--project = With a name: the project it is pinned to (key or ID).
op-dashboard-add-widget--widget = The widget.

## glyphkeel dashboard edit-widget <dashboard> <widget>

op-dashboard-update-widget = Change a widget's title or settings, make it wider or narrower, or move it to another place in the grid.
op-dashboard-update-widget--clear = Settings to clear.
op-dashboard-update-widget--dashboard = The dashboard: its ID, or its name.
op-dashboard-update-widget--position = Move it to this place in the grid (`1` is the first).
op-dashboard-update-widget--project = With a name: the project it is pinned to (key or ID).
op-dashboard-update-widget--settings = Settings to change; those left out keep their value.
op-dashboard-update-widget--title = A new title; empty: the kind's own.
op-dashboard-update-widget--widget = The widget: its ID, or its place in the grid (`1` is the first).
op-dashboard-update-widget--width = A new width in grid columns, 1 to 3.

## glyphkeel dashboard remove-widget <dashboard> <widget>

op-dashboard-remove-widget = Remove a widget from a dashboard.
op-dashboard-remove-widget--dashboard = The dashboard: its ID, or its name.
op-dashboard-remove-widget--project = With a name: the project it is pinned to (key or ID).
op-dashboard-remove-widget--widget = The widget: its ID, or its place in the grid (`1` is the first).

## glyphkeel schedule

op-schedule-get = The schedule of a project or a query for the Gantt chart: dated tasks as a tree with summary ranges for parents, milestones, the tasks without dates, blocks links as finish-to-start dependencies, and problems such as a task starting before its blocker ends.
op-schedule-get--from = Leave out tasks that end before this day: `YYYY-MM-DD` or a natural
    date such as `today` or `-2w`.
op-schedule-get--include-subprojects = With `project`: its sub-projects' tasks too. Off unless asked.
op-schedule-get--project = The project (key or ID). Give this, `query`, or both.
op-schedule-get--query = Only the tasks matching this query (`is:open label:release`); with
    `project`, inside that project. Its `sort:` orders the top-level
    rows; by default they go by start date, then due date.
op-schedule-get--to = Leave out tasks that begin after this day, written as for `from`.

## glyphkeel wiki list

op-wiki-list = A project's wiki pages as a tree: titles, slugs, nesting and when each changed.
op-wiki-list--project = The project: its key or ID.

## glyphkeel wiki search <text>

op-wiki-search = Find wiki pages by title, heading or text, for example to link one; `Page#Heading` lists a page's headings.
op-wiki-search--limit = At most this many results, 1 to 200; default 20.
op-wiki-search--project = Only this project (key or ID); default: every active project.
op-wiki-search--text = Words to find in titles, headings and text. Written `Page#Heading`,
    it finds the headings of the pages whose titles match `Page`. Empty
    lists the most recently changed pages.

## glyphkeel wiki get <page>

op-wiki-get = One wiki page with its text, revision, headings, place in the tree and, on request, what links to it; or an older revision of it.
op-wiki-get--include = What to add: `backlinks`, `references`, `checklist`, `files`.
op-wiki-get--page = The page: its title (any case), slug or ID.
op-wiki-get--project = The project (key or ID); needed unless `page` is an ID.
op-wiki-get--revision = An older revision to read instead of the current text.

## glyphkeel wiki new

op-wiki-create = Create a wiki page with a title unique in its project, optionally inside another page and with its text or a template's.
op-wiki-create--after = Place it after this page of the same parent.
op-wiki-create--at = Place it first or last; default: last.
op-wiki-create--before = Place it before this page of the same parent.
op-wiki-create--body = The text (Markdown).
op-wiki-create--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of creating a second page.
op-wiki-create--message = A note saved with the first revision.
op-wiki-create--parent = The page to put it in (title, slug or ID); default: at the top.
op-wiki-create--project = The project: its key or ID.
op-wiki-create--template = Start from this page's text: a template (or any page) by title,
    slug, ID or `KEY:Page` for another project's; not with `body`.
op-wiki-create--title = The title, 1 to 200 characters, unique in the project ignoring case,
    without `[`, `]`, `#` or `|`.

## glyphkeel wiki put <page>

op-wiki-update = Change a wiki page: its text (whole, with a base revision, or by targeted edits), title, parent or place, whether it is the home page or a template; or resolve a conflict copy.
op-wiki-update--after = Place it after this page of the same parent.
op-wiki-update--agent-brief = Mark it as the project's brief for agents (`true`; another page
    marked before is unmarked), or unmark it (`false`). Only people can
    change this, not agents.
op-wiki-update--at = Place it first or last among its parent's pages.
op-wiki-update--base-revision = The revision your new text was edited from. If somebody saved in
    between, the two edits are merged; when they changed the same lines,
    yours is kept as a conflict copy and the page stays as it is.
op-wiki-update--before = Place it before this page of the same parent.
op-wiki-update--body = A new text (Markdown), replacing the old one.
op-wiki-update--clear = Fields to clear.
op-wiki-update--edits = Targeted changes to the text instead of a new one: each `old_text`
    must occur exactly once.
op-wiki-update--home = Make it the wiki's home page (`true`; the page that was the home
    page before stops being it), or stop it being the home page
    (`false`). Without a home page the wiki opens on its first page.
op-wiki-update--message = A note saved with the new revision.
op-wiki-update--page = The page: its title (any case), slug or ID.
op-wiki-update--parent = A new parent page (title, slug or ID, same project).
op-wiki-update--project = The project (key or ID); needed unless `page` is an ID.
op-wiki-update--resolve-conflict = Resolve a conflict copy of the text.
op-wiki-update--template = Mark it as a template (`true`), which the app offers when a new page
    is made, or unmark it (`false`).
op-wiki-update--title = A new title. The old slug keeps finding the page, so links by the
    old title still resolve until another page takes that name.

## glyphkeel wiki move <page>

op-wiki-move = Move a wiki page, with the pages inside it, into another parent page or to the top, and to a place among its siblings; refuses moves into itself; links to it keep working.
op-wiki-move--after = Place it after this page of the (new) parent.
op-wiki-move--at = Place it first or last; default when the parent changes: last.
op-wiki-move--before = Place it before this page of the (new) parent.
op-wiki-move--page = The page: its title (any case), slug or ID.
op-wiki-move--parent = The new parent page (title, slug or ID, same project). Not the page
    itself or a page inside it.
op-wiki-move--project = The project (key or ID); needed unless `page` is an ID.
op-wiki-move--top-level = Move it to the top of the tree, out of its parent.

## glyphkeel wiki delete <page>

op-wiki-delete = Move a wiki page, with the pages inside it, to the trash; links to it stop resolving until it is restored.
op-wiki-delete--page = The page: its title (any case), slug or ID.
op-wiki-delete--project = The project (key or ID); needed unless `page` is an ID.

## glyphkeel wiki history <page>

op-wiki-history = List the revisions of a wiki page, newest first, conflict copies included; read one with wiki.get and its revision.
op-wiki-history--page = The page: its title (any case), slug or ID.
op-wiki-history--project = The project (key or ID); needed unless `page` is an ID.

## glyphkeel wiki export <page>

op-wiki-export = Export a wiki page, optionally with the pages inside it, or a project's whole wiki, as Markdown files or web pages with their attachments, in a .zip file or a folder; links between the exported pages and to the attachments become relative links.
op-wiki-export--encrypt = Encrypt the archive (a `.zip` file) with the workspace's passphrase,
    so that it opens only in Glyphkeel with that passphrase. Left out:
    on when the workspace is encrypted and the archive is a `.zip` file
    or goes into the `exports` folder. Only an encrypted workspace can
    encrypt an export.
op-wiki-export--format = A `.zip` file (the default) or a folder.
op-wiki-export--page = The page: its title (any case), slug or ID. Left out: the project's
    whole wiki, every page in it.
op-wiki-export--path = Where to write it: a new file or folder. For agents, a plain name
    goes into the workspace's `exports` folder; other paths must be
    inside a folder agents may use. Left out: a name made from the
    page's slug and the date, in the `exports` folder.
op-wiki-export--project = The project (key or ID); needed unless `page` is an ID.
op-wiki-export--subpages = Include the pages inside it, each in a folder named after its parent
    (always, for a whole wiki).
op-wiki-export--text-format = Markdown files (the default) or web pages.

## glyphkeel wiki import <path>

op-wiki-import = Import a folder of Markdown files as wiki pages: sub-folders become pages inside pages, relative links between the files and `[[…]]` links become wiki links, and linked images and files are attached to their pages; one change that Undo takes back.
op-wiki-import--dry-run = Only check the folder and say what would be imported.
op-wiki-import--parent = The page to put the imported pages in (title, slug or ID); left out:
    at the top of the wiki.
op-wiki-import--path = The folder (or a `.zip` file of one). For agents, a plain name is
    one in the workspace's `exports` folder; other paths must be inside
    a folder agents may use.
op-wiki-import--project = The project whose wiki the pages go to (key or ID).

## glyphkeel wiki backlinks <page>

op-wiki-backlinks = The wiki pages, task descriptions and comments that link to a wiki page, by its title, slug, an old slug or its ID.
op-wiki-backlinks--page = The page: its title (any case), slug or ID.
op-wiki-backlinks--project = The project (key or ID); needed unless `page` is an ID.

## glyphkeel wiki references <page>

op-wiki-references = The pages and tasks a wiki page's text links to, in order, each resolved now; links that find nothing are marked.
op-wiki-references--page = The page: its title (any case), slug or ID.
op-wiki-references--project = The project (key or ID); needed unless `page` is an ID.

## glyphkeel task backlinks <key>

op-task-backlinks = The wiki pages, other tasks' descriptions and comments that mention a task by its key (current or old).
op-task-backlinks--key = The task: its key (current or old) or ID.

## glyphkeel task references <key>

op-task-references = The pages and tasks a task's description and comments link to, in order, each resolved now; links that find nothing are marked.
op-task-references--key = The task: its key (current or old) or ID.

## glyphkeel file show <attachment>

op-file-get = An attached file's name, type, size and owner.
op-file-get--attachment = The attachment's ID.

## glyphkeel file cat <attachment>

op-file-read = An attached file's content: text for text files, Base64 for images, otherwise only its details.
op-file-read--attachment = The attachment's ID.

## glyphkeel file attach

op-file-attach = Attach a local file, or Base64 data with a name, to a task, a comment or a wiki page.
op-file-attach--client-request-id = Your own ID for this request; repeating it within 24 hours returns
    the first answer instead of attaching the file again.
op-file-attach--comment = The comment to attach it to: its ID.
op-file-attach--data-base64 = The content as Base64, instead of `path`.
op-file-attach--name = The file name; default: the path's last part.
op-file-attach--page = The wiki page to attach it to: its title (any case), slug or ID.
op-file-attach--path = The path of a local file to read. For agents, a plain name is a file
    in the workspace's `exports` folder; other paths must be inside a
    folder agents may use.
op-file-attach--project = The page's project (key or ID); needed unless `page` is an ID.
op-file-attach--task = The task to attach it to: its key or ID.

## glyphkeel file delete <attachment>

op-file-delete = Move an attached file to the trash.
op-file-delete--attachment = The attachment's ID.

## glyphkeel file cover <task>

op-file-cover = Choose the PNG, JPEG, GIF or WebP image attached to a task that its board card shows as its cover, or choose none; boards show covers when their layout's covers setting is on.
op-file-cover--attachment = The image attached to the task itself to show, by its ID; left out
    to choose none.
op-file-cover--task = The task: its key or ID.

## glyphkeel agents sessions

op-agent-sessions = List agent sessions, newest first, with their client, profile, times and counts of reads, writes and reads refused by the read limits.
op-agent-sessions--limit = At most this many, newest first (default 50, at most 200).
op-agent-sessions--running-only = Only sessions that haven't ended.

## glyphkeel agents policy

op-agent-policy-update = Set the most an agent may do, whatever profile it asks for, whether agents may ask the person to allow one change beyond it, and which of their changes wait for the person's approval.
op-agent-policy-update--approvals = Which changes of agents wait for the person's approval, per
    profile, and how long a proposal waits; left out: unchanged.
op-agent-policy-update--confirmations = Whether agents whose client can ask the person may ask to make one
    change beyond their profile, or one that deletes for good or
    deletes many items; left out: unchanged.
op-agent-policy-update--max-profile = The most an agent may do, whatever profile it asks for.

## glyphkeel proposals list

op-proposal-list = List changes agents proposed, newest first, with their status, a preview of what each changes, the fields changed since and the person's decision. An agent sees its own.
op-proposal-list--ids = Only these proposals, by ID.
op-proposal-list--limit = At most this many, newest first (default 50, at most 200).
op-proposal-list--session = Only proposals of this agent session (an ID, or `current` for an
    agent's own).
op-proposal-list--status = Only proposals with this status.

## glyphkeel proposals show <id>

op-proposal-get = One proposed change: what it runs, its preview, the fields changed since and the decision.
op-proposal-get--id = The proposal's ID.

## glyphkeel proposals approve <ids>

op-proposal-approve = Approve proposed changes, by ID or every waiting one of an agent session: each runs as the agent, and history records that the person approved it.
op-proposal-approve--force = Approve although what it changes changed since it was proposed.
op-proposal-approve--ids = The proposals, by ID, run in the order the agent proposed them.
op-proposal-approve--session = Every waiting proposal of this agent session.

## glyphkeel proposals reject <ids>

op-proposal-reject = Reject proposed changes, by ID or every waiting one of an agent session, with a reason the agent can read.
op-proposal-reject--ids = The proposals, by ID.
op-proposal-reject--reason = Why, for the agent to read; at most 2000 characters.
op-proposal-reject--session = Every waiting proposal of this agent session.

## glyphkeel project rules <project>

op-rules-update = Change a project's status rules (fields set, cleared or required when a task enters a status, fields read-only in it, the timer and claims) and its rules for agents (statuses they may not move tasks into, fields they may not change).
op-rules-update--agents = The rules for agents, replacing the ones the project had.
op-rules-update--project = The project: its key (current or old) or ID.
op-rules-update--statuses = Statuses whose rules to replace.

## glyphkeel automation list

op-automation-list = The automation rules of a project and of the workspace, each with its trigger, condition, actions and latest run.
op-automation-list--project = The project (key or ID): its rules and the workspace's; left out:
    every rule.

## glyphkeel automation show <rule>

op-automation-get = One automation rule with its last runs.
op-automation-get--project = With a name: the project it belongs to (key or ID); left out, a
    rule of the workspace, or the only rule with that name.
op-automation-get--rule = The rule: its ID, or its name.

## glyphkeel automation create

op-automation-create = Make an automation rule: when a trigger happens to a task that matches a query, set fields, change labels, move it, comment, create a subtask or linked task, or leave a notification.
op-automation-create--actions = What it does, 1 to 20 actions, in order.
op-automation-create--condition = A query (TQL) the task must match; empty: every task. A scheduled
    rule needs one: it acts on every task that matches.
op-automation-create--enabled = Whether it runs; default true.
op-automation-create--name = The name, 1 to 100 characters, unique in the project.
op-automation-create--project = The project (key or ID); left out: a rule of the whole workspace.
op-automation-create--trigger = What sets it off.

## glyphkeel automation edit <rule>

op-automation-update = Change an automation rule: its name, trigger, condition or actions, or switch it on or off.
op-automation-update--actions = New actions, replacing the old ones.
op-automation-update--condition = A new condition; empty: every task.
op-automation-update--enabled = Switch it on or off.
op-automation-update--name = A new name.
op-automation-update--project = With a name: the project it belongs to.
op-automation-update--rule = The rule: its ID, or its name.
op-automation-update--trigger = A new trigger.

## glyphkeel automation delete <rule>

op-automation-delete = Delete an automation rule; revert brings it back.
op-automation-delete--project = With a name: the project it belongs to (key or ID); left out, a
    rule of the workspace, or the only rule with that name.
op-automation-delete--rule = The rule: its ID, or its name.

## glyphkeel automation runs

op-automation-runs = The run log of automation rules, newest first: what set each run off, the task, the changes and any error.
op-automation-runs--limit = At most this many, 1 to 200; default 50.
op-automation-runs--problems-only = Only runs that failed or were limited.
op-automation-runs--project = The project: the runs of its rules and the workspace's (and, with
    `rule` as a name, where the rule is looked up).
op-automation-runs--rule = One rule's runs: its ID, or its name.

## glyphkeel automation test <task>

op-automation-test = A dry run of an automation rule, saved or not, against one task: whether it matches and the changes the actions would make. Nothing is changed.
op-automation-test--draft = A rule that isn't saved, instead of `rule`.
op-automation-test--project = With `rule` as a name: the project it belongs to.
op-automation-test--rule = A saved rule: its ID, or its name.
op-automation-test--task = The task: its key or ID.

## glyphkeel automation run

op-automation-run = Run the scheduled and due date automation rules whose time has come; the app does this by itself.

## glyphkeel activity

op-history-feed = The activity log, newest first: who changed what, when and through which program; optionally the agent sessions.
op-history-feed--actor = Only changes by this actor, by name (case-insensitive).
op-history-feed--actor-kind = Only changes by the person (`user`) or only those by agents
    (`agent`).
op-history-feed--cursor = `next_cursor` of the previous page.
op-history-feed--from = Only changes on or after this day: `YYYY-MM-DD`, or a natural form
    such as `today`, `yesterday` or `-7d`, in the caller's time zone.
op-history-feed--group = Only the changes of this history group.
op-history-feed--limit = At most this many changes (default 50, at most 200).
op-history-feed--project = Only changes in this project and its sub-projects: a key or ID.
op-history-feed--session = Only changes of this agent session; `"current"` is the caller's own.
op-history-feed--sessions = Also list the agent sessions.
op-history-feed--task = Only changes to this task (key or ID): its own fields and
    description revisions, its links, its comments and its files,
    including files attached to its comments.
op-history-feed--to = Only changes on or before this day (included), in the same forms as
    `from`.

## glyphkeel revert

op-history-revert = Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed.
op-history-revert--dry-run = Only report what would be reverted.
op-history-revert--groups = History groups to revert.
op-history-revert--session = An agent session whose changes to revert.

## glyphkeel trash list

op-trash-list = The trash: deleted items with what went with them, who deleted them and when they will be purged, newest first.
op-trash-list--cursor = The `next_cursor` of the page before.
op-trash-list--kinds = Only these kinds; all when empty.
op-trash-list--limit = Entries per page, 1 to 200 (50).
op-trash-list--project = Only entries of this project (key or ID), trashed projects
    included.

## glyphkeel trash restore <items>

op-trash-restore = Bring items back out of the trash, with everything that was deleted with them.
op-trash-restore--items = The entries' IDs, 1 to 100.

## glyphkeel trash purge <items>

op-trash-purge = Delete items in the trash for good, and their text from the history. Can't be undone.
op-trash-purge--all = Empty the whole trash instead (not for agents).
op-trash-purge--items = The entries' IDs, up to 100.

## glyphkeel backup create

op-backup-create = Make a backup of the workspace now: a checked copy of the database and its stored files.

## glyphkeel backup list

op-backup-list = The workspace's backups, newest first, with their kind and size.

## glyphkeel backup verify <name>

op-backup-verify = Check a backup: the database copy's integrity and every stored file it needs.
op-backup-verify--name = The backup's name, from the backup list.

## glyphkeel backup restore <name>

op-backup-restore = Replace the workspace with a backup, after backing up the current state. Other programs with the workspace open must restart.
op-backup-restore--name = The backup's name, from the backup list.

## glyphkeel backup delete <name>

op-backup-delete = Delete a backup and the stored files only it used.
op-backup-delete--name = The backup's name, from the backup list.

## glyphkeel export

op-transfer-export = Export projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments.
op-transfer-export--encrypt = Encrypt the archive (a `.zip` file) with the workspace's passphrase,
    so that it opens only in Glyphkeel with that passphrase. Left out:
    on when the workspace is encrypted and the archive is a `.zip` file
    or goes into the `exports` folder. Only an encrypted workspace can
    encrypt an export.
op-transfer-export--format = A `.zip` file (the default) or a folder.
op-transfer-export--path = Where to write it: a new file or folder. For agents, a plain name
    goes into the workspace's `exports` folder; other paths must be
    inside a folder agents may use. Left out: a name with the date in
    the `exports` folder.
op-transfer-export--projects = The projects (keys or IDs), each with its sub-projects; the whole
    workspace when empty.

## glyphkeel import <path>

op-transfer-import = Import an archive as new projects; checks it completely first and never changes existing data.
op-transfer-import--dry-run = Check the archive and report what would be imported, without
    importing it.
op-transfer-import--keys = New keys for projects whose key is taken in this workspace.
op-transfer-import--path = The archive: a `.zip` file or a folder. For agents, a plain name is
    one in the workspace's `exports` folder; other paths must be inside
    a folder agents may use.

## glyphkeel import csv <path>

op-transfer-import-csv = Import tasks from a CSV file into a project, with a column mapping or a preset (Jira and Redmine exports are built in); a dry run reports every row's outcome, and the import is one change that Undo takes back. Status rules hold for each task as for task.create; repeating tasks are not imported.
op-transfer-import-csv--columns = Where columns go, over the preset's choice for the same columns.
op-transfer-import-csv--create-missing = Create the statuses, types and labels the file names that the
    project lacks (the default); otherwise such a value is a row's
    error.
op-transfer-import-csv--dry-run = Only check the file and report what each row would become.
op-transfer-import-csv--format = How to read the file, where detection is not wanted.
op-transfer-import-csv--only-problems = List only the rows with problems, not every row.
op-transfer-import-csv--path = The CSV file. For agents, a plain name is a file in the workspace's
    `exports` folder; other paths must be inside a folder agents may
    use.
op-transfer-import-csv--preset = The preset to start from: `jira`, `redmine`, or the name of a saved
    one. Left out: detected from the header row, else columns are
    matched by their names.
op-transfer-import-csv--project = The project the tasks go to (key or ID).
op-transfer-import-csv--skip-invalid = Import the other rows when some have errors, leaving those out.
    Without it, an error in any row stops the whole import.
op-transfer-import-csv--values = How values become values here, over the preset's.

## glyphkeel csv-preset list

op-csv-preset-list = List the CSV import presets: the built-in ones for Jira and Redmine, and the workspace's own.

## glyphkeel csv-preset save <name>

op-csv-preset-save = Save a CSV import mapping as a preset of the workspace, replacing one with the same name.
op-csv-preset-save--columns = Where columns go.
op-csv-preset-save--format = How to read the file.
op-csv-preset-save--name = Its name, at most 60 characters; not `jira` or `redmine`.
op-csv-preset-save--values = How values become values here.

## glyphkeel csv-preset delete <name>

op-csv-preset-delete = Delete a CSV import preset of the workspace.
op-csv-preset-delete--name = Its name.
