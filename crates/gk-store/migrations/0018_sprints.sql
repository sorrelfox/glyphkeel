-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 18: sprints of a project, short time-boxed cycles of work,
-- and the sprint a task is in.
--
-- A sprint is planned, active or closed; a project has at most one active
-- sprint at a time. Names are unique per project, ignoring case. The dates
-- are local days; the moments it was started and completed are kept apart
-- from them, for the burndown. A task is in at most one sprint, of its own
-- project. The engine keeps the reference sound (a sprint is deleted only
-- once no task names it), so the column carries no foreign key and an
-- older schema can still drop it.

CREATE TABLE sprints (
  id           BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id   BLOB NOT NULL REFERENCES projects (id),
  name         TEXT NOT NULL CHECK (name <> '' AND length(name) <= 100),
  name_fold    TEXT NOT NULL,
  goal         TEXT NOT NULL DEFAULT '' CHECK (length(goal) <= 4000),
  state        TEXT NOT NULL DEFAULT 'planned' CHECK (state IN ('planned', 'active', 'closed')),
  start_date   TEXT CHECK (start_date IS NULL
                           OR start_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  end_date     TEXT CHECK (end_date IS NULL
                           OR end_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  started_at   INTEGER,
  completed_at INTEGER,
  rank         TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  UNIQUE (project_id, name_fold),
  CHECK (start_date IS NULL OR end_date IS NULL OR start_date <= end_date)
) STRICT;

CREATE UNIQUE INDEX sprints_one_active ON sprints (project_id) WHERE state = 'active';

ALTER TABLE tasks ADD COLUMN sprint_id BLOB CHECK (sprint_id IS NULL OR length(sprint_id) = 16);
CREATE INDEX tasks_sprint ON tasks (sprint_id) WHERE sprint_id IS NOT NULL;
