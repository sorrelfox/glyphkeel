// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The colour a new label or project gets when none is chosen: the first
//! colour of the palette that its neighbours (the project's other labels, or
//! the sibling projects) don't use yet, so items added one after another
//! come out different without anyone picking a colour.

use crate::kinds::Hue;

/// The palette in the order colours are suggested: the design's order.
pub const PALETTE: &[Hue] = Hue::ALL;

/// The colour to suggest next, given the colours already `used` (one entry
/// per item, repeats allowed): the first palette colour not used yet; when
/// every colour is taken, the least-used one, the earlier in the palette on
/// a tie.
#[must_use]
pub fn suggest_hue(used: &[Hue]) -> Hue {
    let mut best = PALETTE[0];
    let mut best_count = usize::MAX;
    for &hue in PALETTE {
        let count = used.iter().filter(|u| **u == hue).count();
        if count < best_count {
            best = hue;
            best_count = count;
            if count == 0 {
                break;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_unused_colour_in_palette_order() {
        assert_eq!(suggest_hue(&[]), Hue::Red);
        assert_eq!(suggest_hue(&[Hue::Red]), Hue::Orange);
        assert_eq!(
            suggest_hue(&[Hue::Orange, Hue::Red, Hue::Teal]),
            Hue::Yellow
        );
        assert_eq!(suggest_hue(&[Hue::Grey, Hue::Grey]), Hue::Red);
    }

    #[test]
    fn the_least_used_colour_when_all_are_taken() {
        let mut used: Vec<Hue> = PALETTE.to_vec();
        assert_eq!(suggest_hue(&used), Hue::Red);
        used.extend([Hue::Red, Hue::Orange, Hue::Yellow]);
        assert_eq!(suggest_hue(&used), Hue::Green);
        used.extend(PALETTE.iter().copied().filter(|h| *h != Hue::Blue));
        assert_eq!(suggest_hue(&used), Hue::Blue);
    }

    #[test]
    fn adding_one_after_another_cycles_the_palette() {
        let mut used = Vec::new();
        for _ in 0..PALETTE.len() * 2 {
            used.push(suggest_hue(&used));
        }
        assert_eq!(&used[..PALETTE.len()], PALETTE);
        assert_eq!(&used[PALETTE.len()..], PALETTE);
    }
}
