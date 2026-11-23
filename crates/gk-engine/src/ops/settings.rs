// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `settings.get`, `settings.update`, and the settings rows themselves.
//!
//! A setting that was never set has no row and takes its default. History
//! records the stored values, so reverting a first change removes the row
//! again.

use gk_api::OpError;
use gk_api::ops::settings::{
    SettingKey, Settings, SettingsChange, SettingsGet, SettingsPatch, SettingsUpdate,
};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Map, Value};

use super::desc;
use crate::failure::Failure;
use crate::registry::Registry;
use crate::write::WriteCtx;

/// The kind of item settings changes are recorded under; the item's ID is
/// the workspace's.
pub(crate) const ENTITY: &str = "settings";

pub(super) fn register(r: &mut Registry) {
    r.query::<SettingsGet, _>(desc::<SettingsGet>(), |q, _input| {
        load(q.tx).map_err(Failure::from)
    });
    r.command::<SettingsUpdate, _>(desc::<SettingsUpdate>(), update);
}

fn update(w: &mut WriteCtx<'_>, patch: SettingsPatch) -> Result<SettingsChange, Failure> {
    let mut cleared = Vec::new();
    for key in &patch.clear {
        let name = key.as_str();
        let given = match key {
            SettingKey::TrashDays => patch.trash_days.is_some(),
            SettingKey::ReminderTime => patch.reminder_time.is_some(),
            SettingKey::HoursPerDay => patch.hours_per_day.is_some(),
            SettingKey::DaysPerWeek => patch.days_per_week.is_some(),
            SettingKey::WeekStart => patch.week_start.is_some(),
        };
        if given {
            return Err(
                OpError::invalid_at("clear", format!("{name} is both given and cleared.")).into(),
            );
        }
        if raw(w.tx(), name)?.is_some() {
            write_raw(w, name, None)?;
            cleared.push(name.to_owned());
        }
    }
    let mut wanted: Vec<(&str, Value)> = Vec::new();
    if let Some(days) = patch.trash_days {
        in_range("trash_days", days, 1, 3650)?;
        wanted.push(("trash_days", days.into()));
    }
    if let Some(time) = patch.reminder_time {
        if !is_clock_time(&time) {
            return Err(OpError::invalid_at(
                "reminder_time",
                format!("\"{time}\" is not a time of day; write it as HH:MM, for example 09:00."),
            )
            .into());
        }
        wanted.push(("reminder_time", time.into()));
    }
    if let Some(hours) = patch.hours_per_day {
        in_range("hours_per_day", hours, 1, 24)?;
        wanted.push(("hours_per_day", hours.into()));
    }
    if let Some(days) = patch.days_per_week {
        in_range("days_per_week", days, 1, 7)?;
        wanted.push(("days_per_week", days.into()));
    }
    if let Some(start) = patch.week_start {
        wanted.push(("week_start", to_value(start)?));
    }
    let mut change = set_all(w, &wanted)?;
    change.changed.extend(cleared);
    Ok(change)
}

/// Sets each `(key, value)` that differs from the current value, and
/// returns the settings afterwards.
pub(crate) fn set_all(
    w: &mut WriteCtx<'_>,
    wanted: &[(&str, Value)],
) -> Result<SettingsChange, Failure> {
    let current = to_value(load(w.tx())?)?;
    let mut changed = Vec::new();
    for (key, value) in wanted {
        if current.get(*key) != Some(value) {
            write_raw(w, key, Some(value.clone()))?;
            changed.push((*key).to_owned());
        }
    }
    Ok(SettingsChange {
        settings: load(w.tx())?,
        changed,
        group: w.group_if_changed(),
    })
}

fn in_range(field: &str, value: u32, min: u32, max: u32) -> Result<(), Failure> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(OpError::invalid_at(
            field,
            format!("{field} must be from {min} to {max}; got {value}."),
        )
        .with_detail("min", min)
        .with_detail("max", max)
        .into())
    }
}

fn is_clock_time(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit())
        && (b[0] - b'0') * 10 + (b[1] - b'0') < 24
        && (b[3] - b'0') * 10 + (b[4] - b'0') < 60
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, Failure> {
    serde_json::to_value(v).map_err(|e| Failure::Op(OpError::storage(e.to_string())))
}

/// The settings with defaults filled in. A stored value that doesn't fit
/// (written by a newer program, say) is ignored with a warning.
pub(crate) fn load(tx: &Transaction<'_>) -> Result<Settings, StoreError> {
    let defaults = serde_json::to_value(Settings::default())
        .map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let Value::Object(mut merged) = defaults else {
        return Err(StoreError::Corrupt("settings are not an object".into()));
    };
    let mut stmt = tx.prepare_cached("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (key, text) = row?;
        if !merged.contains_key(&key) {
            continue;
        }
        let fits = serde_json::from_str::<Value>(&text).ok().filter(|value| {
            let mut candidate: Map<String, Value> = merged.clone();
            candidate.insert(key.clone(), value.clone());
            serde_json::from_value::<Settings>(Value::Object(candidate)).is_ok()
        });
        match fits {
            Some(value) => {
                merged.insert(key, value);
            }
            None => tracing::warn!(key, "ignoring a stored setting this program can't read"),
        }
    }
    serde_json::from_value(Value::Object(merged)).map_err(|e| StoreError::Corrupt(e.to_string()))
}

/// The stored value of `key`; `None` when it has none.
pub(crate) fn raw(tx: &Transaction<'_>, key: &str) -> Result<Option<Value>, StoreError> {
    let text: Option<String> = tx
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

/// Stores `value` for `key` (`None` removes it, so the default applies) and
/// records the change.
pub(crate) fn write_raw(
    w: &mut WriteCtx<'_>,
    key: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    let before = raw(w.tx(), key)?;
    if before == value {
        return Ok(());
    }
    match &value {
        Some(v) => {
            w.tx().execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, v.to_string(), w.now()],
            )?;
        }
        None => {
            w.tx()
                .execute("DELETE FROM settings WHERE key = ?1", [key])?;
        }
    }
    let id = w.workspace_id();
    w.record_field(ENTITY, id, None, key, before, value);
    Ok(())
}
