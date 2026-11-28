// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Small files of every kind the extraction reads, made in code so the
//! tests need no binary files and no other programs. The engine's tests
//! include this module too.

#![allow(dead_code, unreachable_pub)]

use std::io::Write as _;

use zip::write::SimpleFileOptions;

/// A PDF of one page whose text layer says `text` (ASCII, no parentheses
/// or backslashes), in the standard Helvetica font.
pub fn text_pdf(text: &str) -> Vec<u8> {
    let content = format!("BT /F1 18 Tf 72 720 Td ({text}) Tj ET");
    pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_owned(),
        stream(&content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ])
}

/// A PDF of `pages` pages, each showing `text`: slow to read when there
/// are many.
pub fn text_pdf_pages(text: &str, pages: usize) -> Vec<u8> {
    // Objects: 1 catalog, 2 pages, 3 font, then a page and its content for
    // each page.
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            kids.join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    for i in 0..pages {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            5 + 2 * i
        ));
        objects.push(stream(&format!(
            "BT /F1 18 Tf 72 720 Td ({text} {i}) Tj ET"
        )));
    }
    pdf(&objects)
}

/// A PDF of one page that only shows an image, as a scanner makes them:
/// no text layer.
pub fn scanned_pdf() -> Vec<u8> {
    let pixels = "x".repeat(4 * 4);
    pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>"
            .to_owned(),
        stream("q 612 0 0 792 0 0 cm /Im1 Do Q"),
        format!(
            "<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Length {} >>\nstream\n{pixels}\nendstream",
            pixels.len()
        ),
    ])
}

fn stream(content: &str) -> String {
    format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    )
}

/// A PDF of `objects` (numbered from 1), with a correct cross-reference
/// table.
fn pdf(objects: &[String]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for at in offsets {
        out.extend(format!("{at:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

/// A ZIP archive of `entries`, compressed.
pub fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default())
            .expect("entry");
        zip.write_all(bytes).expect("write");
    }
    zip.finish().expect("finish").into_inner()
}

const CONTENT_TYPES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#;

/// A Word document of two paragraphs.
pub fn docx(first: &str, second: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t>{first}</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">{second}</w:t></w:r></w:p>
</w:body></w:document>"#
    );
    zip_of(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("word/document.xml", document.as_bytes()),
    ])
}

/// A spreadsheet whose cells hold `cells` as shared strings, and one
/// inline string.
pub fn xlsx(cells: &[&str], inline: &str) -> Vec<u8> {
    let items: String = cells
        .iter()
        .map(|c| format!("<si><t>{c}</t></si>"))
        .collect();
    let strings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{n}" uniqueCount="{n}">{items}</sst>"#,
        n = cells.len()
    );
    let sheet = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>42</v></c></row>
<row r="2"><c r="A2" t="inlineStr"><is><t>{inline}</t></is></c></row></sheetData></worksheet>"#
    );
    zip_of(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("xl/sharedStrings.xml", strings.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

/// An OpenDocument text with a heading and a paragraph.
pub fn odt(heading: &str, paragraph: &str) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
<office:body><office:text><text:h>{heading}</text:h><text:p>{paragraph}</text:p></office:text></office:body>
</office:document-content>"#
    );
    zip_of(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
        (
            "META-INF/manifest.xml",
            br#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"/>"#,
        ),
    ])
}

/// `text` as UTF-16 (little-endian) with a byte-order mark, as Windows
/// programs write it.
pub fn utf16_text(text: &str) -> Vec<u8> {
    let mut out = vec![0xff, 0xfe];
    for unit in text.encode_utf16() {
        out.extend(unit.to_le_bytes());
    }
    out
}

/// A ZIP bomb named like a Word document: its document part unpacks from
/// a few kilobytes to `megabytes` megabytes.
pub fn bomb_docx(megabytes: usize) -> Vec<u8> {
    let filler = vec![b' '; megabytes * 1024 * 1024];
    zip_of(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("word/document.xml", &filler),
    ])
}
