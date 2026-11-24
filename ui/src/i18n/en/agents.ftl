# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Preferences: Agents & CLI

preferences-page-agents = Agents & CLI

agents-cli = Command-line tool
agents-cli-tool = glyphkeel
agents-cli-outside-app = The desktop app shows where the tool is. The set-up below uses the bare command name.
agents-cli-flatpak = Glyphkeel runs as a Flatpak, so agents start the tool through Flatpak with this command.
agents-cli-on-path = The shell finds it at { $path }.
agents-cli-on-path-chip = On the PATH
agents-cli-not-on-path = The shell doesn't find it. Installing puts a link to the copy that came with the app at { $link }.
agents-cli-not-on-path-dir = The shell doesn't find it. Installing puts a link to the copy that came with the app at { $link }; that folder is not on your PATH yet, so add it in your shell's settings too.
agents-cli-install = Install command-line tool
agents-cli-installed = The command-line tool is installed.
agents-cli-missing = It is not on the PATH, and no copy came with the app. Install it with your package manager.
agents-cli-missing-chip = Not found

agents-clients = Connect an agent
agents-clients-about = Agents connect over MCP and work on this workspace while the app is open or closed.
agents-client = Client
agents-client-run = Run this in a terminal to add Glyphkeel to { $client }:
agents-client-file = Add this to { $client }'s settings file, { $file }:
agents-client-snippet = Set-up for { $client }
agents-project-scope = Only one project
agents-project-scope-about = For a repository: the agent sees only this project and its sub-projects.
agents-project-file = Put this in the repository's { $file }:
agents-project-snippet = Set-up limited to { $project }
agents-copy = Copy
agents-copied = Copied
agents-copy-failed = The text could not be copied. Select it and copy it by hand.

agents-policy = What agents may do
agents-max-profile = Most an agent may do
agents-profile-read-only = Read only
agents-profile-standard = Standard
agents-profile-full = Full
agents-profile-read-only-about = Agents can read, search and report, but change nothing.
agents-profile-standard-about = Agents can create and change tasks, and move items to the trash, but delete nothing for good.
agents-profile-full-about = Agents can do everything you can, including emptying the trash and deleting projects.
agents-raise = Letting agents do more
agents-raise-about = Letting agents do more, or raising their limits, asks you in a window of the desktop, so that an agent can't widen its own rights.

agents-limits = Limits on each agent session
agents-limits-only-here = Who can change them
agents-limits-only-here-about = Only you, here in the app. Agents and the command line can't change these limits or undo your changes to them.
agents-no-limit = No limit
agents-limit-writes = Changes a minute
agents-limit-writes-about = Calls that change something, in any minute. Default: { $default }.
agents-limit-writes-unit = a minute
agents-limit-writes-none = No limit on changes a minute
agents-limit-tasks = New tasks a day
agents-limit-tasks-about = Tasks a session creates in any 24 hours. Default: no limit.
agents-limit-tasks-unit = a day
agents-limit-tasks-none = No limit on new tasks a day
agents-limit-trash-call = Items to the trash at a time
agents-limit-trash-call-about = With the Standard profile, in one call; subtasks count. Default: { $default }.
agents-limit-trash-call-unit = per call
agents-limit-trash-call-none = No limit on items to the trash at a time
agents-limit-trash-hour = Items to the trash an hour
agents-limit-trash-hour-about = With the Standard profile, in any hour. Default: { $default }.
agents-limit-trash-hour-unit = an hour
agents-limit-trash-hour-none = No limit on items to the trash an hour

agents-sessions = Agent sessions
agents-sessions-none = No agent has connected yet.
agents-sessions-none-about = Each time an agent starts the MCP server, a session appears here, with what it read and changed.
agents-sessions-more = { $count ->
    [one] One older session is not shown.
   *[other] { $count } older sessions are not shown.
}
agents-session-name-version = { $name } { $version }
agents-session-running = Connected
agents-session-since = Since { $started }, last active { $seen }
agents-session-ran = From { $started } to { $ended }
agents-session-counts = { $reads ->
    [one] One read
   *[other] { $reads } reads
}, { $writes ->
    [one] one change
   *[other] { $writes } changes
}
agents-revert = Revert session…
agents-revert-nothing = Nothing of this session can be reverted.
agents-revert-confirm = { $count ->
    [one] Revert the one change { $name } made in this session?
   *[other] Revert the { $count } changes { $name } made in this session?
} Changes made since to the same items are kept.
agents-revert-confirm-button = { $count ->
    [one] Revert one change
   *[other] Revert { $count } changes
}
agents-reverted = { $skipped ->
    [0] { $reverted ->
        [one] One change reverted
       *[other] { $reverted } changes reverted
    }
   *[other] { $reverted ->
        [one] One change reverted
       *[other] { $reverted } changes reverted
    }; { $skipped ->
        [one] one was left alone
       *[other] { $skipped } were left alone
    }
}
