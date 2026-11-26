-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 8: the links that texts make. Wiki pages, task
-- descriptions and comments link to pages (`[[Page]]`, `[[Page#Heading]]`,
-- `[[KEY:Page]]`) and to tasks (`TM-41`). Each link is kept as written, one
-- row per link, so "what links here" is a lookup instead of a scan of every
-- text. A link is resolved when it is read, by title, slug, old slug or ID,
-- so renaming, moving, deleting and restoring a page or task needs no
-- change here. `[[Page]]` looks in the project the source is in when the
-- link is read.
--
-- The rows are derived from the texts, like the search index: the engine
-- rewrites a source's rows whenever its text changes. Rows of a source in
-- the trash stay, so its links are back as soon as it is.

CREATE TABLE text_links (
  source_kind TEXT NOT NULL CHECK (source_kind IN ('wiki', 'task', 'comment')),
  source_id   BLOB NOT NULL CHECK (length(source_id) = 16),
  -- The order of first mention in the text, from 0.
  position    INTEGER NOT NULL CHECK (position >= 0),
  target_kind TEXT NOT NULL CHECK (target_kind IN ('wiki', 'task')),
  -- The page name or task key as written.
  target      TEXT NOT NULL CHECK (target <> ''),
  -- The heading of a page link, if it names one.
  heading     TEXT,
  -- Page links: the name folded for comparison and as a slug; for
  -- `KEY:Page`, the same of `Page`. Empty for task links.
  name_fold   TEXT NOT NULL DEFAULT '',
  name_slug   TEXT NOT NULL DEFAULT '',
  tail_fold   TEXT NOT NULL DEFAULT '',
  tail_slug   TEXT NOT NULL DEFAULT '',
  -- Task links: the number of the key.
  task_number INTEGER,
  PRIMARY KEY (source_kind, source_id, position)
) STRICT;
CREATE INDEX text_links_name_fold ON text_links (name_fold) WHERE target_kind = 'wiki';
CREATE INDEX text_links_name_slug ON text_links (name_slug) WHERE target_kind = 'wiki';
CREATE INDEX text_links_tail_fold ON text_links (tail_fold) WHERE tail_fold <> '';
CREATE INDEX text_links_tail_slug ON text_links (tail_slug) WHERE tail_slug <> '';
CREATE INDEX text_links_task      ON text_links (task_number) WHERE target_kind = 'task';

-- The texts written before this version are read once, when the engine
-- next opens the workspace; it removes this mark when it is done.
INSERT INTO meta (key, value) VALUES ('text_links_pending', '1')
  ON CONFLICT (key) DO UPDATE SET value = excluded.value;
