// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Links between tasks: how a link as one task sees it ("blocked by TM-4")
//! is stored, and how a stored row reads from either end.
//!
//! A row is stored once: "A blocks B" and "B is blocked by A" are the same
//! row read from its two ends, as are "duplicates"/"duplicated by" and
//! "clones"/"cloned by". "Relates" has no direction; its row puts the
//! smaller ID first, so a pair has only one.

use std::fmt;
use std::str::FromStr;

use crate::id::Id;

/// The kind of a stored relation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relation {
    /// `from` must be finished before `to` can be (finish to start).
    Blocks,
    /// The two tasks are related; no direction.
    Relates,
    /// `from` is a duplicate of `to`.
    Duplicates,
    /// `from` was made as a copy of `to`.
    Clones,
}

impl Relation {
    /// The name stored in the database.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Relates => "relates",
            Self::Duplicates => "duplicates",
            Self::Clones => "clones",
        }
    }
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Relation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "blocks" => Self::Blocks,
            "relates" => Self::Relates,
            "duplicates" => Self::Duplicates,
            "clones" => Self::Clones,
            other => return Err(format!("\"{other}\" is not a relation")),
        })
    }
}

/// A link as one task sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkKind {
    /// This task blocks the other.
    Blocks,
    /// The other task blocks this one.
    BlockedBy,
    /// The two are related.
    Relates,
    /// This task duplicates the other.
    Duplicates,
    /// The other task duplicates this one.
    DuplicatedBy,
    /// This task is a copy of the other.
    Clones,
    /// The other task is a copy of this one.
    ClonedBy,
}

impl LinkKind {
    /// The stored row for a link of this kind from `this` to `other`:
    /// `(relation, from, to)`.
    #[must_use]
    pub fn stored(self, this: Id, other: Id) -> (Relation, Id, Id) {
        match self {
            Self::Blocks => (Relation::Blocks, this, other),
            Self::BlockedBy => (Relation::Blocks, other, this),
            Self::Duplicates => (Relation::Duplicates, this, other),
            Self::DuplicatedBy => (Relation::Duplicates, other, this),
            Self::Clones => (Relation::Clones, this, other),
            Self::ClonedBy => (Relation::Clones, other, this),
            Self::Relates => {
                if this.as_bytes() <= other.as_bytes() {
                    (Relation::Relates, this, other)
                } else {
                    (Relation::Relates, other, this)
                }
            }
        }
    }

    /// How the stored row `relation` from `from` to `to` reads from
    /// `this` end, and the task at the other end; `None` when `this` is
    /// neither end.
    #[must_use]
    pub fn seen_from(relation: Relation, from: Id, to: Id, this: Id) -> Option<(Self, Id)> {
        let forward = this == from;
        if !forward && this != to {
            return None;
        }
        let other = if forward { to } else { from };
        let kind = match (relation, forward) {
            (Relation::Blocks, true) => Self::Blocks,
            (Relation::Blocks, false) => Self::BlockedBy,
            (Relation::Relates, _) => Self::Relates,
            (Relation::Duplicates, true) => Self::Duplicates,
            (Relation::Duplicates, false) => Self::DuplicatedBy,
            (Relation::Clones, true) => Self::Clones,
            (Relation::Clones, false) => Self::ClonedBy,
        };
        Some((kind, other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_and_its_reverse_are_one_row() {
        let (a, b) = (Id::generate(), Id::generate());
        assert_eq!(
            LinkKind::Blocks.stored(a, b),
            LinkKind::BlockedBy.stored(b, a)
        );
        assert_eq!(
            LinkKind::Duplicates.stored(a, b),
            LinkKind::DuplicatedBy.stored(b, a)
        );
        assert_eq!(
            LinkKind::Clones.stored(a, b),
            LinkKind::ClonedBy.stored(b, a)
        );
        assert_eq!(
            LinkKind::Relates.stored(a, b),
            LinkKind::Relates.stored(b, a)
        );
        let (_, first, _) = LinkKind::Relates.stored(b, a);
        assert!(first.as_bytes() <= a.as_bytes().max(b.as_bytes()));
        assert_eq!(first.as_bytes(), a.as_bytes().min(b.as_bytes()));
    }

    #[test]
    fn rows_read_from_either_end() {
        let (a, b, c) = (Id::generate(), Id::generate(), Id::generate());
        let kinds = [
            LinkKind::Blocks,
            LinkKind::BlockedBy,
            LinkKind::Relates,
            LinkKind::Duplicates,
            LinkKind::DuplicatedBy,
            LinkKind::Clones,
            LinkKind::ClonedBy,
        ];
        for kind in kinds {
            let (relation, from, to) = kind.stored(a, b);
            assert_eq!(LinkKind::seen_from(relation, from, to, a), Some((kind, b)));
            assert_eq!(LinkKind::seen_from(relation, from, to, c), None);
            assert_eq!(relation.as_str().parse::<Relation>(), Ok(relation));
        }
        let (relation, from, to) = LinkKind::Blocks.stored(a, b);
        assert_eq!(
            LinkKind::seen_from(relation, from, to, b),
            Some((LinkKind::BlockedBy, a))
        );
    }
}
