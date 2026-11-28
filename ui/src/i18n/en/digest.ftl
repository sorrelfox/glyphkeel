# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The digest at the top of Today: what agents, automation rules, reminders
# and the command line did while the person was away; the sidebar's Inbox;
# and the activity log of one agent session.

## The panel

digest-title = While you were away
# $time is a moment, formatted for the locale.
digest-since = Since { $time }
digest-dismiss = Mark as seen
digest-dismiss-about = The digest starts again from now; what changes later shows here.

## Agents

digest-agents = Agents
digest-agents-summary = { $sessions ->
    [one] One agent session changed { $changes ->
        [one] one item
       *[other] { $changes } items
    }.
   *[other] { $sessions } agent sessions changed { $changes ->
        [one] one item
       *[other] { $changes } items
    }.
  }
digest-session-changes = { $tasks ->
    [0] { $changes ->
        [one] One change
       *[other] { $changes } changes
    }
    [one] { $changes ->
        [one] One change to one task
       *[other] { $changes } changes to one task
    }
   *[other] { $changes ->
        [one] One change to { $tasks } tasks
       *[other] { $changes } changes to { $tasks } tasks
    }
  }
digest-session-running = Still at work
digest-session-reverted = Reverted
# $time is the moment of the session's latest change.
digest-session-last = Last change { $time }
digest-open-activity = Open in Activity
# The button's name starts with its visible text, then names the session.
digest-open-activity-label = Open in Activity: { $name }
digest-revert-session = Revert session
digest-revert-session-label = Revert session: { $name }
digest-reverted-toast = { $count ->
    [one] One change was reverted.
   *[other] { $count } changes were reverted.
  }
digest-revert-skipped = { $count ->
    [one] One change was left as it is, because something changed it since.
   *[other] { $count } changes were left as they are, because something changed them since.
  }
digest-more-tasks = { $count ->
    [one] and one more task
   *[other] and { $count } more tasks
  }

## Automation

digest-automation = Automation
digest-automation-summary = { $rules ->
    [one] One rule ran { $runs ->
        [one] once.
       *[other] { $runs } times.
    }
   *[other] { $rules } rules ran { $runs ->
        [one] once.
       *[other] { $runs } times.
    }
  }
digest-automation-failures = { $count ->
    [one] One run failed.
   *[other] { $count } runs failed.
  }
digest-rule-runs = { $runs ->
    [one] One run
   *[other] { $runs } runs
  }
digest-rule-failures = { $count ->
    [one] One failed
   *[other] { $count } failed
  }
digest-rule-log = Run log
digest-rule-log-label = Run log: { $name }
digest-rule-revert = Revert its changes
digest-rule-revert-label = Revert its changes: { $name }

## Reminders and due dates

digest-due = Reminders and due dates
digest-reminders-summary = { $count ->
    [one] One reminder came due.
   *[other] { $count } reminders came due.
  }
digest-became-due-summary = { $count ->
    [one] One task became due.
   *[other] { $count } tasks became due.
  }
digest-due-overdue = Overdue
digest-due-today = Due today
digest-reminder = Reminder

## The command line

digest-cli = Command line
digest-cli-summary = { $commands ->
    [one] One command changed { $changes ->
        [one] one item
       *[other] { $changes } items
    }.
   *[other] { $commands } commands changed { $changes ->
        [one] one item
       *[other] { $changes } items
    }.
  }
digest-cli-revert = Revert these changes

## The sidebar

sidebar-inbox = Inbox

## The activity log of one agent session

activity-session-only = Only the changes of one agent session
activity-session-all = Show all changes
