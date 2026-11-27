-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 17: the image a task's board card shows as its cover.
--
-- The cover is one of the task's own attached images. Like
-- `tasks.fix_version_id`, the reference has no foreign key: an attachment
-- moves to the trash and comes back, and the engine shows a cover only
-- while its file is live and attached to the task. How a board colours its
-- cards and whether it shows covers is part of the board layout in
-- `view_settings`, which needs no change.

ALTER TABLE tasks ADD COLUMN cover_id BLOB CHECK (cover_id IS NULL OR length(cover_id) = 16);
