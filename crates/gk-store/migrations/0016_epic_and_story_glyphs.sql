-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 16: two more task type shapes, epic and story, for the
-- built-in "Software project" template and for anyone who wants them.
--
-- SQLite can't change a CHECK constraint in place, so `task_types` is made
-- again with the wider list and its rows copied back. Tasks point at their
-- type: while the old table is gone they would break that foreign key, so
-- the check waits for the end of the transaction (`defer_foreign_keys`),
-- and copying the rows into the new `task_types` puts every reference right
-- again before then.

PRAGMA defer_foreign_keys = ON;

CREATE TABLE task_types_v16 AS SELECT * FROM task_types;

DROP TABLE task_types;

CREATE TABLE task_types (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB NOT NULL REFERENCES projects (id),
  name            TEXT NOT NULL CHECK (name <> ''),
  name_fold       TEXT NOT NULL,
  glyph           TEXT NOT NULL
                  CHECK (glyph IN ('task', 'bug', 'feature', 'milestone', 'epic', 'story')),
  hue             TEXT NOT NULL,
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  is_default      INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
  due_offset_days INTEGER CHECK (due_offset_days IS NULL OR due_offset_days BETWEEN 0 AND 3650),
  UNIQUE (project_id, name_fold)
) STRICT;

INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default,
                        due_offset_days)
SELECT id, project_id, name, name_fold, glyph, hue, rank, is_default, due_offset_days
  FROM task_types_v16;

DROP TABLE task_types_v16;

-- At most one default type per project.
CREATE UNIQUE INDEX task_types_one_default ON task_types (project_id) WHERE is_default = 1;
