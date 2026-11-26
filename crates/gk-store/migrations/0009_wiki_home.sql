-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 9: a project's wiki can have a home page, the page the
-- wiki opens on. At most one page per project is the home page; a page that
-- goes to the trash stops being it. Without one, the wiki opens on its
-- first page.
--
-- Files attached to wiki pages need no change: attachments already name
-- their owner as ('wiki_page', page ID).

ALTER TABLE wiki_pages ADD COLUMN is_home INTEGER NOT NULL DEFAULT 0 CHECK (is_home IN (0, 1));
CREATE UNIQUE INDEX wiki_pages_one_home ON wiki_pages (project_id) WHERE is_home = 1;
