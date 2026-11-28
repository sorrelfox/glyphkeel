-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 24: changes agents propose for the person's approval.
-- When the workspace setting `agent_approvals` covers an agent's change,
-- the change is kept here instead of being made: the operation and its
-- input, the agent session, a summary and a preview of the fields it would
-- change, each before and after. Approving runs the operation as the agent
-- and stores the history group of the change; rejecting stores the reason.
-- An operation that doesn't run in one transaction (revert, purge, files,
-- backups, imports) is claimed first: `approving`, with the time, by one
-- conditional update, so only one program runs it; it becomes `approved`
-- after it ran, or `pending` again with `last_error` when it failed. One
-- left `approving` by a program that stopped stays so, for the person to see.
-- A waiting proposal expires after the setting's number of days; it stays
-- here, as the agent session's record.
--
-- The decision names its actor by kind and name, as every other record
-- does, so a workspace shared by several people could tell them apart.

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

CREATE INDEX proposals_waiting ON proposals (created_at) WHERE status = 'pending';
CREATE INDEX proposals_session ON proposals (session_id, created_at);
CREATE INDEX proposals_agent ON proposals (agent_name, created_at);
