# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors

## Approvals: changes agents proposed, waiting for the person

view-approvals = Approvals
action-approvals = Approvals
approvals-badge = { $count ->
    [one] One change waits for your approval
   *[other] { $count } changes wait for your approval
}
approvals-toolbar = Proposed changes
approvals-filter = Show
approvals-filter-pending = Waiting
approvals-filter-approved = Approved
approvals-filter-rejected = Rejected
approvals-filter-expired = Expired
approvals-about = Changes agents proposed wait here until you approve or reject them. An approved change runs as the agent, and its history notes that you approved it.
approvals-none = No changes wait for your approval.
approvals-none-decided = There are none.
approvals-session = { $agent } · session { $session }
approvals-session-count = { $count ->
    [one] One change
   *[other] { $count } changes
}
approvals-approve = Approve
approvals-approve-anyway = Approve anyway
approvals-approve-all = Approve all in this session
approvals-reject = Reject…
approvals-being-approved = Being approved…
approvals-interrupted = Its approval began { $time } and was interrupted: the program running it stopped. Check the activity log for whether the change was made, then set it aside. It never runs again by itself.
approvals-set-aside = Set aside…
approvals-set-aside-title = Set the change aside
approvals-set-aside-confirm = Set aside
approvals-last-error = The last approval failed: { $error }
approvals-open-task = Open task
approvals-proposed = Proposed { $time }
approvals-expires = Expires { $time }
approvals-decided = { $status ->
    [approved] Approved { $time }
    [rejected] Rejected { $time }
   *[expired] Expired { $time }
}
approvals-reason = Reason: { $reason }
approvals-no-preview = This change works with files outside the workspace, so it has no preview.
approvals-no-changes = The preview found nothing to change.
approvals-new-item = New: { $entity }
approvals-field = { $field }: { $before } → { $after }
approvals-field-new = { $field }: { $after }
approvals-conflicts = { $count ->
    [one] One field changed since the agent proposed this. Approving overwrites it.
   *[other] { $count } fields changed since the agent proposed this. Approving overwrites them.
}
approvals-conflict = { $field }: { $then } when proposed, { $now } now
approvals-changes = What it changes
approvals-reject-title = Reject the change
approvals-reject-reason = Reason, for the agent to read (optional)
approvals-reject-confirm = Reject
approvals-approved-toast = { $count ->
    [one] The change was approved and made.
   *[other] { $count } changes were approved and made.
}
approvals-not-approved = { $count ->
    [one] One change wasn't made.
   *[other] { $count } changes weren't made.
}
approvals-rejected-toast = { $count ->
    [one] The change was rejected.
   *[other] { $count } changes were rejected.
}

## What a proposed change does, by operation; others show the engine's summary

approvals-op-task-create = Create a task
approvals-op-task-update = Change a task
approvals-op-task-move = Move a task
approvals-op-task-delete = Move tasks to the trash
approvals-op-task-bulk-update = Change several tasks
approvals-op-task-duplicate = Duplicate a task
approvals-op-task-claim = Claim a task
approvals-op-task-release = Release a task
approvals-op-checklist-update = Change a checklist
approvals-op-relation-add = Link tasks
approvals-op-relation-remove = Unlink tasks
approvals-op-comment-add = Add a comment
approvals-op-comment-update = Edit a comment
approvals-op-comment-delete = Delete a comment
approvals-op-time-log = Log time
approvals-op-project-create = Create a project
approvals-op-project-update = Change a project
approvals-op-project-delete = Delete a project
approvals-op-wiki-create = Create a wiki page
approvals-op-wiki-update = Change a wiki page
approvals-op-wiki-delete = Move a wiki page to the trash
approvals-op-file-attach = Attach a file
approvals-op-file-delete = Delete a file
approvals-op-trash-purge = Delete from the trash for good
approvals-op-history-revert = Revert changes

## History

history-approved-by-you = You approved this change the agent proposed.
activity-entity-proposal = Proposed change
activity-proposal-pending = Change proposed
activity-proposal-approved = Proposal approved
activity-proposal-rejected = Proposal rejected

## Preferences

agents-approvals = Changes need approval
agents-approvals-about = The changes of agents with this profile wait in Approvals until you approve or reject them. Deletions are moves to the trash and deleting for good. Read-only agents may then propose what a standard agent may do.
agents-approvals-read-only = Read-only agents
agents-approvals-standard = Standard agents
agents-approvals-full = Full agents
agents-approvals-off = Off
agents-approvals-destructive = Deletions only
agents-approvals-all-writes = All changes
agents-approvals-days = Proposals expire after
agents-approvals-days-about = A proposal nobody decides on expires and stays in the agent session's record.
agents-approvals-days-unit = { $count ->
    [one] day
   *[other] days
}
