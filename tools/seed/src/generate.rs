// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Builds data sets S and L.

use std::collections::BTreeMap;

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::archive::Archive;
use crate::blobs;
use crate::clock::{self, DAY_MS, HOUR_MS, MINUTE_MS};
use crate::model::{
    Activity, AgentSession, Attachment, Claim, Comment, CommentRevision, Counts, Label, Modules,
    Project, ProjectFile, Relation, Revision, Status, Task, TaskType, TimeEntry, WikiPage,
};
use crate::rng::Rng;
use crate::text::{self, Context};

/// What one project of a data set contains.
#[derive(Debug, Clone)]
pub(crate) struct ProjectSpec {
    pub key: &'static str,
    pub name: &'static str,
    /// Index of the parent project in the same list.
    pub parent: Option<usize>,
    pub hue: &'static str,
    pub archived: bool,
    pub tasks: usize,
    /// Exact number of open tasks (todo or in progress); `None` draws it.
    pub open: Option<usize>,
    pub dated: usize,
    pub blocks: usize,
    pub relates: usize,
    pub comments: usize,
    pub time_entries: usize,
    pub files: usize,
    pub wiki: usize,
    pub trash: usize,
    pub claims: usize,
    pub subtasks_per_mille: u64,
    /// Share of the other tasks that get a due date without a start date.
    pub due_only_per_mille: u64,
    pub cancelled_status: bool,
    pub labels: usize,
    /// Days over which tasks are created.
    pub span_days: i64,
    pub rich: bool,
}

const SESSION_NAMES: &[&str] = &[
    "Sample agent Wren",
    "Sample agent Moth",
    "Sample agent Pike",
];

/// "Me": the person at the computer.
pub(crate) const USER: &str = "Me";

#[derive(Debug, Clone)]
pub(crate) struct Actor {
    pub kind: &'static str,
    pub name: String,
    pub via: &'static str,
    pub session: Option<usize>,
}

/// An activity row before `seq` is known.
#[derive(Debug)]
struct Pending {
    at: i64,
    project: usize,
    row: Activity,
}

/// State shared by every project of one archive.
#[derive(Debug)]
pub(crate) struct Builder {
    pub seed: u64,
    pub archive: Archive,
    pub counts: Counts,
    pub sessions: Vec<AgentSession>,
    pub projects: Vec<(String, String, String)>,
    pending: Vec<Pending>,
    project_keys: Vec<String>,
    id_rng: Rng,
}

impl Builder {
    pub(crate) fn new(seed: u64) -> Self {
        let mut id_rng = Rng::new(seed, 1);
        let anchor = clock::anchor_ms();
        let sessions = SESSION_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let started = anchor + i as i64 * DAY_MS;
                AgentSession {
                    id: clock::uuid_v7(started, &mut id_rng),
                    client_id: Some("sample-client".into()),
                    client_version: Some("0.1.0".into()),
                    display_name: (*name).into(),
                    profile: "standard".into(),
                    scope_project: None,
                    pid: None,
                    started_at: started,
                    last_seen_at: started,
                    ended_at: None,
                    reads: 0,
                    writes: 0,
                }
            })
            .collect();
        Self {
            seed,
            archive: Archive::default(),
            counts: Counts::default(),
            sessions,
            projects: Vec::new(),
            pending: Vec::new(),
            project_keys: Vec::new(),
            id_rng,
        }
    }

    pub(crate) fn id(&mut self, at: i64) -> String {
        clock::uuid_v7(at, &mut self.id_rng)
    }

    pub(crate) fn actor(&self, rng: &mut Rng, agent_per_mille: u64) -> Actor {
        if rng.chance(agent_per_mille) {
            let s = rng.index(self.sessions.len());
            Actor {
                kind: "agent",
                name: self.sessions[s].display_name.clone(),
                via: "mcp",
                session: Some(s),
            }
        } else {
            Actor {
                kind: "user",
                name: USER.into(),
                via: if rng.chance(150) { "cli" } else { "app" },
                session: None,
            }
        }
    }

    pub(crate) fn session_id(&self, actor: &Actor) -> Option<String> {
        actor.session.map(|s| self.sessions[s].id.clone())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &mut self,
        project: usize,
        project_id: &str,
        at: i64,
        op: &str,
        actor: &Actor,
        entity_kind: &str,
        entity_id: &str,
        changes: serde_json::Value,
    ) {
        if let Some(s) = actor.session {
            let session = &mut self.sessions[s];
            session.writes += 1;
            session.last_seen_at = session.last_seen_at.max(at);
        }
        // Object keys in a fixed (sorted) order, whatever map type
        // serde_json was built with.
        let changes = match changes {
            serde_json::Value::Object(map) => {
                let sorted: BTreeMap<String, serde_json::Value> = map.into_iter().collect();
                serde_json::Value::Object(sorted.into_iter().collect())
            }
            other => other,
        };
        let group_id = self.id(at);
        let session_id = self.session_id(actor);
        self.pending.push(Pending {
            at,
            project,
            row: Activity {
                seq: 0,
                group_id,
                at,
                op: op.into(),
                actor_kind: actor.kind.into(),
                actor_name: actor.name.clone(),
                via: actor.via.into(),
                session_id,
                project_id: Some(project_id.into()),
                entity_kind: entity_kind.into(),
                entity_id: entity_id.into(),
                changes,
                inverse: None,
                reverted_by: None,
            },
        });
    }

    /// Stores a file's bytes once and returns its SHA-256.
    pub(crate) fn put_blob(&mut self, bytes: Vec<u8>) -> String {
        let sha = hex(&Sha256::digest(&bytes));
        let path = format!("files/{sha}");
        if !self.archive.files.contains_key(&path) {
            self.counts.blobs += 1;
            self.archive.files.insert(path, bytes);
        }
        sha
    }

    pub(crate) fn put_jsonl<T: serde::Serialize>(
        &mut self,
        path: String,
        rows: &[T],
    ) -> anyhow::Result<()> {
        let mut out = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut out, row)?;
            out.push(b'\n');
        }
        self.archive.files.insert(path, out);
        Ok(())
    }

    pub(crate) fn put_json<T: serde::Serialize>(
        &mut self,
        path: String,
        value: &T,
    ) -> anyhow::Result<()> {
        let mut out = serde_json::to_vec_pretty(value)?;
        out.push(b'\n');
        self.archive.files.insert(path, out);
        Ok(())
    }

    /// Writes the activity files and the session list, and fills in the
    /// manifest. Call once, after every project.
    pub(crate) fn finish(mut self, set: &str) -> anyhow::Result<Archive> {
        let mut pending = std::mem::take(&mut self.pending);
        // Stable sort: rows at the same moment keep the order they were made in.
        pending.sort_by_key(|p| p.at);
        let mut per_project: Vec<Vec<Activity>> = vec![Vec::new(); self.project_keys.len()];
        for (i, mut p) in pending.into_iter().enumerate() {
            p.row.seq = i as i64 + 1;
            per_project[p.project].push(p.row);
        }
        self.counts.activity = per_project.iter().map(|v| v.len() as u64).sum();
        for (i, rows) in per_project.iter().enumerate() {
            let path = format!("projects/{}/activity.jsonl", self.project_keys[i]);
            self.put_jsonl(path, rows)?;
        }
        let exported_at = exported_at();
        for s in &mut self.sessions {
            s.ended_at = Some(s.last_seen_at + HOUR_MS);
            s.reads = s.writes * 3;
        }
        self.counts.agent_sessions = self.sessions.len() as u64;
        let sessions = std::mem::take(&mut self.sessions);
        self.put_jsonl("agent_sessions.jsonl".into(), &sessions)?;

        let workspace_id = clock::uuid_v7(clock::anchor_ms(), &mut Rng::new(self.seed, 2));
        let projects = self
            .projects
            .iter()
            .map(|(id, key, name)| crate::model::ManifestProject {
                id: id.clone(),
                key: key.clone(),
                name: name.clone(),
                path: format!("projects/{key}"),
            })
            .collect();
        self.archive.finish(crate::archive::ManifestInput {
            workspace_id,
            exported_at,
            set: set.into(),
            seed: self.seed,
            projects,
            counts: self.counts,
        })
    }

    /// The latest moment recorded so far for project `p`.
    pub(crate) fn last_at(&self, p: usize) -> Option<i64> {
        self.pending
            .iter()
            .filter(|r| r.project == p)
            .map(|r| r.at)
            .max()
    }

    pub(crate) fn add_project_key(&mut self, id: &str, key: &str, name: &str) -> usize {
        self.project_keys.push(key.into());
        self.projects.push((id.into(), key.into(), name.into()));
        self.counts.projects += 1;
        self.project_keys.len() - 1
    }
}

/// The moment every generated archive claims to have been exported at; later
/// than every moment inside it.
pub(crate) fn exported_at() -> i64 {
    clock::anchor_ms() + 600 * DAY_MS
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Default statuses of a new project: name, category, hue.
const STATUSES: &[(&str, &str, &str)] = &[
    ("Backlog", "todo", "grey"),
    ("To do", "todo", "blue"),
    ("In progress", "in_progress", "orange"),
    ("Review", "in_progress", "purple"),
    ("Done", "done", "green"),
];

/// Default types of a new project: name, glyph, hue, weight in per mille.
const TYPES: &[(&str, &str, &str, u64)] = &[
    ("Task", "task", "blue", 500),
    ("Bug", "bug", "red", 250),
    ("Feature", "feature", "green", 200),
    ("Milestone", "milestone", "purple", 50),
];

const HUES: &[&str] = &[
    "blue", "green", "grey", "orange", "pink", "purple", "red", "teal", "yellow",
];

#[derive(Debug, Clone)]
struct StoredFile {
    file_name: String,
    mime: String,
    sha256: String,
    size: u64,
    width: Option<u32>,
    height: Option<u32>,
}

/// Everything about one task that later steps need.
#[derive(Debug, Clone)]
pub(crate) struct TaskState {
    pub task: Task,
    pub description: String,
    pub depth: u32,
    /// Start and due day numbers, for tasks with both dates.
    pub span: Option<(i64, i64)>,
    pub ancestors: Vec<usize>,
    pub open: bool,
    pub has_children: bool,
}

/// Builds one project and returns the IDs of its projects for children.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_project(
    b: &mut Builder,
    spec: &ProjectSpec,
    index_in_set: usize,
    parent_id: Option<&str>,
) -> anyhow::Result<String> {
    let mut rng = Rng::new(b.seed, 100 + index_in_set as u64);
    let anchor = clock::anchor_ms();
    let created = anchor + index_in_set as i64 * HOUR_MS;
    let project_id = b.id(created);
    let p = b.add_project_key(&project_id, spec.key, spec.name);
    let me = Actor {
        kind: "user",
        name: USER.into(),
        via: "app",
        session: None,
    };

    // Statuses, types, labels.
    let mut statuses: Vec<Status> = STATUSES
        .iter()
        .enumerate()
        .map(|(i, (name, category, hue))| Status {
            id: String::new(),
            name: (*name).into(),
            category: (*category).into(),
            hue: (*hue).into(),
            rank: clock::rank(i),
            wip_limit: (*name == "In progress").then_some(5),
        })
        .collect();
    if spec.cancelled_status {
        statuses.push(Status {
            id: String::new(),
            name: "Dropped".into(),
            category: "cancelled".into(),
            hue: "grey".into(),
            rank: clock::rank(statuses.len()),
            wip_limit: None,
        });
    }
    for s in &mut statuses {
        s.id = b.id(created);
    }
    let types: Vec<TaskType> = TYPES
        .iter()
        .enumerate()
        .map(|(i, (name, glyph, hue, _))| TaskType {
            id: b.id(created),
            name: (*name).into(),
            glyph: (*glyph).into(),
            hue: (*hue).into(),
            rank: clock::rank(i),
            is_default: i == 0,
        })
        .collect();
    let labels: Vec<Label> = text::LABEL_NAMES
        .iter()
        .take(spec.labels)
        .enumerate()
        .map(|(i, name)| Label {
            id: b.id(created),
            name: (*name).into(),
            hue: HUES[i % HUES.len()].into(),
            rank: clock::rank(i),
        })
        .collect();

    let project_desc = format!(
        "# {}\n\nSample project for tests. Tasks use the key `{}`.\n",
        spec.name, spec.key
    );
    let project_rev = b.id(created);
    let mut revisions = vec![Revision {
        id: project_rev.clone(),
        owner_kind: "project".into(),
        owner_id: project_id.clone(),
        body: project_desc.clone(),
        base_rev: None,
        kind: "normal".into(),
        message: None,
        actor_kind: "user".into(),
        actor_name: USER.into(),
        session_id: None,
        created_at: created,
        resolved_at: None,
    }];
    b.record(
        p,
        &project_id,
        created,
        "project.create",
        &me,
        "project",
        &project_id,
        json!({ "key": spec.key, "name": spec.name }),
    );

    // Wiki titles come first so that descriptions can link to them.
    let wiki_titles: Vec<String> = (0..spec.wiki)
        .map(|i| {
            let base = text::WIKI_TITLES[i % text::WIKI_TITLES.len()];
            let round = i / text::WIKI_TITLES.len();
            if round == 0 {
                base.to_owned()
            } else {
                format!("{base} {}", round + 1)
            }
        })
        .collect();

    // Which tasks are open, dated, in the trash.
    let span_ms = spec.span_days * DAY_MS;
    let step = (span_ms / spec.tasks.max(1) as i64).max(MINUTE_MS);
    let mut order: Vec<usize> = (0..spec.tasks).collect();
    rng.shuffle(&mut order);
    let open_set: Vec<bool> = match spec.open {
        Some(n) => {
            let mut v = vec![false; spec.tasks];
            for &i in order.iter().take(n) {
                v[i] = true;
            }
            v
        }
        None => (0..spec.tasks).map(|_| rng.chance(550)).collect(),
    };
    rng.shuffle(&mut order);
    let mut dated = vec![false; spec.tasks];
    for &i in order.iter().take(spec.dated) {
        dated[i] = true;
    }

    let todo: Vec<usize> = (0..statuses.len())
        .filter(|&i| matches!(statuses[i].category.as_str(), "todo" | "in_progress"))
        .collect();
    let done_status = statuses
        .iter()
        .position(|s| s.category == "done")
        .unwrap_or(0);
    let dropped_status = statuses.iter().position(|s| s.category == "cancelled");
    let first_todo = statuses.iter().position(|s| s.name == "To do").unwrap_or(0);

    // Tasks.
    let mut tasks: Vec<TaskState> = Vec::with_capacity(spec.tasks);
    for i in 0..spec.tasks {
        let number = i as i64 + 1;
        let key = format!("{}-{number}", spec.key);
        let created_at = created + HOUR_MS + i as i64 * step + rng.range(0, step - 1);
        let actor = b.actor(&mut rng, 150);
        let title = text::task_title(&mut rng, number);
        let ctx = Context {
            project_key: spec.key,
            max_number: i as i64,
            wiki_titles: &wiki_titles,
        };
        let md = text::description(&mut rng, &ctx, spec.rich);

        let mut type_roll = rng.below(1000);
        let mut type_index = 0;
        for (t, (_, _, _, weight)) in TYPES.iter().enumerate() {
            if type_roll < *weight {
                type_index = t;
                break;
            }
            type_roll -= weight;
        }

        let open = open_set[i];
        let status_index = if open {
            todo[rng.index(todo.len())]
        } else {
            match dropped_status {
                Some(d) if rng.chance(100) => d,
                _ => done_status,
            }
        };
        let initial = if statuses[status_index].category == "todo" {
            status_index
        } else {
            first_todo
        };
        let moved = status_index != initial;
        let status_entered_at = if moved {
            created_at + rng.range(HOUR_MS, 20 * DAY_MS)
        } else {
            created_at
        };

        // Parent: an earlier task, so there are never cycles.
        let mut parent = None;
        if i > 0 && rng.chance(spec.subtasks_per_mille) {
            let candidate = rng.index(i);
            if tasks[candidate].depth < 4 {
                parent = Some(candidate);
            }
        }
        let (depth, ancestors) = match parent {
            Some(pi) => {
                let mut a = tasks[pi].ancestors.clone();
                a.push(pi);
                (tasks[pi].depth + 1, a)
            }
            None => (0, Vec::new()),
        };
        if let Some(pi) = parent {
            tasks[pi].has_children = true;
        }

        // Start and due day of tasks with both; set as text once the
        // dependencies are placed, which may move them later.
        let span = if dated[i] {
            let start_day = created_at.div_euclid(DAY_MS) + rng.range(0, 30);
            Some((start_day, start_day + rng.range(0, 20)))
        } else {
            None
        };
        let due_only = (span.is_none() && rng.chance(spec.due_only_per_mille))
            .then(|| clock::day_string(created_at.div_euclid(DAY_MS) + rng.range(1, 60)));

        let mut task_labels: Vec<String> = Vec::new();
        if !labels.is_empty() {
            for _ in 0..rng.below(4) {
                let l = &labels[rng.index(labels.len())].id;
                if !task_labels.contains(l) {
                    task_labels.push(l.clone());
                }
            }
        }
        task_labels.sort();

        let estimate_min = rng
            .chance(600)
            .then(|| *rng.pick(&[15, 30, 60, 90, 120, 240, 480, 960, 2400]));
        let progress_override = rng.chance(30).then(|| rng.range(0, 10) as u8 * 10);

        let task_id = b.id(created_at);
        let mut version = 1;
        let mut description_rev = None;
        let mut edited_at = created_at;
        if !md.text.is_empty() {
            let mut base = None;
            if rng.chance(100) {
                // An earlier, shorter text.
                let first = md.text.split("\n\n").next().unwrap_or("").to_owned() + "\n";
                let rev = b.id(created_at);
                revisions.push(Revision {
                    id: rev.clone(),
                    owner_kind: "task".into(),
                    owner_id: task_id.clone(),
                    body: first,
                    base_rev: None,
                    kind: "normal".into(),
                    message: None,
                    actor_kind: actor.kind.into(),
                    actor_name: actor.name.clone(),
                    session_id: b.session_id(&actor),
                    created_at,
                    resolved_at: None,
                });
                base = Some(rev);
            }
            let at = if base.is_some() {
                created_at + rng.range(MINUTE_MS, 3 * DAY_MS)
            } else {
                created_at
            };
            let rev = b.id(at);
            revisions.push(Revision {
                id: rev.clone(),
                owner_kind: "task".into(),
                owner_id: task_id.clone(),
                body: md.text.clone(),
                base_rev: base.clone(),
                kind: "normal".into(),
                message: None,
                actor_kind: actor.kind.into(),
                actor_name: actor.name.clone(),
                session_id: b.session_id(&actor),
                created_at: at,
                resolved_at: None,
            });
            if base.is_some() {
                version += 1;
                edited_at = at;
                b.record(
                    p,
                    &project_id,
                    at,
                    "task.update",
                    &actor,
                    "task",
                    &task_id,
                    json!({ "description_rev": [base, rev] }),
                );
            }
            description_rev = Some(rev);
        }

        b.record(
            p,
            &project_id,
            created_at,
            "task.create",
            &actor,
            "task",
            &task_id,
            json!({ "key": key, "status": statuses[initial].name, "title": title }),
        );
        if moved {
            version += 1;
            edited_at = edited_at.max(status_entered_at);
            b.record(
                p,
                &project_id,
                status_entered_at,
                "task.move",
                &me,
                "task",
                &task_id,
                json!({ "status": [statuses[initial].name, statuses[status_index].name] }),
            );
        }
        let closed = matches!(
            statuses[status_index].category.as_str(),
            "done" | "cancelled"
        );

        tasks.push(TaskState {
            task: Task {
                id: task_id,
                project_id: project_id.clone(),
                number,
                key,
                title,
                description_rev,
                status_id: statuses[status_index].id.clone(),
                status_entered_at,
                type_id: types[type_index].id.clone(),
                priority: rng.below(5) as u8,
                parent_id: parent.map(|pi| tasks[pi].task.id.clone()),
                rank: String::new(),
                start_date: None,
                due_date: due_only,
                estimate_min,
                progress_override,
                checklist_done: md.checklist_done,
                checklist_total: md.checklist_total,
                spent_min: 0,
                claim: None,
                labels: task_labels,
                relations: Vec::new(),
                created_at,
                created_by_kind: actor.kind.into(),
                created_by_name: actor.name.clone(),
                edited_at,
                updated_at: edited_at,
                closed_at: closed.then_some(status_entered_at),
                deleted_at: None,
                deleted_group: None,
                version,
            },
            description: md.text,
            depth,
            span,
            ancestors,
            open,
            has_children: false,
        });
    }

    // Trash: leaf tasks only, so nothing live points at a deleted parent.
    let mut trash_left = spec.trash;
    let mut in_trash = vec![false; tasks.len()];
    for i in (0..tasks.len()).rev() {
        if trash_left == 0 {
            break;
        }
        if !tasks[i].has_children && i % 7 == 3 {
            in_trash[i] = true;
            trash_left -= 1;
        }
    }

    // Dependencies. `blocks` always goes from an earlier task to a later one,
    // so there is never a cycle, and never between a task and its ancestor.
    // Then the later task's dates move so it starts after its blockers end.
    let dated_live: Vec<usize> = (0..tasks.len())
        .filter(|&i| !in_trash[i] && tasks[i].span.is_some())
        .collect();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for (x, &a) in dated_live.iter().enumerate() {
        for &c in &dated_live[x + 1..] {
            if !tasks[c].ancestors.contains(&a) {
                candidates.push((a, c));
            }
        }
    }
    anyhow::ensure!(
        candidates.len() >= spec.blocks,
        "{}: only {} possible dependencies for {}",
        spec.key,
        candidates.len(),
        spec.blocks
    );
    rng.shuffle(&mut candidates);
    candidates.truncate(spec.blocks);
    candidates.sort_unstable();
    let mut blockers: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(a, c) in &candidates {
        blockers.entry(c).or_default().push(a);
    }
    for &c in &dated_live {
        let earliest = blockers
            .get(&c)
            .into_iter()
            .flatten()
            .filter_map(|&a| tasks[a].span.map(|(_, due)| due + 1))
            .max();
        if let (Some(earliest), Some((start, due))) = (earliest, tasks[c].span)
            && start < earliest
        {
            tasks[c].span = Some((earliest, due + earliest - start));
        }
    }
    for t in &mut tasks {
        if let Some((start, due)) = t.span {
            t.task.start_date = Some(clock::day_string(start));
            t.task.due_date = Some(clock::day_string(due));
        }
    }
    let mut pairs: Vec<(usize, usize, &'static str)> = candidates
        .into_iter()
        .map(|(a, c)| (a, c, "blocks"))
        .collect();
    let live: Vec<usize> = (0..tasks.len()).filter(|&i| !in_trash[i]).collect();
    let mut tries = 0;
    while pairs.iter().filter(|r| r.2 != "blocks").count() < spec.relates {
        tries += 1;
        anyhow::ensure!(tries < 100_000, "{}: cannot place relations", spec.key);
        let a = live[rng.index(live.len())];
        let c = live[rng.index(live.len())];
        if a == c || tasks[a].ancestors.contains(&c) || tasks[c].ancestors.contains(&a) {
            continue;
        }
        // `relates` is stored once, from the smaller ID.
        let (a, c) = if tasks[a].task.id < tasks[c].task.id {
            (a, c)
        } else {
            (c, a)
        };
        let kind = if rng.chance(800) {
            "relates"
        } else {
            "duplicates"
        };
        if pairs
            .iter()
            .any(|r| (r.0 == a && r.1 == c) || (r.0 == c && r.1 == a))
        {
            continue;
        }
        pairs.push((a, c, kind));
    }
    for (a, c, kind) in pairs {
        let at = tasks[a].task.created_at.max(tasks[c].task.created_at) + HOUR_MS;
        let id = b.id(at);
        let to_task = tasks[c].task.id.clone();
        b.record(
            p,
            &project_id,
            at,
            "relation.add",
            &me,
            "task",
            &tasks[a].task.id.clone(),
            json!({ "relation": [kind, tasks[c].task.key] }),
        );
        let task = &mut tasks[a].task;
        task.relations.push(Relation {
            id,
            to_task,
            kind: kind.into(),
            lag_days: None,
            created_at: at,
            created_by_kind: "user".into(),
            created_by_name: USER.into(),
        });
        task.updated_at = task.updated_at.max(at);
        if kind == "blocks" {
            b.counts.blocks_relations += 1;
        }
        b.counts.relations += 1;
    }
    for t in &mut tasks {
        t.task.relations.sort_by(|x, y| x.id.cmp(&y.id));
    }

    // Claims: open tasks held by an agent session.
    let mut claims_left = spec.claims;
    for i in 0..tasks.len() {
        if claims_left == 0 {
            break;
        }
        if tasks[i].open && !in_trash[i] && i % 5 == 1 {
            let s = claims_left % b.sessions.len();
            let at = tasks[i].task.created_at + HOUR_MS;
            let actor = Actor {
                kind: "agent",
                name: b.sessions[s].display_name.clone(),
                via: "mcp",
                session: Some(s),
            };
            tasks[i].task.claim = Some(Claim {
                session_id: b.sessions[s].id.clone(),
                name: actor.name.clone(),
                until: exported_at() + 30 * MINUTE_MS,
            });
            tasks[i].task.version += 1;
            let id = tasks[i].task.id.clone();
            b.record(
                p,
                &project_id,
                at,
                "task.claim",
                &actor,
                "task",
                &id,
                json!({ "claim": [null, actor.name] }),
            );
            claims_left -= 1;
        }
    }

    // Comments.
    let mut comments: Vec<Comment> = Vec::with_capacity(spec.comments);
    let mut by_task: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];
    for _ in 0..spec.comments {
        let ti = live[rng.index(live.len())];
        let ctx = Context {
            project_key: spec.key,
            max_number: tasks.len() as i64,
            wiki_titles: &wiki_titles,
        };
        let body = text::comment(&mut rng, &ctx);
        let actor = b.actor(&mut rng, 250);
        let base = tasks[ti].task.created_at;
        let reply_to = if !by_task[ti].is_empty() && rng.chance(150) {
            Some(by_task[ti][rng.index(by_task[ti].len())])
        } else {
            None
        };
        let earliest = reply_to.map_or(base, |r| comments[r].created_at);
        let created_at = earliest + rng.range(MINUTE_MS, 30 * DAY_MS);
        let id = b.id(created_at);
        let mut revisions_c = Vec::new();
        let mut edited_at = None;
        let mut version = 1;
        if rng.chance(80) {
            revisions_c.push(CommentRevision {
                id: b.id(created_at),
                body: format!("{body} (draft)"),
                actor_kind: actor.kind.into(),
                actor_name: actor.name.clone(),
                created_at,
            });
            let at = created_at + rng.range(MINUTE_MS, DAY_MS);
            edited_at = Some(at);
            version = 2;
            b.record(
                p,
                &project_id,
                at,
                "comment.update",
                &actor,
                "comment",
                &id,
                json!({ "task": tasks[ti].task.key }),
            );
        }
        b.record(
            p,
            &project_id,
            created_at,
            "comment.add",
            &actor,
            "comment",
            &id,
            json!({ "task": tasks[ti].task.key }),
        );
        let t = &mut tasks[ti].task;
        t.updated_at = t.updated_at.max(edited_at.unwrap_or(created_at));
        let session_id = b.session_id(&actor);
        by_task[ti].push(comments.len());
        comments.push(Comment {
            id,
            task_id: t.id.clone(),
            body,
            reply_to: reply_to.map(|r| comments[r].id.clone()),
            actor_kind: actor.kind.into(),
            actor_name: actor.name,
            session_id,
            via: actor.via.into(),
            created_at,
            edited_at,
            deleted_at: None,
            deleted_group: None,
            version,
            revisions: revisions_c,
        });
    }
    b.counts.comments += comments.len() as u64;

    // Time entries.
    let mut time: Vec<TimeEntry> = Vec::with_capacity(spec.time_entries);
    for _ in 0..spec.time_entries {
        let ti = live[rng.index(live.len())];
        let actor = b.actor(&mut rng, 100);
        let day = tasks[ti].task.created_at.div_euclid(DAY_MS) + rng.range(0, 20);
        let created_at = (day * DAY_MS + 17 * HOUR_MS).max(tasks[ti].task.created_at + MINUTE_MS);
        let minutes = rng.range(1, 32) * 15;
        let id = b.id(created_at);
        b.record(
            p,
            &project_id,
            created_at,
            "time.log",
            &actor,
            "time_entry",
            &id,
            json!({ "minutes": minutes, "task": tasks[ti].task.key }),
        );
        let t = &mut tasks[ti].task;
        t.spent_min += minutes;
        t.updated_at = t.updated_at.max(created_at);
        let session_id = b.session_id(&actor);
        time.push(TimeEntry {
            id,
            task_id: t.id.clone(),
            work_date: clock::day_string(day),
            minutes,
            note: text::time_note(&mut rng),
            activity: None,
            actor_kind: actor.kind.into(),
            actor_name: actor.name,
            session_id,
            created_at,
            edited_at: None,
            deleted_at: None,
            deleted_group: None,
            version: 1,
        });
    }
    b.counts.time_entries += time.len() as u64;

    // Wiki pages: a tree at most three levels deep under "Home".
    let mut pages: Vec<WikiPage> = Vec::with_capacity(spec.wiki);
    let mut page_depth: Vec<u32> = Vec::new();
    let mut slugs: BTreeMap<String, usize> = BTreeMap::new();
    let mut brief_set = false;
    for (i, title) in wiki_titles.iter().enumerate() {
        let created_at = created + DAY_MS + i as i64 * (span_ms / spec.wiki.max(1) as i64);
        let parent = if i == 0 {
            None
        } else if rng.chance(450) {
            let c = rng.index(i);
            (page_depth[c] < 2).then_some(c)
        } else {
            Some(0)
        };
        let mut slug = clock::slug(title);
        let n = slugs.entry(slug.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            slug = format!("{slug}-{n}");
        }
        let path = match parent {
            Some(pi) => format!("{}/{slug}.md", pages[pi].path.trim_end_matches(".md")),
            None => format!("{slug}.md"),
        };
        let id = b.id(created_at);
        let ctx = Context {
            project_key: spec.key,
            max_number: tasks.len() as i64,
            wiki_titles: &wiki_titles,
        };
        let saves = rng.range(1, 3);
        let mut base: Option<String> = None;
        let mut at = created_at;
        let mut body = String::new();
        for s in 0..saves {
            let actor = b.actor(&mut rng, 200);
            body = text::wiki_body(&mut rng, title, &ctx).text;
            let rev = b.id(at);
            revisions.push(Revision {
                id: rev.clone(),
                owner_kind: "wiki_page".into(),
                owner_id: id.clone(),
                body: body.clone(),
                base_rev: base.clone(),
                kind: "normal".into(),
                message: (s > 0).then(|| "Update".to_owned()),
                actor_kind: actor.kind.into(),
                actor_name: actor.name.clone(),
                session_id: b.session_id(&actor),
                created_at: at,
                resolved_at: None,
            });
            b.record(
                p,
                &project_id,
                at,
                if s == 0 { "wiki.create" } else { "wiki.save" },
                &actor,
                "wiki_page",
                &id,
                json!({ "title": title }),
            );
            base = Some(rev);
            if s + 1 < saves {
                at += rng.range(HOUR_MS, 10 * DAY_MS);
            }
        }
        let is_brief = !brief_set && title == "Onboarding";
        brief_set |= is_brief;
        b.archive.files.insert(
            format!("projects/{}/wiki/{path}", spec.key),
            body.into_bytes(),
        );
        page_depth.push(parent.map_or(0, |pi| page_depth[pi] + 1));
        pages.push(WikiPage {
            id,
            project_id: project_id.clone(),
            parent_id: parent.map(|pi| pages[pi].id.clone()),
            rank: String::new(),
            title: title.clone(),
            slug,
            path,
            current_rev: base.unwrap_or_default(),
            is_agent_brief: is_brief,
            created_at,
            updated_at: at,
            deleted_at: None,
            deleted_group: None,
        });
    }
    // Ranks among siblings.
    let mut sibling_count: BTreeMap<Option<String>, usize> = BTreeMap::new();
    for page in &mut pages {
        let n = sibling_count.entry(page.parent_id.clone()).or_insert(0);
        page.rank = clock::rank(*n);
        *n += 1;
    }
    b.counts.wiki_pages += pages.len() as u64;

    // Attachments.
    let mut files: Vec<Attachment> = Vec::with_capacity(spec.files);
    // Earlier contents, so some files can share a blob.
    let mut seen: Vec<StoredFile> = Vec::new();
    for n in 0..spec.files {
        let roll = rng.below(1000);
        let mut owner_task = None;
        let (owner_kind, owner_id, after) = if roll < 100 && !comments.is_empty() {
            let c = &comments[rng.index(comments.len())];
            ("comment", c.id.clone(), c.created_at)
        } else if roll < 150 && !pages.is_empty() {
            let w = &pages[rng.index(pages.len())];
            ("wiki_page", w.id.clone(), w.created_at)
        } else if roll < 180 {
            ("project", project_id.clone(), created)
        } else {
            let ti = live[rng.index(live.len())];
            owner_task = Some(ti);
            let t = &tasks[ti].task;
            ("task", t.id.clone(), t.created_at)
        };
        let created_at = after + rng.range(MINUTE_MS, 5 * DAY_MS);
        let actor = b.actor(&mut rng, 150);
        let stored = if !seen.is_empty() && rng.chance(50) {
            // The same content attached again under another name.
            let mut copy = seen[rng.index(seen.len())].clone();
            copy.file_name = format!("copy-of-{}", copy.file_name);
            copy
        } else {
            let blob = blobs::generate(&mut rng, n + 1);
            let size = blob.bytes.len() as u64;
            let sha256 = b.put_blob(blob.bytes);
            let stored = StoredFile {
                file_name: blob.file_name,
                mime: blob.mime,
                sha256,
                size,
                width: blob.width,
                height: blob.height,
            };
            seen.push(stored.clone());
            stored
        };
        let id = b.id(created_at);
        b.record(
            p,
            &project_id,
            created_at,
            "file.attach",
            &actor,
            owner_kind,
            &owner_id,
            json!({ "file": stored.file_name }),
        );
        if let Some(ti) = owner_task {
            let t = &mut tasks[ti].task;
            t.updated_at = t.updated_at.max(created_at);
        }
        files.push(Attachment {
            id,
            owner_kind: owner_kind.into(),
            owner_id,
            file_name: stored.file_name,
            mime: stored.mime,
            size: stored.size,
            sha256: stored.sha256,
            width: stored.width,
            height: stored.height,
            actor_kind: actor.kind.into(),
            actor_name: actor.name,
            created_at,
            deleted_at: None,
            deleted_group: None,
        });
    }
    b.counts.attachments += files.len() as u64;

    // Trash, now that every other change is in place.
    for (i, t) in tasks.iter_mut().enumerate() {
        if in_trash[i] {
            let at = t.task.updated_at + DAY_MS;
            let group = b.id(at);
            t.task.deleted_at = Some(at);
            t.task.deleted_group = Some(group);
            t.task.updated_at = at;
            t.task.version += 1;
            let id = t.task.id.clone();
            b.record(
                p,
                &project_id,
                at,
                "task.delete",
                &me,
                "task",
                &id,
                json!({ "key": t.task.key }),
            );
        }
    }

    // Ranks inside each status column, in key order.
    let mut column_count: BTreeMap<String, usize> = BTreeMap::new();
    for t in &mut tasks {
        let n = column_count.entry(t.task.status_id.clone()).or_insert(0);
        t.task.rank = clock::rank(*n);
        *n += 1;
    }

    let mut project_updated = b.last_at(p).unwrap_or(created);
    for t in &tasks {
        project_updated = project_updated.max(t.task.updated_at);
    }
    let state = if spec.archived { "archived" } else { "active" };
    if spec.archived {
        project_updated += DAY_MS;
        b.record(
            p,
            &project_id,
            project_updated,
            "project.archive",
            &me,
            "project",
            &project_id,
            json!({ "state": ["active", "archived"] }),
        );
    }

    // Counts and files.
    b.counts.tasks += tasks.len() as u64;
    b.counts.open_tasks += tasks
        .iter()
        .filter(|t| t.open && t.task.deleted_at.is_none())
        .count() as u64;
    b.counts.dated_tasks += tasks.iter().filter(|t| t.task.start_date.is_some()).count() as u64;
    b.counts.revisions += revisions.len() as u64;

    let dir = format!("projects/{}", spec.key);
    for t in &tasks {
        if !t.description.is_empty() {
            b.archive.files.insert(
                format!("{dir}/tasks/{}.md", t.task.key),
                t.description.clone().into_bytes(),
            );
        }
    }
    let project_file = ProjectFile {
        project: Project {
            id: project_id.clone(),
            parent_id: parent_id.map(str::to_owned),
            rank: clock::rank(index_in_set),
            key: spec.key.into(),
            name: spec.name.into(),
            description: project_desc,
            description_rev: Some(project_rev),
            hue: spec.hue.into(),
            icon: None,
            state: state.into(),
            modules: Modules {
                board: 1,
                list: 1,
                gantt: 1,
                wiki: u8::from(spec.wiki > 0),
                time: 1,
            },
            next_number: spec.tasks as i64 + 1,
            created_at: created,
            updated_at: project_updated,
            deleted_at: None,
            deleted_group: None,
        },
        statuses,
        types,
        labels,
        views: Vec::new(),
        layouts: Vec::new(),
    };
    b.put_json(format!("{dir}/project.json"), &project_file)?;
    let task_rows: Vec<&Task> = tasks.iter().map(|t| &t.task).collect();
    b.put_jsonl(format!("{dir}/tasks.jsonl"), &task_rows)?;
    comments.sort_by(|x, y| (x.created_at, &x.id).cmp(&(y.created_at, &y.id)));
    b.put_jsonl(format!("{dir}/comments.jsonl"), &comments)?;
    time.sort_by(|x, y| (x.created_at, &x.id).cmp(&(y.created_at, &y.id)));
    b.put_jsonl(format!("{dir}/time.jsonl"), &time)?;
    b.put_jsonl(format!("{dir}/revisions.jsonl"), &revisions)?;
    b.put_jsonl(format!("{dir}/wiki.jsonl"), &pages)?;
    files.sort_by(|x, y| (x.created_at, &x.id).cmp(&(y.created_at, &y.id)));
    b.put_jsonl(format!("{dir}/files.jsonl"), &files)?;
    Ok(project_id)
}

/// Data set S: three projects, one of them with two sub-projects; 200 tasks,
/// 500 comments, 50 files and 10 wiki pages in all.
pub(crate) fn small() -> Vec<ProjectSpec> {
    let base = ProjectSpec {
        key: "",
        name: "",
        parent: None,
        hue: "blue",
        archived: false,
        tasks: 0,
        open: None,
        dated: 0,
        blocks: 0,
        relates: 0,
        comments: 0,
        time_entries: 0,
        files: 0,
        wiki: 0,
        trash: 0,
        claims: 0,
        subtasks_per_mille: 150,
        due_only_per_mille: 40,
        cancelled_status: false,
        labels: 6,
        span_days: 120,
        rich: true,
    };
    vec![
        ProjectSpec {
            key: "TM",
            name: "Task manager",
            hue: "blue",
            tasks: 60,
            dated: 15,
            blocks: 6,
            relates: 4,
            comments: 150,
            time_entries: 40,
            files: 15,
            wiki: 4,
            trash: 2,
            claims: 2,
            labels: 10,
            ..base.clone()
        },
        ProjectSpec {
            key: "TMAPP",
            name: "Desktop app",
            parent: Some(0),
            hue: "teal",
            tasks: 40,
            dated: 10,
            blocks: 4,
            relates: 2,
            comments: 100,
            time_entries: 25,
            files: 10,
            wiki: 2,
            trash: 1,
            claims: 1,
            ..base.clone()
        },
        ProjectSpec {
            key: "TMDOC",
            name: "Handbook",
            parent: Some(0),
            hue: "purple",
            tasks: 30,
            dated: 5,
            blocks: 2,
            relates: 1,
            comments: 75,
            time_entries: 15,
            files: 8,
            wiki: 2,
            ..base.clone()
        },
        ProjectSpec {
            key: "GARDEN",
            name: "Garden plan",
            hue: "green",
            tasks: 40,
            dated: 10,
            blocks: 3,
            comments: 100,
            time_entries: 20,
            files: 10,
            wiki: 1,
            cancelled_status: true,
            ..base.clone()
        },
        ProjectSpec {
            key: "LAB",
            name: "Lab notes",
            hue: "orange",
            archived: true,
            tasks: 30,
            dated: 5,
            comments: 75,
            time_entries: 10,
            files: 7,
            wiki: 1,
            labels: 0,
            ..base
        },
    ]
}

/// Data set L: one project with 20,000 tasks (1,000 open), 50,000 comments,
/// 2,000 files, 500 tasks with dates, 300 dependencies and 300 wiki pages.
pub(crate) fn large() -> Vec<ProjectSpec> {
    vec![ProjectSpec {
        key: "BULK",
        name: "Bulk load",
        parent: None,
        hue: "blue",
        archived: false,
        tasks: 20_000,
        open: Some(1_000),
        dated: 500,
        blocks: 300,
        relates: 0,
        comments: 50_000,
        time_entries: 5_000,
        files: 2_000,
        wiki: 300,
        trash: 0,
        claims: 0,
        subtasks_per_mille: 100,
        due_only_per_mille: 0,
        cancelled_status: false,
        labels: 10,
        span_days: 400,
        rich: false,
    }]
}

pub(crate) fn build(set: &str, seed: u64, specs: &[ProjectSpec]) -> anyhow::Result<Archive> {
    let mut b = Builder::new(seed);
    let mut ids: Vec<String> = Vec::new();
    for (i, spec) in specs.iter().enumerate() {
        let parent = spec.parent.map(|pi| ids[pi].clone());
        let id = build_project(&mut b, spec, i, parent.as_deref())?;
        ids.push(id);
    }
    b.finish(set)
}
