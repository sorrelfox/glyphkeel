-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 22: project folders. A folder is one level of grouping
-- for top-level projects in the sidebar: a name (unique, ignoring case),
-- an optional colour and a place among the folders. Folders never hold
-- folders. A top-level project names the folder it is in, if any; its
-- sub-projects follow it and name none. Projects keep their own order
-- (`rank` among the top-level projects) inside and outside folders.
--
-- A folder is history's `record` field like a dashboard, and a project's
-- folder is a project field, so every change can be undone. Folders
-- belong to the workspace; they could take an owner later, should a
-- workspace ever have several people.

CREATE TABLE project_folders (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL UNIQUE,
  hue             TEXT,
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;

ALTER TABLE projects ADD COLUMN folder_id BLOB REFERENCES project_folders (id);
CREATE INDEX projects_folder ON projects (folder_id) WHERE folder_id IS NOT NULL;
