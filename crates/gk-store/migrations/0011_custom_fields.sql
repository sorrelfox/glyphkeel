-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 11: custom fields. A field belongs to one project, or to
-- every project when it has none. Its kind decides its values; select and
-- multi-select fields list their options in order. A field can be limited
-- to some task types, by type name (an empty list means every type), and
-- can be required. A new sub-project gets copies of its parent's own
-- fields, as it gets its statuses.
--
-- Values are kept one row per value, so a multi-select field has a row per
-- chosen option. A value is text (text, Markdown, dates as YYYY-MM-DD,
-- options and links), a whole number (whole numbers, and yes/no as 1 or
-- 0) or a real number (decimals), so values compare and sort by their
-- kind. Deleting a task or a field deletes its values.

CREATE TABLE custom_fields (
  id            BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id    BLOB REFERENCES projects (id) ON DELETE CASCADE,
  name          TEXT NOT NULL CHECK (name <> ''),
  name_fold     TEXT NOT NULL,
  -- The name queries use after `cf.`: folded, words joined by hyphens.
  query_name    TEXT NOT NULL CHECK (query_name <> ''),
  kind          TEXT NOT NULL CHECK (kind IN ('text', 'markdown', 'integer', 'decimal', 'date',
                                              'boolean', 'select', 'multi_select', 'url')),
  options       TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(options) AND json_type(options) = 'array'),
  default_value TEXT CHECK (default_value IS NULL OR json_valid(default_value)),
  type_names    TEXT NOT NULL DEFAULT '[]'
                CHECK (json_valid(type_names) AND json_type(type_names) = 'array'),
  required      INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
  rank          TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at    INTEGER NOT NULL
) STRICT;
-- One name per project, and one among the fields of every project.
CREATE UNIQUE INDEX custom_fields_name ON custom_fields (ifnull(project_id, x''), name_fold);
CREATE INDEX custom_fields_project ON custom_fields (project_id);
CREATE INDEX custom_fields_query_name ON custom_fields (query_name);

CREATE TABLE custom_values (
  task_id  BLOB NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  field_id BLOB NOT NULL REFERENCES custom_fields (id) ON DELETE CASCADE,
  value    ANY NOT NULL CHECK (typeof(value) IN ('text', 'integer', 'real')),
  PRIMARY KEY (task_id, field_id, value)
) STRICT, WITHOUT ROWID;
CREATE INDEX custom_values_field ON custom_values (field_id, value);
