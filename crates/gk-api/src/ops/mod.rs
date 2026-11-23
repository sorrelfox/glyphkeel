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
pub mod common;
pub mod history;
pub mod settings;
pub mod workspace;
