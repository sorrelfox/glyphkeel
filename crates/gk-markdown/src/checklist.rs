// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Checkboxes: numbering, and ticking or clearing one by changing one byte.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, parse_document};

use crate::text::{collapse_whitespace, push_inline_text};

/// One checkbox (`- [ ]` or `- [x]`) of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItem {
    /// The item's number, from 1, in the order the reader shows the items.
    /// The reader's HTML carries the same number in `data-item`.
    pub number: usize,
    /// Whether the box is ticked (`x` or `X`).
    pub checked: bool,
    /// The text of the item's first paragraph, without nested items, with
    /// whitespace collapsed. Formatting marks are left out; code spans and
    /// math keep their content.
    pub text: String,
    /// The byte offset in the source of the character between the brackets.
    pub symbol_offset: usize,
    /// How many checklist items this one is nested in.
    pub depth: usize,
}

/// Why a checkbox could not be changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ToggleError {
    /// The document has no item with that number.
    #[error("there is no checklist item {number}; the text has {count}")]
    NoSuchItem {
        /// The number asked for.
        number: usize,
        /// How many items the document has.
        count: usize,
    },
    /// The item with that number has other text: the document changed since
    /// the number was read, so the item is not touched.
    #[error("checklist item {number} reads \"{actual}\", not \"{expected}\"")]
    TextMismatch {
        /// The number asked for.
        number: usize,
        /// The text the caller expected, with whitespace collapsed.
        expected: String,
        /// The item's current text.
        actual: String,
    },
    /// The parser placed the checkbox where the source holds no `[ ]`, `[x]`
    /// or `[X]`. This is a defect, reported instead of changing a wrong byte.
    #[error("checklist item {number} is not where the parser placed it")]
    Misplaced {
        /// The number asked for.
        number: usize,
    },
}

/// Lists the checkboxes of `source` in reading order.
///
/// ```
/// let items = gk_markdown::checklist("- [ ] one\n  - [x] two\n\n```\n- [ ] in code\n```\n");
/// assert_eq!(items.len(), 2);
/// assert_eq!((items[1].number, items[1].checked, items[1].depth), (2, true, 1));
/// assert_eq!(items[1].text, "two");
/// ```
#[must_use]
pub fn checklist(source: &str) -> Vec<ChecklistItem> {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    items(root, source)
}

/// Ticks a cleared item or clears a ticked one, and returns the new text.
///
/// `number` is the item's number from [`checklist`] or the reader's
/// `data-item`. `expected_text` must equal the item's [`ChecklistItem::text`]
/// (whitespace differences don't count): if the document changed and the
/// number now points at another item, nothing is changed and
/// [`ToggleError::TextMismatch`] is returned.
///
/// Exactly one byte changes: a space becomes `x`, and `x` or `X` becomes a
/// space.
///
/// ```
/// use gk_markdown::{ToggleError, toggle};
///
/// let text = "- [ ] alpha\n- [X] beta\n";
/// assert_eq!(toggle(text, 2, "beta").unwrap(), "- [ ] alpha\n- [ ] beta\n");
/// assert!(matches!(toggle(text, 2, "alpha"), Err(ToggleError::TextMismatch { .. })));
/// assert!(matches!(toggle(text, 3, "gamma"), Err(ToggleError::NoSuchItem { count: 2, .. })));
/// ```
pub fn toggle(source: &str, number: usize, expected_text: &str) -> Result<String, ToggleError> {
    let item = find(source, number, expected_text)?;
    let checked = !item.checked;
    Ok(write_symbol(source, item.symbol_offset, checked))
}

/// Ticks (`checked` true) or clears an item, like [`toggle`], but returns
/// `None` when the item is already in that state, so repeating a request
/// changes nothing.
///
/// ```
/// let text = "- [ ] alpha\n";
/// let done = gk_markdown::set_checked(text, 1, "alpha", true).unwrap();
/// assert_eq!(done.as_deref(), Some("- [x] alpha\n"));
/// assert_eq!(gk_markdown::set_checked("- [x] alpha\n", 1, "alpha", true).unwrap(), None);
/// ```
pub fn set_checked(
    source: &str,
    number: usize,
    expected_text: &str,
    checked: bool,
) -> Result<Option<String>, ToggleError> {
    let item = find(source, number, expected_text)?;
    if item.checked == checked {
        return Ok(None);
    }
    Ok(Some(write_symbol(source, item.symbol_offset, checked)))
}

fn find(source: &str, number: usize, expected_text: &str) -> Result<ChecklistItem, ToggleError> {
    let mut items = checklist(source);
    let count = items.len();
    if number == 0 || number > count {
        return Err(ToggleError::NoSuchItem { number, count });
    }
    let item = items.swap_remove(number - 1);
    let expected = collapse_whitespace(expected_text);
    if item.text != expected {
        return Err(ToggleError::TextMismatch {
            number,
            expected,
            actual: item.text,
        });
    }
    let bytes = source.as_bytes();
    let offset = item.symbol_offset;
    let in_brackets = offset > 0
        && bytes.get(offset - 1) == Some(&b'[')
        && bytes.get(offset + 1) == Some(&b']')
        && matches!(bytes[offset], b' ' | b'x' | b'X');
    if !in_brackets {
        return Err(ToggleError::Misplaced { number });
    }
    Ok(item)
}

/// Replaces the ASCII byte at `offset` with `x` or a space.
fn write_symbol(source: &str, offset: usize, checked: bool) -> String {
    let symbol = if checked { "x" } else { " " };
    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..offset]);
    out.push_str(symbol);
    out.push_str(&source[offset + 1..]);
    out
}

/// The checklist of a parsed document. Items are numbered in the order of a
/// depth-first walk, which is the order the renderer meets them in.
pub(crate) fn items<'a>(root: &'a AstNode<'a>, source: &str) -> Vec<ChecklistItem> {
    let lines = LineIndex::new(source);
    let mut out = Vec::new();
    for node in root.descendants() {
        let NodeValue::TaskItem(task) = &node.data().value else {
            continue;
        };
        let position = task.symbol_sourcepos.start;
        let depth = node
            .ancestors()
            .skip(1)
            .filter(|a| matches!(a.data().value, NodeValue::TaskItem(_)))
            .count();
        out.push(ChecklistItem {
            number: out.len() + 1,
            checked: task.symbol.is_some(),
            text: item_text(node),
            symbol_offset: lines.offset(position.line, position.column),
            depth,
        });
    }
    out
}

/// The text of an item's first paragraph.
fn item_text<'a>(item: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    if let Some(paragraph) = item
        .children()
        .find(|c| matches!(c.data().value, NodeValue::Paragraph))
    {
        push_inline_text(paragraph, &mut text);
    }
    collapse_whitespace(&text)
}

/// Maps the parser's line and column (both from 1, the column counted in
/// bytes) to byte offsets in the source.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    /// Lines end at `\n`, `\r\n` or a lone `\r`, as in the parser.
    fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => starts.push(i + 1),
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    starts.push(i + 1);
                }
                _ => {}
            }
            i += 1;
        }
        Self { starts }
    }

    fn offset(&self, line: usize, column: usize) -> usize {
        self.starts[line.saturating_sub(1).min(self.starts.len() - 1)] + column.saturating_sub(1)
    }
}
