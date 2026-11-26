// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! CSV text as records: the separator and quote detected or given, quoted
//! fields with doubled quotes and line breaks, and limits on what a file
//! may hold, so that a hostile file is refused before it is used.

/// The most fields one record holds.
pub(crate) const MAX_COLUMNS: usize = 1000;
/// The longest field, in bytes: the longest description.
pub(crate) const MAX_FIELD: usize = gk_core::text::MARKDOWN_MAX_BYTES;
/// The longest column name, in characters.
pub(crate) const MAX_HEADER: usize = 200;

/// The separators tried, in order of preference.
const SEPARATORS: [char; 4] = [',', ';', '\t', '|'];
/// How many records detection looks at.
const SAMPLE_RECORDS: usize = 30;

/// One record and the line it starts on (from 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) line: u64,
    pub(crate) fields: Vec<String>,
}

/// Why text is not CSV this program reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// A quoted field that never ends, starting on this line.
    Unterminated(u64),
    /// A field longer than [`MAX_FIELD`], on this line.
    FieldTooLong(u64),
    /// More fields than [`MAX_COLUMNS`], on this line.
    TooManyColumns(u64),
    /// More records than allowed.
    TooManyRows(usize),
    /// No header row.
    Empty,
}

/// Splits `text` into records, at most `max_records` (the header
/// included). Empty lines are left out.
pub(crate) fn records(
    text: &str,
    separator: char,
    quote: char,
    max_records: usize,
) -> Result<Vec<Record>, ParseError> {
    let mut out = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut line: u64 = 1;
    let mut start_line: u64 = 1;
    let mut quoted = false;
    // The field began with a quote (so a quote inside it closes it).
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let push_field = |fields: &mut Vec<String>, field: &mut String, at: u64| {
        if fields.len() >= MAX_COLUMNS {
            return Err(ParseError::TooManyColumns(at));
        }
        fields.push(std::mem::take(field));
        Ok(())
    };
    let mut started = false;
    while let Some(c) = chars.next() {
        if !started {
            start_line = line;
            started = true;
        }
        if in_quotes {
            if c == quote {
                if chars.peek() == Some(&quote) {
                    chars.next();
                    field.push(quote);
                } else {
                    in_quotes = false;
                }
            } else {
                if c == '\n' || (c == '\r' && chars.peek() != Some(&'\n')) {
                    line += 1;
                }
                field.push(c);
            }
        } else if c == quote && field.is_empty() && !quoted {
            in_quotes = true;
            quoted = true;
        } else if c == separator {
            push_field(&mut fields, &mut field, start_line)?;
            quoted = false;
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            push_field(&mut fields, &mut field, start_line)?;
            quoted = false;
            if !(fields.len() == 1 && fields[0].is_empty()) {
                if out.len() >= max_records {
                    return Err(ParseError::TooManyRows(max_records.saturating_sub(1)));
                }
                out.push(Record {
                    line: start_line,
                    fields: std::mem::take(&mut fields),
                });
            }
            fields.clear();
            line += 1;
            started = false;
            continue;
        } else {
            field.push(c);
        }
        if field.len() > MAX_FIELD {
            return Err(ParseError::FieldTooLong(start_line));
        }
    }
    if in_quotes {
        return Err(ParseError::Unterminated(start_line));
    }
    if started {
        push_field(&mut fields, &mut field, start_line)?;
        if !(fields.len() == 1 && fields[0].is_empty()) {
            if out.len() >= max_records {
                return Err(ParseError::TooManyRows(max_records.saturating_sub(1)));
            }
            out.push(Record {
                line: start_line,
                fields,
            });
        }
    }
    if out.is_empty() {
        return Err(ParseError::Empty);
    }
    Ok(out)
}

/// The separator of `text`: the one that splits the first records into
/// the same number of fields, more than one, most often.
pub(crate) fn detect_separator(text: &str, quote: char) -> char {
    let mut best = (',', 0usize, 0usize);
    for sep in SEPARATORS {
        let sample = sample_records(text, sep, quote);
        let Some(first) = sample.first() else {
            continue;
        };
        let width = first.fields.len();
        if width < 2 {
            continue;
        }
        let agreeing = sample.iter().filter(|r| r.fields.len() == width).count();
        if agreeing > best.1 || (agreeing == best.1 && width > best.2) {
            best = (sep, agreeing, width);
        }
    }
    best.0
}

/// The quote of `text`: `"`, unless only `'` stands next to separators.
pub(crate) fn detect_quote(text: &str) -> char {
    let head: String = text.chars().take(64 * 1024).collect();
    let around = |q: char| {
        SEPARATORS
            .iter()
            .map(|s| head.matches(&format!("{s}{q}")).count())
            .sum::<usize>()
            + usize::from(head.starts_with(q))
    };
    if around('"') == 0 && around('\'') > 0 {
        '\''
    } else {
        '"'
    }
}

/// The first records of `text`, for detection; a broken tail is fine.
fn sample_records(text: &str, separator: char, quote: char) -> Vec<Record> {
    let mut end = 0;
    let mut lines = 0;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            lines += 1;
            end = i + 1;
            if lines >= SAMPLE_RECORDS {
                break;
            }
        }
    }
    let head = if lines < SAMPLE_RECORDS || end == 0 {
        text
    } else {
        &text[..end]
    };
    match records(head, separator, quote, SAMPLE_RECORDS + 1) {
        Ok(r) => r,
        Err(_) => records(head.lines().next().unwrap_or_default(), separator, quote, 2)
            .unwrap_or_default(),
    }
}

/// Whether `c` is a control character that no field may hold: C0 and C1
/// controls other than tab and line breaks, and DEL.
pub(crate) fn is_forbidden_control(c: char) -> bool {
    (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{FEFF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(text: &str, sep: char) -> Vec<Vec<String>> {
        records(text, sep, '"', 100)
            .expect("csv")
            .into_iter()
            .map(|r| r.fields)
            .collect()
    }

    #[test]
    fn quoted_fields_with_quotes_separators_and_line_breaks() {
        let text = "a,b,c\r\n\"x, y\",\"say \"\"hi\"\"\",\"two\nlines\"\n\nlast,,\n";
        assert_eq!(
            fields(text, ','),
            vec![
                vec!["a", "b", "c"],
                vec!["x, y", "say \"hi\"", "two\nlines"],
                vec!["last", "", ""],
            ]
        );
        let r = records(text, ',', '"', 100).expect("csv");
        assert_eq!(r.iter().map(|r| r.line).collect::<Vec<_>>(), [1, 2, 5]);
    }

    #[test]
    fn a_quote_inside_a_plain_field_is_kept() {
        assert_eq!(fields("a,5\" disk\n", ','), vec![vec!["a", "5\" disk"]]);
    }

    #[test]
    fn separators_and_quotes_are_detected() {
        assert_eq!(detect_separator("a;b;c\n1;2,5;3\n", '"'), ';');
        assert_eq!(detect_separator("a,b\n1,2\n", '"'), ',');
        assert_eq!(detect_separator("a\tb\tc\n1\t2\t3\n", '"'), '\t');
        assert_eq!(detect_separator("\"a;b\",c\n\"1;2\",3\n", '"'), ',');
        assert_eq!(detect_quote("'a b';c\n'x';y"), '\'');
        assert_eq!(detect_quote("\"a\",b"), '"');
    }

    #[test]
    fn hostile_input_is_refused() {
        assert_eq!(
            records("a,\"open\n", ',', '"', 10),
            Err(ParseError::Unterminated(1))
        );
        let long = format!("a\n{}\n", "x".repeat(MAX_FIELD + 1));
        assert_eq!(
            records(&long, ',', '"', 10),
            Err(ParseError::FieldTooLong(2))
        );
        let wide = ",".repeat(MAX_COLUMNS + 1);
        assert_eq!(
            records(&wide, ',', '"', 10),
            Err(ParseError::TooManyColumns(1))
        );
        assert_eq!(
            records("h\n1\n2\n3\n", ',', '"', 3),
            Err(ParseError::TooManyRows(2))
        );
        assert_eq!(records("\n\n", ',', '"', 3), Err(ParseError::Empty));
        assert!(is_forbidden_control('\u{7}'));
        assert!(is_forbidden_control('\u{85}'));
        assert!(!is_forbidden_control('\t'));
    }
}
