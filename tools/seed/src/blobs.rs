// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Contents of attached files: plain text, Markdown, CSV, JSON, SVG and PNG.

use std::fmt::Write as _;

use crate::rng::Rng;

/// A generated file.
#[derive(Debug, Clone)]
pub struct Blob {
    pub file_name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

const HUES: &[(u8, u8, u8)] = &[
    (0x35, 0x84, 0xe4),
    (0x2e, 0xc2, 0x7e),
    (0xf6, 0xd3, 0x2d),
    (0xff, 0x78, 0x00),
    (0xe0, 0x1b, 0x24),
    (0x91, 0x41, 0xac),
];

pub fn generate(rng: &mut Rng, n: usize) -> Blob {
    match rng.below(6) {
        0 => text_log(rng, n),
        1 => markdown_note(rng, n),
        2 => csv(rng, n),
        3 => json(rng, n),
        4 => svg(rng, n),
        _ => png(rng, n),
    }
}

fn text_log(rng: &mut Rng, n: usize) -> Blob {
    let lines = rng.range(5, 1500);
    let mut s = String::new();
    for i in 0..lines {
        let _ = writeln!(
            s,
            "2026-02-{:02}T{:02}:{:02}:{:02}Z {} step {i}: {} ms",
            rng.range(1, 28),
            rng.range(0, 23),
            rng.range(0, 59),
            rng.range(0, 59),
            rng.pick(&["INFO", "INFO", "INFO", "WARN", "ERROR", "DEBUG"]),
            rng.range(1, 2000)
        );
    }
    Blob {
        file_name: format!("build-log-{n:04}.txt"),
        mime: "text/plain".into(),
        bytes: s.into_bytes(),
        width: None,
        height: None,
    }
}

fn markdown_note(rng: &mut Rng, n: usize) -> Blob {
    let mut s = format!("# Notes {n}\n\n");
    for i in 0..rng.range(2, 30) {
        let _ = writeln!(s, "- point {i}: value {}", rng.range(1, 999));
    }
    Blob {
        file_name: format!("notes-{n:04}.md"),
        mime: "text/markdown".into(),
        bytes: s.into_bytes(),
        width: None,
        height: None,
    }
}

fn csv(rng: &mut Rng, n: usize) -> Blob {
    let mut s = String::from("task,estimate_h,spent_h\n");
    for i in 0..rng.range(3, 800) {
        let _ = writeln!(s, "row-{i},{},{}", rng.range(1, 40), rng.range(0, 60));
    }
    Blob {
        file_name: format!("estimates-{n:04}.csv"),
        mime: "text/csv".into(),
        bytes: s.into_bytes(),
        width: None,
        height: None,
    }
}

fn json(rng: &mut Rng, n: usize) -> Blob {
    let mut s = String::from("{\"samples\":[");
    for i in 0..rng.range(1, 400) {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{{\"run\":{i},\"ms\":{}}}", rng.range(1, 999));
    }
    s.push_str("]}\n");
    Blob {
        file_name: format!("measurements-{n:04}.json"),
        mime: "application/json".into(),
        bytes: s.into_bytes(),
        width: None,
        height: None,
    }
}

fn svg(rng: &mut Rng, n: usize) -> Blob {
    let w = rng.range(64, 640) as u32;
    let h = rng.range(64, 480) as u32;
    let (r, g, b) = *rng.pick(HUES);
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n"
    );
    for _ in 0..rng.range(1, 40) {
        let _ = writeln!(
            s,
            "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{r:02x}{g:02x}{b:02x}\" opacity=\"0.{}\"/>",
            rng.range(0, i64::from(w) - 1),
            rng.range(0, i64::from(h) - 1),
            rng.range(4, 60),
            rng.range(4, 60),
            rng.range(2, 9)
        );
    }
    s.push_str("</svg>\n");
    Blob {
        file_name: format!("diagram-{n:04}.svg"),
        mime: "image/svg+xml".into(),
        bytes: s.into_bytes(),
        width: Some(w),
        height: Some(h),
    }
}

fn png(rng: &mut Rng, n: usize) -> Blob {
    let w = rng.range(8, 96) as u32;
    let h = rng.range(8, 96) as u32;
    let (r, g, b) = *rng.pick(HUES);
    // Horizontal bands, one colour each, so thumbnails are not all alike.
    let band = rng.range(2, 16) as u32;
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0); // filter: none
        let shade = if (y / band).is_multiple_of(2) { 0 } else { 40 };
        for _ in 0..w {
            raw.extend_from_slice(&[
                r.saturating_sub(shade),
                g.saturating_sub(shade),
                b.saturating_sub(shade),
            ]);
        }
    }
    Blob {
        file_name: format!("screenshot-{n:04}.png"),
        mime: "image/png".into(),
        bytes: encode_png(w, h, &raw),
        width: Some(w),
        height: Some(h),
    }
}

/// A valid RGB PNG with uncompressed ("stored") deflate blocks.
fn encode_png(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);

    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, part) in chunks.iter().enumerate() {
        z.push(u8::from(i + 1 == chunks.len()));
        let len = part.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(part);
    }
    z.extend_from_slice(&adler32(raw).to_be_bytes());
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn png_header() {
        let mut rng = Rng::new(3, 0);
        let blob = png(&mut rng, 1);
        assert_eq!(&blob.bytes[1..4], b"PNG");
        assert_eq!(&blob.bytes[12..16], b"IHDR");
        assert_eq!(
            &blob.bytes[blob.bytes.len() - 8..blob.bytes.len() - 4],
            b"IEND"
        );
    }
}
