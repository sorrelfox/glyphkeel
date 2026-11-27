# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The command line's own text, in English: its help, the labels of its
# output and its errors. The operations' summaries and their options' help
# come from the operation catalog; the engine's messages are in the
# engine's file. One message per line; placeables are { $name }.

## Help

cli-about = Glyphkeel, a local task manager, from the command line.
cli-long-about = Without a command, starts the desktop app (with `--quick-add`, only its quick-add window). Every other command runs one operation on the workspace, also while the app is open.
cli-exit-codes = Exit codes: 0 success, 1 usage error or invalid input, 2 not found, 3 change rejected, 4 workspace or storage problem.
cli-quick-add-help = Open the desktop app's quick-add window to capture a task, starting the app if it isn't running (a command for a desktop shortcut)
cli-workspace-help = The workspace: a name in the data folder, or a folder path [default: the one last opened, else Personal]
cli-json-help = Print the operation's output as JSON
cli-quiet-help = Print nothing on success; errors still go to standard error
cli-no-color-help = No colours in help and messages (also with NO_COLOR set)
cli-tz-help = The time zone of "today" and local times for this command: "system" or an IANA name such as Europe/Berlin [default: the personal setting]
cli-open-about = Start the desktop app, optionally at tasks or other links
cli-open-links-help = A task key (`TM-41`) or a `glyphkeel://` link
cli-mcp-about = Serve the workspace to an AI agent over MCP (standard input and output, or local HTTP with --http)
cli-mcp-long-about = Serve the workspace to an AI agent over MCP, on standard input and output, or with --http on 127.0.0.1 for clients that connect to an address. The arguments are those of the MCP server; `glyphkeel mcp --help` lists them.
cli-completions-about = Print the shell completion script
cli-man-about = Print the man page (roff)
cli-group-about = Commands: { $commands }
cli-input-help = The whole input as JSON, or `-` to read it from standard input; options given as well take precedence
cli-yes-help = Confirm in advance: this can't be undone

## Values of options

cli-expected-bool = expected `true` or `false`
cli-expected-integer = expected a whole number
cli-expected-number = expected a number
cli-expected-json = expected JSON: { $error }

## Output

cli-error = glyphkeel: { $message }
cli-hint = hint: { $hint }
# One field's change in a table cell: its name, the value before and the value after.
cli-field-change = { $field }: { $before } → { $after }
# A value that wasn't set, in a field's change.
cli-no-value = –
cli-choices = choices: { $choices }
cli-field = field: { $field }
cli-shown-of-total = { $shown } of { $total }
cli-next-page = next page: --cursor { $cursor }
cli-none = (none)
cli-true = yes
cli-false = no
cli-confirm = { $summary }
    This can't be undone. Continue? [y/N]
# The answers that confirm, separated by spaces.
cli-confirm-yes = y yes

## Errors

cli-quick-add-alone = --quick-add opens the app's quick-add window and takes no command.
cli-quick-add-alone-hint = Run `glyphkeel --quick-add` on its own.
cli-man-failed = Can't write the man page: { $error }.
cli-output-failed = Can't write the output: { $error }.
cli-input-unreadable = Can't read the input: { $error }.
cli-input-not-object = The input must be a JSON object.
cli-input-not-json = The input is not valid JSON: { $error }.
cli-no-home = Can't find the home folder, so there is no default data folder.
cli-no-home-hint = Name the workspace folder with --workspace <path>.
cli-workspace-name = "{ $name }" cannot be a workspace name: use letters, digits, spaces, '-' or '_', at most 64 characters.
cli-workspace-name-hint = A name has letters, digits, spaces, '-' and '_'; to name a folder, give a path such as ./my-workspace.
cli-workspace-missing = There is no workspace at { $path }.
cli-workspace-missing-hint = Check the name or path given with --workspace.
cli-workspace-list-about = List the workspaces in the data folder; the one commands work in is marked
cli-workspace-create-about = Make a new, empty workspace in the data folder
cli-workspace-create-name-help = Its name: letters, digits, spaces, '-' and '_'
cli-workspace-rename-about = Rename a workspace in the data folder; nobody may have it open
cli-workspace-rename-old-help = The workspace's name now
cli-workspace-rename-new-help = Its new name
cli-workspace-open-about = Work in this workspace from now on: commands, agents and the app's next start use it
cli-workspace-open-name-help = A name in the data folder, or a folder path
cli-workspace-list-current = { $name } (commands work in this one)
cli-workspace-list-other = { $name }
cli-workspace-list-none = There are no workspaces in the data folder yet.
cli-workspace-created = Made the workspace "{ $name }".
cli-workspace-renamed = Renamed the workspace "{ $old }" to "{ $new }".
cli-workspace-opened = Commands and agents work in "{ $name }" now; the app opens it at its next start.
cli-link-invalid = '{ $link }' is neither a task key nor a glyphkeel:// link.
cli-link-invalid-hint = Give a key such as TM-41, or a link such as glyphkeel://open/TM-41.
cli-app-missing = The desktop app ({ $app }) is neither next to this program nor on PATH.
cli-app-missing-hint = Install the desktop app, or use the commands listed by `glyphkeel --help`.
cli-app-start-failed = Can't start { $app }: { $error }.

## Dashboards, as `dashboard show` prints them

cli-dashboard-heading = { $name }
# A dashboard pinned to a project: its name and the project's key.
cli-dashboard-heading-project = { $name } ({ $project })
cli-dashboard-empty = This dashboard has no widgets yet.
# A widget's heading: its place in the grid and its title.
cli-dashboard-widget = { $place }. { $title }
# A widget that can't be shown, with why.
cli-dashboard-error = ! { $message }
cli-dashboard-tasks = { $count ->
        [one] 1 task
       *[other] { $count } tasks
    }
# A listed task's status and due date.
cli-dashboard-task-due = ({ $status }, due { $due })
# One bar or line of figures: what it counts and how many.
cli-dashboard-bar = { $name }: { $count }
# The bar of the tasks without a value.
cli-dashboard-no-value = (none)
cli-dashboard-created-closed = { $from } to { $to }: { $created } created, { $closed } closed, { $open } open at the end
cli-dashboard-cycle-time = { $count } closed; median cycle time { $cycle } days, median lead time { $lead } days
cli-dashboard-time-total = { $from } to { $to }: { $total } in all
cli-dashboard-burndown = { $sprint }: { $open } of { $total } tasks open
cli-dashboard-version = { $name }: { $closed } of { $total } tasks closed ({ $percent }%)
# One change of the activity: when, who, the operation and the item.
cli-dashboard-change = { $at }  { $actor }  { $op }  { $item }
# One failed automation run: when, the rule and why.
cli-dashboard-run = { $at }  { $rule }: { $message }

## The widgets' own titles

cli-widget-query-count = Matching tasks
cli-widget-query-list = Task list
cli-widget-created-closed = Created and closed
cli-widget-cumulative-flow = Cumulative flow
cli-widget-cycle-time = Cycle time
cli-widget-time-report = Time logged
cli-widget-burndown = Sprint burndown
cli-widget-version-progress = Version progress
cli-widget-tasks-by-field = Tasks by field
cli-widget-due-soon = Due soon
cli-widget-overdue = Overdue
cli-widget-activity = Recent activity
cli-widget-automation-failures = Automation failures
cli-widget-note = Note
