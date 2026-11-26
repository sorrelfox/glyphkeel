-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 14: recurring tasks, task and project templates, and a
-- default due date per task type.
--
-- A recurring task belongs to a series. The series keeps the rule, the
-- details every new occurrence starts from (`pattern`, the same shape as a
-- task template) and the occurrence that is current. Each occurrence is an
-- ordinary task that names its series; closed occurrences keep the name, so
-- the series' history stays visible. `current_task` and `tasks.series_id`
-- have no foreign keys: purging a task from the trash or ending a series
-- leaves the other side to read as "none".
--
-- Templates hold their content as JSON that names statuses, types and labels
-- by name, never by ID, so a template outlives the items it was made from
-- and can be used in another project. Task templates belong to a project;
-- project templates belong to the workspace (no project). Both record who
-- made them, as every other item does.

ALTER TABLE task_types ADD COLUMN due_offset_days INTEGER
    CHECK (due_offset_days IS NULL OR due_offset_days BETWEEN 0 AND 3650);

CREATE TABLE task_series (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB NOT NULL REFERENCES projects (id),
  rule            TEXT NOT NULL CHECK (json_valid(rule)),
  pattern         TEXT NOT NULL CHECK (json_valid(pattern)),
  current_task    BLOB CHECK (current_task IS NULL OR length(current_task) = 16),
  next_date       TEXT CHECK (next_date IS NULL
                              OR next_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  made            INTEGER NOT NULL DEFAULT 1 CHECK (made >= 0),
  ended_at        INTEGER,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX task_series_project ON task_series (project_id);
CREATE INDEX task_series_due ON task_series (next_date)
    WHERE ended_at IS NULL AND next_date IS NOT NULL;

ALTER TABLE tasks ADD COLUMN series_id BLOB
    CHECK (series_id IS NULL OR length(series_id) = 16);
CREATE INDEX tasks_series ON tasks (series_id) WHERE series_id IS NOT NULL;

CREATE TABLE templates (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB REFERENCES projects (id),
  kind            TEXT NOT NULL CHECK (kind IN ('task', 'project')),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 1000),
  content         TEXT NOT NULL CHECK (json_valid(content)
                                       AND octet_length(content) <= 4194304),
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL,
  CHECK ((kind = 'task') = (project_id IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX templates_name ON templates (kind, coalesce(project_id, x''), name_fold);
