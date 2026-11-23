// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Identifiers: UUIDv7, stored as 16 bytes and shown as lower-case hyphenated
//! text.
//!
//! Every stored entity has one. They are kept although nothing syncs: archives
//! from two workspaces stay mergeable, and the IDs sort roughly by creation
//! time, which keeps the database's primary-key indexes compact.

use std::fmt;
use std::str::FromStr;

use uuid::{NoContext, Timestamp, Uuid};

/// A UUIDv7 identifier.
///
/// IDs made by [`Id::generate`] increase within one process, even within the
/// same millisecond.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(Uuid);

/// Why text or bytes are not an [`Id`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    /// The text is not a UUID.
    #[error("\"{0}\" is not an ID: expected 32 hexadecimal digits, optionally hyphenated")]
    Malformed(String),
    /// The bytes are not 16 long.
    #[error("an ID is 16 bytes, not {0}")]
    WrongLength(usize),
}

impl Id {
    /// A new ID for the current moment.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// A new ID whose time part is `unix_ms` (milliseconds since 1970 UTC); the
    /// rest is random. For callers that use an injected clock.
    #[must_use]
    pub fn generate_at(unix_ms: u64) -> Self {
        let ts = Timestamp::from_unix(
            NoContext,
            unix_ms / 1000,
            u32::try_from((unix_ms % 1000) * 1_000_000).unwrap_or(0),
        );
        Self(Uuid::new_v7(ts))
    }

    /// The ID with these 16 bytes, as stored in the database.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// The ID stored in `bytes`, which must be exactly 16 long.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, IdError> {
        let array: [u8; 16] = bytes
            .try_into()
            .map_err(|_| IdError::WrongLength(bytes.len()))?;
        Ok(Self::from_bytes(array))
    }

    /// The 16 bytes, as stored in the database.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Whether this is a version 7 UUID. IDs from other sources (imports)
    /// are accepted as they are, so this is not guaranteed.
    #[must_use]
    pub fn is_v7(&self) -> bool {
        self.0.get_version_num() == 7
    }

    /// The moment encoded in a version 7 ID, in milliseconds since 1970 UTC.
    #[must_use]
    pub fn unix_ms(&self) -> Option<u64> {
        if !self.is_v7() {
            return None;
        }
        let b = self.as_bytes();
        Some(
            b[..6]
                .iter()
                .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte)),
        )
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0.hyphenated())
    }
}

impl FromStr for Id {
    type Err = IdError;

    /// Parses hyphenated or plain hexadecimal text, in either case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.len() != 32 && trimmed.len() != 36 {
            return Err(IdError::Malformed(s.to_owned()));
        }
        Uuid::try_parse(trimmed)
            .map(Self)
            .map_err(|_| IdError::Malformed(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_v7_and_increase() {
        let ids: Vec<Id> = (0..1000).map(|_| Id::generate()).collect();
        assert!(ids.iter().all(Id::is_v7));
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn time_part_round_trips() {
        let ms = 1_767_603_600_123;
        let id = Id::generate_at(ms);
        assert_eq!(id.unix_ms(), Some(ms));
    }

    #[test]
    fn text_and_bytes_round_trip() {
        let id = Id::generate();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text, text.to_lowercase());
        assert_eq!(text.parse::<Id>(), Ok(id));
        assert_eq!(text.replace('-', "").to_uppercase().parse::<Id>(), Ok(id));
        assert_eq!(Id::from_slice(id.as_bytes()), Ok(id));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!("".parse::<Id>().is_err());
        assert!("0193-not-an-id".parse::<Id>().is_err());
        assert!(
            "{01936e2c-7d2b-7cc4-8f5e-6a1b2c3d4e5f}"
                .parse::<Id>()
                .is_err()
        );
        assert_eq!(Id::from_slice(&[0; 15]), Err(IdError::WrongLength(15)));
    }

    #[test]
    fn byte_order_matches_text_order() {
        let a = Id::generate_at(1_000);
        let b = Id::generate_at(2_000);
        assert!(a < b);
        assert!(a.to_string() < b.to_string());
        assert!(a.as_bytes() < b.as_bytes());
    }
}
