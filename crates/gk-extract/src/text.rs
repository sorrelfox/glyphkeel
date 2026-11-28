// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Text files: plain text, Markdown, CSV, JSON, XML and source code, in
//! whatever encoding they were written.

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};

use crate::{Collector, Limits, Skipped};

/// Types that are text although they are not `text/…`.
const TEXT_TYPES: &[&str] = &[
    "application/json",
    "application/xml",
    "application/yaml",
    "application/toml",
    "application/javascript",
    "application/sql",
    "application/x-sh",
    "application/x-ndjson",
];

/// Name extensions of text files, for files whose type says nothing (a
/// UTF-16 file sniffs as binary data, for example).
const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "text",
    "md",
    "markdown",
    "csv",
    "tsv",
    "json",
    "jsonl",
    "ndjson",
    "xml",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "log",
    "rst",
    "adoc",
    "tex",
    "html",
    "htm",
    "css",
    "scss",
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "rs",
    "py",
    "rb",
    "go",
    "java",
    "kt",
    "kts",
    "swift",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "fs",
    "php",
    "pl",
    "lua",
    "r",
    "scala",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "sql",
    "graphql",
    "proto",
    "vue",
    "svelte",
    "dart",
    "ex",
    "exs",
    "erl",
    "hs",
    "ml",
    "clj",
    "el",
    "vim",
    "diff",
    "patch",
    "srt",
    "vtt",
    "properties",
    "env",
];

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Whether a file is read as text: by its type, by its name, or because
/// it starts with a UTF-16 byte-order mark.
pub(crate) fn is_texty(bytes: &[u8], mime: &str, name: &str) -> bool {
    let base = mime.split(';').next().unwrap_or("").trim();
    base.starts_with("text/")
        || TEXT_TYPES.contains(&base)
        || base.ends_with("+json")
        || base.ends_with("+xml")
        || TEXT_EXTENSIONS.contains(&extension(name).as_str())
        || bytes.starts_with(&[0xff, 0xfe])
        || bytes.starts_with(&[0xfe, 0xff])
}

/// The encoding of UTF-16 text without a byte-order mark: most of the
/// bytes on one side of each pair are zero (Latin script), and none of the
/// pairs is zero on both sides.
fn utf16_without_mark(bytes: &[u8]) -> Option<&'static Encoding> {
    let sample = &bytes[..bytes.len().min(4096) & !1];
    if sample.len() < 4 {
        return None;
    }
    let pairs = sample.len() / 2;
    let (mut even, mut odd) = (0usize, 0usize);
    for pair in sample.as_chunks::<2>().0 {
        match (pair[0] == 0, pair[1] == 0) {
            (true, true) => return None,
            (true, false) => even += 1,
            (false, true) => odd += 1,
            (false, false) => {}
        }
    }
    if odd * 10 >= pairs * 3 && even == 0 {
        Some(UTF_16LE)
    } else if even * 10 >= pairs * 3 && odd == 0 {
        Some(UTF_16BE)
    } else {
        None
    }
}

/// The encoding of `bytes` and how many bytes of byte-order mark to skip.
fn encoding_of(bytes: &[u8]) -> Result<(&'static Encoding, usize), Skipped> {
    if let Some((encoding, mark)) = Encoding::for_bom(bytes) {
        return Ok((encoding, mark));
    }
    if std::str::from_utf8(bytes).is_ok() {
        return Ok((encoding_rs::UTF_8, 0));
    }
    if let Some(encoding) = utf16_without_mark(bytes) {
        return Ok((encoding, 0));
    }
    // A cut-off last character still counts as UTF-8.
    if let Err(e) = std::str::from_utf8(bytes)
        && e.error_len().is_none()
    {
        return Ok((encoding_rs::UTF_8, 0));
    }
    if bytes.contains(&0) {
        return Err(Skipped::Binary);
    }
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(&bytes[..bytes.len().min(256 * 1024)], true);
    Ok((detector.guess(None, Utf8Detection::Allow), 0))
}

pub(crate) fn extract(bytes: &[u8], limits: &Limits) -> Result<Collector, Skipped> {
    let (encoding, mark) = encoding_of(bytes)?;
    let body = &bytes[mark..];
    // Decoding grows UTF-16 by half at most and single-byte encodings by
    // three times, so a slice of four times the limit is always enough.
    let take = body.len().min(limits.max_text.saturating_mul(4));
    let (text, _) = encoding.decode_without_bom_handling(&body[..take]);
    if text.contains('\0') && encoding != UTF_16LE && encoding != UTF_16BE {
        return Err(Skipped::Binary);
    }
    let mut out = Collector::for_limits(limits);
    out.push(&text);
    if take < body.len() && !out.full() {
        // The slice ended before the limit was reached: what follows is
        // cut off all the same.
        out.truncated = true;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Extracted, extract as run};

    fn text_of(bytes: &[u8], name: &str) -> Extracted {
        run(bytes, "application/octet-stream", name, &Limits::default())
    }

    fn utf16(text: &str, little: bool, mark: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if mark {
            out.extend(if little { [0xff, 0xfe] } else { [0xfe, 0xff] });
        }
        for unit in text.encode_utf16() {
            out.extend(if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        out
    }

    fn found(text: &str) -> Extracted {
        Extracted::Text {
            text: text.to_owned(),
            truncated: false,
        }
    }

    #[test]
    fn utf16_with_and_without_a_mark_is_decoded() {
        let words = "Grüße aus Zürich, invoice 4711";
        assert_eq!(text_of(&utf16(words, true, true), "a.txt"), found(words));
        assert_eq!(text_of(&utf16(words, false, true), "a.txt"), found(words));
        assert_eq!(text_of(&utf16(words, true, false), "a.txt"), found(words));
        assert_eq!(text_of(&utf16(words, false, false), "a.txt"), found(words));
        // A byte-order mark is enough, whatever the name.
        assert_eq!(text_of(&utf16(words, true, true), "notes"), found(words));
    }

    #[test]
    fn utf8_and_legacy_encodings_are_decoded() {
        assert_eq!(
            text_of("naïve café".as_bytes(), "a.md"),
            found("naïve café")
        );
        assert_eq!(
            text_of(b"\xef\xbb\xbfwith a mark", "a.csv"),
            found("with a mark")
        );
        // Windows-1252.
        assert_eq!(
            text_of(b"na\xefve caf\xe9 cr\xe8me br\xfbl\xe9e", "a.txt"),
            found("naïve café crème brûlée")
        );
    }

    #[test]
    fn binary_data_named_as_text_is_skipped() {
        assert_eq!(
            text_of(b"\x00\x01\x02\x03\x00\x00\xff\x10", "a.txt"),
            Extracted::Skipped(Skipped::Binary)
        );
    }

    #[test]
    fn long_text_is_cut_at_the_limit() {
        let limits = Limits {
            max_text: 10,
            ..Limits::default()
        };
        assert_eq!(
            run(b"0123456789abcdef", "text/plain", "a.txt", &limits),
            Extracted::Text {
                text: "0123456789".to_owned(),
                truncated: true
            }
        );
    }

    #[test]
    fn code_and_data_types_are_text() {
        assert!(is_texty(b"{}", "application/json", "x"));
        assert!(is_texty(b"", "application/octet-stream", "main.RS"));
        assert!(is_texty(b"", "application/ld+json", "x"));
        assert!(!is_texty(b"\x00", "application/octet-stream", "photo.raw"));
    }
}
