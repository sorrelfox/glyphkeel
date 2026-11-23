// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Attached files: their names, their sniffed type, image sizes, and the
//! Base64 and hexadecimal text forms of their bytes.
//!
//! A file's type is read from its first bytes, never trusted from its name
//! or from whoever sent it. The name only tells text formats apart once
//! the bytes are known to be text.

/// The default limit of one attached file, in megabytes (a setting).
pub const DEFAULT_FILE_MAX_MB: u32 = 100;
/// Longest file name, in bytes of UTF-8.
pub const FILE_NAME_MAX_BYTES: usize = 255;

/// Why a file name or encoded data was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    /// Nothing left after cleaning.
    #[error("the file name is empty")]
    EmptyName,
    /// Not valid Base64.
    #[error("the data is not valid Base64 (at character {0})")]
    BadBase64(usize),
}

/// What kind of content a file holds, which decides how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A picture the app can show inline (PNG, JPEG, GIF, WebP, BMP, AVIF,
    /// SVG shown as an image).
    Image,
    /// Text in UTF-8.
    Text,
    /// A program or script: only ever saved, never opened or shown.
    Executable,
    /// Anything else.
    Other,
}

/// The file's name made safe to store and show: the last path component,
/// control characters dropped, white space at the ends trimmed, cut to
/// [`FILE_NAME_MAX_BYTES`] at a character boundary.
pub fn clean_file_name(name: &str) -> Result<String, FileError> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(FileError::EmptyName);
    }
    let mut end = trimmed.len().min(FILE_NAME_MAX_BYTES);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    Ok(trimmed[..end].trim_end().to_owned())
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// The media type of a file, from its bytes (and, for text, its name).
#[must_use]
pub fn sniff(bytes: &[u8], name: &str) -> &'static str {
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    let at = |offset: usize, magic: &[u8]| bytes.get(offset..offset + magic.len()) == Some(magic);
    if starts(b"\x89PNG\r\n\x1a\n") {
        return "image/png";
    }
    if starts(b"\xff\xd8\xff") {
        return "image/jpeg";
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return "image/gif";
    }
    if starts(b"RIFF") && at(8, b"WEBP") {
        return "image/webp";
    }
    if starts(b"BM") && bytes.len() > 26 && at(6, b"\0\0\0\0") {
        return "image/bmp";
    }
    if at(4, b"ftypavif") || at(4, b"ftypavis") {
        return "image/avif";
    }
    if at(4, b"ftyp") {
        return if at(8, b"qt  ") {
            "video/quicktime"
        } else {
            "video/mp4"
        };
    }
    if starts(b"%PDF-") {
        return "application/pdf";
    }
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") {
        return match extension(name).as_str() {
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "odt" => "application/vnd.oasis.opendocument.text",
            "ods" => "application/vnd.oasis.opendocument.spreadsheet",
            _ => "application/zip",
        };
    }
    if starts(b"\x1f\x8b") {
        return "application/gzip";
    }
    if starts(b"7z\xbc\xaf\x27\x1c") {
        return "application/x-7z-compressed";
    }
    if starts(b"\xfd7zXZ\0") {
        return "application/x-xz";
    }
    if starts(b"ID3") || starts(b"\xff\xfb") {
        return "audio/mpeg";
    }
    if starts(b"OggS") {
        return "audio/ogg";
    }
    if starts(b"RIFF") && at(8, b"WAVE") {
        return "audio/wav";
    }
    if starts(b"\x1aE\xdf\xa3") {
        return "video/webm";
    }
    if starts(b"\x7fELF") {
        return "application/x-executable";
    }
    if starts(b"MZ") {
        return "application/x-msdownload";
    }
    if starts(b"\xcf\xfa\xed\xfe") || starts(b"\xfe\xed\xfa\xcf") || starts(b"\xca\xfe\xba\xbe") {
        return "application/x-mach-binary";
    }
    if starts(b"SQLite format 3\0") {
        return "application/vnd.sqlite3";
    }
    if is_text(bytes) {
        return text_type(bytes, name);
    }
    "application/octet-stream"
}

/// Whether `bytes` read as text: UTF-8 (a cut-off last character allowed)
/// without NUL bytes.
fn is_text(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return false;
    }
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && bytes.len() - e.valid_up_to() < 4,
    }
}

fn text_type(bytes: &[u8], name: &str) -> &'static str {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg")) {
        return "image/svg+xml";
    }
    if head.starts_with("#!") {
        return "text/x-shellscript";
    }
    match extension(name).as_str() {
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "xml" => "application/xml",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "sh" | "bash" | "zsh" => "text/x-shellscript",
        "ps1" | "bat" | "cmd" => "text/x-script",
        _ => "text/plain",
    }
}

/// What kind of content a file of type `mime` named `name` holds. Programs
/// and scripts are recognised by their type and by their name.
#[must_use]
pub fn kind_of(mime: &str, name: &str) -> FileKind {
    const RISKY: &[&str] = &[
        "exe", "msi", "bat", "cmd", "com", "scr", "ps1", "vbs", "js", "jse", "wsf", "sh", "bash",
        "zsh", "run", "appimage", "deb", "rpm", "jar", "app", "dmg", "pkg", "desktop", "lnk", "py",
        "pl", "rb",
    ];
    let risky_type = matches!(
        mime,
        "application/x-executable"
            | "application/x-msdownload"
            | "application/x-mach-binary"
            | "text/x-shellscript"
            | "text/x-script"
            | "text/javascript"
            | "text/html"
    );
    if risky_type || RISKY.contains(&extension(name).as_str()) {
        return FileKind::Executable;
    }
    if mime.starts_with("image/") {
        return FileKind::Image;
    }
    if mime.starts_with("text/")
        || matches!(
            mime,
            "application/json" | "application/xml" | "application/yaml" | "application/toml"
        )
    {
        return FileKind::Text;
    }
    FileKind::Other
}

/// Width and height of a PNG, GIF, JPEG, WebP or BMP image, read from its
/// header; `None` for anything else or a broken header.
#[must_use]
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be16 = |i: usize| -> Option<u32> {
        Some(u32::from(u16::from_be_bytes(
            bytes.get(i..i + 2)?.try_into().ok()?,
        )))
    };
    let le16 = |i: usize| -> Option<u32> {
        Some(u32::from(u16::from_le_bytes(
            bytes.get(i..i + 2)?.try_into().ok()?,
        )))
    };
    let be32 = |i: usize| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(i..i + 4)?.try_into().ok()?))
    };
    let le32 = |i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?))
    };
    let le24 = |i: usize| -> Option<u32> {
        let b = bytes.get(i..i + 3)?;
        Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
    };
    let size = match sniff(&bytes[..bytes.len().min(64)], "") {
        "image/png" => (be32(16)?, be32(20)?),
        "image/gif" => (le16(6)?, le16(8)?),
        "image/bmp" => {
            let h = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
            (le32(18)?, h.unsigned_abs())
        }
        "image/webp" => match bytes.get(12..16)? {
            b"VP8 " => (le16(26)? & 0x3fff, le16(28)? & 0x3fff),
            b"VP8L" => {
                let v = le32(21)?;
                ((v & 0x3fff) + 1, ((v >> 14) & 0x3fff) + 1)
            }
            b"VP8X" => (le24(24)? + 1, le24(27)? + 1),
            _ => return None,
        },
        "image/jpeg" => {
            let mut i = 2;
            loop {
                while bytes.get(i) == Some(&0xff) {
                    i += 1;
                }
                let marker = *bytes.get(i)?;
                i += 1;
                if matches!(marker, 0xd8 | 0x01 | 0xd0..=0xd7) {
                    continue;
                }
                let len = usize::try_from(be16(i)?).ok()?;
                // Start-of-frame markers, except DHT, JPG and DAC.
                if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                    break (be16(i + 5)?, be16(i + 3)?);
                }
                if len < 2 {
                    return None;
                }
                i += len;
            }
        }
        _ => return None,
    };
    (size.0 > 0 && size.1 > 0).then_some(size)
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in standard Base64 with padding.
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(BASE64[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes standard or URL-safe Base64, with or without padding; white
/// space is ignored.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, FileError> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits, mut ended) = (0u32, 0u32, false);
    for (i, c) in text.bytes().enumerate() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => {
                ended = true;
                continue;
            }
            c if c.is_ascii_whitespace() => continue,
            _ => return Err(FileError::BadBase64(i)),
        };
        if ended {
            return Err(FileError::BadBase64(i));
        }
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if bits >= 6 {
        return Err(FileError::BadBase64(text.len()));
    }
    Ok(out)
}

/// `bytes` as lower-case hexadecimal digits.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_only_their_last_part() {
        assert_eq!(clean_file_name("../../etc/passwd").as_deref(), Ok("passwd"));
        assert_eq!(
            clean_file_name("C:\\Users\\me\\Report 1.pdf").as_deref(),
            Ok("Report 1.pdf")
        );
        assert_eq!(clean_file_name(" a\u{7}b.txt ").as_deref(), Ok("ab.txt"));
        assert_eq!(clean_file_name("dir/"), Err(FileError::EmptyName));
        assert_eq!(clean_file_name(".."), Err(FileError::EmptyName));
        let long = "é".repeat(200);
        let cut = clean_file_name(&long).unwrap();
        assert!(cut.len() <= FILE_NAME_MAX_BYTES);
        assert!(cut.chars().all(|c| c == 'é'));
    }

    #[test]
    fn types_come_from_the_bytes_not_the_name() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....", "x.txt"), "image/png");
        assert_eq!(sniff(b"\xff\xd8\xff\xe0", "x.png"), "image/jpeg");
        assert_eq!(sniff(b"%PDF-1.7", "x"), "application/pdf");
        assert_eq!(
            sniff(b"\x7fELF\x02", "notes.txt"),
            "application/x-executable"
        );
        assert_eq!(sniff(b"# Title\n", "notes.md"), "text/markdown");
        assert_eq!(sniff(b"plain", "notes"), "text/plain");
        assert_eq!(sniff(b"a\0b", "notes.txt"), "application/octet-stream");
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?>\n<svg xmlns=\"\"/>", "a.xml"),
            "image/svg+xml"
        );
        assert_eq!(sniff(b"#!/bin/sh\nrm -rf /", "x.txt"), "text/x-shellscript");
        assert_eq!(sniff("Привет".as_bytes(), "a.txt"), "text/plain");
        // A last character cut in half is still text.
        assert_eq!(sniff(&"Привет".as_bytes()[..5], "a.txt"), "text/plain");
    }

    #[test]
    fn programs_and_scripts_are_never_shown() {
        assert_eq!(kind_of("image/png", "a.png"), FileKind::Image);
        assert_eq!(kind_of("image/svg+xml", "a.svg"), FileKind::Image);
        assert_eq!(kind_of("text/plain", "run.sh"), FileKind::Executable);
        assert_eq!(kind_of("text/html", "page.html"), FileKind::Executable);
        assert_eq!(
            kind_of("application/x-msdownload", "setup"),
            FileKind::Executable
        );
        assert_eq!(kind_of("text/markdown", "a.md"), FileKind::Text);
        assert_eq!(kind_of("application/json", "a.json"), FileKind::Text);
        assert_eq!(kind_of("application/pdf", "a.pdf"), FileKind::Other);
    }

    #[test]
    fn image_sizes_come_from_their_headers() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(image_size(&png), Some((640, 480)));
        let gif = b"GIF89a\x20\x03\x58\x02rest";
        assert_eq!(image_size(gif), Some((800, 600)));
        // SOI, an APP0 segment of 4 bytes, then SOF0 with height 2, width 3.
        let jpeg = b"\xff\xd8\xff\xe0\x00\x04ab\xff\xc0\x00\x11\x08\x00\x02\x00\x03\x03";
        assert_eq!(image_size(jpeg), Some((3, 2)));
        assert_eq!(image_size(b"plain text"), None);
        assert_eq!(image_size(b"\x89PNG\r\n\x1a\n"), None);
    }

    #[test]
    fn base64_round_trips() {
        for data in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            let text = base64_encode(data);
            assert_eq!(base64_decode(&text).unwrap(), data);
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_decode("Zm8").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(base64_decode("-_8=").unwrap(), [0xfb, 0xff]);
        assert!(base64_decode("Zm9v!").is_err());
        assert!(base64_decode("Zg=a").is_err());
        assert!(base64_decode("Z").is_err());
    }

    #[test]
    fn hex_is_lower_case() {
        assert_eq!(hex(&[0, 0xab, 0x10]), "00ab10");
    }
}
