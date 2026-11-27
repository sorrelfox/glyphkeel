-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.0.5.
-- release: 0.0.5
-- schema: 5
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
CREATE TABLE drafts (
  owner_kind TEXT NOT NULL CHECK (owner_kind IN
               ('task', 'comment', 'new_comment', 'project', 'wiki_page', 'new_wiki_page')),
  owner_id   BLOB NOT NULL,
  title      TEXT,
  body       TEXT NOT NULL,
  base_rev   BLOB,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (owner_kind, owner_id)
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
) STRICT;
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
INSERT INTO "meta" VALUES('workspace_id','01a21bc4-adf9-7aeb-90ad-22f56f7452ba');
INSERT INTO "meta" VALUES('created_at','1795762204153');
INSERT INTO "meta" VALUES('created_by_version','0.0.5');
INSERT INTO "projects" VALUES(x'01a21bc4ae1174fa9c862c90f7d52bf3',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21bc4ae117e51b1486079c9cc4fed','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',4,1795762204177,1795762204272,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21bc4ae217143b6e192f4d8f66c07',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795762204193,1795762204289,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae11712eba48a3a2bec5bcb7',x'01a21bc4ae1174fa9c862c90f7d52bf3','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae117f4db4820cb78566331d',x'01a21bc4ae1174fa9c862c90f7d52bf3','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae1178b7a1d0e9b49d4cfb4f',x'01a21bc4ae1174fa9c862c90f7d52bf3','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae1179fcbe4c11f0f6f2e06d',x'01a21bc4ae1174fa9c862c90f7d52bf3','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae117f17902c25d1b18fb684',x'01a21bc4ae1174fa9c862c90f7d52bf3','Done','done','done','green','u',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae217ffca086c964b182a72a',x'01a21bc4ae217143b6e192f4d8f66c07','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae2179889a58c3227a41b319',x'01a21bc4ae217143b6e192f4d8f66c07','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae217375aaddd844c91e31fa',x'01a21bc4ae217143b6e192f4d8f66c07','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae217518baea29857f20b49b',x'01a21bc4ae217143b6e192f4d8f66c07','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4ae2178bc9aba9e59e62171d7',x'01a21bc4ae217143b6e192f4d8f66c07','Done','done','done','green','u',NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4ae11718984a611e2faa40212',x'01a21bc4ae1174fa9c862c90f7d52bf3','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4ae117a5990215bfa34f9a8cb',x'01a21bc4ae1174fa9c862c90f7d52bf3','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4ae1171bc970fa50b2b8a8177',x'01a21bc4ae1174fa9c862c90f7d52bf3','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4ae1172bcb860bb29ed7ebc80',x'01a21bc4ae1174fa9c862c90f7d52bf3','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "task_types" VALUES(x'01a21bc4ae2171c6aa1c59d47f6d7955',x'01a21bc4ae217143b6e192f4d8f66c07','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4ae2173499d6f63480a20173d',x'01a21bc4ae217143b6e192f4d8f66c07','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4ae2175d986cb6be4a65995b4',x'01a21bc4ae217143b6e192f4d8f66c07','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4ae21725089f7a04613b224b6',x'01a21bc4ae217143b6e192f4d8f66c07','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "labels" VALUES(x'01a21bc4ae34733c9e4cf60095cff8dc',x'01a21bc4ae1174fa9c862c90f7d52bf3','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21bc4ae487fa5b1245c88c66c88e4',x'01a21bc4ae1174fa9c862c90f7d52bf3',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4aea67b40bc68c456f301806f',x'01a21bc4ae11712eba48a3a2bec5bcb7',1795762204232,x'01a21bc4ae11718984a611e2faa40212',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,90,NULL,NULL,NULL,1795762204232,'user','You',1795762204326,1795762204366,NULL,NULL,NULL,2,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ae5d7efabebb196c4859ff78',x'01a21bc4ae1174fa9c862c90f7d52bf3',2,'Buy seeds','buy seeds','',NULL,x'01a21bc4ae11712eba48a3a2bec5bcb7',1795762204253,x'01a21bc4ae11718984a611e2faa40212',0,x'01a21bc4ae487fa5b1245c88c66c88e4','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762204253,'user','You',1795762204253,1795762204253,NULL,NULL,NULL,1,'i');
INSERT INTO "tasks" VALUES(x'01a21bc4ae70744f8b11b5c16aaca446',x'01a21bc4ae1174fa9c862c90f7d52bf3',3,'Fix the fence','fix the fence','',NULL,x'01a21bc4ae117f17902c25d1b18fb684',1795762204409,x'01a21bc4ae11718984a611e2faa40212',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762204272,'user','You',1795762204409,1795762204409,1795762204409,NULL,NULL,4,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ae81722dbd3a78da5bd234ad',x'01a21bc4ae217143b6e192f4d8f66c07',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21bc4ae817713909a3f30a5159dd6',x'01a21bc4ae217ffca086c964b182a72a',1795762204289,x'01a21bc4ae2171c6aa1c59d47f6d7955',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762204289,'user','You',1795762204289,1795762204289,NULL,NULL,NULL,1,NULL);
INSERT INTO "task_labels" VALUES(x'01a21bc4ae487fa5b1245c88c66c88e4',x'01a21bc4ae34733c9e4cf60095cff8dc');
INSERT INTO "task_relations" VALUES(x'01a21bc4ae937fb38d8f738fe1f6f265',x'01a21bc4ae70744f8b11b5c16aaca446',x'01a21bc4ae487fa5b1245c88c66c88e4','blocks',NULL,1795762204307,'user','You');
INSERT INTO "comments" VALUES(x'01a21bc4aeb97e9aa7a56594290a2399',x'01a21bc4ae487fa5b1245c88c66c88e4','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795762204345,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ae117e51b1486079c9cc4fed','project',x'01a21bc4ae1174fa9c862c90f7d52bf3','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795762204177,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ae487dd2a7c6315abafdb0a6','task',x'01a21bc4ae487fa5b1245c88c66c88e4','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795762204232,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ae817713909a3f30a5159dd6','task',x'01a21bc4ae81722dbd3a78da5bd234ad','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795762204289,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4aea67b40bc68c456f301806f','task',x'01a21bc4ae487fa5b1245c88c66c88e4','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4ae487dd2a7c6315abafdb0a6','normal',NULL,'user','You',NULL,1795762204326,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4aece7fee81f4d851db937bcd',x'01a21bc4ae487fa5b1245c88c66c88e4','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795762204366,NULL,NULL,NULL,1);
INSERT INTO "timer" VALUES(1,x'01a21bc4ae5d7efabebb196c4859ff78',1795762204426,'user','You',1795762204426,0,'',NULL);
INSERT INTO "activity" VALUES(1,x'01a21bc4ae1179fc95668651770d09d5',1795762204177,'project.create','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','project',x'01a21bc4ae1174fa9c862c90f7d52bf3','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21bc4ae217443b0cfd6be45c0cb04',1795762204193,'project.create','user','You','cli',NULL,x'01a21bc4ae217143b6e192f4d8f66c07','project',x'01a21bc4ae217143b6e192f4d8f66c07','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21bc4ae3471ae982bf6d083cac7a2',1795762204212,'label.create','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','label',x'01a21bc4ae34733c9e4cf60095cff8dc','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21bc4ae487201818b0e499fdac852',1795762204232,'task.create','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae487fa5b1245c88c66c88e4','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21bc4-ae11-712e-ba48-a3a2bec5bcb7"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21bc4ae5d7bd19e1231d269a737d4',1795762204253,'task.create','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae5d7efabebb196c4859ff78','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21bc4-ae11-712e-ba48-a3a2bec5bcb7"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21bc4ae707cd3bf4dea81c36dda59',1795762204272,'task.create','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae70744f8b11b5c16aaca446','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21bc4-ae11-712e-ba48-a3a2bec5bcb7"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21bc4ae817bb7b66924f1f558f435',1795762204289,'task.create','user','You','cli',NULL,x'01a21bc4ae217143b6e192f4d8f66c07','task',x'01a21bc4ae81722dbd3a78da5bd234ad','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21bc4-ae21-7ffc-a086-c964b182a72a"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21bc4ae937b7ca1bc53ac260a82b3',1795762204307,'relation.add','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','relation',x'01a21bc4ae937fb38d8f738fe1f6f265','{"fields":{"link":{"after":{"from":"01a21bc4-ae70-744f-8b11-b5c16aaca446","kind":"blocks","to":"01a21bc4-ae48-7fa5-b124-5c88c66c88e4"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21bc4aea6736d862221e67c37bc52',1795762204326,'task.update','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae487fa5b1245c88c66c88e4','{"fields":{"description":{"after":{"revision":"01a21bc4-aea6-7b40-bc68-c456f301806f"},"before":{"revision":"01a21bc4-ae48-7dd2-a7c6-315abafdb0a6"}}}}','{"set":{"description":{"revision":"01a21bc4-ae48-7dd2-a7c6-315abafdb0a6"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21bc4aeb97bd3bf91bf9cc96be944',1795762204345,'comment.add','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','comment',x'01a21bc4aeb97e9aa7a56594290a2399','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21bc4aece7c978f0aa291b571024a',1795762204366,'time.log','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','time_entry',x'01a21bc4aece7fee81f4d851db937bcd','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21bc4aee279e8b8a5e522e02a82ba',1795762204386,'reminder.set','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae487fa5b1245c88c66c88e4','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21bc4aef975b9adfbe88804742930',1795762204409,'task.move','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','task',x'01a21bc4ae70744f8b11b5c16aaca446','{"fields":{"closed_at":{"after":1795762204409},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21bc4-ae11-7f17-902c-25d1b18fb684","before":"01a21bc4-ae11-712e-ba48-a3a2bec5bcb7"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21bc4-ae11-712e-ba48-a3a2bec5bcb7"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21bc4af0a7e02b9dceff5888f041b',1795762204426,'timer.start','user','You','cli',NULL,x'01a21bc4ae1174fa9c862c90f7d52bf3','timer',x'01a21bc4adf97aeb90ad22f56f7452ba','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795762204426,"started_at":1795762204426,"task":"01a21bc4-ae5d-7efa-bebb-196c4859ff78"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21bc4ae487fa5b1245c88c66c88e4',x'01a21bc4ae487fa5b1245c88c66c88e4',x'01a21bc4ae1174fa9c862c90f7d52bf3','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795762204326);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21bc4ae5d7efabebb196c4859ff78',x'01a21bc4ae5d7efabebb196c4859ff78',x'01a21bc4ae1174fa9c862c90f7d52bf3','Buy seeds','',1795762204253);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21bc4ae70744f8b11b5c16aaca446',x'01a21bc4ae70744f8b11b5c16aaca446',x'01a21bc4ae1174fa9c862c90f7d52bf3','Fix the fence','',1795762204272);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21bc4ae81722dbd3a78da5bd234ad',x'01a21bc4ae81722dbd3a78da5bd234ad',x'01a21bc4ae217143b6e192f4d8f66c07','Launch the homepage','Ship it soon.
',1795762204289);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21bc4aeb97e9aa7a56594290a2399',x'01a21bc4ae487fa5b1245c88c66c88e4',x'01a21bc4ae1174fa9c862c90f7d52bf3','Plan the garden','The neighbours like the garden plan.
',1795762204345);
INSERT INTO "reminders" VALUES(x'01a21bc4aee27d349165df2bde9e6f3c',x'01a21bc4ae487fa5b1245c88c66c88e4',1796029200000,NULL,NULL,'','user','You',1795762204386,NULL,NULL);
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
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 5;
COMMIT;
