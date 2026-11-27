// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Export and import over real workspaces: the test data sets load through
//! the ordinary import, an export of them imports into another workspace
//! unchanged (file hashes included), taken keys and IDs are handled, and
//! undoing an import moves it to the trash.

#![allow(clippy::unwrap_used, clippy::print_stdout)]

use std::path::Path;
use std::time::Instant;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use gk_store::rusqlite::types::ValueRef;
use serde_json::{Value, json};
use tempfile::TempDir;

fn open(dir: &Path, name: &str) -> Engine {
    let layout = WorkspaceLayout::new(dir.join(name));
    Engine::open(&layout, EngineConfig::new(Role::Cli)).unwrap()
}

fn me() -> Context {
    Context::user(Via::Cli)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn seed(dir: &Path, set: gk_seed::DataSet) -> std::path::PathBuf {
    let out = dir.join(format!("seed-{}", set.name()));
    gk_seed::generate(set, 1).unwrap().write_to(&out).unwrap();
    out
}

fn count(e: &Engine, sql: &str) -> i64 {
    e.store()
        .read(|tx| Ok::<_, gk_store::StoreError>(tx.query_row(sql, [], |r| r.get::<_, i64>(0))?))
        .unwrap()
}

/// Every row of `table` (ordered by `order`, without the columns in
/// `skip`), as text, for comparing two workspaces.
fn rows(e: &Engine, table: &str, order: &str, skip: &[&str]) -> Vec<String> {
    e.store()
        .read(|tx| {
            let mut stmt = tx.prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))?;
            let names: Vec<String> = stmt.column_names().map(str::to_owned).collect();
            let mut out = Vec::new();
            let mut q = stmt.query([])?;
            while let Some(r) = q.next()? {
                let mut cells = Vec::new();
                for (i, name) in names.iter().enumerate() {
                    if skip.contains(&name.as_str()) {
                        continue;
                    }
                    cells.push(match r.get_ref(i)? {
                        ValueRef::Null => "∅".to_owned(),
                        ValueRef::Integer(n) => n.to_string(),
                        ValueRef::Real(f) => f.to_string(),
                        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
                        ValueRef::Blob(b) => b.iter().map(|x| format!("{x:02x}")).collect(),
                    });
                }
                out.push(cells.join("|"));
            }
            Ok::<_, gk_store::StoreError>(out)
        })
        .unwrap()
}

const TABLES: &[(&str, &str, &[&str])] = &[
    ("projects", "id", &[]),
    ("statuses", "id", &[]),
    ("task_types", "id", &[]),
    ("labels", "id", &[]),
    ("tasks", "id", &[]),
    ("task_labels", "task_id, label_id", &[]),
    ("task_relations", "id", &[]),
    ("task_key_aliases", "project_key, number", &[]),
    ("comments", "id", &[]),
    ("comment_revisions", "id", &[]),
    ("time_entries", "id", &[]),
    ("text_revisions", "id", &[]),
    ("wiki_pages", "id", &[]),
    ("wiki_revisions", "id", &[]),
    ("wiki_redirects", "project_id, slug", &[]),
    ("attachments", "id", &[]),
    ("blobs", "sha256", &["created_at"]),
    ("saved_views", "id", &[]),
    ("agent_sessions", "id", &["scope_project"]),
    ("text_links", "source_kind, source_id, position", &[]),
];

fn history(e: &Engine) -> Vec<String> {
    e.store()
        .read(|tx| {
            let mut stmt = tx.prepare(
                "SELECT group_id, at, op, actor_name, entity_kind, entity_id, changes
                   FROM activity WHERE op <> 'transfer.import' ORDER BY seq",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(format!(
                        "{:?}|{}|{}|{}|{}|{:?}|{}",
                        r.get::<_, Vec<u8>>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Vec<u8>>(5)?,
                        r.get::<_, String>(6)?
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, gk_store::StoreError>(rows)
        })
        .unwrap()
}

fn stored_hashes(layout: &WorkspaceLayout) -> Vec<String> {
    let mut out = Vec::new();
    for dir in std::fs::read_dir(layout.files_dir()).unwrap().flatten() {
        if dir.file_name() == "tmp" {
            continue;
        }
        for f in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let found = gk_store::sha256_file(&f.path()).unwrap();
            assert_eq!(
                found.sha256,
                f.file_name().to_string_lossy(),
                "stored file hash"
            );
            out.push(found.sha256);
        }
    }
    out.sort();
    out
}

#[test]
fn data_set_s_imports_and_round_trips_through_an_export() {
    let dir = TempDir::new().unwrap();
    let archive = seed(dir.path(), gk_seed::DataSet::Small);
    let a = open(dir.path(), "A");

    let check = call(
        &a,
        "transfer.import",
        json!({ "path": archive.to_string_lossy(), "dry_run": true }),
    );
    assert_eq!(check["dry_run"], true);
    assert_eq!(check["counts"]["tasks"], 200);
    assert_eq!(count(&a, "SELECT count(*) FROM projects"), 0);

    let done = call(
        &a,
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    );
    assert_eq!(done["projects"].as_array().unwrap().len(), 5, "{done}");
    assert_eq!(done["new_ids"], 0);
    assert_eq!(count(&a, "SELECT count(*) FROM tasks"), 200);
    assert_eq!(count(&a, "SELECT count(*) FROM comments"), 500);
    assert_eq!(count(&a, "SELECT count(*) FROM attachments"), 50);
    assert_eq!(count(&a, "SELECT count(*) FROM wiki_pages"), 10);
    // The links in the texts are indexed as they come in.
    assert!(count(&a, "SELECT count(*) FROM text_links") > 0);
    assert_eq!(stored_hashes(a.store().layout()).len(), 50);
    // The data reads back through the operations: tasks, search, trash.
    let tm = call(&a, "task.get", json!({ "keys": ["TM-1"] }));
    assert_eq!(tm["items"][0]["key"], "TM-1", "{tm}");
    let board = call(&a, "board.get", json!({ "project": "TM" }));
    assert!(!board["columns"].as_array().unwrap().is_empty());
    let found = call(&a, "search.query", json!({ "text": "reminder" }));
    assert!(!found["items"].as_array().unwrap().is_empty());
    let trash = call(&a, "trash.list", json!({}));
    assert!(trash["total"].as_u64().unwrap() > 0);

    // Export everything and import it into an empty workspace: the same
    // records and files.
    let zip = dir.path().join("all.zip");
    let exported = call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );
    assert_eq!(exported["format"], "zip");
    assert_eq!(exported["counts"]["tasks"], 200);
    let b = open(dir.path(), "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    for (table, order, skip) in TABLES {
        assert_eq!(
            rows(&a, table, order, skip),
            rows(&b, table, order, skip),
            "{table} differs"
        );
    }
    assert_eq!(history(&a), history(&b));
    assert_eq!(
        stored_hashes(a.store().layout()),
        stored_hashes(b.store().layout())
    );

    // The same as a folder.
    let folder = dir.path().join("all");
    let exported = call(
        &a,
        "transfer.export",
        json!({ "path": folder.to_string_lossy(), "format": "folder" }),
    );
    assert_eq!(exported["format"], "folder");
    assert!(folder.join("manifest.json").is_file());
}

/// The hostile set nests quotes and lists thousands deep; the import works
/// out its plain text on worker threads with the default stack, which an
/// unoptimised build used to overflow.
#[test]
fn hostile_data_set_imports_and_renders() {
    let dir = TempDir::new().unwrap();
    let archive = seed(dir.path(), gk_seed::DataSet::Hostile);
    let layout = WorkspaceLayout::new(dir.path().join("W"));
    let e = Engine::open(&layout, EngineConfig::new(Role::App)).unwrap();
    let app = Context::user(Via::App);
    let call = |op: &str, input: Value| {
        e.call(&app, op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    };
    let done = call(
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    );
    let tasks = done["counts"]["tasks"].as_u64().unwrap();
    assert!(tasks > 0, "{done}");
    assert_eq!(
        count(&e, "SELECT count(*) FROM tasks"),
        i64::try_from(tasks).unwrap()
    );

    let documents: Vec<Value> = (1..=tasks)
        .map(|n| json!({ "task": format!("HOST-{n}") }))
        .collect();
    let rendered = call("markdown.render", json!({ "documents": documents }));
    assert_eq!(
        rendered["items"].as_array().unwrap().len(),
        usize::try_from(tasks).unwrap()
    );
    let found = call("search.query", json!({ "text": "deep quote" }));
    assert!(!found["items"].as_array().unwrap().is_empty(), "{found}");
}

#[test]
fn taken_keys_need_new_ones_and_existing_ids_are_replaced() {
    let dir = TempDir::new().unwrap();
    let archive = seed(dir.path(), gk_seed::DataSet::Small);
    let path = archive.to_string_lossy().into_owned();
    let e = open(dir.path(), "W");
    let first = call(&e, "transfer.import", json!({ "path": path }));

    let err = e
        .call(&me(), "transfer.import", json!({ "path": path }))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(err.details["message_id"], "transfer-keys-taken");
    let check = call(
        &e,
        "transfer.import",
        json!({ "path": path, "dry_run": true }),
    );
    let taken: Vec<String> = serde_json::from_value(check["taken_keys"].clone()).unwrap();
    assert_eq!(taken.len(), 5);
    let keys: Vec<Value> = taken
        .iter()
        .map(|k| json!({ "from": k, "to": format!("{}2", &k[..k.len().min(8)]) }))
        .collect();
    let second = call(&e, "transfer.import", json!({ "path": path, "keys": keys }));
    assert!(second["new_ids"].as_u64().unwrap() > 1000, "{second}");
    assert_eq!(count(&e, "SELECT count(*) FROM tasks"), 400);
    assert_eq!(count(&e, "SELECT count(*) FROM projects"), 10);
    // The copies' history can be read but not reverted.
    let imported_group = count(
        &e,
        "SELECT count(DISTINCT group_id) FROM activity WHERE json_extract(changes, '$.imported') = 1",
    );
    assert!(imported_group > 0);

    // Undoing an import moves its projects to the trash; nothing else.
    let undo = call(&e, "history.revert", json!({ "groups": [second["group"]] }));
    assert_eq!(undo["reverted"].as_array().unwrap().len(), 1, "{undo}");
    assert_eq!(
        count(&e, "SELECT count(*) FROM projects WHERE deleted_at IS NULL"),
        5
    );
    assert_eq!(
        count(&e, "SELECT count(*) FROM tasks WHERE deleted_at IS NULL"),
        count(
            &e,
            "SELECT count(*) FROM tasks WHERE deleted_at IS NULL AND project_id IN
                   (SELECT id FROM projects WHERE deleted_at IS NULL)"
        )
    );
    let _ = first;
}

#[test]
fn an_agent_imports_and_exports_only_in_the_folders_agents_may_use() {
    let dir = TempDir::new().unwrap();
    let shared = dir.path().join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    let shared = gk_store::canonical_path(&shared).unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("W"));
    let mut config = EngineConfig::new(Role::Cli);
    config.agent_folders = gk_engine::AgentFolders::Fixed(vec![shared.clone()]);
    let e = Engine::open(&layout, config).unwrap();
    let session = e
        .start_session(&gk_engine::SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: gk_api::Profile::Standard,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, gk_api::Profile::Standard);
    let outside = dir.path().join("elsewhere.zip");
    for (path, code) in [
        (outside.to_string_lossy().into_owned(), ErrorCode::Forbidden),
        (
            if cfg!(windows) {
                r"C:\Windows\Temp\x.zip"
            } else {
                "/tmp/x.zip"
            }
            .to_owned(),
            ErrorCode::Forbidden,
        ),
        ("../x.zip".to_owned(), ErrorCode::Invalid),
        ("sub/../../x.zip".to_owned(), ErrorCode::Invalid),
        (".hidden".to_owned(), ErrorCode::Invalid),
        ("NUL.zip".to_owned(), ErrorCode::Invalid),
    ] {
        let err = e
            .call(&agent, "transfer.export", json!({ "path": path }))
            .unwrap_err();
        assert_eq!(err.code, code, "{path}: {err:?}");
        assert!(
            err.hint
                .as_deref()
                .unwrap_or_default()
                .contains(&shared.to_string_lossy().into_owned()),
            "{err:?}"
        );
    }
    assert!(!outside.exists());
    call(&e, "project.create", json!({ "name": "T", "key": "TM" }));
    let out = e
        .call(&agent, "transfer.export", json!({ "path": "tm.zip" }))
        .unwrap();
    let exports = gk_store::canonical_path(&e.store().layout().exports_dir()).unwrap();
    assert_eq!(
        Path::new(out["path"].as_str().unwrap()),
        exports.join("tm.zip")
    );
    let check = e
        .call(
            &agent,
            "transfer.import",
            json!({ "path": "tm.zip", "dry_run": true }),
        )
        .unwrap();
    assert_eq!(check["taken_keys"], json!(["TM"]));
    // A plain name may have folders in front, inside exports.
    let out = e
        .call(&agent, "transfer.export", json!({ "path": "sub/tm.zip" }))
        .unwrap();
    assert_eq!(
        Path::new(out["path"].as_str().unwrap()),
        exports.join("sub").join("tm.zip")
    );
    // A folder the person added takes absolute paths.
    let there = shared.join("tm.zip");
    let out = e
        .call(
            &agent,
            "transfer.export",
            json!({ "path": there.to_string_lossy() }),
        )
        .unwrap();
    assert_eq!(Path::new(out["path"].as_str().unwrap()), there);
    let check = e
        .call(
            &agent,
            "transfer.import",
            json!({ "path": there.to_string_lossy(), "dry_run": true }),
        )
        .unwrap();
    assert_eq!(check["taken_keys"], json!(["TM"]));
    // The person's paths are not limited.
    let mine = dir.path().join("mine.zip");
    call(
        &e,
        "transfer.export",
        json!({ "path": mine.to_string_lossy() }),
    );
    assert!(mine.is_file());
    let err = e
        .call(
            &agent,
            "transfer.import",
            json!({ "path": mine.to_string_lossy(), "dry_run": true }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
    // get_workspace tells the agent where it may put files.
    let info = e.call(&agent, "workspace.get", json!({})).unwrap();
    assert_eq!(
        info["agent_folders"],
        json!([exports.to_string_lossy(), shared.to_string_lossy()])
    );
    let info = call(&e, "workspace.get", json!({}));
    assert!(info.get("agent_folders").is_none());
}

#[cfg(unix)]
#[test]
fn an_agent_cant_read_through_a_link_in_the_exports_folder() {
    let dir = TempDir::new().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("W"));
    let mut config = EngineConfig::new(Role::Cli);
    config.agent_folders = gk_engine::AgentFolders::Fixed(Vec::new());
    let e = Engine::open(&layout, config).unwrap();
    call(&e, "project.create", json!({ "name": "T", "key": "TM" }));
    let mine = dir.path().join("mine.zip");
    call(
        &e,
        "transfer.export",
        json!({ "path": mine.to_string_lossy() }),
    );
    let exports = e.store().layout().exports_dir();
    std::fs::create_dir_all(&exports).unwrap();
    std::os::unix::fs::symlink(&mine, exports.join("link.zip")).unwrap();
    std::os::unix::fs::symlink(dir.path(), exports.join("up")).unwrap();
    let session = e
        .start_session(&gk_engine::SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: gk_api::Profile::Standard,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, gk_api::Profile::Standard);
    for (op, path) in [
        ("transfer.import", "link.zip"),
        ("transfer.import", "up/mine.zip"),
        ("transfer.export", "up/new.zip"),
        ("transfer.export", "link.zip"),
    ] {
        let input = if op == "transfer.import" {
            json!({ "path": path, "dry_run": true })
        } else {
            json!({ "path": path })
        };
        let err = e.call(&agent, op, input).unwrap_err();
        assert_eq!(err.code, ErrorCode::Forbidden, "{op} {path}: {err:?}");
        assert_eq!(err.details["message_id"], "agent-path-link", "{op} {path}");
    }
    assert!(!dir.path().join("new.zip").exists());
}

/// Every file and folder below `root`, as paths with `/`.
fn listing(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            let rel = path.strip_prefix(root).unwrap().to_string_lossy();
            out.push(rel.replace('\\', "/"));
        }
    }
    out.sort();
    out
}

#[test]
fn archives_use_names_windows_accepts_and_older_ones_still_import() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    call(
        &e,
        "project.create",
        json!({ "name": "Console", "key": "CON" }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "CON", "title": "A", "description": "Text." }),
    );
    call(
        &e,
        "wiki.create",
        json!({ "project": "CON", "title": "NUL", "body": "Page." }),
    );
    call(
        &e,
        "wiki.create",
        json!({ "project": "CON", "title": "com1", "parent": "NUL" }),
    );
    let out = dir.path().join("out");
    call(
        &e,
        "transfer.export",
        json!({ "path": out.to_string_lossy(), "format": "folder" }),
    );
    let names = listing(&out);
    for name in &names {
        for part in name.split('/') {
            assert_eq!(gk_core::portable_file_name(part), part, "{names:?}");
        }
    }
    for expected in [
        "projects/_CON/project.json",
        "projects/_CON/tasks/CON-1.md",
        "projects/_CON/wiki/_nul.md",
        "projects/_CON/wiki/_nul/_com1.md",
    ] {
        assert!(names.iter().any(|n| n == expected), "{expected}: {names:?}");
    }

    let other = open(dir.path(), "Other");
    let done = call(
        &other,
        "transfer.import",
        json!({ "path": out.to_string_lossy() }),
    );
    assert_eq!(done["counts"]["projects"], 1, "{done}");
    assert_eq!(
        count(
            &other,
            "SELECT count(*) FROM wiki_pages WHERE slug IN ('nul', 'com1')"
        ),
        2
    );

    // An archive written before used the key as it is. Windows can't hold
    // such a folder at all ("Bad device type"), so only elsewhere.
    if cfg!(windows) {
        return;
    }
    std::fs::rename(out.join("projects/_CON"), out.join("projects/CON")).unwrap();
    let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap();
    std::fs::write(
        out.join("manifest.json"),
        manifest.replace("projects/_CON", "projects/CON"),
    )
    .unwrap();
    let third = open(dir.path(), "Third");
    let done = call(
        &third,
        "transfer.import",
        json!({ "path": out.to_string_lossy() }),
    );
    assert_eq!(done["counts"]["projects"], 1, "{done}");
    assert_eq!(count(&third, "SELECT count(*) FROM tasks"), 1);
}

/// Data set L: run with `cargo test -p gk-engine --release --test transfer -- --ignored`.
#[test]
#[ignore = "a timing run on the large data set"]
fn data_set_l_imports_in_reasonable_time() {
    let dir = TempDir::new().unwrap();
    let archive = seed(dir.path(), gk_seed::DataSet::Large);
    let e = open(dir.path(), "L");
    let started = Instant::now();
    let last = std::sync::Mutex::new(None);
    let progress: gk_engine::transfer::Progress = std::sync::Arc::new(move |p| {
        let mut last = last.lock().unwrap();
        if *last != Some(p.phase) {
            println!("{:>8.2?} {:?}", started.elapsed(), p.phase);
            *last = Some(p.phase);
        }
    });
    let input = gk_api::ops::transfer::ImportInput {
        path: archive.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let done = serde_json::to_value(e.import_archive(&me(), &input, &progress).unwrap()).unwrap();
    let took = started.elapsed();
    println!("data set L imported in {took:.2?}: {}", done["counts"]);
    assert_eq!(count(&e, "SELECT count(*) FROM tasks"), 20_000);
    assert_eq!(count(&e, "SELECT count(*) FROM comments"), 50_000);
    let zip = dir.path().join("l.zip");
    let started = Instant::now();
    call(
        &e,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );
    println!("data set L exported in {:.2?}", started.elapsed());
}

/// An import writes in one transaction. Another program that wants to
/// write meanwhile waits for it within its busy budget, and past the
/// budget gets a clear `busy` error instead of waiting on.
#[test]
fn a_writer_waiting_on_an_import_gets_busy_rather_than_hanging() {
    let dir = TempDir::new().unwrap();
    let source = open(dir.path(), "S");
    call(
        &source,
        "project.create",
        json!({ "name": "T", "key": "TM" }),
    );
    call(
        &source,
        "task.create",
        json!({ "project": "TM", "title": "One" }),
    );
    let zip = dir.path().join("tm.zip");
    call(
        &source,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let importer = open(dir.path(), "W");
    let mut impatient = EngineConfig::new(Role::Cli);
    impatient.store.busy_budget = std::time::Duration::from_millis(100);
    impatient.store.busy_retries = 0;
    let other = Engine::open(&WorkspaceLayout::new(dir.path().join("W")), impatient).unwrap();
    call(
        &other,
        "project.create",
        json!({ "name": "Mine", "key": "MY" }),
    );

    let (writing, started) = std::sync::mpsc::channel();
    let once = std::sync::Mutex::new(Some(writing));
    let progress: gk_engine::transfer::Progress = std::sync::Arc::new(move |p| {
        if p.phase == gk_api::ops::transfer::TransferPhase::Writing
            && let Some(tx) = once.lock().unwrap().take()
        {
            tx.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    });
    let input = gk_api::ops::transfer::ImportInput {
        path: zip.to_string_lossy().into_owned(),
        ..Default::default()
    };
    std::thread::scope(|scope| {
        let job = scope.spawn(|| importer.import_archive(&me(), &input, &progress));
        started.recv().unwrap();
        let asked = Instant::now();
        let err = other
            .call(
                &me(),
                "task.create",
                json!({ "project": "MY", "title": "Meanwhile" }),
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Busy, "{err:?}");
        assert!(err.hint.is_some());
        assert!(asked.elapsed() < std::time::Duration::from_millis(1200));
        job.join().unwrap().unwrap();
    });
    // Once the import is done, the write goes through.
    call(
        &other,
        "task.create",
        json!({ "project": "MY", "title": "After" }),
    );
    assert_eq!(count(&importer, "SELECT count(*) FROM tasks"), 2);
}
