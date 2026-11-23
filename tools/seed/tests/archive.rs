// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Determinism and validity of the generated archives.
//!
//! Every file is read back through the record types (which reject unknown
//! fields) and must serialise to the same bytes; then the rules of the data
//! model are checked across the records.

// The helpers below are part of the tests: a failure should panic.
#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use gk_seed::archive::MANIFEST;
use gk_seed::model::{
    Activity, AgentSession, Attachment, Comment, Counts, FORMAT_NAME, FORMAT_VERSION, Fixture,
    Manifest, ProjectFile, Revision, Task, TimeEntry, WikiPage,
};
use gk_seed::{Archive, DataSet, generate, hostile, text};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

const MARKDOWN_LIMIT: usize = 1024 * 1024;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn utf8<'a>(archive: &'a Archive, path: &str) -> &'a str {
    let bytes = archive
        .files
        .get(path)
        .unwrap_or_else(|| panic!("missing {path}"));
    std::str::from_utf8(bytes).unwrap_or_else(|e| panic!("{path} is not UTF-8: {e}"))
}

/// Parses every line and checks that it serialises back to the same bytes.
fn lines<T: DeserializeOwned + Serialize>(archive: &Archive, path: &str) -> Vec<T> {
    let text = utf8(archive, path);
    assert!(text.is_empty() || text.ends_with('\n'), "{path}: last line");
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            let row: T =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("{path}:{}: {e}", i + 1));
            assert_eq!(
                serde_json::to_string(&row).unwrap(),
                line,
                "{path}:{} does not round-trip",
                i + 1
            );
            row
        })
        .collect()
}

fn document<T: DeserializeOwned + Serialize>(archive: &Archive, path: &str) -> T {
    let text = utf8(archive, path);
    let value: T = serde_json::from_str(text).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(
        serde_json::to_string_pretty(&value).unwrap() + "\n",
        text,
        "{path} does not round-trip"
    );
    value
}

fn is_uuid_v7(id: &str) -> bool {
    let b = id.as_bytes();
    id.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| b[i] == b'-')
        && b[14] == b'7'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
        && id
            .chars()
            .all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 10 && b[4] == b'-' && b[7] == b'-'
}

fn is_project_key(key: &str) -> bool {
    (2..=10).contains(&key.len())
        && key.starts_with(|c: char| c.is_ascii_uppercase())
        && key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Every address in generated text points at a reserved or local host.
fn check_addresses(path: &str, text: &str) {
    for (i, _) in text.match_indices("://") {
        let rest = &text[i + 3..];
        let ok = ["example.invalid", "www.w3.org", "localhost", "/"]
            .iter()
            .any(|allowed| rest.starts_with(allowed));
        assert!(
            ok,
            "{path}: address to an unexpected host: {}",
            &rest[..rest.len().min(40)]
        );
    }
    assert!(
        !text.contains("@gmail") && !text.contains("@example.com"),
        "{path}: e-mail address"
    );
}

/// Collects IDs and fails on a repeat.
#[derive(Default)]
struct Ids(BTreeSet<String>);

impl Ids {
    fn add(&mut self, id: &str) {
        assert!(is_uuid_v7(id), "not a UUIDv7: {id}");
        assert!(self.0.insert(id.to_owned()), "duplicate id {id}");
    }
}

/// Checks a whole archive and returns the counts found in it.
#[allow(clippy::too_many_lines)]
fn check(archive: &Archive) -> Counts {
    let manifest: Manifest = document(archive, MANIFEST);
    assert_eq!(manifest.format, FORMAT_NAME);
    assert_eq!(manifest.format_version, FORMAT_VERSION);
    let listed: BTreeSet<&String> = manifest.hashes.keys().collect();
    let present: BTreeSet<&String> = archive.files.keys().filter(|p| *p != MANIFEST).collect();
    assert_eq!(
        listed, present,
        "manifest hashes list exactly the other files"
    );
    for (path, sha) in &manifest.hashes {
        assert_eq!(&hex(&Sha256::digest(&archive.files[path])), sha, "{path}");
    }
    for path in archive.files.keys().filter(|p| !p.starts_with("files/")) {
        check_addresses(path, utf8(archive, path));
    }
    let exported_at = manifest.exported_at;
    let moment = |at: i64| assert!(at > 0 && at < exported_at, "moment {at} out of range");

    let mut ids = Ids::default();
    ids.add(&manifest.workspace_id);
    let mut c = Counts::default();

    let sessions: Vec<AgentSession> = lines(archive, "agent_sessions.jsonl");
    let session_ids: BTreeSet<String> = sessions.iter().map(|s| s.id.clone()).collect();
    for s in &sessions {
        ids.add(&s.id);
        moment(s.started_at);
        assert!(s.started_at <= s.last_seen_at);
        assert!(!s.display_name.is_empty());
    }
    c.agent_sessions = sessions.len() as u64;

    let mut project_ids: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut all_activity: Vec<Activity> = Vec::new();
    let mut writes_by_session: BTreeMap<String, i64> = BTreeMap::new();

    for mp in &manifest.projects {
        let dir = &mp.path;
        let pf: ProjectFile = document(archive, &format!("{dir}/project.json"));
        let project = &pf.project;
        assert_eq!(project.id, mp.id);
        assert_eq!(project.key, mp.key);
        assert!(is_project_key(&project.key), "key {}", project.key);
        ids.add(&project.id);
        moment(project.created_at);
        moment(project.updated_at);
        project_ids.insert(project.id.clone(), project.parent_id.clone());
        assert!(matches!(project.state.as_str(), "active" | "archived"));
        c.projects += 1;

        let mut categories = BTreeMap::new();
        for s in &pf.statuses {
            ids.add(&s.id);
            assert!(matches!(
                s.category.as_str(),
                "todo" | "in_progress" | "done" | "cancelled"
            ));
            categories.insert(s.id.clone(), s.category.clone());
        }
        let type_ids: BTreeSet<&String> = pf.types.iter().map(|t| &t.id).collect();
        for t in &pf.types {
            ids.add(&t.id);
            assert!(matches!(
                t.glyph.as_str(),
                "task" | "bug" | "feature" | "milestone"
            ));
        }
        assert_eq!(pf.types.iter().filter(|t| t.is_default).count(), 1);
        let label_ids: BTreeSet<&String> = pf.labels.iter().map(|l| &l.id).collect();
        for l in &pf.labels {
            ids.add(&l.id);
        }

        // Revisions first: tasks and pages point at them.
        let revisions: Vec<Revision> = lines(archive, &format!("{dir}/revisions.jsonl"));
        let mut rev_by_id: BTreeMap<&str, &Revision> = BTreeMap::new();
        for r in &revisions {
            ids.add(&r.id);
            moment(r.created_at);
            assert!(matches!(
                r.owner_kind.as_str(),
                "task" | "project" | "wiki_page"
            ));
            assert!(r.body.len() <= MARKDOWN_LIMIT);
            rev_by_id.insert(&r.id, r);
        }
        for r in &revisions {
            if let Some(base) = &r.base_rev {
                assert!(rev_by_id.contains_key(base.as_str()));
            }
        }
        c.revisions += revisions.len() as u64;

        // Tasks.
        let tasks: Vec<Task> = lines(archive, &format!("{dir}/tasks.jsonl"));
        let by_id: BTreeMap<&str, &Task> = tasks.iter().map(|t| (t.id.as_str(), t)).collect();
        let mut numbers = BTreeSet::new();
        let mut ranks: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for t in &tasks {
            ids.add(&t.id);
            assert_eq!(t.project_id, project.id);
            assert!(numbers.insert(t.number), "number {} twice", t.number);
            assert_eq!(t.key, format!("{}-{}", project.key, t.number));
            let chars = t.title.chars().count();
            assert!(
                (1..=500).contains(&chars),
                "{}: title length {chars}",
                t.key
            );
            assert!(categories.contains_key(&t.status_id));
            assert!(type_ids.contains(&t.type_id));
            assert!(t.priority <= 4);
            assert!(t.labels.iter().all(|l| label_ids.contains(l)));
            assert!(
                t.labels.windows(2).all(|w| w[0] < w[1]),
                "labels sorted and distinct"
            );
            assert!(
                ranks.entry(&t.status_id).or_default().insert(&t.rank),
                "{}: rank repeats in its column",
                t.key
            );
            for d in [&t.start_date, &t.due_date].into_iter().flatten() {
                assert!(is_day(d), "{d}");
            }
            if let (Some(s), Some(d)) = (&t.start_date, &t.due_date) {
                assert!(s <= d, "{}: start after due", t.key);
            }
            if let Some(p) = t.progress_override {
                assert!(p <= 100);
            }
            moment(t.created_at);
            assert!(t.created_at <= t.status_entered_at);
            assert!(t.status_entered_at <= t.edited_at);
            assert!(t.edited_at <= t.updated_at);
            moment(t.updated_at);
            let closed = matches!(categories[&t.status_id].as_str(), "done" | "cancelled");
            assert_eq!(t.closed_at.is_some(), closed, "{}: closed_at", t.key);
            if let Some(claim) = &t.claim {
                assert!(session_ids.contains(&claim.session_id));
            }
            assert_eq!(t.deleted_at.is_some(), t.deleted_group.is_some());

            // Description, its file and its revision.
            let md_path = format!("{dir}/tasks/{}.md", t.key);
            match (&t.description_rev, archive.files.get(&md_path)) {
                (Some(rev), Some(_)) => {
                    let body = utf8(archive, &md_path);
                    assert!(
                        body.len() <= MARKDOWN_LIMIT,
                        "{}: description too long",
                        t.key
                    );
                    let r = rev_by_id[rev.as_str()];
                    assert_eq!(r.owner_id, t.id);
                    assert_eq!(r.body, body, "{}: revision text", t.key);
                    assert_eq!(
                        text::count_checklist(body),
                        (t.checklist_done, t.checklist_total),
                        "{}: checklist counts",
                        t.key
                    );
                }
                (None, None) => assert_eq!((t.checklist_done, t.checklist_total), (0, 0)),
                _ => panic!("{}: description file and revision disagree", t.key),
            }

            for r in &t.relations {
                ids.add(&r.id);
                assert!(by_id.contains_key(r.to_task.as_str()), "relation target");
                assert_ne!(r.to_task, t.id, "relation to itself");
                assert!(matches!(
                    r.kind.as_str(),
                    "blocks" | "relates" | "duplicates" | "clones"
                ));
                if r.kind == "relates" {
                    assert!(t.id < r.to_task, "relates is stored from the smaller id");
                }
                c.relations += 1;
                c.blocks_relations += u64::from(r.kind == "blocks");
            }
        }
        if let Some(max) = numbers.iter().max() {
            assert!(project.next_number > *max);
        }

        // Hierarchy: same project, depth at most five, no cycles.
        let ancestors = |t: &Task| -> Vec<String> {
            let mut out = Vec::new();
            let mut cur = t.parent_id.clone();
            while let Some(p) = cur {
                assert!(!out.contains(&p), "{}: parent cycle", t.key);
                assert!(out.len() < 10, "{}: runaway parent chain", t.key);
                let parent = by_id.get(p.as_str()).expect("parent in the same project");
                assert!(parent.deleted_at.is_none() || t.deleted_at.is_some());
                cur = parent.parent_id.clone();
                out.push(p);
            }
            out
        };
        let mut chains: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for t in &tasks {
            let a = ancestors(t);
            assert!(a.len() < 5, "{}: deeper than five levels", t.key);
            chains.insert(&t.id, a);
        }

        // Relations: one per pair and kind; none between ancestor and
        // descendant; `blocks` has no cycle; dated blockers end first.
        let mut seen_pairs = BTreeSet::new();
        let mut blocks: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for t in &tasks {
            for r in &t.relations {
                assert!(seen_pairs.insert((&t.id, &r.to_task, &r.kind)));
                assert!(!chains[t.id.as_str()].contains(&r.to_task));
                assert!(!chains[r.to_task.as_str()].contains(&t.id));
                if r.kind == "blocks" {
                    blocks.entry(&t.id).or_default().push(&r.to_task);
                    let other = by_id[r.to_task.as_str()];
                    if let (Some(due), Some(start)) = (&t.due_date, &other.start_date) {
                        assert!(due < start, "{} blocks {} but ends later", t.key, other.key);
                    }
                }
            }
        }
        // Kahn's algorithm: every node leaves the queue iff there is no cycle.
        let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();
        for (from, tos) in &blocks {
            indegree.entry(from).or_insert(0);
            for to in tos {
                *indegree.entry(to).or_insert(0) += 1;
            }
        }
        let mut queue: Vec<&str> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut removed = 0;
        while let Some(n) = queue.pop() {
            removed += 1;
            for to in blocks.get(n).into_iter().flatten() {
                let d = indegree.get_mut(to).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push(to);
                }
            }
        }
        assert_eq!(removed, indegree.len(), "dependency cycle");

        c.tasks += tasks.len() as u64;
        c.open_tasks += tasks
            .iter()
            .filter(|t| {
                t.deleted_at.is_none()
                    && matches!(categories[&t.status_id].as_str(), "todo" | "in_progress")
            })
            .count() as u64;
        c.dated_tasks += tasks.iter().filter(|t| t.start_date.is_some()).count() as u64;

        // Comments.
        let comments: Vec<Comment> = lines(archive, &format!("{dir}/comments.jsonl"));
        let comment_by_id: BTreeMap<&str, &Comment> =
            comments.iter().map(|x| (x.id.as_str(), x)).collect();
        for x in &comments {
            ids.add(&x.id);
            let task = by_id[x.task_id.as_str()];
            assert!(task.deleted_at.is_none());
            assert!(x.created_at > task.created_at);
            assert!(x.created_at <= task.updated_at);
            assert!(x.body.len() <= 256 * 1024);
            assert!(matches!(x.via.as_str(), "app" | "cli" | "mcp"));
            assert_eq!(x.session_id.is_some(), x.actor_kind == "agent");
            if let Some(r) = &x.reply_to {
                let parent = comment_by_id[r.as_str()];
                assert_eq!(parent.task_id, x.task_id);
                assert!(parent.created_at < x.created_at);
            }
            assert_eq!(x.version, 1 + x.revisions.len() as i64);
            for r in &x.revisions {
                ids.add(&r.id);
            }
            moment(x.created_at);
        }
        c.comments += comments.len() as u64;

        // Time entries and the spent time they add up to.
        let time: Vec<TimeEntry> = lines(archive, &format!("{dir}/time.jsonl"));
        let mut spent: BTreeMap<&str, i64> = BTreeMap::new();
        for e in &time {
            ids.add(&e.id);
            assert!(e.minutes > 0);
            assert!(e.created_at > by_id[e.task_id.as_str()].created_at);
            assert!(is_day(&e.work_date));
            moment(e.created_at);
            if e.deleted_at.is_none() {
                *spent.entry(&e.task_id).or_default() += e.minutes;
            }
        }
        for t in &tasks {
            assert_eq!(t.spent_min, spent.get(t.id.as_str()).copied().unwrap_or(0));
        }
        c.time_entries += time.len() as u64;

        // Wiki pages.
        let pages: Vec<WikiPage> = lines(archive, &format!("{dir}/wiki.jsonl"));
        let page_ids: BTreeSet<&String> = pages.iter().map(|p| &p.id).collect();
        let mut slugs = BTreeSet::new();
        assert!(pages.iter().filter(|p| p.is_agent_brief).count() <= 1);
        for p in &pages {
            ids.add(&p.id);
            assert!(slugs.insert(p.slug.to_lowercase()), "slug {} twice", p.slug);
            assert!(!p.slug.contains(['/', '\\']) && !p.slug.starts_with('.'));
            if let Some(parent) = &p.parent_id {
                assert!(page_ids.contains(parent));
            }
            let body = utf8(archive, &format!("{dir}/wiki/{}", p.path));
            let rev = rev_by_id[p.current_rev.as_str()];
            assert_eq!(rev.owner_id, p.id);
            assert_eq!(rev.body, body);
        }
        c.wiki_pages += pages.len() as u64;

        // Attachments and their stored bytes.
        let files: Vec<Attachment> = lines(archive, &format!("{dir}/files.jsonl"));
        for f in &files {
            ids.add(&f.id);
            let bytes = archive
                .files
                .get(&format!("files/{}", f.sha256))
                .unwrap_or_else(|| panic!("blob of {} missing", f.file_name));
            assert_eq!(bytes.len() as u64, f.size, "{}", f.file_name);
            assert_eq!(hex(&Sha256::digest(bytes)), f.sha256);
            assert!(matches!(
                f.owner_kind.as_str(),
                "task" | "comment" | "wiki_page" | "project"
            ));
            moment(f.created_at);
        }
        c.attachments += files.len() as u64;

        // History.
        let activity: Vec<Activity> = lines(archive, &format!("{dir}/activity.jsonl"));
        for a in &activity {
            ids.add(&a.group_id);
            moment(a.at);
            assert_eq!(a.project_id.as_deref(), Some(project.id.as_str()));
            if let Some(s) = &a.session_id {
                *writes_by_session.entry(s.clone()).or_default() += 1;
            }
        }
        // Each item's history starts with its creation; an archived
        // project's ends with the archiving.
        let mut first_op: BTreeMap<(&str, &str), &str> = BTreeMap::new();
        for a in &activity {
            first_op
                .entry((&a.entity_kind, &a.entity_id))
                .or_insert(&a.op);
        }
        for ((kind, id), op) in &first_op {
            let expected = match *kind {
                "project" => "project.create",
                "task" => "task.create",
                "comment" => "comment.add",
                "time_entry" => "time.log",
                "wiki_page" => "wiki.create",
                other => panic!("unexpected entity kind {other}"),
            };
            assert_eq!(*op, expected, "history of {kind} {id} starts with {op}");
        }
        if project.state == "archived" {
            assert_eq!(
                activity.last().map(|a| a.op.as_str()),
                Some("project.archive")
            );
        }
        c.activity += activity.len() as u64;
        all_activity.extend(activity);
    }

    // Sub-projects point at listed projects, at most three levels deep.
    for (id, parent) in &project_ids {
        let mut depth = 0;
        let mut cur = parent.clone();
        while let Some(p) = cur {
            depth += 1;
            assert!(depth < 3, "{id}: sub-projects too deep");
            cur = project_ids.get(&p).expect("parent project listed").clone();
        }
    }

    // One sequence across the archive, in time order.
    all_activity.sort_by_key(|a| a.seq);
    for (i, a) in all_activity.iter().enumerate() {
        assert_eq!(a.seq, i as i64 + 1, "activity sequence has a gap");
    }
    assert!(all_activity.windows(2).all(|w| w[0].at <= w[1].at));
    for s in &sessions {
        assert_eq!(s.writes, writes_by_session.get(&s.id).copied().unwrap_or(0));
    }

    c.blobs = archive
        .files
        .keys()
        .filter(|p| p.starts_with("files/"))
        .count() as u64;
    assert_eq!(manifest.counts, c, "manifest counts");
    c
}

fn seed_binary(set: &str, seed: u64, out: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_gk-seed"))
        .args(["--set", set, "--seed", &seed.to_string(), "--out"])
        .arg(out)
        .status()
        .expect("run gk-seed");
    assert!(status.success(), "gk-seed --set {set} failed");
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("gk-seed")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    dir
}

#[test]
fn binary_output_is_byte_identical_for_the_same_seed() {
    for set in ["S", "hostile"] {
        let a = fresh_dir(&format!("{set}-a"));
        let b = fresh_dir(&format!("{set}-b"));
        seed_binary(set, 42, &a);
        seed_binary(set, 42, &b);
        let read_a = Archive::read_from(&a).unwrap();
        let read_b = Archive::read_from(&b).unwrap();
        assert_eq!(read_a, read_b, "two runs of set {set} differ");
        let in_memory = generate(DataSet::parse(set).unwrap(), 42).unwrap();
        assert_eq!(read_a, in_memory, "written set {set} differs from memory");
    }
}

#[test]
fn different_seeds_differ() {
    let a = generate(DataSet::Small, 1).unwrap();
    let b = generate(DataSet::Small, 2).unwrap();
    assert_ne!(a.files[MANIFEST], b.files[MANIFEST]);
}

#[test]
fn refuses_a_non_empty_folder() {
    let dir = fresh_dir("non-empty");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("keep.txt"), b"x").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_gk-seed"))
        .args(["--set", "S", "--out"])
        .arg(&dir)
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(std::fs::read(dir.join("keep.txt")).unwrap(), b"x");
}

#[test]
fn small_set_is_valid_and_has_the_planned_size() {
    for seed in [1, 7, 12345] {
        let archive = generate(DataSet::Small, seed).unwrap();
        let c = check(&archive);
        // Three top-level projects, one of which has two sub-projects.
        assert_eq!(c.projects, 5);
        let manifest: Manifest = document(&archive, MANIFEST);
        let roots = manifest
            .projects
            .iter()
            .filter(|p| {
                let pf: ProjectFile = document(&archive, &format!("{}/project.json", p.path));
                pf.project.parent_id.is_none()
            })
            .count();
        assert_eq!(roots, 3);
        assert_eq!(c.tasks, 200);
        assert_eq!(c.comments, 500);
        assert_eq!(c.attachments, 50);
        assert_eq!(c.wiki_pages, 10);
        assert!(c.blocks_relations > 0 && c.open_tasks > 0);
    }
}

#[test]
fn large_set_is_valid_and_has_the_planned_size() {
    let archive = generate(DataSet::Large, 1).unwrap();
    let c = check(&archive);
    assert_eq!(c.projects, 1);
    assert_eq!(c.tasks, 20_000);
    assert_eq!(c.open_tasks, 1_000);
    assert_eq!(c.comments, 50_000);
    assert_eq!(c.attachments, 2_000);
    assert_eq!(c.dated_tasks, 500);
    assert_eq!(c.blocks_relations, 300);
    assert_eq!(c.wiki_pages, 300);
    let tasks: Vec<Task> = lines(&archive, "projects/BULK/tasks.jsonl");
    let any_date = tasks
        .iter()
        .filter(|t| t.start_date.is_some() || t.due_date.is_some())
        .count();
    assert_eq!(any_date, 500, "tasks with any date");
    // The manifest holds the hash of every other file.
    let again = generate(DataSet::Large, 1).unwrap();
    assert_eq!(archive.files[MANIFEST], again.files[MANIFEST]);
}

#[test]
fn hostile_set_is_valid_and_documented() {
    let archive = generate(DataSet::Hostile, 1).unwrap();
    for item in hostile::items() {
        assert!(item.body.len() <= hostile::MARKDOWN_LIMIT, "{}", item.name);
        assert!(
            item.title.chars().count() <= hostile::TITLE_LIMIT,
            "{}",
            item.name
        );
        assert!(!item.body.is_empty() && !item.targets.is_empty());
    }
    let c = check(&archive);
    let fixtures: Vec<Fixture> = document(&archive, "fixtures.json");
    assert_eq!(fixtures.len() as u64, c.tasks);
    let names: BTreeSet<&str> = fixtures.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names.len(), fixtures.len(), "fixture names are unique");
    for name in [
        "raw-html-script",
        "huge-table",
        "deep-nesting",
        "katex-hostile",
        "mermaid-hostile",
        "malformed-svg",
    ] {
        assert!(names.contains(name), "{name} missing");
    }
}
