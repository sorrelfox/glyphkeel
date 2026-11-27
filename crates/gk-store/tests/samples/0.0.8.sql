-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.0.8.
-- release: 0.0.8
-- schema: 6
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
INSERT INTO "meta" VALUES('workspace_id','01a21bc4-a48a-7248-afa4-91d866c543aa');
INSERT INTO "meta" VALUES('created_at','1795762201738');
INSERT INTO "meta" VALUES('created_by_version','0.0.8');
INSERT INTO "projects" VALUES(x'01a21bc4a4a67d59a25eb326f85a5034',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21bc4a4a67a39bb9329bc739705a6','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',4,1795762201766,1795762201867,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21bc4a4c679ebbefb1fe87552573e',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795762201798,1795762201883,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4a6757e94c85df9b3de8a00',x'01a21bc4a4a67d59a25eb326f85a5034','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4a67a5da4e4c21eccce8199',x'01a21bc4a4a67d59a25eb326f85a5034','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4a67ad9a17dc478411906e9',x'01a21bc4a4a67d59a25eb326f85a5034','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4a67cd59bbe5d5848753255',x'01a21bc4a4a67d59a25eb326f85a5034','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4a67ad18f4db2c6d09939ee',x'01a21bc4a4a67d59a25eb326f85a5034','Done','done','done','green','u',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4c67804b832e034a5d86082',x'01a21bc4a4c679ebbefb1fe87552573e','Backlog','backlog','todo','grey','6',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4c677a3a4d2b42cd5f3a34f',x'01a21bc4a4c679ebbefb1fe87552573e','To do','to do','todo','blue','c',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4c67dad991aea36743465f4',x'01a21bc4a4c679ebbefb1fe87552573e','In progress','in progress','in_progress','orange','i',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4c67dee930a10eb4b570936',x'01a21bc4a4c679ebbefb1fe87552573e','Review','review','in_progress','purple','o',NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4a4c67e4a92672c654ad67b6d',x'01a21bc4a4c679ebbefb1fe87552573e','Done','done','done','green','u',NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4a4a67ddd9b99ed1a1d7091c1',x'01a21bc4a4a67d59a25eb326f85a5034','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a4a6718e8120ee2b3137c033',x'01a21bc4a4a67d59a25eb326f85a5034','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a4a67dfca1cd9303385df48b',x'01a21bc4a4a67d59a25eb326f85a5034','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a4a67884a2acf5c50ce10182',x'01a21bc4a4a67d59a25eb326f85a5034','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a4c67b53a35fc8b5b1bd8aa7',x'01a21bc4a4c679ebbefb1fe87552573e','Task','task','task','blue','7777',1);
INSERT INTO "task_types" VALUES(x'01a21bc4a4c67099a66cecd8cd08dc90',x'01a21bc4a4c679ebbefb1fe87552573e','Bug','bug','bug','red','eeee',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a4c6799fb4968cd92a69a534',x'01a21bc4a4c679ebbefb1fe87552573e','Feature','feature','feature','green','llll',0);
INSERT INTO "task_types" VALUES(x'01a21bc4a4c67f91b0f4374e884f4253',x'01a21bc4a4c679ebbefb1fe87552573e','Milestone','milestone','milestone','purple','ssss',0);
INSERT INTO "labels" VALUES(x'01a21bc4a4d67ef0acaffff4e0309f15',x'01a21bc4a4a67d59a25eb326f85a5034','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21bc4a4e87f56a3c57d9197106bc2',x'01a21bc4a4a67d59a25eb326f85a5034',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a53f75ee8c444cf09e5b9d27',x'01a21bc4a4a6757e94c85df9b3de8a00',1795762201832,x'01a21bc4a4a67ddd9b99ed1a1d7091c1',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,90,NULL,NULL,NULL,1795762201832,'user','You',1795762201919,1795762201954,NULL,NULL,NULL,2,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a4fc7d6c89c2a475d1e44d83',x'01a21bc4a4a67d59a25eb326f85a5034',2,'Buy seeds','buy seeds','',NULL,x'01a21bc4a4a6757e94c85df9b3de8a00',1795762201852,x'01a21bc4a4a67ddd9b99ed1a1d7091c1',0,x'01a21bc4a4e87f56a3c57d9197106bc2','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762201852,'user','You',1795762201852,1795762201852,NULL,NULL,NULL,1,'i');
INSERT INTO "tasks" VALUES(x'01a21bc4a50b7f54ae87466a888b82e8',x'01a21bc4a4a67d59a25eb326f85a5034',3,'Fix the fence','fix the fence','',NULL,x'01a21bc4a4a67ad18f4db2c6d09939ee',1795762201992,x'01a21bc4a4a67ddd9b99ed1a1d7091c1',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762201867,'user','You',1795762201992,1795762201992,1795762201992,NULL,NULL,4,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4a51b7169ad5089da3c0462cf',x'01a21bc4a4c679ebbefb1fe87552573e',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21bc4a51b7f4491df2daa80a25068',x'01a21bc4a4c67804b832e034a5d86082',1795762201883,x'01a21bc4a4c67b53a35fc8b5b1bd8aa7',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762201883,'user','You',1795762201883,1795762201883,NULL,NULL,NULL,1,NULL);
INSERT INTO "task_labels" VALUES(x'01a21bc4a4e87f56a3c57d9197106bc2',x'01a21bc4a4d67ef0acaffff4e0309f15');
INSERT INTO "task_relations" VALUES(x'01a21bc4a52c7393af84b8fb75de91ea',x'01a21bc4a50b7f54ae87466a888b82e8',x'01a21bc4a4e87f56a3c57d9197106bc2','blocks',NULL,1795762201900,'user','You');
INSERT INTO "comments" VALUES(x'01a21bc4a55072ee9b2ce5cd9543b5cc',x'01a21bc4a4e87f56a3c57d9197106bc2','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795762201936,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a4a67a39bb9329bc739705a6','project',x'01a21bc4a4a67d59a25eb326f85a5034','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795762201766,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a4e87beb9e8c1f6afd8b572a','task',x'01a21bc4a4e87f56a3c57d9197106bc2','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795762201832,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a51b7f4491df2daa80a25068','task',x'01a21bc4a51b7169ad5089da3c0462cf','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795762201883,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4a53f75ee8c444cf09e5b9d27','task',x'01a21bc4a4e87f56a3c57d9197106bc2','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4a4e87beb9e8c1f6afd8b572a','normal',NULL,'user','You',NULL,1795762201919,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4a56277d5b593a501e329a8ac',x'01a21bc4a4e87f56a3c57d9197106bc2','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795762201954,NULL,NULL,NULL,1);
INSERT INTO "timer" VALUES(1,x'01a21bc4a4fc7d6c89c2a475d1e44d83',1795762202008,'user','You',1795762202008,0,'',NULL);
INSERT INTO "activity" VALUES(1,x'01a21bc4a4a67610b8e4f2b5b245bc35',1795762201766,'project.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','project',x'01a21bc4a4a67d59a25eb326f85a5034','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21bc4a4c67117ace796cc514397ac',1795762201798,'project.create','user','You','cli',NULL,x'01a21bc4a4c679ebbefb1fe87552573e','project',x'01a21bc4a4c679ebbefb1fe87552573e','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21bc4a4d6761cb974505759825772',1795762201814,'label.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','label',x'01a21bc4a4d67ef0acaffff4e0309f15','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21bc4a4e87b3b9fb54a4182c13642',1795762201832,'task.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a4e87f56a3c57d9197106bc2','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21bc4-a4a6-757e-94c8-5df9b3de8a00"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21bc4a4fc72b5b8d67adaf59aa724',1795762201852,'task.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a4fc7d6c89c2a475d1e44d83','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21bc4-a4a6-757e-94c8-5df9b3de8a00"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21bc4a50b77f7be3419997882cc7c',1795762201867,'task.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a50b7f54ae87466a888b82e8','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21bc4-a4a6-757e-94c8-5df9b3de8a00"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21bc4a51b7dff99a190dbcb428234',1795762201883,'task.create','user','You','cli',NULL,x'01a21bc4a4c679ebbefb1fe87552573e','task',x'01a21bc4a51b7169ad5089da3c0462cf','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21bc4-a4c6-7804-b832-e034a5d86082"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21bc4a52c756dadcd5be1ca6dab46',1795762201900,'relation.add','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','relation',x'01a21bc4a52c7393af84b8fb75de91ea','{"fields":{"link":{"after":{"from":"01a21bc4-a50b-7f54-ae87-466a888b82e8","kind":"blocks","to":"01a21bc4-a4e8-7f56-a3c5-7d9197106bc2"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21bc4a53f7b8385cbd678a473be32',1795762201919,'task.update','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a4e87f56a3c57d9197106bc2','{"fields":{"description":{"after":{"revision":"01a21bc4-a53f-75ee-8c44-4cf09e5b9d27"},"before":{"revision":"01a21bc4-a4e8-7beb-9e8c-1f6afd8b572a"}}}}','{"set":{"description":{"revision":"01a21bc4-a4e8-7beb-9e8c-1f6afd8b572a"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21bc4a55070c9b629af7114b1487d',1795762201936,'comment.add','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','comment',x'01a21bc4a55072ee9b2ce5cd9543b5cc','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21bc4a5627343838f2cd163651cd2',1795762201954,'time.log','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','time_entry',x'01a21bc4a56277d5b593a501e329a8ac','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21bc4a5757029bc3dfeb31e387030',1795762201973,'reminder.set','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a4e87f56a3c57d9197106bc2','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21bc4a588724c9d470210e448358f',1795762201992,'task.move','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','task',x'01a21bc4a50b7f54ae87466a888b82e8','{"fields":{"closed_at":{"after":1795762201992},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21bc4-a4a6-7ad1-8f4d-b2c6d09939ee","before":"01a21bc4-a4a6-757e-94c8-5df9b3de8a00"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21bc4-a4a6-757e-94c8-5df9b3de8a00"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21bc4a5987bd490534be15e801926',1795762202008,'timer.start','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','timer',x'01a21bc4a48a7248afa491d866c543aa','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795762202008,"started_at":1795762202008,"task":"01a21bc4-a4fc-7d6c-89c2-a475d1e44d83"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21bc4a5a9786d8f5356c2f07ee712',1795762202025,'wiki.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','wiki_page',x'01a21bc4a5a97746883506b77c905b8f','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21bc4a5bb775eaabe931c5e053e6c',1795762202043,'wiki.create','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','wiki_page',x'01a21bc4a5bb7576bf41683a27453d15','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21bc4a5ce743382d326e631619701',1795762202062,'wiki.update','user','You','cli',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','wiki_page',x'01a21bc4a5bb7576bf41683a27453d15','{"fields":{"body":{"after":{"revision":"01a21bc4-a5ce-7c9b-8452-19d14174162f"},"before":{"revision":"01a21bc4-a5bb-7ec6-bcd0-e3967ce9afa8"}}}}','{"set":{"body":{"revision":"01a21bc4-a5bb-7ec6-bcd0-e3967ce9afa8"}}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21bc4a4e87f56a3c57d9197106bc2',x'01a21bc4a4e87f56a3c57d9197106bc2',x'01a21bc4a4a67d59a25eb326f85a5034','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795762201919);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21bc4a4fc7d6c89c2a475d1e44d83',x'01a21bc4a4fc7d6c89c2a475d1e44d83',x'01a21bc4a4a67d59a25eb326f85a5034','Buy seeds','',1795762201852);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21bc4a50b7f54ae87466a888b82e8',x'01a21bc4a50b7f54ae87466a888b82e8',x'01a21bc4a4a67d59a25eb326f85a5034','Fix the fence','',1795762201867);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21bc4a51b7169ad5089da3c0462cf',x'01a21bc4a51b7169ad5089da3c0462cf',x'01a21bc4a4c679ebbefb1fe87552573e','Launch the homepage','Ship it soon.
',1795762201883);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21bc4a55072ee9b2ce5cd9543b5cc',x'01a21bc4a4e87f56a3c57d9197106bc2',x'01a21bc4a4a67d59a25eb326f85a5034','Plan the garden','The neighbours like the garden plan.
',1795762201936);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21bc4a5a97746883506b77c905b8f',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795762202025);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21bc4a5bb7576bf41683a27453d15',NULL,x'01a21bc4a4a67d59a25eb326f85a5034','Seeds','Tomatoes, beans and peas.
',1795762202062);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a5a97746883506b77c905b8f',x'01a21bc4a4a67d59a25eb326f85a5034',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21bc4a5a971879762357e29d68d47',0,1795762202025,1795762202025,NULL,NULL);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4a5bb7576bf41683a27453d15',x'01a21bc4a4a67d59a25eb326f85a5034',x'01a21bc4a5a97746883506b77c905b8f','i','Seeds','seeds','seeds',x'01a21bc4a5ce7c9b845219d14174162f',0,1795762202043,1795762202062,NULL,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a5a971879762357e29d68d47',x'01a21bc4a5a97746883506b77c905b8f','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795762202025,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a5bb7ec6bcd0e3967ce9afa8',x'01a21bc4a5bb7576bf41683a27453d15','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795762202043,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4a5ce7c9b845219d14174162f',x'01a21bc4a5bb7576bf41683a27453d15','Tomatoes, beans and peas.',x'01a21bc4a5bb7ec6bcd0e3967ce9afa8','normal',NULL,'user','You',NULL,1795762202062,NULL);
INSERT INTO "reminders" VALUES(x'01a21bc4a5757a5fa66e3e00b63b25cb',x'01a21bc4a4e87f56a3c57d9197106bc2',1796029200000,NULL,NULL,'','user','You',1795762201973,NULL,NULL);
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
PRAGMA user_version = 6;
COMMIT;
