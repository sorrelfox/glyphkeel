-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 2: answers to repeated requests, and wiki page names that
-- are unique ignoring case.

-- The first answer to a command sent with a `client_request_id`, so a
-- repeat from any process within 24 hours gets the same answer instead of
-- running again. Rows older than that are purged as new ones are written.
CREATE TABLE request_log (
  op         TEXT NOT NULL,
  caller     TEXT NOT NULL,
  request_id TEXT NOT NULL CHECK (request_id <> ''),
  output     TEXT NOT NULL CHECK (json_valid(output)),
  at         INTEGER NOT NULL,
  PRIMARY KEY (op, caller, request_id)
) STRICT;
CREATE INDEX request_log_at ON request_log (at);

-- A wiki link names a page by its title ignoring case, so two live pages of
-- one project can't have titles that differ only in case. Slugs are made
-- from titles in lower case, so they follow.
CREATE UNIQUE INDEX wiki_pages_name ON wiki_pages (project_id, title_fold)
  WHERE deleted_at IS NULL;
