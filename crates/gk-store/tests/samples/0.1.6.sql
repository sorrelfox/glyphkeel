-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.6.
-- release: 0.1.6
-- schema: 16
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
, agent_rules TEXT
    CHECK (agent_rules IS NULL OR (json_valid(agent_rules) AND json_type(agent_rules) = 'object'))) STRICT;
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
  wip_limit  INTEGER CHECK (wip_limit IS NULL OR wip_limit > 0), rules TEXT
    CHECK (rules IS NULL OR (json_valid(rules) AND json_type(rules) = 'object')), wip_min INTEGER CHECK (wip_min IS NULL OR wip_min > 0),
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
  CHECK (subtask_rank IS NULL OR (subtask_rank <> '' AND subtask_rank NOT GLOB '*[^0-9a-z]*')), fix_version_id BLOB CHECK (fix_version_id IS NULL
                                                     OR length(fix_version_id) = 16), series_id BLOB
    CHECK (series_id IS NULL OR length(series_id) = 16),
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
, work_kind_id BLOB
    CHECK (work_kind_id IS NULL OR length(work_kind_id) = 16)) STRICT;
CREATE TABLE timer (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  task_id    BLOB NOT NULL REFERENCES tasks (id),
  started_at INTEGER NOT NULL,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent')),
  actor_name TEXT NOT NULL
, running_since INTEGER, elapsed_ms INTEGER NOT NULL DEFAULT 0 CHECK (elapsed_ms >= 0), note TEXT NOT NULL DEFAULT '', reminded_at INTEGER, work_kind_id BLOB
    CHECK (work_kind_id IS NULL OR length(work_kind_id) = 16)) STRICT;
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
CREATE TABLE custom_values (
  task_id  BLOB NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  field_id BLOB NOT NULL REFERENCES custom_fields (id) ON DELETE CASCADE,
  value    ANY NOT NULL CHECK (typeof(value) IN ('text', 'integer', 'real')),
  PRIMARY KEY (task_id, field_id, value)
) STRICT, WITHOUT ROWID;
CREATE TABLE versions (
  id          BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id  BLOB NOT NULL REFERENCES projects (id),
  name        TEXT NOT NULL CHECK (name <> '' AND length(name) <= 100),
  name_fold   TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4000),
  status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'locked', 'closed')),
  start_date  TEXT CHECK (start_date IS NULL
                          OR start_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  due_date    TEXT CHECK (due_date IS NULL
                          OR due_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  sharing     TEXT NOT NULL DEFAULT 'none' CHECK (sharing IN ('none', 'descendants')),
  rank        TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  UNIQUE (project_id, name_fold),
  CHECK (start_date IS NULL OR due_date IS NULL OR start_date <= due_date)
) STRICT;
CREATE TABLE task_series (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB NOT NULL REFERENCES projects (id),
  rule            TEXT NOT NULL CHECK (json_valid(rule)),
  pattern         TEXT NOT NULL CHECK (json_valid(pattern)),
  current_task    BLOB CHECK (current_task IS NULL OR length(current_task) = 16),
  next_date       TEXT CHECK (next_date IS NULL
                              OR next_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  made            INTEGER NOT NULL DEFAULT 1 CHECK (made >= 0),
  ended_at        INTEGER,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE TABLE templates (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB REFERENCES projects (id),
  kind            TEXT NOT NULL CHECK (kind IN ('task', 'project')),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 1000),
  content         TEXT NOT NULL CHECK (json_valid(content)
                                       AND octet_length(content) <= 4194304),
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL,
  CHECK ((kind = 'task') = (project_id IS NOT NULL))
) STRICT;
CREATE TABLE work_kinds (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
  name_fold       TEXT NOT NULL UNIQUE,
  archived_at     INTEGER,
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE TABLE task_types (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB NOT NULL REFERENCES projects (id),
  name            TEXT NOT NULL CHECK (name <> ''),
  name_fold       TEXT NOT NULL,
  glyph           TEXT NOT NULL
                  CHECK (glyph IN ('task', 'bug', 'feature', 'milestone', 'epic', 'story')),
  hue             TEXT NOT NULL,
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  is_default      INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
  due_offset_days INTEGER CHECK (due_offset_days IS NULL OR due_offset_days BETWEEN 0 AND 3650),
  UNIQUE (project_id, name_fold)
) STRICT;
INSERT INTO "meta" VALUES('workspace_id','01a21bc4-aa95-71d9-b940-b8024b1e2bd1');
INSERT INTO "meta" VALUES('created_at','1795762203285');
INSERT INTO "meta" VALUES('created_by_version','0.1.6');
INSERT INTO "projects" VALUES(x'01a21bc4aad27a599ef94a6d02f73bc4',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21bc4aad2772883fdaefdb538c988','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',5,1795762203346,1795762203881,NULL,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21bc4aaf578c389358350df13aa11',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795762203381,1795762203476,NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aad27f06ad6a95ca5bc0f7b7',x'01a21bc4aad27a599ef94a6d02f73bc4','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aad272bca97ec3e1cbee3242',x'01a21bc4aad27a599ef94a6d02f73bc4','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aad278088136589363c013b7',x'01a21bc4aad27a599ef94a6d02f73bc4','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aad27ad2896d533097c352b3',x'01a21bc4aad27a599ef94a6d02f73bc4','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aad27727b698905ce3873d72',x'01a21bc4aad27a599ef94a6d02f73bc4','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aaf5772fa846d6c3a69f4abe',x'01a21bc4aaf578c389358350df13aa11','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aaf57b33bebdd27f80b5a080',x'01a21bc4aaf578c389358350df13aa11','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aaf57a9ab0ca65ed85f09af7',x'01a21bc4aaf578c389358350df13aa11','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aaf57455a4bf562b7ae64e9c',x'01a21bc4aaf578c389358350df13aa11','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21bc4aaf5709990cfa0b35eeaeb17',x'01a21bc4aaf578c389358350df13aa11','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "labels" VALUES(x'01a21bc4ab067079b7026de82f99b21d',x'01a21bc4aad27a599ef94a6d02f73bc4','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4aad27a599ef94a6d02f73bc4',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4ab7b7bc6a4e4ea6959caffdd',x'01a21bc4aad27f06ad6a95ca5bc0f7b7',1795762203417,x'01a21bc4aad27f35a63985e2b0304939',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,135,NULL,NULL,NULL,1795762203417,'user','You',1795762203803,1795762203846,NULL,NULL,NULL,4,NULL,x'01a21bc4ac617c3e964d4c0e9d7a670b',NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ab2f7166a09725ea88a1b589',x'01a21bc4aad27a599ef94a6d02f73bc4',2,'Buy seeds','buy seeds','',NULL,x'01a21bc4aad27f06ad6a95ca5bc0f7b7',1795762203439,x'01a21bc4aad27f35a63985e2b0304939',0,x'01a21bc4ab197bceaf66488cf089d85c','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762203439,'user','You',1795762203439,1795762203439,NULL,NULL,NULL,1,'i',NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ab417115a31203143c005a79',x'01a21bc4aad27a599ef94a6d02f73bc4',3,'Fix the fence','fix the fence','',NULL,x'01a21bc4aad27727b698905ce3873d72',1795762203595,x'01a21bc4aad27f35a63985e2b0304939',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762203457,'user','You',1795762203595,1795762203595,1795762203595,NULL,NULL,4,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ab5473e5a504c116b812b47f',x'01a21bc4aaf578c389358350df13aa11',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21bc4ab547e67979cce686a3519ba',x'01a21bc4aaf5772fa846d6c3a69f4abe',1795762203476,x'01a21bc4aaf57213b3e3c6c36bf62247',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795762203476,'user','You',1795762203476,1795762203476,NULL,NULL,NULL,1,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21bc4ace97432bb596cff403ff69f',x'01a21bc4aad27a599ef94a6d02f73bc4',4,'Water the plants','water the plants','',NULL,x'01a21bc4aad27f06ad6a95ca5bc0f7b7',1795762203881,x'01a21bc4aad27f35a63985e2b0304939',0,NULL,'i002',NULL,'2026-12-02',NULL,NULL,0,0,0,NULL,NULL,NULL,1795762203881,'user','You',1795762203899,1795762203899,NULL,NULL,NULL,2,NULL,NULL,x'01a21bc4acfb7109a2439a02268a7266');
INSERT INTO "task_labels" VALUES(x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4ab067079b7026de82f99b21d');
INSERT INTO "task_relations" VALUES(x'01a21bc4ab67716aac6d8ec1a36cfd27',x'01a21bc4ab417115a31203143c005a79',x'01a21bc4ab197bceaf66488cf089d85c','blocks',NULL,1795762203495,'user','You');
INSERT INTO "comments" VALUES(x'01a21bc4ab9074e78fea38c7104a57f5',x'01a21bc4ab197bceaf66488cf089d85c','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795762203536,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21bc4aad2772883fdaefdb538c988','project',x'01a21bc4aad27a599ef94a6d02f73bc4','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795762203346,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ab1972e6a27c60b1644f182e','task',x'01a21bc4ab197bceaf66488cf089d85c','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795762203417,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ab547e67979cce686a3519ba','task',x'01a21bc4ab5473e5a504c116b812b47f','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795762203476,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21bc4ab7b7bc6a4e4ea6959caffdd','task',x'01a21bc4ab197bceaf66488cf089d85c','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21bc4ab1972e6a27c60b1644f182e','normal',NULL,'user','You',NULL,1795762203515,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4aba4746a9d0c37d8c86a420c',x'01a21bc4ab197bceaf66488cf089d85c','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795762203556,NULL,NULL,NULL,1,NULL);
INSERT INTO "time_entries" VALUES(x'01a21bc4acc677ebbd39c2ca030721e6',x'01a21bc4ab197bceaf66488cf089d85c','2026-11-21',45,'',NULL,'user','You',NULL,1795762203846,NULL,NULL,NULL,1,x'01a21bc4acb172188740ed2fb432fcf1');
INSERT INTO "timer" VALUES(1,x'01a21bc4ab2f7166a09725ea88a1b589',1795762203613,'user','You',1795762203613,0,'',NULL,NULL);
INSERT INTO "activity" VALUES(1,x'01a21bc4aad275f9bbcc8360c2ca38f2',1795762203346,'project.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','project',x'01a21bc4aad27a599ef94a6d02f73bc4','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21bc4aaf578678ade55adfbb273fa',1795762203381,'project.create','user','You','cli',NULL,x'01a21bc4aaf578c389358350df13aa11','project',x'01a21bc4aaf578c389358350df13aa11','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21bc4ab067071925d2668d81afa97',1795762203398,'label.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','label',x'01a21bc4ab067079b7026de82f99b21d','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21bc4ab197b65bc69f3f0025829ea',1795762203417,'task.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab197bceaf66488cf089d85c','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21bc4ab2f70c089689c0bd2819a0c',1795762203439,'task.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab2f7166a09725ea88a1b589','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21bc4ab417dfdaefa5be57e65089a',1795762203457,'task.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab417115a31203143c005a79','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21bc4ab5477dd9970c782dbacce05',1795762203476,'task.create','user','You','cli',NULL,x'01a21bc4aaf578c389358350df13aa11','task',x'01a21bc4ab5473e5a504c116b812b47f','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21bc4-aaf5-772f-a846-d6c3a69f4abe"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21bc4ab677a6281cef6e7b9135f48',1795762203495,'relation.add','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','relation',x'01a21bc4ab67716aac6d8ec1a36cfd27','{"fields":{"link":{"after":{"from":"01a21bc4-ab41-7115-a312-03143c005a79","kind":"blocks","to":"01a21bc4-ab19-7bce-af66-488cf089d85c"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21bc4ab7b7939b18d369b149812f7',1795762203515,'task.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab197bceaf66488cf089d85c','{"fields":{"description":{"after":{"revision":"01a21bc4-ab7b-7bc6-a4e4-ea6959caffdd"},"before":{"revision":"01a21bc4-ab19-72e6-a27c-60b1644f182e"}}}}','{"set":{"description":{"revision":"01a21bc4-ab19-72e6-a27c-60b1644f182e"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21bc4ab907d4e9aaa26e322672490',1795762203536,'comment.add','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','comment',x'01a21bc4ab9074e78fea38c7104a57f5','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21bc4aba47e7f8f722ee981e482fa',1795762203556,'time.log','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','time_entry',x'01a21bc4aba4746a9d0c37d8c86a420c','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21bc4abb87b2e8d109c4c426ddd29',1795762203576,'reminder.set','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab197bceaf66488cf089d85c','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21bc4abcb72d981daa9d66bbeb4f6',1795762203595,'task.move','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab417115a31203143c005a79','{"fields":{"closed_at":{"after":1795762203595},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21bc4-aad2-7727-b698-905ce3873d72","before":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21bc4abdd765fbfb570d253a3d7a7',1795762203613,'timer.start','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','timer',x'01a21bc4aa9571d9b940b8024b1e2bd1','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795762203613,"started_at":1795762203613,"task":"01a21bc4-ab2f-7166-a097-25ea88a1b589"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21bc4abef73208a862b292e7ea303',1795762203631,'wiki.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4abef75f2b5b7ab5da67db44b','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21bc4ac037d40858b12bd3f270c1b',1795762203651,'wiki.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4ac037806becac07bb0b3ff51','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21bc4ac167daa80c17e8400d75389',1795762203670,'wiki.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4ac037806becac07bb0b3ff51','{"fields":{"body":{"after":{"revision":"01a21bc4-ac16-7137-8f14-924ee3108571"},"before":{"revision":"01a21bc4-ac03-79df-b7b3-9e75870dee8d"}}}}','{"set":{"body":{"revision":"01a21bc4-ac03-79df-b7b3-9e75870dee8d"}}}',NULL);
INSERT INTO "activity" VALUES(18,x'01a21bc4ac297521bff21483fb065f63',1795762203689,'wiki.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4abef75f2b5b7ab5da67db44b','{"fields":{"home":{"after":true,"before":false}}}','{"set":{"home":false}}',NULL);
INSERT INTO "activity" VALUES(19,x'01a21bc4ac3c79ceb131bda4c831ca0d',1795762203708,'wiki.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4ac3c71cbad82343406c005f3','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(20,x'01a21bc4ac4f73348c1185e39e927750',1795762203727,'wiki.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','wiki_page',x'01a21bc4ac3c71cbad82343406c005f3','{"fields":{"template":{"after":true,"before":false}}}','{"set":{"template":false}}',NULL);
INSERT INTO "activity" VALUES(21,x'01a21bc4ac61790abed69f9254227300',1795762203745,'version.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','version',x'01a21bc4ac617c3e964d4c0e9d7a670b','{"fields":{"record":{"after":{"created_at":1795762203745,"description":"","due_date":"2027-03-01","name":"Spring","project":"01a21bc4-aad2-7a59-9ef9-4a6d02f73bc4","rank":"i","sharing":"none","status":"open","updated_at":1795762203745}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(22,x'01a21bc4ac757c239bb6ca7f0bba1411',1795762203765,'task.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab197bceaf66488cf089d85c','{"fields":{"fix_version":{"after":"01a21bc4-ac61-7c3e-964d-4c0e9d7a670b"}}}','{"set":{"fix_version":null}}',NULL);
INSERT INTO "activity" VALUES(23,x'01a21bc4ac8777a286a0b212b2f208ee',1795762203783,'field.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','custom_field',x'01a21bc4ac8774a699a1e0548715d0a5','{"fields":{"created":{"after":true},"name":{"after":"Cost"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(24,x'01a21bc4ac9b7445a5e9a24852e3f02e',1795762203803,'task.update','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ab197bceaf66488cf089d85c','{"fields":{"cf:01a21bc4-ac87-74a6-99a1-e0548715d0a5":{"after":12.5}}}','{"set":{"cf:01a21bc4-ac87-74a6-99a1-e0548715d0a5":null}}',NULL);
INSERT INTO "activity" VALUES(25,x'01a21bc4acb17e36a4f2129a5aecdf08',1795762203825,'work_kind.create','user','You','cli',NULL,NULL,'work_kind',x'01a21bc4acb172188740ed2fb432fcf1','{"fields":{"record":{"after":{"created_at":1795762203825,"created_by_kind":"user","created_by_name":"You","name":"Digging","updated_at":1795762203825}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(26,x'01a21bc4acc67bb28c84503c7e5d09d2',1795762203846,'time.log','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','time_entry',x'01a21bc4acc677ebbd39c2ca030721e6','{"fields":{"created":{"after":true},"date":{"after":"2026-11-21"},"minutes":{"after":45},"task":{"after":"TM-1"},"work_kind":{"after":"01a21bc4-acb1-7218-8740-ed2fb432fcf1"}}}','{"set":{"created":null,"date":null,"minutes":null,"task":null,"work_kind":null}}',NULL);
INSERT INTO "activity" VALUES(27,x'01a21bc4acd7760797de315fbdfb7c7c',1795762203863,'type.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task_type',x'01a21bc4acd77513abc3b2be43faace8','{"fields":{"created":{"after":true},"name":{"after":"Epic"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(28,x'01a21bc4ace974448b4545b2fc863898',1795762203881,'task.create','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ace97432bb596cff403ff69f','{"fields":{"created":{"after":true},"key":{"after":"TM-4"},"status":{"after":"01a21bc4-aad2-7f06-ad6a-95ca5bc0f7b7"},"title":{"after":"Water the plants"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(29,x'01a21bc4acfb7d6f82040a9af3b37c7f',1795762203899,'recurrence.set','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','series',x'01a21bc4acfb7109a2439a02268a7266','{"fields":{"record":{"after":{"created_at":1795762203899,"created_by_kind":"user","created_by_name":"You","current":"01a21bc4-ace9-7432-bb59-6cff403ff69f","made":1,"pattern":{"due_in_days":0,"title":"Water the plants","type":"Task"},"project":"01a21bc4-aad2-7a59-9ef9-4a6d02f73bc4","rule":{"create":"on_close","every":"week","from":"schedule","interval":1,"starts":"2026-12-02","weekdays":["wed"]},"updated_at":1795762203899}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(30,x'01a21bc4acfb7d6f82040a9af3b37c7f',1795762203899,'recurrence.set','user','You','cli',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','task',x'01a21bc4ace97432bb596cff403ff69f','{"fields":{"series":{"after":"01a21bc4-acfb-7109-a243-9a02268a7266"}}}','{"set":{"series":null}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4aad27a599ef94a6d02f73bc4','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795762203515);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21bc4ab2f7166a09725ea88a1b589',x'01a21bc4ab2f7166a09725ea88a1b589',x'01a21bc4aad27a599ef94a6d02f73bc4','Buy seeds','',1795762203439);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21bc4ab417115a31203143c005a79',x'01a21bc4ab417115a31203143c005a79',x'01a21bc4aad27a599ef94a6d02f73bc4','Fix the fence','',1795762203457);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21bc4ab5473e5a504c116b812b47f',x'01a21bc4ab5473e5a504c116b812b47f',x'01a21bc4aaf578c389358350df13aa11','Launch the homepage','Ship it soon.
',1795762203476);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21bc4ab9074e78fea38c7104a57f5',x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4aad27a599ef94a6d02f73bc4','Plan the garden','The neighbours like the garden plan.
',1795762203536);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21bc4abef75f2b5b7ab5da67db44b',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795762203631);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21bc4ac037806becac07bb0b3ff51',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','Seeds','Tomatoes, beans and peas.
',1795762203670);
INSERT INTO "search_docs" VALUES(8,'wiki',x'01a21bc4ac3c71cbad82343406c005f3',NULL,x'01a21bc4aad27a599ef94a6d02f73bc4','Meeting notes','Who
What
',1795762203708);
INSERT INTO "search_docs" VALUES(9,'task',x'01a21bc4ace97432bb596cff403ff69f',x'01a21bc4ace97432bb596cff403ff69f',x'01a21bc4aad27a599ef94a6d02f73bc4','Water the plants','',1795762203881);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4abef75f2b5b7ab5da67db44b',x'01a21bc4aad27a599ef94a6d02f73bc4',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21bc4abef77b0b3f157dddb527217',0,1795762203631,1795762203689,NULL,NULL,1,0);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4ac037806becac07bb0b3ff51',x'01a21bc4aad27a599ef94a6d02f73bc4',x'01a21bc4abef75f2b5b7ab5da67db44b','i','Seeds','seeds','seeds',x'01a21bc4ac1671378f14924ee3108571',0,1795762203651,1795762203670,NULL,NULL,0,0);
INSERT INTO "wiki_pages" VALUES(x'01a21bc4ac3c71cbad82343406c005f3',x'01a21bc4aad27a599ef94a6d02f73bc4',NULL,'i001','Meeting notes','meeting notes','meeting-notes',x'01a21bc4ac3c776eb564641bf3f84c25',0,1795762203708,1795762203727,NULL,NULL,0,1);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4abef77b0b3f157dddb527217',x'01a21bc4abef75f2b5b7ab5da67db44b','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795762203631,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4ac0379dfb7b39e75870dee8d',x'01a21bc4ac037806becac07bb0b3ff51','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795762203651,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4ac1671378f14924ee3108571',x'01a21bc4ac037806becac07bb0b3ff51','Tomatoes, beans and peas.',x'01a21bc4ac0379dfb7b39e75870dee8d','normal',NULL,'user','You',NULL,1795762203670,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21bc4ac3c776eb564641bf3f84c25',x'01a21bc4ac3c71cbad82343406c005f3','## Who

## What',NULL,'normal',NULL,'user','You',NULL,1795762203708,NULL);
INSERT INTO "reminders" VALUES(x'01a21bc4abb87124ae770d3f09be6ce9',x'01a21bc4ab197bceaf66488cf089d85c',1796029200000,NULL,NULL,'','user','You',1795762203576,NULL,NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21bc4abef75f2b5b7ab5da67db44b',0,'wiki','Seeds',NULL,'seeds','seeds','','',NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21bc4abef75f2b5b7ab5da67db44b',1,'task','TM-1',NULL,'','','','',1);
INSERT INTO "custom_fields" VALUES(x'01a21bc4ac8774a699a1e0548715d0a5',x'01a21bc4aad27a599ef94a6d02f73bc4','Cost','cost','cost','decimal','[]',NULL,'[]',0,'i',1795762203783);
INSERT INTO "custom_values" VALUES(x'01a21bc4ab197bceaf66488cf089d85c',x'01a21bc4ac8774a699a1e0548715d0a5',12.5);
INSERT INTO "versions" VALUES(x'01a21bc4ac617c3e964d4c0e9d7a670b',x'01a21bc4aad27a599ef94a6d02f73bc4','Spring','spring','','open',NULL,'2027-03-01','none','i',1795762203745,1795762203745);
INSERT INTO "task_series" VALUES(x'01a21bc4acfb7109a2439a02268a7266',x'01a21bc4aad27a599ef94a6d02f73bc4','{"every":"week","interval":1,"weekdays":["wed"],"starts":"2026-12-02","from":"schedule","create":"on_close"}','{"title":"Water the plants","type":"Task","due_in_days":0}',x'01a21bc4ace97432bb596cff403ff69f',NULL,1,NULL,1795762203899,'user','You',1795762203899);
INSERT INTO "work_kinds" VALUES(x'01a21bc4acb172188740ed2fb432fcf1','Digging','digging',NULL,1795762203825,'user','You',1795762203825);
INSERT INTO "task_types" VALUES(x'01a21bc4aad27f35a63985e2b0304939',x'01a21bc4aad27a599ef94a6d02f73bc4','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aad27496ae96f99745149c3d',x'01a21bc4aad27a599ef94a6d02f73bc4','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aad27011ad857673eb6f4992',x'01a21bc4aad27a599ef94a6d02f73bc4','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aad27410992db24de532214c',x'01a21bc4aad27a599ef94a6d02f73bc4','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aaf57213b3e3c6c36bf62247',x'01a21bc4aaf578c389358350df13aa11','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aaf573ee8592faeb641623b6',x'01a21bc4aaf578c389358350df13aa11','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aaf57091a52198a06a0d8686',x'01a21bc4aaf578c389358350df13aa11','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4aaf578668f016e9b9f7fab6b',x'01a21bc4aaf578c389358350df13aa11','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21bc4acd77513abc3b2be43faace8',x'01a21bc4aad27a599ef94a6d02f73bc4','Epic','epic','epic','grey','ssst',0,NULL);
CREATE INDEX projects_parent ON projects (parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX project_key_aliases_project ON project_key_aliases (project_id);
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
CREATE UNIQUE INDEX custom_fields_name ON custom_fields (ifnull(project_id, x''), name_fold);
CREATE INDEX custom_fields_project ON custom_fields (project_id);
CREATE INDEX custom_fields_query_name ON custom_fields (query_name);
CREATE INDEX custom_values_field ON custom_values (field_id, value);
CREATE INDEX tasks_fix_version ON tasks (fix_version_id) WHERE fix_version_id IS NOT NULL;
CREATE INDEX task_series_project ON task_series (project_id);
CREATE INDEX task_series_due ON task_series (next_date)
    WHERE ended_at IS NULL AND next_date IS NOT NULL;
CREATE INDEX tasks_series ON tasks (series_id) WHERE series_id IS NOT NULL;
CREATE UNIQUE INDEX templates_name ON templates (kind, coalesce(project_id, x''), name_fold);
CREATE INDEX time_entries_work_kind ON time_entries (work_kind_id)
    WHERE work_kind_id IS NOT NULL;
CREATE UNIQUE INDEX task_types_one_default ON task_types (project_id) WHERE is_default = 1;
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 16;
COMMIT;
