-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- A Glyphkeel workspace written by glyphkeel 0.1.8.
-- release: 0.1.8
-- schema: 20
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
INSERT INTO "meta" VALUES('workspace_id','01a21c85-ce81-7d00-8681-41b2585279ce');
INSERT INTO "meta" VALUES('created_at','1795774860929');
INSERT INTO "meta" VALUES('created_by_version','0.1.8');
INSERT INTO "projects" VALUES(x'01a21c85cecd73818014ba317af29697',NULL,'i','TM','Home tasks','home tasks','Things to do at home and in the garden.',x'01a21c85cecd71258377c379191f26cb','green','rocket','active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',5,1795774861005,1795774861577,NULL,NULL,NULL);
INSERT INTO "projects" VALUES(x'01a21c85cef37bb59f42cb999baed734',NULL,'i001','ACM','Acme website','acme website','',NULL,'blue',NULL,'active','{"board":true,"gantt":true,"list":true,"time":true,"wiki":true}',2,1795774861043,1795774861146,NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cecd797ca97dfa174302938c',x'01a21c85cecd73818014ba317af29697','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cecd7942b9f190fb886141ad',x'01a21c85cecd73818014ba317af29697','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cecd7c08a7628cf455158dd9',x'01a21c85cecd73818014ba317af29697','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cecd72d3b6608dc0e7b26b7a',x'01a21c85cecd73818014ba317af29697','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cecd7487aab2f40c49d9c8ea',x'01a21c85cecd73818014ba317af29697','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cef371d69445dde387d441dc',x'01a21c85cef37bb59f42cb999baed734','Backlog','backlog','todo','grey','6',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cef37a12905cf6909449238a',x'01a21c85cef37bb59f42cb999baed734','To do','to do','todo','blue','c',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cef377f5a460577cfac3e00c',x'01a21c85cef37bb59f42cb999baed734','In progress','in progress','in_progress','orange','i',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cef374ac886b4bbb5d0f02d7',x'01a21c85cef37bb59f42cb999baed734','Review','review','in_progress','purple','o',NULL,NULL,NULL);
INSERT INTO "statuses" VALUES(x'01a21c85cef371fbacd0cc1891b39158',x'01a21c85cef37bb59f42cb999baed734','Done','done','done','green','u',NULL,NULL,NULL);
INSERT INTO "labels" VALUES(x'01a21c85cf077bd785534d146cd3678c',x'01a21c85cecd73818014ba317af29697','Urgent','urgent','red','i');
INSERT INTO "tasks" VALUES(x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85cecd73818014ba317af29697',1,'Plan the garden','plan the garden','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21c85cf827fa0b90bafe8a634cc67',x'01a21c85cecd797ca97dfa174302938c',1795774861084,x'01a21c85cecd7d969159587ffa3c2b94',3,NULL,'i',NULL,'2026-12-01',120,NULL,1,2,135,NULL,NULL,NULL,1795774861084,'user','You',1795774861496,1795774861537,NULL,NULL,NULL,4,NULL,x'01a21c85d07c70499c0d442769f71c51',NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21c85cf317b599bfbd03badf0d356',x'01a21c85cecd73818014ba317af29697',2,'Buy seeds','buy seeds','',NULL,x'01a21c85cecd797ca97dfa174302938c',1795774861105,x'01a21c85cecd7d969159587ffa3c2b94',0,x'01a21c85cf1c7a769efdd8bdd3b1b79e','i001',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795774861105,'user','You',1795774861105,1795774861105,NULL,NULL,NULL,1,'i',NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21c85cf4571178106b60cd5ef47b0',x'01a21c85cecd73818014ba317af29697',3,'Fix the fence','fix the fence','',NULL,x'01a21c85cecd7487aab2f40c49d9c8ea',1795774861267,x'01a21c85cecd7d969159587ffa3c2b94',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795774861125,'user','You',1795774861267,1795774861267,1795774861267,NULL,NULL,4,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21c85cf5a7cd7884c73458c233d19',x'01a21c85cef37bb59f42cb999baed734',1,'Launch the homepage','launch the homepage','Ship it **soon**.',x'01a21c85cf5a7ee8b0ee43171b39cfbb',x'01a21c85cef371d69445dde387d441dc',1795774861146,x'01a21c85cef378a19d5a0bf04d713c7e',0,NULL,'i',NULL,NULL,NULL,NULL,0,0,0,NULL,NULL,NULL,1795774861146,'user','You',1795774861146,1795774861146,NULL,NULL,NULL,1,NULL,NULL,NULL,NULL,NULL);
INSERT INTO "tasks" VALUES(x'01a21c85d1097d6c90acf68b5215efac',x'01a21c85cecd73818014ba317af29697',4,'Water the plants','water the plants','',NULL,x'01a21c85cecd797ca97dfa174302938c',1795774861577,x'01a21c85cecd7d969159587ffa3c2b94',0,NULL,'i002',NULL,'2026-12-02',NULL,NULL,0,0,0,NULL,NULL,NULL,1795774861577,'user','You',1795774861598,1795774861598,NULL,NULL,NULL,2,NULL,NULL,x'01a21c85d11e76fd8b5d03a5cd9dbbc7',NULL,NULL);
INSERT INTO "task_labels" VALUES(x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85cf077bd785534d146cd3678c');
INSERT INTO "task_relations" VALUES(x'01a21c85cf6d7b2abae3db6180e83f9f',x'01a21c85cf4571178106b60cd5ef47b0',x'01a21c85cf1c7a769efdd8bdd3b1b79e','blocks',NULL,1795774861165,'user','You');
INSERT INTO "comments" VALUES(x'01a21c85cf947947b11b28422b8f5c53',x'01a21c85cf1c7a769efdd8bdd3b1b79e','The neighbours like the garden plan.',NULL,'user','You',NULL,'cli',1795774861204,NULL,NULL,NULL,1);
INSERT INTO "text_revisions" VALUES(x'01a21c85cecd71258377c379191f26cb','project',x'01a21c85cecd73818014ba317af29697','Things to do at home and in the garden.',NULL,'normal',NULL,'user','You',NULL,1795774861005,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21c85cf1c714dae8e5c1880220bf7','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','Beds, paths and a pond.

- [ ] measure
- [x] sketch',NULL,'normal',NULL,'user','You',NULL,1795774861084,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21c85cf5a7ee8b0ee43171b39cfbb','task',x'01a21c85cf5a7cd7884c73458c233d19','Ship it **soon**.',NULL,'normal',NULL,'user','You',NULL,1795774861146,NULL);
INSERT INTO "text_revisions" VALUES(x'01a21c85cf827fa0b90bafe8a634cc67','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','Beds, paths, a pond and a shed.

- [ ] measure
- [x] sketch',x'01a21c85cf1c714dae8e5c1880220bf7','normal',NULL,'user','You',NULL,1795774861186,NULL);
INSERT INTO "time_entries" VALUES(x'01a21c85cfaa7e11ba52cf57d1b3c18d',x'01a21c85cf1c7a769efdd8bdd3b1b79e','2026-11-20',90,'First sketch',NULL,'user','You',NULL,1795774861226,NULL,NULL,NULL,1,NULL);
INSERT INTO "time_entries" VALUES(x'01a21c85d0e17bc68ee15d31eeb3f425',x'01a21c85cf1c7a769efdd8bdd3b1b79e','2026-11-21',45,'',NULL,'user','You',NULL,1795774861537,NULL,NULL,NULL,1,x'01a21c85d0cc78acb686b154ea537a87');
INSERT INTO "timer" VALUES(1,x'01a21c85cf317b599bfbd03badf0d356',1795774861288,'user','You',1795774861288,0,'',NULL,NULL);
INSERT INTO "activity" VALUES(1,x'01a21c85cecd744ba5152c41414298fa',1795774861005,'project.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','project',x'01a21c85cecd73818014ba317af29697','{"fields":{"created":{"after":true},"key":{"after":"TM"},"name":{"after":"Home tasks"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(2,x'01a21c85cef3721ab9d77d3a4d44cede',1795774861043,'project.create','user','You','cli',NULL,x'01a21c85cef37bb59f42cb999baed734','project',x'01a21c85cef37bb59f42cb999baed734','{"fields":{"created":{"after":true},"key":{"after":"ACM"},"name":{"after":"Acme website"}}}','{"set":{"created":null,"key":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(3,x'01a21c85cf077d1698b7f6f5d289ebbf',1795774861063,'label.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','label',x'01a21c85cf077bd785534d146cd3678c','{"fields":{"created":{"after":true},"name":{"after":"Urgent"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(4,x'01a21c85cf1c78f587e844848f377f1c',1795774861084,'task.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','{"fields":{"created":{"after":true},"key":{"after":"TM-1"},"status":{"after":"01a21c85-cecd-797c-a97d-fa174302938c"},"title":{"after":"Plan the garden"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(5,x'01a21c85cf3178099122a846b4dcab06',1795774861105,'task.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf317b599bfbd03badf0d356','{"fields":{"created":{"after":true},"key":{"after":"TM-2"},"status":{"after":"01a21c85-cecd-797c-a97d-fa174302938c"},"title":{"after":"Buy seeds"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(6,x'01a21c85cf457d298b20a06f8eba7bb8',1795774861125,'task.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf4571178106b60cd5ef47b0','{"fields":{"created":{"after":true},"key":{"after":"TM-3"},"status":{"after":"01a21c85-cecd-797c-a97d-fa174302938c"},"title":{"after":"Fix the fence"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(7,x'01a21c85cf5a7de7bf743a29ad0a2e19',1795774861146,'task.create','user','You','cli',NULL,x'01a21c85cef37bb59f42cb999baed734','task',x'01a21c85cf5a7cd7884c73458c233d19','{"fields":{"created":{"after":true},"key":{"after":"ACM-1"},"status":{"after":"01a21c85-cef3-71d6-9445-dde387d441dc"},"title":{"after":"Launch the homepage"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(8,x'01a21c85cf6d750899e5c12a97b4cc72',1795774861165,'relation.add','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','relation',x'01a21c85cf6d7b2abae3db6180e83f9f','{"fields":{"link":{"after":{"from":"01a21c85-cf45-7117-8106-b60cd5ef47b0","kind":"blocks","to":"01a21c85-cf1c-7a76-9efd-d8bdd3b1b79e"}}}}','{"set":{"link":null}}',NULL);
INSERT INTO "activity" VALUES(9,x'01a21c85cf827f7b81a13fab91ad733c',1795774861186,'task.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','{"fields":{"description":{"after":{"revision":"01a21c85-cf82-7fa0-b90b-afe8a634cc67"},"before":{"revision":"01a21c85-cf1c-714d-ae8e-5c1880220bf7"}}}}','{"set":{"description":{"revision":"01a21c85-cf1c-714d-ae8e-5c1880220bf7"}}}',NULL);
INSERT INTO "activity" VALUES(10,x'01a21c85cf947fb7a7bc1cd4a4e8d257',1795774861204,'comment.add','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','comment',x'01a21c85cf947947b11b28422b8f5c53','{"fields":{"body":{"after":"The neighbours like the garden plan."},"created":{"after":true},"task":{"after":"TM-1"}}}','{"set":{"body":null,"created":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(11,x'01a21c85cfaa70ca90238db6aa299b06',1795774861226,'time.log','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','time_entry',x'01a21c85cfaa7e11ba52cf57d1b3c18d','{"fields":{"created":{"after":true},"date":{"after":"2026-11-20"},"minutes":{"after":90},"note":{"after":"First sketch"},"task":{"after":"TM-1"}}}','{"set":{"created":null,"date":null,"minutes":null,"note":null,"task":null}}',NULL);
INSERT INTO "activity" VALUES(12,x'01a21c85cfbe76cbb7dd63df305feed8',1795774861246,'reminder.set','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','{"fields":{"reminder":{"after":{"at":"2026-11-30T09:00:00.000Z"}}}}','{"set":{"reminder":null}}',NULL);
INSERT INTO "activity" VALUES(13,x'01a21c85cfd371d8816acaf266149b98',1795774861267,'task.move','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf4571178106b60cd5ef47b0','{"fields":{"closed_at":{"after":1795774861267},"rank":{"after":"i","before":"i002"},"status":{"after":"01a21c85-cecd-7487-aab2-f40c49d9c8ea","before":"01a21c85-cecd-797c-a97d-fa174302938c"}}}','{"set":{"closed_at":null,"rank":"i002","status":"01a21c85-cecd-797c-a97d-fa174302938c"}}',NULL);
INSERT INTO "activity" VALUES(14,x'01a21c85cfe8705799ddcfb876d2ed78',1795774861288,'timer.start','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','timer',x'01a21c85ce817d00868141b2585279ce','{"fields":{"timer":{"after":{"actor_kind":"user","actor_name":"You","elapsed_ms":0,"note":"","running_since":1795774861288,"started_at":1795774861288,"task":"01a21c85-cf31-7b59-9bfb-d03badf0d356"}}}}','{"set":{"timer":null}}',NULL);
INSERT INTO "activity" VALUES(15,x'01a21c85cffe7208b2ea1fc79f1084cf',1795774861310,'wiki.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85cffe7994b7d230cadacc6ba4','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(16,x'01a21c85d015782aa343f68e789fa3fb',1795774861333,'wiki.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85d0157d25973c9bdb8d425d22','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(17,x'01a21c85d02b7ae698e6ef0f101683a5',1795774861355,'wiki.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85d0157d25973c9bdb8d425d22','{"fields":{"body":{"after":{"revision":"01a21c85-d02b-708f-a15a-3adbb09e6f49"},"before":{"revision":"01a21c85-d015-78a9-920e-ff984c5991f9"}}}}','{"set":{"body":{"revision":"01a21c85-d015-78a9-920e-ff984c5991f9"}}}',NULL);
INSERT INTO "activity" VALUES(18,x'01a21c85d03d79eba5853b725a813092',1795774861373,'wiki.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85cffe7994b7d230cadacc6ba4','{"fields":{"home":{"after":true,"before":false}}}','{"set":{"home":false}}',NULL);
INSERT INTO "activity" VALUES(19,x'01a21c85d0527fae9b12a4422ea24bd2',1795774861394,'wiki.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85d0527d3bbdb5429e4b89ec6d','{"fields":{"created":{"after":true}}}','{"set":{"created":null}}',NULL);
INSERT INTO "activity" VALUES(20,x'01a21c85d068739d9de39b4f145ba130',1795774861416,'wiki.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','wiki_page',x'01a21c85d0527d3bbdb5429e4b89ec6d','{"fields":{"template":{"after":true,"before":false}}}','{"set":{"template":false}}',NULL);
INSERT INTO "activity" VALUES(21,x'01a21c85d07c7c78b88aef393f12dc4b',1795774861436,'version.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','version',x'01a21c85d07c70499c0d442769f71c51','{"fields":{"record":{"after":{"created_at":1795774861436,"description":"","due_date":"2027-03-01","name":"Spring","project":"01a21c85-cecd-7381-8014-ba317af29697","rank":"i","sharing":"none","status":"open","updated_at":1795774861436}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(22,x'01a21c85d0917de8bd607cfb521b36c9',1795774861457,'task.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','{"fields":{"fix_version":{"after":"01a21c85-d07c-7049-9c0d-442769f71c51"}}}','{"set":{"fix_version":null}}',NULL);
INSERT INTO "activity" VALUES(23,x'01a21c85d0a37565a1429b361a2df4b4',1795774861475,'field.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','custom_field',x'01a21c85d0a370c59c2132f7e10ac43a','{"fields":{"created":{"after":true},"name":{"after":"Cost"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(24,x'01a21c85d0b87eda83df0978d5e84ac2',1795774861496,'task.update','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85cf1c7a769efdd8bdd3b1b79e','{"fields":{"cf:01a21c85-d0a3-70c5-9c21-32f7e10ac43a":{"after":12.5}}}','{"set":{"cf:01a21c85-d0a3-70c5-9c21-32f7e10ac43a":null}}',NULL);
INSERT INTO "activity" VALUES(25,x'01a21c85d0cc7577b4ed97c3c8af1c2c',1795774861516,'work_kind.create','user','You','cli',NULL,NULL,'work_kind',x'01a21c85d0cc78acb686b154ea537a87','{"fields":{"record":{"after":{"created_at":1795774861516,"created_by_kind":"user","created_by_name":"You","name":"Digging","updated_at":1795774861516}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(26,x'01a21c85d0e1775f9437ca29577725ea',1795774861537,'time.log','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','time_entry',x'01a21c85d0e17bc68ee15d31eeb3f425','{"fields":{"created":{"after":true},"date":{"after":"2026-11-21"},"minutes":{"after":45},"task":{"after":"TM-1"},"work_kind":{"after":"01a21c85-d0cc-78ac-b686-b154ea537a87"}}}','{"set":{"created":null,"date":null,"minutes":null,"task":null,"work_kind":null}}',NULL);
INSERT INTO "activity" VALUES(27,x'01a21c85d0f572a2bdc03b9c91deec74',1795774861557,'type.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task_type',x'01a21c85d0f573e3b6dee765f4b6d662','{"fields":{"created":{"after":true},"name":{"after":"Epic"}}}','{"set":{"created":null,"name":null}}',NULL);
INSERT INTO "activity" VALUES(28,x'01a21c85d109734b8f8525bb54a284f9',1795774861577,'task.create','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85d1097d6c90acf68b5215efac','{"fields":{"created":{"after":true},"key":{"after":"TM-4"},"status":{"after":"01a21c85-cecd-797c-a97d-fa174302938c"},"title":{"after":"Water the plants"}}}','{"set":{"created":null,"key":null,"status":null,"title":null}}',NULL);
INSERT INTO "activity" VALUES(29,x'01a21c85d11e7b10b5f74de9e93e5d3b',1795774861598,'recurrence.set','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','series',x'01a21c85d11e76fd8b5d03a5cd9dbbc7','{"fields":{"record":{"after":{"created_at":1795774861598,"created_by_kind":"user","created_by_name":"You","current":"01a21c85-d109-7d6c-90ac-f68b5215efac","made":1,"pattern":{"due_in_days":0,"title":"Water the plants","type":"Task"},"project":"01a21c85-cecd-7381-8014-ba317af29697","rule":{"create":"on_close","every":"week","from":"schedule","interval":1,"starts":"2026-12-02","weekdays":["wed"]},"updated_at":1795774861598}}}}','{"set":{"record":null}}',NULL);
INSERT INTO "activity" VALUES(30,x'01a21c85d11e7b10b5f74de9e93e5d3b',1795774861598,'recurrence.set','user','You','cli',NULL,x'01a21c85cecd73818014ba317af29697','task',x'01a21c85d1097d6c90acf68b5215efac','{"fields":{"series":{"after":"01a21c85-d11e-76fd-8b5d-03a5cd9dbbc7"}}}','{"set":{"series":null}}',NULL);
INSERT INTO "search_docs" VALUES(1,'task',x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85cecd73818014ba317af29697','Plan the garden','Beds, paths, a pond and a shed.
measure
sketch
',1795774861186);
INSERT INTO "search_docs" VALUES(2,'task',x'01a21c85cf317b599bfbd03badf0d356',x'01a21c85cf317b599bfbd03badf0d356',x'01a21c85cecd73818014ba317af29697','Buy seeds','',1795774861105);
INSERT INTO "search_docs" VALUES(3,'task',x'01a21c85cf4571178106b60cd5ef47b0',x'01a21c85cf4571178106b60cd5ef47b0',x'01a21c85cecd73818014ba317af29697','Fix the fence','',1795774861125);
INSERT INTO "search_docs" VALUES(4,'task',x'01a21c85cf5a7cd7884c73458c233d19',x'01a21c85cf5a7cd7884c73458c233d19',x'01a21c85cef37bb59f42cb999baed734','Launch the homepage','Ship it soon.
',1795774861146);
INSERT INTO "search_docs" VALUES(5,'comment',x'01a21c85cf947947b11b28422b8f5c53',x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85cecd73818014ba317af29697','Plan the garden','The neighbours like the garden plan.
',1795774861204);
INSERT INTO "search_docs" VALUES(6,'wiki',x'01a21c85cffe7994b7d230cadacc6ba4',NULL,x'01a21c85cecd73818014ba317af29697','Garden notes','The plan is in TM-1. Seeds are on Seeds.
',1795774861310);
INSERT INTO "search_docs" VALUES(7,'wiki',x'01a21c85d0157d25973c9bdb8d425d22',NULL,x'01a21c85cecd73818014ba317af29697','Seeds','Tomatoes, beans and peas.
',1795774861355);
INSERT INTO "search_docs" VALUES(8,'wiki',x'01a21c85d0527d3bbdb5429e4b89ec6d',NULL,x'01a21c85cecd73818014ba317af29697','Meeting notes','Who
What
',1795774861394);
INSERT INTO "search_docs" VALUES(9,'task',x'01a21c85d1097d6c90acf68b5215efac',x'01a21c85d1097d6c90acf68b5215efac',x'01a21c85cecd73818014ba317af29697','Water the plants','',1795774861577);
INSERT INTO "wiki_pages" VALUES(x'01a21c85cffe7994b7d230cadacc6ba4',x'01a21c85cecd73818014ba317af29697',NULL,'i','Garden notes','garden notes','garden-notes',x'01a21c85cffe771aa45fb14b198e5945',0,1795774861310,1795774861373,NULL,NULL,1,0);
INSERT INTO "wiki_pages" VALUES(x'01a21c85d0157d25973c9bdb8d425d22',x'01a21c85cecd73818014ba317af29697',x'01a21c85cffe7994b7d230cadacc6ba4','i','Seeds','seeds','seeds',x'01a21c85d02b708fa15a3adbb09e6f49',0,1795774861333,1795774861355,NULL,NULL,0,0);
INSERT INTO "wiki_pages" VALUES(x'01a21c85d0527d3bbdb5429e4b89ec6d',x'01a21c85cecd73818014ba317af29697',NULL,'i001','Meeting notes','meeting notes','meeting-notes',x'01a21c85d052786195043ece2a1864c2',0,1795774861394,1795774861416,NULL,NULL,0,1);
INSERT INTO "wiki_revisions" VALUES(x'01a21c85cffe771aa45fb14b198e5945',x'01a21c85cffe7994b7d230cadacc6ba4','The plan is in TM-1. Seeds are on [[Seeds]].',NULL,'normal',NULL,'user','You',NULL,1795774861310,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21c85d01578a9920eff984c5991f9',x'01a21c85d0157d25973c9bdb8d425d22','Tomatoes and beans.',NULL,'normal',NULL,'user','You',NULL,1795774861333,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21c85d02b708fa15a3adbb09e6f49',x'01a21c85d0157d25973c9bdb8d425d22','Tomatoes, beans and peas.',x'01a21c85d01578a9920eff984c5991f9','normal',NULL,'user','You',NULL,1795774861355,NULL);
INSERT INTO "wiki_revisions" VALUES(x'01a21c85d052786195043ece2a1864c2',x'01a21c85d0527d3bbdb5429e4b89ec6d','## Who

## What',NULL,'normal',NULL,'user','You',NULL,1795774861394,NULL);
INSERT INTO "reminders" VALUES(x'01a21c85cfbe7761a3b835f81c367ac6',x'01a21c85cf1c7a769efdd8bdd3b1b79e',1796029200000,NULL,NULL,'','user','You',1795774861246,NULL,NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21c85cffe7994b7d230cadacc6ba4',0,'wiki','Seeds',NULL,'seeds','seeds','','',NULL);
INSERT INTO "text_links" VALUES('wiki',x'01a21c85cffe7994b7d230cadacc6ba4',1,'task','TM-1',NULL,'','','','',1);
INSERT INTO "custom_fields" VALUES(x'01a21c85d0a370c59c2132f7e10ac43a',x'01a21c85cecd73818014ba317af29697','Cost','cost','cost','decimal','[]',NULL,'[]',0,'i',1795774861475);
INSERT INTO "custom_values" VALUES(x'01a21c85cf1c7a769efdd8bdd3b1b79e',x'01a21c85d0a370c59c2132f7e10ac43a',12.5);
INSERT INTO "versions" VALUES(x'01a21c85d07c70499c0d442769f71c51',x'01a21c85cecd73818014ba317af29697','Spring','spring','','open',NULL,'2027-03-01','none','i',1795774861436,1795774861436);
INSERT INTO "task_series" VALUES(x'01a21c85d11e76fd8b5d03a5cd9dbbc7',x'01a21c85cecd73818014ba317af29697','{"every":"week","interval":1,"weekdays":["wed"],"starts":"2026-12-02","from":"schedule","create":"on_close"}','{"title":"Water the plants","type":"Task","due_in_days":0}',x'01a21c85d1097d6c90acf68b5215efac',NULL,1,NULL,1795774861598,'user','You',1795774861598);
INSERT INTO "work_kinds" VALUES(x'01a21c85d0cc78acb686b154ea537a87','Digging','digging',NULL,1795774861516,'user','You',1795774861516);
INSERT INTO "task_types" VALUES(x'01a21c85cecd7d969159587ffa3c2b94',x'01a21c85cecd73818014ba317af29697','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cecd7c4f962a8654fafb5d2b',x'01a21c85cecd73818014ba317af29697','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cecd7be38aaa40656b99a879',x'01a21c85cecd73818014ba317af29697','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cecd7280b489358ccfcde6c7',x'01a21c85cecd73818014ba317af29697','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cef378a19d5a0bf04d713c7e',x'01a21c85cef37bb59f42cb999baed734','Task','task','task','blue','7777',1,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cef376c98863ff46986a290e',x'01a21c85cef37bb59f42cb999baed734','Bug','bug','bug','red','eeee',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cef37a099293b38e387c41aa',x'01a21c85cef37bb59f42cb999baed734','Feature','feature','feature','green','llll',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85cef37580973593f518abb274',x'01a21c85cef37bb59f42cb999baed734','Milestone','milestone','milestone','purple','ssss',0,NULL);
INSERT INTO "task_types" VALUES(x'01a21c85d0f573e3b6dee765f4b6d662',x'01a21c85cecd73818014ba317af29697','Epic','epic','epic','grey','ssst',0,NULL);
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
INSERT INTO "fts_words"("fts_words") VALUES('rebuild');
INSERT INTO "fts_grams"("fts_grams") VALUES('rebuild');
PRAGMA user_version = 20;
COMMIT;
