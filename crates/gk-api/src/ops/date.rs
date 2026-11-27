// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dates in words, read the way every date field reads them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};

/// A date typed in a field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DateParseInput {
    /// What was typed: `2026-12-31`, `today`, `fri`, `next mon`, `in 3d`,
    /// `+3d`, `dec 1`, `end of month`.
    pub text: String,
    /// The day the words count from (`YYYY-MM-DD`), for example today in
    /// the person's own time zone; default: today in the workspace's time
    /// zone. Weeks start on the workspace's first day of the week.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub today: Option<String>,
}

/// The day the words mean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParsedDate {
    /// The day, `YYYY-MM-DD`.
    pub date: String,
}

op!(
    /// Reads a date typed in words.
    DateParse,
    "date.parse",
    DateParseInput,
    ParsedDate
);

pub(crate) const DATE_PARSE: OpDescriptor = OpDescriptor::of::<DateParse>(Meta {
    summary: "Read a date typed in words (fri, next mon, in 3d, +3d, dec 1, end of month, 2026-12-31) as the day it means, the way every date field reads it. Words that are no date fail with message date-invalid.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It shows the app's date fields which day the words mean while they are typed; agents and scripts pass the same words to the date fields themselves, which read them the same way.",
    },
    cli: None,
    input_name: "DateParseInput",
    output_name: "ParsedDate",
});
