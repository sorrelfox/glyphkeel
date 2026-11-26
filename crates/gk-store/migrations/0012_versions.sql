-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 12: versions (releases) of a project, and the version a
-- task is planned for (its fix version).
--
-- A version is open, locked (no new tasks, the ones in it stay) or closed.
-- Its sharing says who can plan tasks for it: only its own project
-- ('none'), or its sub-projects at any depth too ('descendants').
-- Names are unique per project, ignoring case. The dates are local days.
-- A task's fix version is a version of its own project or one shared with
-- it by an ancestor; it is cleared when that stops being true. The engine
-- keeps the reference sound (a version is deleted only once no task names
-- it), so the column carries no foreign key and an older schema can still
-- drop it.

CREATE TABLE versions (
  id          BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id  BLOB NOT NULL REFERENCES projects (id),
  name        TEXT NOT NULL CHECK (name <> '' AND length(name) <= 100),
  name_fold   TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4000),
  status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'locked', 'closed')),
  start_date  TEXT CHECK (start_date IS NULL
                          OR start_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  due_date    TEXT CHECK (due_date IS NULL
                          OR due_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  sharing     TEXT NOT NULL DEFAULT 'none' CHECK (sharing IN ('none', 'descendants')),
  rank        TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  UNIQUE (project_id, name_fold),
  CHECK (start_date IS NULL OR due_date IS NULL OR start_date <= due_date)
) STRICT;

ALTER TABLE tasks ADD COLUMN fix_version_id BLOB CHECK (fix_version_id IS NULL
                                                     OR length(fix_version_id) = 16);
CREATE INDEX tasks_fix_version ON tasks (fix_version_id) WHERE fix_version_id IS NOT NULL;
