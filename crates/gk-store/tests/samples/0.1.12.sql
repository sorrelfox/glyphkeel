-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.12.
-- release: 0.1.12
-- schema: 24
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
CREATE TABLE personal_marks (
  person TEXT NOT NULL CHECK (length(person) BETWEEN 1 AND 100),
  mark   TEXT NOT NULL CHECK (mark IN ('digest_seen')),
  at     INTEGER NOT NULL,
  PRIMARY KEY (person, mark)
) STRICT, WITHOUT ROWID;
CREATE TABLE proposals (
  id              BLOB PRIMARY KEY CHECK (length(id) = 16),
  created_at      INTEGER NOT NULL,
  op              TEXT NOT NULL,
  input           TEXT NOT NULL,
  session_id      BLOB REFERENCES agent_sessions (id),
  agent_name      TEXT NOT NULL,
  profile         TEXT NOT NULL CHECK (profile IN ('read-only', 'standard', 'full')),
  project_scope   TEXT,
  project_id      BLOB,
  task_id         BLOB,
  summary         TEXT NOT NULL,
  previewed       INTEGER NOT NULL CHECK (previewed IN (0, 1)),
  preview         TEXT NOT NULL,
  request_key     TEXT UNIQUE,
  status          TEXT NOT NULL
                  CHECK (status IN ('pending', 'approving', 'approved', 'rejected')),
  claimed_at      INTEGER,
  last_error      TEXT,
  decided_at      INTEGER,
  decided_by_kind TEXT CHECK (decided_by_kind IN ('user', 'agent', 'automation')),
  decided_by_name TEXT,
  reason          TEXT,
  group_id        BLOB
) STRICT;
INSERT INTO "meta" VALUES('workspace_id','01a2234c-8059-7852-a81f-29d4669157b2');
INSERT INTO "meta" VALUES('created_at','1795888545881');
INSERT INTO "meta" VALUES('created_by_version','0.1.12');
INSERT INTO "projects" VALUES(x'01a2234c80af7347817103a3bbfdf52a',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a2234c80af75f7888b8f277a8b0d3d','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',5,1795888545967,1795888547522,NULL,NULL,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a2234c80f779d59b108132dab1736c',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795888546039,1795888546340,NULL,NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80af7c5d8307d58f45126d52',x'01a2234c80af7347817103a3bbfdf52a','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80af7216babbf6eacd13597d',x'01a2234c80af7347817103a3bbfdf52a','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80af77e390ea204f193827f6',x'01a2234c80af7347817103a3bbfdf52a','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80af7922bc3910ece0bb9c49',x'01a2234c80af7347817103a3bbfdf52a','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80af7c7ba71be2e26e49c63d',x'01a2234c80af7347817103a3bbfdf52a','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80f77fb7b7d458039c0f6944',x'01a2234c80f779d59b108132dab1736c','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80f77394b86df36f4c385133',x'01a2234c80f779d59b108132dab1736c','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80f77475b083f3706dd23f5c',x'01a2234c80f779d59b108132dab1736c','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80f7770197055b697ac855fa',x'01a2234c80f779d59b108132dab1736c','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a2234c80f770be8851489703a87eb2',x'01a2234c80f779d59b108132dab1736c','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "labels" VALUES(x'01a2234c813d75c581b01b63ed8b759b',x'01a2234c80af7347817103a3bbfdf52a','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c80af7347817103a3bbfdf52a',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a2234c82947c56809571116a2220c8',x'01a2234c80af7c5d8307d58f45126d52',1795888546165,x'01a2234c80af794fac694da1c0c42657',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,135,NULL,NULL,NULL,1795888546165,'user','You',1795888547300,1795888547415,NULL,NULL,NULL,4,NULL,x'01a2234c85377db1bdb0f92f190e816e',NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a2234c81b47f7fb4c14f0e0dfd2346',x'01a2234c80af7347817103a3bbfdf52a',2,'Buy seeds','buy seeds','',NULL,x'01a2234c80af7c5d8307d58f45126d52',1795888546228,x'01a2234c80af794fac694da1c0c42657',0,x'01a2234c81757ab5a3df34f2f1464a95','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795888546228,'user','You',1795888546228,1795888546228,NULL,NULL,NULL,1,'i',NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a2234c81ed7130a3e7729be2f14d5a',x'01a2234c80af7347817103a3bbfdf52a',3,'Fix the fence','fix the fence','',NULL,x'01a2234c80af7c7ba71be2e26e49c63d',1795888546680,x'01a2234c80af794fac694da1c0c42657',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795888546285,'user','You',1795888546680,1795888546680,1795888546680,NULL,NULL,4,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a2234c822478e4a7a44cda652b03b4',x'01a2234c80f779d59b108132dab1736c',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a2234c82247106bdcadabc541c19d4',x'01a2234c80f77fb7b7d458039c0f6944',1795888546340,x'01a2234c80f77cc3b1ed0bb10e132a9c',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795888546340,'user','You',1795888546340,1795888546340,NULL,NULL,NULL,1,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a2234c86c2705e9278e0471ec55c35',x'01a2234c80af7347817103a3bbfdf52a',4,'Water the plants','water the plants','',NULL,x'01a2234c80af7c5d8307d58f45126d52',1795888547522,x'01a2234c80af794fac694da1c0c42657',0,NULL,'i002',NULL,'2026-12-02',NULL,NULL,0,0,0,NULL,NULL,NULL,1795888547522,'user','You',1795888547579,1795888547579,NULL,NULL,NULL,2,NULL,NULL,x'01a2234c86fb7c3f823989a8c20a64a0',NULL,NULL);
INSERT INTO "task_labels" VALUES(x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c813d75c581b01b63ed8b759b');
INSERT INTO "task_relations" VALUES(x'01a2234c825b72b08aa500b725d23548',x'01a2234c81ed7130a3e7729be2f14d5a',x'01a2234c81757ab5a3df34f2f1464a95','blocks',NULL,1795888546395,'user','You');
INSERT INTO "comments" VALUES(x'01a2234c82ca7926813aa5a2f3faf277',x'01a2234c81757ab5a3df34f2f1464a95','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795888546506,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a2234c80af75f7888b8f277a8b0d3d','project',x'01a2234c80af7347817103a3bbfdf52a','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795888545967,NULL);
INSERT INTO "text_revisions" VALUES(x'01a2234c8175700091d7a07d0918bd2f','task',x'01a2234c81757ab5a3df34f2f1464a95','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795888546165,NULL);
INSERT INTO "text_revisions" VALUES(x'01a2234c82247106bdcadabc541c19d4','task',x'01a2234c822478e4a7a44cda652b03b4','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795888546340,NULL);
INSERT INTO "text_revisions" VALUES(x'01a2234c82947c56809571116a2220c8','task',x'01a2234c81757ab5a3df34f2f1464a95','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a2234c8175700091d7a07d0918bd2f','normal',NULL,'user','You',NULL,1795888546452,NULL);
INSERT INTO "time_entries" VALUES(x'01a2234c83047076a39a1fd75b315919',x'01a2234c81757ab5a3df34f2f1464a95','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795888546564,NULL,NULL,NULL,1,NULL);
INSERT INTO "time_entries" VALUES(x'01a2234c8657795fa7d4e70022c97db2',x'01a2234c81757ab5a3df34f2f1464a95','2026-11-21',45,'',NULL,'user','You',NULL,1795888547415,NULL,NULL,NULL,1,x'01a2234c861e7b06bd33e681d643e940');
INSERT INTO "timer" VALUES(1,x'01a2234c81b47f7fb4c14f0e0dfd2346',1795888546734,'user','You',1795888546734,0,'',NULL,NULL);
INSERT INTO "activity" VALUES(1,x'01a2234c80af7655997d62ad1d7243e4',1795888545967,'project.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','project',x'01a2234c80af7347817103a3bbfdf52a','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a2234c80f77ff6ae216ad71acdefeb',1795888546039,'project.create','user','You','cli',NULL,x'01a2234c80f779d59b108132dab1736c','project',x'01a2234c80f779d59b108132dab1736c','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a2234c813d7bebbdd00e8d1f2da20e',1795888546109,'label.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','label',x'01a2234c813d75c581b01b63ed8b759b','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a2234c817578df943dcebbd4656a01',1795888546165,'task.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81757ab5a3df34f2f1464a95','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a2234c-80af-7c5d-8307-d58f45126d52"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a2234c81b47c9fbf9058ad0413adbe',1795888546228,'task.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81b47f7fb4c14f0e0dfd2346','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a2234c-80af-7c5d-8307-d58f45126d52"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a2234c81ed78d7a3d87c566880a519',1795888546285,'task.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81ed7130a3e7729be2f14d5a','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a2234c-80af-7c5d-8307-d58f45126d52"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a2234c82247fa7a37d252c639297b0',1795888546340,'task.create','user','You','cli',NULL,x'01a2234c80f779d59b108132dab1736c','task',x'01a2234c822478e4a7a44cda652b03b4','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a2234c-80f7-7fb7-b7d4-58039c0f6944"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a2234c825b73699848c7159bb02e2d',1795888546395,'relation.add','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','relation',x'01a2234c825b72b08aa500b725d23548','{"fields":{"link":{"after":{"from":"01a2234c-81ed-7130-a3e7-729be2f14d5a","kind":"blocks","to":"01a2234c-8175-7ab5-a3df-34f2f1464a95"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a2234c82947cdd8cdaa89923c1aac6',1795888546452,'task.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81757ab5a3df34f2f1464a95','{"fields":{"description":{"after":{"revision":"01a2234c-8294-7c56-8095-71116a2220c8"},"before":{"revision":"01a2234c-8175-7000-91d7-a07d0918bd2f"}}}}','{"set":{"description":{"revision":"01a2234c-8175-7000-91d7-a07d0918bd2f"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a2234c82ca74c2bdd24ad28e7aa740',1795888546506,'comment.add','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','comment',x'01a2234c82ca7926813aa5a2f3faf277','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a2234c83047524b9d5a81d3c239942',1795888546564,'time.log','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','time_entry',x'01a2234c83047076a39a1fd75b315919','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a2234c833f75cf9dd277acd6b889e9',1795888546623,'reminder.set','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81757ab5a3df34f2f1464a95','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a2234c83787449b2def831cd94f93d',1795888546680,'task.move','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81ed7130a3e7729be2f14d5a','{"fields":{"closed_at":{"after":1795888546680},"rank":{"after":"i","before":"i002"},"status":{"after":"01a2234c-80af-7c7b-a71b-e2e26e49c63d","before":"01a2234c-80af-7c5d-8307-d58f45126d52"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a2234c-80af-7c5d-8307-d58f45126d52"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a2234c83ae755497b0b5761ee48afa',1795888546734,'timer.start','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','timer',x'01a2234c80597852a81f29d4669157b2','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795888546734,"started_at":1795888546734,"task":"01a2234c-81b4-7f7f-b4c1-4f0e0dfd2346"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a2234c83e67c938cbaa2501436bd3b',1795888546790,'wiki.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c83e67b7a89044f0523d055a2','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a2234c84217d32a98ce63e1d40a942',1795888546849,'wiki.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c842179caa336453f98117432','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a2234c845973cb8122972516015ffe',1795888546905,'wiki.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c842179caa336453f98117432','{"fields":{"body":{"after":{"revision":"01a2234c-8459-7e2a-8d65-0b1cc6dae41e"},"before":{"revision":"01a2234c-8421-7609-b942-c7ccb60e5de4"}}}}','{"set":{"body":{"revision":"01a2234c-8421-7609-b942-c7ccb60e5de4"}}}',NULL);
INSERT INTO "activity" VALUES(18,x'01a2234c849079d8a518d4fbd83e826b',1795888546960,'wiki.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c83e67b7a89044f0523d055a2','{"fields":{"home":{"after":true,"before":false}}}','{"set":{"home":false}}',NULL);
INSERT INTO "activity" VALUES(19,x'01a2234c84c673e9ab4eeb7bc0cf4dbd',1795888547014,'wiki.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c84c6749b82f51b2b727ae688','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(20,x'01a2234c84ff7ee19f3ec60e089186c5',1795888547071,'wiki.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','wiki_page',x'01a2234c84c6749b82f51b2b727ae688','{"fields":{"template":{"after":true,"before":false}}}','{"set":{"template":false}}',NULL);
INSERT INTO "activity" VALUES(21,x'01a2234c8537759ebc187726edd8e296',1795888547127,'version.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','version',x'01a2234c85377db1bdb0f92f190e816e','{"fields":{"record":{"after":{"created_at":1795888547127,"description":"","due_date":"2027-03-01","name":"Spring","project":"01a2234c-80af-7347-8171-03a3bbfdf52a","rank":"i","sharing":"none","status":"open","updated_at":1795888547127}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(22,x'01a2234c85707fd69e918ddb1885488b',1795888547184,'task.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81757ab5a3df34f2f1464a95','{"fields":{"fix_version":{"after":"01a2234c-8537-7db1-bdb0-f92f190e816e"}}}','{"set":{"fix_version":null}}',NULL);
INSERT INTO "activity" VALUES(23,x'01a2234c85a87980b175ca4b66da1370',1795888547240,'field.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','custom_field',x'01a2234c85a8798993f0e1543180cf96','{"fields":{"created":{"after":true},"name":{"after":"Cost"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(24,x'01a2234c85e47c0aa0c2b98237c73afa',1795888547300,'task.update','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c81757ab5a3df34f2f1464a95','{"fields":{"cf:01a2234c-85a8-7989-93f0-e1543180cf96":{"after":12.5}}}','{"set":{"cf:01a2234c-85a8-7989-93f0-e1543180cf96":null}}',NULL);
INSERT INTO "activity" VALUES(25,x'01a2234c861e72aea65f79b9c72be331',1795888547358,'work_kind.create','user','You','cli',NULL,NULL,'work_kind',x'01a2234c861e7b06bd33e681d643e940','{"fields":{"record":{"after":{"created_at":1795888547358,"created_by_kind":"user","created_by_name":"You","name":"Digging","updated_at":1795888547358}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(26,x'01a2234c865772dcba5b4021f0ae2dd4',1795888547415,'time.log','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','time_entry',x'01a2234c8657795fa7d4e70022c97db2','{"fields":{"created":{"after":true},"date":{"after":"2026-11-21"},"minutes":{"after":45},"task":{"after":"TM-1"},"work_kind":{"after":"01a2234c-861e-7b06-bd33-e681d643e940"}}}','{"set":{"created":null,"date":null,"minutes":null,"task":null,"work_kind":null}}',NULL);
INSERT INTO "activity" VALUES(27,x'01a2234c868c75f7a755f7af110736cb',1795888547468,'type.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task_type',x'01a2234c868c755f814d73ab6783b568','{"fields":{"created":{"after":true},"name":{"after":"Epic"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(28,x'01a2234c86c27a0a87612226fcfc9a4a',1795888547522,'task.create','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c86c2705e9278e0471ec55c35','{"fields":{"created":{"after":true},"key":{"after":"TM-4"},"status":{"after":"01a2234c-80af-7c5d-8307-d58f45126d52"},"title":{"after":"Water the plants"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(29,x'01a2234c86fb75e28546d0dc771195d6',1795888547579,'recurrence.set','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','series',x'01a2234c86fb7c3f823989a8c20a64a0','{"fields":{"record":{"after":{"created_at":1795888547579,"created_by_kind":"user","created_by_name":"You","current":"01a2234c-86c2-705e-9278-e0471ec55c35","made":1,"pattern":{"due_in_days":0,"title":"Water the plants","type":"Task"},"project":"01a2234c-80af-7347-8171-03a3bbfdf52a","rule":{"create":"on_close","every":"week","from":"schedule","interval":1,"starts":"2026-12-02","weekdays":["wed"]},"updated_at":1795888547579}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(30,x'01a2234c86fb75e28546d0dc771195d6',1795888547579,'recurrence.set','user','You','cli',NULL,x'01a2234c80af7347817103a3bbfdf52a','task',x'01a2234c86c2705e9278e0471ec55c35','{"fields":{"series":{"after":"01a2234c-86fb-7c3f-8239-89a8c20a64a0"}}}','{"set":{"series":null}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c80af7347817103a3bbfdf52a','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795888546452);
INSERT INTO "search_docs" VALUES(2,'task',x'01a2234c81b47f7fb4c14f0e0dfd2346',x'01a2234c81b47f7fb4c14f0e0dfd2346',x'01a2234c80af7347817103a3bbfdf52a','Buy seeds','',1795888546228);
INSERT INTO "search_docs" VALUES(3,'task',x'01a2234c81ed7130a3e7729be2f14d5a',x'01a2234c81ed7130a3e7729be2f14d5a',x'01a2234c80af7347817103a3bbfdf52a','Fix the fence','',1795888546285);
INSERT INTO "search_docs" VALUES(4,'task',x'01a2234c822478e4a7a44cda652b03b4',x'01a2234c822478e4a7a44cda652b03b4',x'01a2234c80f779d59b108132dab1736c','Launch the homepage','Ship it soon.
',1795888546340);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a2234c82ca7926813aa5a2f3faf277',x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c80af7347817103a3bbfdf52a','Plan the garden','The neighbours like the garden plan.
',1795888546506);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a2234c83e67b7a89044f0523d055a2',NULL,x'01a2234c80af7347817103a3bbfdf52a','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795888546790);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a2234c842179caa336453f98117432',NULL,x'01a2234c80af7347817103a3bbfdf52a','Seeds','Tomatoes, beans and peas.
',1795888546905);
INSERT INTO "search_docs" VALUES(8,'wiki',x'01a2234c84c6749b82f51b2b727ae688',NULL,x'01a2234c80af7347817103a3bbfdf52a','Meeting notes','Who
What
',1795888547014);
INSERT INTO "search_docs" VALUES(9,'task',x'01a2234c86c2705e9278e0471ec55c35',x'01a2234c86c2705e9278e0471ec55c35',x'01a2234c80af7347817103a3bbfdf52a','Water the plants','',1795888547522);
INSERT INTO "wiki_pages" VALUES(x'01a2234c83e67b7a89044f0523d055a2',x'01a2234c80af7347817103a3bbfdf52a',NULL,'i','Garden notes','garden notes','garden-notes',x'01a2234c83e67172a6d11f5c5b1fb0c8',0,1795888546790,1795888546960,NULL,NULL,1,0);
INSERT INTO "wiki_pages" VALUES(x'01a2234c842179caa336453f98117432',x'01a2234c80af7347817103a3bbfdf52a',x'01a2234c83e67b7a89044f0523d055a2','i','Seeds','seeds','seeds',x'01a2234c84597e2a8d650b1cc6dae41e',0,1795888546849,1795888546905,NULL,NULL,0,0);
INSERT INTO "wiki_pages" VALUES(x'01a2234c84c6749b82f51b2b727ae688',x'01a2234c80af7347817103a3bbfdf52a',NULL,'i001','Meeting notes','meeting notes','meeting-notes',x'01a2234c84c67032bb9fe65dea8c61f8',0,1795888547014,1795888547071,NULL,NULL,0,1);
INSERT INTO "wiki_revisions" VALUES(x'01a2234c83e67172a6d11f5c5b1fb0c8',x'01a2234c83e67b7a89044f0523d055a2','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795888546790,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a2234c84217609b942c7ccb60e5de4',x'01a2234c842179caa336453f98117432','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795888546849,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a2234c84597e2a8d650b1cc6dae41e',x'01a2234c842179caa336453f98117432','Tomatoes, beans and peas.',x'01a2234c84217609b942c7ccb60e5de4','normal',NULL,'user','You',NULL,1795888546905,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a2234c84c67032bb9fe65dea8c61f8',x'01a2234c84c6749b82f51b2b727ae688','## Who

## What',NULL,'normal',NULL,'user','You',NULL,1795888547014,NULL);
INSERT INTO "reminders" VALUES(x'01a2234c833f7d8c862893c0cbe8ba93',x'01a2234c81757ab5a3df34f2f1464a95',1796029200000,NULL,NULL,'','user','You',1795888546623,NULL,NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a2234c83e67b7a89044f0523d055a2',0,'wiki','Seeds',NULL,'seeds','seeds','','',NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a2234c83e67b7a89044f0523d055a2',1,'task','TM-1',NULL,'','','','',1);
INSERT INTO "custom_fields" VALUES(x'01a2234c85a8798993f0e1543180cf96',x'01a2234c80af7347817103a3bbfdf52a','Cost','cost','cost','decimal','[]',NULL,'[]',0,'i',1795888547240);
INSERT INTO "custom_values" VALUES(x'01a2234c81757ab5a3df34f2f1464a95',x'01a2234c85a8798993f0e1543180cf96',12.5);
INSERT INTO "versions" VALUES(x'01a2234c85377db1bdb0f92f190e816e',x'01a2234c80af7347817103a3bbfdf52a','Spring','spring','','open',NULL,'2027-03-01','none','i',1795888547127,1795888547127);
INSERT INTO "task_series" VALUES(x'01a2234c86fb7c3f823989a8c20a64a0',x'01a2234c80af7347817103a3bbfdf52a','{"every":"week","interval":1,"weekdays":["wed"],"starts":"2026-12-02","from":"schedule","create":"on_close"}','{"title":"Water the plants","type":"Task","due_in_days":0}',x'01a2234c86c2705e9278e0471ec55c35',NULL,1,NULL,1795888547579,'user','You',1795888547579);
INSERT INTO "work_kinds" VALUES(x'01a2234c861e7b06bd33e681d643e940','Digging','digging',NULL,1795888547358,'user','You',1795888547358);
INSERT INTO "task_types" VALUES(x'01a2234c80af794fac694da1c0c42657',x'01a2234c80af7347817103a3bbfdf52a','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80af7712a616e9c952205ea2',x'01a2234c80af7347817103a3bbfdf52a','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80af7758a497c48652d0e44c',x'01a2234c80af7347817103a3bbfdf52a','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80af76efa2ba821d74947515',x'01a2234c80af7347817103a3bbfdf52a','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80f77cc3b1ed0bb10e132a9c',x'01a2234c80f779d59b108132dab1736c','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80f777b4b1b873bd80259b3d',x'01a2234c80f779d59b108132dab1736c','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80f774af967adbaa93aee87a',x'01a2234c80f779d59b108132dab1736c','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c80f7799c9c35b2717daecc49',x'01a2234c80f779d59b108132dab1736c','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a2234c868c755f814d73ab6783b568',x'01a2234c80af7347817103a3bbfdf52a','Epic','epic','epic','grey','ssst',0,NULL);
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
CREATE INDEX activity_at ON activity (at);
CREATE INDEX proposals_waiting ON proposals (created_at) WHERE status = 'pending';
CREATE INDEX proposals_session ON proposals (session_id, created_at);
CREATE INDEX proposals_agent ON proposals (agent_name, created_at);
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 24;
COMMIT;
