-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 15: a minimum next to the WIP limit of a board column, and
-- the kinds of work that logged time can be marked with.
--
-- The lag of a dependency needs no change here: `task_relations.lag_days`
-- has been in the schema from the start (only `blocks` links take one); the
-- engine keeps it between 0 and 365 days.
--
-- Kinds of work (Development, Design, Meeting, ...) belong to the workspace,
-- not to a project, so a report can group the time of every project by them.
-- An entry and the timer may name one. A kind that entries still use is
-- archived rather than deleted: it leaves the pickers and keeps its name in
-- reports. Like `tasks.fix_version_id`, the references have no foreign key,
-- and the engine keeps them sound.
--
-- History conditions in queries (`was:`, `changed:`, `changed-by:`) read the
-- activity table through its index on `(entity_kind, entity_id, seq)`; no
-- new index is needed for them.

ALTER TABLE statuses ADD COLUMN wip_min INTEGER CHECK (wip_min IS NULL OR wip_min > 0);

CREATE TABLE work_kinds (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
  name_fold       TEXT NOT NULL UNIQUE,
  archived_at     INTEGER,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;

ALTER TABLE time_entries ADD COLUMN work_kind_id BLOB
    CHECK (work_kind_id IS NULL OR length(work_kind_id) = 16);
CREATE INDEX time_entries_work_kind ON time_entries (work_kind_id)
    WHERE work_kind_id IS NOT NULL;

ALTER TABLE timer ADD COLUMN work_kind_id BLOB
    CHECK (work_kind_id IS NULL OR length(work_kind_id) = 16);
