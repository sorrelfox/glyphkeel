// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `date.parse`: a date typed in words, read exactly as the date fields of
//! the other operations read it, so that a field can show the day while the
//! person types.

use gk_api::ops::date::{DateParse, DateParseInput, ParsedDate};
use gk_core::Day;

use super::desc;
use super::task::{invalid, parse_date};
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};

pub(super) fn register(r: &mut Registry) {
    r.query::<DateParse, _>(desc::<DateParse>(), parse);
}

fn parse(q: &QueryCtx<'_>, input: DateParseInput) -> Result<ParsedDate, Failure> {
    let zone = &q.env.zone;
    let now = match input.today.as_deref() {
        None => q.now,
        // Noon of that day: inside it whatever the clocks do at midnight.
        Some(text) => {
            let day: Day = text.trim().parse().map_err(|e| invalid("today", e))?;
            zone.moment(day, 12 * 60)
        }
    };
    let day = parse_date(q.tx, zone, now, "text", &input.text)?;
    Ok(ParsedDate {
        date: day.to_string(),
    })
}
