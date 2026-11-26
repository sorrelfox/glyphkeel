-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 13: rules. A status can carry rules for tasks entering
-- it: fields set or cleared on the way in, the timer stopped, an agent's
-- claim released, fields that must have a value first, fields that can't
-- change while a task is in it, and whether agents may move tasks into it.
-- A project can name task fields agents may not change. Both are JSON
-- objects; no rules is NULL. Fields are named by their API name, or a
-- custom field by its ID. A new sub-project copies its parent's rules with
-- the statuses.

ALTER TABLE statuses ADD COLUMN rules TEXT
    CHECK (rules IS NULL OR (json_valid(rules) AND json_type(rules) = 'object'));

ALTER TABLE projects ADD COLUMN agent_rules TEXT
    CHECK (agent_rules IS NULL OR (json_valid(agent_rules) AND json_type(agent_rules) = 'object'));
