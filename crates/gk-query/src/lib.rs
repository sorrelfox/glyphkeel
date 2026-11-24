// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The task query language: lexer, parser, printer, syntax tree, field
//! registry, relative dates and the SQL it compiles to.
//!
//! A query runs in four steps: [`parse`] the text into a syntax tree with
//! spans; [`check`] it against the field registry, which also evaluates
//! relative dates; let the engine look up the names it uses
//! ([`Checked::names`]); and build its SQL ([`Compiled`]). The parser and
//! the checker need no workspace, so the query field can use them on every
//! keystroke.

pub mod check;
pub mod chips;
pub mod dates;
pub mod error;
pub mod fields;
pub mod parse;
pub mod print;
pub mod rename;
pub mod sql;
pub mod syntax;

pub use check::{CheckEnv, Checked, NameKind, NameUse, check};
pub use chips::{Chip, ChipKind, chips};
pub use dates::{Calendar, FixedCalendar, parse_natural, parse_query_date};
pub use error::QueryError;
pub use parse::{parse, parse_with_tokens};
pub use print::print;
pub use sql::{Compiled, Param, Scope, Sql, SqlEnv};
pub use syntax::{Query, Span, Token, TokenKind};

/// Parses and checks `src` in one step.
///
/// # Errors
/// The first mistake: in the text, or a field, operator or value that
/// doesn't fit.
pub fn compile(src: &str, env: CheckEnv<'_>) -> Result<(Query, Checked), QueryError> {
    let query = parse(src)?;
    let checked = check(&query, env)?;
    Ok((query, checked))
}
