// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The operation catalog: every operation's input and output types with their
//! JSON Schemas, its metadata (name, summary, where it is exposed, how risky it
//! is) and the common error type. It holds no logic.
//!
//! The desktop app, the MCP server and the command line all reach the engine
//! through this catalog, so they offer the same operations.
