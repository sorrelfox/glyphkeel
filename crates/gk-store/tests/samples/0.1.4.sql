-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.4.
-- release: 0.1.4
-- schema: 10
-- Made by `cargo xtask migration-samples`. Never edit or regenerate it:
-- it stands for the workspaces this release left on people's disks.
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;
CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL CHECK (json_valid(value)),
  updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE agent_sessions (
  id             BLOB PRIMARY KEY CHECK (length(id) = 16),
  client_id      TEXT,
  client_version TEXT,
  display_name   TEXT NOT NULL,
  profile        TEXT NOT NULL CHECK (profile IN ('read-only', 'standard', 'full')),
  scope_project  BLOB,
  pid            INTEGER,
  started_at     INTEGER NOT NULL,
  last_seen_at   INTEGER NOT NULL,
  ended_at       INTEGER,
  reads          INTEGER NOT NULL DEFAULT 0,
  writes         INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE projects (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  parent_id       BLOB REFERENCES projects (id),
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  key             TEXT NOT NULL UNIQUE
                  CHECK (length(key) BETWEEN 2 AND 10 AND key GLOB '[A-Z]*'
                         AND key NOT GLOB '*[^A-Z0-9]*'),
  name            TEXT NOT NULL CHECK (name <> ''),
  name_fold       TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '' CHECK (octet_length(description) <= 1048576),
  description_rev BLOB,
  hue             TEXT NOT NULL,
  icon            TEXT,
  state           TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'archived')),
  modules         TEXT NOT NULL CHECK (json_valid(modules)),
  next_number     INTEGER NOT NULL DEFAULT 1 CHECK (next_number >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  deleted_at      INTEGER,
  deleted_group   BLOB
) STRICT;
CREATE TABLE project_key_aliases (
  key        TEXT PRIMARY KEY,
  project_id BLOB NOT NULL REFERENCES projects (id)
) STRICT;
CREATE TABLE statuses (
  id         BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id BLOB NOT NULL REFERENCES projects (id),
  name       TEXT NOT NULL CHECK (name <> ''),
  name_fold  TEXT NOT NULL,
  category   TEXT NOT NULL CHECK (category IN ('todo', 'in_progress', 'done', 'cancelled')),
  hue        TEXT NOT NULL,
  rank       TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  wip_limit  INTEGER CHECK (wip_limit IS NULL OR wip_limit > 0),
  UNIQUE (project_id, name_fold)
) STRICT;
CREATE TABLE task_types (
  id         BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id BLOB NOT NULL REFERENCES projects (id),
  name       TEXT NOT NULL CHECK (name <> ''),
  name_fold  TEXT NOT NULL,
  glyph      TEXT NOT NULL CHECK (glyph IN ('task', 'bug', 'feature', 'milestone')),
  hue        TEXT NOT NULL,
  rank       TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
  UNIQUE (project_id, name_fold)
) STRICT;
CREATE TABLE labels (
  id         BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id BLOB NOT NULL REFERENCES projects (id),
  name       TEXT NOT NULL CHECK (name <> ''),
  name_fold  TEXT NOT NULL,
  hue        TEXT NOT NULL,
  rank       TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  UNIQUE (project_id, name_fold)
) STRICT;
CREATE TABLE tasks (
  id                BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id        BLOB NOT NULL REFERENCES projects (id),
  number            INTEGER NOT NULL CHECK (number >= 1),
  title             TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 500),
  title_fold        TEXT NOT NULL,
  description       TEXT NOT NULL DEFAULT '' CHECK (octet_length(description) <= 1048576),
  description_rev   BLOB,
  status_id         BLOB NOT NULL REFERENCES statuses (id),
  status_entered_at INTEGER NOT NULL,
  type_id           BLOB NOT NULL REFERENCES task_types (id),
  priority          INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 4),
  parent_id         BLOB REFERENCES tasks (id),
  rank              TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  start_date        TEXT CHECK (start_date IS NULL
                                OR start_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  due_date          TEXT CHECK (due_date IS NULL
                                OR due_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  estimate_min      INTEGER CHECK (estimate_min IS NULL OR estimate_min >= 0),
  progress_override INTEGER CHECK (progress_override IS NULL
                                   OR progress_override BETWEEN 0 AND 100),
  checklist_done    INTEGER NOT NULL DEFAULT 0,
  checklist_total   INTEGER NOT NULL DEFAULT 0,
  spent_min         INTEGER NOT NULL DEFAULT 0 CHECK (spent_min >= 0),
  claim_session     BLOB,
  claim_name        TEXT,
  claim_until       INTEGER,
  created_at        INTEGER NOT NULL,
  created_by_kind   TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name   TEXT NOT NULL,
  edited_at         INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  closed_at         INTEGER,
  deleted_at        INTEGER,
  deleted_group     BLOB,
  version           INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1), subtask_rank TEXT
  CHECK (subtask_rank IS NULL OR (subtask_rank <> '' AND subtask_rank NOT GLOB '*[^0-9a-z]*')),
  UNIQUE (project_id, number),
  CHECK (start_date IS NULL OR due_date IS NULL OR start_date <= due_date),
  CHECK (checklist_done BETWEEN 0 AND checklist_total),
  CHECK ((claim_session IS NULL) = (claim_until IS NULL)),
  CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;
CREATE TABLE task_key_aliases (
  project_key TEXT NOT NULL,
  number      INTEGER NOT NULL,
  task_id     BLOB NOT NULL REFERENCES tasks (id),
  PRIMARY KEY (project_key, number)
) STRICT;
CREATE TABLE task_labels (
  task_id  BLOB NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  label_id BLOB NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, label_id)
) STRICT;
CREATE TABLE task_relations (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  from_task       BLOB NOT NULL REFERENCES tasks (id),
  to_task         BLOB NOT NULL REFERENCES tasks (id),
  kind            TEXT NOT NULL CHECK (kind IN ('blocks', 'relates', 'duplicates', 'clones')),
  lag_days        INTEGER,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  UNIQUE (from_task, to_task, kind),
  CHECK (from_task <> to_task),
  CHECK (kind <> 'relates' OR from_task < to_task),
  CHECK (lag_days IS NULL OR kind = 'blocks')
) STRICT;
CREATE TABLE comments (
  id            BLOB PRIMARY KEY CHECK (length(id) = 16),
  task_id       BLOB NOT NULL REFERENCES tasks (id),
  body          TEXT NOT NULL CHECK (octet_length(body) <= 262144),
  reply_to      BLOB REFERENCES comments (id),
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name    TEXT NOT NULL,
  session_id    BLOB,
  via           TEXT NOT NULL CHECK (via IN ('app', 'cli', 'mcp')),
  created_at    INTEGER NOT NULL,
  edited_at     INTEGER,
  deleted_at    INTEGER,
  deleted_group BLOB,
  version       INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1)
) STRICT;
CREATE TABLE comment_revisions (
  id         BLOB PRIMARY KEY CHECK (length(id) = 16),
  comment_id BLOB NOT NULL REFERENCES comments (id),
  body       TEXT NOT NULL,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE text_revisions (
  id          BLOB PRIMARY KEY CHECK (length(id) = 16),
  owner_kind  TEXT NOT NULL CHECK (owner_kind IN ('task', 'project')),
  owner_id    BLOB NOT NULL,
  body        TEXT NOT NULL,
  base_rev    BLOB,
  kind        TEXT NOT NULL CHECK (kind IN ('normal', 'merged', 'conflict', 'restored', 'resolved')),
  message     TEXT,
  actor_kind  TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name  TEXT NOT NULL,
  session_id  BLOB,
  created_at  INTEGER NOT NULL,
  resolved_at INTEGER
) STRICT;
CREATE TABLE time_entries (
  id            BLOB PRIMARY KEY CHECK (length(id) = 16),
  task_id       BLOB NOT NULL REFERENCES tasks (id),
  work_date     TEXT NOT NULL CHECK (work_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  minutes       INTEGER NOT NULL CHECK (minutes > 0),
  note          TEXT NOT NULL DEFAULT '',
  activity      TEXT,
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name    TEXT NOT NULL,
  session_id    BLOB,
  created_at    INTEGER NOT NULL,
  edited_at     INTEGER,
  deleted_at    INTEGER,
  deleted_group BLOB,
  version       INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1)
) STRICT;
CREATE TABLE timer (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  task_id    BLOB NOT NULL REFERENCES tasks (id),
  started_at INTEGER NOT NULL,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name TEXT NOT NULL
, running_since INTEGER, elapsed_ms INTEGER NOT NULL DEFAULT 0 CHECK (elapsed_ms >= 0), note TEXT NOT NULL DEFAULT '', reminded_at INTEGER) STRICT;
CREATE TABLE reminder_log (
  task_id  BLOB NOT NULL,
  due_date TEXT NOT NULL,
  fired_at INTEGER NOT NULL,
  PRIMARY KEY (task_id, due_date)
) STRICT;
CREATE TABLE blobs (
  sha256     TEXT PRIMARY KEY CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
  size       INTEGER NOT NULL CHECK (size >= 0),
  created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE attachments (
  id            BLOB PRIMARY KEY CHECK (length(id) = 16),
  owner_kind    TEXT NOT NULL CHECK (owner_kind IN ('task', 'comment', 'wiki_page', 'project')),
  owner_id      BLOB NOT NULL,
  file_name     TEXT NOT NULL CHECK (file_name <> ''),
  mime          TEXT NOT NULL,
  size          INTEGER NOT NULL CHECK (size >= 0),
  sha256        TEXT NOT NULL REFERENCES blobs (sha256),
  width         INTEGER,
  height        INTEGER,
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name    TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  deleted_at    INTEGER,
  deleted_group BLOB
) STRICT;
CREATE TABLE saved_views (
  id          BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id  BLOB REFERENCES projects (id),
  view_kind   TEXT NOT NULL CHECK (view_kind IN ('list', 'board', 'gantt', 'calendar')),
  name        TEXT NOT NULL CHECK (name <> ''),
  description TEXT NOT NULL DEFAULT '',
  query       TEXT NOT NULL,
  columns     TEXT CHECK (columns IS NULL OR json_valid(columns)),
  sort        TEXT CHECK (sort IS NULL OR json_valid(sort)),
  group_by    TEXT CHECK (group_by IS NULL OR json_valid(group_by)),
  totals      TEXT CHECK (totals IS NULL OR json_valid(totals)),
  options     TEXT CHECK (options IS NULL OR json_valid(options)),
  is_default  INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
  rank        TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
) STRICT;
CREATE TABLE view_settings (
  scope      TEXT NOT NULL,
  view_kind  TEXT NOT NULL,
  settings   TEXT NOT NULL CHECK (json_valid(settings)),
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (scope, view_kind)
) STRICT;
CREATE TABLE activity (
  seq         INTEGER PRIMARY KEY AUTOINCREMENT,
  group_id    BLOB NOT NULL,
  at          INTEGER NOT NULL,
  op          TEXT NOT NULL,
  actor_kind  TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name  TEXT NOT NULL,
  via         TEXT NOT NULL CHECK (via IN ('app', 'cli', 'mcp')),
  session_id  BLOB,
  project_id  BLOB,
  entity_kind TEXT NOT NULL,
  entity_id   BLOB NOT NULL,
  changes     TEXT NOT NULL CHECK (json_valid(changes)),
  inverse     TEXT CHECK (inverse IS NULL OR json_valid(inverse)),
  reverted_by BLOB
) STRICT;
CREATE TABLE search_docs (
  docid      INTEGER PRIMARY KEY,
  kind       TEXT NOT NULL CHECK (kind IN ('task', 'comment', 'wiki', 'file')),
  entity_id  BLOB NOT NULL,
  task_id    BLOB,
  project_id BLOB NOT NULL,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (kind, entity_id)
) STRICT;
CREATE VIRTUAL TABLE fts_words USING fts5 (
  title, body, content = 'search_docs', content_rowid = 'docid',
  tokenize = 'porter unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE fts_grams USING fts5 (
  title, body, content = 'search_docs', content_rowid = 'docid',
  tokenize = 'trigram remove_diacritics 1'
);
CREATE TABLE wiki_pages (
  id             BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id     BLOB NOT NULL REFERENCES projects (id),
  parent_id      BLOB REFERENCES wiki_pages (id),
  rank           TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  title          TEXT NOT NULL CHECK (title <> ''),
  title_fold     TEXT NOT NULL,
  slug           TEXT NOT NULL CHECK (slug <> ''),
  current_rev    BLOB,
  is_agent_brief INTEGER NOT NULL DEFAULT 0 CHECK (is_agent_brief IN (0, 1)),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  deleted_at     INTEGER,
  deleted_group  BLOB, is_home INTEGER NOT NULL DEFAULT 0 CHECK (is_home IN (0, 1)), is_template INTEGER NOT NULL DEFAULT 0
    CHECK (is_template IN (0, 1)),
  UNIQUE (project_id, slug)
) STRICT;
CREATE TABLE wiki_revisions (
  id         BLOB PRIMARY KEY CHECK (length(id) = 16),
  page_id    BLOB NOT NULL REFERENCES wiki_pages (id),
  body       TEXT NOT NULL CHECK (octet_length(body) <= 1048576),
  base_rev   BLOB,
  kind       TEXT NOT NULL CHECK (kind IN ('normal', 'merged', 'conflict', 'restored', 'resolved')),
  message    TEXT,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name TEXT NOT NULL,
  session_id BLOB,
  created_at INTEGER NOT NULL
, resolved_at INTEGER) STRICT;
CREATE TABLE wiki_redirects (
  project_id BLOB NOT NULL,
  slug       TEXT NOT NULL,
  page_id    BLOB NOT NULL REFERENCES wiki_pages (id),
  PRIMARY KEY (project_id, slug)
) STRICT;
CREATE TABLE request_log (
  op         TEXT NOT NULL,
  caller     TEXT NOT NULL,
  request_id TEXT NOT NULL CHECK (request_id <> ''),
  output     TEXT NOT NULL CHECK (json_valid(output)),
  at         INTEGER NOT NULL,
  PRIMARY KEY (op, caller, request_id)
) STRICT;
CREATE TABLE "reminders" (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  task_id         BLOB NOT NULL UNIQUE REFERENCES tasks (id),
  remind_at       INTEGER,
  days_before_due INTEGER CHECK (days_before_due IS NULL OR days_before_due BETWEEN 0 AND 365),
  time_of_day     INTEGER CHECK (time_of_day IS NULL OR time_of_day BETWEEN 0 AND 1439),
  note            TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 500),
  actor_kind      TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name      TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  acked_at        INTEGER,
  acked_moment    INTEGER,
  CHECK ((remind_at IS NULL) <> (days_before_due IS NULL)),
  CHECK (time_of_day IS NULL OR days_before_due IS NOT NULL),
  CHECK ((acked_at IS NULL) = (acked_moment IS NULL))
) STRICT;
CREATE TABLE "drafts" (
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
INSERT INTO "meta" VALUES('workspace_id','01a21bc4-a866-795f-bee9-b901edb800d9');
INSERT INTO "meta" VALUES('created_at','1795762202726');
INSERT INTO "meta" VALUES('created_by_version','0.1.4');
INSERT INTO "projects" VALUES(x'01a21bc4a88f750497deca96c920e3d6',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21bc4a88f7a7fa1cdeb3fa634658a','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',4,1795762202767,1795762202874,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21bc4a8b176c49f58a697917e6fef',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795762202801,1795762202891,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a88f7defacbb891a35b4303a',x'01a21bc4a88f750497deca96c920e3d6','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a88f7609a1cefec428ff4f4e',x'01a21bc4a88f750497deca96c920e3d6','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a88f7c1780a754201229af48',x'01a21bc4a88f750497deca96c920e3d6','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a88f7498acff3ba6853ec985',x'01a21bc4a88f750497deca96c920e3d6','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a88f7c23b8c2d78cdcfdd92e',x'01a21bc4a88f750497deca96c920e3d6','Done','done','done','green','u',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a8b17902a4632de85cf56c70',x'01a21bc4a8b176c49f58a697917e6fef','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a8b171f28a02bd5245b61264',x'01a21bc4a8b176c49f58a697917e6fef','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a8b17e4d9989de28ef13a3d2',x'01a21bc4a8b176c49f58a697917e6fef','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a8b174a4ad9b3b46076bf4b1',x'01a21bc4a8b176c49f58a697917e6fef','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a8b17b6dbcf1614a891e178b',x'01a21bc4a8b176c49f58a697917e6fef','Done','done','done','green','u',NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4a88f75d99a3e995c602f739c',x'01a21bc4a88f750497deca96c920e3d6','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a88f70b197f348a7789995b2',x'01a21bc4a88f750497deca96c920e3d6','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a88f7208aa7eeebb524f499a',x'01a21bc4a88f750497deca96c920e3d6','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a88f7a02bde5e76c8dd79afd',x'01a21bc4a88f750497deca96c920e3d6','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a8b1743594ba29bbc80d751e',x'01a21bc4a8b176c49f58a697917e6fef','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a8b17454b7bf92d797dae8d0',x'01a21bc4a8b176c49f58a697917e6fef','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a8b173efa9760df5f32d1af9',x'01a21bc4a8b176c49f58a697917e6fef','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a8b1735eadd89a0bfd58e3db',x'01a21bc4a8b176c49f58a697917e6fef','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "labels" VALUES(x'01a21bc4a8c17b7d93e5c9e203047c64',x'01a21bc4a88f750497deca96c920e3d6','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21bc4a8d575ea8f3813c3634c508e',x'01a21bc4a88f750497deca96c920e3d6',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a9327a9da61959f921a50bab',x'01a21bc4a88f7defacbb891a35b4303a',1795762202837,x'01a21bc4a88f75d99a3e995c602f739c',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,90,NULL,NULL,NULL,1795762202837,'user','You',1795762202930,1795762202965,NULL,NULL,NULL,2,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a8e97f09be96e5a2b4e4d0a7',x'01a21bc4a88f750497deca96c920e3d6',2,'Buy seeds','buy seeds','',NULL,x'01a21bc4a88f7defacbb891a35b4303a',1795762202857,x'01a21bc4a88f75d99a3e995c602f739c',0,x'01a21bc4a8d575ea8f3813c3634c508e','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202857,'user','You',1795762202857,1795762202857,NULL,NULL,NULL,1,'i');
INSERT INTO "tasks" VALUES(x'01a21bc4a8fa7c5bb60b4bd37578fc36',x'01a21bc4a88f750497deca96c920e3d6',3,'Fix the fence','fix the fence','',NULL,x'01a21bc4a88f7c23b8c2d78cdcfdd92e',1795762203001,x'01a21bc4a88f75d99a3e995c602f739c',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202874,'user','You',1795762203001,1795762203001,1795762203001,NULL,NULL,4,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a90b70019fbc7d3b846384f1',x'01a21bc4a8b176c49f58a697917e6fef',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21bc4a90b7e788303983e79345da3',x'01a21bc4a8b17902a4632de85cf56c70',1795762202891,x'01a21bc4a8b1743594ba29bbc80d751e',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202891,'user','You',1795762202891,1795762202891,NULL,NULL,NULL,1,NULL);
INSERT INTO "task_labels" VALUES(x'01a21bc4a8d575ea8f3813c3634c508e',x'01a21bc4a8c17b7d93e5c9e203047c64');
INSERT INTO "task_relations" VALUES(x'01a21bc4a91d7dd9a12fbcd999297f99',x'01a21bc4a8fa7c5bb60b4bd37578fc36',x'01a21bc4a8d575ea8f3813c3634c508e','blocks',NULL,1795762202909,'user','You');
INSERT INTO "comments" VALUES(x'01a21bc4a9427f0c9895cc2fd54fcf78',x'01a21bc4a8d575ea8f3813c3634c508e','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795762202946,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a88f7a7fa1cdeb3fa634658a','project',x'01a21bc4a88f750497deca96c920e3d6','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795762202767,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a8d57d26a234794c1ac18f65','task',x'01a21bc4a8d575ea8f3813c3634c508e','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795762202837,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a90b7e788303983e79345da3','task',x'01a21bc4a90b70019fbc7d3b846384f1','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795762202891,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a9327a9da61959f921a50bab','task',x'01a21bc4a8d575ea8f3813c3634c508e','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a8d57d26a234794c1ac18f65','normal',NULL,'user','You',NULL,1795762202930,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4a9557580b9321719137edce5',x'01a21bc4a8d575ea8f3813c3634c508e','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795762202965,NULL,NULL,NULL,1);
INSERT INTO "timer" VALUES(1,x'01a21bc4a8e97f09be96e5a2b4e4d0a7',1795762203017,'user','You',1795762203017,0,'',NULL);
INSERT INTO "activity" VALUES(1,x'01a21bc4a88f7104b3cc2389ec021969',1795762202767,'project.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','project',x'01a21bc4a88f750497deca96c920e3d6','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21bc4a8b17765bb24ea075619866c',1795762202801,'project.create','user','You','cli',NULL,x'01a21bc4a8b176c49f58a697917e6fef','project',x'01a21bc4a8b176c49f58a697917e6fef','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21bc4a8c17feaa7f1088091bb4402',1795762202817,'label.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','label',x'01a21bc4a8c17b7d93e5c9e203047c64','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21bc4a8d57b248cab69a8e3529091',1795762202837,'task.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8d575ea8f3813c3634c508e','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21bc4-a88f-7def-acbb-891a35b4303a"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21bc4a8e97dafba92222bcb37b570',1795762202857,'task.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8e97f09be96e5a2b4e4d0a7','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21bc4-a88f-7def-acbb-891a35b4303a"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21bc4a8fa7fcf9ee6e924e84ba5d0',1795762202874,'task.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8fa7c5bb60b4bd37578fc36','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21bc4-a88f-7def-acbb-891a35b4303a"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21bc4a90b7b2d9f95774eebc5f286',1795762202891,'task.create','user','You','cli',NULL,x'01a21bc4a8b176c49f58a697917e6fef','task',x'01a21bc4a90b70019fbc7d3b846384f1','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21bc4-a8b1-7902-a463-2de85cf56c70"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21bc4a91d7c9eb5e3381e740bb796',1795762202909,'relation.add','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','relation',x'01a21bc4a91d7dd9a12fbcd999297f99','{"fields":{"link":{"after":{"from":"01a21bc4-a8fa-7c5b-b60b-4bd37578fc36","kind":"blocks","to":"01a21bc4-a8d5-75ea-8f38-13c3634c508e"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21bc4a93278c69806383cbf92a548',1795762202930,'task.update','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8d575ea8f3813c3634c508e','{"fields":{"description":{"after":{"revision":"01a21bc4-a932-7a9d-a619-59f921a50bab"},"before":{"revision":"01a21bc4-a8d5-7d26-a234-794c1ac18f65"}}}}','{"set":{"description":{"revision":"01a21bc4-a8d5-7d26-a234-794c1ac18f65"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21bc4a9427dbe8cad2ce694a9d654',1795762202946,'comment.add','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','comment',x'01a21bc4a9427f0c9895cc2fd54fcf78','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21bc4a955737ebf4c0d6493b9b48a',1795762202965,'time.log','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','time_entry',x'01a21bc4a9557580b9321719137edce5','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21bc4a9687a4997406e53f2534e61',1795762202984,'reminder.set','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8d575ea8f3813c3634c508e','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21bc4a979731eb17cbe63415bde27',1795762203001,'task.move','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','task',x'01a21bc4a8fa7c5bb60b4bd37578fc36','{"fields":{"closed_at":{"after":1795762203001},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21bc4-a88f-7c23-b8c2-d78cdcfdd92e","before":"01a21bc4-a88f-7def-acbb-891a35b4303a"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21bc4-a88f-7def-acbb-891a35b4303a"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21bc4a9897740bd491749a08d06d8',1795762203017,'timer.start','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','timer',x'01a21bc4a866795fbee9b901edb800d9','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795762203017,"started_at":1795762203017,"task":"01a21bc4-a8e9-7f09-be96-e5a2b4e4d0a7"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21bc4a998751cbc0c31de584651f4',1795762203032,'wiki.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a99876d3a0895b61ba30b36c','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21bc4a9ab7647ab70e347a855323c',1795762203051,'wiki.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a9ab73c883a30e06e2e13492','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21bc4a9bc7d11b29574bdd9b0f610',1795762203068,'wiki.update','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a9ab73c883a30e06e2e13492','{"fields":{"body":{"after":{"revision":"01a21bc4-a9bc-7824-a94d-d82815963142"},"before":{"revision":"01a21bc4-a9ab-76c1-88c1-1716c114845d"}}}}','{"set":{"body":{"revision":"01a21bc4-a9ab-76c1-88c1-1716c114845d"}}}',NULL);
INSERT INTO "activity" VALUES(18,x'01a21bc4a9cd7848a3dc6e37e9b76b80',1795762203085,'wiki.update','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a99876d3a0895b61ba30b36c','{"fields":{"home":{"after":true,"before":false}}}','{"set":{"home":false}}',NULL);
INSERT INTO "activity" VALUES(19,x'01a21bc4a9dd7a0db3df55f1bab888fd',1795762203101,'wiki.create','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a9dd7ca3b073aabb62998420','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(20,x'01a21bc4a9f0758788131709e5a55e1f',1795762203120,'wiki.update','user','You','cli',NULL,x'01a21bc4a88f750497deca96c920e3d6','wiki_page',x'01a21bc4a9dd7ca3b073aabb62998420','{"fields":{"template":{"after":true,"before":false}}}','{"set":{"template":false}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21bc4a8d575ea8f3813c3634c508e',x'01a21bc4a8d575ea8f3813c3634c508e',x'01a21bc4a88f750497deca96c920e3d6','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795762202930);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21bc4a8e97f09be96e5a2b4e4d0a7',x'01a21bc4a8e97f09be96e5a2b4e4d0a7',x'01a21bc4a88f750497deca96c920e3d6','Buy seeds','',1795762202857);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21bc4a8fa7c5bb60b4bd37578fc36',x'01a21bc4a8fa7c5bb60b4bd37578fc36',x'01a21bc4a88f750497deca96c920e3d6','Fix the fence','',1795762202874);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21bc4a90b70019fbc7d3b846384f1',x'01a21bc4a90b70019fbc7d3b846384f1',x'01a21bc4a8b176c49f58a697917e6fef','Launch the homepage','Ship it soon.
',1795762202891);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21bc4a9427f0c9895cc2fd54fcf78',x'01a21bc4a8d575ea8f3813c3634c508e',x'01a21bc4a88f750497deca96c920e3d6','Plan the garden','The neighbours like the garden plan.
',1795762202946);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21bc4a99876d3a0895b61ba30b36c',NULL,x'01a21bc4a88f750497deca96c920e3d6','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795762203032);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21bc4a9ab73c883a30e06e2e13492',NULL,x'01a21bc4a88f750497deca96c920e3d6','Seeds','Tomatoes, beans and peas.
',1795762203068);
INSERT INTO "search_docs" VALUES(8,'wiki',x'01a21bc4a9dd7ca3b073aabb62998420',NULL,x'01a21bc4a88f750497deca96c920e3d6','Meeting notes','Who
What
',1795762203101);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a99876d3a0895b61ba30b36c',x'01a21bc4a88f750497deca96c920e3d6',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21bc4a9987748b1a6a6de76475e00',0,1795762203032,1795762203085,NULL,NULL,1,0);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a9ab73c883a30e06e2e13492',x'01a21bc4a88f750497deca96c920e3d6',x'01a21bc4a99876d3a0895b61ba30b36c','i','Seeds','seeds','seeds',x'01a21bc4a9bc7824a94dd82815963142',0,1795762203051,1795762203068,NULL,NULL,0,0);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a9dd7ca3b073aabb62998420',x'01a21bc4a88f750497deca96c920e3d6',NULL,'i001','Meeting notes','meeting notes','meeting-notes',x'01a21bc4a9dd7dd08ec0d807b18c430b',0,1795762203101,1795762203120,NULL,NULL,0,1);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a9987748b1a6a6de76475e00',x'01a21bc4a99876d3a0895b61ba30b36c','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795762203032,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a9ab76c188c11716c114845d',x'01a21bc4a9ab73c883a30e06e2e13492','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795762203051,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a9bc7824a94dd82815963142',x'01a21bc4a9ab73c883a30e06e2e13492','Tomatoes, beans and peas.',x'01a21bc4a9ab76c188c11716c114845d','normal',NULL,'user','You',NULL,1795762203068,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a9dd7dd08ec0d807b18c430b',x'01a21bc4a9dd7ca3b073aabb62998420','## Who

## What',NULL,'normal',NULL,'user','You',NULL,1795762203101,NULL);
INSERT INTO "reminders" VALUES(x'01a21bc4a9687bc4b2b00482b597335f',x'01a21bc4a8d575ea8f3813c3634c508e',1796029200000,NULL,NULL,'','user','You',1795762202984,NULL,NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21bc4a99876d3a0895b61ba30b36c',0,'wiki','Seeds',NULL,'seeds','seeds','','',NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21bc4a99876d3a0895b61ba30b36c',1,'task','TM-1',NULL,'','','','',1);
CREATE INDEX projects_parent ON projects (parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX project_key_aliases_project ON project_key_aliases (project_id);
CREATE UNIQUE INDEX task_types_one_default ON task_types (project_id) WHERE is_default = 1;
CREATE INDEX tasks_board   ON tasks (project_id, status_id, rank) WHERE deleted_at IS NULL;
CREATE INDEX tasks_due     ON tasks (due_date) WHERE deleted_at IS NULL AND due_date IS NOT NULL;
CREATE INDEX tasks_parent  ON tasks (parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX tasks_updated ON tasks (project_id, updated_at);
CREATE INDEX tasks_trash   ON tasks (deleted_group) WHERE deleted_at IS NOT NULL;
CREATE INDEX task_key_aliases_task ON task_key_aliases (task_id);
CREATE INDEX task_labels_label ON task_labels (label_id);
CREATE INDEX task_relations_to ON task_relations (to_task, kind);
CREATE INDEX comments_task  ON comments (task_id, created_at);
CREATE INDEX comments_trash ON comments (deleted_group) WHERE deleted_at IS NOT NULL;
CREATE INDEX comment_revisions_comment ON comment_revisions (comment_id, created_at);
CREATE INDEX text_revisions_owner ON text_revisions (owner_kind, owner_id, created_at);
CREATE INDEX time_entries_task  ON time_entries (task_id, work_date);
CREATE INDEX time_entries_date  ON time_entries (work_date) WHERE deleted_at IS NULL;
CREATE INDEX time_entries_trash ON time_entries (deleted_group) WHERE deleted_at IS NOT NULL;
CREATE INDEX attachments_owner ON attachments (owner_kind, owner_id);
CREATE INDEX attachments_blob  ON attachments (sha256);
CREATE INDEX attachments_trash ON attachments (deleted_group) WHERE deleted_at IS NOT NULL;
CREATE INDEX saved_views_project ON saved_views (project_id);
CREATE INDEX activity_entity  ON activity (entity_kind, entity_id, seq);
CREATE INDEX activity_project ON activity (project_id, seq);
CREATE INDEX activity_session ON activity (session_id, seq) WHERE session_id IS NOT NULL;
CREATE INDEX activity_group   ON activity (group_id);
CREATE TRIGGER search_docs_insert AFTER INSERT ON search_docs BEGIN
  INSERT INTO fts_words (rowid, title, body) VALUES (new.docid, new.title, new.body);
  INSERT INTO fts_grams (rowid, title, body) VALUES (new.docid, new.title, new.body);
END;
CREATE TRIGGER search_docs_delete AFTER DELETE ON search_docs BEGIN
  INSERT INTO fts_words (fts_words, rowid, title, body) VALUES ('delete', old.docid, old.title, old.body);
  INSERT INTO fts_grams (fts_grams, rowid, title, body) VALUES ('delete', old.docid, old.title, old.body);
END;
CREATE TRIGGER search_docs_update AFTER UPDATE OF title, body ON search_docs BEGIN
  INSERT INTO fts_words (fts_words, rowid, title, body) VALUES ('delete', old.docid, old.title, old.body);
  INSERT INTO fts_grams (fts_grams, rowid, title, body) VALUES ('delete', old.docid, old.title, old.body);
  INSERT INTO fts_words (rowid, title, body) VALUES (new.docid, new.title, new.body);
  INSERT INTO fts_grams (rowid, title, body) VALUES (new.docid, new.title, new.body);
END;
CREATE INDEX wiki_pages_parent ON wiki_pages (project_id, parent_id, rank);
CREATE UNIQUE INDEX wiki_pages_one_brief ON wiki_pages (project_id) WHERE is_agent_brief = 1;
CREATE INDEX wiki_revisions_page ON wiki_revisions (page_id, created_at);
CREATE INDEX request_log_at ON request_log (at);
CREATE UNIQUE INDEX wiki_pages_name ON wiki_pages (project_id, title_fold)
  WHERE deleted_at IS NULL;
CREATE INDEX tasks_subtasks ON tasks (parent_id, subtask_rank)
  WHERE parent_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX reminders_at ON reminders (remind_at) WHERE remind_at IS NOT NULL;
CREATE INDEX drafts_owner ON drafts (owner_kind, owner_id);
CREATE INDEX drafts_updated ON drafts (updated_at);
CREATE INDEX text_links_name_fold ON text_links (name_fold) WHERE target_kind = 'wiki';
CREATE INDEX text_links_name_slug ON text_links (name_slug) WHERE target_kind = 'wiki';
CREATE INDEX text_links_tail_fold ON text_links (tail_fold) WHERE tail_fold <> '';
CREATE INDEX text_links_tail_slug ON text_links (tail_slug) WHERE tail_slug <> '';
CREATE INDEX text_links_task      ON text_links (task_number) WHERE target_kind = 'task';
CREATE UNIQUE INDEX wiki_pages_one_home ON wiki_pages (project_id) WHERE is_home = 1;
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 10;
COMMIT;
