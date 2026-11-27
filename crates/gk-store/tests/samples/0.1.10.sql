-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.10.
-- release: 0.1.10
-- schema: 22
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
, rate_limited INTEGER NOT NULL DEFAULT 0) STRICT;
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
    CHECK (agent_rules IS NULL OR (json_valid(agent_rules) AND json_type(agent_rules) = 'object')), folder_id BLOB REFERENCES project_folders (id)) STRICT;
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
  created_by_kind   TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
  created_by_name   TEXT NOT NULL,
  edited_at         INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  closed_at         INTEGER,
  deleted_at        INTEGER,
  deleted_group     BLOB,
  version           INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1), subtask_rank TEXT
  CHECK (subtask_rank IS NULL OR (subtask_rank <> '' AND subtask_rank NOT GLOB '*[^0-9a-z]*')), fix_version_id BLOB CHECK (fix_version_id IS NULL
                                                     OR length(fix_version_id) = 16), series_id BLOB
    CHECK (series_id IS NULL OR length(series_id) = 16), cover_id BLOB CHECK (cover_id IS NULL OR length(cover_id) = 16), sprint_id BLOB CHECK (sprint_id IS NULL OR length(sprint_id) = 16),
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
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
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
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind  TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind  TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  actor_kind      TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
  actor_name      TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  acked_at        INTEGER,
  acked_moment    INTEGER,
  CHECK ((remind_at IS NULL) <> (days_before_due IS NULL)),
  CHECK (time_of_day IS NULL OR days_before_due IS NOT NULL),
  CHECK ((acked_at IS NULL) = (acked_moment IS NULL))
) STRICT;
CREATE TABLE "drafts" (
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'automation')),
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
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
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
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
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
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
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
CREATE TABLE sprints (
  id           BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id   BLOB NOT NULL REFERENCES projects (id),
  name         TEXT NOT NULL CHECK (name <> '' AND length(name) <= 100),
  name_fold    TEXT NOT NULL,
  goal         TEXT NOT NULL DEFAULT '' CHECK (length(goal) <= 4000),
  state        TEXT NOT NULL DEFAULT 'planned' CHECK (state IN ('planned', 'active', 'closed')),
  start_date   TEXT CHECK (start_date IS NULL
                           OR start_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  end_date     TEXT CHECK (end_date IS NULL
                           OR end_date GLOB '[0-9][0-9][0-9][0-9]-[01][0-9]-[0-3][0-9]'),
  started_at   INTEGER,
  completed_at INTEGER,
  rank         TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  UNIQUE (project_id, name_fold),
  CHECK (start_date IS NULL OR end_date IS NULL OR start_date <= end_date)
) STRICT;
CREATE TABLE automation_rules (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  project_id      BLOB REFERENCES projects (id),
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  name_fold       TEXT NOT NULL,
  enabled         INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  trigger_kind    TEXT NOT NULL CHECK (trigger_kind IN ('task_created', 'field_changed',
                    'moved_to_status', 'comment_added', 'due_date_reached', 'overdue',
                    'schedule')),
  definition      TEXT NOT NULL CHECK (json_valid(definition)
                                       AND octet_length(definition) <= 262144),
  rank            TEXT NOT NULL CHECK (rank <> '' AND rank NOT GLOB '*[^0-9a-z]*'),
  created_at      INTEGER NOT NULL,
  created_by_kind TEXT NOT NULL CHECK (created_by_kind IN ('user', 'agent', 'automation')),
  created_by_name TEXT NOT NULL,
  updated_at      INTEGER NOT NULL,
  updated_by_kind TEXT NOT NULL CHECK (updated_by_kind IN ('user', 'agent', 'automation')),
  scheduled_at    INTEGER,
  window_start    INTEGER,
  window_runs     INTEGER NOT NULL DEFAULT 0 CHECK (window_runs >= 0)
) STRICT;
CREATE TABLE automation_runs (
  id           BLOB PRIMARY KEY CHECK (length(id) = 16),
  rule_id      BLOB NOT NULL CHECK (length(rule_id) = 16),
  rule_name    TEXT NOT NULL,
  at           INTEGER NOT NULL,
  result       TEXT NOT NULL CHECK (result IN ('done', 'failed', 'limited')),
  trigger_kind TEXT NOT NULL,
  task_id      BLOB CHECK (task_id IS NULL OR length(task_id) = 16),
  group_id     BLOB CHECK (group_id IS NULL OR length(group_id) = 16),
  changes      INTEGER NOT NULL DEFAULT 0 CHECK (changes >= 0),
  depth        INTEGER NOT NULL DEFAULT 0 CHECK (depth >= 0),
  error        TEXT CHECK (error IS NULL OR json_valid(error))
) STRICT;
CREATE TABLE automation_fired (
  rule_id  BLOB NOT NULL CHECK (length(rule_id) = 16),
  task_id  BLOB NOT NULL CHECK (length(task_id) = 16),
  mark     TEXT NOT NULL,
  fired_at INTEGER NOT NULL,
  PRIMARY KEY (rule_id, task_id, mark)
) STRICT, WITHOUT ROWID;
CREATE TABLE automation_notices (
  id       BLOB PRIMARY KEY CHECK (length(id) = 16),
  rule_id  BLOB NOT NULL CHECK (length(rule_id) = 16),
  task_id  BLOB NOT NULL CHECK (length(task_id) = 16),
  at       INTEGER NOT NULL,
  text     TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 2000),
  acked_at INTEGER
) STRICT;
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
INSERT INTO "meta" VALUES('workspace_id','01a21d6f-727e-7715-b929-d4ab7617a69b');
INSERT INTO "meta" VALUES('created_at','1795790172798');
INSERT INTO "meta" VALUES('created_by_version','0.1.10');
INSERT INTO "projects" VALUES(x'01a21d6f72cb7718897aaa00d0a4d932',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21d6f72cb7726a1932930c3780557','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',5,1795790172875,1795790174304,NULL,NULL,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21d6f730d714db8051f74aa304511',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795790172941,1795790173206,NULL,NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f72cb7d41bb968e5c797c40cf',x'01a21d6f72cb7718897aaa00d0a4d932','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f72cb7cf2a1e8e6668fe56996',x'01a21d6f72cb7718897aaa00d0a4d932','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f72cb745391cefaac285bd305',x'01a21d6f72cb7718897aaa00d0a4d932','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f72cb7d0a9effe447d72c1679',x'01a21d6f72cb7718897aaa00d0a4d932','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f72cb784e8e665562a9105807',x'01a21d6f72cb7718897aaa00d0a4d932','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f730d75ba8a8369f91280692d',x'01a21d6f730d714db8051f74aa304511','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f730d757489de26342955fd24',x'01a21d6f730d714db8051f74aa304511','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f730d7ebf9554b5bd999f45aa',x'01a21d6f730d714db8051f74aa304511','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f730d78b5b692285a5360fc47',x'01a21d6f730d714db8051f74aa304511','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21d6f730d7102a4ac81113342b807',x'01a21d6f730d714db8051f74aa304511','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "labels" VALUES(x'01a21d6f734273e1acbb8d1fbff14fe2',x'01a21d6f72cb7718897aaa00d0a4d932','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f72cb7718897aaa00d0a4d932',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21d6f747e70708e7d1969a657c86d',x'01a21d6f72cb7d41bb968e5c797c40cf',1795790173049,x'01a21d6f72cb705c97e1bc22f42d9655',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,135,NULL,NULL,NULL,1795790173049,'user','You',1795790174094,1795790174201,NULL,NULL,NULL,4,NULL,x'01a21d6f76f2739d9ddbe540c8cac44e',NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21d6f73ae7847b70f3f8f8ea989ad',x'01a21d6f72cb7718897aaa00d0a4d932',2,'Buy seeds','buy seeds','',NULL,x'01a21d6f72cb7d41bb968e5c797c40cf',1795790173102,x'01a21d6f72cb705c97e1bc22f42d9655',0,x'01a21d6f737979e8a17b6ac8781c6492','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795790173102,'user','You',1795790173102,1795790173102,NULL,NULL,NULL,1,'i',NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21d6f73e07414bf8760d4c82e3eeb',x'01a21d6f72cb7718897aaa00d0a4d932',3,'Fix the fence','fix the fence','',NULL,x'01a21d6f72cb784e8e665562a9105807',1795790173522,x'01a21d6f72cb705c97e1bc22f42d9655',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795790173152,'user','You',1795790173522,1795790173522,1795790173522,NULL,NULL,4,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21d6f74167466a0817ee75bfb0e5d',x'01a21d6f730d714db8051f74aa304511',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21d6f741674b3baae059bf09a5267',x'01a21d6f730d75ba8a8369f91280692d',1795790173206,x'01a21d6f730d74fc8530df7522f94ec2',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795790173206,'user','You',1795790173206,1795790173206,NULL,NULL,NULL,1,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21d6f78607857b3493d809969372b',x'01a21d6f72cb7718897aaa00d0a4d932',4,'Water the plants','water the plants','',NULL,x'01a21d6f72cb7d41bb968e5c797c40cf',1795790174304,x'01a21d6f72cb705c97e1bc22f42d9655',0,NULL,'i002',NULL,'2026-12-02',NULL,NULL,0,0,0,NULL,NULL,NULL,1795790174304,'user','You',1795790174361,1795790174361,NULL,NULL,NULL,2,NULL,NULL,x'01a21d6f78997bf4b5f35eb09a858108',NULL,NULL);
INSERT INTO "task_labels" VALUES(x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f734273e1acbb8d1fbff14fe2');
INSERT INTO "task_relations" VALUES(x'01a21d6f7449779599fbd0a1a5e3e8f0',x'01a21d6f73e07414bf8760d4c82e3eeb',x'01a21d6f737979e8a17b6ac8781c6492','blocks',NULL,1795790173257,'user','You');
INSERT INTO "comments" VALUES(x'01a21d6f74b27152a3f004839427d6fa',x'01a21d6f737979e8a17b6ac8781c6492','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795790173362,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21d6f72cb7726a1932930c3780557','project',x'01a21d6f72cb7718897aaa00d0a4d932','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795790172875,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21d6f73797deabd7b14f0d9116ec0','task',x'01a21d6f737979e8a17b6ac8781c6492','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795790173049,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21d6f741674b3baae059bf09a5267','task',x'01a21d6f74167466a0817ee75bfb0e5d','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795790173206,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21d6f747e70708e7d1969a657c86d','task',x'01a21d6f737979e8a17b6ac8781c6492','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21d6f73797deabd7b14f0d9116ec0','normal',NULL,'user','You',NULL,1795790173310,NULL);
INSERT INTO "time_entries" VALUES(x'01a21d6f74e774019471e915950921b6',x'01a21d6f737979e8a17b6ac8781c6492','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795790173415,NULL,NULL,NULL,1,NULL);
INSERT INTO "time_entries" VALUES(x'01a21d6f77f97f81a52d1158f3ac9f20',x'01a21d6f737979e8a17b6ac8781c6492','2026-11-21',45,'',NULL,'user','You',NULL,1795790174201,NULL,NULL,NULL,1,x'01a21d6f77c473efbcb57d84b93ec574');
INSERT INTO "timer" VALUES(1,x'01a21d6f73ae7847b70f3f8f8ea989ad',1795790173573,'user','You',1795790173573,0,'',NULL,NULL);
INSERT INTO "activity" VALUES(1,x'01a21d6f72cb78e4a4edf29b2f4f3f15',1795790172875,'project.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','project',x'01a21d6f72cb7718897aaa00d0a4d932','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21d6f730d79569c4115d0439bea67',1795790172941,'project.create','user','You','cli',NULL,x'01a21d6f730d714db8051f74aa304511','project',x'01a21d6f730d714db8051f74aa304511','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21d6f734276dcaeeb5dc3ebc91a13',1795790172994,'label.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','label',x'01a21d6f734273e1acbb8d1fbff14fe2','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21d6f7379732cb4968370848c500d',1795790173049,'task.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f737979e8a17b6ac8781c6492','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21d6f73ae75df9533dd9e943cb6be',1795790173102,'task.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f73ae7847b70f3f8f8ea989ad','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21d6f73e071b2a400195957dd5563',1795790173152,'task.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f73e07414bf8760d4c82e3eeb','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21d6f741679878721f661f5786e03',1795790173206,'task.create','user','You','cli',NULL,x'01a21d6f730d714db8051f74aa304511','task',x'01a21d6f74167466a0817ee75bfb0e5d','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21d6f-730d-75ba-8a83-69f91280692d"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21d6f74497303b768d6d8490b31c0',1795790173257,'relation.add','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','relation',x'01a21d6f7449779599fbd0a1a5e3e8f0','{"fields":{"link":{"after":{"from":"01a21d6f-73e0-7414-bf87-60d4c82e3eeb","kind":"blocks","to":"01a21d6f-7379-79e8-a17b-6ac8781c6492"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21d6f747e7dde9d62b8009afb6428',1795790173310,'task.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f737979e8a17b6ac8781c6492','{"fields":{"description":{"after":{"revision":"01a21d6f-747e-7070-8e7d-1969a657c86d"},"before":{"revision":"01a21d6f-7379-7dea-bd7b-14f0d9116ec0"}}}}','{"set":{"description":{"revision":"01a21d6f-7379-7dea-bd7b-14f0d9116ec0"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21d6f74b270f8b2f63bd71d00987b',1795790173362,'comment.add','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','comment',x'01a21d6f74b27152a3f004839427d6fa','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21d6f74e7755e8bf0821f145bee44',1795790173415,'time.log','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','time_entry',x'01a21d6f74e774019471e915950921b6','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21d6f751c76648a9b758b7bebcf01',1795790173468,'reminder.set','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f737979e8a17b6ac8781c6492','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21d6f75527064978cd723ca3afa20',1795790173522,'task.move','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f73e07414bf8760d4c82e3eeb','{"fields":{"closed_at":{"after":1795790173522},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21d6f-72cb-784e-8e66-5562a9105807","before":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21d6f758570dcbda0a9bc9c8bafc5',1795790173573,'timer.start','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','timer',x'01a21d6f727e7715b929d4ab7617a69b','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795790173573,"started_at":1795790173573,"task":"01a21d6f-73ae-7847-b70f-3f8f8ea989ad"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21d6f75bc770fb7b5c5fee7e5b9a1',1795790173628,'wiki.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f75bc7ab3a99b326209fadfa0','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21d6f75f27795b29bf818a8005dad',1795790173682,'wiki.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f75f279dabab78e4a7ef886a5','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21d6f762670f28f0f1ab6e8ffa1bd',1795790173734,'wiki.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f75f279dabab78e4a7ef886a5','{"fields":{"body":{"after":{"revision":"01a21d6f-7626-78fb-b9eb-46f34aec3f98"},"before":{"revision":"01a21d6f-75f2-7618-a581-dad3bb3629ac"}}}}','{"set":{"body":{"revision":"01a21d6f-75f2-7618-a581-dad3bb3629ac"}}}',NULL);
INSERT INTO "activity" VALUES(18,x'01a21d6f76597e068675a8c63bffbaa4',1795790173785,'wiki.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f75bc7ab3a99b326209fadfa0','{"fields":{"home":{"after":true,"before":false}}}','{"set":{"home":false}}',NULL);
INSERT INTO "activity" VALUES(19,x'01a21d6f768b7823ae0c0d959bd9a05a',1795790173835,'wiki.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f768b7fd18de1350210cd95d5','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(20,x'01a21d6f76be7ec48f3380956254fa20',1795790173886,'wiki.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','wiki_page',x'01a21d6f768b7fd18de1350210cd95d5','{"fields":{"template":{"after":true,"before":false}}}','{"set":{"template":false}}',NULL);
INSERT INTO "activity" VALUES(21,x'01a21d6f76f2768ca0f134e62b4c5cbf',1795790173938,'version.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','version',x'01a21d6f76f2739d9ddbe540c8cac44e','{"fields":{"record":{"after":{"created_at":1795790173938,"description":"","due_date":"2027-03-01","name":"Spring","project":"01a21d6f-72cb-7718-897a-aa00d0a4d932","rank":"i","sharing":"none","status":"open","updated_at":1795790173938}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(22,x'01a21d6f772774fd8a07888964ed6571',1795790173991,'task.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f737979e8a17b6ac8781c6492','{"fields":{"fix_version":{"after":"01a21d6f-76f2-739d-9ddb-e540c8cac44e"}}}','{"set":{"fix_version":null}}',NULL);
INSERT INTO "activity" VALUES(23,x'01a21d6f77597d269cab213d97efce4a',1795790174041,'field.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','custom_field',x'01a21d6f77597d23804e0009aac1b438','{"fields":{"created":{"after":true},"name":{"after":"Cost"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(24,x'01a21d6f778e7c6e9b163b6ad983ff8a',1795790174094,'task.update','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f737979e8a17b6ac8781c6492','{"fields":{"cf:01a21d6f-7759-7d23-804e-0009aac1b438":{"after":12.5}}}','{"set":{"cf:01a21d6f-7759-7d23-804e-0009aac1b438":null}}',NULL);
INSERT INTO "activity" VALUES(25,x'01a21d6f77c475259fa9f1e2c857af6a',1795790174148,'work_kind.create','user','You','cli',NULL,NULL,'work_kind',x'01a21d6f77c473efbcb57d84b93ec574','{"fields":{"record":{"after":{"created_at":1795790174148,"created_by_kind":"user","created_by_name":"You","name":"Digging","updated_at":1795790174148}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(26,x'01a21d6f77f97549b83fc3738ddeb41a',1795790174201,'time.log','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','time_entry',x'01a21d6f77f97f81a52d1158f3ac9f20','{"fields":{"created":{"after":true},"date":{"after":"2026-11-21"},"minutes":{"after":45},"task":{"after":"TM-1"},"work_kind":{"after":"01a21d6f-77c4-73ef-bcb5-7d84b93ec574"}}}','{"set":{"created":null,"date":null,"minutes":null,"task":null,"work_kind":null}}',NULL);
INSERT INTO "activity" VALUES(27,x'01a21d6f782c7ed0a971674cc5140cb0',1795790174252,'type.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task_type',x'01a21d6f782c741a9285b63ea89d7960','{"fields":{"created":{"after":true},"name":{"after":"Epic"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(28,x'01a21d6f78607fd19be5ecde4be5a44c',1795790174304,'task.create','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f78607857b3493d809969372b','{"fields":{"created":{"after":true},"key":{"after":"TM-4"},"status":{"after":"01a21d6f-72cb-7d41-bb96-8e5c797c40cf"},"title":{"after":"Water the plants"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(29,x'01a21d6f789974538d2d5ee9b2648b5b',1795790174361,'recurrence.set','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','series',x'01a21d6f78997bf4b5f35eb09a858108','{"fields":{"record":{"after":{"created_at":1795790174361,"created_by_kind":"user","created_by_name":"You","current":"01a21d6f-7860-7857-b349-3d809969372b","made":1,"pattern":{"due_in_days":0,"title":"Water the plants","type":"Task"},"project":"01a21d6f-72cb-7718-897a-aa00d0a4d932","rule":{"create":"on_close","every":"week","from":"schedule","interval":1,"starts":"2026-12-02","weekdays":["wed"]},"updated_at":1795790174361}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(30,x'01a21d6f789974538d2d5ee9b2648b5b',1795790174361,'recurrence.set','user','You','cli',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','task',x'01a21d6f78607857b3493d809969372b','{"fields":{"series":{"after":"01a21d6f-7899-7bf4-b5f3-5eb09a858108"}}}','{"set":{"series":null}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f72cb7718897aaa00d0a4d932','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795790173310);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21d6f73ae7847b70f3f8f8ea989ad',x'01a21d6f73ae7847b70f3f8f8ea989ad',x'01a21d6f72cb7718897aaa00d0a4d932','Buy seeds','',1795790173102);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21d6f73e07414bf8760d4c82e3eeb',x'01a21d6f73e07414bf8760d4c82e3eeb',x'01a21d6f72cb7718897aaa00d0a4d932','Fix the fence','',1795790173152);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21d6f74167466a0817ee75bfb0e5d',x'01a21d6f74167466a0817ee75bfb0e5d',x'01a21d6f730d714db8051f74aa304511','Launch the homepage','Ship it soon.
',1795790173206);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21d6f74b27152a3f004839427d6fa',x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f72cb7718897aaa00d0a4d932','Plan the garden','The neighbours like the garden plan.
',1795790173362);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21d6f75bc7ab3a99b326209fadfa0',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795790173628);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21d6f75f279dabab78e4a7ef886a5',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','Seeds','Tomatoes, beans and peas.
',1795790173734);
INSERT INTO "search_docs" VALUES(8,'wiki',x'01a21d6f768b7fd18de1350210cd95d5',NULL,x'01a21d6f72cb7718897aaa00d0a4d932','Meeting notes','Who
What
',1795790173835);
INSERT INTO "search_docs" VALUES(9,'task',x'01a21d6f78607857b3493d809969372b',x'01a21d6f78607857b3493d809969372b',x'01a21d6f72cb7718897aaa00d0a4d932','Water the plants','',1795790174304);
INSERT INTO "wiki_pages" VALUES(x'01a21d6f75bc7ab3a99b326209fadfa0',x'01a21d6f72cb7718897aaa00d0a4d932',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21d6f75bc7400b6cc751158ca465d',0,1795790173628,1795790173785,NULL,NULL,1,0);
INSERT INTO "wiki_pages" VALUES(x'01a21d6f75f279dabab78e4a7ef886a5',x'01a21d6f72cb7718897aaa00d0a4d932',x'01a21d6f75bc7ab3a99b326209fadfa0','i','Seeds','seeds','seeds',x'01a21d6f762678fbb9eb46f34aec3f98',0,1795790173682,1795790173734,NULL,NULL,0,0);
INSERT INTO "wiki_pages" VALUES(x'01a21d6f768b7fd18de1350210cd95d5',x'01a21d6f72cb7718897aaa00d0a4d932',NULL,'i001','Meeting notes','meeting notes','meeting-notes',x'01a21d6f768b7bba8378b9c6ba18f8c0',0,1795790173835,1795790173886,NULL,NULL,0,1);
INSERT INTO "wiki_revisions" VALUES(x'01a21d6f75bc7400b6cc751158ca465d',x'01a21d6f75bc7ab3a99b326209fadfa0','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795790173628,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21d6f75f27618a581dad3bb3629ac',x'01a21d6f75f279dabab78e4a7ef886a5','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795790173682,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21d6f762678fbb9eb46f34aec3f98',x'01a21d6f75f279dabab78e4a7ef886a5','Tomatoes, beans and peas.',x'01a21d6f75f27618a581dad3bb3629ac','normal',NULL,'user','You',NULL,1795790173734,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21d6f768b7bba8378b9c6ba18f8c0',x'01a21d6f768b7fd18de1350210cd95d5','## Who

## What',NULL,'normal',NULL,'user','You',NULL,1795790173835,NULL);
INSERT INTO "reminders" VALUES(x'01a21d6f751c7cfe9499487af42f38f0',x'01a21d6f737979e8a17b6ac8781c6492',1796029200000,NULL,NULL,'','user','You',1795790173468,NULL,NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21d6f75bc7ab3a99b326209fadfa0',0,'wiki','Seeds',NULL,'seeds','seeds','','',NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21d6f75bc7ab3a99b326209fadfa0',1,'task','TM-1',NULL,'','','','',1);
INSERT INTO "custom_fields" VALUES(x'01a21d6f77597d23804e0009aac1b438',x'01a21d6f72cb7718897aaa00d0a4d932','Cost','cost','cost','decimal','[]',NULL,'[]',0,'i',1795790174041);
INSERT INTO "custom_values" VALUES(x'01a21d6f737979e8a17b6ac8781c6492',x'01a21d6f77597d23804e0009aac1b438',12.5);
INSERT INTO "versions" VALUES(x'01a21d6f76f2739d9ddbe540c8cac44e',x'01a21d6f72cb7718897aaa00d0a4d932','Spring','spring','','open',NULL,'2027-03-01','none','i',1795790173938,1795790173938);
INSERT INTO "task_series" VALUES(x'01a21d6f78997bf4b5f35eb09a858108',x'01a21d6f72cb7718897aaa00d0a4d932','{"every":"week","interval":1,"weekdays":["wed"],"starts":"2026-12-02","from":"schedule","create":"on_close"}','{"title":"Water the plants","type":"Task","due_in_days":0}',x'01a21d6f78607857b3493d809969372b',NULL,1,NULL,1795790174361,'user','You',1795790174361);
INSERT INTO "work_kinds" VALUES(x'01a21d6f77c473efbcb57d84b93ec574','Digging','digging',NULL,1795790174148,'user','You',1795790174148);
INSERT INTO "task_types" VALUES(x'01a21d6f72cb705c97e1bc22f42d9655',x'01a21d6f72cb7718897aaa00d0a4d932','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f72cb716f822ab93851928fcd',x'01a21d6f72cb7718897aaa00d0a4d932','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f72cb737288baad1e094051ac',x'01a21d6f72cb7718897aaa00d0a4d932','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f72cb712c9680d97f1c28b918',x'01a21d6f72cb7718897aaa00d0a4d932','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f730d74fc8530df7522f94ec2',x'01a21d6f730d714db8051f74aa304511','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f730d76eaad4ca262a33c33dd',x'01a21d6f730d714db8051f74aa304511','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f730d70a98cfc667439150aa4',x'01a21d6f730d714db8051f74aa304511','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f730d72f7b5c3dcd3552e4bdc',x'01a21d6f730d714db8051f74aa304511','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21d6f782c741a9285b63ea89d7960',x'01a21d6f72cb7718897aaa00d0a4d932','Epic','epic','epic','grey','ssst',0,NULL);
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
CREATE UNIQUE INDEX sprints_one_active ON sprints (project_id) WHERE state = 'active';
CREATE INDEX tasks_sprint ON tasks (sprint_id) WHERE sprint_id IS NOT NULL;
CREATE UNIQUE INDEX automation_rules_name
    ON automation_rules (coalesce(project_id, x''), name_fold);
CREATE INDEX automation_rules_trigger ON automation_rules (trigger_kind) WHERE enabled = 1;
CREATE INDEX automation_runs_rule ON automation_runs (rule_id, at);
CREATE INDEX automation_runs_at ON automation_runs (at);
CREATE INDEX automation_notices_open ON automation_notices (at) WHERE acked_at IS NULL;
CREATE UNIQUE INDEX dashboards_name ON dashboards (coalesce(project_id, x''), name_fold);
CREATE INDEX projects_folder ON projects (folder_id) WHERE folder_id IS NOT NULL;
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 22;
COMMIT;
