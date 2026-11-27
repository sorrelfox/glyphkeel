-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 17: automation rules. A rule says what happens when
-- something happens to a task: a trigger (a task created, a field changed,
-- a task moved to a status, a comment added, a due date reached or passed,
-- a time of day or week), an optional query the task must match, and the
-- actions to take. Rules belong to a project, or to the whole workspace
-- when `project_id` is NULL.
--
-- A rule's actions are changes like any other, made by the rule itself:
-- history names it as the actor, of the new kind 'automation'. Every table
-- that records who did something takes that kind too. SQLite can't change a
-- CHECK constraint in place; widening a list of allowed values keeps every
-- existing row valid and the stored format the same, so the table
-- definitions are rewritten where they are kept, and the schema read again.
--
-- The rule as history records it is kept in its columns (one `record`
-- field in history, so creating, changing and deleting one can be undone).
-- Beside it, outside history: when a scheduled rule last ran, and how many
-- runs it had in the current hour (the rate limit).
--
-- `automation_runs` is the run log: the last runs of each rule, done or
-- failed, with the error. `automation_fired` remembers which tasks a due
-- date rule already acted on, for which due date, so each acts once.
-- `automation_notices` are the notifications rules leave; the app shows
-- them with the reminders. None of these three is history.

PRAGMA writable_schema = ON;

UPDATE sqlite_schema
   SET sql = replace(sql, 'IN (''user'', ''agent'')', 'IN (''user'', ''agent'', ''automation'')')
 WHERE type = 'table' AND sql LIKE '%IN (''user'', ''agent'')%';

PRAGMA writable_schema = RESET;

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
CREATE UNIQUE INDEX automation_rules_name
    ON automation_rules (coalesce(project_id, x''), name_fold);
CREATE INDEX automation_rules_trigger ON automation_rules (trigger_kind) WHERE enabled = 1;

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
CREATE INDEX automation_runs_rule ON automation_runs (rule_id, at);
CREATE INDEX automation_runs_at ON automation_runs (at);

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
CREATE INDEX automation_notices_open ON automation_notices (at) WHERE acked_at IS NULL;
