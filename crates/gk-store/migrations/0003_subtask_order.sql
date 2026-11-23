-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 3: subtasks keep an order of their own under their parent,
-- apart from their places on the board.

-- A task's place among its parent's subtasks, as a rank like `rank`. Set
-- while the task has a parent, NULL otherwise.
ALTER TABLE tasks ADD COLUMN subtask_rank TEXT
  CHECK (subtask_rank IS NULL OR (subtask_rank <> '' AND subtask_rank NOT GLOB '*[^0-9a-z]*'));

-- Existing subtasks keep the order they were shown in: by status, then by
-- their place in the column. The k-th subtask gets k in three base-36
-- digits followed by "i", so the ranks increase with k and never end in 0.
WITH ordered AS (
  SELECT t.id AS id,
         row_number() OVER (PARTITION BY t.parent_id ORDER BY s.rank, t.rank, t.id) AS k
    FROM tasks t JOIN statuses s ON s.id = t.status_id
   WHERE t.parent_id IS NOT NULL
)
UPDATE tasks
   SET subtask_rank =
         substr('0123456789abcdefghijklmnopqrstuvwxyz', (ordered.k / 1296) % 36 + 1, 1)
      || substr('0123456789abcdefghijklmnopqrstuvwxyz', (ordered.k / 36) % 36 + 1, 1)
      || substr('0123456789abcdefghijklmnopqrstuvwxyz', ordered.k % 36 + 1, 1)
      || 'i'
  FROM ordered
 WHERE tasks.id = ordered.id;

CREATE INDEX tasks_subtasks ON tasks (parent_id, subtask_rank)
  WHERE parent_id IS NOT NULL AND deleted_at IS NULL;
