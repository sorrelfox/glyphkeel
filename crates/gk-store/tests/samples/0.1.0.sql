-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.0.
-- release: 0.1.0
-- schema: 7
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
  deleted_group  BLOB,
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
INSERT INTO "meta" VALUES('workspace_id','01a21bc4-a678-7b34-8fd1-f93e61211827');
INSERT INTO "meta" VALUES('created_at','1795762202232');
INSERT INTO "meta" VALUES('created_by_version','0.1.0');
INSERT INTO "projects" VALUES(x'01a21bc4a6967e73ac82d92d348af2d0',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21bc4a6967b2a8a64b8363a771c5b','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',4,1795762202262,1795762202365,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21bc4a6b777f4bdaa3fe13433bbda',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795762202295,1795762202380,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6967780a666c198237d9af7',x'01a21bc4a6967e73ac82d92d348af2d0','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a696716684d14ccfb56de18b',x'01a21bc4a6967e73ac82d92d348af2d0','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a696742eba87a1e71370673e',x'01a21bc4a6967e73ac82d92d348af2d0','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a69677099be03a706c063c69',x'01a21bc4a6967e73ac82d92d348af2d0','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6967b75a8ea1286a7bcdc8c',x'01a21bc4a6967e73ac82d92d348af2d0','Done','done','done','green','u',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6b7736aa28755620a01199f',x'01a21bc4a6b777f4bdaa3fe13433bbda','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6b77f8a9206bc20c16f3f3b',x'01a21bc4a6b777f4bdaa3fe13433bbda','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6b771d9bf8a616975651137',x'01a21bc4a6b777f4bdaa3fe13433bbda','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6b770ed9c86a8ece4cd0f91',x'01a21bc4a6b777f4bdaa3fe13433bbda','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a6b774c3bda3aa3c41bd3837',x'01a21bc4a6b777f4bdaa3fe13433bbda','Done','done','done','green','u',NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4a696743e8e4334ab57ab3c84',x'01a21bc4a6967e73ac82d92d348af2d0','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a6967dfc843e7052166ce3a5',x'01a21bc4a6967e73ac82d92d348af2d0','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a6967a2ab673772600b30c7a',x'01a21bc4a6967e73ac82d92d348af2d0','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a69670a7b8f1cc6274a31bac',x'01a21bc4a6967e73ac82d92d348af2d0','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a6b777e0b3ef22be36ea6dbf',x'01a21bc4a6b777f4bdaa3fe13433bbda','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a6b77d0ab01c1937fcb9a535',x'01a21bc4a6b777f4bdaa3fe13433bbda','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a6b77cfb96e132479feebf67',x'01a21bc4a6b777f4bdaa3fe13433bbda','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a6b777cdb047b820d69851e1',x'01a21bc4a6b777f4bdaa3fe13433bbda','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "labels" VALUES(x'01a21bc4a6c973f1b75a2b19a232f133',x'01a21bc4a6967e73ac82d92d348af2d0','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21bc4a6da7edebe17e9e871cbcdd4',x'01a21bc4a6967e73ac82d92d348af2d0',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a7317f219ea51e0a2af61a9b',x'01a21bc4a6967780a666c198237d9af7',1795762202330,x'01a21bc4a696743e8e4334ab57ab3c84',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,90,NULL,NULL,NULL,1795762202330,'user','You',1795762202417,1795762202454,NULL,NULL,NULL,2,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a6ed7f2eaae84bae91eabdda',x'01a21bc4a6967e73ac82d92d348af2d0',2,'Buy seeds','buy seeds','',NULL,x'01a21bc4a6967780a666c198237d9af7',1795762202349,x'01a21bc4a696743e8e4334ab57ab3c84',0,x'01a21bc4a6da7edebe17e9e871cbcdd4','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202349,'user','You',1795762202349,1795762202349,NULL,NULL,NULL,1,'i');
INSERT INTO "tasks" VALUES(x'01a21bc4a6fd782396a14dd32448a195',x'01a21bc4a6967e73ac82d92d348af2d0',3,'Fix the fence','fix the fence','',NULL,x'01a21bc4a6967b75a8ea1286a7bcdc8c',1795762202488,x'01a21bc4a696743e8e4334ab57ab3c84',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202365,'user','You',1795762202488,1795762202488,1795762202488,NULL,NULL,4,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a70c72b69b9f49f12cbfda73',x'01a21bc4a6b777f4bdaa3fe13433bbda',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21bc4a70c71d4b0c082a51ac70c5e',x'01a21bc4a6b7736aa28755620a01199f',1795762202380,x'01a21bc4a6b777e0b3ef22be36ea6dbf',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762202380,'user','You',1795762202380,1795762202380,NULL,NULL,NULL,1,NULL);
INSERT INTO "task_labels" VALUES(x'01a21bc4a6da7edebe17e9e871cbcdd4',x'01a21bc4a6c973f1b75a2b19a232f133');
INSERT INTO "task_relations" VALUES(x'01a21bc4a71c7cb492e8ad7743202ce8',x'01a21bc4a6fd782396a14dd32448a195',x'01a21bc4a6da7edebe17e9e871cbcdd4','blocks',NULL,1795762202396,'user','You');
INSERT INTO "comments" VALUES(x'01a21bc4a74472fda0b74f44a9c6b12f',x'01a21bc4a6da7edebe17e9e871cbcdd4','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795762202436,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a6967b2a8a64b8363a771c5b','project',x'01a21bc4a6967e73ac82d92d348af2d0','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795762202262,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a6da79a7bb4710d429ddb30c','task',x'01a21bc4a6da7edebe17e9e871cbcdd4','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795762202330,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a70c71d4b0c082a51ac70c5e','task',x'01a21bc4a70c72b69b9f49f12cbfda73','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795762202380,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a7317f219ea51e0a2af61a9b','task',x'01a21bc4a6da7edebe17e9e871cbcdd4','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a6da79a7bb4710d429ddb30c','normal',NULL,'user','You',NULL,1795762202417,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4a7567501a88b5f7f4134d3ef',x'01a21bc4a6da7edebe17e9e871cbcdd4','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795762202454,NULL,NULL,NULL,1);
INSERT INTO "timer" VALUES(1,x'01a21bc4a6ed7f2eaae84bae91eabdda',1795762202503,'user','You',1795762202503,0,'',NULL);
INSERT INTO "activity" VALUES(1,x'01a21bc4a6967f3eac29c868bfd846ad',1795762202262,'project.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','project',x'01a21bc4a6967e73ac82d92d348af2d0','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21bc4a6b778d094763f28758f21d2',1795762202295,'project.create','user','You','cli',NULL,x'01a21bc4a6b777f4bdaa3fe13433bbda','project',x'01a21bc4a6b777f4bdaa3fe13433bbda','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21bc4a6c97afeb3decd82ff264a2a',1795762202313,'label.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','label',x'01a21bc4a6c973f1b75a2b19a232f133','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21bc4a6da758bb8a32be64db9eb4d',1795762202330,'task.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6da7edebe17e9e871cbcdd4','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21bc4-a696-7780-a666-c198237d9af7"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21bc4a6ed7f2db9abb2241230ca8d',1795762202349,'task.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6ed7f2eaae84bae91eabdda','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21bc4-a696-7780-a666-c198237d9af7"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21bc4a6fd7e989f369d8fe9314bd6',1795762202365,'task.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6fd782396a14dd32448a195','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21bc4-a696-7780-a666-c198237d9af7"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21bc4a70c76f7b49d76a68e2a500e',1795762202380,'task.create','user','You','cli',NULL,x'01a21bc4a6b777f4bdaa3fe13433bbda','task',x'01a21bc4a70c72b69b9f49f12cbfda73','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21bc4-a6b7-736a-a287-55620a01199f"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21bc4a71c7023bbb810af2ade0be1',1795762202396,'relation.add','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','relation',x'01a21bc4a71c7cb492e8ad7743202ce8','{"fields":{"link":{"after":{"from":"01a21bc4-a6fd-7823-96a1-4dd32448a195","kind":"blocks","to":"01a21bc4-a6da-7ede-be17-e9e871cbcdd4"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21bc4a73176919513d877fb3d184e',1795762202417,'task.update','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6da7edebe17e9e871cbcdd4','{"fields":{"description":{"after":{"revision":"01a21bc4-a731-7f21-9ea5-1e0a2af61a9b"},"before":{"revision":"01a21bc4-a6da-79a7-bb47-10d429ddb30c"}}}}','{"set":{"description":{"revision":"01a21bc4-a6da-79a7-bb47-10d429ddb30c"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21bc4a74479e2a86b84ff776f30b7',1795762202436,'comment.add','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','comment',x'01a21bc4a74472fda0b74f44a9c6b12f','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21bc4a75679519bad5d38d394c9f4',1795762202454,'time.log','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','time_entry',x'01a21bc4a7567501a88b5f7f4134d3ef','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21bc4a76773d2b3b7cf7103513c13',1795762202471,'reminder.set','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6da7edebe17e9e871cbcdd4','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21bc4a77875d086c944b785a0a8ff',1795762202488,'task.move','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','task',x'01a21bc4a6fd782396a14dd32448a195','{"fields":{"closed_at":{"after":1795762202488},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21bc4-a696-7b75-a8ea-1286a7bcdc8c","before":"01a21bc4-a696-7780-a666-c198237d9af7"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21bc4-a696-7780-a666-c198237d9af7"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21bc4a7877071addf379573deae3b',1795762202503,'timer.start','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','timer',x'01a21bc4a6787b348fd1f93e61211827','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795762202503,"started_at":1795762202503,"task":"01a21bc4-a6ed-7f2e-aae8-4bae91eabdda"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21bc4a7967f2ba146b2163cc4c99a',1795762202518,'wiki.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','wiki_page',x'01a21bc4a796795491be9c98880585cc','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21bc4a7ac7223ae67ea6add911f80',1795762202540,'wiki.create','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','wiki_page',x'01a21bc4a7ac7351b562cd056c4f0646','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21bc4a7bf7d159a15564b9a196dc6',1795762202559,'wiki.update','user','You','cli',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','wiki_page',x'01a21bc4a7ac7351b562cd056c4f0646','{"fields":{"body":{"after":{"revision":"01a21bc4-a7bf-705f-948a-6844288e7f9f"},"before":{"revision":"01a21bc4-a7ac-7f0c-868e-cda8e7b2e01d"}}}}','{"set":{"body":{"revision":"01a21bc4-a7ac-7f0c-868e-cda8e7b2e01d"}}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21bc4a6da7edebe17e9e871cbcdd4',x'01a21bc4a6da7edebe17e9e871cbcdd4',x'01a21bc4a6967e73ac82d92d348af2d0','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795762202417);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21bc4a6ed7f2eaae84bae91eabdda',x'01a21bc4a6ed7f2eaae84bae91eabdda',x'01a21bc4a6967e73ac82d92d348af2d0','Buy seeds','',1795762202349);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21bc4a6fd782396a14dd32448a195',x'01a21bc4a6fd782396a14dd32448a195',x'01a21bc4a6967e73ac82d92d348af2d0','Fix the fence','',1795762202365);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21bc4a70c72b69b9f49f12cbfda73',x'01a21bc4a70c72b69b9f49f12cbfda73',x'01a21bc4a6b777f4bdaa3fe13433bbda','Launch the homepage','Ship it soon.
',1795762202380);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21bc4a74472fda0b74f44a9c6b12f',x'01a21bc4a6da7edebe17e9e871cbcdd4',x'01a21bc4a6967e73ac82d92d348af2d0','Plan the garden','The neighbours like the garden plan.
',1795762202436);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21bc4a796795491be9c98880585cc',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795762202518);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21bc4a7ac7351b562cd056c4f0646',NULL,x'01a21bc4a6967e73ac82d92d348af2d0','Seeds','Tomatoes, beans and peas.
',1795762202559);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a796795491be9c98880585cc',x'01a21bc4a6967e73ac82d92d348af2d0',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21bc4a796781ea6f737c677536092',0,1795762202518,1795762202518,NULL,NULL);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a7ac7351b562cd056c4f0646',x'01a21bc4a6967e73ac82d92d348af2d0',x'01a21bc4a796795491be9c98880585cc','i','Seeds','seeds','seeds',x'01a21bc4a7bf705f948a6844288e7f9f',0,1795762202540,1795762202559,NULL,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a796781ea6f737c677536092',x'01a21bc4a796795491be9c98880585cc','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795762202518,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a7ac7f0c868ecda8e7b2e01d',x'01a21bc4a7ac7351b562cd056c4f0646','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795762202540,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a7bf705f948a6844288e7f9f',x'01a21bc4a7ac7351b562cd056c4f0646','Tomatoes, beans and peas.',x'01a21bc4a7ac7f0c868ecda8e7b2e01d','normal',NULL,'user','You',NULL,1795762202559,NULL);
INSERT INTO "reminders" VALUES(x'01a21bc4a767786c8621ba5e7c4f0ef8',x'01a21bc4a6da7edebe17e9e871cbcdd4',1796029200000,NULL,NULL,'','user','You',1795762202471,NULL,NULL);
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
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 7;
COMMIT;
