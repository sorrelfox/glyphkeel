-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- The text inside attached files, for search. One row per stored file
-- content (a blob), written by the background extraction: the text, or
-- why there is none. A blob without a row, or with a row of an older
-- extraction `version`, is waiting to be read; files attached before this
-- version are read by the same background job, so nothing is done here.
--
-- The text also goes into the file's search document (`search_docs.body`,
-- kind 'file'), one per attachment, so search finds it with its snippet;
-- the copy here puts it back when an attachment comes back from the trash
-- or the same content is attached again, without reading the file again.
-- Both live only in the database, which an encrypted workspace encrypts:
-- there is no other copy of the text on disk.
--
-- `outcome`: 'text' (the text, perhaps cut off: `truncated`), 'none' (a
-- kind that is read but gave no text, for the reason in `reason`) or
-- 'unsupported' (a kind that is not read; found by its name only).

CREATE TABLE file_texts (
  sha256       TEXT PRIMARY KEY CHECK (length(sha256) = 64),
  version      INTEGER NOT NULL CHECK (version >= 0),
  outcome      TEXT NOT NULL CHECK (outcome IN ('text', 'none', 'unsupported')),
  reason       TEXT,
  text         TEXT NOT NULL DEFAULT '' CHECK (octet_length(text) <= 1048576),
  truncated    INTEGER NOT NULL DEFAULT 0 CHECK (truncated IN (0, 1)),
  extracted_at INTEGER NOT NULL
) STRICT;

-- The text goes with the stored file.
CREATE TRIGGER file_texts_with_blob AFTER DELETE ON blobs BEGIN
  DELETE FROM file_texts WHERE sha256 = old.sha256;
END;

-- A task's documents (`has:attachment-text`, the trash).
CREATE INDEX search_docs_task ON search_docs (task_id, kind) WHERE task_id IS NOT NULL;
