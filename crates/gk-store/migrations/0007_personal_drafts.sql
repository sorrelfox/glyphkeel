-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 7: drafts belong to a person. Each person keeps one draft
-- per editor, apart from the workspace's data: drafts are not history and
-- are not exported. Old drafts are found by `updated_at` and expire.
--
-- Version 6 had one draft per editor for everyone; such drafts become the
-- person's ('user', 'You'), the name the app writes for its user.

CREATE TABLE drafts_v7 (
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name TEXT NOT NULL,
  owner_kind TEXT NOT NULL CHECK (owner_kind IN
               ('task', 'comment', 'new_comment', 'project', 'wiki_page', 'new_wiki_page')),
  owner_id   BLOB NOT NULL CHECK (length(owner_id) = 16),
  title      TEXT,
  body       TEXT NOT NULL,
  base_rev   BLOB,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (actor_kind, actor_name, owner_kind, owner_id)
) STRICT;

INSERT INTO drafts_v7 (actor_kind, actor_name, owner_kind, owner_id, title, body, base_rev,
                       updated_at)
SELECT 'user', 'You', owner_kind, owner_id, title, body, base_rev, updated_at
  FROM drafts
 WHERE length(owner_id) = 16;

DROP TABLE drafts;
ALTER TABLE drafts_v7 RENAME TO drafts;
CREATE INDEX drafts_owner ON drafts (owner_kind, owner_id);
CREATE INDEX drafts_updated ON drafts (updated_at);
