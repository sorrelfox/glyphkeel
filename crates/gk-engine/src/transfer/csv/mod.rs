// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Importing tasks from a CSV file into an existing project.
//!
//! The file is read whole and checked before anything is written: its size,
//! its encoding, its records and every value the mapping uses. Each row is
//! planned outside the transaction; the command then creates the missing
//! statuses, types and labels, and each row's task through the same code
//! as `task.create`. Parents and links come second, so that they may name
//! rows further down the file. Everything is one command: one history
//! group, so Undo takes the whole import back.
//!
//! A dry run runs the same command and takes it back (a savepoint), so it
//! reports exactly what each row would become, with the keys it would
//! get.

mod decode;
mod parse;
mod presets;
mod target;
mod values;

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::Arc;

use gk_api::ops::comment::CommentAddInput;
use gk_api::ops::common::Priority;
use gk_api::ops::common::{Category, Hue};
use gk_api::ops::csv_import::{
    CsvColumn, CsvColumnMapping, CsvCounts, CsvDateOrder, CsvDetected, CsvEncoding, CsvFormat,
    CsvImportInput, CsvImportResult, CsvOutcome, CsvPreset, CsvProblem, CsvRow, CsvSeverity,
    CsvUnit, CsvValueMaps, TransferImportCsv,
};
use gk_api::ops::project::{LabelChange, StatusChange, TypeChange};
use gk_api::ops::task::{RelationInput, RelationKind, TaskCreateInput, TaskUpdateInput};
use gk_api::ops::time::TimeLogInput;
use gk_api::{ErrorCode, OpError, Operation, descriptor};
use gk_core::{Day, DurationUnits, FieldKind, fold};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::target::Target;
use super::resolve;
use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::ops::lookup::{self, List};
use crate::ops::{comment, custom_field, project, relation, rules, task, time, version};
use crate::registry::{CommandFn, Registry};
use crate::write::WriteCtx;

/// The largest file read.
const MAX_BYTES: u64 = 64 << 20;
/// The most rows one import takes from the person.
const MAX_ROWS: usize = 20_000;
/// The most from an agent.
const MAX_ROWS_AGENT: usize = 2_000;
/// Values shown per column.
const SAMPLES: usize = 3;
/// The longest value shown, in characters.
const SAMPLE_CHARS: usize = 60;
/// The longest stretch of time one entry holds; longer time spent becomes
/// several entries.
const ENTRY_MINUTES: i64 = gk_core::worklog::ENTRY_MAX_MINUTES;
/// The most time entries one row makes.
const MAX_ENTRIES: i64 = 100;

pub(crate) fn register(r: &mut Registry) {
    presets::register(r);
    r.engine_op::<TransferImportCsv, _>(
        crate::ops::desc::<TransferImportCsv>(),
        |engine, ctx, input| engine.import_csv(ctx, &input),
    );
}

/// One character of a format field, or why not.
fn one_char(field: &str, text: &str) -> Result<char, Failure> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !matches!(c, '\n' | '\r') && !parse::is_forbidden_control(c) => Ok(c),
        _ => Err(messages::invalid(
            field,
            "csv-format-one-character",
            &[("text", text.to_owned())],
        )),
    }
}

/// Refuses a format whose separator or quote is not one usable character.
pub(crate) fn check_format(f: &CsvFormat) -> Result<(), Failure> {
    let sep = f
        .separator
        .as_deref()
        .map(|s| one_char("format.separator", s))
        .transpose()?;
    let quote = f
        .quote
        .as_deref()
        .map(|s| one_char("format.quote", s))
        .transpose()?;
    if sep.is_some() && sep == quote {
        return Err(messages::invalid(
            "format.quote",
            "csv-format-same-character",
            &[],
        ));
    }
    Ok(())
}

/// Reads at most [`MAX_BYTES`] of the file.
fn read_file(path: &std::path::Path) -> Result<Vec<u8>, OpError> {
    let unreadable = |e: std::io::Error| {
        messages::invalid_at("path", "csv-unreadable", &[("detail", e.to_string())])
    };
    let meta = std::fs::metadata(path).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(messages::invalid_at("path", "csv-not-a-file", &[]));
    }
    let too_large = || {
        messages::invalid_at(
            "path",
            "csv-too-large",
            &[("max", (MAX_BYTES >> 20).to_string())],
        )
    };
    if meta.len() > MAX_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(unreadable)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

fn encoding_name(e: CsvEncoding) -> &'static str {
    match e {
        CsvEncoding::Utf8 => "UTF-8",
        CsvEncoding::Utf16le => "UTF-16LE",
        CsvEncoding::Utf16be => "UTF-16BE",
        CsvEncoding::Windows1252 => "Windows-1252",
        CsvEncoding::Latin1 => "ISO-8859-1",
    }
}

fn decode_error(e: &decode::DecodeError) -> OpError {
    match e {
        decode::DecodeError::Invalid(enc, at) => messages::invalid_at(
            "path",
            "csv-bad-encoding",
            &[
                ("encoding", encoding_name(*enc).to_owned()),
                ("offset", at.to_string()),
            ],
        ),
        decode::DecodeError::Binary => messages::invalid_at("path", "csv-binary", &[]),
    }
}

fn parse_error(e: &parse::ParseError) -> OpError {
    let line = |id: &str, l: u64| messages::invalid_at("path", id, &[("line", l.to_string())]);
    match e {
        parse::ParseError::Unterminated(l) => line("csv-unterminated", *l),
        parse::ParseError::FieldTooLong(l) => messages::invalid_at(
            "path",
            "csv-field-too-long",
            &[
                ("line", l.to_string()),
                ("max", (parse::MAX_FIELD >> 10).to_string()),
            ],
        ),
        parse::ParseError::TooManyColumns(l) => messages::invalid_at(
            "path",
            "csv-too-many-columns",
            &[
                ("line", l.to_string()),
                ("max", parse::MAX_COLUMNS.to_string()),
            ],
        ),
        parse::ParseError::TooManyRows(max) => {
            messages::with("csv-too-many-rows", &[("max", max.to_string())], |m| {
                OpError::limit(m, *max as u64)
            })
        }
        parse::ParseError::Empty => messages::invalid_at("path", "csv-empty", &[]),
    }
}

/// A problem of a row, with its message.
fn problem(
    severity: CsvSeverity,
    column: Option<&str>,
    code: &str,
    id: &str,
    args: &[(&str, String)],
) -> CsvProblem {
    CsvProblem {
        severity,
        column: column.map(str::to_owned),
        code: code.to_owned(),
        message: messages::text(id, args),
    }
}

fn warning(column: &str, code: &str, id: &str, args: &[(&str, String)]) -> CsvProblem {
    problem(CsvSeverity::Warning, Some(column), code, id, args)
}

/// A problem from an error of the operation that refused the value.
fn refused(severity: CsvSeverity, column: Option<&str>, code: &str, e: &OpError) -> CsvProblem {
    CsvProblem {
        severity,
        column: column.map(str::to_owned),
        code: code.to_owned(),
        message: e.message.clone(),
    }
}

/// One column's place in the import.
#[derive(Debug, Clone)]
struct Mapped {
    index: usize,
    name: String,
    target: Target,
    unit: CsvUnit,
}

/// The value a link points to, and the column it came from.
#[derive(Debug, Clone)]
struct LinkRef {
    kind: RelationKind,
    other: String,
    column: String,
}

/// One row, ready for the command.
#[derive(Debug, Clone, Default)]
struct Planned {
    line: u64,
    title: String,
    description: Option<String>,
    status: Option<(String, String)>,
    task_type: Option<(String, String)>,
    priority: Option<Priority>,
    labels: Vec<(String, String)>,
    fix_version: Option<(String, String)>,
    start: Option<Day>,
    due: Option<Day>,
    estimate: Option<i64>,
    spent: Option<i64>,
    comments: Vec<String>,
    keys: Vec<String>,
    source_key: Option<String>,
    parent: Option<(String, String)>,
    links: Vec<LinkRef>,
    fields: BTreeMap<String, Value>,
    problems: Vec<CsvProblem>,
}

impl Planned {
    fn has_errors(&self) -> bool {
        self.problems
            .iter()
            .any(|p| p.severity == CsvSeverity::Error)
    }
}

/// What the import needs to know of the project.
struct Setup {
    key: String,
    fields: Vec<(String, FieldKind)>,
    preset: Option<CsvPreset>,
    units: DurationUnits,
}

/// The column mapping: the input's, over the preset's, over names.
fn mapping(
    headers: &[String],
    input: &CsvImportInput,
    setup: &Setup,
) -> Result<Vec<Mapped>, Failure> {
    let given: Vec<(usize, &CsvColumnMapping)> = input.columns.iter().enumerate().collect();
    for (i, c) in &given {
        if !headers
            .iter()
            .any(|h| fold(h.trim()) == fold(c.column.trim()))
        {
            return Err(messages::invalid(
                &format!("columns[{i}].column"),
                "csv-unknown-column",
                &[("column", c.column.clone())],
            ));
        }
    }
    let field_named = |name: &str| {
        setup
            .fields
            .iter()
            .find(|(n, _)| fold(n) == fold(name))
            .map(|(n, _)| n.clone())
    };
    let mut out = Vec::with_capacity(headers.len());
    for (index, name) in headers.iter().enumerate() {
        let wanted = fold(name.trim());
        let chosen = given
            .iter()
            .find(|(_, c)| fold(c.column.trim()) == wanted)
            .map(|(i, c)| (format!("columns[{i}].to"), (*c).clone()))
            .or_else(|| {
                setup.preset.as_ref().and_then(|p| {
                    p.columns
                        .iter()
                        .find(|c| fold(c.column.trim()) == wanted)
                        .map(|c| ("preset".to_owned(), CsvColumnMapping::from(c.clone())))
                })
            });
        let (target, unit) = match chosen {
            Some((path, c)) => {
                let target = Target::parse(&c.to).map_err(|()| {
                    messages::invalid(&path, "csv-unknown-target", &[("target", c.to.clone())])
                })?;
                let target = match target {
                    Target::Field(f) => match field_named(&f) {
                        Some(n) => Target::Field(n),
                        None => {
                            return Err(messages::invalid(
                                &path,
                                "csv-unknown-field",
                                &[("field", f), ("project", setup.key.clone())],
                            ));
                        }
                    },
                    t => t,
                };
                (target, c.unit.unwrap_or(CsvUnit::Text))
            }
            None if setup.preset.is_none() => {
                let t = presets::by_name(name)
                    .and_then(|t| Target::parse(t).ok())
                    .or_else(|| field_named(name.trim()).map(Target::Field))
                    .unwrap_or(Target::Ignore);
                (t, CsvUnit::Text)
            }
            None => (Target::Ignore, CsvUnit::Text),
        };
        out.push(Mapped {
            index,
            name: name.clone(),
            target,
            unit,
        });
    }
    Ok(out)
}

/// The name a target is shown by.
fn target_name(t: &Target) -> Option<String> {
    Some(
        match t {
            Target::Title => "title",
            Target::Description => "description",
            Target::Status => "status",
            Target::Type => "type",
            Target::Priority => "priority",
            Target::Labels => "labels",
            Target::FixVersion => "fix_version",
            Target::StartDate => "start_date",
            Target::DueDate => "due_date",
            Target::Estimate => "estimate",
            Target::Spent => "spent",
            Target::Comment => "comment",
            Target::Key => "key",
            Target::Parent => "parent",
            Target::Link(RelationKind::Blocks) => "blocks",
            Target::Link(RelationKind::BlockedBy) => "blocked_by",
            Target::Link(RelationKind::Relates) => "relates",
            Target::Link(RelationKind::Duplicates) => "duplicates",
            Target::Link(RelationKind::DuplicatedBy) => "duplicated_by",
            Target::Link(RelationKind::Clones) => "clones",
            Target::Link(RelationKind::ClonedBy) => "cloned_by",
            Target::Link(_) => return None,
            Target::Relations => "relations",
            Target::Field(name) => return Some(format!("field.{name}")),
            Target::Ignore => return None,
        }
        .to_owned(),
    )
}

/// What reading the values needs.
struct Reading<'a> {
    order: CsvDateOrder,
    comma: bool,
    units: DurationUnits,
    values: &'a CsvValueMaps,
    fields: &'a [(String, FieldKind)],
    jira: bool,
}

fn looked_up(map: &BTreeMap<String, String>, value: &str) -> String {
    map.iter()
        .find(|(k, _)| fold(k.trim()) == fold(value))
        .map_or_else(|| value.to_owned(), |(_, v)| v.trim().to_owned())
}

/// Plans one record.
fn plan_row(record: &parse::Record, columns: &[Mapped], r: &Reading<'_>) -> Planned {
    let mut p = Planned {
        line: record.line,
        ..Planned::default()
    };
    let width = columns.len();
    if record.fields.len() > width && record.fields[width..].iter().any(|f| !f.trim().is_empty()) {
        p.problems.push(problem(
            CsvSeverity::Warning,
            None,
            "extra_fields",
            "csv-row-extra-fields",
            &[("count", (record.fields.len() - width).to_string())],
        ));
    }
    let mut multi_field: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for col in columns {
        if col.target == Target::Ignore {
            continue;
        }
        let raw = record.fields.get(col.index).map_or("", String::as_str);
        if raw.trim().is_empty() {
            continue;
        }
        let column = col.name.as_str();
        if raw.chars().any(parse::is_forbidden_control) {
            p.problems.push(problem(
                CsvSeverity::Error,
                Some(column),
                "control_character",
                "csv-row-control",
                &[("column", col.name.clone())],
            ));
            continue;
        }
        let value = raw.trim();
        let first = |slot: &mut Option<(String, String)>| {
            if slot.is_none() {
                *slot = Some((value.to_owned(), col.name.clone()));
            }
        };
        match &col.target {
            Target::Title => {
                if p.title.is_empty() {
                    p.title = value.split_whitespace().collect::<Vec<_>>().join(" ");
                }
            }
            Target::Description => {
                if p.description.is_none() {
                    p.description = Some(raw.trim_end().replace("\r\n", "\n"));
                }
            }
            Target::Status => {
                if p.status.is_none() {
                    p.status = Some((looked_up(&r.values.statuses, value), col.name.clone()));
                }
            }
            Target::Type => {
                if p.task_type.is_none() {
                    p.task_type = Some((looked_up(&r.values.types, value), col.name.clone()));
                }
            }
            Target::Priority => {
                if p.priority.is_some() {
                    continue;
                }
                let given = r
                    .values
                    .priorities
                    .iter()
                    .find(|(k, _)| fold(k.trim()) == fold(value))
                    .map(|(_, v)| *v);
                match given.or_else(|| values::known_priority(value)) {
                    Some(pr) => p.priority = Some(pr),
                    None => p.problems.push(warning(
                        column,
                        "unknown_priority",
                        "csv-row-unknown-priority",
                        &[("value", value.to_owned())],
                    )),
                }
            }
            Target::Labels => {
                for l in value
                    .split([',', ';'])
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                {
                    if !p.labels.iter().any(|(x, _)| fold(x) == fold(l)) {
                        p.labels.push((l.to_owned(), col.name.clone()));
                    }
                }
            }
            Target::FixVersion => first(&mut p.fix_version),
            Target::StartDate | Target::DueDate => {
                let slot = if col.target == Target::StartDate {
                    &mut p.start
                } else {
                    &mut p.due
                };
                if slot.is_some() {
                    continue;
                }
                match values::parse_date(value, r.order) {
                    Some(d) => *slot = Some(d),
                    None => p.problems.push(warning(
                        column,
                        "bad_date",
                        "csv-row-bad-date",
                        &[("value", value.to_owned())],
                    )),
                }
            }
            Target::Estimate | Target::Spent => {
                let slot = if col.target == Target::Estimate {
                    &mut p.estimate
                } else {
                    &mut p.spent
                };
                if slot.is_some() {
                    continue;
                }
                match values::parse_duration(value, col.unit, r.comma, r.units) {
                    Some(0) => {}
                    Some(m) => *slot = Some(m),
                    None => p.problems.push(warning(
                        column,
                        "bad_duration",
                        "csv-row-bad-duration",
                        &[("value", value.to_owned())],
                    )),
                }
            }
            Target::Comment => {
                let body = if r.jira {
                    values::jira_comment(raw)
                } else {
                    raw
                };
                let body = body.trim();
                if !body.is_empty() {
                    p.comments.push(body.replace("\r\n", "\n"));
                }
            }
            Target::Key => {
                if p.source_key.is_none() {
                    p.source_key = Some(value.to_owned());
                }
                let k = values::reference(value);
                if !p.keys.contains(&k) {
                    p.keys.push(k);
                }
            }
            Target::Parent => first(&mut p.parent),
            Target::Link(kind) => {
                for other in value
                    .split([',', ';'])
                    .map(str::trim)
                    .filter(|o| !o.is_empty())
                {
                    p.links.push(LinkRef {
                        kind: *kind,
                        other: other.to_owned(),
                        column: col.name.clone(),
                    });
                }
            }
            Target::Relations => {
                for item in values::redmine_relations(value) {
                    match item {
                        Ok((kind, other)) => p.links.push(LinkRef {
                            kind,
                            other,
                            column: col.name.clone(),
                        }),
                        Err(text) => p.problems.push(warning(
                            column,
                            "bad_relation",
                            "csv-row-bad-relation",
                            &[("value", text)],
                        )),
                    }
                }
            }
            Target::Field(name) => {
                multi_field
                    .entry(name.clone())
                    .or_default()
                    .push(value.to_owned());
            }
            Target::Ignore => {}
        }
    }
    for (name, vals) in multi_field {
        let kind = r
            .fields
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(FieldKind::Text, |(_, k)| *k);
        let one = |v: &str| -> Option<Value> {
            match kind {
                FieldKind::Date => {
                    values::parse_date(v, r.order).map(|d| Value::String(d.to_string()))
                }
                FieldKind::Decimal | FieldKind::Integer if r.comma => {
                    Some(Value::String(v.replace('.', "").replace(',', ".")))
                }
                _ => Some(Value::String(v.to_owned())),
            }
        };
        let value = if kind == FieldKind::MultiSelect {
            let items: Vec<Value> = vals
                .iter()
                .flat_map(|v| v.split(',').map(str::trim).filter(|s| !s.is_empty()))
                .filter_map(one)
                .collect();
            Some(Value::Array(items))
        } else {
            vals.first().and_then(|v| one(v))
        };
        match value {
            Some(v) => {
                p.fields.insert(name, v);
            }
            None => p.problems.push(warning(
                &name,
                "bad_date",
                "csv-row-bad-date",
                &[("value", vals.first().cloned().unwrap_or_default())],
            )),
        }
    }
    if p.title.is_empty() {
        p.problems.push(problem(
            CsvSeverity::Error,
            None,
            "no_title",
            "csv-row-no-title",
            &[],
        ));
    }
    if let (Some(s), Some(d)) = (p.start, p.due)
        && s > d
    {
        p.problems.push(problem(
            CsvSeverity::Warning,
            None,
            "start_after_due",
            "csv-row-start-after-due",
            &[("start", s.to_string()), ("due", d.to_string())],
        ));
        p.start = None;
    }
    p
}

/// What the command found and did, row by row.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Done {
    rows: Vec<RowDone>,
    parents: u64,
    links: u64,
    time_entries: u64,
    comments: u64,
    group: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RowDone {
    key: Option<String>,
    problems: Vec<CsvProblem>,
}

/// Whether a failure must stop the whole import rather than one row.
fn stops_all(f: &Failure) -> bool {
    match f {
        Failure::Op(e) => matches!(
            e.code,
            ErrorCode::Limit | ErrorCode::Forbidden | ErrorCode::Storage | ErrorCode::Busy
        ),
        Failure::Store(_) | Failure::SnapshotFirst { .. } => true,
    }
}

/// Splits a failure into one that stops everything, or a row's problem.
fn row_failure(
    f: Failure,
    severity: CsvSeverity,
    column: Option<&str>,
    code: &str,
) -> Result<CsvProblem, Failure> {
    if stops_all(&f) {
        return Err(f);
    }
    let e = OpError::from(f);
    Ok(refused(severity, column, code, &e))
}

/// The project's name for each value of one list the rows use, creating
/// the missing ones when allowed. Values that can't be had are left out.
fn resolve_values(
    w: &mut WriteCtx<'_>,
    p: &lookup::ProjectRow,
    list: List,
    wanted: &[String],
    create: bool,
    made: &mut Vec<String>,
) -> Result<HashMap<String, Result<String, CsvProblem>>, Failure> {
    let mut out = HashMap::new();
    for name in wanted {
        let key = fold(name);
        if out.contains_key(&key) {
            continue;
        }
        let found = match lookup::named(w.tx(), list, p, "value", name) {
            Ok(n) => Ok(n.name),
            Err(Failure::Op(e)) if e.code == ErrorCode::NotFound && create => {
                let made_it = w.attempt(|w| match list {
                    List::Statuses => {
                        let category = values::guess_category(name);
                        let hue = match category {
                            Category::Done => Hue::Green,
                            Category::InProgress => Hue::Blue,
                            Category::Todo | Category::Cancelled => Hue::Grey,
                        };
                        project::apply_status(
                            w,
                            p,
                            &StatusChange {
                                name: Some(name.clone()),
                                category: Some(category),
                                hue: Some(hue),
                                ..StatusChange::default()
                            },
                        )
                    }
                    List::Types => project::apply_type(
                        w,
                        p,
                        &TypeChange {
                            name: Some(name.clone()),
                            ..TypeChange::default()
                        },
                    ),
                    List::Labels => project::apply_label(
                        w,
                        p,
                        &LabelChange {
                            name: Some(name.clone()),
                            ..LabelChange::default()
                        },
                    ),
                });
                match made_it {
                    Ok(()) => {
                        made.push(name.clone());
                        Ok(name.clone())
                    }
                    Err(f) => Err(row_failure(f, CsvSeverity::Error, None, "refused")?),
                }
            }
            Err(f @ Failure::Op(_)) => {
                let (code, id) = match list {
                    List::Statuses => ("unknown_status", "csv-row-unknown-status"),
                    List::Types => ("unknown_type", "csv-row-unknown-type"),
                    List::Labels => ("unknown_label", "csv-row-unknown-label"),
                };
                if stops_all(&f) {
                    return Err(f);
                }
                Err(problem(
                    CsvSeverity::Error,
                    None,
                    code,
                    id,
                    &[("value", name.clone()), ("project", p.key.clone())],
                ))
            }
            Err(f) => return Err(f),
        };
        out.insert(key, found);
    }
    Ok(out)
}

/// The project's names for the statuses, types and labels the rows use,
/// with those created to have them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Resolved {
    statuses: HashMap<String, Result<String, CsvProblem>>,
    types: HashMap<String, Result<String, CsvProblem>>,
    labels: HashMap<String, Result<String, CsvProblem>>,
    new_statuses: Vec<String>,
    new_types: Vec<String>,
    new_labels: Vec<String>,
    group: Option<String>,
}

/// The rows without errors of their own.
fn live(rows: &[Planned]) -> Vec<usize> {
    (0..rows.len()).filter(|i| !rows[*i].has_errors()).collect()
}

/// Finds, or creates when allowed, the statuses, types and labels the
/// rows name. Its own command: Undo of the import moves the tasks to the
/// trash, where they still use what this made.
fn find_or_create(
    w: &mut WriteCtx<'_>,
    project_key: &str,
    rows: &[Planned],
    create: bool,
) -> Result<Resolved, Failure> {
    let p = lookup::project(w.tx(), "project", project_key)?;
    lookup::ensure_active(&p)?;
    let live = live(rows);
    let statuses: Vec<String> = live
        .iter()
        .filter_map(|i| rows[*i].status.as_ref().map(|s| s.0.clone()))
        .collect();
    let types: Vec<String> = live
        .iter()
        .filter_map(|i| rows[*i].task_type.as_ref().map(|s| s.0.clone()))
        .collect();
    let labels: Vec<String> = live
        .iter()
        .flat_map(|i| rows[*i].labels.iter().map(|l| l.0.clone()))
        .collect();
    let mut r = Resolved::default();
    r.statuses = resolve_values(
        w,
        &p,
        List::Statuses,
        &statuses,
        create,
        &mut r.new_statuses,
    )?;
    r.types = resolve_values(w, &p, List::Types, &types, create, &mut r.new_types)?;
    r.labels = resolve_values(w, &p, List::Labels, &labels, create, &mut r.new_labels)?;
    Ok(r)
}

/// The command: a task for every row, then what refers to other rows.
fn run(
    w: &mut WriteCtx<'_>,
    project_key: &str,
    rows: &[Planned],
    resolved: &Resolved,
) -> Result<Done, Failure> {
    let p = lookup::project(w.tx(), "project", project_key)?;
    lookup::ensure_active(&p)?;
    let mut done = Done {
        rows: rows
            .iter()
            .map(|r| RowDone {
                key: None,
                problems: r.problems.clone(),
            })
            .collect(),
        ..Done::default()
    };
    let live = live(rows);
    let (statuses, types, labels) = (&resolved.statuses, &resolved.types, &resolved.labels);

    // First the tasks.
    let mut by_ref: HashMap<String, String> = HashMap::new();
    for &i in &live {
        let row = &rows[i];
        let out = &mut done.rows[i];
        let pick = |v: &Option<(String, String)>,
                    found: &HashMap<String, Result<String, CsvProblem>>,
                    problems: &mut Vec<CsvProblem>| {
            v.as_ref()
                .and_then(|(name, column)| match found.get(&fold(name)) {
                    Some(Ok(n)) => Some(n.clone()),
                    Some(Err(prob)) => {
                        let mut prob = prob.clone();
                        prob.column = Some(column.clone());
                        problems.push(prob);
                        None
                    }
                    None => None,
                })
        };
        let status = pick(&row.status, statuses, &mut out.problems);
        let task_type = pick(&row.task_type, types, &mut out.problems);
        let mut label_names = Vec::new();
        for (l, column) in &row.labels {
            match labels.get(&fold(l)) {
                Some(Ok(n)) => label_names.push(n.clone()),
                Some(Err(prob)) => {
                    let mut prob = prob.clone();
                    prob.column = Some(column.clone());
                    out.problems.push(prob);
                }
                None => {}
            }
        }
        if out
            .problems
            .iter()
            .any(|p| p.severity == CsvSeverity::Error)
        {
            continue;
        }
        // A version the project doesn't have, or that isn't open, is left
        // out; the task is still made.
        let fix_version = match &row.fix_version {
            Some((name, column)) => {
                match version::resolve_for_task(w.tx(), &p, "fix_version", name) {
                    Ok(id) => Some(id.to_string()),
                    Err(f) => {
                        out.problems.push(row_failure(
                            f,
                            CsvSeverity::Warning,
                            Some(column),
                            "unknown_version",
                        )?);
                        None
                    }
                }
            }
            None => None,
        };
        let input = TaskCreateInput {
            project: p.id.to_string(),
            title: row.title.clone(),
            description: row.description.clone(),
            status,
            task_type,
            priority: row.priority,
            labels: label_names,
            fix_version,
            start_date: row.start.map(|d| d.to_string()),
            due_date: row.due.map(|d| d.to_string()),
            estimate: row.estimate.map(|m| format!("{m}m")),
            fields: row.fields.clone(),
            ..TaskCreateInput::default()
        };
        // The rules of the task's status hold for it as for `task.create`:
        // a required field it lacks makes the row an error.
        let mut by_rule = false;
        let made = w.attempt(|w| {
            let change = task::create(w, input)?;
            let id = lookup::task(w.tx(), "key", &change.task.key)?.id;
            rules::enforce_created(w, id).inspect_err(|_| by_rule = true)?;
            Ok(change)
        });
        match made {
            Ok(change) => {
                for k in &row.keys {
                    by_ref
                        .entry(k.clone())
                        .or_insert_with(|| change.task.key.clone());
                }
                out.key = Some(change.task.key);
            }
            Err(f) if by_rule && !matches!(f, Failure::Store(_)) => {
                let e = OpError::from(f);
                out.problems
                    .push(refused(CsvSeverity::Error, None, "status_rule", &e));
            }
            Err(f) => out
                .problems
                .push(row_failure(f, CsvSeverity::Error, None, "refused")?),
        }
    }

    // Then what refers to other rows, and the rest.
    let find = |by_ref: &HashMap<String, String>, w: &WriteCtx<'_>, text: &str| {
        by_ref.get(&values::reference(text)).cloned().or_else(|| {
            lookup::task(w.tx(), "key", text.trim())
                .ok()
                .and_then(|t| lookup::task_key(w.tx(), t.project, t.number).ok())
        })
    };
    for &i in &live {
        let row = &rows[i];
        let Some(key) = done.rows[i].key.clone() else {
            continue;
        };
        let mut problems = Vec::new();
        if let Some((parent, column)) = &row.parent {
            match find(&by_ref, w, parent) {
                Some(pk) if pk == key => {}
                Some(pk) => {
                    let input = TaskUpdateInput {
                        key: key.clone(),
                        parent: Some(pk),
                        ..TaskUpdateInput::default()
                    };
                    match w.attempt(|w| task::update(w, input)) {
                        Ok(_) => done.parents += 1,
                        Err(f) => problems.push(row_failure(
                            f,
                            CsvSeverity::Warning,
                            Some(column),
                            "parent_refused",
                        )?),
                    }
                }
                None => problems.push(warning(
                    column,
                    "parent_not_found",
                    "csv-row-parent-not-found",
                    &[("value", parent.clone())],
                )),
            }
        }
        for link in &row.links {
            let Some(other) = find(&by_ref, w, &link.other) else {
                problems.push(warning(
                    &link.column,
                    "link_not_found",
                    "csv-row-link-not-found",
                    &[("value", link.other.clone())],
                ));
                continue;
            };
            if other == key {
                continue;
            }
            let input = RelationInput {
                key: key.clone(),
                kind: link.kind,
                other,
                lag_days: None,
            };
            let before = w.changed_of(relation::ENTITY).len();
            match w.attempt(|w| relation::add(w, input)) {
                Ok(_) => {
                    if w.changed_of(relation::ENTITY).len() > before {
                        done.links += 1;
                    }
                }
                Err(f) => problems.push(row_failure(
                    f,
                    CsvSeverity::Warning,
                    Some(&link.column),
                    "link_refused",
                )?),
            }
        }
        if let Some(mut minutes) = row.spent {
            let mut entries = 0;
            while minutes > 0 && entries < MAX_ENTRIES {
                let chunk = minutes.min(ENTRY_MINUTES);
                let input = TimeLogInput {
                    key: key.clone(),
                    duration: format!("{chunk}m"),
                    ..TimeLogInput::default()
                };
                match w.attempt(|w| time::log(w, input)) {
                    Ok(_) => done.time_entries += 1,
                    Err(f) => {
                        problems.push(row_failure(f, CsvSeverity::Warning, None, "time_refused")?);
                        break;
                    }
                }
                minutes -= chunk;
                entries += 1;
            }
        }
        for body in &row.comments {
            let input = CommentAddInput {
                key: key.clone(),
                body: body.clone(),
                ..CommentAddInput::default()
            };
            match w.attempt(|w| comment::add_comment(w, &input, "body")) {
                Ok(_) => done.comments += 1,
                Err(f) => {
                    problems.push(row_failure(
                        f,
                        CsvSeverity::Warning,
                        None,
                        "comment_refused",
                    )?);
                }
            }
        }
        done.rows[i].problems.extend(problems);
    }
    Ok(done)
}

fn to_json<T: Serialize>(v: &T) -> Result<Value, Failure> {
    serde_json::to_value(v).map_err(|e| {
        Failure::Op(messages::storage(
            "storage-json",
            &[("error", e.to_string())],
        ))
    })
}

fn from_json<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, OpError> {
    serde_json::from_value(v)
        .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))
}

fn rows_have_errors(count: usize) -> OpError {
    messages::hint(
        messages::with(
            "csv-rows-have-errors",
            &[("count", count.to_string())],
            OpError::invalid,
        ),
        "csv-rows-have-errors-hint",
        &[],
    )
    .with_detail("count", count as u64)
}

impl Engine {
    /// Imports the CSV file `input.path` names into `input.project`, or with
    /// `dry_run` only checks it and reports each row's outcome.
    pub fn import_csv(
        &self,
        ctx: &Context,
        input: &CsvImportInput,
    ) -> Result<CsvImportResult, OpError> {
        let format = input.format.clone().unwrap_or_default();
        check_format(&format).map_err(OpError::from)?;
        let path = resolve(
            self,
            ctx,
            &input.path,
            "path",
            crate::agent_files::Access::Read,
        )?;
        let setup = self
            .store()
            .read(|tx| {
                let _scope = crate::scope::enter(tx, ctx)?;
                let project = lookup::project(tx, "project", &input.project)?;
                lookup::ensure_active(&project)?;
                let fields = custom_field::rows(tx, Some(project.id))?
                    .into_iter()
                    .map(|f| (f.name, f.kind))
                    .collect();
                let preset = input
                    .preset
                    .as_deref()
                    .map(|name| presets::find(tx, name))
                    .transpose()?;
                Ok::<_, Failure>(Setup {
                    key: project.key,
                    fields,
                    preset,
                    units: task::units(tx)?,
                })
            })
            .map_err(OpError::from)?;
        let bytes = read_file(&path)?;
        let preset_format: CsvFormat = setup
            .preset
            .as_ref()
            .and_then(|p| p.format.clone())
            .map(Into::into)
            .unwrap_or_default();
        let encoding = format.encoding.or(preset_format.encoding);
        let decoded = decode::decode(&bytes, encoding).map_err(|e| decode_error(&e))?;
        drop(bytes);
        let quote = match format.quote.as_deref().or(preset_format.quote.as_deref()) {
            Some(q) => one_char("format.quote", q).map_err(OpError::from)?,
            None => parse::detect_quote(&decoded.text),
        };
        let separator = match format
            .separator
            .as_deref()
            .or(preset_format.separator.as_deref())
        {
            Some(s) => one_char("format.separator", s).map_err(OpError::from)?,
            None => parse::detect_separator(&decoded.text, quote),
        };
        let max_rows = if ctx.is_agent() {
            MAX_ROWS_AGENT
        } else {
            MAX_ROWS
        };
        let records = parse::records(&decoded.text, separator, quote, max_rows + 1)
            .map_err(|e| parse_error(&e))?;
        let headers: Vec<String> = records[0]
            .fields
            .iter()
            .map(|h| h.trim().to_owned())
            .collect();
        for h in &headers {
            if h.chars().count() > parse::MAX_HEADER
                || h.chars()
                    .any(|c| parse::is_forbidden_control(c) || c == '\n' || c == '\r')
            {
                return Err(messages::invalid_at(
                    "path",
                    "csv-bad-header",
                    &[("max", parse::MAX_HEADER.to_string())],
                ));
            }
        }
        // Without a preset, one the header row looks like.
        let mut setup = setup;
        let mut preset_name = setup.preset.as_ref().map(|p| p.name.clone());
        if setup.preset.is_none()
            && let Some(name) = presets::detect(&headers)
        {
            setup.preset = Some(
                self.store()
                    .read(|tx| presets::find(tx, name))
                    .map_err(OpError::from)?,
            );
            preset_name = Some(name.to_owned());
        }
        let columns = mapping(&headers, input, &setup).map_err(OpError::from)?;
        let data = &records[1..];

        let date_columns: Vec<usize> = columns
            .iter()
            .filter(|c| match &c.target {
                Target::StartDate | Target::DueDate => true,
                Target::Field(n) => setup
                    .fields
                    .iter()
                    .any(|(f, k)| f == n && *k == FieldKind::Date),
                _ => false,
            })
            .map(|c| c.index)
            .collect();
        let detected_order = values::detect_order(
            data.iter()
                .flat_map(|r| date_columns.iter().filter_map(|i| r.fields.get(*i)))
                .map(String::as_str),
        );
        let order = format
            .date_order
            .or(preset_format.date_order)
            .or(detected_order);
        let comma = format
            .decimal_comma
            .or(preset_format.decimal_comma)
            .unwrap_or(separator == ';');
        let mut value_maps: CsvValueMaps = setup
            .preset
            .as_ref()
            .and_then(|p| p.values.clone())
            .map(Into::into)
            .unwrap_or_default();
        if let Some(v) = &input.values {
            value_maps.statuses.extend(v.statuses.clone());
            value_maps.types.extend(v.types.clone());
            value_maps.priorities.extend(v.priorities.clone());
        }
        let reading = Reading {
            order: order.unwrap_or(CsvDateOrder::Dmy),
            comma,
            units: setup.units,
            values: &value_maps,
            fields: &setup.fields,
            jira: preset_name.as_deref() == Some(presets::JIRA),
        };
        let mut planned: Vec<Planned> = data
            .iter()
            .map(|r| plan_row(r, &columns, &reading))
            .collect();
        // A key two rows share refers to the first.
        let mut seen: HashMap<String, u64> = HashMap::new();
        for p in &mut planned {
            let mut dup = None;
            for k in &p.keys {
                if let Some(first) = seen.get(k) {
                    dup = Some((k.clone(), *first));
                } else {
                    seen.insert(k.clone(), p.line);
                }
            }
            if let Some((k, first)) = dup {
                p.problems.push(problem(
                    CsvSeverity::Warning,
                    None,
                    "duplicate_key",
                    "csv-row-duplicate-key",
                    &[("value", k), ("line", first.to_string())],
                ));
            }
        }

        let shown: Vec<CsvColumn> = columns
            .iter()
            .map(|c| CsvColumn {
                index: u32::try_from(c.index).unwrap_or(u32::MAX),
                name: c.name.clone(),
                to: target_name(&c.target),
                unit: matches!(c.target, Target::Estimate | Target::Spent).then_some(c.unit),
                samples: data
                    .iter()
                    .filter_map(|r| r.fields.get(c.index))
                    .map(|v| v.trim())
                    .filter(|v| !v.is_empty())
                    .take(SAMPLES)
                    .map(|v| {
                        let clean: String = v
                            .chars()
                            .map(|ch| if ch.is_control() { ' ' } else { ch })
                            .collect();
                        let mut s: String = clean.chars().take(SAMPLE_CHARS).collect();
                        if clean.chars().count() > SAMPLE_CHARS {
                            s.push('…');
                        }
                        s
                    })
                    .collect(),
            })
            .collect();

        let dry_run = input.dry_run;
        let create = input.create_missing.unwrap_or(true);
        let skip_invalid = input.skip_invalid;
        let rows = Arc::new(planned);
        let key = setup.key.clone();
        let command_rows = Arc::clone(&rows);
        let op = descriptor(TransferImportCsv::ID)
            .ok_or_else(|| OpError::storage("transfer.import_csv is not in the catalog"))?;
        let value = serde_json::to_value(input)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        let errors = rows.iter().filter(|r| r.has_errors()).count();
        let (resolved, done) = if dry_run {
            let f: CommandFn = Arc::new(move |w, _| {
                let rows = &command_rows;
                let both = w.preview(|w| {
                    let resolved = find_or_create(w, &key, rows, create)?;
                    let done = run(w, &key, rows, &resolved)?;
                    Ok((resolved, done))
                })?;
                to_json(&both)
            });
            let output = self.execute_command(ctx, op, f, value)?;
            from_json::<(Resolved, Done)>(output)?
        } else {
            if errors > 0 && !skip_invalid {
                return Err(rows_have_errors(errors));
            }
            let setup_rows = Arc::clone(&command_rows);
            let setup_key = key.clone();
            let f: CommandFn = Arc::new(move |w, _| {
                let mut resolved = find_or_create(w, &setup_key, &setup_rows, create)?;
                resolved.group = w.group_if_changed();
                to_json(&resolved)
            });
            let output = self.execute_command(ctx, op, f, value.clone())?;
            let resolved: Resolved = from_json(output)?;
            let shared = Arc::new(resolved.clone());
            let f: CommandFn = Arc::new(move |w, _| {
                let rows = &command_rows;
                let mut done = run(w, &key, rows, &shared)?;
                let skipped = done.rows.iter().filter(|r| r.key.is_none()).count();
                if skipped > 0 && !skip_invalid {
                    return Err(Failure::Op(rows_have_errors(skipped)));
                }
                done.group = w.group_if_changed();
                to_json(&done)
            });
            match self.execute_command(ctx, op, f, value) {
                Ok(output) => (resolved, from_json::<Done>(output)?),
                Err(e) => {
                    // Take back what the first step made for nothing.
                    if let Some(g) = &resolved.group {
                        let undone =
                            self.call(ctx, "history.revert", serde_json::json!({ "groups": [g] }));
                        if let Err(u) = undone {
                            tracing::warn!(error = %u.message, "could not take back the statuses, types and labels of a failed CSV import");
                        }
                    }
                    return Err(e);
                }
            }
        };

        let mut counts = CsvCounts {
            rows: rows.len() as u64,
            parents: done.parents,
            links: done.links,
            time_entries: done.time_entries,
            comments: done.comments,
            ..CsvCounts::default()
        };
        let mut out_rows = Vec::with_capacity(rows.len());
        for (plan, row) in rows.iter().zip(done.rows) {
            let outcome = if row.key.is_some() {
                counts.created += 1;
                CsvOutcome::Created
            } else {
                counts.skipped += 1;
                CsvOutcome::Skipped
            };
            if row
                .problems
                .iter()
                .any(|p| p.severity == CsvSeverity::Warning)
            {
                counts.warnings += 1;
            }
            if input.only_problems && row.problems.is_empty() {
                continue;
            }
            out_rows.push(CsvRow {
                line: plan.line,
                outcome,
                key: row.key,
                source_key: plan.source_key.clone(),
                title: plan.title.chars().take(200).collect(),
                problems: row.problems,
            });
        }
        Ok(CsvImportResult {
            dry_run,
            project: setup.key,
            preset: preset_name,
            format: CsvDetected {
                encoding: decoded.encoding,
                bom: decoded.bom,
                separator: separator.to_string(),
                quote: quote.to_string(),
                date_order: order,
                decimal_comma: comma,
            },
            columns: shown,
            blocked: dry_run && !skip_invalid && counts.skipped > 0,
            counts,
            new_statuses: resolved.new_statuses,
            new_types: resolved.new_types,
            new_labels: resolved.new_labels,
            setup_group: resolved.group,
            rows: out_rows,
            group: done.group,
        })
    }
}
