// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Long text edited by several writers: a line-based three-way merge,
//! targeted edits, and the differences between two texts.
//!
//! Texts are compared by lines, each line with its own line ending (`\n` or
//! `\r\n`), so a merge only ever copies whole lines from its inputs: the
//! result keeps every byte as it was written, line endings included.
//!
//! The merge follows the usual rule of version control: two changes conflict
//! when they touch the same lines of the common base or lines right next to
//! each other. Both sides making the same change is no conflict.

use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, TextDiff, capture_diff_slices_deadline};

/// How long one diff may search for the smallest difference before it
/// settles for a larger, still correct one.
const DIFF_TIME: Duration = Duration::from_secs(2);

/// The result of a three-way merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// Both changes fit together; the merged text.
    Clean(String),
    /// The changes touch the same lines.
    Conflict,
}

/// The lines of `text`, each with its line ending; the last line may have
/// none. An empty text has no lines.
#[must_use]
pub fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// One changed stretch of the base: base lines `start..end` became the
/// side's lines `new_start..new_end`.
#[derive(Debug, Clone, Copy)]
struct Hunk {
    start: usize,
    end: usize,
    new_start: usize,
    new_end: usize,
    side: usize,
}

fn hunks(base: &[&str], side: &[&str], which: usize) -> Vec<Hunk> {
    let deadline = Instant::now().checked_add(DIFF_TIME);
    let mut out: Vec<Hunk> = Vec::new();
    for op in capture_diff_slices_deadline(Algorithm::Myers, base, side, deadline) {
        let (start, end, new_start, new_end) = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => (old_index, old_index + old_len, new_index, new_index),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => (old_index, old_index, new_index, new_index + new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => (
                old_index,
                old_index + old_len,
                new_index,
                new_index + new_len,
            ),
        };
        match out.last_mut() {
            Some(last) if last.end == start && last.new_end == new_start => {
                last.end = end;
                last.new_end = new_end;
            }
            _ => out.push(Hunk {
                start,
                end,
                new_start,
                new_end,
                side: which,
            }),
        }
    }
    out
}

/// Merges two texts that were both edited from `base`: `ours` (the text
/// stored now) and `theirs` (the late edit).
///
/// Changes on different lines are combined. Changes to the same or
/// neighbouring lines of the base conflict unless both made the same
/// change.
///
/// ```
/// use gk_core::merge::{Merge, merge3};
///
/// let base = "one\n\ntwo\n\nthree\n";
/// let ours = "one!\n\ntwo\n\nthree\n";
/// let theirs = "one\n\ntwo\n\nthree?\n";
/// assert_eq!(merge3(base, ours, theirs), Merge::Clean("one!\n\ntwo\n\nthree?\n".into()));
/// assert_eq!(merge3(base, ours, "one?\n\ntwo\n\nthree\n"), Merge::Conflict);
/// ```
#[must_use]
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Merge {
    if ours == theirs || theirs == base {
        return Merge::Clean(ours.to_owned());
    }
    if ours == base {
        return Merge::Clean(theirs.to_owned());
    }
    let (b, o, t) = (lines(base), lines(ours), lines(theirs));
    let sides = [&o, &t];
    let mut all = hunks(&b, &o, 0);
    all.extend(hunks(&b, &t, 1));
    all.sort_by_key(|h| (h.start, h.end, h.side));

    let mut out = String::with_capacity(ours.len().max(theirs.len()));
    // How far each side's line numbers run ahead of the base's outside the
    // changes seen so far.
    let mut delta = [0isize; 2];
    let mut copied = 0;
    let mut i = 0;
    while i < all.len() {
        let start = all[i].start;
        let mut end = all[i].end;
        let mut j = i + 1;
        while j < all.len() && all[j].start <= end {
            end = end.max(all[j].end);
            j += 1;
        }
        let cluster = &all[i..j];
        for line in &b[copied..start] {
            out.push_str(line);
        }
        let mut texts: [Option<String>; 2] = [None, None];
        let mut grow = [0isize; 2];
        for h in cluster {
            grow[h.side] += signed(h.new_end - h.new_start) - signed(h.end - h.start);
        }
        for side in 0..2 {
            if cluster.iter().any(|h| h.side == side) {
                let from = offset(start, delta[side]);
                let to = offset(end, delta[side] + grow[side]);
                texts[side] = Some(sides[side][from..to].concat());
            }
        }
        match texts {
            [Some(a), Some(b)] if a != b => return Merge::Conflict,
            [Some(a), _] | [None, Some(a)] => out.push_str(&a),
            [None, None] => {}
        }
        delta[0] += grow[0];
        delta[1] += grow[1];
        copied = end;
        i = j;
    }
    for line in &b[copied..] {
        out.push_str(line);
    }
    Merge::Clean(out)
}

fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

fn offset(base: usize, delta: isize) -> usize {
    usize::try_from(signed(base) + delta).unwrap_or(0)
}

/// A targeted edit: `old_text` must occur exactly once in the text and is
/// replaced by `new_text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edit<'a> {
    /// The text to find.
    pub old_text: &'a str,
    /// What replaces it.
    pub new_text: &'a str,
}

/// Why a targeted edit could not be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The edit's `old_text` is empty.
    #[error("edit {index} has no text to find")]
    Empty {
        /// The edit's position in the list, from 0.
        index: usize,
    },
    /// `old_text` is not in the text.
    #[error("the text of edit {index} was not found")]
    NotFound {
        /// The edit's position in the list, from 0.
        index: usize,
    },
    /// `old_text` occurs more than once.
    #[error("the text of edit {index} occurs {count} times; add more around it")]
    Ambiguous {
        /// The edit's position in the list, from 0.
        index: usize,
        /// How often it occurs.
        count: usize,
    },
}

impl EditError {
    /// The position in the list of the edit that failed.
    #[must_use]
    pub const fn index(&self) -> usize {
        match self {
            Self::Empty { index } | Self::NotFound { index } | Self::Ambiguous { index, .. } => {
                *index
            }
        }
    }
}

/// Applies `edits` to `text` one after another, each to the result of the
/// ones before it. Nothing is applied when one fails.
///
/// ```
/// use gk_core::merge::{Edit, apply_edits};
///
/// let edits = [Edit { old_text: "two", new_text: "2" }];
/// assert_eq!(apply_edits("one two", &edits).unwrap(), "one 2");
/// ```
///
/// # Errors
/// An [`EditError`] naming the first edit whose text is empty, missing or
/// found more than once.
pub fn apply_edits(text: &str, edits: &[Edit<'_>]) -> Result<String, EditError> {
    let mut out = text.to_owned();
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(EditError::Empty { index });
        }
        let count = out.matches(edit.old_text).count();
        match count {
            0 => return Err(EditError::NotFound { index }),
            1 => out = out.replacen(edit.old_text, edit.new_text, 1),
            count => return Err(EditError::Ambiguous { index, count }),
        }
    }
    Ok(out)
}

/// The differences from `old` to `new` as a unified diff with three lines
/// of context, headed `old_name` and `new_name`. Empty when the texts are
/// the same.
#[must_use]
pub fn unified_diff(old: &str, new: &str, old_name: &str, new_name: &str) -> String {
    if old == new {
        return String::new();
    }
    TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(DIFF_TIME)
        .diff_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(old_name, new_name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(base: &str, ours: &str, theirs: &str) -> String {
        match merge3(base, ours, theirs) {
            Merge::Clean(t) => t,
            Merge::Conflict => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn changes_to_different_paragraphs_merge() {
        let base = "# Plan\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird.\n";
        let ours = "# Plan\n\nFirst paragraph, edited.\n\nSecond paragraph.\n\nThird.\n";
        let theirs = "# Plan\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird, by the agent.\n";
        assert_eq!(
            clean(base, ours, theirs),
            "# Plan\n\nFirst paragraph, edited.\n\nSecond paragraph.\n\nThird, by the agent.\n"
        );
    }

    #[test]
    fn the_same_lines_conflict() {
        let base = "a\nb\nc\n";
        assert_eq!(merge3(base, "a\nB\nc\n", "a\nb!\nc\n"), Merge::Conflict);
        // Neighbouring lines conflict too.
        assert_eq!(merge3(base, "A\nb\nc\n", "a\nB\nc\n"), Merge::Conflict);
    }

    #[test]
    fn the_same_change_on_both_sides_is_kept_once() {
        let base = "a\n\nb\n\nc\n";
        let both = "a\n\nB\n\nc\n";
        assert_eq!(clean(base, both, both), both);
        let ours = "A\n\nB\n\nc\n";
        assert_eq!(clean(base, ours, both), ours);
    }

    #[test]
    fn one_sided_changes_win() {
        assert_eq!(clean("a\n", "a\n", "b\n"), "b\n");
        assert_eq!(clean("a\n", "b\n", "a\n"), "b\n");
        assert_eq!(clean("", "", "new\n"), "new\n");
    }

    #[test]
    fn insertions_and_deletions_merge() {
        let base = "1\n\n2\n\n3\n\n4\n";
        let ours = "0\n1\n\n2\n\n3\n\n4\n";
        let theirs = "1\n\n2\n\n4\n";
        assert_eq!(clean(base, ours, theirs), "0\n1\n\n2\n\n4\n");
        let theirs = "1\n\n2\n\n3\n\n4\n5\n";
        assert_eq!(clean(base, ours, theirs), "0\n1\n\n2\n\n3\n\n4\n5\n");
    }

    #[test]
    fn bytes_and_line_endings_are_kept() {
        let base = "one\r\n\r\ntwo\r\n\r\nthree";
        let ours = "one \u{e9}\r\n\r\ntwo\r\n\r\nthree";
        let theirs = "one\r\n\r\ntwo\r\n\r\nthree\tend";
        assert_eq!(
            clean(base, ours, theirs),
            "one \u{e9}\r\n\r\ntwo\r\n\r\nthree\tend"
        );
        // Mixed endings stay as written.
        let base = "a\r\n\nb\n\nc\r\n";
        let ours = "a!\r\n\nb\n\nc\r\n";
        let theirs = "a\r\n\nb\n\nc?\r\n";
        assert_eq!(clean(base, ours, theirs), "a!\r\n\nb\n\nc?\r\n");
    }

    #[test]
    fn a_line_ending_change_is_a_change() {
        assert_eq!(merge3("a\nb\n", "a\r\nb\n", "a\nB\n"), Merge::Conflict);
    }

    #[test]
    fn edits_apply_in_order_or_not_at_all() {
        let e = |o, n| Edit {
            old_text: o,
            new_text: n,
        };
        assert_eq!(
            apply_edits("a b c", &[e("a", "x"), e("x b", "y")]).unwrap(),
            "y c"
        );
        assert_eq!(
            apply_edits("a a", &[e("a", "b")]),
            Err(EditError::Ambiguous { index: 0, count: 2 })
        );
        assert_eq!(
            apply_edits("a", &[e("a", "b"), e("z", "")]),
            Err(EditError::NotFound { index: 1 })
        );
        assert_eq!(
            apply_edits("a", &[e("", "b")]).map_err(|e| e.index()),
            Err(0)
        );
    }

    #[test]
    fn diffs_show_the_changed_lines() {
        let d = unified_diff("a\nb\n", "a\nc\n", "r1", "r2");
        assert!(d.contains("--- r1\n+++ r2\n"), "{d}");
        assert!(d.contains("-b\n+c\n"), "{d}");
        assert_eq!(unified_diff("x", "x", "a", "b"), "");
    }

    mod properties {
        use proptest::prelude::*;

        use super::super::*;

        fn doc() -> impl Strategy<Value = Vec<String>> {
            prop::collection::vec("[a-c]{0,2}(\r\n|\n)", 0..12)
        }

        proptest! {
            /// Merging a change with no change gives the change; a clean
            /// merge contains only lines of the inputs.
            #[test]
            fn merges_are_made_of_input_lines(base in doc(), ours in doc(), theirs in doc()) {
                let (base, ours, theirs) = (base.concat(), ours.concat(), theirs.concat());
                prop_assert_eq!(merge3(&base, &base, &theirs), Merge::Clean(theirs.clone()));
                prop_assert_eq!(merge3(&base, &ours, &base), Merge::Clean(ours.clone()));
                if let Merge::Clean(m) = merge3(&base, &ours, &theirs) {
                    for line in lines(&m) {
                        prop_assert!(
                            lines(&ours).contains(&line) || lines(&theirs).contains(&line),
                            "{:?} comes from neither side", line
                        );
                    }
                }
            }
        }
    }
}
