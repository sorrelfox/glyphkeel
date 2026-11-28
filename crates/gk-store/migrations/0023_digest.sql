-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 23: the "while you were away" digest.
--
-- `personal_marks` keeps moments that belong to one person, not to the
-- workspace: today only `digest_seen`, the moment up to which the person
-- has seen what agents, automation rules and other programs changed. It
-- moves when the person dismisses the digest. Every row names its person;
-- there are no accounts yet, so the person at this computer is `me`, and
-- a workspace shared by several people later gives each their own rows.
-- Marks are not history: moving one changes no data and can't be undone.
--
-- The digest reads history by time, so `activity` gets an index on `at`.

CREATE TABLE personal_marks (
  person TEXT NOT NULL CHECK (length(person) BETWEEN 1 AND 100),
  mark   TEXT NOT NULL CHECK (mark IN ('digest_seen')),
  at     INTEGER NOT NULL,
  PRIMARY KEY (person, mark)
) STRICT, WITHOUT ROWID;

CREATE INDEX activity_at ON activity (at);
