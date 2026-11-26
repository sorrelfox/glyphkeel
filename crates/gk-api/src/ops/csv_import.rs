// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Importing tasks from a CSV file, such as an export of Jira or Redmine,
//! into an existing project.
//!
//! The file's encoding (UTF-8 with or without a byte order mark, UTF-16,
//! Windows-1252 or Latin-1), its separator and its quote are detected, and
//! can be given instead. Each column is mapped to a field of the tasks:
//! a built-in one, a custom field (`field.<name>`), or nothing. A mapping
//! can be saved as a preset of the workspace; presets for the CSV exports
//! of Jira and Redmine are built in.
//!
//! A dry run reports what each row would become, with its problems; the
//! tasks are one change, so one Undo moves all of them to the trash. Rows may
//! name their parent and linked tasks by the key or ID they had in the
//! other tool, also when those rows come later in the file.
//!
//! Each task is held to the rules of its status as `task.create` holds it:
//! a row whose task would lack a field its status requires is an error.
//! Repeating tasks are not read from the file; each row becomes one task
//! that doesn't repeat.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Priority;

/// A text encoding of a CSV file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvEncoding {
    /// UTF-8, with or without a byte order mark.
    Utf8,
    /// UTF-16, little-endian.
    Utf16le,
    /// UTF-16, big-endian.
    Utf16be,
    /// Windows-1252, which Excel and Redmine write on Western systems.
    Windows1252,
    /// ISO-8859-1.
    Latin1,
}

/// The order of day, month and year in dates such as `03/04/2025`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvDateOrder {
    /// Day, month, year: `31/12/2025`, `31.12.2025`.
    Dmy,
    /// Month, day, year: `12/31/2025`.
    Mdy,
    /// Year, month, day: `2025/12/31`.
    Ymd,
}

/// How to read the file; what is left out is detected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvFormat {
    /// The text encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<CsvEncoding>,
    /// The separator between fields: one character, such as `,`, `;`, a
    /// tab or `|`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// The character around quoted fields: `"` or `'`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// The order of day, month and year in numeric dates; detected from the
    /// dates themselves when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_order: Option<CsvDateOrder>,
    /// Whether numbers use a comma before their fraction (`2,5`); detected
    /// from the separator when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal_comma: Option<bool>,
}

/// The unit of a column of durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvUnit {
    /// Seconds, as Jira exports estimates and time spent.
    Seconds,
    /// Minutes.
    Minutes,
    /// Hours, with a fraction, as Redmine exports them.
    Hours,
    /// Days of the workspace's working day.
    Days,
    /// Written durations such as `1h 30m` or `1:30`; plain numbers are
    /// hours.
    Text,
}

/// Where one column goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvColumnMapping {
    /// The column's name in the header row, ignoring case. Every column with this
    /// name goes to the same place; their values are taken together (Jira
    /// writes one `Labels` column per label).
    pub column: String,
    /// The target: `title`, `description`, `status`, `type`, `priority`,
    /// `labels`, `fix_version` (an open version of the project, by name),
    /// `start_date`, `due_date`, `estimate`, `spent` (time
    /// logged on the import day), `comment`, `key` (the row's key or ID in
    /// the other tool, for references), `parent` (a key or ID, of a row of
    /// the file or a task here), `blocks`, `blocked_by`, `relates`,
    /// `duplicates`, `duplicated_by`, `clones`, `cloned_by`, `relations`
    /// (Redmine's "Blocks #12, Related to #13"), `field.<name>` for a custom
    /// field, or `ignore`.
    pub to: String,
    /// For `estimate` and `spent`: the unit of the numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<CsvUnit>,
}

/// How values of the file become values here. Keys are the file's values;
/// matching ignores case.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvValueMaps {
    /// Status names: the file's to this project's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub statuses: BTreeMap<String, String>,
    /// Type names: the file's (Jira's issue types, Redmine's trackers) to
    /// this project's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, String>,
    /// Priority names to priorities; the usual names of Jira and Redmine
    /// are known without this.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub priorities: BTreeMap<String, Priority>,
}

/// What to import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvImportInput {
    /// The project the tasks go to (key or ID).
    pub project: String,
    /// The CSV file. For agents, a plain name is a file in the workspace's
    /// `exports` folder; other paths must be inside a folder agents may
    /// use.
    pub path: String,
    /// The preset to start from: `jira`, `redmine`, or the name of a saved
    /// one. Left out: detected from the header row, else columns are
    /// matched by their names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// How to read the file, where detection is not wanted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<CsvFormat>,
    /// Where columns go, over the preset's choice for the same columns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<CsvColumnMapping>,
    /// How values become values here, over the preset's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<CsvValueMaps>,
    /// Create the statuses, types and labels the file names that the
    /// project lacks (the default); otherwise such a value is a row's
    /// error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_missing: Option<bool>,
    /// Import the other rows when some have errors, leaving those out.
    /// Without it, an error in any row stops the whole import.
    #[serde(default)]
    pub skip_invalid: bool,
    /// List only the rows with problems, not every row.
    #[serde(default)]
    pub only_problems: bool,
    /// Only check the file and report what each row would become.
    #[serde(default)]
    pub dry_run: bool,
}

/// What was detected about the file, or given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvDetected {
    /// Its encoding.
    pub encoding: CsvEncoding,
    /// Whether it began with a byte order mark.
    pub bom: bool,
    /// The separator.
    pub separator: String,
    /// The quote.
    pub quote: String,
    /// The order of numeric dates, when the file has any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_order: Option<CsvDateOrder>,
    /// Whether numbers have a decimal comma.
    pub decimal_comma: bool,
}

/// One column of the file and where it goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvColumn {
    /// Its place, from 0.
    pub index: u32,
    /// Its name in the header row.
    pub name: String,
    /// Its target; left out when it is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The unit of its durations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<CsvUnit>,
    /// Its first few values that are not empty, shortened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<String>,
}

/// How bad a row's problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvSeverity {
    /// The row can't be imported.
    Error,
    /// The row is imported without this value.
    Warning,
}

/// A problem with one row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvProblem {
    /// How bad it is.
    pub severity: CsvSeverity,
    /// The column, when one is to blame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    /// A stable code, such as `bad_date`.
    pub code: String,
    /// What is wrong, for people.
    pub message: String,
}

/// What became of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CsvOutcome {
    /// A task was (or, in a dry run, would be) created.
    Created,
    /// The row was left out because of its errors.
    Skipped,
}

/// One row of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvRow {
    /// The line of the file it starts on, from 1 (the header is line 1).
    pub line: u64,
    /// What became of it.
    pub outcome: CsvOutcome,
    /// The new task's key; in a dry run, the key it would get.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Its key or ID in the other tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_key: Option<String>,
    /// Its title.
    pub title: String,
    /// Its problems.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<CsvProblem>,
}

/// How many of each.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvCounts {
    /// Rows of data, the header left out.
    pub rows: u64,
    /// Tasks created, or that would be.
    pub created: u64,
    /// Rows left out.
    pub skipped: u64,
    /// Rows with warnings.
    pub warnings: u64,
    /// Tasks placed under a parent.
    pub parents: u64,
    /// Links between tasks.
    pub links: u64,
    /// Time entries.
    pub time_entries: u64,
    /// Comments.
    pub comments: u64,
}

/// The result of a CSV import, or of checking one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvImportResult {
    /// Whether this only checked the file.
    pub dry_run: bool,
    /// The project's key.
    pub project: String,
    /// The preset used, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// How the file was read.
    pub format: CsvDetected,
    /// Its columns, in order, with where each goes.
    pub columns: Vec<CsvColumn>,
    /// How many of each.
    pub counts: CsvCounts,
    /// Statuses created (or that would be).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_statuses: Vec<String>,
    /// Types created.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_types: Vec<String>,
    /// Labels created.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_labels: Vec<String>,
    /// The rows, in the file's order; only those with problems when asked.
    pub rows: Vec<CsvRow>,
    /// Whether the import would stop because of rows with errors (dry run
    /// without `skip_invalid`).
    pub blocked: bool,
    /// The history group of the tasks and what they hold, for revert and
    /// Undo (which moves the tasks to the trash); left out for a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The history group that created the statuses, types and labels,
    /// apart from `group`: the tasks in the trash still use them. Revert it
    /// after `group` once the tasks are deleted for good.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_group: Option<String>,
}

/// A saved or built-in mapping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPreset {
    /// Its name: `jira` and `redmine` are built in.
    pub name: String,
    /// Whether it is built in and can't be changed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub built_in: bool,
    /// How to read the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<CsvPresetFormat>,
    /// Where columns go.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<CsvPresetColumn>,
    /// How values become values here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<CsvPresetValues>,
}

/// How a preset reads files: [`CsvFormat`] as outputs carry it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetFormat {
    /// The text encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<CsvEncoding>,
    /// The separator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// The quote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// The order of numeric dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_order: Option<CsvDateOrder>,
    /// Whether numbers have a decimal comma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal_comma: Option<bool>,
}

/// A column of a preset: [`CsvColumnMapping`] as outputs carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetColumn {
    /// The column's name, ignoring case.
    pub column: String,
    /// Its target.
    pub to: String,
    /// The unit of its durations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<CsvUnit>,
}

/// A preset's value maps: [`CsvValueMaps`] as outputs carry them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetValues {
    /// Status names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub statuses: BTreeMap<String, String>,
    /// Type names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, String>,
    /// Priority names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub priorities: BTreeMap<String, Priority>,
}

impl From<CsvFormat> for CsvPresetFormat {
    fn from(f: CsvFormat) -> Self {
        Self {
            encoding: f.encoding,
            separator: f.separator,
            quote: f.quote,
            date_order: f.date_order,
            decimal_comma: f.decimal_comma,
        }
    }
}

impl From<CsvPresetFormat> for CsvFormat {
    fn from(f: CsvPresetFormat) -> Self {
        Self {
            encoding: f.encoding,
            separator: f.separator,
            quote: f.quote,
            date_order: f.date_order,
            decimal_comma: f.decimal_comma,
        }
    }
}

impl From<CsvColumnMapping> for CsvPresetColumn {
    fn from(c: CsvColumnMapping) -> Self {
        Self {
            column: c.column,
            to: c.to,
            unit: c.unit,
        }
    }
}

impl From<CsvPresetColumn> for CsvColumnMapping {
    fn from(c: CsvPresetColumn) -> Self {
        Self {
            column: c.column,
            to: c.to,
            unit: c.unit,
        }
    }
}

impl From<CsvValueMaps> for CsvPresetValues {
    fn from(v: CsvValueMaps) -> Self {
        Self {
            statuses: v.statuses,
            types: v.types,
            priorities: v.priorities,
        }
    }
}

impl From<CsvPresetValues> for CsvValueMaps {
    fn from(v: CsvPresetValues) -> Self {
        Self {
            statuses: v.statuses,
            types: v.types,
            priorities: v.priorities,
        }
    }
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetListInput {}

/// The presets: built-in ones first, then the workspace's by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetList {
    /// The presets.
    pub items: Vec<CsvPreset>,
}

/// A preset to save; one with the same name is replaced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetSaveInput {
    /// Its name, at most 60 characters; not `jira` or `redmine`.
    pub name: String,
    /// How to read the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<CsvFormat>,
    /// Where columns go.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<CsvColumnMapping>,
    /// How values become values here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<CsvValueMaps>,
}

/// A preset to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetDeleteInput {
    /// Its name.
    pub name: String,
}

/// The presets after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsvPresetChange {
    /// The presets now.
    pub items: Vec<CsvPreset>,
    /// The history group, when something changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// Import tasks from a CSV file.
    TransferImportCsv,
    "transfer.import_csv",
    CsvImportInput,
    CsvImportResult
);
op!(
    /// The CSV import presets.
    CsvPresetListOp,
    "csv_preset.list",
    CsvPresetListInput,
    CsvPresetList
);
op!(
    /// Save a CSV import preset.
    CsvPresetSave,
    "csv_preset.save",
    CsvPresetSaveInput,
    CsvPresetChange
);
op!(
    /// Delete a CSV import preset.
    CsvPresetDelete,
    "csv_preset.delete",
    CsvPresetDeleteInput,
    CsvPresetChange
);

pub(crate) const TRANSFER_IMPORT_CSV: OpDescriptor = OpDescriptor::of::<TransferImportCsv>(Meta {
    summary: "Import tasks from a CSV file into a project, with a column mapping or a preset (Jira and Redmine exports are built in); a dry run reports every row's outcome, and the import is one change that Undo takes back. Status rules hold for each task as for task.create; repeating tasks are not imported.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("import csv <path>"),
    input_name: "CsvImportInput",
    output_name: "CsvImportResult",
});

pub(crate) const CSV_PRESET_LIST: OpDescriptor = OpDescriptor::of::<CsvPresetListOp>(Meta {
    summary: "List the CSV import presets: the built-in ones for Jira and Redmine, and the workspace's own.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("csv-preset list"),
    input_name: "CsvPresetListInput",
    output_name: "CsvPresetList",
});

pub(crate) const CSV_PRESET_SAVE: OpDescriptor = OpDescriptor::of::<CsvPresetSave>(Meta {
    summary: "Save a CSV import mapping as a preset of the workspace, replacing one with the same name.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("csv-preset save <name>"),
    input_name: "CsvPresetSaveInput",
    output_name: "CsvPresetChange",
});

pub(crate) const CSV_PRESET_DELETE: OpDescriptor = OpDescriptor::of::<CsvPresetDelete>(Meta {
    summary: "Delete a CSV import preset of the workspace.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("csv-preset delete <name>"),
    input_name: "CsvPresetDeleteInput",
    output_name: "CsvPresetChange",
});
