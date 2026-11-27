// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// One CSV cell, written the way the engine writes its own CSV exports.

/**
 * A CSV cell (RFC 4180). A text cell a spreadsheet would run as a formula
 * (it starts with `=`, `+`, `-`, `@`, a tab or a carriage return) gets a
 * leading apostrophe; whole numbers stay as they are. A cell holding a
 * separator, a quote or a line break is quoted.
 */
export function csvCell(cell: string): string {
  const guarded = !/^\d+$/.test(cell) && /^[=+\-@\t\r]/.test(cell) ? `'${cell}` : cell;
  return /[",\n\r]/.test(guarded) ? `"${guarded.replace(/"/g, '""')}"` : guarded;
}
