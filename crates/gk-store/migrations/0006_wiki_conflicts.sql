-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 6: wiki pages are saved like descriptions. An edit that
-- clashes with another is kept as a conflict copy until someone resolves
-- it, so wiki revisions record when that happened, as description
-- revisions do.

ALTER TABLE wiki_revisions ADD COLUMN resolved_at INTEGER;
