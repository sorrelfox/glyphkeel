// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Workspace settings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::agent::AgentLimits;
use crate::ops::proposal::AgentApprovals;
use crate::profile::Profile;

/// The workspace's settings, with defaults filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Days an item stays in the trash before it is purged.
    pub trash_days: u32,
    /// When reminders for a due date fire, as `HH:MM`.
    pub reminder_time: String,
    /// Working hours in a day, for durations written in days (`2d`).
    pub hours_per_day: u32,
    /// Working days in a week, for durations written in weeks (`1w`).
    pub days_per_week: u32,
    /// The first day of the week in calendars and in `this-week`.
    pub week_start: WeekStart,
    /// The largest file that can be attached, in megabytes.
    pub file_max_mb: u32,
    /// The most an agent may do, whatever profile it asks for.
    pub agent_max_profile: Profile,
    /// Whether an agent whose client can ask the person may ask to make
    /// one change its profile doesn't allow, or one that deletes for good
    /// or deletes many items; the person allows or declines each.
    pub agent_confirmations: bool,
    /// The limits on every agent session. Only the person sets them, in the
    /// app.
    pub agent_limits: AgentLimits,
    /// Which changes of agents wait for the person's approval, per
    /// profile, and how long a proposal waits. Only the person sets it, in
    /// the app.
    pub agent_approvals: AgentApprovals,
    /// Hours a timer runs before it reminds that it is still running; 0
    /// never.
    pub timer_reminder_hours: u32,
    /// The folder that holds this workspace's backups, as an absolute path;
    /// left out: the default folder. Only the person sets it, in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_folder: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            trash_days: 30,
            reminder_time: "09:00".to_owned(),
            hours_per_day: 8,
            days_per_week: 5,
            week_start: WeekStart::Monday,
            file_max_mb: 100,
            agent_max_profile: Profile::Standard,
            agent_confirmations: true,
            agent_limits: AgentLimits::DEFAULT,
            agent_approvals: AgentApprovals::DEFAULT,
            timer_reminder_hours: 8,
            backup_folder: None,
        }
    }
}

/// The first day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WeekStart {
    /// Monday.
    Monday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsGetInput {}

/// The settings to change; fields left out keep their value. Settings that
/// control agents are changed in the app only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    /// Days in the trash before purging, 1 to 3650.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_days: Option<u32>,
    /// Reminder time, `HH:MM` (24-hour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder_time: Option<String>,
    /// Working hours in a day, 1 to 24.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hours_per_day: Option<u32>,
    /// Working days in a week, 1 to 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_per_week: Option<u32>,
    /// First day of the week.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub week_start: Option<WeekStart>,
    /// The largest file that can be attached, in megabytes, 1 to 1024.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_max_mb: Option<u32>,
    /// Hours a timer runs before it reminds that it is still running, 0
    /// (never) to 24.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_reminder_hours: Option<u32>,
    /// Settings to set back to their default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<SettingKey>,
}

/// A setting that can be set back to its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettingKey {
    /// Days in the trash before purging (default 30).
    TrashDays,
    /// Reminder time (default 09:00).
    ReminderTime,
    /// Working hours in a day (default 8).
    HoursPerDay,
    /// Working days in a week (default 5).
    DaysPerWeek,
    /// First day of the week (default Monday).
    WeekStart,
    /// The largest file that can be attached (default 100 MB).
    FileMaxMb,
    /// Hours before a running timer reminds (default 8).
    TimerReminderHours,
}

impl SettingKey {
    /// The setting's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrashDays => "trash_days",
            Self::ReminderTime => "reminder_time",
            Self::HoursPerDay => "hours_per_day",
            Self::DaysPerWeek => "days_per_week",
            Self::WeekStart => "week_start",
            Self::FileMaxMb => "file_max_mb",
            Self::TimerReminderHours => "timer_reminder_hours",
        }
    }
}

/// The settings after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsChange {
    /// All settings, after the change.
    pub settings: Settings,
    /// The settings whose value changed.
    pub changed: Vec<String>,
    /// The history group of the change, for revert and Undo; left out when
    /// nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// Read the workspace settings.
    SettingsGet,
    "settings.get",
    SettingsGetInput,
    Settings
);
op!(
    /// Change workspace settings that are not about agents.
    SettingsUpdate,
    "settings.update",
    SettingsPatch,
    SettingsChange
);

pub(crate) const SETTINGS_GET: OpDescriptor = OpDescriptor::of::<SettingsGet>(Meta {
    summary: "Read the workspace settings.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("settings get"),
    input_name: "SettingsGetInput",
    output_name: "Settings",
});

pub(crate) const SETTINGS_UPDATE: OpDescriptor = OpDescriptor::of::<SettingsUpdate>(Meta {
    summary: "Change workspace settings that are not about agents: trash retention, reminder time, working hours and days, first day of the week, the timer reminder.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("settings set"),
    input_name: "SettingsPatch",
    output_name: "SettingsChange",
});
