# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## First run: the set-up offered when the workspace was just made

first-run-title = Set up Glyphkeel
first-run-step = Step { $step } of { $steps }
first-run-skip = Skip set-up
first-run-back = Back
first-run-next = Next
first-run-finish = Start working

first-run-welcome = Welcome to Glyphkeel
first-run-workspace = Your tasks are kept on this computer, in the workspace { $name }. Nothing is sent anywhere.
first-run-workspace-shared = The command line and agents work on the same workspace. Preferences › Data and backups shows its backups.

first-run-look = Choose a look
first-run-look-later = Accent colours and more are in Preferences › Appearance.

first-run-project = Start a project
first-run-project-about = A project holds tasks on a board and in a list. Give your first one a name, or skip this step.
first-run-project-exists = { $count ->
    [one] The workspace has one project already. Add another, or go on.
   *[other] The workspace has { $count } projects already. Add another, or go on.
}
first-run-project-placeholder = Project name
first-run-project-optional = Its key, shown before task numbers, comes from the name; you can change it in the project's settings.

first-run-agents = Work with agents
first-run-agents-about = Agents such as Claude Code, Codex or Cursor can read and change your tasks over MCP, with limits you set.
first-run-agents-later = The set-up for each client is in Preferences › Agents & CLI.
first-run-agents-open = Set up agents
