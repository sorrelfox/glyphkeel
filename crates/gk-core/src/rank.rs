// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Ranks: fractional indexes that order items in a list (tasks in a board
//! column, statuses, projects in the sidebar).
//!
//! A rank is a string of lower-case base-36 digits (`0`–`9`, `a`–`z`), read
//! as the digits of a fraction between 0 and 1 after the point: `"i"` is
//! 18/36 = 0.5. Ranks are compared as plain byte strings, which is what
//! SQLite's default `BINARY` collation does, so `ORDER BY rank` is the list's
//! order. Moving one item changes only that item's rank.
//!
//! Ranks made here never end in `0`, so a new rank fits between any two
//! ranks that differ in value. Ranks read from elsewhere (imports) may end in
//! `0`: the seed tool writes seven digits such as `"0001000"`. Two ranks with
//! the same value (`"0001"` and `"0001000"`) leave no room between them;
//! [`Rank::between`] reports that, and the caller re-spreads the list with
//! [`Rank::spread`].
//!
//! Appending at the end (or the start) adds a fixed step of 36⁻⁴ to the
//! four-digit prefix, matching the seed tool's spacing, so about 840,000
//! appends after [`Rank::first`] stay at four digits. Inserting again and
//! again at the same place adds a digit about every five inserts;
//! [`Rank::needs_respread`] says when a list should be re-spread.

use std::fmt;
use std::str::FromStr;

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: u8 = 36;
/// Digits of the fixed step used by [`Rank::after`] and [`Rank::before`].
const STEP_DIGITS: usize = 4;
/// Longest rank accepted from outside.
pub const MAX_LEN: usize = 256;
/// Length above which [`Rank::needs_respread`] says yes.
pub const RESPREAD_LEN: usize = 48;

/// A position in an ordered list. See the [module documentation](self).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(String);

/// Why a rank could not be read or made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RankError {
    /// Not made of `0`–`9` and `a`–`z`, empty, all zeros, or too long.
    #[error("\"{0}\" is not a rank: expected 1 to {MAX_LEN} digits 0-9 and a-z, not all zero")]
    Malformed(String),
    /// [`Rank::between`] was given bounds that are not in increasing order
    /// of value, or that have the same value.
    #[error("no rank fits between \"{lower}\" and \"{upper}\"; the list needs re-spreading")]
    NoRoom {
        /// The lower bound.
        lower: String,
        /// The upper bound.
        upper: String,
    },
}

/// Digit values of `s`, which must be valid rank text.
fn values(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            _ => b - b'a' + 10,
        })
        .collect()
}

/// Rank text from digit values (each below 36), with trailing zeros removed.
fn from_values(mut v: Vec<u8>) -> Rank {
    while v.last() == Some(&0) {
        v.pop();
    }
    debug_assert!(!v.is_empty(), "a made rank is never zero");
    Rank(
        v.into_iter()
            .map(|d| char::from(DIGITS[usize::from(d)]))
            .collect(),
    )
}

fn strip_zeros(v: &[u8]) -> &[u8] {
    let end = v.iter().rposition(|&d| d != 0).map_or(0, |i| i + 1);
    &v[..end]
}

/// A value strictly between `a` and `b` (`None` = 1.0), with no trailing
/// zeros, as short as possible. `a < b` in value; neither has trailing zeros.
fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(0) == b[n] {
            n += 1;
        }
        if n > 0 {
            let mut out = b[..n].to_vec();
            out.extend(midpoint(a.get(n..).unwrap_or(&[]), Some(&b[n..])));
            return out;
        }
    }
    let da = a.first().copied().unwrap_or(0);
    let db = b.map_or(BASE, |b| b[0]);
    if db - da > 1 {
        vec![(da + db).div_ceil(2)]
    } else if let Some(b) = b.filter(|b| b.len() > 1) {
        vec![b[0]]
    } else {
        let mut out = vec![da];
        out.extend(midpoint(a.get(1..).unwrap_or(&[]), None));
        out
    }
}

/// `v` cut or padded to `STEP_DIGITS` digits.
fn prefix(v: &[u8]) -> Vec<u8> {
    let mut p: Vec<u8> = v.iter().copied().take(STEP_DIGITS).collect();
    p.resize(STEP_DIGITS, 0);
    p
}

impl Rank {
    /// The rank of the first item of an empty list: the middle, `"i"`.
    #[must_use]
    pub fn first() -> Self {
        Self("i".to_owned())
    }

    /// Checks rank text read from the database or an archive.
    pub fn parse(s: &str) -> Result<Self, RankError> {
        let ok = !s.is_empty()
            && s.len() <= MAX_LEN
            && s.bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
            && s.bytes().any(|b| b != b'0');
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(RankError::Malformed(s.to_owned()))
        }
    }

    /// The rank as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A rank after this one, for appending at the end of a list.
    #[must_use]
    pub fn after(&self) -> Self {
        let v = values(&self.0);
        let mut p = prefix(&v);
        for i in (0..STEP_DIGITS).rev() {
            if p[i] + 1 < BASE {
                p[i] += 1;
                return from_values(p);
            }
            p[i] = 0;
        }
        // The prefix was all `z`: no fixed step is left below 1.0.
        from_values(midpoint(strip_zeros(&v), None))
    }

    /// A rank before this one, for inserting at the start of a list.
    #[must_use]
    pub fn before(&self) -> Self {
        let v = values(&self.0);
        let stripped = strip_zeros(&v);
        let mut p = prefix(stripped);
        if stripped.len() > STEP_DIGITS && p.iter().any(|&d| d != 0) {
            // The four-digit prefix is already below this rank.
            return from_values(p);
        }
        for i in (0..STEP_DIGITS).rev() {
            if p[i] > 0 {
                p[i] -= 1;
                if p.iter().any(|&d| d != 0) {
                    return from_values(p);
                }
                break;
            }
            p[i] = BASE - 1;
        }
        // No fixed step is left above zero.
        from_values(midpoint(&[], Some(stripped)))
    }

    /// A rank between `lower` and `upper`; `None` means the start or the end
    /// of the list. Fails when there is no room: the bounds have the same
    /// value or are in the wrong order.
    pub fn between(lower: Option<&Rank>, upper: Option<&Rank>) -> Result<Self, RankError> {
        match (lower, upper) {
            (None, None) => Ok(Self::first()),
            (Some(a), None) => Ok(a.after()),
            (None, Some(b)) => Ok(b.before()),
            (Some(a), Some(b)) => {
                let va = values(&a.0);
                let vb = values(&b.0);
                let (sa, sb) = (strip_zeros(&va), strip_zeros(&vb));
                if sa >= sb {
                    return Err(RankError::NoRoom {
                        lower: a.0.clone(),
                        upper: b.0.clone(),
                    });
                }
                Ok(from_values(midpoint(sa, Some(sb))))
            }
        }
    }

    /// `n` evenly spaced ranks in increasing order, for a new list or for
    /// re-spreading one whose ranks grew long. They are short (a few digits
    /// more than the base-36 logarithm of `n`) and leave room for many
    /// inserts between neighbours.
    #[must_use]
    pub fn spread(n: usize) -> Vec<Self> {
        let n = n as u128;
        // Enough digits that neighbours are at least 36² apart.
        let mut width = STEP_DIGITS;
        while 36u128.pow(width as u32) < (n + 1) * 36 * 36 && width < 24 {
            width += 1;
        }
        let scale = 36u128.pow(width as u32);
        (1..=n)
            .map(|k| {
                let mut value = scale * k / (n + 1);
                let mut v = vec![0u8; width];
                for slot in v.iter_mut().rev() {
                    *slot = (value % 36) as u8;
                    value /= 36;
                }
                from_values(v)
            })
            .collect()
    }

    /// Whether this rank has grown long enough that its list should be
    /// re-spread with [`Rank::spread`].
    #[must_use]
    pub fn needs_respread(&self) -> bool {
        self.0.len() > RESPREAD_LEN
    }
}

impl FromStr for Rank {
    type Err = RankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rank({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn r(s: &str) -> Rank {
        Rank::parse(s).unwrap()
    }

    #[test]
    fn parse_checks_the_alphabet() {
        for bad in ["", "0", "000", "A", "a-b", "é", &"z".repeat(MAX_LEN + 1)] {
            assert!(Rank::parse(bad).is_err(), "{bad:?}");
        }
        for good in ["1", "i", "0001000", "zz", "00z"] {
            assert_eq!(r(good).as_str(), good);
        }
    }

    #[test]
    fn appending_uses_the_fixed_step() {
        assert_eq!(Rank::first().after().as_str(), "i001");
        assert_eq!(r("0001000").after().as_str(), "0002");
        assert_eq!(r("000z").after().as_str(), "001");
        assert_eq!(r("i0015").after().as_str(), "i002");
    }

    #[test]
    fn appending_near_the_end_still_fits() {
        let mut rank = r("zzzz");
        for _ in 0..200 {
            let next = rank.after();
            assert!(next > rank, "{next:?} > {rank:?}");
            rank = next;
        }
    }

    #[test]
    fn prepending_near_zero_still_fits() {
        let mut rank = r("0001");
        for _ in 0..200 {
            let prev = rank.before();
            assert!(prev < rank, "{prev:?} < {rank:?}");
            assert!(Rank::parse(prev.as_str()).is_ok());
            rank = prev;
        }
        assert_eq!(r("i").before().as_str(), "hzzz");
        assert_eq!(r("i0015").before().as_str(), "i001");
    }

    #[test]
    fn between_seed_ranks() {
        let a = r("0001000");
        let b = r("0002000");
        let mid = Rank::between(Some(&a), Some(&b)).unwrap();
        assert!(a < mid && mid < b, "{mid:?}");
        assert_eq!(mid.as_str(), "0001i");
    }

    #[test]
    fn between_reports_no_room() {
        let a = r("0001");
        let b = r("0001000");
        assert!(matches!(
            Rank::between(Some(&a), Some(&b)),
            Err(RankError::NoRoom { .. })
        ));
        assert!(Rank::between(Some(&r("b")), Some(&r("a"))).is_err());
    }

    #[test]
    fn repeated_inserts_at_one_place_grow_slowly() {
        let lower = r("1");
        let mut upper = r("2");
        for _ in 0..100 {
            upper = Rank::between(Some(&lower), Some(&upper)).unwrap();
        }
        assert!(upper.as_str().len() <= 30, "{upper:?}");
    }

    #[test]
    fn spread_is_increasing_and_roomy() {
        for n in [0, 1, 2, 10, 1000, 50_000] {
            let ranks = Rank::spread(n);
            assert_eq!(ranks.len(), n);
            assert!(ranks.windows(2).all(|w| w[0] < w[1]));
            for w in ranks.windows(2) {
                let mid = Rank::between(Some(&w[0]), Some(&w[1])).unwrap();
                assert!(w[0] < mid && mid < w[1]);
            }
        }
    }

    /// A small deterministic generator, so a failing case can be replayed
    /// from the seed proptest prints.
    struct SplitMix(u64);
    impl SplitMix {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) % n as u64) as usize
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        /// 10,000 random moves within and between board columns keep every
        /// column strictly ordered.
        #[test]
        fn random_moves_keep_columns_strictly_ordered(seed in any::<u64>()) {
            let mut rng = SplitMix(seed);
            let mut columns: Vec<Vec<Rank>> = vec![Vec::new(); 5];
            // 200 tasks appended across the columns.
            for i in 0..200 {
                let column = &mut columns[i % 5];
                let rank = Rank::between(column.last(), None).unwrap();
                column.push(rank);
            }
            for _ in 0..10_000 {
                let from = rng.below(columns.len());
                if columns[from].is_empty() {
                    continue;
                }
                let index = rng.below(columns[from].len());
                columns[from].remove(index);
                let to = rng.below(columns.len());
                let column = &mut columns[to];
                let at = rng.below(column.len() + 1);
                let lower = at.checked_sub(1).map(|i| &column[i]);
                let upper = column.get(at);
                let rank = Rank::between(lower, upper).unwrap();
                prop_assert!(lower.is_none_or(|l| *l < rank));
                prop_assert!(upper.is_none_or(|u| rank < *u));
                prop_assert!(!rank.as_str().ends_with('0'));
                column.insert(at, rank);
            }
            for column in &columns {
                prop_assert!(column.windows(2).all(|w| w[0] < w[1]));
                prop_assert!(column.iter().all(|r| r.as_str().len() <= MAX_LEN));
            }
        }

        #[test]
        fn between_any_two_distinct_values(a in "[0-9a-z]{1,12}", b in "[0-9a-z]{1,12}") {
            prop_assume!(a.bytes().any(|c| c != b'0') && b.bytes().any(|c| c != b'0'));
            let (a, b) = (r(&a), r(&b));
            match Rank::between(Some(&a), Some(&b)) {
                Ok(mid) => prop_assert!(a < mid && mid < b),
                Err(_) => {
                    let (va, vb) = (values(a.as_str()), values(b.as_str()));
                    prop_assert!(strip_zeros(&va) >= strip_zeros(&vb));
                }
            }
        }
    }
}
