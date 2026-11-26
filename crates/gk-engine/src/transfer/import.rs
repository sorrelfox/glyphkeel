// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Importing an archive: read and check everything, plan keys and IDs,
//! copy the stored files, write the records in one command.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use gk_api::ops::transfer::{
    ArchiveCounts, ImportInput, ImportResult, ImportedProject, TransferPhase,
};
use gk_api::{OpError, descriptor};
use gk_core::{Id, ProjectKey, Rank, fold};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::model::{
    Activity, AgentSession, Attachment, Comment, Manifest, ProjectFile, Revision, Task, TimeEntry,
    WikiPage,
};
use super::source::{Source, safe_path};
use super::{
    FORMAT_NAME, FORMAT_VERSION, Limits, Progress, Rejected, plain_project_dir, project_dir, report,
};
use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::ops::fields::PROJECT;
use crate::ops::links::{self, Source as LinkSource};
use crate::registry::CommandFn;
use crate::write::WriteCtx;

const MANIFEST: &str = "manifest.json";
const SESSIONS: &str = "agent_sessions.jsonl";
const FIXTURES: &str = "fixtures.json";
/// Records per progress report.
const STEP: u64 = 1000;
/// How deep tasks nest below a top-level task.
const MAX_TASK_DEPTH: usize = 5;
/// How deep wiki pages nest.
const MAX_PAGE_DEPTH: usize = 8;

/// One project of a checked archive.
#[derive(Debug)]
pub(crate) struct Proj {
    pub(crate) file: ProjectFile,
    pub(crate) tasks: Vec<Task>,
    /// Task descriptions by task number.
    pub(crate) descriptions: HashMap<i64, String>,
    pub(crate) comments: Vec<Comment>,
    pub(crate) time: Vec<TimeEntry>,
    pub(crate) revisions: Vec<Revision>,
    pub(crate) wiki: Vec<WikiPage>,
    pub(crate) files: Vec<Attachment>,
    pub(crate) activity: Vec<Activity>,
}

/// A checked archive.
#[derive(Debug)]
pub(crate) struct Parsed {
    pub(crate) manifest: Manifest,
    pub(crate) sessions: Vec<AgentSession>,
    /// Parents before their sub-projects.
    pub(crate) projects: Vec<Proj>,
    /// Stored files by SHA-256, with their sizes.
    pub(crate) blobs: BTreeMap<String, u64>,
}

/// What an entry of the archive is.
enum Class {
    Manifest,
    Sessions,
    Fixtures,
    Blob(String),
    ProjectJson,
    Jsonl,
    TaskMarkdown,
    WikiMarkdown,
}

const PROJECT_FILES: [&str; 8] = [
    "project.json",
    "tasks.jsonl",
    "comments.jsonl",
    "time.jsonl",
    "revisions.jsonl",
    "wiki.jsonl",
    "files.jsonl",
    "activity.jsonl",
];

fn is_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `keys` maps each project folder's name to its project's key.
fn classify(path: &str, keys: &HashMap<String, String>) -> Option<Class> {
    match path {
        MANIFEST => return Some(Class::Manifest),
        SESSIONS => return Some(Class::Sessions),
        FIXTURES => return Some(Class::Fixtures),
        _ => {}
    }
    if let Some(sha) = path.strip_prefix("files/") {
        return is_hash(sha).then(|| Class::Blob(sha.to_owned()));
    }
    let rest = path.strip_prefix("projects/")?;
    let (folder, inner) = rest.split_once('/')?;
    let key = keys.get(folder)?.as_str();
    if inner == "project.json" {
        return Some(Class::ProjectJson);
    }
    if PROJECT_FILES.contains(&inner) {
        return Some(Class::Jsonl);
    }
    if let Some(name) = inner.strip_prefix("tasks/") {
        let number = name
            .strip_prefix(key)
            .and_then(|n| n.strip_prefix('-'))
            .and_then(|n| n.strip_suffix(".md"))?;
        return number.parse::<u32>().is_ok().then_some(Class::TaskMarkdown);
    }
    if inner.starts_with("wiki/") && inner.ends_with(".md") {
        return Some(Class::WikiMarkdown);
    }
    None
}

fn lines<T: DeserializeOwned>(
    path: &str,
    bytes: &[u8],
    limits: &Limits,
    records: &mut u64,
) -> Result<Vec<T>, Rejected> {
    let mut out = Vec::new();
    for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        if line.len() > limits.max_line {
            return Err(Rejected::LineTooLong(path.to_owned(), i + 1));
        }
        let value = serde_json::from_slice(line)
            .map_err(|e| Rejected::Json(path.to_owned(), i + 1, e.to_string()))?;
        out.push(value);
        *records += 1;
        if *records > limits.max_records {
            return Err(Rejected::TooManyRecords(limits.max_records));
        }
    }
    Ok(out)
}

/// Reads and checks the whole archive.
pub(crate) fn read(
    source: &mut Source,
    limits: &Limits,
    progress: &Progress,
) -> Result<Parsed, Rejected> {
    let manifest_bytes = source.read(MANIFEST, limits.max_manifest)?;
    let raw: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| Rejected::Json(MANIFEST.into(), e.line(), e.to_string()))?;
    // The format and version first, so a newer archive says so instead of
    // failing on a field this version doesn't know.
    let format = raw.get("format").and_then(Value::as_str).unwrap_or("");
    if format != FORMAT_NAME {
        return Err(Rejected::Format(format.chars().take(64).collect()));
    }
    let version = raw
        .get("format_version")
        .and_then(Value::as_u64)
        .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX));
    if version != FORMAT_VERSION {
        return Err(Rejected::Version(version));
    }
    let manifest: Manifest = serde_json::from_value(raw)
        .map_err(|e| Rejected::Json(MANIFEST.into(), 1, e.to_string()))?;

    let mut keys = HashMap::new();
    let mut seen = HashSet::new();
    for p in &manifest.projects {
        let dir = project_dir(&p.key);
        if ProjectKey::new(&p.key).is_err()
            || (p.path != dir && p.path != plain_project_dir(&p.key))
            || !safe_path(&p.path)
        {
            return Err(Rejected::BadPath(p.path.clone()));
        }
        if !seen.insert(p.key.clone()) {
            return Err(Rejected::Duplicate(dir));
        }
        let folder = p.path.trim_start_matches("projects/").to_owned();
        keys.insert(folder, p.key.clone());
    }
    let listing: Vec<(String, u64)> = source
        .entries()
        .iter()
        .map(|(p, e)| (p.clone(), e.size))
        .collect();
    for (path, _) in &listing {
        if path != MANIFEST && !manifest.hashes.contains_key(path) {
            return Err(Rejected::Unlisted(path.clone()));
        }
    }
    for path in manifest.hashes.keys() {
        if !safe_path(path) {
            return Err(Rejected::BadPath(path.clone()));
        }
        if !source.entries().contains_key(path) {
            return Err(Rejected::Missing(path.clone()));
        }
    }

    // Every entry: its place in the layout, its size and its hash.
    let total = listing.len() as u64;
    let mut texts: HashMap<String, Vec<u8>> = HashMap::new();
    let mut blobs = BTreeMap::new();
    for (i, (path, _)) in listing.iter().enumerate() {
        if (i as u64).is_multiple_of(STEP) {
            report(progress, TransferPhase::Checking, i as u64, total);
        }
        let class = classify(path, &keys).ok_or_else(|| Rejected::Unexpected(path.clone()))?;
        let expected = manifest.hashes.get(path.as_str());
        match class {
            Class::Manifest => {}
            Class::Blob(sha) => {
                let mut reader = source.open_entry(path, limits.max_file)?;
                let found = gk_store::sha256_reader(&mut reader)
                    .map_err(|e| Rejected::Unreadable(format!("{path}: {e}")))?;
                reader.finish()?;
                if found.sha256 != sha || expected != Some(&sha) {
                    return Err(Rejected::Hash(path.clone()));
                }
                blobs.insert(sha, found.size);
            }
            other => {
                let limit = match other {
                    Class::TaskMarkdown | Class::WikiMarkdown => limits.max_markdown,
                    _ => limits.max_json,
                };
                let bytes = source.read(path, limit)?;
                if expected.map(String::as_str) != Some(gk_store::sha256_hex(&bytes).as_str()) {
                    return Err(Rejected::Hash(path.clone()));
                }
                if !matches!(other, Class::WikiMarkdown | Class::Fixtures) {
                    texts.insert(path.clone(), bytes);
                }
            }
        }
    }
    report(progress, TransferPhase::Checking, total, total);

    let mut records = 0u64;
    let sessions = match texts.remove(SESSIONS) {
        Some(bytes) => lines(SESSIONS, &bytes, limits, &mut records)?,
        None => Vec::new(),
    };
    let mut projects = Vec::new();
    for mp in &manifest.projects {
        // The folder the manifest names: checked to be the key's.
        let dir = mp.path.clone();
        let file_path = format!("{dir}/project.json");
        let bytes = texts
            .remove(&file_path)
            .ok_or_else(|| Rejected::Missing(file_path.clone()))?;
        let file: ProjectFile = serde_json::from_slice(&bytes)
            .map_err(|e| Rejected::Json(file_path.clone(), e.line(), e.to_string()))?;
        if file.project.id != mp.id || file.project.key != mp.key {
            return Err(bad(file_path, 1, "transfer-invalid-manifest-project"));
        }
        let mut list =
            |name: &str| -> Vec<u8> { texts.remove(&format!("{dir}/{name}")).unwrap_or_default() };
        let (t, c, ti, r, w, f, a) = (
            list("tasks.jsonl"),
            list("comments.jsonl"),
            list("time.jsonl"),
            list("revisions.jsonl"),
            list("wiki.jsonl"),
            list("files.jsonl"),
            list("activity.jsonl"),
        );
        let mut proj = Proj {
            file,
            tasks: lines(&format!("{dir}/tasks.jsonl"), &t, limits, &mut records)?,
            descriptions: HashMap::new(),
            comments: lines(&format!("{dir}/comments.jsonl"), &c, limits, &mut records)?,
            time: lines(&format!("{dir}/time.jsonl"), &ti, limits, &mut records)?,
            revisions: lines(&format!("{dir}/revisions.jsonl"), &r, limits, &mut records)?,
            wiki: lines(&format!("{dir}/wiki.jsonl"), &w, limits, &mut records)?,
            files: lines(&format!("{dir}/files.jsonl"), &f, limits, &mut records)?,
            activity: lines(&format!("{dir}/activity.jsonl"), &a, limits, &mut records)?,
        };
        for task in &proj.tasks {
            let md = format!("{dir}/tasks/{}-{}.md", mp.key, task.number);
            if let Some(bytes) = texts.remove(&md) {
                let text = String::from_utf8(bytes)
                    .map_err(|_| bad(md.clone(), 1, "transfer-invalid-utf8"))?;
                proj.descriptions.insert(task.number, text);
            }
        }
        projects.push(proj);
    }
    if let Some(left) = texts.keys().find(|k| k.contains("/tasks/")) {
        return Err(bad(left.clone(), 1, "transfer-invalid-stray-description"));
    }
    let mut parsed = Parsed {
        manifest,
        sessions,
        projects,
        blobs,
    };
    check(&mut parsed)?;
    Ok(parsed)
}

/// Record `line` of `path` is invalid, as message `id` says.
fn bad(path: String, line: usize, id: &'static str) -> Rejected {
    Rejected::Invalid(path, line, id, Vec::new())
}

fn color(path: &str, line: usize, value: &str) -> Rejected {
    bad_with(
        path.to_owned(),
        line,
        "transfer-invalid-color",
        &[("value", value)],
    )
}

/// [`bad`] with the message's own arguments.
fn bad_with(
    path: String,
    line: usize,
    id: &'static str,
    args: &[(&'static str, &str)],
) -> Rejected {
    let args = args.iter().map(|(k, v)| (*k, (*v).to_owned())).collect();
    Rejected::Invalid(path, line, id, args)
}

fn id_of(path: &str, line: usize, text: &str) -> Result<Id, Rejected> {
    text.parse().map_err(|_| {
        bad_with(
            path.to_owned(),
            line,
            "transfer-invalid-id",
            &[("value", text)],
        )
    })
}

fn rank_ok(path: &str, line: usize, rank: &str) -> Result<(), Rejected> {
    Rank::parse(rank).map(drop).map_err(|_| {
        bad_with(
            path.to_owned(),
            line,
            "transfer-invalid-rank",
            &[("value", rank)],
        )
    })
}

/// The references between records, sizes and enumerations the schema
/// doesn't already enforce, and the counts of the manifest.
fn check(parsed: &mut Parsed) -> Result<(), Rejected> {
    let mut ids: HashSet<Id> = HashSet::new();
    let mut unique = |path: &str, line: usize, text: &str| -> Result<Id, Rejected> {
        let id = id_of(path, line, text)?;
        if ids.insert(id) {
            Ok(id)
        } else {
            Err(bad_with(
                path.to_owned(),
                line,
                "transfer-invalid-id-twice",
                &[("id", text)],
            ))
        }
    };
    for (i, s) in parsed.sessions.iter().enumerate() {
        unique(SESSIONS, i + 1, &s.id)?;
    }
    let archive_projects: HashSet<String> = parsed
        .projects
        .iter()
        .map(|p| p.file.project.id.clone())
        .collect();
    let all_tasks: HashSet<String> = parsed
        .projects
        .iter()
        .flat_map(|p| p.tasks.iter().map(|t| t.id.clone()))
        .collect();
    let mut counts = ArchiveCounts::default();
    for proj in &parsed.projects {
        let key = proj.file.project.key.clone();
        let dir = project_dir(&key);
        let pj = format!("{dir}/project.json");
        let p = &proj.file.project;
        let project_id = p.id.clone();
        unique(&pj, 1, &p.id)?;
        rank_ok(&pj, 1, &p.rank)?;
        if p.name.trim().is_empty() || !matches!(p.state.as_str(), "active" | "archived") {
            return Err(bad(pj, 1, "transfer-invalid-project"));
        }
        gk_core::Hue::from_str(&p.hue).map_err(|_| color(&pj, 1, &p.hue))?;
        if let Some(icon) = &p.icon {
            gk_core::ProjectIcon::from_str(icon).map_err(|_| {
                bad_with(pj.clone(), 1, "transfer-invalid-icon", &[("value", icon)])
            })?;
        }
        if let Some(parent) = &p.parent_id {
            id_of(&pj, 1, parent)?;
        }
        let mut statuses = HashSet::new();
        for s in &proj.file.statuses {
            unique(&pj, 1, &s.id)?;
            rank_ok(&pj, 1, &s.rank)?;
            gk_core::Hue::from_str(&s.hue).map_err(|_| color(&pj, 1, &s.hue))?;
            statuses.insert(s.id.clone());
        }
        let mut types = HashSet::new();
        for t in &proj.file.types {
            unique(&pj, 1, &t.id)?;
            rank_ok(&pj, 1, &t.rank)?;
            gk_core::Hue::from_str(&t.hue).map_err(|_| color(&pj, 1, &t.hue))?;
            types.insert(t.id.clone());
        }
        if statuses.is_empty() || types.is_empty() {
            return Err(bad(pj, 1, "transfer-invalid-no-statuses"));
        }
        let mut labels = HashSet::new();
        for l in &proj.file.labels {
            unique(&pj, 1, &l.id)?;
            rank_ok(&pj, 1, &l.rank)?;
            gk_core::Hue::from_str(&l.hue).map_err(|_| color(&pj, 1, &l.hue))?;
            labels.insert(l.id.clone());
        }
        let mut version_names = HashSet::new();
        for v in &proj.file.versions {
            unique(&pj, 1, &v.id)?;
            rank_ok(&pj, 1, &v.rank)?;
            let dates_ok = [&v.start_date, &v.due_date].iter().all(|d| {
                d.as_deref()
                    .is_none_or(|d| d.parse::<gk_core::Day>().is_ok())
            }) && (v.start_date.is_none()
                || v.due_date.is_none()
                || v.start_date <= v.due_date);
            if v.name.trim().is_empty()
                || v.name.chars().count() > 100
                || v.description.chars().count() > 4000
                || !matches!(v.status.as_str(), "open" | "locked" | "closed")
                || !matches!(v.sharing.as_str(), "none" | "descendants")
                || !dates_ok
                || !version_names.insert(fold(&v.name))
            {
                return Err(bad_with(
                    pj.clone(),
                    1,
                    "transfer-invalid-version",
                    &[("version", &v.name)],
                ));
            }
        }
        for v in &proj.file.views {
            unique(&pj, 1, &v.id)?;
            rank_ok(&pj, 1, &v.rank)?;
        }
        let tp = format!("{dir}/tasks.jsonl");
        super::custom_fields::check(&pj, &tp, &proj.file.custom_fields, &proj.tasks)?;
        let mut numbers = HashSet::new();
        let mut parents: HashMap<String, Option<String>> = HashMap::new();
        for (i, t) in proj.tasks.iter().enumerate() {
            let line = i + 1;
            unique(&tp, line, &t.id)?;
            if t.project_id != project_id {
                return Err(bad(tp, line, "transfer-invalid-task-project"));
            }
            if t.number < 1 || !numbers.insert(t.number) {
                return Err(bad_with(
                    tp,
                    line,
                    "transfer-invalid-task-number",
                    &[("number", &t.number.to_string())],
                ));
            }
            if !statuses.contains(&t.status_id) || !types.contains(&t.type_id) {
                return Err(bad(tp, line, "transfer-invalid-task-status"));
            }
            if let Some(l) = t.labels.iter().find(|l| !labels.contains(*l)) {
                return Err(bad_with(
                    tp,
                    line,
                    "transfer-invalid-task-label",
                    &[("label", l)],
                ));
            }
            gk_core::check_title(&t.title)
                .map_err(|_| bad(tp.clone(), line, "transfer-invalid-title"))?;
            rank_ok(&tp, line, &t.rank)?;
            if let Some(r) = &t.subtask_rank {
                rank_ok(&tp, line, r)?;
            }
            if let Some(d) = proj.descriptions.get(&t.number)
                && d.len() > 1 << 20
            {
                return Err(bad(tp, line, "transfer-invalid-description-size"));
            }
            for r in &t.relations {
                unique(&tp, line, &r.id)?;
                if !all_tasks.contains(&r.to_task) || r.to_task == t.id {
                    return Err(bad(tp, line, "transfer-invalid-link-target"));
                }
                if !matches!(
                    r.kind.as_str(),
                    "blocks" | "relates" | "duplicates" | "clones"
                ) {
                    return Err(bad_with(
                        tp,
                        line,
                        "transfer-invalid-link-kind",
                        &[("kind", &r.kind)],
                    ));
                }
            }
            if let Some(c) = &t.claim {
                id_of(&tp, line, &c.session_id)?;
            }
            parents.insert(t.id.clone(), t.parent_id.clone());
            counts.tasks += 1;
        }
        // Parents in the same project, no cycles, at most five levels.
        for (i, t) in proj.tasks.iter().enumerate() {
            let mut depth = 0;
            let mut at = t.parent_id.clone();
            while let Some(p) = at {
                depth += 1;
                if depth > MAX_TASK_DEPTH || p == t.id {
                    return Err(bad(tp, i + 1, "transfer-invalid-task-nesting"));
                }
                at = parents
                    .get(&p)
                    .ok_or_else(|| bad(tp.clone(), i + 1, "transfer-invalid-task-parent"))?
                    .clone();
            }
        }
        let task_ids: HashSet<&String> = proj.tasks.iter().map(|t| &t.id).collect();
        let cp = format!("{dir}/comments.jsonl");
        let mut comment_task: HashMap<&String, &String> = HashMap::new();
        for (i, c) in proj.comments.iter().enumerate() {
            unique(&cp, i + 1, &c.id)?;
            if !task_ids.contains(&c.task_id) {
                return Err(bad(cp, i + 1, "transfer-invalid-comment-task"));
            }
            for r in &c.revisions {
                unique(&cp, i + 1, &r.id)?;
            }
            comment_task.insert(&c.id, &c.task_id);
            counts.comments += 1;
        }
        for (i, c) in proj.comments.iter().enumerate() {
            if let Some(r) = &c.reply_to
                && comment_task.get(r) != Some(&&c.task_id)
            {
                return Err(bad(cp, i + 1, "transfer-invalid-comment-reply"));
            }
        }
        let ep = format!("{dir}/time.jsonl");
        for (i, e) in proj.time.iter().enumerate() {
            unique(&ep, i + 1, &e.id)?;
            if !task_ids.contains(&e.task_id) {
                return Err(bad(ep, i + 1, "transfer-invalid-time-task"));
            }
            counts.time_entries += 1;
        }
        let wp = format!("{dir}/wiki.jsonl");
        let page_ids: HashMap<&String, Option<&String>> = proj
            .wiki
            .iter()
            .map(|w| (&w.id, w.parent_id.as_ref()))
            .collect();
        let mut briefs = 0;
        let mut homes = 0;
        for (i, w) in proj.wiki.iter().enumerate() {
            unique(&wp, i + 1, &w.id)?;
            if w.is_agent_brief {
                briefs += 1;
                if briefs > 1 {
                    return Err(bad(wp, i + 1, "transfer-invalid-two-briefs"));
                }
            }
            if w.is_home && w.deleted_at.is_none() {
                homes += 1;
                if homes > 1 {
                    return Err(bad(wp, i + 1, "transfer-invalid-two-homes"));
                }
            }
            if w.project_id != project_id {
                return Err(bad(wp, i + 1, "transfer-invalid-page-project"));
            }
            gk_core::check_page_title(&w.title)
                .map_err(|_| bad(wp.clone(), i + 1, "transfer-invalid-page-title"))?;
            rank_ok(&wp, i + 1, &w.rank)?;
            if w.slug.is_empty() || w.slug.len() > 200 {
                return Err(bad(wp, i + 1, "transfer-invalid-page-slug"));
            }
            let mut depth = 0;
            let mut at = w.parent_id.as_ref();
            while let Some(p) = at {
                depth += 1;
                if depth > MAX_PAGE_DEPTH || *p == w.id {
                    return Err(bad(wp, i + 1, "transfer-invalid-page-nesting"));
                }
                at = *page_ids
                    .get(p)
                    .ok_or_else(|| bad(wp.clone(), i + 1, "transfer-invalid-page-parent"))?;
            }
            counts.wiki_pages += 1;
        }
        let rp = format!("{dir}/revisions.jsonl");
        for (i, r) in proj.revisions.iter().enumerate() {
            unique(&rp, i + 1, &r.id)?;
            let owner_ok = match r.owner_kind.as_str() {
                "task" => task_ids.contains(&r.owner_id),
                "project" => r.owner_id == project_id,
                "wiki_page" => page_ids.contains_key(&r.owner_id),
                _ => false,
            };
            if !owner_ok {
                return Err(bad(rp, i + 1, "transfer-invalid-revision-owner"));
            }
            if r.body.len() > 1 << 20 {
                return Err(bad(rp, i + 1, "transfer-invalid-revision-size"));
            }
            counts.revisions += 1;
        }
        let fp = format!("{dir}/files.jsonl");
        for (i, a) in proj.files.iter().enumerate() {
            unique(&fp, i + 1, &a.id)?;
            let owner_ok = match a.owner_kind.as_str() {
                "task" => task_ids.contains(&a.owner_id),
                "comment" => comment_task.contains_key(&a.owner_id),
                "wiki_page" => page_ids.contains_key(&a.owner_id),
                "project" => a.owner_id == project_id,
                _ => false,
            };
            if !owner_ok {
                return Err(bad(fp, i + 1, "transfer-invalid-file-owner"));
            }
            if parsed.blobs.get(&a.sha256) != Some(&a.size) {
                return Err(bad(fp, i + 1, "transfer-invalid-file-missing"));
            }
            if a.file_name.is_empty() || a.file_name.len() > 1024 {
                return Err(bad(fp, i + 1, "transfer-invalid-file-name"));
            }
            counts.attachments += 1;
        }
        let ap = format!("{dir}/activity.jsonl");
        for (i, a) in proj.activity.iter().enumerate() {
            id_of(&ap, i + 1, &a.group_id)?;
            id_of(&ap, i + 1, &a.entity_id)?;
            counts.activity += 1;
        }
        counts.projects += 1;
        if let Some(parent) = &p.parent_id
            && archive_projects.contains(parent)
            && *parent == project_id
        {
            return Err(bad(pj, 1, "transfer-invalid-project-parent"));
        }
    }
    counts.agent_sessions = parsed.sessions.len() as u64;
    counts.files = parsed.blobs.len() as u64;
    let said = &parsed.manifest.counts;
    for (what, said, found) in [
        ("projects", said.projects, counts.projects),
        ("tasks", said.tasks, counts.tasks),
        ("comments", said.comments, counts.comments),
        ("time_entries", said.time_entries, counts.time_entries),
        ("revisions", said.revisions, counts.revisions),
        ("wiki_pages", said.wiki_pages, counts.wiki_pages),
        ("attachments", said.attachments, counts.attachments),
        ("blobs", said.blobs, counts.files),
        ("activity", said.activity, counts.activity),
        ("agent_sessions", said.agent_sessions, counts.agent_sessions),
    ] {
        if said != found {
            return Err(Rejected::Counts(what.into(), said, found));
        }
    }
    sort_projects(parsed)
}

/// Puts parents before their sub-projects.
fn sort_projects(parsed: &mut Parsed) -> Result<(), Rejected> {
    let mut left: Vec<Proj> = std::mem::take(&mut parsed.projects);
    let ids: HashSet<String> = left.iter().map(|p| p.file.project.id.clone()).collect();
    let mut placed: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let before = left.len();
        let mut rest = Vec::new();
        for p in left {
            let ready = match &p.file.project.parent_id {
                Some(parent) if ids.contains(parent) => placed.contains(parent),
                _ => true,
            };
            if ready {
                placed.insert(p.file.project.id.clone());
                out.push(p);
            } else {
                rest.push(p);
            }
        }
        if rest.len() == before {
            let key = rest[0].file.project.key.clone();
            return Err(bad(
                format!("{}/project.json", project_dir(&key)),
                1,
                "transfer-invalid-project-nesting",
            ));
        }
        left = rest;
    }
    parsed.projects = out;
    Ok(())
}

// ---------------------------------------------------------------------
// Planning: keys and IDs in this workspace

/// The keys and IDs the records get in this workspace.
#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    /// The key of each project, in the order of [`Parsed::projects`].
    pub(crate) keys: Vec<String>,
    /// Archive keys that are taken here and have no new key.
    pub(crate) taken: Vec<String>,
    /// IDs that exist here already, with their replacements.
    pub(crate) ids: HashMap<Id, Id>,
    /// History groups that exist here already, with their replacements.
    pub(crate) groups: HashMap<Id, Id>,
}

impl Plan {
    fn id(&self, text: &str) -> Id {
        let id: Id = text.parse().unwrap_or_else(|_| Id::generate());
        self.ids.get(&id).copied().unwrap_or(id)
    }

    fn opt(&self, text: Option<&String>) -> Option<Vec<u8>> {
        text.map(|t| self.id(t).as_bytes().to_vec())
    }

    fn group(&self, text: Option<&String>) -> Option<Vec<u8>> {
        let id: Id = text?.parse().ok()?;
        Some(
            self.groups
                .get(&id)
                .copied()
                .unwrap_or(id)
                .as_bytes()
                .to_vec(),
        )
    }
}

fn exists(tx: &Transaction<'_>, table: &str, id: &str) -> Result<bool, Failure> {
    let Ok(id) = id.parse::<Id>() else {
        return Ok(false);
    };
    let mut stmt = tx.prepare_cached(&format!("SELECT 1 FROM {table} WHERE id = ?1"))?;
    Ok(stmt
        .query_row([id.as_bytes().as_slice()], |_| Ok(()))
        .optional()?
        .is_some())
}

fn key_taken(tx: &Transaction<'_>, key: &str) -> Result<bool, Failure> {
    let found: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM projects WHERE key = ?1
             UNION ALL SELECT 1 FROM project_key_aliases WHERE key = ?1 LIMIT 1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Plans keys and IDs against the workspace `tx` reads. With `strict`, a
/// taken key without a new one is an error.
pub(crate) fn plan(
    tx: &Transaction<'_>,
    parsed: &Parsed,
    input: &ImportInput,
    strict: bool,
) -> Result<Plan, Failure> {
    let mut plan = Plan::default();
    let archive_keys: HashSet<&str> = parsed
        .projects
        .iter()
        .map(|p| p.file.project.key.as_str())
        .collect();
    for (i, k) in input.keys.iter().enumerate() {
        if !archive_keys.contains(k.from.as_str()) {
            return Err(messages::invalid(
                &format!("keys[{i}].from"),
                "transfer-key-unknown",
                &[("key", k.from.clone())],
            ));
        }
        if ProjectKey::new(&k.to).is_err() {
            return Err(messages::invalid(
                &format!("keys[{i}].to"),
                "transfer-key-invalid",
                &[("key", k.to.clone())],
            ));
        }
    }
    let mut chosen = HashSet::new();
    for p in &parsed.projects {
        let from = &p.file.project.key;
        let key = input
            .keys
            .iter()
            .find(|k| &k.from == from)
            .map_or_else(|| from.clone(), |k| k.to.clone());
        if key_taken(tx, &key)? || !chosen.insert(key.clone()) {
            plan.taken.push(from.clone());
        }
        plan.keys.push(key);
    }
    if strict && !plan.taken.is_empty() {
        let args = [
            ("keys", plan.taken.join(", ")),
            ("count", plan.taken.len().to_string()),
        ];
        return Err(Failure::Op(
            messages::hint(
                messages::conflict("transfer-keys-taken", &args),
                "transfer-keys-taken-hint",
                &[],
            )
            .with_detail("taken_keys", plan.taken.clone()),
        ));
    }
    let mut replace = |table: &str, id: &str| -> Result<(), Failure> {
        if exists(tx, table, id)?
            && let Ok(old) = id.parse::<Id>()
        {
            plan.ids.insert(old, Id::generate());
        }
        Ok(())
    };
    for p in &parsed.projects {
        replace("projects", &p.file.project.id)?;
        for s in &p.file.statuses {
            replace("statuses", &s.id)?;
        }
        for t in &p.file.types {
            replace("task_types", &t.id)?;
        }
        for l in &p.file.labels {
            replace("labels", &l.id)?;
        }
        for v in &p.file.versions {
            replace("versions", &v.id)?;
        }
        for v in &p.file.views {
            replace("saved_views", &v.id)?;
        }
        for t in &p.tasks {
            replace("tasks", &t.id)?;
            for r in &t.relations {
                replace("task_relations", &r.id)?;
            }
        }
        for c in &p.comments {
            replace("comments", &c.id)?;
            for r in &c.revisions {
                replace("comment_revisions", &r.id)?;
            }
        }
        for e in &p.time {
            replace("time_entries", &e.id)?;
        }
        for r in &p.revisions {
            let table = if r.owner_kind == "wiki_page" {
                "wiki_revisions"
            } else {
                "text_revisions"
            };
            replace(table, &r.id)?;
        }
        for w in &p.wiki {
            replace("wiki_pages", &w.id)?;
        }
        for a in &p.files {
            replace("attachments", &a.id)?;
        }
    }
    super::custom_fields::plan(
        tx,
        parsed
            .projects
            .iter()
            .flat_map(|p| p.file.custom_fields.iter()),
        &mut plan.ids,
    )?;
    let mut groups: HashSet<String> = HashSet::new();
    for p in &parsed.projects {
        for a in &p.activity {
            groups.insert(a.group_id.clone());
        }
    }
    let mut stmt = tx.prepare_cached("SELECT 1 FROM activity WHERE group_id = ?1 LIMIT 1")?;
    for g in groups {
        if let Ok(id) = g.parse::<Id>()
            && stmt
                .query_row([id.as_bytes().as_slice()], |_| Ok(()))
                .optional()?
                .is_some()
        {
            plan.groups.insert(id, Id::generate());
        }
    }
    Ok(plan)
}

fn counts_of(parsed: &Parsed) -> ArchiveCounts {
    let c = &parsed.manifest.counts;
    ArchiveCounts {
        projects: c.projects,
        tasks: c.tasks,
        comments: c.comments,
        time_entries: c.time_entries,
        revisions: c.revisions,
        wiki_pages: c.wiki_pages,
        attachments: c.attachments,
        files: c.blobs,
        activity: c.activity,
        agent_sessions: c.agent_sessions,
    }
}

fn result(parsed: &Parsed, plan: &Plan, dry_run: bool, group: Option<String>) -> ImportResult {
    ImportResult {
        dry_run,
        projects: parsed
            .projects
            .iter()
            .zip(&plan.keys)
            .map(|(p, key)| ImportedProject {
                key: key.clone(),
                from_key: p.file.project.key.clone(),
                name: p.file.project.name.clone(),
                id: plan.id(&p.file.project.id).to_string(),
            })
            .collect(),
        counts: counts_of(parsed),
        taken_keys: plan.taken.clone(),
        new_ids: plan.ids.len() as u64,
        format_version: parsed.manifest.format_version,
        app_version: parsed.manifest.app_version.clone(),
        group,
    }
}

// ---------------------------------------------------------------------
// Running it

/// Imports the archive at `path` (or checks it, for a dry run).
pub(crate) fn import(
    engine: &Engine,
    ctx: &Context,
    input: &ImportInput,
    path: &Path,
    limits: &Limits,
    progress: &Progress,
) -> Result<ImportResult, OpError> {
    report(progress, TransferPhase::Reading, 0, 1);
    let mut source = Source::open(path, limits)?;
    let parsed = Arc::new(read(&mut source, limits, progress)?);
    if input.dry_run {
        let plan = engine
            .store()
            .read(|tx| plan(tx, &parsed, input, false))
            .map_err(OpError::from)?;
        report(progress, TransferPhase::Done, 1, 1);
        return Ok(result(&parsed, &plan, true, None));
    }
    // Taken keys are found before any file is copied.
    engine
        .store()
        .read(|tx| plan(tx, &parsed, input, true))
        .map_err(OpError::from)?;

    let layout = engine.store().layout().clone();
    let total = parsed.blobs.len() as u64;
    let mut added = Vec::new();
    for (i, (sha, size)) in parsed.blobs.iter().enumerate() {
        report(progress, TransferPhase::Files, i as u64, total);
        if gk_store::has_file(&layout, sha, *size) {
            continue;
        }
        let entry = format!("files/{sha}");
        let mut reader = source.open_entry(&entry, limits.max_file)?;
        let stored = gk_store::store_reader_unsynced(&layout, &mut reader, limits.max_file)
            .map_err(|e| {
                messages::storage(
                    "transfer-store-entry-failed",
                    &[("path", entry.clone()), ("error", e.to_string())],
                )
            })?;
        if &stored.sha256 != sha {
            let _ = gk_store::remove_file(&layout, &stored.sha256);
            return Err(Rejected::Hash(entry).into());
        }
        added.push(stored.sha256);
    }
    drop(source);
    if let Err(e) = gk_store::sync_files(&layout, &added) {
        for sha in &added {
            let _ = gk_store::remove_file(&layout, sha);
        }
        return Err(messages::storage(
            "transfer-store-files-failed",
            &[("error", e.to_string())],
        ));
    }
    report(progress, TransferPhase::Files, total, total);
    let texts = Arc::new(Texts::of(&parsed));

    let outcome = Arc::new(Mutex::new(None::<ImportResult>));
    let out = Arc::clone(&outcome);
    let input_in = input.clone();
    let parsed_in = Arc::clone(&parsed);
    let progress_in = Arc::clone(progress);
    let f: CommandFn = Arc::new(move |w, _| {
        let plan = plan(w.tx(), &parsed_in, &input_in, true)?;
        write(w, &parsed_in, &texts, &plan, &progress_in)?;
        let r = result(&parsed_in, &plan, false, w.group_if_changed());
        *out.lock().unwrap_or_else(PoisonError::into_inner) = Some(r.clone());
        serde_json::to_value(r).map_err(|e| {
            Failure::Op(messages::storage(
                "storage-json",
                &[("error", e.to_string())],
            ))
        })
    });
    let desc = descriptor("transfer.import").ok_or_else(|| OpError::storage("no import"))?;
    match engine.execute_command(ctx, desc, f, Value::Null) {
        Ok(value) => {
            report(progress, TransferPhase::Done, 1, 1);
            serde_json::from_value(value)
                .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))
        }
        Err(e) => {
            // Nothing was written, so the files copied for it are unused.
            for sha in &added {
                let _ = gk_store::remove_file(&layout, sha);
            }
            Err(e)
        }
    }
}

/// The derived text of a task's description.
#[derive(Debug, Default)]
struct Derived {
    plain: String,
    refs: gk_markdown::References,
    done: i64,
    total: i64,
}

/// What the search index and the checklist counts need from the archive's
/// texts, worked out on several threads before the write transaction.
#[derive(Debug, Default)]
struct Texts {
    /// Per project, per task.
    tasks: Vec<Vec<Derived>>,
    /// Per project, per comment: the plain text and the links.
    comments: Vec<Vec<(String, gk_markdown::References)>>,
    /// The plain text and the links of the wiki revisions that are a
    /// page's current text, by revision ID.
    pages: HashMap<String, (String, gk_markdown::References)>,
}

/// Runs `f` on every item of `items` on several threads, keeping the order.
fn on_threads<T: Sync, R: Send + Default>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().clamp(1, 16));
    let chunk = items.len().div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|scope| {
        let jobs: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(f).collect::<Vec<R>>()))
            .collect();
        jobs.into_iter()
            .flat_map(|j| j.join().unwrap_or_default())
            .collect()
    })
}

impl Texts {
    fn of(parsed: &Parsed) -> Self {
        let mut out = Self::default();
        for p in &parsed.projects {
            let descriptions: Vec<&str> = p
                .tasks
                .iter()
                .map(|t| p.descriptions.get(&t.number).map_or("", String::as_str))
                .collect();
            out.tasks.push(on_threads(&descriptions, |d| {
                if d.is_empty() {
                    return Derived::default();
                }
                let a = gk_markdown::analyze(d);
                Derived {
                    done: a.checklist.iter().filter(|i| i.checked).count() as i64,
                    total: a.checklist.len() as i64,
                    plain: a.plain_text,
                    refs: a.references,
                }
            }));
            let bodies: Vec<&str> = p.comments.iter().map(|c| c.body.as_str()).collect();
            out.comments.push(on_threads(&bodies, |b| {
                let a = gk_markdown::analyze(b);
                (a.plain_text, a.references)
            }));
            let current: HashSet<&String> = p.wiki.iter().map(|w| &w.current_rev).collect();
            let revs: Vec<&Revision> = p
                .revisions
                .iter()
                .filter(|r| r.owner_kind == "wiki_page" && current.contains(&r.id))
                .collect();
            let plain = on_threads(&revs, |r| {
                let a = gk_markdown::analyze(&r.body);
                (a.plain_text, a.references)
            });
            out.pages
                .extend(revs.iter().map(|r| r.id.clone()).zip(plain));
        }
        out
    }
}

fn b(id: Id) -> Vec<u8> {
    id.as_bytes().to_vec()
}

/// Writes every record of `parsed` as `plan` says, inside the command.
#[allow(clippy::too_many_lines)]
fn write(
    w: &mut WriteCtx<'_>,
    parsed: &Parsed,
    texts: &Texts,
    plan: &Plan,
    progress: &Progress,
) -> Result<(), Failure> {
    let tx = w.tx();
    let now = w.now();
    let total: u64 = parsed
        .projects
        .iter()
        .map(|p| {
            (p.tasks.len() + p.comments.len() + p.time.len() + p.revisions.len() + p.wiki.len())
                as u64
        })
        .sum();
    report(progress, TransferPhase::Writing, 0, total);
    let mut done = 0u64;
    let mut tick = |n: u64| {
        done += n;
        if done % STEP < n {
            report(progress, TransferPhase::Writing, done, total);
        }
    };

    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR IGNORE INTO agent_sessions (id, client_id, client_version, display_name,
               profile, scope_project, pid, started_at, last_seen_at, ended_at, reads, writes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for s in &parsed.sessions {
            stmt.execute(params![
                b(s.id.parse().unwrap_or_else(|_| Id::generate())),
                s.client_id,
                s.client_version,
                s.display_name,
                s.profile,
                plan.opt(s.scope_project.as_ref()),
                s.pid,
                s.started_at,
                s.last_seen_at,
                s.ended_at,
                s.reads,
                s.writes,
            ])?;
        }
    }

    // Top-level projects go after the workspace's own.
    let last_top: Option<String> = tx
        .query_row(
            "SELECT max(rank) FROM projects WHERE parent_id IS NULL",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let mut next_top = last_top.and_then(|r| Rank::parse(&r).ok());
    let archive_projects: HashSet<&String> =
        parsed.projects.iter().map(|p| &p.file.project.id).collect();
    let mut imported = Vec::new();

    for (pi, (proj, key)) in parsed.projects.iter().zip(&plan.keys).enumerate() {
        let p = &proj.file.project;
        let id = plan.id(&p.id);
        let parent = p
            .parent_id
            .as_ref()
            .filter(|parent| archive_projects.contains(parent))
            .map(|parent| plan.id(parent));
        let rank = if parent.is_some() {
            p.rank.clone()
        } else {
            let r = next_top.as_ref().map_or_else(Rank::first, Rank::after);
            let text = r.as_str().to_owned();
            next_top = Some(r);
            text
        };
        let max_number = proj.tasks.iter().map(|t| t.number).max().unwrap_or(0);
        tx.prepare_cached(
            "INSERT INTO projects (id, parent_id, rank, key, name, name_fold, description,
               description_rev, hue, icon, state, modules, next_number, created_at, updated_at,
               deleted_at, deleted_group)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )?
        .execute(params![
            b(id),
            parent.map(b),
            rank,
            key,
            p.name,
            fold(&p.name),
            p.description,
            plan.opt(p.description_rev.as_ref()),
            p.hue,
            p.icon,
            p.state,
            serde_json::to_string(&p.modules).unwrap_or_else(|_| "{}".into()),
            p.next_number.max(max_number + 1),
            p.created_at,
            p.updated_at,
            p.deleted_at,
            plan.group(p.deleted_group.as_ref()),
        ])?;
        for alias in &proj.file.key_aliases {
            if alias != key && !key_taken(tx, alias)? && ProjectKey::new(alias).is_ok() {
                tx.execute(
                    "INSERT OR IGNORE INTO project_key_aliases (key, project_id) VALUES (?1, ?2)",
                    params![alias, b(id)],
                )?;
            }
        }
        for s in &proj.file.statuses {
            tx.prepare_cached(
                "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank,
                   wip_limit) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?
            .execute(params![
                b(plan.id(&s.id)),
                b(id),
                s.name,
                fold(&s.name),
                s.category,
                s.hue,
                s.rank,
                s.wip_limit
            ])?;
        }
        for t in &proj.file.types {
            tx.prepare_cached(
                "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank,
                   is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?
            .execute(params![
                b(plan.id(&t.id)),
                b(id),
                t.name,
                fold(&t.name),
                t.glyph,
                t.hue,
                t.rank,
                t.is_default
            ])?;
        }
        for l in &proj.file.labels {
            tx.prepare_cached(
                "INSERT INTO labels (id, project_id, name, name_fold, hue, rank)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![
                b(plan.id(&l.id)),
                b(id),
                l.name,
                fold(&l.name),
                l.hue,
                l.rank
            ])?;
        }
        for v in &proj.file.versions {
            tx.prepare_cached(
                "INSERT INTO versions (id, project_id, name, name_fold, description, status,
                   start_date, due_date, sharing, rank, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?
            .execute(params![
                b(plan.id(&v.id)),
                b(id),
                v.name,
                fold(&v.name),
                v.description,
                v.status,
                v.start_date,
                v.due_date,
                v.sharing,
                v.rank,
                v.created_at,
                v.updated_at
            ])?;
        }
        super::custom_fields::write_fields(tx, &proj.file.custom_fields, id, &|t| plan.id(t), now)?;
        for v in &proj.file.views {
            let j = |v: &Option<Value>| v.as_ref().map(Value::to_string);
            tx.prepare_cached(
                "INSERT INTO saved_views (id, project_id, view_kind, name, description, query,
                   columns, sort, group_by, totals, options, is_default, rank, created_at,
                   updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?
            .execute(params![
                b(plan.id(&v.id)),
                b(id),
                v.view_kind,
                v.name,
                v.description,
                v.query,
                j(&v.columns),
                j(&v.sort),
                j(&v.group_by),
                j(&v.totals),
                j(&v.options),
                v.is_default,
                v.rank,
                v.created_at,
                v.updated_at
            ])?;
        }
        for l in &proj.file.layouts {
            tx.prepare_cached(
                "INSERT OR REPLACE INTO view_settings (scope, view_kind, settings, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )?
            .execute(params![
                id.to_string(),
                l.view_kind,
                l.settings.to_string(),
                l.updated_at
            ])?;
        }

        // Tasks: rows first, parents second (they refer to each other).
        let mut spent: HashMap<&String, i64> = HashMap::new();
        for e in &proj.time {
            if e.deleted_at.is_none() {
                *spent.entry(&e.task_id).or_default() += e.minutes;
            }
        }
        let mut siblings: HashMap<&String, Vec<&Task>> = HashMap::new();
        for t in &proj.tasks {
            if let Some(parent) = &t.parent_id {
                siblings.entry(parent).or_default().push(t);
            }
        }
        let mut subtask_ranks: HashMap<&String, String> = HashMap::new();
        for kids in siblings.values_mut() {
            kids.sort_by(|a, c| {
                a.subtask_rank
                    .cmp(&c.subtask_rank)
                    .then(a.rank.cmp(&c.rank))
                    .then(a.id.cmp(&c.id))
            });
            let spread = Rank::spread(kids.len());
            for (t, r) in kids.iter().zip(spread) {
                subtask_ranks.insert(
                    &t.id,
                    t.subtask_rank
                        .clone()
                        .unwrap_or_else(|| r.as_str().to_owned()),
                );
            }
        }
        for (ti, t) in proj.tasks.iter().enumerate() {
            let description = proj
                .descriptions
                .get(&t.number)
                .cloned()
                .unwrap_or_default();
            let analysis = &texts.tasks[pi][ti];
            let (checked, total) = (analysis.done, analysis.total);
            let tid = plan.id(&t.id);
            links::index_refs(tx, LinkSource::Task(tid), &analysis.refs)?;
            tx.prepare_cached(
                "INSERT INTO tasks (id, project_id, number, title, title_fold, description,
                   description_rev, status_id, status_entered_at, type_id, priority, rank,
                   subtask_rank, start_date, due_date, estimate_min, progress_override,
                   checklist_done, checklist_total, spent_min, claim_session, claim_name,
                   claim_until, created_at, created_by_kind, created_by_name, edited_at,
                   updated_at, closed_at, deleted_at, deleted_group, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                   ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31,
                   ?32)",
            )?
            .execute(params![
                b(tid),
                b(id),
                t.number,
                t.title,
                fold(&t.title),
                description,
                plan.opt(t.description_rev.as_ref()),
                b(plan.id(&t.status_id)),
                t.status_entered_at,
                b(plan.id(&t.type_id)),
                t.priority,
                t.rank,
                subtask_ranks.get(&t.id),
                t.start_date,
                t.due_date,
                t.estimate_min,
                t.progress_override,
                checked,
                total,
                spent.get(&t.id).copied().unwrap_or(0),
                t.claim.as_ref().and_then(|c| plan.opt(Some(&c.session_id))),
                t.claim.as_ref().map(|c| c.name.clone()),
                t.claim.as_ref().map(|c| c.until),
                t.created_at,
                t.created_by_kind,
                t.created_by_name,
                t.edited_at,
                t.updated_at,
                t.closed_at,
                t.deleted_at,
                plan.group(t.deleted_group.as_ref()),
                t.version,
            ])?;
            if t.deleted_at.is_none() && p.deleted_at.is_none() {
                tx.prepare_cached(
                    "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                       updated_at) VALUES ('task', ?1, ?1, ?2, ?3, ?4, ?5)",
                )?
                .execute(params![
                    b(tid),
                    b(id),
                    t.title,
                    analysis.plain,
                    t.updated_at
                ])?;
            }
            tick(1);
        }
        for t in &proj.tasks {
            let tid = plan.id(&t.id);
            if let Some(parent) = &t.parent_id {
                tx.prepare_cached("UPDATE tasks SET parent_id = ?2 WHERE id = ?1")?
                    .execute(params![b(tid), b(plan.id(parent))])?;
            }
            for l in &t.labels {
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
                )?
                .execute(params![b(tid), b(plan.id(l))])?;
            }
            super::custom_fields::write_values(tx, tid, &t.custom_values, &|t| plan.id(t))?;
            for alias in &t.key_aliases {
                if let Ok(old) = alias.parse::<gk_core::TaskKey>() {
                    tx.prepare_cached(
                        "INSERT OR IGNORE INTO task_key_aliases (project_key, number, task_id)
                         VALUES (?1, ?2, ?3)",
                    )?
                    .execute(params![
                        old.project().as_str(),
                        i64::from(old.number()),
                        b(tid)
                    ])?;
                }
            }
        }
        for t in &proj.tasks {
            for r in &t.relations {
                let (mut from, mut to) = (plan.id(&t.id), plan.id(&r.to_task));
                if r.kind == "relates" && to < from {
                    std::mem::swap(&mut from, &mut to);
                }
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO task_relations (id, from_task, to_task, kind, lag_days,
                       created_at, created_by_kind, created_by_name)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )?
                .execute(params![
                    b(plan.id(&r.id)),
                    b(from),
                    b(to),
                    r.kind,
                    r.lag_days,
                    r.created_at,
                    r.created_by_kind,
                    r.created_by_name
                ])?;
            }
        }

        let live_tasks: HashMap<&String, (&String, bool)> = proj
            .tasks
            .iter()
            .map(|t| {
                (
                    &t.id,
                    (&t.title, t.deleted_at.is_none() && p.deleted_at.is_none()),
                )
            })
            .collect();
        for (ci, c) in proj.comments.iter().enumerate() {
            let cid = plan.id(&c.id);
            links::index_refs(tx, LinkSource::Comment(cid), &texts.comments[pi][ci].1)?;
            tx.prepare_cached(
                "INSERT INTO comments (id, task_id, body, actor_kind, actor_name, session_id, via,
                   created_at, edited_at, deleted_at, deleted_group, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?
            .execute(params![
                b(cid),
                b(plan.id(&c.task_id)),
                c.body,
                c.actor_kind,
                c.actor_name,
                plan.opt(c.session_id.as_ref()),
                c.via,
                c.created_at,
                c.edited_at,
                c.deleted_at,
                plan.group(c.deleted_group.as_ref()),
                c.version,
            ])?;
            for r in &c.revisions {
                tx.prepare_cached(
                    "INSERT INTO comment_revisions (id, comment_id, body, actor_kind, actor_name,
                       created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![
                    b(plan.id(&r.id)),
                    b(cid),
                    r.body,
                    r.actor_kind,
                    r.actor_name,
                    r.created_at
                ])?;
            }
            if let Some((title, true)) = live_tasks.get(&c.task_id)
                && c.deleted_at.is_none()
            {
                tx.prepare_cached(
                    "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                       updated_at) VALUES ('comment', ?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![
                    b(cid),
                    b(plan.id(&c.task_id)),
                    b(id),
                    title,
                    texts.comments[pi][ci].0,
                    c.edited_at.unwrap_or(c.created_at)
                ])?;
            }
            tick(1);
        }
        for c in &proj.comments {
            if let Some(r) = &c.reply_to {
                tx.prepare_cached("UPDATE comments SET reply_to = ?2 WHERE id = ?1")?
                    .execute(params![b(plan.id(&c.id)), b(plan.id(r))])?;
            }
        }
        for e in &proj.time {
            tx.prepare_cached(
                "INSERT INTO time_entries (id, task_id, work_date, minutes, note, activity,
                   actor_kind, actor_name, session_id, created_at, edited_at, deleted_at,
                   deleted_group, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?
            .execute(params![
                b(plan.id(&e.id)),
                b(plan.id(&e.task_id)),
                e.work_date,
                e.minutes,
                e.note,
                e.activity,
                e.actor_kind,
                e.actor_name,
                plan.opt(e.session_id.as_ref()),
                e.created_at,
                e.edited_at,
                e.deleted_at,
                plan.group(e.deleted_group.as_ref()),
                e.version,
            ])?;
            tick(1);
        }

        // Pages: rows first, parents second.
        for page in &proj.wiki {
            tx.prepare_cached(
                "INSERT INTO wiki_pages (id, project_id, rank, title, title_fold, slug,
                   current_rev, is_agent_brief, created_at, updated_at, deleted_at, deleted_group,
                   is_home, is_template)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?
            .execute(params![
                b(plan.id(&page.id)),
                b(id),
                page.rank,
                page.title,
                fold(&page.title),
                page.slug,
                b(plan.id(&page.current_rev)),
                page.is_agent_brief,
                page.created_at,
                page.updated_at,
                page.deleted_at,
                plan.group(page.deleted_group.as_ref()),
                page.is_home && page.deleted_at.is_none(),
                page.is_template,
            ])?;
            for slug in &page.redirects {
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO wiki_redirects (project_id, slug, page_id)
                     VALUES (?1, ?2, ?3)",
                )?
                .execute(params![b(id), slug, b(plan.id(&page.id))])?;
            }
            tick(1);
        }
        for page in &proj.wiki {
            if let Some(parent) = &page.parent_id {
                tx.prepare_cached("UPDATE wiki_pages SET parent_id = ?2 WHERE id = ?1")?
                    .execute(params![b(plan.id(&page.id)), b(plan.id(parent))])?;
            }
        }
        for r in &proj.revisions {
            let rid = plan.id(&r.id);
            if r.owner_kind == "wiki_page" {
                tx.prepare_cached(
                    "INSERT INTO wiki_revisions (id, page_id, body, base_rev, kind, message,
                       actor_kind, actor_name, session_id, created_at, resolved_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                )?
                .execute(params![
                    b(rid),
                    b(plan.id(&r.owner_id)),
                    r.body,
                    plan.opt(r.base_rev.as_ref()),
                    r.kind,
                    r.message,
                    r.actor_kind,
                    r.actor_name,
                    plan.opt(r.session_id.as_ref()),
                    r.created_at,
                    r.resolved_at,
                ])?;
            } else {
                tx.prepare_cached(
                    "INSERT INTO text_revisions (id, owner_kind, owner_id, body, base_rev, kind,
                       message, actor_kind, actor_name, session_id, created_at, resolved_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )?
                .execute(params![
                    b(rid),
                    r.owner_kind,
                    b(plan.id(&r.owner_id)),
                    r.body,
                    plan.opt(r.base_rev.as_ref()),
                    r.kind,
                    r.message,
                    r.actor_kind,
                    r.actor_name,
                    plan.opt(r.session_id.as_ref()),
                    r.created_at,
                    r.resolved_at,
                ])?;
            }
            tick(1);
        }
        for page in &proj.wiki {
            if let Some((_, refs)) = texts.pages.get(&page.current_rev) {
                links::index_refs(tx, LinkSource::Wiki(plan.id(&page.id)), refs)?;
            }
            if page.deleted_at.is_none() && p.deleted_at.is_none() {
                let body = texts
                    .pages
                    .get(&page.current_rev)
                    .map_or("", |(plain, _)| plain.as_str());
                tx.prepare_cached(
                    "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                       updated_at) VALUES ('wiki', ?1, NULL, ?2, ?3, ?4, ?5)",
                )?
                .execute(params![
                    b(plan.id(&page.id)),
                    b(id),
                    page.title,
                    body,
                    page.updated_at
                ])?;
            }
        }

        let comment_task: HashMap<&String, &String> =
            proj.comments.iter().map(|c| (&c.id, &c.task_id)).collect();
        for a in &proj.files {
            tx.prepare_cached(
                "INSERT OR IGNORE INTO blobs (sha256, size, created_at) VALUES (?1, ?2, ?3)",
            )?
            .execute(params![
                a.sha256,
                i64::try_from(a.size).unwrap_or(i64::MAX),
                now
            ])?;
            let aid = plan.id(&a.id);
            tx.prepare_cached(
                "INSERT INTO attachments (id, owner_kind, owner_id, file_name, mime, size, sha256,
                   width, height, actor_kind, actor_name, created_at, deleted_at, deleted_group)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?
            .execute(params![
                b(aid),
                a.owner_kind,
                b(plan.id(&a.owner_id)),
                a.file_name,
                a.mime,
                i64::try_from(a.size).unwrap_or(i64::MAX),
                a.sha256,
                a.width,
                a.height,
                a.actor_kind,
                a.actor_name,
                a.created_at,
                a.deleted_at,
                plan.group(a.deleted_group.as_ref()),
            ])?;
            let task = match a.owner_kind.as_str() {
                "task" => Some(&a.owner_id),
                "comment" => comment_task.get(&a.owner_id).copied(),
                _ => None,
            };
            if let Some(task) = task
                && let Some((_, true)) = live_tasks.get(task)
                && a.deleted_at.is_none()
            {
                tx.prepare_cached(
                    "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                       updated_at) VALUES ('file', ?1, ?2, ?3, ?4, '', ?5)",
                )?
                .execute(params![
                    b(aid),
                    b(plan.id(task)),
                    b(id),
                    a.file_name,
                    a.created_at
                ])?;
            }
        }
        imported.push(id);
    }

    // Tasks' versions, once every project's versions are in: a version
    // that isn't in the archive is left off.
    let archive_versions: HashSet<&String> = parsed
        .projects
        .iter()
        .flat_map(|p| p.file.versions.iter().map(|v| &v.id))
        .collect();
    for proj in &parsed.projects {
        for t in &proj.tasks {
            if let Some(v) = t
                .fix_version_id
                .as_ref()
                .filter(|v| archive_versions.contains(v))
            {
                tx.prepare_cached("UPDATE tasks SET fix_version_id = ?2 WHERE id = ?1")?
                    .execute(params![b(plan.id(&t.id)), b(plan.id(v))])?;
            }
        }
    }

    // History, in its original order across the archive.
    let mut rows: Vec<(&Activity, Id)> = parsed
        .projects
        .iter()
        .flat_map(|p| {
            let id = plan.id(&p.file.project.id);
            p.activity.iter().map(move |a| (a, id))
        })
        .collect();
    rows.sort_by_key(|(a, _)| a.seq);
    let activity_total = rows.len() as u64;
    for (i, (a, project)) in rows.into_iter().enumerate() {
        let mut changes = a.changes.clone();
        match &mut changes {
            Value::Object(map) => {
                map.insert("imported".into(), Value::Bool(true));
            }
            other => {
                *other = json!({ "value": other.clone(), "imported": true });
            }
        }
        tx.prepare_cached(
            "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via, session_id,
               project_id, entity_kind, entity_id, changes, inverse, reverted_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?
        .execute(params![
            plan.group(Some(&a.group_id)),
            a.at,
            a.op,
            a.actor_kind,
            a.actor_name,
            a.via,
            plan.opt(a.session_id.as_ref()),
            a.project_id.as_ref().map(|_| b(project)),
            a.entity_kind,
            b(plan.id(&a.entity_id)),
            changes.to_string(),
            a.inverse.as_ref().map(Value::to_string),
            plan.group(a.reverted_by.as_ref()),
        ])?;
        if (i as u64).is_multiple_of(STEP * 10) {
            report(progress, TransferPhase::Writing, i as u64, activity_total);
        }
    }

    for id in imported {
        w.record_field(
            PROJECT,
            id,
            Some(id),
            "imported",
            None,
            Some(Value::Bool(true)),
        );
    }
    Ok(())
}
