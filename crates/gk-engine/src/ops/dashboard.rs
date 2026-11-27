// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards. A dashboard is kept whole in history (one `record` field),
//! so making, changing and deleting one, and every change to its widgets,
//! can be undone and reverted.
//!
//! A widget computes nothing of its own: each kind asks the engine's
//! existing reports and searches, with the widget's settings, so one widget
//! is one call and shows exactly what the matching screen shows.
//!
//! Dashboards belong to the workspace: everyone using it sees the same
//! ones. The table can take an owner later, should a workspace ever have
//! several people.

use gk_api::OpError;
use gk_api::ops::automation::AutomationRunsInput;
use gk_api::ops::common::ActorKind;
use gk_api::ops::dashboard::{
    Dashboard, DashboardAddWidget, DashboardAddWidgetInput, DashboardChange, DashboardCreate,
    DashboardCreateInput, DashboardDelete, DashboardField, DashboardList, DashboardListInput,
    DashboardListOp, DashboardRef, DashboardRemoveWidget, DashboardRemoveWidgetInput,
    DashboardShow, DashboardShowOp, DashboardUpdate, DashboardUpdateInput, DashboardUpdateWidget,
    DashboardUpdateWidgetInput, DashboardWidgetInput, DashboardWidgetOp, Widget, WidgetBar,
    WidgetKind, WidgetSetting, WidgetSettings, WidgetSpec, WidgetTask, WidgetValue,
};
use gk_api::ops::history::ActivityQuery;
use gk_api::ops::query::{ListGrouping, TaskSearchInput};
use gk_api::ops::report::FlowReportInput;
use gk_api::ops::sprint::SprintBurndownInput;
use gk_api::ops::time::{TimeGroupBy, TimeReportInput};
use gk_api::ops::version::{VersionListInput, VersionStatus};
use gk_core::{Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::lookup::{self, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for dashboards.
pub(crate) const ENTITY: &str = "dashboard";
/// The one recorded field: the whole dashboard.
pub(crate) const RECORD: &str = "record";
/// The longest dashboard name, in characters.
const NAME_MAX: usize = 100;
/// The longest description, in characters.
const DESCRIPTION_MAX: usize = 1000;
/// The longest widget title, in characters.
const TITLE_MAX: usize = 100;
/// The longest note, in characters.
const TEXT_MAX: usize = 10_000;
/// The longest query, in characters.
const QUERY_MAX: usize = 2000;
/// The most widgets on one dashboard.
const WIDGETS_MAX: usize = 40;
/// The most rows a widget lists.
const LIMIT_MAX: u32 = 50;
/// How many rows a widget lists unless told.
const LIMIT_DEFAULT: u32 = 10;
/// The most days a widget looks at.
const DAYS_MAX: u32 = 366;
/// How many days a widget looks at unless told.
const DAYS_DEFAULT: u32 = 7;

pub(super) fn register(r: &mut Registry) {
    r.query::<DashboardListOp, _>(desc::<DashboardListOp>(), |q, input: DashboardListInput| {
        list(q.tx, &input)
    });
    r.query::<DashboardShowOp, _>(desc::<DashboardShowOp>(), |q, input: DashboardRef| {
        show(q, &input)
    });
    r.query::<DashboardWidgetOp, _>(
        desc::<DashboardWidgetOp>(),
        |q, input: DashboardWidgetInput| {
            let (_, record) = find(q.tx, &input.dashboard, input.project.as_ref())?;
            let (at, stored) = widget_ref(&record, &input.widget)?;
            let widget = widget_out(q.tx, stored)?;
            value(q, &record, at, widget)
        },
    );
    r.command::<DashboardCreate, _>(desc::<DashboardCreate>(), create);
    r.command::<DashboardUpdate, _>(desc::<DashboardUpdate>(), update);
    r.command::<DashboardDelete, _>(desc::<DashboardDelete>(), delete);
    r.command::<DashboardAddWidget, _>(desc::<DashboardAddWidget>(), add_widget);
    r.command::<DashboardUpdateWidget, _>(desc::<DashboardUpdateWidget>(), update_widget);
    r.command::<DashboardRemoveWidget, _>(desc::<DashboardRemoveWidget>(), remove_widget);
}

// ---------------------------------------------------------------------
// The stored dashboard

/// A widget as stored: its project, if any, by ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Stored {
    pub(crate) id: String,
    pub(crate) kind: WidgetKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) title: String,
    pub(crate) width: u8,
    #[serde(default)]
    pub(crate) settings: WidgetSettings,
}

/// A dashboard as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) project: Option<String>,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) widgets: Vec<Stored>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

impl Record {
    pub(crate) fn project_id(&self) -> Option<Id> {
        self.project.as_deref().and_then(|p| p.parse().ok())
    }
}

const COLUMNS: &str = "id, project_id, name, description, widgets, rank, created_at, \
                       created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    let widgets: String = r.get(4)?;
    let widgets: Vec<Stored> = serde_json::from_str(&widgets).map_err(|e| {
        gk_store::rusqlite::Error::FromSqlConversionFailure(
            4,
            gk_store::rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    Ok((
        id_at(r, 0)?,
        Record {
            project: opt_id_at(r, 1)?.map(|p| p.to_string()),
            name: r.get(2)?,
            description: r.get(3)?,
            widgets,
            rank: r.get(5)?,
            created_at: r.get(6)?,
            created_by_kind: r.get(7)?,
            created_by_name: r.get(8)?,
            updated_at: r.get(9)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM dashboards WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored dashboard `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Writes dashboard `id` as history recorded it (`None` deletes it),
/// without recording.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM dashboards WHERE id = ?1", [b(id)])?;
    let Some(value) = value else {
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    let widgets = serde_json::to_string(&r.widgets)
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    w.tx().execute(
        "INSERT INTO dashboards (id, project_id, name, name_fold, description, widgets, rank,
                                 created_at, created_by_kind, created_by_name, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            b(id),
            r.project_id().map(b),
            r.name,
            fold(&r.name),
            r.description,
            widgets,
            r.rank,
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
        ],
    )?;
    Ok(())
}

/// Writes dashboard `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    let project = record.and_then(Record::project_id).or_else(|| {
        before
            .as_ref()
            .and_then(|v| v.get("project"))
            .and_then(Value::as_str)
            .and_then(|p| p.parse().ok())
    });
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, project, RECORD, before, after);
    Ok(())
}

/// Every dashboard: the workspace's first, then by project, each in order.
fn rows(tx: &Transaction<'_>) -> Result<Vec<(Id, Record)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM dashboards
          ORDER BY project_id IS NOT NULL, project_id, rank, id"
    ))?;
    let rows = stmt
        .query_map([], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, r)| r.project_id().is_none_or(crate::scope::sees))
        .collect())
}

/// The dashboards of exactly `project` (`None`: the workspace's own), in
/// order, for an export.
pub(crate) fn of_scope(
    tx: &Transaction<'_>,
    project: Option<Id>,
) -> Result<Vec<(Id, Record)>, Failure> {
    Ok(rows(tx)?
        .into_iter()
        .filter(|(_, r)| r.project_id() == project)
        .collect())
}

/// Whether the workspace's own dashboards (`project` `None`) or a
/// project's already have one named `name`.
pub(crate) fn name_taken(
    tx: &Transaction<'_>,
    project: Option<Id>,
    name: &str,
) -> Result<bool, Failure> {
    let wanted = fold(name);
    Ok(of_scope(tx, project)?
        .iter()
        .any(|(_, r)| fold(&r.name) == wanted))
}

/// The widgets of an archive's dashboard, if each one is a widget as this
/// program keeps it: known kind and settings, a width of 1 to 3, its own ID,
/// and no more than a dashboard holds.
pub(crate) fn archived_widgets(values: &[Value]) -> Option<Vec<Stored>> {
    if values.len() > WIDGETS_MAX {
        return None;
    }
    let mut ids = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let w: Stored = serde_json::from_value(v.clone()).ok()?;
        if !(1..=3).contains(&w.width)
            || w.title.chars().count() > TITLE_MAX
            || w.id.parse::<Id>().is_err()
            || !ids.insert(w.id.clone())
        {
            return None;
        }
        out.push(w);
    }
    Some(out)
}

/// Stores dashboard `id` from an archive, outside history (an import is
/// one change of its own).
pub(crate) fn insert_imported(
    tx: &Transaction<'_>,
    id: Id,
    record: &Record,
) -> Result<(), Failure> {
    let widgets = serde_json::to_string(&record.widgets)
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    tx.execute(
        "INSERT INTO dashboards (id, project_id, name, name_fold, description, widgets, rank,
                                 created_at, created_by_kind, created_by_name, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            b(id),
            record.project_id().map(b),
            record.name,
            fold(&record.name),
            record.description,
            widgets,
            record.rank,
            record.created_at,
            record.created_by_kind,
            record.created_by_name,
            record.updated_at,
        ],
    )?;
    Ok(())
}

/// Deletes the dashboards pinned to `project`, which is being deleted for
/// good. History already keeps no content for its items.
pub(crate) fn purge_project(tx: &Transaction<'_>, project: Id) -> Result<(), Failure> {
    tx.execute("DELETE FROM dashboards WHERE project_id = ?1", [b(project)])?;
    Ok(())
}

fn project_key(tx: &Transaction<'_>, project: Option<Id>) -> Result<Option<String>, Failure> {
    Ok(match project {
        Some(p) => tx
            .query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get(0)
            })
            .optional()?,
        None => None,
    })
}

/// A widget for callers: its project by key (by ID while no project has
/// it, so the setting still shows).
fn widget_out(tx: &Transaction<'_>, w: &Stored) -> Result<Widget, Failure> {
    let mut settings = w.settings.clone();
    if let Some(p) = &settings.project
        && let Ok(id) = p.parse::<Id>()
        && let Some(key) = project_key(tx, Some(id))?
    {
        settings.project = Some(key);
    }
    Ok(Widget {
        id: w.id.clone(),
        kind: w.kind,
        title: w.title.clone(),
        width: w.width,
        settings,
    })
}

fn output(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<Dashboard, Failure> {
    Ok(Dashboard {
        name: r.name.clone(),
        project: project_key(tx, r.project_id())?,
        description: r.description.clone(),
        widgets: r
            .widgets
            .iter()
            .map(|w| widget_out(tx, w))
            .collect::<Result<_, _>>()?,
        rank: r.rank.clone(),
        created_by: gk_api::ops::common::Actor {
            kind: ActorKind::from_stored(&r.created_by_kind),
            name: r.created_by_name.clone(),
        },
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        id: id.to_string(),
    })
}

fn list(tx: &Transaction<'_>, input: &DashboardListInput) -> Result<DashboardList, Failure> {
    let scope = match &input.project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let items = rows(tx)?
        .into_iter()
        .filter(|(_, r)| input.all || r.project_id() == scope)
        .map(|(id, r)| output(tx, id, &r))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DashboardList {
        total: items.len() as u64,
        items,
    })
}

/// The dashboard `text` names: its ID, or its name among the dashboards of
/// `project` (the workspace's own without one).
fn find(
    tx: &Transaction<'_>,
    text: &str,
    project: Option<&String>,
) -> Result<(Id, Record), Failure> {
    if let Ok(id) = text.trim().parse::<Id>()
        && let Some(r) = load(tx, id)?
        && r.project_id().is_none_or(crate::scope::sees)
    {
        return Ok((id, r));
    }
    let scope = match project {
        Some(p) => Some(lookup::project(tx, "project", p)?.id),
        None => None,
    };
    let wanted = fold(text.trim());
    let all: Vec<(Id, Record)> = rows(tx)?
        .into_iter()
        .filter(|(_, r)| r.project_id() == scope)
        .collect();
    if let Some(found) = all.iter().find(|(_, r)| fold(&r.name) == wanted) {
        return Ok(found.clone());
    }
    let names: Vec<&str> = all.iter().map(|(_, r)| r.name.as_str()).collect();
    let args = [("dashboard", text.trim().to_owned())];
    let error = messages::with("dashboard-not-found", &args, |m| {
        OpError::not_found(m, &names)
    })
    .with_detail("field", "dashboard");
    Err(messages::hint(error, "dashboard-not-found-hint", &[]).into())
}

/// The place of the widget `text` names: its ID, or its place (`1` is the
/// first).
fn widget_at(r: &Record, text: &str) -> Result<usize, Failure> {
    widget_ref(r, text).map(|(at, _)| at)
}

/// The widget `text` names, with its place: by its ID, or by its place
/// (`1` is the first).
fn widget_ref<'a>(r: &'a Record, text: &str) -> Result<(usize, &'a Stored), Failure> {
    let text = text.trim();
    if let Some(found) = r.widgets.iter().enumerate().find(|(_, w)| w.id == text) {
        return Ok(found);
    }
    if let Some(found) = text
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|at| r.widgets.get(at).map(|w| (at, w)))
    {
        return Ok(found);
    }
    let error = messages::not_found(
        "dashboard-widget-not-found",
        &[
            ("widget", text.to_owned()),
            ("dashboard", r.name.clone()),
            ("count", r.widgets.len().to_string()),
        ],
    )
    .with_detail("field", "widget");
    Err(messages::hint(error, "dashboard-widget-not-found-hint", &[]).into())
}

// ---------------------------------------------------------------------
// Checking widgets

fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    super::task::invalid(field, e)
}

fn is_set(setting: WidgetSetting, s: &WidgetSettings) -> bool {
    match setting {
        WidgetSetting::Query => s.query.is_some(),
        WidgetSetting::Project => s.project.is_some(),
        WidgetSetting::Field => s.field.is_some(),
        WidgetSetting::Limit => s.limit.is_some(),
        WidgetSetting::Days => s.days.is_some(),
        WidgetSetting::Bucket => s.bucket.is_some(),
        WidgetSetting::GroupBy => s.group_by.is_some(),
        WidgetSetting::ActorKind => s.actor_kind.is_some(),
        WidgetSetting::Text => s.text.is_some(),
    }
}

const ALL_SETTINGS: [WidgetSetting; 9] = [
    WidgetSetting::Query,
    WidgetSetting::Project,
    WidgetSetting::Field,
    WidgetSetting::Limit,
    WidgetSetting::Days,
    WidgetSetting::Bucket,
    WidgetSetting::GroupBy,
    WidgetSetting::ActorKind,
    WidgetSetting::Text,
];

fn clear(setting: WidgetSetting, s: &mut WidgetSettings) {
    match setting {
        WidgetSetting::Query => s.query = None,
        WidgetSetting::Project => s.project = None,
        WidgetSetting::Field => s.field = None,
        WidgetSetting::Limit => s.limit = None,
        WidgetSetting::Days => s.days = None,
        WidgetSetting::Bucket => s.bucket = None,
        WidgetSetting::GroupBy => s.group_by = None,
        WidgetSetting::ActorKind => s.actor_kind = None,
        WidgetSetting::Text => s.text = None,
    }
}

/// The settings `new` gives, over `old`.
fn merge(old: &WidgetSettings, new: &WidgetSettings) -> WidgetSettings {
    WidgetSettings {
        query: new.query.clone().or_else(|| old.query.clone()),
        project: new.project.clone().or_else(|| old.project.clone()),
        field: new.field.clone().or_else(|| old.field.clone()),
        limit: new.limit.or(old.limit),
        days: new.days.or(old.days),
        bucket: new.bucket.or(old.bucket),
        group_by: new.group_by.or(old.group_by),
        actor_kind: new.actor_kind.or(old.actor_kind),
        text: new.text.clone().or_else(|| old.text.clone()),
    }
}

/// The fields `tasks_by_field` counts by, besides custom fields.
const FIELDS: [&str; 4] = ["status", "priority", "type", "label"];

/// Checks `settings` for a widget of `kind` on a dashboard pinned to
/// `pinned`, and returns them as stored: trimmed, the project by ID.
fn check_settings(
    w: &WriteCtx<'_>,
    kind: WidgetKind,
    settings: &WidgetSettings,
    pinned: Option<Id>,
) -> Result<WidgetSettings, Failure> {
    let path = |s: WidgetSetting| format!("settings.{}", s.as_str());
    for setting in ALL_SETTINGS {
        if is_set(setting, settings) && !kind.settings().contains(&setting) {
            return Err(messages::invalid(
                &path(setting),
                "dashboard-setting-unused",
                &[
                    ("setting", setting.as_str().to_owned()),
                    ("kind", kind.as_str().to_owned()),
                ],
            ));
        }
    }
    let mut out = settings.clone();
    out.query = match settings.query.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(q) => {
            if q.chars().count() > QUERY_MAX {
                return Err(messages::invalid(
                    &path(WidgetSetting::Query),
                    "dashboard-query-too-long",
                    &[("max", QUERY_MAX.to_string())],
                ));
            }
            super::query::compile(w.tx(), w.zone(), w.now(), q, &path(WidgetSetting::Query))?;
            Some(q.to_owned())
        }
    };
    out.project = match settings.project.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(p) => Some(
            lookup::project(w.tx(), &path(WidgetSetting::Project), p)?
                .id
                .to_string(),
        ),
    };
    if matches!(kind, WidgetKind::Burndown | WidgetKind::VersionProgress)
        && out.project.is_none()
        && pinned.is_none()
    {
        return Err(messages::invalid(
            &path(WidgetSetting::Project),
            "dashboard-needs-project",
            &[("kind", kind.as_str().to_owned())],
        ));
    }
    if kind == WidgetKind::TasksByField {
        let field = settings.field.as_deref().map(str::trim).unwrap_or_default();
        if field.is_empty() {
            return Err(messages::invalid(
                &path(WidgetSetting::Field),
                "dashboard-needs-field",
                &[],
            ));
        }
        let lower = field.to_lowercase();
        if FIELDS.contains(&lower.as_str()) {
            out.field = Some(lower);
        } else if gk_query::fields::strip_custom_prefix(field).is_some() {
            super::query::field_grouping(w.tx(), field)
                .map_err(|f| at_path(f, &path(WidgetSetting::Field)))?;
            out.field = Some(field.to_owned());
        } else {
            let error = messages::invalid_at(
                &path(WidgetSetting::Field),
                "dashboard-field-unknown",
                &[("field", field.to_owned())],
            );
            return Err(messages::hint(error, "dashboard-field-unknown-hint", &[]).into());
        }
    }
    if let Some(limit) = settings.limit
        && !(1..=LIMIT_MAX).contains(&limit)
    {
        return Err(messages::invalid(
            &path(WidgetSetting::Limit),
            "dashboard-limit",
            &[("max", LIMIT_MAX.to_string())],
        ));
    }
    if let Some(days) = settings.days
        && !(1..=DAYS_MAX).contains(&days)
    {
        return Err(messages::invalid(
            &path(WidgetSetting::Days),
            "dashboard-days",
            &[("max", DAYS_MAX.to_string())],
        ));
    }
    if let Some(text) = &settings.text
        && text.chars().count() > TEXT_MAX
    {
        return Err(messages::invalid(
            &path(WidgetSetting::Text),
            "dashboard-text-too-long",
            &[("max", TEXT_MAX.to_string())],
        ));
    }
    Ok(out)
}

/// `failure` with its path set to `path`.
fn at_path(failure: Failure, path: &str) -> Failure {
    match failure {
        Failure::Op(e) => Failure::Op(e.with_detail("path", path)),
        other => other,
    }
}

fn check_title(title: &str) -> Result<String, Failure> {
    if title.trim().is_empty() {
        return Ok(String::new());
    }
    gk_core::check_line("title", title, TITLE_MAX).map_err(|e| invalid("title", e))
}

fn check_width(width: u8) -> Result<u8, Failure> {
    if (1..=3).contains(&width) {
        Ok(width)
    } else {
        Err(messages::invalid("width", "dashboard-width", &[]))
    }
}

/// How wide a widget of `kind` starts: charts take two columns.
const fn default_width(kind: WidgetKind) -> u8 {
    match kind {
        WidgetKind::CreatedClosed
        | WidgetKind::CumulativeFlow
        | WidgetKind::CycleTime
        | WidgetKind::Burndown => 2,
        _ => 1,
    }
}

fn new_widget(w: &WriteCtx<'_>, spec: &WidgetSpec, pinned: Option<Id>) -> Result<Stored, Failure> {
    Ok(Stored {
        id: Id::generate_at(u64::try_from(w.now()).unwrap_or(0)).to_string(),
        kind: spec.kind,
        title: check_title(spec.title.as_deref().unwrap_or_default())?,
        width: check_width(spec.width.unwrap_or_else(|| default_width(spec.kind)))?,
        settings: check_settings(w, spec.kind, &spec.settings, pinned)?,
    })
}

fn check_count(count: usize) -> Result<(), Failure> {
    if count > WIDGETS_MAX {
        return Err(messages::invalid(
            "widgets",
            "dashboard-too-many-widgets",
            &[("max", WIDGETS_MAX.to_string())],
        ));
    }
    Ok(())
}

/// The starter widgets: open tasks, overdue and due soon, open tasks by
/// status, created against closed and the latest changes.
fn starter() -> Vec<WidgetSpec> {
    let spec = |kind: WidgetKind, settings: WidgetSettings| WidgetSpec {
        kind,
        title: None,
        width: None,
        settings,
    };
    vec![
        WidgetSpec {
            title: Some(messages::text("dashboard-starter-open", &[])),
            ..spec(
                WidgetKind::QueryCount,
                WidgetSettings {
                    query: Some("is:open".to_owned()),
                    ..WidgetSettings::default()
                },
            )
        },
        spec(WidgetKind::Overdue, WidgetSettings::default()),
        spec(WidgetKind::DueSoon, WidgetSettings::default()),
        spec(
            WidgetKind::TasksByField,
            WidgetSettings {
                field: Some("status".to_owned()),
                ..WidgetSettings::default()
            },
        ),
        spec(WidgetKind::CreatedClosed, WidgetSettings::default()),
        spec(WidgetKind::Activity, WidgetSettings::default()),
    ]
}

// ---------------------------------------------------------------------
// Commands

/// Refuses a name another dashboard of the same project has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let wanted = fold(&r.name);
    let taken = rows(tx)?.into_iter().any(|(other, o)| {
        other != id && o.project_id() == r.project_id() && fold(&o.name) == wanted
    });
    if taken {
        let error = messages::conflict("dashboard-name-taken", &[("dashboard", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "dashboard-name-taken-hint", &[]).into());
    }
    Ok(())
}

fn change(
    w: &WriteCtx<'_>,
    id: Id,
    r: &Record,
    widget: Option<&Stored>,
    deleted: bool,
) -> Result<DashboardChange, Failure> {
    Ok(DashboardChange {
        dashboard: output(w.tx(), id, r)?,
        widget: widget.map(|s| widget_out(w.tx(), s)).transpose()?,
        deleted,
        group: w.group_if_changed(),
    })
}

/// Refuses changes to a dashboard pinned to an archived project.
fn ensure_open(w: &WriteCtx<'_>, r: &Record) -> Result<(), Failure> {
    if let Some(p) = r.project_id()
        && let Some(p) = lookup::project_by_id(w.tx(), p)?
    {
        lookup::ensure_active(&p)?;
    }
    Ok(())
}

fn create(w: &mut WriteCtx<'_>, input: DashboardCreateInput) -> Result<DashboardChange, Failure> {
    let name =
        gk_core::check_line("name", &input.name, NAME_MAX).map_err(|e| invalid("name", e))?;
    let project = match &input.project {
        Some(p) => {
            let p = lookup::project(w.tx(), "project", p)?;
            lookup::ensure_active(&p)?;
            Some(p.id)
        }
        None => None,
    };
    let description = match &input.description {
        Some(d) if !d.trim().is_empty() => gk_core::check_line("description", d, DESCRIPTION_MAX)
            .map_err(|e| invalid("description", e))?,
        _ => String::new(),
    };
    let mut specs = if input.starter { starter() } else { Vec::new() };
    specs.extend(input.widgets.iter().cloned());
    check_count(specs.len())?;
    let widgets = specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            new_widget(w, spec, project).map_err(|f| match f {
                Failure::Op(mut e) => {
                    // Point at the widget given, not at the starter's.
                    if !input.starter || i >= starter().len() {
                        let at = i - if input.starter { starter().len() } else { 0 };
                        let inner = e
                            .details
                            .get("path")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .unwrap_or_default();
                        let path = if inner.is_empty() {
                            format!("widgets[{at}]")
                        } else {
                            format!("widgets[{at}].{inner}")
                        };
                        e = e.with_detail("path", path);
                    }
                    Failure::Op(e)
                }
                other => other,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let caller = w.caller().actor.clone();
    let last = rows(w.tx())?
        .into_iter()
        .filter(|(_, r)| r.project_id() == project)
        .filter_map(|(_, r)| Rank::parse(&r.rank).ok())
        .max_by(|a, b| a.as_str().cmp(b.as_str()));
    let rank = last.map_or_else(Rank::first, |r| r.after());
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let record = Record {
        project: project.map(|p| p.to_string()),
        name,
        description,
        widgets,
        rank: rank.as_str().to_owned(),
        created_at: w.now(),
        created_by_kind: caller.kind.as_str().to_owned(),
        created_by_name: caller.name,
        updated_at: w.now(),
    };
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    change(w, id, &record, None, false)
}

fn update(w: &mut WriteCtx<'_>, input: DashboardUpdateInput) -> Result<DashboardChange, Failure> {
    let (id, old) = find(w.tx(), &input.dashboard, input.project.as_ref())?;
    ensure_open(w, &old)?;
    let clears_description = input.clear.contains(&DashboardField::Description);
    if input.description.is_some() && clears_description {
        return Err(messages::invalid(
            "clear",
            "field-given-and-cleared",
            &[("field", "description".to_owned())],
        ));
    }
    let mut r = old.clone();
    if let Some(name) = &input.name {
        r.name = gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))?;
    }
    if let Some(d) = &input.description {
        r.description = if d.trim().is_empty() {
            String::new()
        } else {
            gk_core::check_line("description", d, DESCRIPTION_MAX)
                .map_err(|e| invalid("description", e))?
        };
    }
    if clears_description {
        r.description.clear();
    }
    let unpins = input.clear.contains(&DashboardField::PinTo);
    if input.pin_to.is_some() && unpins {
        return Err(messages::invalid(
            "clear",
            "field-given-and-cleared",
            &[("field", "pin_to".to_owned())],
        ));
    }
    if let Some(p) = &input.pin_to {
        let p = lookup::project(w.tx(), "pin_to", p)?;
        lookup::ensure_active(&p)?;
        r.project = Some(p.id.to_string());
    }
    if unpins {
        r.project = None;
    }
    if r != old {
        r.updated_at = w.now();
        check_unique(w.tx(), id, &r)?;
        set(w, id, Some(&r))?;
    }
    change(w, id, &r, None, false)
}

fn delete(w: &mut WriteCtx<'_>, input: DashboardRef) -> Result<DashboardChange, Failure> {
    let (id, old) = find(w.tx(), &input.dashboard, input.project.as_ref())?;
    let out = change(w, id, &old, None, true)?;
    set(w, id, None)?;
    Ok(DashboardChange {
        group: w.group_if_changed(),
        ..out
    })
}

/// The index a 1-based `position` stands for among `len` places.
fn place(position: u32, len: usize) -> Result<usize, Failure> {
    let n = usize::try_from(position).unwrap_or(usize::MAX);
    if n == 0 || n > len {
        return Err(messages::invalid(
            "position",
            "dashboard-position",
            &[("max", len.to_string())],
        ));
    }
    Ok(n - 1)
}

fn add_widget(
    w: &mut WriteCtx<'_>,
    input: DashboardAddWidgetInput,
) -> Result<DashboardChange, Failure> {
    let (id, old) = find(w.tx(), &input.dashboard, input.project.as_ref())?;
    ensure_open(w, &old)?;
    check_count(old.widgets.len() + 1)?;
    let widget = new_widget(w, &input.widget, old.project_id()).map_err(|f| match f {
        Failure::Op(e) => {
            let inner = e
                .details
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default();
            let path = if inner.is_empty() {
                "widget".to_owned()
            } else {
                format!("widget.{inner}")
            };
            Failure::Op(e.with_detail("path", path))
        }
        other => other,
    })?;
    let mut r = old;
    let at = match input.position {
        Some(p) => place(p, r.widgets.len() + 1)?,
        None => r.widgets.len(),
    };
    r.widgets.insert(at, widget.clone());
    r.updated_at = w.now();
    set(w, id, Some(&r))?;
    change(w, id, &r, Some(&widget), false)
}

fn update_widget(
    w: &mut WriteCtx<'_>,
    input: DashboardUpdateWidgetInput,
) -> Result<DashboardChange, Failure> {
    let (id, old) = find(w.tx(), &input.dashboard, input.project.as_ref())?;
    ensure_open(w, &old)?;
    let at = widget_at(&old, &input.widget)?;
    let mut r = old.clone();
    let mut widget = r.widgets.remove(at);
    if let Some(title) = &input.title {
        widget.title = check_title(title)?;
    }
    if let Some(width) = input.width {
        widget.width = check_width(width)?;
    }
    if input.settings.is_some() || !input.clear.is_empty() {
        let given = input.settings.clone().unwrap_or_default();
        for setting in &input.clear {
            if is_set(*setting, &given) {
                return Err(messages::invalid(
                    "clear",
                    "field-given-and-cleared",
                    &[("field", setting.as_str().to_owned())],
                ));
            }
        }
        // The stored project is an ID; check it again as given.
        let mut merged = merge(&widget.settings, &given);
        for setting in &input.clear {
            clear(*setting, &mut merged);
        }
        widget.settings = check_settings(w, widget.kind, &merged, r.project_id())?;
    }
    let to = match input.position {
        Some(p) => place(p, r.widgets.len() + 1)?,
        None => at,
    };
    r.widgets.insert(to, widget.clone());
    if r != old {
        r.updated_at = w.now();
        set(w, id, Some(&r))?;
    }
    change(w, id, &r, Some(&widget), false)
}

fn remove_widget(
    w: &mut WriteCtx<'_>,
    input: DashboardRemoveWidgetInput,
) -> Result<DashboardChange, Failure> {
    let (id, old) = find(w.tx(), &input.dashboard, input.project.as_ref())?;
    ensure_open(w, &old)?;
    let at = widget_at(&old, &input.widget)?;
    let mut r = old;
    let widget = r.widgets.remove(at);
    r.updated_at = w.now();
    set(w, id, Some(&r))?;
    change(w, id, &r, Some(&widget), false)
}

/// Rewrites the queries of the widgets that look at `project` (through
/// their own setting or their dashboard's; every widget for `None`) that
/// `rewrite` changes, in the
/// running command, so each change shows in history and undoes with the
/// rest.
pub(crate) fn rewrite_queries(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    rewrite: &dyn Fn(&str) -> Option<String>,
) -> Result<(), Failure> {
    for (id, mut record) in rows(w.tx())? {
        let pinned = record.project_id();
        let mut touched = false;
        for widget in &mut record.widgets {
            // A widget looks at its own project, else at its dashboard's.
            let looks_at = match &widget.settings.project {
                Some(p) => p.parse::<Id>().ok(),
                None => pinned,
            };
            let Some(q) = widget.settings.query.as_deref() else {
                continue;
            };
            // A workspace-wide widget follows a project's change when its
            // query keeps it to that project.
            if let Some(p) = project
                && looks_at != Some(p)
                && !(looks_at.is_none() && super::lookup::query_limits_to(w.tx(), q, p)?)
            {
                continue;
            }
            if let Some(new) = rewrite(q) {
                widget.settings.query = Some(new);
                touched = true;
            }
        }
        if touched {
            record.updated_at = w.now();
            set(w, id, Some(&record))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Values

fn show(q: &QueryCtx<'_>, input: &DashboardRef) -> Result<DashboardShow, Failure> {
    let (id, record) = find(q.tx, &input.dashboard, input.project.as_ref())?;
    let dashboard = output(q.tx, id, &record)?;
    let widgets = dashboard
        .widgets
        .iter()
        .enumerate()
        .map(|(at, widget)| value(q, &record, at, widget.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DashboardShow { dashboard, widgets })
}

/// An empty value of `widget`.
fn blank(widget: Widget) -> WidgetValue {
    WidgetValue {
        widget,
        error: None,
        query: None,
        count: None,
        tasks: None,
        bars: None,
        created_closed: None,
        cumulative_flow: None,
        cycle_time: None,
        time: None,
        burndown: None,
        versions: None,
        activity: None,
        runs: None,
        text: None,
    }
}

/// What the widget at `at` of `record` shows now. A widget that its own
/// settings keep from showing (a project gone, a query that names a status
/// no longer there) has `error`; storage failures fail the call.
fn value(
    q: &QueryCtx<'_>,
    record: &Record,
    at: usize,
    widget: Widget,
) -> Result<WidgetValue, Failure> {
    let Some(stored) = record.widgets.get(at) else {
        return Ok(blank(widget));
    };
    let mut out = blank(widget);
    match compute(q, record, stored, &mut out) {
        Ok(()) => Ok(out),
        Err(Failure::Op(e)) => {
            let widget = out.widget;
            Ok(WidgetValue {
                error: Some(e),
                ..blank(widget)
            })
        }
        Err(other) => Err(other),
    }
}

/// The project a widget looks at, by key: its own, else its dashboard's.
fn scope_key(
    q: &QueryCtx<'_>,
    record: &Record,
    stored: &Stored,
) -> Result<Option<String>, Failure> {
    let id = match &stored.settings.project {
        Some(p) => Some(p.parse::<Id>().map_err(|_| project_gone(p))?),
        None => record.project_id(),
    };
    match id {
        Some(id) => match project_key(q.tx, Some(id))? {
            Some(key) => Ok(Some(key)),
            None => Err(project_gone(&id.to_string())),
        },
        None => Ok(None),
    }
}

fn project_gone(id: &str) -> Failure {
    messages::hint(
        messages::not_found("dashboard-project-gone", &[("project", id.to_owned())]),
        "dashboard-project-gone-hint",
        &[],
    )
    .into()
}

/// A value for a query: quoted when it is not a plain word.
fn quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if plain {
        return text.to_owned();
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Whether `query` has a top-level `OR`, so that adding conditions needs it
/// in parentheses.
fn has_or(query: &str) -> bool {
    query.split_whitespace().any(|w| w == "OR")
}

/// The widget's query with its project and `extra` conditions; `sort` is
/// added when the query has no order of its own.
fn combined(
    project: Option<&str>,
    query: Option<&str>,
    extra: &[&str],
    sort: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = project {
        parts.push(format!("project:{p}/*"));
    }
    let query = query.map(str::trim).filter(|q| !q.is_empty());
    let sorted = query.is_some_and(|q| q.contains("sort:"));
    if let Some(q) = query {
        if has_or(q) && !sorted && (project.is_some() || !extra.is_empty()) {
            parts.push(format!("({q})"));
        } else {
            parts.push(q.to_owned());
        }
    }
    parts.extend(extra.iter().map(|e| (*e).to_owned()));
    if let Some(s) = sort
        && !sorted
    {
        parts.push(format!("sort:{s}"));
    }
    parts.join(" ")
}

fn limit_of(stored: &Stored) -> u32 {
    stored.settings.limit.unwrap_or(LIMIT_DEFAULT)
}

fn days_of(stored: &Stored) -> u32 {
    stored.settings.days.unwrap_or(DAYS_DEFAULT)
}

/// Runs `query` and fills the value's count, tasks and normal query.
fn tasks(
    q: &QueryCtx<'_>,
    query: String,
    limit: u32,
    out: &mut WidgetValue,
) -> Result<(), Failure> {
    let found = super::query::search(
        q,
        TaskSearchInput {
            query,
            limit: Some(limit),
            ..TaskSearchInput::default()
        },
    )?;
    out.count = Some(found.total);
    out.query = Some(found.query);
    out.tasks = Some(
        found
            .items
            .into_iter()
            .map(|t| WidgetTask {
                key: t.key,
                title: t.title,
                status: t.status,
                priority: t.priority,
                due_date: t.due_date,
            })
            .collect(),
    );
    Ok(())
}

fn flow_input(stored: &Stored, project: Option<String>) -> FlowReportInput {
    FlowReportInput {
        bucket: stored.settings.bucket,
        include_subprojects: project.as_ref().map(|_| true),
        project,
        query: stored
            .settings
            .query
            .clone()
            .filter(|q| !q.trim().is_empty()),
        ..FlowReportInput::default()
    }
}

fn compute(
    q: &QueryCtx<'_>,
    record: &Record,
    stored: &Stored,
    out: &mut WidgetValue,
) -> Result<(), Failure> {
    let s = &stored.settings;
    match stored.kind {
        WidgetKind::Note => {
            out.text = Some(s.text.clone().unwrap_or_default());
        }
        WidgetKind::QueryCount => {
            let key = scope_key(q, record, stored)?;
            let query = combined(key.as_deref(), s.query.as_deref(), &[], None);
            tasks(q, query, 1, out)?;
            out.tasks = None;
        }
        WidgetKind::QueryList => {
            let key = scope_key(q, record, stored)?;
            let query = combined(key.as_deref(), s.query.as_deref(), &[], None);
            tasks(q, query, limit_of(stored), out)?;
        }
        WidgetKind::DueSoon => {
            let key = scope_key(q, record, stored)?;
            let until = format!("due<=+{}d", days_of(stored));
            let query = combined(
                key.as_deref(),
                s.query.as_deref(),
                &["is:open", "due>=today", &until],
                Some("due,-priority"),
            );
            tasks(q, query, limit_of(stored), out)?;
        }
        WidgetKind::Overdue => {
            let key = scope_key(q, record, stored)?;
            let query = combined(
                key.as_deref(),
                s.query.as_deref(),
                &["is:open", "due<today"],
                Some("due,-priority"),
            );
            tasks(q, query, limit_of(stored), out)?;
        }
        WidgetKind::TasksByField => by_field(q, record, stored, out)?,
        WidgetKind::CreatedClosed => {
            let input = flow_input(stored, scope_key(q, record, stored)?);
            out.created_closed = Some(super::report::created_closed_report(q, &input)?);
        }
        WidgetKind::CumulativeFlow => {
            let input = flow_input(stored, scope_key(q, record, stored)?);
            out.cumulative_flow = Some(super::report::cumulative_flow_report(q, &input)?);
        }
        WidgetKind::CycleTime => {
            let input = FlowReportInput {
                tasks: true,
                ..flow_input(stored, scope_key(q, record, stored)?)
            };
            out.cycle_time = Some(super::report::cycle_time_report(q, &input)?);
        }
        WidgetKind::TimeReport => {
            let today = q.env.zone.day_of(q.now);
            let back = i64::from(days_of(stored)) - 1;
            let from = today
                .add_days(-back)
                .map_err(|e| invalid("settings.days", e))?;
            let project = scope_key(q, record, stored)?;
            out.time = Some(super::time::report(
                q.tx,
                &q.env.zone,
                q.now,
                TimeReportInput {
                    from: Some(from.to_string()),
                    to: Some(today.to_string()),
                    include_subprojects: project.as_ref().map(|_| true),
                    project,
                    group_by: Some(s.group_by.unwrap_or(TimeGroupBy::Project)),
                    ..TimeReportInput::default()
                },
            )?);
        }
        WidgetKind::Burndown => {
            let Some(project) = scope_key(q, record, stored)? else {
                return Err(messages::invalid(
                    "settings.project",
                    "dashboard-needs-project",
                    &[("kind", stored.kind.as_str().to_owned())],
                ));
            };
            out.burndown = Some(super::sprint::burndown(
                q,
                &SprintBurndownInput {
                    sprint: "active".to_owned(),
                    project: Some(project),
                },
            )?);
        }
        WidgetKind::VersionProgress => {
            let Some(project) = scope_key(q, record, stored)? else {
                return Err(messages::invalid(
                    "settings.project",
                    "dashboard-needs-project",
                    &[("kind", stored.kind.as_str().to_owned())],
                ));
            };
            let versions = super::version::list(
                q.tx,
                &VersionListInput {
                    project,
                    status: vec![VersionStatus::Open, VersionStatus::Locked],
                    ..VersionListInput::default()
                },
            )?;
            out.versions = Some(versions.items);
        }
        WidgetKind::Activity => {
            let page = super::history::feed(
                q,
                ActivityQuery {
                    project: scope_key(q, record, stored)?,
                    actor_kind: s.actor_kind,
                    from: s.days.map(|d| format!("-{}d", d.saturating_sub(1))),
                    limit: Some(limit_of(stored)),
                    ..ActivityQuery::default()
                },
            )?;
            out.count = Some(page.total);
            out.activity = Some(page.items);
        }
        WidgetKind::AutomationFailures => {
            let runs = super::automation::runs(
                q.tx,
                &AutomationRunsInput {
                    project: scope_key(q, record, stored)?,
                    problems_only: true,
                    limit: Some(limit_of(stored)),
                    ..AutomationRunsInput::default()
                },
            )?;
            out.count = Some(runs.total);
            out.runs = Some(runs.items);
        }
    }
    Ok(())
}

/// Open tasks (or the widget's query) counted by a field, one bar per
/// value in the field's order, each with the query that lists its tasks.
fn by_field(
    q: &QueryCtx<'_>,
    record: &Record,
    stored: &Stored,
    out: &mut WidgetValue,
) -> Result<(), Failure> {
    let s = &stored.settings;
    let key = scope_key(q, record, stored)?;
    let field = s.field.clone().unwrap_or_else(|| "status".to_owned());
    let base_query = s
        .query
        .clone()
        .filter(|q| !q.trim().is_empty())
        .unwrap_or_else(|| "is:open".to_owned());
    let query = combined(key.as_deref(), Some(&base_query), &[], None);
    let (group_by, group_by_field) = match field.as_str() {
        "status" => (Some(ListGrouping::Status), None),
        "priority" => (Some(ListGrouping::Priority), None),
        "type" => (Some(ListGrouping::Type), None),
        "label" => (Some(ListGrouping::Label), None),
        other => (None, Some(other.to_owned())),
    };
    let found = super::query::search(
        q,
        TaskSearchInput {
            query: query.clone(),
            limit: Some(1),
            group_by,
            group_by_field: group_by_field.clone(),
            ..TaskSearchInput::default()
        },
    )?;
    let term = |value: &str| -> String {
        match field.as_str() {
            "status" | "priority" | "type" | "label" => format!("{field}:{}", quote(value)),
            custom => {
                let bare = gk_query::fields::strip_custom_prefix(custom).unwrap_or(custom);
                format!("cf.{bare}:{}", quote(value))
            }
        }
    };
    out.count = Some(found.total);
    out.query = Some(found.query);
    out.bars = Some(
        found
            .groups
            .into_iter()
            .map(|g| {
                let query = match (&g.value, field.as_str()) {
                    (Some(v), _) => Some(combined(None, Some(&query), &[&term(v)], None)),
                    (None, "label") => Some(combined(None, Some(&query), &["-has:labels"], None)),
                    (None, _) => None,
                };
                WidgetBar {
                    value: g.value,
                    hue: g.hue,
                    count: g.count,
                    query,
                }
            })
            .collect(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_combined_with_their_project_and_order() {
        assert_eq!(
            combined(Some("TM"), Some("label:bug"), &["is:open"], Some("due")),
            "project:TM/* label:bug is:open sort:due"
        );
        assert_eq!(
            combined(Some("TM"), Some("a OR b"), &[], None),
            "project:TM/* (a OR b)"
        );
        assert_eq!(
            combined(
                None,
                Some("is:open sort:-priority"),
                &["due<today"],
                Some("due")
            ),
            "is:open sort:-priority due<today"
        );
        assert_eq!(combined(None, None, &[], None), "");
    }

    #[test]
    fn values_are_quoted_when_they_are_not_one_word() {
        assert_eq!(quote("Done"), "Done");
        assert_eq!(quote("In progress"), "\"In progress\"");
        assert_eq!(quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn every_kind_takes_its_own_settings_only() {
        for kind in WidgetKind::ALL {
            let wanted = kind.settings();
            assert!(!wanted.is_empty(), "{kind:?}");
            assert_eq!(
                kind == WidgetKind::Note,
                wanted.contains(&WidgetSetting::Text)
            );
        }
    }
}
