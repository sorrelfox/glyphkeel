// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Office Open XML (`.docx`, `.xlsx`, `.pptx`) and OpenDocument (`.odt`,
//! `.ods`, `.odp`): ZIP archives of XML parts, whose text nodes are read.
//!
//! The archive's directory is checked before anything is unpacked: the
//! number of entries, every entry's declared size, the total, and the
//! ratio of unpacked to packed size of large entries. Reading an entry is
//! capped again by what it really inflates to, so a directory that lies
//! about sizes gains nothing.
//!
//! The XML reader only reports events: it never reads a DTD, and of the
//! references in text only the five predefined entities and character
//! references are resolved. Any other entity is left out.

use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

use crate::{Collector, Limits, Skipped};

/// The extensions of the Office files read; a plain `.zip` is not.
const EXTENSIONS: &[&str] = &[
    "docx", "docm", "dotx", "xlsx", "xlsm", "xltx", "pptx", "pptm", "potx", "odt", "ott", "ods",
    "ots", "odp", "otp",
];

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Whether a ZIP file is one of the Office formats, by its type or name.
pub(crate) fn is_office(mime: &str, name: &str) -> bool {
    mime.starts_with("application/vnd.openxmlformats-officedocument.")
        || mime.starts_with("application/vnd.oasis.opendocument.")
        || EXTENSIONS.contains(&extension(name).as_str())
}

/// Which parts hold the text, and how it is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// Office Open XML: text in `t` elements (`w:t`, `a:t`, a spreadsheet's
    /// `t`); paragraphs, rows and string items end lines.
    Ooxml,
    /// OpenDocument: every text node in the body.
    Odf,
}

/// The XML parts of an Office Open XML file that hold its text, in
/// reading order.
fn ooxml_part(name: &str) -> Option<(u8, u32)> {
    let numbered = |prefix: &str| {
        name.strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(".xml"))
            .and_then(|n| n.parse::<u32>().ok())
    };
    match name {
        "word/document.xml" => Some((0, 0)),
        "word/footnotes.xml" => Some((2, 0)),
        "word/endnotes.xml" => Some((2, 1)),
        "word/comments.xml" => Some((3, 0)),
        "xl/sharedStrings.xml" => Some((0, 0)),
        _ => numbered("word/header")
            .map(|n| (1, n))
            .or_else(|| numbered("word/footer").map(|n| (1, 1000 + n)))
            .or_else(|| numbered("ppt/slides/slide").map(|n| (0, n)))
            .or_else(|| numbered("ppt/notesSlides/notesSlide").map(|n| (1, n)))
            // Cells written as inline strings rather than shared ones.
            .or_else(|| numbered("xl/worksheets/sheet").map(|n| (1, n))),
    }
}

pub(crate) fn extract(bytes: &[u8], limits: &Limits) -> Result<Collector, Skipped> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| Skipped::Malformed)?;
    if zip.len() > limits.max_entries {
        return Err(Skipped::TooManyEntries);
    }
    let mut out = Collector::for_limits(limits);
    let mut total = 0u64;
    let mut parts: Vec<((u8, u32), usize)> = Vec::new();
    let mut layout = None;
    for index in 0..zip.len() {
        out.in_time()?;
        let entry = zip.by_index_raw(index).map_err(|_| Skipped::Malformed)?;
        let size = entry.size();
        let packed = entry.compressed_size().max(1);
        if size > limits.max_entry {
            return Err(Skipped::Bomb);
        }
        if size > 1024 * 1024 && size / packed > limits.max_ratio {
            return Err(Skipped::Bomb);
        }
        total = total.saturating_add(size);
        if total > limits.max_unpacked {
            return Err(Skipped::Bomb);
        }
        let name = std::str::from_utf8(entry.name_raw()).unwrap_or("");
        if name == "[Content_Types].xml" {
            layout = Some(Layout::Ooxml);
        } else if name == "mimetype" && layout.is_none() {
            layout = Some(Layout::Odf);
        }
        if let Some(order) = ooxml_part(name) {
            parts.push((order, index));
        } else if name == "content.xml" {
            parts.push(((0, 0), index));
        }
        if entry.encrypted() && (name == "content.xml" || ooxml_part(name).is_some()) {
            return Err(Skipped::Encrypted);
        }
    }
    let Some(layout) = layout else {
        return Err(Skipped::Malformed);
    };
    // An encrypted Office Open XML file is not a ZIP archive at all, and an
    // OpenDocument file keeps its manifest readable but its content not.
    if layout == Layout::Odf
        && let Some(index) = zip.index_for_name("META-INF/manifest.xml")
    {
        let manifest = read_entry(&mut zip, index, limits.max_entry, &mut total)?;
        if contains(&manifest, b"encryption-data") {
            return Err(Skipped::Encrypted);
        }
    }
    parts.sort();
    let mut unpacked = 0u64;
    for (_, index) in parts {
        let name = zip
            .name_for_index(index)
            .and_then(Result::ok)
            .map(|n| n.into_owned())
            .unwrap_or_default();
        let wanted = match layout {
            Layout::Ooxml => name != "content.xml",
            Layout::Odf => name == "content.xml",
        };
        if !wanted {
            continue;
        }
        let xml = read_entry(&mut zip, index, limits.max_entry, &mut unpacked)?;
        if unpacked > limits.max_unpacked {
            return Err(Skipped::Bomb);
        }
        match layout {
            Layout::Ooxml => ooxml_text(&xml, &mut out)?,
            Layout::Odf => odf_text(&xml, &mut out)?,
        }
        out.line_break();
        if out.full() {
            break;
        }
    }
    Ok(out)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Entry `index`, unpacked, failing as a bomb beyond `max` bytes whatever
/// its declared size; adds what it read to `counted`.
fn read_entry(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    index: usize,
    max: u64,
    counted: &mut u64,
) -> Result<Vec<u8>, Skipped> {
    let entry = zip.by_index(index).map_err(|e| match e {
        zip::result::ZipError::UnsupportedArchive(m) if m.contains("assword") => Skipped::Encrypted,
        _ => Skipped::Malformed,
    })?;
    let mut out = Vec::new();
    entry
        .take(max + 1)
        .read_to_end(&mut out)
        .map_err(|_| Skipped::Malformed)?;
    if out.len() as u64 > max {
        return Err(Skipped::Bomb);
    }
    *counted = counted.saturating_add(out.len() as u64);
    Ok(out)
}

fn reader(xml: &[u8]) -> Reader<&[u8]> {
    let mut reader = Reader::from_reader(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.check_end_names = false;
    config.allow_unmatched_ends = true;
    config.allow_dangling_amp = true;
    reader
}

/// The text of a reference in text: a predefined entity or a character
/// reference; anything else (an entity a DTD would define) is dropped.
fn resolve(reference: &quick_xml::events::BytesRef<'_>) -> Option<String> {
    if let Ok(Some(c)) = reference.resolve_char_ref() {
        return Some(c.to_string());
    }
    let name = reference.xml10_content();
    quick_xml::escape::resolve_predefined_entity(&name).map(str::to_owned)
}

fn ooxml_text(xml: &[u8], out: &mut Collector) -> Result<(), Skipped> {
    let mut reader = reader(xml);
    let mut buf = Vec::new();
    // Inside a `t` element (text), at what depth of nesting.
    let mut in_text = 0u32;
    // A mistake in the XML ends the text; what was read so far counts.
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(e) => {
                if e.local_name().as_ref() == "t" {
                    in_text += 1;
                }
            }
            Event::Empty(e) => match e.local_name().as_ref() {
                "tab" => out.push("\t"),
                "br" | "cr" => out.line_break(),
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "t" => in_text = in_text.saturating_sub(1),
                // Paragraphs, table cells, rows and shared strings.
                "p" | "tc" | "si" | "row" | "is" => out.line_break(),
                _ => {}
            },
            Event::Text(t) if in_text > 0 => out.push(&t.xml10_content()),
            Event::CData(t) if in_text > 0 => out.push(&t.xml10_content()),
            Event::GeneralRef(r) if in_text > 0 => {
                if let Some(s) = resolve(&r) {
                    out.push(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if out.full() {
            break;
        }
        buf.clear();
    }
    Ok(())
}

fn odf_text(xml: &[u8], out: &mut Collector) -> Result<(), Skipped> {
    let mut reader = reader(xml);
    let mut buf = Vec::new();
    let mut in_body = false;
    // Text counts only inside paragraphs and headings; the white space
    // between elements is layout.
    let mut in_paragraph = 0u32;
    // A mistake in the XML ends the text; what was read so far counts.
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(e) => match e.local_name().as_ref() {
                "body" => in_body = true,
                "p" | "h" => in_paragraph += 1,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "body" => in_body = false,
                "p" | "h" => {
                    in_paragraph = in_paragraph.saturating_sub(1);
                    out.line_break();
                }
                "table-cell" | "list-item" => out.line_break(),
                _ => {}
            },
            Event::Empty(e) if in_body => match e.local_name().as_ref() {
                "s" => out.push(" "),
                "tab" => out.push("\t"),
                "line-break" => out.line_break(),
                _ => {}
            },
            Event::Text(t) if in_body && in_paragraph > 0 => out.push(&t.xml10_content()),
            Event::CData(t) if in_body && in_paragraph > 0 => out.push(&t.xml10_content()),
            Event::GeneralRef(r) if in_body && in_paragraph > 0 => {
                if let Some(s) = resolve(&r) {
                    out.push(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if out.full() {
            break;
        }
        buf.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(f: fn(&[u8], &mut Collector) -> Result<(), Skipped>, xml: &str) -> String {
        let mut out = Collector::new(1 << 20);
        f(xml.as_bytes(), &mut out).expect("read");
        match out.finish() {
            crate::Extracted::Text { text, .. } => text,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn word_text_runs_and_paragraphs() {
        let xml = r#"<?xml version="1.0"?><w:document xmlns:w="x"><w:body>
            <w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t xml:space="preserve"> world</w:t></w:r></w:p>
            <w:p><w:r><w:t>Fish &amp; chips &#169;</w:t><w:tab/><w:t>x</w:t></w:r></w:p>
            <w:p><w:r><w:instrText>PAGE</w:instrText></w:r></w:p>
            </w:body></w:document>"#;
        assert_eq!(collect(ooxml_text, xml), "Hello world\nFish & chips ©\tx");
    }

    #[test]
    fn entities_from_a_dtd_are_never_expanded() {
        let xml = r#"<?xml version="1.0"?>
            <!DOCTYPE w [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;">]>
            <w:document xmlns:w="x"><w:p><w:t>start &b; end</w:t></w:p></w:document>"#;
        assert_eq!(collect(ooxml_text, xml), "start  end");
    }

    #[test]
    fn open_document_body_text() {
        let xml = r#"<office:document-content xmlns:office="o" xmlns:text="t">
            <office:meta><dc:title>not this</dc:title></office:meta>
            <office:body><office:text><text:h>Title</text:h>
            <text:p>one<text:s/>two<text:line-break/>three</text:p></office:text></office:body>
            </office:document-content>"#;
        assert_eq!(collect(odf_text, xml), "Title\none two\nthree");
    }

    #[test]
    fn office_names_and_types() {
        assert!(is_office("application/zip", "Report.DOCX"));
        assert!(is_office("application/vnd.oasis.opendocument.text", "x"));
        assert!(!is_office("application/zip", "photos.zip"));
    }
}
