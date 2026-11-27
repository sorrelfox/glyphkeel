-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 20: dashboards. A dashboard is a named page of widgets,
-- for the whole workspace (`project_id` NULL) or pinned to one project.
-- Its widgets are kept in order as one JSON list, each with its ID, kind,
-- title, width and settings; the engine checks them. Names are unique per
-- project (or among the workspace's dashboards), ignoring case.
--
-- A dashboard is history's `record` field like a saved view, so making,
-- changing and deleting one can be undone. It belongs to the workspace; it
-- could take an owner later, should a workspace ever have several people.

CREATE TABLE dashboards (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB REFERENCES projects (id),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 1000),
  widgets         TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(widgets)
                                                    AND json_type(widgets) = 'array'
                                                    AND octet_length(widgets) <= 1048576),
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE UNIQUE INDEX dashboards_name ON dashboards (coalesce(project_id, x''), name_fold);
