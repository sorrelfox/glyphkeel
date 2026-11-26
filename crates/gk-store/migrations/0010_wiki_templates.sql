-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 10: a wiki page can be marked as a template, which the app
-- offers when a new page is made. Any number of pages per project can be
-- templates; the mark stays while a page is in the trash.

ALTER TABLE wiki_pages ADD COLUMN is_template INTEGER NOT NULL DEFAULT 0
    CHECK (is_template IN (0, 1));
