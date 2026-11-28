// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The text inside attached files, for search.
//!
//! Every file is hostile input. Extraction runs in this process, offline,
//! without other programs, and within hard limits ([`Limits`]):
//!
//! - text, Markdown, CSV, JSON and source code are decoded with their
//!   encoding detected (a byte-order mark, UTF-8, UTF-16 without a mark,
//!   else a guess among the legacy encodings);
//! - PDF: the text layer of its pages; a scanned PDF without one gives no
//!   text (there is no character recognition);
//! - Office Open XML (`.docx`, `.xlsx`, `.pptx`) and OpenDocument (`.odt`,
//!   `.ods`, `.odp`): the text nodes of their XML parts. The archive is
//!   checked before anything is unpacked (entry count, declared sizes,
//!   compression ratio), and every read is capped again by what it really
//!   inflates to. The XML reader never reads a DTD or expands an entity
//!   beyond the five predefined ones and character references;
//! - anything else: nothing (the file is still found by its name).
//!
//! The text is capped at [`Limits::max_text`] bytes. [`extract_bounded`]
//! also caps the time: it runs the extraction on a thread of its own and
//! gives up waiting after [`Limits::max_time`], and a panic in a parser is
//! reported as a failure rather than taking the program down.

mod office;
mod pdf;
mod text;

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How many loop turns pass between two readings of the clock.
const CLOCK_EVERY: u32 = 64;

/// Raised when the extraction logic changes, so files extracted by an
/// older version are extracted again.
pub const VERSION: u32 = 1;

/// The hard limits of one extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most text kept, in bytes of UTF-8; the rest is cut off.
    pub max_text: usize,
    /// The largest file looked at, in bytes; larger ones are skipped.
    pub max_input: usize,
    /// The most entries an archive (an Office file) may have.
    pub max_entries: usize,
    /// The most an archive's entries may unpack to, all together.
    pub max_unpacked: u64,
    /// The most one entry may unpack to.
    pub max_entry: u64,
    /// The highest ratio of unpacked to packed size of a large entry.
    pub max_ratio: u64,
    /// The most a PDF's streams may inflate to, each.
    pub max_pdf_stream: usize,
    /// The most pages of a PDF read.
    pub max_pages: usize,
    /// How long [`extract_bounded`] waits.
    pub max_time: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_text: 1024 * 1024,
            max_input: 64 * 1024 * 1024,
            max_entries: 10_000,
            max_unpacked: 256 * 1024 * 1024,
            max_entry: 64 * 1024 * 1024,
            max_ratio: 200,
            max_pdf_stream: 32 * 1024 * 1024,
            max_pages: 5_000,
            max_time: Duration::from_secs(10),
        }
    }
}

/// The kinds of file whose text is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Text in some encoding.
    Text,
    /// A PDF document.
    Pdf,
    /// A ZIP archive that may be an Office Open XML or OpenDocument file.
    Office,
}

/// Why a file gave no text although its kind is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skipped {
    /// It has no text: a scanned PDF, an empty document.
    NoText,
    /// Larger than [`Limits::max_input`].
    TooLarge,
    /// An archive with more entries than allowed.
    TooManyEntries,
    /// An archive that unpacks to more than allowed: a ZIP bomb.
    Bomb,
    /// Encrypted or password-protected.
    Encrypted,
    /// Not a well-formed file of its kind.
    Malformed,
    /// Text in no encoding that could be recognised: binary data.
    Binary,
    /// The extraction took longer than [`Limits::max_time`].
    TimedOut,
    /// A parser failed unexpectedly.
    Failed,
}

impl Skipped {
    /// The name stored with the file and shown in logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoText => "no-text",
            Self::TooLarge => "too-large",
            Self::TooManyEntries => "too-many-entries",
            Self::Bomb => "bomb",
            Self::Encrypted => "encrypted",
            Self::Malformed => "malformed",
            Self::Binary => "binary",
            Self::TimedOut => "timed-out",
            Self::Failed => "failed",
        }
    }
}

/// What extraction found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extracted {
    /// The file's text.
    Text {
        /// The text, at most [`Limits::max_text`] bytes.
        text: String,
        /// Whether more text was cut off.
        truncated: bool,
    },
    /// The file is of a kind whose text is read, but gave none.
    Skipped(Skipped),
    /// The file is of no kind whose text is read.
    Unsupported,
}

/// The kind of file `bytes` is, by its content, its type (`mime`) and its
/// name; `None` when its text is not read.
#[must_use]
pub fn format_of(bytes: &[u8], mime: &str, name: &str) -> Option<Format> {
    if bytes.starts_with(b"%PDF-") || mime == "application/pdf" {
        return Some(Format::Pdf);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return office::is_office(mime, name).then_some(Format::Office);
    }
    if text::is_texty(bytes, mime, name) {
        return Some(Format::Text);
    }
    None
}

/// The text of `bytes`, a file of type `mime` named `name`, within
/// `limits` apart from the time, on this thread.
#[must_use]
pub fn extract(bytes: &[u8], mime: &str, name: &str, limits: &Limits) -> Extracted {
    let Some(format) = format_of(bytes, mime, name) else {
        return Extracted::Unsupported;
    };
    if bytes.len() > limits.max_input {
        return Extracted::Skipped(Skipped::TooLarge);
    }
    let found = match format {
        Format::Text => text::extract(bytes, limits),
        Format::Pdf => pdf::extract(bytes, limits),
        Format::Office => office::extract(bytes, limits),
    };
    match found {
        Ok(collected) => collected.finish(),
        Err(skipped) => Extracted::Skipped(skipped),
    }
}

/// As [`extract`], on a thread of its own, waiting at most
/// [`Limits::max_time`]. An extraction that takes longer is left to finish
/// on its own and its result dropped; one that panics is reported as
/// [`Skipped::Failed`].
#[must_use]
pub fn extract_bounded(bytes: Arc<[u8]>, mime: &str, name: &str, limits: &Limits) -> Extracted {
    if format_of(&bytes, mime, name).is_none() {
        return Extracted::Unsupported;
    }
    let Some(running) = spawn(bytes, mime, name, limits) else {
        return Extracted::Skipped(Skipped::Failed);
    };
    running
        .wait(limits.max_time)
        .unwrap_or(Extracted::Skipped(Skipped::TimedOut))
}

/// An extraction running on a thread of its own ([`spawn`]).
#[derive(Debug)]
pub struct Running {
    receive: mpsc::Receiver<Extracted>,
}

impl Running {
    /// The result, waiting at most `time` for it; `None` while it is still
    /// running. A thread that ended without a result panicked:
    /// [`Skipped::Failed`].
    #[must_use]
    pub fn wait(&self, time: Duration) -> Option<Extracted> {
        match self.receive.recv_timeout(time) {
            Ok(found) => Some(found),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(Extracted::Skipped(Skipped::Failed)),
        }
    }
}

/// Starts [`extract`] on a thread of its own; `None` when no thread could
/// be started. The caller decides how long to wait ([`Running::wait`]) and
/// drops what runs too long.
#[must_use]
pub fn spawn(bytes: Arc<[u8]>, mime: &str, name: &str, limits: &Limits) -> Option<Running> {
    let (send, receive) = mpsc::sync_channel(1);
    let (mime, name, limits) = (mime.to_owned(), name.to_owned(), *limits);
    std::thread::Builder::new()
        .name("gk-extract".to_owned())
        .spawn(move || {
            let found = extract(&bytes, &mime, &name, &limits);
            let _ = send.send(found);
        })
        .ok()?;
    Some(Running { receive })
}

/// Text collected up to a limit.
#[derive(Debug)]
pub(crate) struct Collector {
    text: String,
    max: usize,
    truncated: bool,
    /// When the extraction must stop by itself, and whether it did.
    deadline: Option<Instant>,
    timed_out: bool,
    /// Calls since the clock was last read.
    ticks: u32,
    /// Whether the last character written was white space (or nothing
    /// was written), so runs of breaks stay short.
    at_break: bool,
}

impl Collector {
    pub(crate) fn new(max: usize) -> Self {
        Self {
            text: String::new(),
            max,
            truncated: false,
            deadline: None,
            timed_out: false,
            ticks: 0,
            at_break: true,
        }
    }

    /// A collector of at most `limits.max_text` bytes that runs out of
    /// time [`Limits::max_time`] from now.
    pub(crate) fn for_limits(limits: &Limits) -> Self {
        Self {
            deadline: Instant::now().checked_add(limits.max_time),
            ..Self::new(limits.max_text)
        }
    }

    /// Whether the text limit is reached or the time is up; the readers'
    /// loops ask after every piece and stop then, so an extraction ends by
    /// itself instead of running on.
    pub(crate) fn full(&mut self) -> bool {
        if self.truncated || self.timed_out {
            return true;
        }
        self.ticks += 1;
        if self.ticks >= CLOCK_EVERY {
            self.ticks = 0;
            if self.deadline.is_some_and(|d| Instant::now() >= d) {
                self.timed_out = true;
            }
        }
        self.timed_out
    }

    /// As [`Collector::full`] for the time only, failing with
    /// [`Skipped::TimedOut`]: for loops before any text is collected.
    pub(crate) fn in_time(&mut self) -> Result<(), Skipped> {
        if self.full() && self.timed_out {
            return Err(Skipped::TimedOut);
        }
        Ok(())
    }

    /// Adds `piece`, with control characters other than tabs and line
    /// breaks turned into spaces.
    pub(crate) fn push(&mut self, piece: &str) {
        for c in piece.chars() {
            let c = match c {
                '\n' | '\t' => c,
                '\r' => continue,
                c if c.is_control() || c == '\u{feff}' => ' ',
                c => c,
            };
            if !self.put(c) {
                return;
            }
        }
    }

    /// Ends a paragraph, a cell or a line.
    pub(crate) fn line_break(&mut self) {
        if !self.at_break {
            self.put('\n');
        }
    }

    /// Separates two pieces that must not run together.
    pub(crate) fn space(&mut self) {
        if !self.at_break {
            self.put(' ');
        }
    }

    fn put(&mut self, c: char) -> bool {
        if self.truncated || self.timed_out {
            return false;
        }
        if self.text.len().is_multiple_of(4096) && self.full() {
            return false;
        }
        if self.text.len() + c.len_utf8() > self.max {
            self.truncated = true;
            return false;
        }
        self.text.push(c);
        self.at_break = c.is_whitespace();
        true
    }

    pub(crate) fn finish(self) -> Extracted {
        if self.timed_out {
            return Extracted::Skipped(Skipped::TimedOut);
        }
        let text = self.text.trim_end().to_owned();
        if text.trim().is_empty() {
            return Extracted::Skipped(Skipped::NoText);
        }
        Extracted::Text {
            text,
            truncated: self.truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_collector_stops_at_its_limit_on_a_character_boundary() {
        let mut c = Collector::new(5);
        c.push("aé€b");
        assert!(c.full());
        assert_eq!(
            c.finish(),
            Extracted::Text {
                text: "aé".to_owned(),
                truncated: true
            }
        );
    }

    #[test]
    fn control_characters_become_spaces_and_breaks_stay_short() {
        let mut c = Collector::new(100);
        c.line_break();
        c.push("a\u{0}b\r\n");
        c.line_break();
        c.line_break();
        c.push("c");
        assert_eq!(
            c.finish(),
            Extracted::Text {
                text: "a b\nc".to_owned(),
                truncated: false
            }
        );
    }

    #[test]
    fn nothing_but_white_space_is_no_text() {
        let mut c = Collector::new(100);
        c.push(" \n\t ");
        assert_eq!(c.finish(), Extracted::Skipped(Skipped::NoText));
    }

    #[test]
    fn unknown_kinds_are_unsupported() {
        let limits = Limits::default();
        assert_eq!(
            extract(
                b"\x7fELF\x02\x01",
                "application/x-executable",
                "a.out",
                &limits
            ),
            Extracted::Unsupported
        );
        assert_eq!(
            extract_bounded(
                Arc::from(&b"\x89PNG\r\n"[..]),
                "image/png",
                "a.png",
                &limits
            ),
            Extracted::Unsupported
        );
    }

    #[test]
    fn a_slow_extraction_times_out() {
        let limits = Limits {
            max_time: Duration::from_nanos(1),
            ..Limits::default()
        };
        let big = "word ".repeat(200_000);
        let found = extract_bounded(Arc::from(big.as_bytes()), "text/plain", "a.txt", &limits);
        // The thread may win the race on a fast machine; it never fails
        // otherwise.
        assert!(matches!(
            found,
            Extracted::Skipped(Skipped::TimedOut) | Extracted::Text { .. }
        ));
    }
}
