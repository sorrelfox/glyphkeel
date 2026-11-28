// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! PDF: the text layer of each page, page by page, until the text limit.
//! Scanned pages carry only images and give nothing.

use lopdf::{Document, LoadOptions};

use crate::{Collector, Limits, Skipped};

pub(crate) fn extract(bytes: &[u8], limits: &Limits) -> Result<Collector, Skipped> {
    let options = LoadOptions::with_max_decompressed_size(limits.max_pdf_stream);
    let document = match Document::load_mem_with_options(bytes, options) {
        Ok(d) => d,
        Err(lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword) => {
            return Err(Skipped::Encrypted);
        }
        Err(_) => return Err(Skipped::Malformed),
    };
    // A document that needs a password other than the empty one stays
    // encrypted after loading.
    if document.is_encrypted() && document.encryption_state.is_none() {
        return Err(Skipped::Encrypted);
    }
    let mut out = Collector::for_limits(limits);
    let pages: Vec<u32> = document
        .get_pages()
        .keys()
        .copied()
        .take(limits.max_pages)
        .collect();
    for number in pages {
        // A page that can't be read is left out; the others still count.
        for text in document
            .extract_text_chunks_with_limit(&[number], limits.max_pdf_stream)
            .into_iter()
            .flatten()
        {
            out.push(&text);
            out.space();
        }
        out.line_break();
        if out.full() {
            break;
        }
    }
    Ok(out)
}
