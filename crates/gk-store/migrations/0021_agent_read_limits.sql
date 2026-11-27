-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 21: limits on how fast agents read. Each agent session
-- counts the reads its limits refused, next to its reads and writes, so the
-- person sees which agent read more than its limits allow.

ALTER TABLE agent_sessions ADD COLUMN rate_limited INTEGER NOT NULL DEFAULT 0;

-- A workspace whose person changed other limits keeps them in the setting;
-- a limit left out of it means no limit, so the new read limits are added
-- with their defaults, as a workspace without the setting has them.
UPDATE settings
   SET value = json_set(value, '$.read_burst', 60, '$.reads_per_minute', 600,
                        '$.read_mb_per_minute', 8)
 WHERE key = 'agent_limits' AND json_type(value) = 'object';
