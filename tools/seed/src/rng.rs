// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A small, fixed pseudo-random generator (SplitMix64).
//!
//! It is written out here rather than taken from a crate, so the stream for a
//! given seed can never change with a dependency update: the same seed must
//! give byte-identical data sets for as long as the archive format lives.

/// SplitMix64. Not for anything secret; only for reproducible test data.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator for `seed`. `stream` separates independent sequences drawn
    /// from the same seed, so adding draws to one part of the data set does not
    /// shift every other part.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: seed ^ stream.wrapping_mul(0xA076_1D64_78BD_642F),
        };
        // Warm up so that nearby seeds diverge immediately.
        rng.next_u64();
        rng.next_u64();
        rng
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n`. `n` must be above zero.
    pub fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        // Multiply-shift: unbiased enough for test data and free of loops.
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi - lo) as u64 + 1;
        lo + self.below(span) as i64
    }

    /// An index into a collection of `len` items.
    pub fn index(&mut self, len: usize) -> usize {
        self.below(len as u64) as usize
    }

    /// True with a probability of `per_mille` / 1000.
    pub fn chance(&mut self, per_mille: u64) -> bool {
        self.below(1000) < per_mille
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut rng = Rng::new(1, 0);
        let first: Vec<u64> = (0..3).map(|_| rng.next_u64()).collect();
        let mut again = Rng::new(1, 0);
        let second: Vec<u64> = (0..3).map(|_| again.next_u64()).collect();
        assert_eq!(first, second);
        assert_ne!(Rng::new(1, 0).next_u64(), Rng::new(2, 0).next_u64());
        assert_ne!(Rng::new(1, 0).next_u64(), Rng::new(1, 1).next_u64());
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::new(7, 0);
        for n in 1..200 {
            assert!(rng.below(n) < n);
        }
        for _ in 0..1000 {
            let v = rng.range(-3, 3);
            assert!((-3..=3).contains(&v));
        }
    }
}
