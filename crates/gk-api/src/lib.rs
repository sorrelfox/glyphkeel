// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The operation catalog: every operation's input and output types with their
//! JSON Schemas, its metadata (name, summary, where it is exposed, how risky it
//! is) and the common error type. It holds no logic.
//!
//! The desktop app, the MCP server and the command line all reach the engine
//! through this catalog, so they offer the same operations.
//!
//! # Shape of an operation
//!
//! An operation is a type implementing [`Operation`]: an `area.verb` name, an
//! input type (decoded with serde, unknown fields refused) and an output type.
//! Its [`OpDescriptor`] in [`CATALOG`] carries the metadata every adapter
//! needs: the [`Safety`] class, the [`Exposure`] (with the written reason
//! when an operation is kept from agents), the command-line command and the
//! JSON Schemas.
//!
//! # Absent values in outputs
//!
//! An output never contains `null`: a value that is absent is left out. The
//! output schemas therefore never carry a `null` type, which keeps them
//! portable for MCP clients that validate results, and the generated
//! TypeScript types read `field?: T`. Inputs are different: in a patch,
//! `null` means "clear this field". [`schema::output_has_null`] checks the
//! rule, and the catalog's tests run it over every output.

pub mod catalog;
pub mod error;
pub mod event;
pub mod mcp;
pub mod ops;
pub mod profile;
pub mod prompts;
pub mod schema;

pub use catalog::{CATALOG, Exposure, OpDescriptor, OpKind, Operation, Safety, descriptor};
pub use error::{ErrorCode, OpError};
pub use event::{Changed, EntityChange, Origin};
pub use mcp::{McpTool, TOOLS, Toolset};
pub use profile::Profile;
