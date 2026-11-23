-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 4: a timer that can be paused, and personal reminders that
-- are either a moment or a number of days before the task's due date.

-- The timer ------------------------------------------------------------------
--
-- `started_at` is when the timer was first started (its entry's day comes
-- from it). Time from earlier running stretches is in `elapsed_ms`; while it
-- runs, `running_since` is when the current stretch began, and it is NULL
-- while the timer is paused. `reminded_at` is when the person acknowledged
-- the reminder about a long-running timer.
ALTER TABLE timer ADD COLUMN running_since INTEGER;
ALTER TABLE timer ADD COLUMN elapsed_ms INTEGER NOT NULL DEFAULT 0 CHECK (elapsed_ms >= 0);
ALTER TABLE timer ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE timer ADD COLUMN reminded_at INTEGER;
UPDATE timer SET running_since = started_at;

-- Personal reminders -----------------------------------------------------------
--
-- At most one per task. Either `remind_at` (a moment) or `days_before_due`
-- is set; the second fires that many days before the due date, at
-- `time_of_day` (minutes after midnight, local time) or else at the
-- workspace's reminder time. `acked_moment` is the moment that was
-- acknowledged, so a reminder whose moment moves (a new due date) fires
-- again.
CREATE TABLE reminders_v4 (
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

-- Version 3 allowed several reminders per task; the latest one is kept.
INSERT INTO reminders_v4 (id, task_id, remind_at, actor_kind, actor_name, created_at,
                          acked_at, acked_moment)
SELECT r.id, r.task_id, r.remind_at, 'user', 'You', r.remind_at,
       coalesce(r.dismissed_at, r.fired_at),
       CASE WHEN coalesce(r.dismissed_at, r.fired_at) IS NULL THEN NULL ELSE r.remind_at END
  FROM reminders r
 WHERE r.id = (SELECT max(o.id) FROM reminders o WHERE o.task_id = r.task_id);

DROP TABLE reminders;
ALTER TABLE reminders_v4 RENAME TO reminders;
CREATE INDEX reminders_at ON reminders (remind_at) WHERE remind_at IS NOT NULL;
