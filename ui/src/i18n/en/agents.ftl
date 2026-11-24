# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Preferences: Agents & CLI

preferences-page-agents = Agents & CLI

agents-cli = Command-line tool
agents-cli-tool = glyphkeel
agents-cli-outside-app = Where the tool is shows in the desktop app. The set-up below uses the bare command name.
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
agents-limits = Limits
agents-limits-about = Each agent session makes at most { $perMinute ->
        [one] one change
       *[other] { $perMinute } changes
    } a minute and creates at most { $perDay ->
        [one] one new task
       *[other] { $perDay } new tasks
    } a day. With Standard, it moves at most { $perCall ->
        [one] one item
       *[other] { $perCall } items
    } to the trash at a time and { $perHour ->
        [one] one
       *[other] { $perHour }
    } an hour.
agents-raise = Raising the limit
agents-raise-about = Letting agents do more asks you in a window of the desktop, so that an agent can't widen its own rights.

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
