// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The engine: opens, locks and migrates workspaces, executes operations,
//! keeps the activity log, undo, redo and revert, publishes change events, and
//! owns files, backups, export, import and background jobs.
