// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Each kind of file gives its text, or a clean skip.

mod fixtures;

use std::sync::Arc;

use gk_extract::{Extracted, Limits, Skipped, extract, extract_bounded};

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const ODT: &str = "application/vnd.oasis.opendocument.text";

fn text(found: Extracted) -> String {
    match found {
        Extracted::Text { text, .. } => text,
        other => panic!("no text: {other:?}"),
    }
}

#[test]
fn a_text_pdf_gives_its_text_layer() {
    let pdf = fixtures::text_pdf("The zebracorn invoice is overdue");
    let found = text(extract(
        &pdf,
        "application/pdf",
        "a.pdf",
        &Limits::default(),
    ));
    assert!(found.contains("zebracorn invoice"), "{found:?}");
}

#[test]
fn a_scanned_pdf_has_no_text() {
    let pdf = fixtures::scanned_pdf();
    assert_eq!(
        extract(&pdf, "application/pdf", "scan.pdf", &Limits::default()),
        Extracted::Skipped(Skipped::NoText)
    );
}

#[test]
fn a_broken_pdf_is_skipped() {
    let mut pdf = fixtures::text_pdf("cut off");
    pdf.truncate(30);
    assert!(matches!(
        extract(&pdf, "application/pdf", "a.pdf", &Limits::default()),
        Extracted::Skipped(_)
    ));
}

#[test]
fn a_word_document_gives_its_paragraphs() {
    let docx = fixtures::docx("Quarterly figures", "Mention of the platypus budget");
    assert_eq!(
        text(extract(&docx, DOCX, "report.docx", &Limits::default())),
        "Quarterly figures\nMention of the platypus budget"
    );
}

#[test]
fn a_spreadsheet_gives_its_strings() {
    let xlsx = fixtures::xlsx(&["Revenue", "Aardvark Ltd"], "inline note");
    assert_eq!(
        text(extract(&xlsx, XLSX, "sheet.xlsx", &Limits::default())),
        "Revenue\nAardvark Ltd\ninline note"
    );
}

#[test]
fn an_open_document_text_gives_its_body() {
    let odt = fixtures::odt("Minutes", "We agreed on the narwhal plan");
    assert_eq!(
        text(extract(&odt, ODT, "minutes.odt", &Limits::default())),
        "Minutes\nWe agreed on the narwhal plan"
    );
}

#[test]
fn utf16_text_is_decoded() {
    let bytes = fixtures::utf16_text("Ünïcödé notes about the axolotl");
    // Sniffing calls UTF-16 binary data; the byte-order mark is enough.
    assert_eq!(
        text(extract(
            &bytes,
            "application/octet-stream",
            "notes.txt",
            &Limits::default()
        )),
        "Ünïcödé notes about the axolotl"
    );
}

#[test]
fn a_zip_bomb_named_docx_is_refused_before_unpacking() {
    let bomb = fixtures::bomb_docx(8);
    assert!(bomb.len() < 100_000, "{}", bomb.len());
    assert_eq!(
        extract(&bomb, DOCX, "bomb.docx", &Limits::default()),
        Extracted::Skipped(Skipped::Bomb)
    );
    // Over the size of one entry too.
    let limits = Limits {
        max_entry: 1024 * 1024,
        max_ratio: u64::MAX,
        ..Limits::default()
    };
    assert_eq!(
        extract(&bomb, DOCX, "bomb.docx", &limits),
        Extracted::Skipped(Skipped::Bomb)
    );
}

#[test]
fn an_archive_of_too_many_entries_is_refused() {
    let names: Vec<String> = (0..50).map(|i| format!("word/media/{i}.xml")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("[Content_Types].xml", b"<Types/>")];
    entries.extend(names.iter().map(|n| (n.as_str(), &b"x"[..])));
    let zip = fixtures::zip_of(&entries);
    let limits = Limits {
        max_entries: 20,
        ..Limits::default()
    };
    assert_eq!(
        extract(&zip, DOCX, "many.docx", &limits),
        Extracted::Skipped(Skipped::TooManyEntries)
    );
}

#[test]
fn a_plain_zip_is_not_read() {
    let zip = fixtures::zip_of(&[("notes.txt", b"hidden words")]);
    assert_eq!(
        extract(&zip, "application/zip", "photos.zip", &Limits::default()),
        Extracted::Unsupported
    );
}

#[test]
fn the_text_limit_holds_for_office_files() {
    let long = "lorem ".repeat(10_000);
    let docx = fixtures::docx(&long, "tail");
    let limits = Limits {
        max_text: 1000,
        ..Limits::default()
    };
    match extract(&docx, DOCX, "long.docx", &limits) {
        Extracted::Text { text, truncated } => {
            assert!(truncated);
            assert!(text.len() <= 1000);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn bounded_extraction_gives_the_same_text() {
    let docx = fixtures::docx("on a thread", "of its own");
    assert_eq!(
        extract_bounded(
            Arc::from(docx.as_slice()),
            DOCX,
            "a.docx",
            &Limits::default()
        ),
        extract(&docx, DOCX, "a.docx", &Limits::default())
    );
}

/// Writes the fuzz targets' seeds from these fixtures:
/// `GK_FUZZ_SEEDS=fuzz/seeds cargo test -p gk-extract --test formats -- --ignored`.
#[test]
#[ignore = "writes the fuzz seeds"]
fn write_fuzz_seeds() {
    let Some(root) = std::env::var_os("GK_FUZZ_SEEDS") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let pdf = root.join("extract_pdf");
    let office = root.join("extract_office");
    std::fs::create_dir_all(&pdf).expect("folder");
    std::fs::create_dir_all(&office).expect("folder");
    std::fs::write(pdf.join("text.pdf"), fixtures::text_pdf("A page of text")).expect("seed");
    std::fs::write(pdf.join("scanned.pdf"), fixtures::scanned_pdf()).expect("seed");
    // The first byte picks the name: 0 Word, 1 Excel, 3 OpenDocument.
    let with = |pick: u8, bytes: Vec<u8>| [vec![pick], bytes].concat();
    let seeds = [
        (
            "docx",
            with(0, fixtures::docx("Heading", "a paragraph &amp; more")),
        ),
        ("xlsx", with(1, fixtures::xlsx(&["one", "two"], "three"))),
        ("odt", with(3, fixtures::odt("Heading", "a paragraph"))),
        ("bomb.docx", with(0, fixtures::bomb_docx(2))),
    ];
    for (name, bytes) in seeds {
        std::fs::write(office.join(name), bytes).expect("seed");
    }
}

#[test]
fn readers_stop_by_themselves_when_the_time_is_up() {
    let limits = Limits {
        max_time: std::time::Duration::from_nanos(1),
        ..Limits::default()
    };
    // Thousands of paragraphs: the XML loop sees the clock long before the
    // end.
    let many = "<w:p><w:r><w:t>word</w:t></w:r></w:p>".repeat(20_000);
    let docx = fixtures::zip_of(&[
        ("[Content_Types].xml", b"<Types/>"),
        (
            "word/document.xml",
            format!("<w:document xmlns:w=\"x\"><w:body>{many}</w:body></w:document>").as_bytes(),
        ),
    ]);
    assert_eq!(
        extract(&docx, DOCX, "slow.docx", &limits),
        Extracted::Skipped(Skipped::TimedOut)
    );
    let text = "word ".repeat(100_000);
    assert_eq!(
        extract(text.as_bytes(), "text/plain", "slow.txt", &limits),
        Extracted::Skipped(Skipped::TimedOut)
    );
    let pages = fixtures::text_pdf_pages("page text", 200);
    assert_eq!(
        extract(&pages, "application/pdf", "slow.pdf", &limits),
        Extracted::Skipped(Skipped::TimedOut)
    );
}
