// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What people read about an error: the ID of its message in the message
//! files and the message's arguments. The English text of every message is
//! in the engine's message file; an error's `Display` stays English for
//! logs and tests.

use crate::custom_field::FieldError;
use crate::day::DayError;
use crate::duration::DurationError;
use crate::file::FileError;
use crate::id::IdError;
use crate::key::{KeyError, PROJECT_KEY_MAX, PROJECT_KEY_MIN};
use crate::kinds::KindError;
use crate::merge::EditError;
use crate::rank::{MAX_LEN, RankError};
use crate::text::TextError;
use crate::wiki::PageTitleError;
use crate::worklog::{REMINDER_MAX_DAYS_BEFORE, WorklogError};

/// A message for people: an ID and its arguments by name.
pub trait Message {
    /// The message's ID (`value-day-format`).
    fn message_id(&self) -> &'static str;
    /// The message's arguments, by name.
    fn message_args(&self) -> Vec<(&'static str, String)>;
}

fn arg(name: &'static str, value: impl ToString) -> (&'static str, String) {
    (name, value.to_string())
}

/// A noun used as a selector in a message: words joined by hyphens.
fn selector(text: &str) -> String {
    text.replace(' ', "-")
}

impl Message for DayError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Format(_) => "value-day-format",
            Self::NoSuchDay(_) => "value-day-not-in-calendar",
            Self::OutOfRange => "value-day-out-of-range",
            Self::StartAfterDue { .. } => "value-day-start-after-due",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Format(text) | Self::NoSuchDay(text) => vec![arg("text", text)],
            Self::OutOfRange => vec![],
            Self::StartAfterDue { start, due } => vec![arg("start", start), arg("due", due)],
        }
    }
}

impl Message for DurationError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Empty => "value-duration-empty",
            Self::Malformed(_) => "value-duration-malformed",
            Self::MissingUnit(_) => "value-duration-missing-unit",
            Self::RepeatedUnit(_) => "value-duration-repeated-unit",
            Self::TooLong(_) => "value-duration-too-long",
            Self::Units { .. } => "value-duration-units",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Empty => vec![],
            Self::Malformed(text)
            | Self::MissingUnit(text)
            | Self::RepeatedUnit(text)
            | Self::TooLong(text) => vec![arg("text", text)],
            Self::Units {
                hours_per_day,
                days_per_week,
            } => vec![arg("hours", hours_per_day), arg("days", days_per_week)],
        }
    }
}

impl Message for FileError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::EmptyName => "value-file-name-empty",
            Self::BadBase64(_) => "value-file-base64",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::EmptyName => vec![],
            Self::BadBase64(at) => vec![arg("position", at)],
        }
    }
}

impl Message for IdError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "value-id-malformed",
            Self::WrongLength(_) => "value-id-length",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Malformed(text) => vec![arg("text", text)],
            Self::WrongLength(len) => vec![arg("length", len)],
        }
    }
}

impl Message for KeyError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Length { .. } => "value-key-length",
            Self::FirstNotLetter(_) => "value-key-first-letter",
            Self::Character { .. } => "value-key-character",
            Self::TaskKeyShape(_) => "value-task-key-shape",
            Self::Number(_) => "value-task-number",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Length { key, len } => vec![
                arg("key", key),
                arg("length", len),
                arg("min", PROJECT_KEY_MIN),
                arg("max", PROJECT_KEY_MAX),
            ],
            Self::FirstNotLetter(key) | Self::TaskKeyShape(key) | Self::Number(key) => {
                vec![arg("key", key)]
            }
            Self::Character { key, ch } => vec![arg("key", key), arg("char", ch)],
        }
    }
}

impl Message for KindError {
    fn message_id(&self) -> &'static str {
        "value-not-one-of"
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        vec![
            arg("value", self.value()),
            arg("what", selector(self.what())),
            arg("expected", self.expected()),
        ]
    }
}

impl Message for EditError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Empty { .. } => "value-edit-empty",
            Self::NotFound { .. } => "value-edit-not-found",
            Self::Ambiguous { .. } => "value-edit-ambiguous",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Empty { index } | Self::NotFound { index } => vec![arg("index", index)],
            Self::Ambiguous { index, count } => vec![arg("index", index), arg("count", count)],
        }
    }
}

impl Message for RankError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "value-rank-malformed",
            Self::NoRoom { .. } => "value-rank-no-room",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Malformed(text) => vec![arg("text", text), arg("max", MAX_LEN)],
            Self::NoRoom { lower, upper } => vec![arg("lower", lower), arg("upper", upper)],
        }
    }
}

impl Message for FieldError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::UnknownKind(_) => "value-field-kind",
            Self::NoQueryName(_) => "value-field-query-name",
            Self::NotInteger(_) => "value-field-integer",
            Self::NotNumber(_) => "value-field-number",
            Self::NotDate(_) => "value-field-date",
            Self::NotBoolean(_) => "value-field-boolean",
            Self::NotUrl(_) => "value-field-url",
            Self::NotAnOption { .. } => "value-field-option",
            Self::TooLong { .. } => "value-field-too-long",
            Self::Empty => "value-field-empty",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::UnknownKind(text)
            | Self::NoQueryName(text)
            | Self::NotInteger(text)
            | Self::NotNumber(text)
            | Self::NotDate(text)
            | Self::NotBoolean(text)
            | Self::NotUrl(text) => vec![arg("text", text)],
            Self::NotAnOption {
                value,
                field,
                choices,
            } => vec![
                arg("text", value),
                arg("field", field),
                arg("choices", choices),
            ],
            Self::TooLong { len, max } => vec![arg("length", len), arg("max", max)],
            Self::Empty => vec![],
        }
    }
}

impl Message for TextError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Empty(_) => "value-text-empty",
            Self::TooLong { unit, .. } if *unit == "bytes" => "value-text-too-long-bytes",
            Self::TooLong { .. } => "value-text-too-long",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Empty(what) => vec![arg("what", selector(what))],
            Self::TooLong { what, len, max, .. } => {
                vec![
                    arg("what", selector(what)),
                    arg("length", len),
                    arg("max", max),
                ]
            }
        }
    }
}

impl Message for PageTitleError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Text(e) => e.message_id(),
            Self::Reserved(_) => "value-page-title-reserved",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Text(e) => e.message_args(),
            Self::Reserved(c) => vec![arg("char", c)],
        }
    }
}

impl Message for WorklogError {
    fn message_id(&self) -> &'static str {
        match self {
            Self::Duration(e) => e.message_id(),
            Self::Day(e) => e.message_id(),
            Self::Zero => "value-log-zero",
            Self::OverADay(_) => "value-log-over-a-day",
            Self::NoteTooLong { .. } => "value-log-note-too-long",
            Self::TooEarly => "value-reminder-too-early",
        }
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Duration(e) => e.message_args(),
            Self::Day(e) => e.message_args(),
            Self::Zero => vec![],
            Self::OverADay(minutes) => vec![arg("minutes", minutes)],
            Self::NoteTooLong { len, max } => vec![arg("length", len), arg("max", max)],
            Self::TooEarly => vec![arg("max", REMINDER_MAX_DAYS_BEFORE)],
        }
    }
}

impl<T: Message + ?Sized> Message for &T {
    fn message_id(&self) -> &'static str {
        (**self).message_id()
    }
    fn message_args(&self) -> Vec<(&'static str, String)> {
        (**self).message_args()
    }
}
