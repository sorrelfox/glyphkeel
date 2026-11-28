<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Generated from the operation catalog by `cargo xtask gen`. Do not edit. -->

# Command line

`glyphkeel <area> <verb> [arguments] [--options]`. Every command runs one operation of the catalog ([operations.md](operations.md)). Global options: `--workspace`, `--json`, `--quiet`, `--no-color`, `--tz`, `--lang`.

With `--json` a command prints exactly the operation's output, as described in [catalog.schema.json](catalog.schema.json).

| Command | Operation | What it does |
|---|---|---|
| `glyphkeel activity` | `history.feed` | The activity log, newest first: who changed what, when and through which program; optionally the agent sessions. |
| `glyphkeel agents policy` | `agent.policy.update` | Set the most an agent may do, whatever profile it asks for, whether agents may ask the person to allow one change beyond it, and which of their changes wait for the person's approval. |
| `glyphkeel agents sessions` | `agent.sessions` | List agent sessions, newest first, with their client, profile, times and counts of reads, writes and reads refused by the read limits. |
| `glyphkeel automation create` | `automation.create` | Make an automation rule: when a trigger happens to a task that matches a query, set fields, change labels, move it, comment, create a subtask or linked task, or leave a notification. |
| `glyphkeel automation delete <rule>` | `automation.delete` | Delete an automation rule; revert brings it back. |
| `glyphkeel automation edit <rule>` | `automation.update` | Change an automation rule: its name, trigger, condition or actions, or switch it on or off. |
| `glyphkeel automation list` | `automation.list` | The automation rules of a project and of the workspace, each with its trigger, condition, actions and latest run. |
| `glyphkeel automation run` | `automation.run` | Run the scheduled and due date automation rules whose time has come; the app does this by itself. |
| `glyphkeel automation runs` | `automation.runs` | The run log of automation rules, newest first: what set each run off, the task, the changes and any error. |
| `glyphkeel automation show <rule>` | `automation.get` | One automation rule with its last runs. |
| `glyphkeel automation test <task>` | `automation.test` | A dry run of an automation rule, saved or not, against one task: whether it matches and the changes the actions would make. Nothing is changed. |
| `glyphkeel backup create` | `backup.create` | Make a backup of the workspace now: a checked copy of the database and its stored files. |
| `glyphkeel backup delete <name>` | `backup.delete` | Delete a backup and the stored files only it used. |
| `glyphkeel backup list` | `backup.list` | The workspace's backups, newest first, with their kind and size. |
| `glyphkeel backup restore <name>` | `backup.restore` | Replace the workspace with a backup, after backing up the current state. Other programs with the workspace open must restart. |
| `glyphkeel backup verify <name>` | `backup.verify` | Check a backup: the database copy's integrity and every stored file it needs. |
| `glyphkeel board <project>` | `board.get` | A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state, compact cards and the swimlanes with their counts. |
| `glyphkeel board all` | `board.get_across` | The open work of every active project on one board: a column for each status category (to do, in progress, recently done), cards with their project and real status, filtered by a query or a folder, in swimlanes by project by default. |
| `glyphkeel board layout set <project>` | `layout.update` | Hide, show, collapse or reorder board columns, set sort modes, card fields, card colours (by type, priority, label or ordered query rules), covers, swimlanes and older closed cards, or reset to the project's layout. |
| `glyphkeel board layout show <project>` | `layout.get` | The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields, card colours, covers and swimlanes. |
| `glyphkeel comment add <key>` | `comment.add` | Add a Markdown comment to a task, optionally answering another. |
| `glyphkeel comment delete <comment>` | `comment.delete` | Move a comment to the trash. |
| `glyphkeel comment edit <comment>` | `comment.update` | Change a comment's text; the earlier text is kept and can be viewed. |
| `glyphkeel comment list <key>` | `comment.list` | A task's comments, oldest first, optionally with their earlier texts. |
| `glyphkeel csv-preset delete <name>` | `csv_preset.delete` | Delete a CSV import preset of the workspace. |
| `glyphkeel csv-preset list` | `csv_preset.list` | List the CSV import presets: the built-in ones for Jira and Redmine, and the workspace's own. |
| `glyphkeel csv-preset save <name>` | `csv_preset.save` | Save a CSV import mapping as a preset of the workspace, replacing one with the same name. |
| `glyphkeel dashboard add-widget <dashboard>` | `dashboard.add_widget` | Add a widget to a dashboard, at the end or at a given place. |
| `glyphkeel dashboard create` | `dashboard.create` | Make a dashboard for the whole workspace or pinned to a project, empty, with the starter widgets or with the widgets given. |
| `glyphkeel dashboard delete <dashboard>` | `dashboard.delete` | Delete a dashboard; it can be brought back with revert. |
| `glyphkeel dashboard edit <dashboard>` | `dashboard.update` | Rename a dashboard or change its description. |
| `glyphkeel dashboard edit-widget <dashboard> <widget>` | `dashboard.update_widget` | Change a widget's title or settings, make it wider or narrower, or move it to another place in the grid. |
| `glyphkeel dashboard list` | `dashboard.list` | The dashboards of the whole workspace or of a project, with their widgets and settings. |
| `glyphkeel dashboard remove-widget <dashboard> <widget>` | `dashboard.remove_widget` | Remove a widget from a dashboard. |
| `glyphkeel dashboard show <dashboard>` | `dashboard.show` | One dashboard with what each of its widgets shows now: counts, tasks, report figures, burndown, versions, changes, failed automation runs and notes. |
| `glyphkeel dashboard widget <dashboard> <widget>` | `dashboard.widget` | What one widget of a dashboard shows now. |
| `glyphkeel description history` | `description.history` | List the revisions of a task's or project's description, newest first, conflict copies included. |
| `glyphkeel description show <revision>` | `description.revision` | One description revision with its text, and the differences from another revision on request. |
| `glyphkeel digest dismiss` | `digest.dismiss` | Mark the digest as seen up to a moment, so that the next one starts there. |
| `glyphkeel digest show` | `digest.get` | What changed without the person since a moment (by default since they last looked): agent sessions with their changes and tasks, automation rules with their runs and failures, reminders and tasks that came due, and changes through the command line (for an agent, also the person's changes in the app), with counts. |
| `glyphkeel export` | `transfer.export` | Export projects, or the whole workspace, to an archive of JSON and Markdown files with the attachments. |
| `glyphkeel field add` | `field.create` | Add a custom field to a project, or to every project: text, Markdown, whole number, decimal, date, yes/no, select, multi-select or link; with options, a default, the task types it shows for, and whether it is required. |
| `glyphkeel field delete` | `field.delete` | Delete a custom field with its values on every task; saved queries lose their conditions on it. Can't be undone. |
| `glyphkeel field edit` | `field.update` | Rename a custom field, rename, add, reorder or remove its options, change its default, task types or whether it is required, or move it; saved queries keep up with renames. |
| `glyphkeel field list` | `field.list` | The custom fields of a project (its own and those of every project) or of the whole workspace: name, query name, kind, options, default, task types, required, and how many tasks have a value. |
| `glyphkeel file attach` | `file.attach` | Attach a local file, or Base64 data with a name, to a task, a comment or a wiki page. |
| `glyphkeel file cat <attachment>` | `file.read` | An attached file's content: text for text files, Base64 for images, otherwise only its details. |
| `glyphkeel file cover <task>` | `file.cover` | Choose the PNG, JPEG, GIF or WebP image attached to a task that its board card shows as its cover, or choose none; boards show covers when their layout's covers setting is on. |
| `glyphkeel file delete <attachment>` | `file.delete` | Move an attached file to the trash. |
| `glyphkeel file show <attachment>` | `file.get` | An attached file's name, type, size and owner. |
| `glyphkeel folder create` | `folder.create` | Make a folder to group top-level projects in the sidebar. |
| `glyphkeel folder delete <folder>` | `folder.delete` | Delete a folder; its projects stay and go back to the top level. Revert brings it back. |
| `glyphkeel folder edit <folder>` | `folder.update` | Rename a folder, change its colour or move it among the folders. |
| `glyphkeel folder list` | `folder.list` | The project folders in sidebar order, each with its projects and their open tasks. |
| `glyphkeel folder move <project>` | `folder.move` | Put a top-level project into a folder or take it out, or move it within its folder. |
| `glyphkeel folder rename <folder> <name>` | `folder.rename` | Rename a folder. |
| `glyphkeel import <path>` | `transfer.import` | Import an archive as new projects; checks it completely first and never changes existing data. |
| `glyphkeel import csv <path>` | `transfer.import_csv` | Import tasks from a CSV file into a project, with a column mapping or a preset (Jira and Redmine exports are built in); a dry run reports every row's outcome, and the import is one change that Undo takes back. Status rules hold for each task as for task.create; repeating tasks are not imported. |
| `glyphkeel label add` | `label.create` | Add a label to a project; without a colour it gets the first palette colour the project's labels don't use yet. |
| `glyphkeel label delete` | `label.delete` | Delete a label and take it off every task. Can't be undone. |
| `glyphkeel label edit` | `label.update` | Rename or recolour a label. |
| `glyphkeel project archive <project>` | `project.archive` | Archive a project and its sub-projects: they become read-only and leave the sidebar. |
| `glyphkeel project brief <project>` | `project.brief` | The project's instructions for agents: the wiki page marked as the brief, else the project's description. |
| `glyphkeel project create` | `project.create` | Create a project, optionally in a folder, or a sub-project that copies its parent's statuses, types and labels; without a colour it gets one its sibling projects don't use yet. |
| `glyphkeel project delete <project>` | `project.delete` | Move a project and its sub-projects, with everything in them, to the trash. Agents need the full profile. |
| `glyphkeel project edit <project>` | `project.update` | Change a project: name, description, colour, icon, modules, key, place in the tree, archived; add, change, reorder or remove statuses, types and labels, all in one change. |
| `glyphkeel project list` | `project.list` | The project tree in sidebar order, with keys, names and open-task counts; archived projects on request. |
| `glyphkeel project move <project>` | `project.move` | Move a project under another one (at most 3 levels), to the top level, or among its siblings. |
| `glyphkeel project rekey <project>` | `project.change_key` | Give a project a new key; the old key and the old task keys keep working. |
| `glyphkeel project rules <project>` | `rules.update` | Change a project's status rules (fields set, cleared or required when a task enters a status, fields read-only in it, the timer and claims) and its rules for agents (statuses they may not move tasks into, fields they may not change). |
| `glyphkeel project show <project>` | `project.get` | One project with its statuses (board order, category, WIP limit), task types, labels, modules, sub-projects and counts. |
| `glyphkeel project unarchive <project>` | `project.unarchive` | Unarchive a project and its sub-projects. |
| `glyphkeel proposals approve <ids>` | `proposal.approve` | Approve proposed changes, by ID or every waiting one of an agent session: each runs as the agent, and history records that the person approved it. |
| `glyphkeel proposals list` | `proposal.list` | List changes agents proposed, newest first, with their status, a preview of what each changes, the fields changed since and the person's decision. An agent sees its own. |
| `glyphkeel proposals reject <ids>` | `proposal.reject` | Reject proposed changes, by ID or every waiting one of an agent session, with a reason the agent can read. |
| `glyphkeel proposals show <id>` | `proposal.get` | One proposed change: what it runs, its preview, the fields changed since and the decision. |
| `glyphkeel recurrence clear <key>` | `recurrence.clear` | Stop a task repeating: it stays, and no more occurrences are made. |
| `glyphkeel recurrence preview` | `recurrence.preview` | Read a recurrence rule, in words or by fields, and list its next dates. |
| `glyphkeel recurrence run` | `recurrence.run` | Make the occurrences of series that are made ahead whose day has come; the app does this by itself. |
| `glyphkeel recurrence set <key>` | `recurrence.set` | Make a task recurring, change its rule, or (`update_details`) make its current details those of the occurrences to come. |
| `glyphkeel recurrence skip <key>` | `recurrence.skip` | Pass over a recurring task's current occurrence: its dates move to the next date of the rule. |
| `glyphkeel reminder ack <ids>` | `reminder.acknowledge` | Acknowledge reminders that were shown, so they don't come due again. |
| `glyphkeel reminder clear <key>` | `reminder.clear` | Remove a task's personal reminder. |
| `glyphkeel reminder due` | `reminder.due` | The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes. |
| `glyphkeel reminder set <key>` | `reminder.set` | Set a task's personal reminder: a moment, or a number of days before its due date. |
| `glyphkeel report burnup` | `report.burnup` | A burn-up of a version or a sprint: the work planned for it and the part completed at the end of each day, from the history, with a forecast of when the two meet. |
| `glyphkeel report created-closed` | `report.created_closed` | Tasks created against tasks closed and reopened per day, week or month, with the open tasks at the end of each period; from the history, with optional CSV. |
| `glyphkeel report cycle-time` | `report.cycle_time` | Cycle time (first move into progress to the last close) and lead time (creation to the last close) of the tasks closed over a range of days, with percentiles per period; optional task list and CSV. |
| `glyphkeel report estimates` | `report.estimates` | Estimate accuracy: for tasks closed over a range of days with an estimate and logged time, the median ratio of spent to estimated time, the share within 20 %, and the outliers, overall and by type, label, project, kind of work or month. |
| `glyphkeel report flow` | `report.cumulative_flow` | How many tasks were in each status (or category) at the end of each day, week or month; from the history, with optional CSV. |
| `glyphkeel report velocity <project>` | `report.velocity` | The velocity of a project's completed sprints: the work each took on at its start against the work it completed, in tasks, estimated hours or a number field, with the average of the last three and the work planned for the sprints to come. |
| `glyphkeel revert` | `history.revert` | Revert history groups or everything an agent session changed. Groups that something changed since are skipped and listed. |
| `glyphkeel schedule` | `schedule.get` | The schedule of a project or a query for the Gantt chart: dated tasks as a tree with summary ranges for parents, milestones, the tasks without dates, blocks links as finish-to-start dependencies, and problems such as a task starting before its blocker ends. |
| `glyphkeel search <text>` | `search.query` | Search task titles and descriptions, comments, wiki pages, file names and the text inside attached files for words and fragments, ranked, with the matches marked; a task key finds that task. |
| `glyphkeel settings get` | `settings.get` | Read the workspace settings. |
| `glyphkeel settings set` | `settings.update` | Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder. |
| `glyphkeel sprint add <sprint>` | `sprint.add_tasks` | Put tasks in a planned or active sprint of their project, moving them from any other sprint. |
| `glyphkeel sprint burndown <sprint>` | `sprint.burndown` | The burndown of an active or closed sprint: its open and total tasks and estimates at the end of each day. |
| `glyphkeel sprint complete <sprint>` | `sprint.complete` | Complete the active sprint and move its open tasks to a planned sprint or back to the backlog. |
| `glyphkeel sprint create` | `sprint.create` | Plan a sprint in a project: name, goal, start and end dates, and tasks to put in it. |
| `glyphkeel sprint delete <sprint>` | `sprint.delete` | Delete a planned or closed sprint; its tasks go back to the backlog. Revert brings both back. |
| `glyphkeel sprint edit <sprint>` | `sprint.update` | Rename a sprint or change its goal or dates. |
| `glyphkeel sprint list <project>` | `sprint.list` | The sprints of a project with their state, dates and progress; with their tasks and the backlog for the sprints page. |
| `glyphkeel sprint remove` | `sprint.remove_tasks` | Take tasks out of their sprint, back to the backlog. |
| `glyphkeel sprint show <sprint>` | `sprint.get` | One sprint with its progress and its tasks. |
| `glyphkeel sprint start <sprint>` | `sprint.start` | Start a planned sprint, with its dates; a project has one active sprint at a time. |
| `glyphkeel status add` | `status.create` | Add a status (a board column) to a project. |
| `glyphkeel status delete` | `status.delete` | Delete a status; its tasks move to the status named. Can't be undone. |
| `glyphkeel status edit` | `status.update` | Rename or recolour a status, change its category or its WIP limit. |
| `glyphkeel status move` | `status.reorder` | Move a status in the project's board order. |
| `glyphkeel task backlinks <key>` | `task.backlinks` | The wiki pages, other tasks' descriptions and comments that mention a task by its key (current or old). |
| `glyphkeel task bulk` | `task.bulk_update` | Make the same change to many tasks chosen by key or query (status, type, priority, labels, parent, dates set or shifted, estimate, custom fields, project), previewed first, as one change for Undo and revert. |
| `glyphkeel task check <key>` | `checklist.update` | Tick, clear, add or remove checklist items in a task's description, a comment or a wiki page, by number plus expected text. |
| `glyphkeel task claim <key>` | `task.claim` | Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress. |
| `glyphkeel task context <key>` | `task.context` | Everything needed to work on one task, in one read within a size: the task with its checklist, fields, sprint and version, its parents, subtasks and links, the recent comments and changes, the status rules, the project's agent brief, the wiki pages it links to, and waiting proposals; says what was trimmed. |
| `glyphkeel task create` | `task.create` | Create a task from a title alone or with its fields; it gets the project's next key. |
| `glyphkeel task delete <keys>` | `task.delete` | Move tasks to the trash, each with its subtasks; they can be restored from there. |
| `glyphkeel task duplicate <key>` | `task.duplicate` | Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone. |
| `glyphkeel task edit <key>` | `task.update` | Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress. |
| `glyphkeel task link <key>` | `relation.add` | Link two tasks (blocks, with an optional lag in days; relates, duplicates, clones; or make one a subtask of the other). |
| `glyphkeel task list` | `task.search` | List the tasks matching a query (is:open due<+7d label:bug sort:due), a page at a time, with totals of estimates and logged time. |
| `glyphkeel task move <key>` | `task.move` | Move a task to another status or place in its column, or to another project (new key; the old one forwards); closing it can close its duplicates too. |
| `glyphkeel task move-check <keys>` | `task.move_check` | What moving tasks to another project would do: their new keys, and which of their statuses, types and labels the target project has no match for. |
| `glyphkeel task references <key>` | `task.references` | The pages and tasks a task's description and comments link to, in order, each resolved now; links that find nothing are marked. |
| `glyphkeel task release <key>` | `task.release` | Give up a claim on a task, optionally moving it and adding a comment. |
| `glyphkeel task reorder-subtask <key>` | `task.reorder_subtask` | Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order. |
| `glyphkeel task resolve <key>` | `task.resolve_conflict` | Resolve a conflict copy of a task's description: keep the description, use the copy, or save a combined text. |
| `glyphkeel task show <keys>` | `task.get` | One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request. |
| `glyphkeel task unlink <key>` | `relation.remove` | Remove a link between two tasks, or a subtask from its parent. |
| `glyphkeel template create <name>` | `template.create` | Make a task template (from a task, or given) or a project template (from a project, or given). |
| `glyphkeel template delete <template>` | `template.delete` | Delete a template; it can be brought back with revert. What was made from it stays. |
| `glyphkeel template edit <template>` | `template.update` | Change a template's name, description or content. |
| `glyphkeel template list` | `template.list` | The project templates and the task templates of a project or of every project. |
| `glyphkeel template show <template>` | `template.get` | One template with its content. |
| `glyphkeel template use <template>` | `template.apply` | Use a template: a task template makes a task with its subtasks; a project template makes a project with its set-up, pages and tasks. |
| `glyphkeel time delete <entry>` | `time.delete` | Move a time entry to the trash. |
| `glyphkeel time edit <entry>` | `time.update` | Change a time entry's duration, day, note or kind of work. |
| `glyphkeel time log <key>` | `time.log` | Log time on a task: a duration, the day of the work (default today), a note and a kind of work. |
| `glyphkeel time report` | `time.report` | Logged time over a range of days, grouped by day, week, month, task, project, person or kind of work, with totals in the workspace's units and optional CSV. |
| `glyphkeel timer pause` | `timer.pause` | Pause the running timer; it keeps its time. |
| `glyphkeel timer start <key>` | `timer.start` | Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged. |
| `glyphkeel timer status` | `timer.get` | The workspace's timer: its task, whether it runs and the time so far. |
| `glyphkeel timer stop` | `timer.stop` | Stop the timer and log its time as an entry on its task. |
| `glyphkeel today` | `today.get` | What needs attention today across projects: overdue tasks, tasks due today, tasks in progress, today's reminders, agents' claims, recent changes and the timer, with counts for the sidebar. |
| `glyphkeel trash list` | `trash.list` | The trash: deleted items with what went with them, who deleted them and when they will be purged, newest first. |
| `glyphkeel trash purge <items>` | `trash.purge` | Delete items in the trash for good, and their text from the history. Can't be undone. |
| `glyphkeel trash restore <items>` | `trash.restore` | Bring items back out of the trash, with everything that was deleted with them. |
| `glyphkeel type add` | `type.create` | Add a task type to a project. |
| `glyphkeel type delete` | `type.delete` | Delete a task type; its tasks get another type. Can't be undone. |
| `glyphkeel type edit` | `type.update` | Rename, reshape, recolour or reorder a task type, or make it the default. |
| `glyphkeel version create` | `version.create` | Create a version of a project: name, description, status, start and due dates, and whether its sub-projects can use it. |
| `glyphkeel version delete <version>` | `version.delete` | Delete a version; its tasks are planned for no version. Revert brings both back. |
| `glyphkeel version edit <version>` | `version.update` | Rename a version, change its description, dates, sharing or status, or close it and move its open tasks to another version. |
| `glyphkeel version list <project>` | `version.list` | The versions of a project and those its ancestors share with it, with status, dates and progress; with their tasks for the roadmap. |
| `glyphkeel version show <version>` | `version.get` | One version with its progress and its tasks. |
| `glyphkeel view create` | `view.create` | Save a query as a named view of a project or of the whole workspace, with its columns, order, grouping and totals. |
| `glyphkeel view delete <view>` | `view.delete` | Delete a saved view; it can be brought back with revert. |
| `glyphkeel view edit <view>` | `view.update` | Rename a saved view, change its query, columns, order, grouping or totals, or make it the default. |
| `glyphkeel view list` | `view.list` | The saved views of a project or of the whole workspace: name, query, columns, order, grouping, totals and which is the default. |
| `glyphkeel wiki backlinks <page>` | `wiki.backlinks` | The wiki pages, task descriptions and comments that link to a wiki page, by its title, slug, an old slug or its ID. |
| `glyphkeel wiki delete <page>` | `wiki.delete` | Move a wiki page, with the pages inside it, to the trash; links to it stop resolving until it is restored. |
| `glyphkeel wiki export <page>` | `wiki.export` | Export a wiki page, optionally with the pages inside it, or a project's whole wiki, as Markdown files or web pages with their attachments, in a .zip file or a folder; links between the exported pages and to the attachments become relative links. |
| `glyphkeel wiki get <page>` | `wiki.get` | One wiki page with its text, revision, headings, place in the tree and, on request, what links to it; or an older revision of it. |
| `glyphkeel wiki history <page>` | `wiki.history` | List the revisions of a wiki page, newest first, conflict copies included; read one with wiki.get and its revision. |
| `glyphkeel wiki import <path>` | `wiki.import` | Import a folder of Markdown files as wiki pages: sub-folders become pages inside pages, relative links between the files and `[[…]]` links become wiki links, and linked images and files are attached to their pages; one change that Undo takes back. |
| `glyphkeel wiki list` | `wiki.list` | A project's wiki pages as a tree: titles, slugs, nesting and when each changed. |
| `glyphkeel wiki move <page>` | `wiki.move` | Move a wiki page, with the pages inside it, into another parent page or to the top, and to a place among its siblings; refuses moves into itself; links to it keep working. |
| `glyphkeel wiki new` | `wiki.create` | Create a wiki page with a title unique in its project, optionally inside another page and with its text or a template's. |
| `glyphkeel wiki put <page>` | `wiki.update` | Change a wiki page: its text (whole, with a base revision, or by targeted edits), title, parent or place, whether it is the home page or a template; or resolve a conflict copy. |
| `glyphkeel wiki references <page>` | `wiki.references` | The pages and tasks a wiki page's text links to, in order, each resolved now; links that find nothing are marked. |
| `glyphkeel wiki search <text>` | `wiki.search` | Find wiki pages by title, heading or text, for example to link one; `Page#Heading` lists a page's headings. |
| `glyphkeel work-kind create` | `work_kind.create` | Add a kind of work that time entries can be marked with. |
| `glyphkeel work-kind delete <kind>` | `work_kind.delete` | Delete a kind of work that no time entry names; archive one that is in use. |
| `glyphkeel work-kind edit <kind>` | `work_kind.update` | Rename a kind of work, or archive it so it is no longer offered while its entries keep it. |
| `glyphkeel work-kind list` | `work_kind.list` | The workspace's kinds of work for logged time (Development, Design, Meeting…), by name, with how many entries use each. |
| `glyphkeel workspace` | `workspace.get` | The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings, the limits on agents and a short guide. |

## Options with several values

An option that takes a list (`--labels`, `--options`, `--include`…) takes its values either repeated or separated by commas: `--labels bug --labels ui` and `--labels bug,ui` are the same. Spaces around the values are dropped. Write `\,` for a comma inside a value (`--labels 'needs\, maybe'`), or give the input as JSON with `--input`, which is always taken exactly. The same goes for arguments that take several values (`glyphkeel task show TM-1,TM-2`).

## Workspaces

A command works in the workspace `--workspace` names (a name in the data folder, or a folder path), else the one `GLYPHKEEL_WORKSPACE` names, else the one last opened in the app or with `glyphkeel workspace open`, else `Personal`. `glyphkeel mcp` chooses the same way. These commands work on the workspaces of the data folder rather than inside one:

| Command | What it does |
|---|---|
| `glyphkeel workspace list` | List the workspaces; the one commands work in is marked |
| `glyphkeel workspace create <name>` | Make a new, empty workspace |
| `glyphkeel workspace rename <old> <new>` | Rename a workspace that nobody has open |
| `glyphkeel workspace encrypt` | Encrypt the workspace with a passphrase, or give it a new one; nobody may have it open |
| `glyphkeel workspace decrypt` | Store the workspace unencrypted again; nobody may have it open |
| `glyphkeel workspace open <name or path>` | Work in this workspace from now on: commands, agents and the app's next start use it |

## Language

Help, output for people and error messages are in the language `--lang` names (a language tag such as `de` or `pt-BR`), else the one `GLYPHKEEL_LANG` names, else the language chosen in the app's Preferences, else the system's (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`; on Windows the display language), else English. A message not yet translated shows in English. Numbers, dates and times in the output for people follow the language; times are shown in the time zone `--tz` names, else the personal setting, else the system's. `en-XA` is a pseudo-language for testing translations. Machine text stays English: `--json` output (an error's `message`, with its `message_id` and `args` for translating it), command and option names, and everything `glyphkeel mcp` serves to agents.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error or invalid input (`invalid`) |
| 2 | Not found (`not_found`) |
| 3 | Change rejected (`conflict`, `forbidden`, `limit`) |
| 4 | Workspace or storage problem (`busy`, `workspace_upgraded`, `workspace_too_new`, `storage`) |
