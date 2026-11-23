-- SPDX-License-Identifier: GPL-3.0-or-later
-- Copyright (C) 2026 Glyphkeel contributors
--
-- Schema version 5: a project's icon is one of a fixed set of names, drawn
-- in the project's colour instead of its circle; no icon means the circle.
-- Earlier versions stored any text there. Names outside the set, and blank
-- ones, become no icon. The program checks new values against the same
-- set, so it can grow without a migration.

UPDATE projects
   SET icon = NULL
 WHERE icon IS NOT NULL
   AND icon NOT IN (
  'folder', 'star', 'bug', 'rocket', 'book', 'house', 'briefcase', 'code', 'terminal',
  'globe', 'heart', 'flag', 'zap', 'target', 'lightbulb', 'wrench', 'hammer',
  'palette', 'music', 'camera', 'graduation-cap', 'shopping-cart', 'wallet',
  'chart-line', 'calendar', 'users', 'building', 'plane', 'leaf', 'coffee', 'gamepad',
  'puzzle', 'shield', 'flask-conical', 'layers', 'package', 'compass', 'mountain',
  'sparkles', 'database'
   );
