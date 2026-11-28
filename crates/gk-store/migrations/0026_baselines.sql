-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schedule baselines: a named snapshot of a project's plan. A baseline
-- belongs to one project and has a name unique in it (ignoring case). Its
-- tasks are the project's dated tasks when it was saved: each one's start
-- date, due date and whether it was a milestone, with its key and title
-- then, so that a task deleted or moved later still shows what it was.
-- The task's ID is kept without a reference, for the same reason.
--
-- A baseline is history's `record` field (its name and who saved it) and
-- its `tasks` field (the snapshot), so saving, renaming and deleting one
-- can be undone. The saver is an actor by kind and name, as everywhere.

CREATE TABLE baselines (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL,
  UNIQUE (project_id, name_fold)
) STRICT;

CREATE INDEX baselines_newest ON baselines (project_id, created_at);

CREATE TABLE baseline_tasks (
  baseline_id     BLOB NOT NULL REFERENCES baselines (id) ON DELETE CASCADE,
  task_id         BLOB NOT NULL CHECK (length(task_id) = 16),
  task_key        TEXT NOT NULL,
  title           TEXT NOT NULL,
  start_date      TEXT,
  due_date        TEXT,
  milestone       INTEGER NOT NULL DEFAULT 0 CHECK (milestone IN (0, 1)),
  PRIMARY KEY (baseline_id, task_id),
  CHECK (start_date IS NOT NULL OR due_date IS NOT NULL)
) STRICT, WITHOUT ROWID;

CREATE INDEX baseline_tasks_task ON baseline_tasks (task_id);
