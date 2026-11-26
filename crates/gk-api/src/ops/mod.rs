// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The operations, by area, with their input and output types.
//!
//! Inputs refuse unknown fields, so a misspelt field is an error rather than
//! a silent no-op. Outputs leave absent values out instead of writing `null`.

/// Declares an operation type: `op!(Name, "area.verb", Input, Output)`.
macro_rules! op {
    ($(#[$doc:meta])* $name:ident, $id:literal, $input:ty, $output:ty) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $crate::catalog::Operation for $name {
            const ID: &'static str = $id;
            type Input = $input;
            type Output = $output;
        }
    };
}

pub mod agent;
pub mod backup;
pub mod board;
pub mod bulk;
pub mod comment;
pub mod common;
pub mod description;
pub mod draft;
pub mod file;
pub mod history;
pub mod link;
pub mod project;
pub mod query;
pub mod rules;
pub mod schedule;
pub mod search;
pub mod settings;
pub mod task;
pub mod time;
pub mod today;
pub mod transfer;
pub mod trash;
pub mod view;
pub mod wiki;
pub mod workspace;
