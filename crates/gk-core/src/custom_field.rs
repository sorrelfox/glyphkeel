// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields: the kinds of value a field holds, how a value written as
//! text is read for each kind, and the name a field has in queries.
//!
//! A value is stored as one of three plain values ([`Stored`]): text for
//! text, Markdown, dates (`YYYY-MM-DD`), choices and links; a whole number
//! for whole numbers and yes/no (1 or 0); a number with a fraction for
//! decimals. Dates as text sort and compare in calendar order. A
//! multi-select field keeps one stored value per chosen option.

use std::fmt;
use std::str::FromStr;

use crate::day::Day;
use crate::text::{clean_line, fold};

/// Longest field name, in characters.
pub const FIELD_NAME_MAX_CHARS: usize = 50;
/// Longest option of a select field, in characters.
pub const OPTION_MAX_CHARS: usize = 100;
/// Most options of one select field.
pub const OPTIONS_MAX: usize = 200;
/// Longest one-line text value, in characters.
pub const TEXT_MAX_CHARS: usize = 1000;
/// Largest Markdown value, in bytes of UTF-8.
pub const MARKDOWN_MAX_BYTES: usize = 64 * 1024;
/// Longest link, in characters.
pub const URL_MAX_CHARS: usize = 2048;

/// What a custom field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// One line of text.
    Text,
    /// Formatted text, several lines.
    Markdown,
    /// A whole number.
    Integer,
    /// A number with a fraction.
    Decimal,
    /// A day.
    Date,
    /// Yes or no.
    Boolean,
    /// One option of a list.
    Select,
    /// Any number of options of a list.
    MultiSelect,
    /// A link.
    Url,
}

impl FieldKind {
    /// Every kind, in the order a person is offered them.
    pub const ALL: [Self; 9] = [
        Self::Text,
        Self::Markdown,
        Self::Integer,
        Self::Decimal,
        Self::Date,
        Self::Boolean,
        Self::Select,
        Self::MultiSelect,
        Self::Url,
    ];

    /// The name stored in the database and used in outputs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Date => "date",
            Self::Boolean => "boolean",
            Self::Select => "select",
            Self::MultiSelect => "multi_select",
            Self::Url => "url",
        }
    }

    /// Whether the field's values are options of its list.
    #[must_use]
    pub const fn has_options(self) -> bool {
        matches!(self, Self::Select | Self::MultiSelect)
    }

    /// Whether a task can have several values of the field.
    #[must_use]
    pub const fn is_multiple(self) -> bool {
        matches!(self, Self::MultiSelect)
    }

    /// Whether values can be compared with `<` and `>` and given as ranges.
    #[must_use]
    pub const fn is_ordered(self) -> bool {
        matches!(self, Self::Integer | Self::Decimal | Self::Date)
    }
}

impl fmt::Display for FieldKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FieldKind {
    type Err = FieldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| FieldError::UnknownKind(s.to_owned()))
    }
}

/// One stored value.
#[derive(Debug, Clone, PartialEq)]
pub enum Stored {
    /// Text, a date or an option.
    Text(String),
    /// A whole number, or yes (1) and no (0).
    Int(i64),
    /// A decimal.
    Real(f64),
}

/// Why a field or a value was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    /// A kind this program doesn't know.
    #[error("{0:?} is not a kind of field")]
    UnknownKind(String),
    /// The name has nothing a query can name it by.
    #[error("the field name {0:?} needs a letter or a digit")]
    NoQueryName(String),
    /// Not a whole number.
    #[error("{0:?} is not a whole number")]
    NotInteger(String),
    /// Not a number.
    #[error("{0:?} is not a number")]
    NotNumber(String),
    /// Not `YYYY-MM-DD`.
    #[error("{0:?} is not a date (YYYY-MM-DD)")]
    NotDate(String),
    /// Not yes or no.
    #[error("{0:?} is not yes or no")]
    NotBoolean(String),
    /// Not a link with a scheme.
    #[error("{0:?} is not a link")]
    NotUrl(String),
    /// Not one of the field's options.
    #[error("{value:?} is not an option of {field}")]
    NotAnOption {
        /// The value given.
        value: String,
        /// The field's name.
        field: String,
        /// The options, joined by ", ".
        choices: String,
    },
    /// Over the limit.
    #[error("the value is {len} long; the limit is {max}")]
    TooLong {
        /// Its length.
        len: usize,
        /// The limit.
        max: usize,
    },
    /// Empty.
    #[error("the value is empty")]
    Empty,
}

/// The name a field has in queries after `cf.`: its name folded, with every
/// run of characters other than letters, digits and `_` written as one
/// `-`, without `-` at either end. `Story points` is `story-points`.
///
/// # Errors
/// When nothing is left: the name has no letter or digit.
pub fn query_name(name: &str) -> Result<String, FieldError> {
    let mut out = String::new();
    let mut gap = false;
    for c in fold(name).chars() {
        if c.is_alphanumeric() || c == '_' {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        Err(FieldError::NoQueryName(name.to_owned()))
    } else {
        Ok(out)
    }
}

/// Reads `text` as a value of a field of `kind`. Options are checked
/// against `options` (any case; the stored value is the option as listed),
/// with `field` naming the field in the error.
///
/// # Errors
/// When the text doesn't fit the kind.
pub fn parse_value(
    kind: FieldKind,
    text: &str,
    options: &[String],
    field: &str,
) -> Result<Stored, FieldError> {
    let line = clean_line(text);
    let checked_len = |s: &str, max: usize| {
        let len = s.chars().count();
        if len > max {
            Err(FieldError::TooLong { len, max })
        } else {
            Ok(())
        }
    };
    match kind {
        FieldKind::Markdown => {
            if text.trim().is_empty() {
                return Err(FieldError::Empty);
            }
            if text.len() > MARKDOWN_MAX_BYTES {
                return Err(FieldError::TooLong {
                    len: text.len(),
                    max: MARKDOWN_MAX_BYTES,
                });
            }
            Ok(Stored::Text(text.to_owned()))
        }
        _ if line.is_empty() => Err(FieldError::Empty),
        FieldKind::Text => {
            checked_len(&line, TEXT_MAX_CHARS)?;
            Ok(Stored::Text(line))
        }
        FieldKind::Integer => line
            .parse::<i64>()
            .map(Stored::Int)
            .map_err(|_| FieldError::NotInteger(line)),
        FieldKind::Decimal => match line.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(Stored::Real(n)),
            _ => Err(FieldError::NotNumber(line)),
        },
        FieldKind::Date => line
            .parse::<Day>()
            .map(|d| Stored::Text(d.to_string()))
            .map_err(|_| FieldError::NotDate(line)),
        FieldKind::Boolean => parse_bool(&line)
            .map(|b| Stored::Int(i64::from(b)))
            .ok_or(FieldError::NotBoolean(line)),
        FieldKind::Url => {
            checked_len(&line, URL_MAX_CHARS)?;
            if is_url(&line) {
                Ok(Stored::Text(line))
            } else {
                Err(FieldError::NotUrl(line))
            }
        }
        FieldKind::Select | FieldKind::MultiSelect => {
            let wanted = fold(&line);
            options
                .iter()
                .find(|o| fold(o) == wanted)
                .map(|o| Stored::Text(o.clone()))
                .ok_or_else(|| FieldError::NotAnOption {
                    value: line,
                    field: field.to_owned(),
                    choices: options.join(", "),
                })
        }
    }
}

/// `yes`, `true`, `1` and `on` are yes; `no`, `false`, `0` and `off` are no.
#[must_use]
pub fn parse_bool(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" | "on" => Some(true),
        "no" | "false" | "0" | "off" => Some(false),
        _ => None,
    }
}

/// A link: a scheme (`https:`, `mailto:`) followed by something, without
/// white space.
#[must_use]
pub fn is_url(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !rest.is_empty()
        && !text.chars().any(char::is_whitespace)
}

/// Checks an option name: one line, 1 to [`OPTION_MAX_CHARS`] characters.
///
/// # Errors
/// When it is empty or too long.
pub fn check_option(text: &str) -> Result<String, FieldError> {
    let line = clean_line(text);
    if line.is_empty() {
        return Err(FieldError::Empty);
    }
    let len = line.chars().count();
    if len > OPTION_MAX_CHARS {
        return Err(FieldError::TooLong {
            len,
            max: OPTION_MAX_CHARS,
        });
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Vec<String> {
        vec!["Low".to_owned(), "High".to_owned()]
    }

    #[test]
    fn kinds_read_back_by_name() {
        for k in FieldKind::ALL {
            assert_eq!(k.as_str().parse::<FieldKind>(), Ok(k));
        }
        assert!("number".parse::<FieldKind>().is_err());
    }

    #[test]
    fn query_names_fold_and_join_words() {
        assert_eq!(query_name("Story points").unwrap(), "story-points");
        assert_eq!(query_name("  Ticket #/ID ").unwrap(), "ticket-id");
        assert_eq!(query_name("Приоритет").unwrap(), "приоритет");
        assert_eq!(query_name("snake_case").unwrap(), "snake_case");
        assert!(query_name("--").is_err());
    }

    #[test]
    fn values_follow_their_kind() {
        let p = |k, t| parse_value(k, t, &opts(), "Level");
        assert_eq!(p(FieldKind::Integer, " 42 "), Ok(Stored::Int(42)));
        assert!(p(FieldKind::Integer, "4.2").is_err());
        assert_eq!(p(FieldKind::Decimal, "4.25"), Ok(Stored::Real(4.25)));
        assert!(p(FieldKind::Decimal, "NaN").is_err());
        assert_eq!(
            p(FieldKind::Date, "2026-11-30"),
            Ok(Stored::Text("2026-11-30".to_owned()))
        );
        assert!(p(FieldKind::Date, "2026-02-30").is_err());
        assert_eq!(p(FieldKind::Boolean, "Yes"), Ok(Stored::Int(1)));
        assert_eq!(p(FieldKind::Boolean, "off"), Ok(Stored::Int(0)));
        assert!(p(FieldKind::Boolean, "maybe").is_err());
        assert_eq!(
            p(FieldKind::Select, "high"),
            Ok(Stored::Text("High".to_owned()))
        );
        assert!(matches!(
            p(FieldKind::MultiSelect, "medium"),
            Err(FieldError::NotAnOption { .. })
        ));
        assert_eq!(
            p(FieldKind::Url, "https://example.org/a"),
            Ok(Stored::Text("https://example.org/a".to_owned()))
        );
        assert!(p(FieldKind::Url, "example.org").is_err());
        assert_eq!(
            p(FieldKind::Text, "two\nlines"),
            Ok(Stored::Text("two lines".to_owned()))
        );
        assert_eq!(p(FieldKind::Text, "  "), Err(FieldError::Empty));
        assert_eq!(
            p(FieldKind::Markdown, "# Title\n\nText"),
            Ok(Stored::Text("# Title\n\nText".to_owned()))
        );
    }

    #[test]
    fn options_are_one_short_line() {
        assert_eq!(check_option(" A\tB ").unwrap(), "A B");
        assert!(check_option("").is_err());
        assert!(check_option(&"x".repeat(OPTION_MAX_CHARS + 1)).is_err());
    }
}
