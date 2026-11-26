// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A CSV file's bytes as text: its encoding detected (a byte order mark,
//! UTF-16 without one, valid UTF-8, else Windows-1252) or given.

use gk_api::ops::csv_import::CsvEncoding;

/// A file's text and how it was stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decoded {
    pub(crate) text: String,
    pub(crate) encoding: CsvEncoding,
    pub(crate) bom: bool,
}

/// Why bytes are not text in an encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodeError {
    /// Not valid in the encoding, from this byte on.
    Invalid(CsvEncoding, usize),
    /// Holds a NUL character: not a text file.
    Binary,
}

/// The characters of Windows-1252 at 0x80 to 0x9F; the five it leaves
/// out stay C1 control characters, as in Latin-1.
const WINDOWS_1252: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

/// The encoding of `bytes`, and whether they start with a byte order mark.
fn detect(bytes: &[u8]) -> (CsvEncoding, bool) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (CsvEncoding::Utf8, true);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return (CsvEncoding::Utf16le, true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return (CsvEncoding::Utf16be, true);
    }
    // UTF-16 without a mark: text of mostly ASCII has a zero in every
    // other byte.
    let head = &bytes[..bytes.len().min(4096)];
    if head.len() >= 4 {
        let pairs = head.len() / 2;
        let even = head.iter().step_by(2).filter(|b| **b == 0).count();
        let odd = head.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
        if odd * 10 >= pairs * 3 && even * 10 < pairs {
            return (CsvEncoding::Utf16le, false);
        }
        if even * 10 >= pairs * 3 && odd * 10 < pairs {
            return (CsvEncoding::Utf16be, false);
        }
    }
    if std::str::from_utf8(bytes).is_ok() {
        (CsvEncoding::Utf8, false)
    } else {
        (CsvEncoding::Windows1252, false)
    }
}

fn utf16(bytes: &[u8], little: bool, encoding: CsvEncoding) -> Result<String, DecodeError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(DecodeError::Invalid(encoding, bytes.len() - 1));
    }
    let units = bytes.as_chunks::<2>().0.iter().map(|p| {
        if little {
            u16::from_le_bytes(*p)
        } else {
            u16::from_be_bytes(*p)
        }
    });
    let mut out = String::with_capacity(bytes.len() / 2);
    for (i, c) in char::decode_utf16(units).enumerate() {
        out.push(c.map_err(|_| DecodeError::Invalid(encoding, i * 2))?);
    }
    Ok(out)
}

/// `bytes` as text, in `forced` or the detected encoding.
pub(crate) fn decode(bytes: &[u8], forced: Option<CsvEncoding>) -> Result<Decoded, DecodeError> {
    let (detected, _) = detect(bytes);
    let encoding = forced.unwrap_or(detected);
    let (body, bom) = match encoding {
        CsvEncoding::Utf8 => match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            Some(rest) => (rest, true),
            None => (bytes, false),
        },
        CsvEncoding::Utf16le => match bytes.strip_prefix(&[0xFF, 0xFE]) {
            Some(rest) => (rest, true),
            None => (bytes, false),
        },
        CsvEncoding::Utf16be => match bytes.strip_prefix(&[0xFE, 0xFF]) {
            Some(rest) => (rest, true),
            None => (bytes, false),
        },
        CsvEncoding::Windows1252 | CsvEncoding::Latin1 => (bytes, false),
    };
    let text = match encoding {
        CsvEncoding::Utf8 => std::str::from_utf8(body)
            .map_err(|e| DecodeError::Invalid(encoding, e.valid_up_to() + usize::from(bom) * 3))?
            .to_owned(),
        CsvEncoding::Utf16le => utf16(body, true, encoding)?,
        CsvEncoding::Utf16be => utf16(body, false, encoding)?,
        CsvEncoding::Latin1 => body.iter().map(|b| char::from(*b)).collect(),
        CsvEncoding::Windows1252 => body
            .iter()
            .map(|b| match b {
                0x80..=0x9F => {
                    char::from_u32(WINDOWS_1252[usize::from(b - 0x80)]).unwrap_or('\u{FFFD}')
                }
                _ => char::from(*b),
            })
            .collect(),
    };
    if text.contains('\0') {
        return Err(DecodeError::Binary);
    }
    Ok(Decoded {
        text,
        encoding,
        bom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str, bom: bool) -> Vec<u8> {
        let mut out = if bom { vec![0xFF, 0xFE] } else { Vec::new() };
        for u in s.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    #[test]
    fn utf8_with_and_without_a_mark() {
        let d = decode("a,b\nü,ß\n".as_bytes(), None).expect("text");
        assert_eq!((d.encoding, d.bom), (CsvEncoding::Utf8, false));
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("a,b".as_bytes());
        let d = decode(&bytes, None).expect("text");
        assert_eq!((d.text.as_str(), d.bom), ("a,b", true));
    }

    #[test]
    fn utf16_with_and_without_a_mark() {
        let d = decode(&utf16le("Summary,Status\nÜber,Done\n", true), None).expect("text");
        assert_eq!(d.encoding, CsvEncoding::Utf16le);
        assert_eq!(d.text, "Summary,Status\nÜber,Done\n");
        let d = decode(&utf16le("Summary,Status\n", false), None).expect("text");
        assert_eq!((d.encoding, d.bom), (CsvEncoding::Utf16le, false));
        let be: Vec<u8> = "ab,cd".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(
            decode(&be, None).expect("text").encoding,
            CsvEncoding::Utf16be
        );
    }

    #[test]
    fn windows_1252_when_not_utf8() {
        // "Café – 5 €" in Windows-1252.
        let bytes = b"Caf\xe9 \x96 5 \x80";
        let d = decode(bytes, None).expect("text");
        assert_eq!(d.encoding, CsvEncoding::Windows1252);
        assert_eq!(d.text, "Café – 5 €");
        let d = decode(bytes, Some(CsvEncoding::Latin1)).expect("text");
        assert_eq!(d.text, "Café \u{96} 5 \u{80}");
    }

    #[test]
    fn bad_bytes_and_binary_are_refused() {
        assert_eq!(
            decode(b"ok\xff", Some(CsvEncoding::Utf8)),
            Err(DecodeError::Invalid(CsvEncoding::Utf8, 2))
        );
        assert_eq!(
            decode(&[0x41, 0x00, 0x00, 0xD8], Some(CsvEncoding::Utf16le)),
            Err(DecodeError::Invalid(CsvEncoding::Utf16le, 2))
        );
        assert_eq!(decode(b"a\0b", None), Err(DecodeError::Binary));
    }
}
