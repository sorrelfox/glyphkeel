// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Secrets: the HTTP endpoint's bearer token and the request states of
//! confirmations, from the system's random source.

/// Fills `bytes` from the system's random source.
///
/// # Panics
/// When the system has no random source, which no supported system lacks;
/// a predictable secret would be worse than stopping.
pub(crate) fn fill(bytes: &mut [u8]) {
    getrandom::fill(bytes).unwrap_or_else(|e| panic!("the system's random source failed: {e}"));
}

/// `bytes` as lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// A new bearer token for the HTTP endpoint: 256 random bits as 64 hex
/// digits.
#[must_use]
pub fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    fill(&mut bytes);
    hex(&bytes)
}

/// Whether `a` and `b` are equal, in time that doesn't depend on where
/// they differ.
#[must_use]
pub(crate) fn same_secret(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_hex_and_differ() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(same_secret(a.as_bytes(), a.as_bytes()));
        assert!(!same_secret(a.as_bytes(), b.as_bytes()));
        assert!(!same_secret(b"ab", b"abc"));
    }
}
