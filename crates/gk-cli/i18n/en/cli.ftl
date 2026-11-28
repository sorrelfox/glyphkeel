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
# The placeholder for the links in the usage line, in capitals.
cli-open-links-value = LINK
cli-mcp-about = Serve the workspace to an AI agent over MCP (standard input and output, or local HTTP with --http)
cli-mcp-long-about = Serve the workspace to an AI agent over MCP, on standard input and output, or with --http on 127.0.0.1 for clients that connect to an address. The arguments are those of the MCP server; `glyphkeel mcp --help` lists them.
cli-completions-about = Print the shell completion script
cli-man-about = Print the man page (roff)
cli-group-about = Commands: { $commands }
cli-input-help = The whole input as JSON, or `-` to read it from standard input; options given as well take precedence
cli-yes-help = Confirm in advance: this can't be undone
cli-lang-help = The language of help, messages and output for people, as a tag such as de or pt-BR [default: GLYPHKEEL_LANG, else the language chosen in the app, else the system's]
cli-completions-shell-help = The shell to write the script for
cli-help-about = Print the help of the program or of the given command
cli-help-commands-help = The command's words, such as `task show`
# The placeholder for those words in the usage line, in capitals.
cli-help-commands-value = COMMAND
# An option's help followed by the words it takes.
cli-help-with-values = { $help } [possible values: { $values }]
# The help's headings, with their punctuation, and the help of the options
# every command has.
cli-help-usage = Usage:
cli-help-commands = Commands:
cli-help-arguments = Arguments:
cli-help-options = Options:
# The words standing for options and for a command in usage lines
# (`glyphkeel task [OPTIONS] <COMMAND>`); keep the brackets.
cli-usage-options = [OPTIONS]
cli-usage-command = <COMMAND>
cli-usage-command-optional = [COMMAND]
# Added to the help of an option that takes several values.
cli-help-many = { $help } (repeat the option or separate values with commas; write \, for a comma inside a value)
cli-help-help = Print help (more with --help)
cli-version-help = Print the version

## Values of options

cli-expected-bool = expected `true` or `false`
cli-expected-integer = expected a whole number
cli-expected-number = expected a number
cli-expected-json = expected JSON: { $error }

## Mistakes on the command line

cli-usage-unknown-argument = unexpected argument '{ $arg }'
cli-usage-unknown-command = unknown command '{ $command }'
cli-usage-missing-command = '{ $command }' needs one of its commands
cli-usage-missing-arguments = missing: { $args }
cli-usage-conflict = '{ $arg }' can't be used with '{ $other }'
cli-usage-repeated = '{ $arg }' can be given only once
cli-usage-no-value = '{ $arg }' needs a value
cli-usage-invalid-value = invalid value '{ $value }' for '{ $arg }'
cli-usage-invalid-value-why = invalid value '{ $value }' for '{ $arg }': { $reason }
cli-usage-invalid-value-choices = invalid value '{ $value }' for '{ $arg }'; possible values: { $values }
cli-usage-no-equals = '{ $arg }' needs its value after an equals sign ('{ $arg }=…')
cli-usage-value-count = '{ $arg }' takes { $count ->
    [one] one value
   *[other] { $count } values
  }, not { $given }
cli-usage-wrong-values = '{ $arg }' got the wrong number of values
cli-usage-invalid-utf8 = an argument isn't valid UTF-8 text
cli-usage-output-failed = the help couldn't be written: { $error }
cli-usage-output-failed-plain = the help couldn't be written
# Any other mistake on the command line.
cli-usage-other = the command line isn't valid
# A correction clap suggests: a command, an option or a value.
cli-usage-did-you-mean = did you mean '{ $suggestion }'?
cli-usage-line = Usage: { $usage }
cli-usage-more = For more information, add --help.
cli-help-unknown = There is no command '{ $command }'.

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
cli-workspace-encrypt-about = Encrypt the workspace with a passphrase, or change its passphrase; nobody may have it open
cli-workspace-encrypt-long = Encrypt the workspace's database with a passphrase, or give an encrypted one a new passphrase. Nobody may have the workspace open: close the app, other commands and agent servers first. A backup is made first, and the backups in its backup folder are encrypted too. Stored files (attachments) stay unencrypted. The passphrase is asked twice on a terminal, or read as one line from standard input. Without the passphrase the data can't be recovered.
cli-workspace-decrypt-about = Store the workspace unencrypted again; nobody may have it open
cli-workspace-encrypted = Encrypted the workspace "{ $name }". The backup made before is { $backup }.
cli-workspace-decrypted = The workspace "{ $name }" is no longer encrypted. The backup made before is { $backup }.
cli-passphrase-prompt = Passphrase for "{ $name }":
cli-passphrase-wrong = That passphrase does not open the workspace.
cli-passphrase-export = Passphrase of the encrypted export:
cli-passphrase-new = New passphrase:
cli-passphrase-repeat = The same passphrase again:
cli-passphrase-mismatch = The two passphrases differ.
cli-passphrase-empty = The passphrase is empty.
cli-passphrase-empty-hint = Type a passphrase, or pipe one line into the command.
cli-passphrase-unreadable = The passphrase could not be read: { $error }
cli-link-invalid = '{ $link }' is neither a task key nor a glyphkeel:// link.
cli-link-invalid-hint = Give a key such as TM-41, or a link such as glyphkeel://open/TM-41.
cli-app-missing = The desktop app ({ $app }) is neither next to this program nor on PATH.
cli-app-missing-hint = Install the desktop app, or use the commands listed by `glyphkeel --help`.
cli-app-start-failed = Can't start { $app }: { $error }.

## Boards, as `board` prints them

# The board's heading: the project's key.
cli-board-heading = Board of { $project }
# A column's heading: the status and how many tasks it holds.
cli-board-column = { $name } ({ $count ->
        [one] 1 task
       *[other] { $count } tasks
    })
cli-board-limit = limit { $limit }
cli-board-over-limit = over its limit of { $limit }
cli-board-under-minimum = under its minimum of { $minimum }
cli-board-collapsed = collapsed
cli-board-no-cards = no cards
# A card's coloured edge: what it stands for (a colour rule's name, the type, label or priority).
cli-board-edge = edge: { $name }
# A card's cover: the image's file name.
cli-board-cover = cover: { $name }
cli-board-older = { $count ->
        [one] 1 older closed task not shown
       *[other] { $count } older closed tasks not shown
    }
cli-board-hidden = { $columns ->
        [one] 1 hidden column
       *[other] { $columns } hidden columns
    }, with { $tasks ->
        [one] 1 task
       *[other] { $tasks } tasks
    }

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
# The title of a widget that counts the open tasks (query is:open), as the starter dashboard has.
cli-widget-open-tasks = Open tasks
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
