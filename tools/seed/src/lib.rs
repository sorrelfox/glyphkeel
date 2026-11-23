// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Deterministic test data for Glyphkeel.
//!
//! The seed tool writes a data set as an unpacked export archive: the same
//! folder layout and records that the app's export uses, so the engine can
//! load it through its ordinary import. The same set and seed always give
//! byte-identical files. See `README.md` next to this crate for the layout.
//!
//! - **S**: three projects (one with two sub-projects), 200 tasks, 500
//!   comments, 50 files and 10 wiki pages. For function tests.
//! - **L**: one project with 20,000 tasks (1,000 of them open), 50,000
//!   comments, 2,000 files, 500 tasks with dates, 300 dependencies and 300
//!   wiki pages. For performance tests.
//! - **hostile**: Markdown, formulas, diagrams, SVG and names made to break
//!   the renderer, the sanitizer and the layout, with what must hold for each
//!   in `fixtures.json`.

pub mod archive;
pub mod blobs;
pub mod clock;
mod generate;
pub mod hostile;
pub mod model;
pub mod rng;
pub mod text;

pub use archive::Archive;

/// A data set the tool can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSet {
    Small,
    Large,
    Hostile,
}

impl DataSet {
    /// The name used on the command line and in the manifest.
    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "S",
            Self::Large => "L",
            Self::Hostile => "hostile",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "S" | "s" => Some(Self::Small),
            "L" | "l" => Some(Self::Large),
            "hostile" => Some(Self::Hostile),
            _ => None,
        }
    }
}

/// Builds a data set in memory.
pub fn generate(set: DataSet, seed: u64) -> anyhow::Result<Archive> {
    match set {
        DataSet::Small => generate::build(set.name(), seed, &generate::small()),
        DataSet::Large => generate::build(set.name(), seed, &generate::large()),
        DataSet::Hostile => hostile::build(seed),
    }
}
