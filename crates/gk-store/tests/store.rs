// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Opening a workspace, its connection settings, the schema, the writer and
//! readers, change detection and checkpoints, against temporary workspaces.

// Helpers outside `#[test]` functions unwrap too; a failure is a test failure.
#![allow(clippy::unwrap_used)]

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use gk_core::{Id, fold};
use gk_store::rusqlite::{self, Transaction, params};
use gk_store::{
    COLLATION, CheckpointOutcome, CheckpointPolicy, CheckpointSchedule, JOURNAL_SIZE_LIMIT,
    REQUEST_WINDOW_MS, RequestKey, SCHEMA_VERSION, SchemaState, Store, StoreConfig, StoreError,
    WorkspaceLayout, open_raw, remember, remembered,
};
use tempfile::TempDir;

fn workspace() -> (TempDir, WorkspaceLayout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::in_data_dir(dir.path(), "Test").unwrap();
    (dir, layout)
}

fn open(layout: &WorkspaceLayout) -> Store {
    Store::open(layout, StoreConfig::default()).unwrap()
}

/// Inserts a project with one status and one type; returns their IDs.
fn insert_project(tx: &Transaction<'_>, key: &str) -> rusqlite::Result<(Id, Id, Id)> {
    let (project, status, kind) = (Id::generate(), Id::generate(), Id::generate());
    tx.execute(
        "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at, updated_at)
         VALUES (?1, 'i', ?2, ?2, ?3, 'blue', '{\"board\":1}', 0, 0)",
        params![project.as_bytes(), key, fold(key)],
    )?;
    tx.execute(
        "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
         VALUES (?1, ?2, 'To do', 'to do', 'todo', 'blue', 'i')",
        params![status.as_bytes(), project.as_bytes()],
    )?;
    tx.execute(
        "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
         VALUES (?1, ?2, 'Task', 'task', 'task', 'blue', 'i', 1)",
        params![kind.as_bytes(), project.as_bytes()],
    )?;
    Ok((project, status, kind))
}

fn insert_task(
    tx: &Transaction<'_>,
    (project, status, kind): (Id, Id, Id),
    number: i64,
    title: &str,
) -> rusqlite::Result<Id> {
    let id = Id::generate();
    tx.execute(
        "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id, status_entered_at,
                            type_id, rank, created_at, created_by_kind, created_by_name,
                            edited_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, 'i', 0, 'user', 'Me', 0, 0)",
        params![
            id.as_bytes(),
            project.as_bytes(),
            number,
            title,
            fold(title),
            status.as_bytes(),
            kind.as_bytes()
        ],
    )?;
    Ok(id)
}

/// Appends one history row in group `group`.
fn insert_activity(tx: &Transaction<'_>, group: Id, op: &str) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via, entity_kind,
                               entity_id, changes)
         VALUES (?1, 0, ?2, 'user', 'Me', 'app', 'task', ?3, '{}')",
        params![group.as_bytes(), op, Id::generate().as_bytes()],
    )?;
    Ok(tx.last_insert_rowid())
}

fn pragma<T: rusqlite::types::FromSql>(tx: &Transaction<'_>, name: &str) -> T {
    tx.pragma_query_value(None, name, |r| r.get(0)).unwrap()
}

#[test]
fn first_open_creates_the_folder_and_one_database() {
    let (_dir, layout) = workspace();
    assert_eq!(Store::probe(&layout).unwrap(), SchemaState::Empty);
    let store = open(&layout);
    for dir in [
        layout.files_dir(),
        layout.thumbs_dir(),
        layout.locks_dir(),
        layout.exports_dir(),
    ] {
        assert!(dir.is_dir(), "{}", dir.display());
    }
    assert!(layout.database().is_file());
    let id = store.workspace_id();
    assert!(id.is_v7());
    store.close();

    assert_eq!(Store::probe(&layout).unwrap(), SchemaState::Current);
    let again = open(&layout);
    assert_eq!(again.workspace_id(), id, "reopening keeps the workspace");
    let dbs: Vec<_> = std::fs::read_dir(layout.root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
        .collect();
    assert_eq!(dbs.len(), 1);
}

#[test]
fn every_connection_gets_the_settings() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    assert_eq!(store.reader_count(), 4);
    let check = |tx: &Transaction<'_>| {
        assert_eq!(pragma::<String>(tx, "journal_mode"), "wal");
        assert_eq!(pragma::<i64>(tx, "synchronous"), 2, "FULL");
        assert_eq!(pragma::<i64>(tx, "foreign_keys"), 1);
        assert_eq!(pragma::<i64>(tx, "temp_store"), 2, "MEMORY");
        assert_eq!(pragma::<i64>(tx, "journal_size_limit"), JOURNAL_SIZE_LIMIT);
        assert_eq!(
            pragma::<i64>(tx, "wal_autocheckpoint"),
            1000,
            "SQLite's default"
        );
        let sorted: String = tx
            .query_row(
                &format!(
                    "SELECT group_concat(x, ' ') FROM (SELECT column1 AS x FROM
                     (VALUES ('Zebra'), ('äpfel'), ('apple')) ORDER BY x COLLATE {COLLATION})"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sorted, "äpfel apple Zebra");
    };
    store
        .write("test.settings", move |tx| {
            check(tx);
            Ok::<_, StoreError>(())
        })
        .unwrap();
    // Several reads at once, so several pooled connections are checked.
    thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                store
                    .read(|tx| {
                        check(tx);
                        assert_eq!(pragma::<i64>(tx, "query_only"), 1);
                        thread::sleep(Duration::from_millis(20));
                        Ok::<_, StoreError>(())
                    })
                    .unwrap();
            });
        }
    });
}

#[test]
fn schema_has_every_table_and_its_rules() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    store
        .write("test.schema", |tx| {
            let tables: Vec<String> = tx
                .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for table in [
                "meta", "settings", "agent_sessions", "projects", "project_key_aliases",
                "statuses", "task_types", "labels", "tasks", "task_key_aliases", "task_labels",
                "task_relations", "comments", "comment_revisions", "text_revisions",
                "time_entries", "timer", "reminders", "reminder_log", "drafts", "blobs",
                "attachments", "saved_views", "view_settings", "activity", "search_docs",
                "fts_words", "fts_grams", "wiki_pages", "wiki_revisions", "wiki_redirects",
            ] {
                assert!(tables.iter().any(|t| t == table), "missing {table}");
            }
            assert_eq!(pragma::<i64>(tx, "user_version"), SCHEMA_VERSION);

            let ids = insert_project(tx, "TM")?;
            let a = insert_task(tx, ids, 1, "Задача номер 1")?;
            let b = insert_task(tx, ids, 2, "Second")?;

            // Rules the database enforces as a backstop for the engine.
            let rejects = |sql: &str, p: &[&dyn rusqlite::ToSql]| {
                assert!(tx.execute(sql, p).is_err(), "accepted: {sql}");
            };
            rejects("UPDATE tasks SET title = '' WHERE number = 1", &[]);
            rejects(&format!("UPDATE tasks SET title = '{}' WHERE number = 1", "x".repeat(501)), &[]);
            rejects("UPDATE tasks SET start_date = '2026-03-02', due_date = '2026-03-01' WHERE number = 1", &[]);
            rejects("UPDATE tasks SET due_date = '1 March' WHERE number = 1", &[]);
            rejects("UPDATE tasks SET priority = 5 WHERE number = 1", &[]);
            rejects("UPDATE tasks SET number = 1 WHERE number = 2", &[]);
            rejects("UPDATE tasks SET status_id = x'00' WHERE number = 1", &[]);
            rejects("UPDATE tasks SET rank = 'A' WHERE number = 1", &[]);
            rejects("UPDATE tasks SET created_by_kind = 'robot' WHERE number = 1", &[]);
            rejects("UPDATE tasks SET estimate_min = 'lots' WHERE number = 1", &[]);
            rejects("UPDATE projects SET key = 'tm'", &[]);
            rejects("UPDATE projects SET key = '1TM'", &[]);
            rejects("UPDATE projects SET modules = 'not json'", &[]);
            rejects("UPDATE statuses SET category = 'closed'", &[]);
            let (low, high) = if a < b { (a, b) } else { (b, a) };
            rejects(
                "INSERT INTO task_relations (id, from_task, to_task, kind, created_at, created_by_kind, created_by_name)
                 VALUES (?1, ?2, ?3, 'relates', 0, 'user', 'Me')",
                &[&Id::generate().as_bytes().as_slice(), &high.as_bytes().as_slice(), &low.as_bytes().as_slice()],
            );
            rejects(
                "INSERT INTO task_relations (id, from_task, to_task, kind, created_at, created_by_kind, created_by_name)
                 VALUES (?1, ?2, ?2, 'blocks', 0, 'user', 'Me')",
                &[&Id::generate().as_bytes().as_slice(), &a.as_bytes().as_slice()],
            );
            tx.execute(
                "INSERT INTO task_relations (id, from_task, to_task, kind, created_at, created_by_kind, created_by_name)
                 VALUES (?1, ?2, ?3, 'relates', 0, 'user', 'Me')",
                params![Id::generate().as_bytes(), low.as_bytes(), high.as_bytes()],
            )?;
            rejects(
                "INSERT INTO time_entries (id, task_id, work_date, minutes, actor_kind, actor_name, created_at)
                 VALUES (?1, ?2, '2026-03-01', 0, 'user', 'Me', 0)",
                &[&Id::generate().as_bytes().as_slice(), &a.as_bytes().as_slice()],
            );
            rejects("INSERT INTO timer (id, task_id, started_at, actor_kind, actor_name) VALUES (2, x'00', 0, 'user', 'Me')", &[]);

            // Matching by folded title, sorting by the collation.
            let found: i64 = tx.query_row(
                "SELECT number FROM tasks WHERE instr(title_fold, ?1) > 0",
                [fold("ЗАДАЧА")],
                |r| r.get(0),
            )?;
            assert_eq!(found, 1);

            let fk: i64 = tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
            assert_eq!(fk, 0);
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

#[test]
fn search_index_follows_search_docs() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    store
        .write("test.search", |tx| {
            let project = Id::generate();
            let insert = |docid: i64, title: &str, body: &str| {
                tx.execute(
                    "INSERT INTO search_docs (docid, kind, entity_id, project_id, title, body, updated_at)
                     VALUES (?1, 'task', ?2, ?3, ?4, ?5, 0)",
                    params![docid, Id::generate().as_bytes(), project.as_bytes(), title, body],
                )
            };
            insert(1, "Connection errors", "ERR_CONN when the user_id is missing")?;
            insert(2, "Tables", "Running the reports")?;
            let words = |q: &str| -> rusqlite::Result<Vec<i64>> {
                tx.prepare("SELECT rowid FROM fts_words WHERE fts_words MATCH ?1 ORDER BY rowid")?
                    .query_map([q], |r| r.get(0))?
                    .collect()
            };
            let grams = |q: &str| -> rusqlite::Result<Vec<i64>> {
                tx.prepare("SELECT rowid FROM fts_grams WHERE fts_grams MATCH ?1 ORDER BY rowid")?
                    .query_map([q], |r| r.get(0))?
                    .collect()
            };
            assert_eq!(words("run")?, [2], "word forms");
            assert_eq!(grams("\"user\"")?, [1], "fragments");
            assert_eq!(grams("\"ERR_CONN\"")?, [1]);
            tx.execute("UPDATE search_docs SET body = 'nothing here' WHERE docid = 1", [])?;
            assert!(grams("\"ERR_CONN\"")?.is_empty());
            assert_eq!(grams("\"Connection\"")?, [1], "title still indexed");
            tx.execute("DELETE FROM search_docs WHERE docid = 2", [])?;
            assert!(words("run")?.is_empty());
            tx.execute("INSERT INTO fts_words (fts_words) VALUES ('integrity-check')", [])?;
            tx.execute("INSERT INTO fts_grams (fts_grams) VALUES ('integrity-check')", [])?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

#[test]
fn a_failed_write_changes_nothing() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let result = store.write("test.fail", |tx| {
        insert_project(tx, "TM")?;
        tx.execute("INSERT INTO projects (id) VALUES (x'00')", [])?;
        Ok::<_, StoreError>(())
    });
    assert!(result.is_err());
    let projects: i64 = store
        .read(|tx| {
            Ok::<_, StoreError>(tx.query_row("SELECT count(*) FROM projects", [], |r| r.get(0))?)
        })
        .unwrap();
    assert_eq!(projects, 0);
    assert_eq!(store.writer_stats().commits, 0);
}

#[test]
fn a_panicking_write_rolls_back_and_the_writer_survives() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.write("test.panic", |tx| {
            insert_project(tx, "TM")?;
            panic!("bug in a command");
            #[allow(unreachable_code)]
            Ok::<_, StoreError>(())
        })
    }));
    assert!(caught.is_err());
    store
        .write("test.after", |tx| {
            let n: i64 = tx.query_row("SELECT count(*) FROM projects", [], |r| r.get(0))?;
            assert_eq!(n, 0);
            insert_project(tx, "TM")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

#[test]
fn a_write_waits_for_another_process_lock() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let other = open_raw(&layout.database()).unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        other.execute_batch("COMMIT").unwrap();
    });
    let started = Instant::now();
    store
        .write("test.wait", |tx| {
            insert_project(tx, "TM")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let waited = started.elapsed();
    release.join().unwrap();
    assert!(waited >= Duration::from_millis(250), "{waited:?}");
    assert!(waited < Duration::from_secs(2), "{waited:?}");
    assert_eq!(
        store.writer_stats().busy_retries,
        0,
        "the busy handler waited; no retry"
    );
}

#[test]
fn a_busy_write_is_retried_whole_then_reports_busy() {
    let (_dir, layout) = workspace();
    let config = StoreConfig {
        busy_budget: Duration::from_millis(60),
        busy_retries: 2,
        ..StoreConfig::default()
    };
    let store = Store::open(&layout, config).unwrap();
    let other = open_raw(&layout.database()).unwrap();

    // Held for longer than one budget, shorter than three: a retry succeeds.
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        other.execute_batch("COMMIT").unwrap();
        other
    });
    store
        .write("test.retry", |tx| {
            insert_project(tx, "TM")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let other = release.join().unwrap();
    let stats = store.writer_stats();
    assert!(stats.busy_retries >= 1, "{stats:?}");
    assert_eq!(stats.busy_failures, 0);

    // Held throughout: every retry fails and the caller gets `Busy`.
    other.execute_batch("BEGIN IMMEDIATE").unwrap();
    let before = store.writer_stats().busy_retries;
    let err = store
        .write("test.busy", |tx| {
            insert_project(tx, "XY")?;
            Ok::<_, StoreError>(())
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::Busy), "{err}");
    let stats = store.writer_stats();
    assert_eq!(stats.busy_retries - before, 2);
    assert_eq!(stats.busy_failures, 1);
    other.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn slow_write_transactions_are_recorded_with_their_operation() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    store
        .write("test.quick", |_| Ok::<_, StoreError>(()))
        .unwrap();
    assert_eq!(store.writer_stats().slow_commits, 0);
    store
        .write("test.slow", |_| {
            thread::sleep(Duration::from_millis(70));
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let stats = store.writer_stats();
    assert_eq!(stats.slow_commits, 1);
    let slow = stats.last_slow.unwrap();
    assert_eq!(slow.op, "test.slow");
    assert!(slow.held >= Duration::from_millis(70));
    assert!(stats.longest_hold >= slow.held);
}

#[test]
fn schema_upgraded_while_running_stops_writes() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    store.write("test.ok", |_| Ok::<_, StoreError>(())).unwrap();
    let newer = open_raw(&layout.database()).unwrap();
    newer
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    let err = store
        .write("test.after", |_| Ok::<_, StoreError>(()))
        .unwrap_err();
    assert!(
        matches!(err, StoreError::WorkspaceUpgraded { found, expected } if found == SCHEMA_VERSION + 1 && expected == SCHEMA_VERSION),
        "{err}"
    );
    store.close();

    // A newer database is refused on open and names the version.
    let err = Store::open(&layout, StoreConfig::default()).unwrap_err();
    assert!(
        matches!(err, StoreError::TooNew { found, .. } if found == SCHEMA_VERSION + 1),
        "{err}"
    );
    assert!(err.to_string().contains(&(SCHEMA_VERSION + 1).to_string()));
    assert_eq!(
        Store::probe(&layout).unwrap(),
        SchemaState::Newer(SCHEMA_VERSION + 1)
    );
}

#[test]
fn a_foreign_database_is_refused() {
    let (_dir, layout) = workspace();
    layout.create_dirs().unwrap();
    let conn = open_raw(&layout.database()).unwrap();
    conn.execute_batch("CREATE TABLE something (x)").unwrap();
    drop(conn);
    let err = Store::open(&layout, StoreConfig::default()).unwrap_err();
    assert!(matches!(err, StoreError::NotAWorkspace(_)), "{err}");
}

#[test]
fn watcher_reports_other_connections_and_skips_own_groups() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let other = open(&layout); // stands in for another process
    let mut watcher = store.watch_changes(None).unwrap();
    assert_eq!(watcher.poll().unwrap(), None, "nothing committed yet");

    // Another process's command.
    let theirs = Id::generate();
    other
        .write("task.create", move |tx| {
            insert_activity(tx, theirs, "task.create")?;
            insert_activity(tx, theirs, "task.create")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    // This process's own command, registered before its commit.
    let ours = Id::generate();
    store.own_groups().insert(ours);
    store
        .write("task.move", move |tx| {
            insert_activity(tx, ours, "task.move")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();

    let batch = watcher.poll().unwrap().unwrap();
    assert_eq!(batch.records.len(), 2);
    assert!(batch.records.iter().all(|r| r.group_id == theirs));
    assert!(batch.records.windows(2).all(|w| w[0].seq < w[1].seq));
    assert_eq!(batch.last_seq, 3, "own rows are skipped but counted");
    assert_eq!(watcher.poll().unwrap(), None);

    // The background poller delivers a later change.
    let (sender, received) = mpsc::channel();
    let handle = watcher
        .spawn(Duration::from_millis(20), move |batch| {
            let _ = sender.send(batch);
        })
        .unwrap();
    let later = Id::generate();
    let committed = Instant::now();
    other
        .write("task.update", move |tx| {
            insert_activity(tx, later, "task.update")?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let batch = received
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert!(committed.elapsed() < Duration::from_millis(500));
    assert_eq!(batch.records[0].group_id, later);
    assert_eq!(batch.records[0].op, "task.update");
    drop(handle);
}

#[test]
fn watcher_reads_a_backlog_in_chunks() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let mut watcher = store.watch_changes(Some(0)).unwrap();
    let group = Id::generate();
    let total = gk_store::MAX_ROWS_PER_POLL + 10;
    store
        .write("bulk", move |tx| {
            for _ in 0..total {
                insert_activity(tx, group, "task.bulk_update")?;
            }
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let first = watcher.poll().unwrap().unwrap();
    assert_eq!(first.records.len(), gk_store::MAX_ROWS_PER_POLL);
    let second = watcher.poll().unwrap().unwrap();
    assert_eq!(second.records.len(), 10);
    assert_eq!(watcher.last_seq(), total as i64);
}

#[test]
fn checkpoint_truncates_the_log_and_never_stalls_writers_for_long() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let checkpointer = store.checkpointer().unwrap();
    let ids = store
        .write("seed", |tx| Ok::<_, StoreError>(insert_project(tx, "TM")?))
        .unwrap();
    for n in 1..=50 {
        store
            .write("task.create", move |tx| {
                insert_task(tx, ids, n, &format!("Task {n}"))?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
    }
    assert!(checkpointer.wal_bytes() > 0);
    assert!(matches!(
        checkpointer.truncate().unwrap(),
        CheckpointOutcome::Truncated { .. }
    ));
    assert_eq!(checkpointer.wal_bytes(), 0);

    // A long reader on an old snapshot, and new writes after it.
    let (started, reader_started) = mpsc::channel();
    let (finish, reader_finish) = mpsc::channel::<()>();
    let store = &store;
    let checkpointer = thread::scope(|s| {
        s.spawn(move || {
            store
                .read(|tx| {
                    let _: i64 = tx.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))?;
                    started.send(()).unwrap();
                    reader_finish.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok::<_, StoreError>(())
                })
                .unwrap();
        });
        reader_started.recv().unwrap();
        for n in 51..=60 {
            store
                .write("task.create", move |tx| {
                    insert_task(tx, ids, n, &format!("Task {n}"))?;
                    Ok::<_, StoreError>(())
                })
                .unwrap();
        }
        // The checkpoint can't finish while the reader holds its snapshot; a
        // writer meanwhile waits at most about the checkpoint's budget.
        let checkpoint = s.spawn(move || {
            let started = Instant::now();
            let outcome = checkpointer.truncate().unwrap();
            assert_eq!(outcome, CheckpointOutcome::Busy);
            assert!(
                started.elapsed() < Duration::from_millis(500),
                "{:?}",
                started.elapsed()
            );
            checkpointer
        });
        thread::sleep(Duration::from_millis(20));
        let started = Instant::now();
        store
            .write("task.create", move |tx| {
                insert_task(tx, ids, 61, "During the checkpoint")?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "{:?}",
            started.elapsed()
        );
        finish.send(()).unwrap();
        checkpoint.join().unwrap()
    });

    // With the reader gone, the schedule truncates once the log is big
    // enough, and the idle rule catches a small one.
    let mut schedule = CheckpointSchedule::new(CheckpointPolicy {
        threshold_bytes: 1,
        ..CheckpointPolicy::default()
    });
    let outcome = schedule
        .tick(Instant::now(), Duration::ZERO, &checkpointer)
        .unwrap();
    assert!(
        matches!(outcome, Some(CheckpointOutcome::Truncated { .. })),
        "{outcome:?}"
    );
    assert_eq!(checkpointer.wal_bytes(), 0);
    assert_eq!(
        schedule
            .tick(Instant::now(), Duration::ZERO, &checkpointer)
            .unwrap(),
        None,
        "not due again before the check interval"
    );
}

#[test]
fn repeated_requests_get_the_first_answer_for_a_day() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let key = RequestKey {
        op: "task.create",
        caller: "cli:You",
        request_id: "r-1",
    };
    let other = RequestKey {
        caller: "session-2",
        ..key
    };
    store
        .write("test", move |tx| {
            assert_eq!(remembered(tx, key, 1_000)?, None);
            remember(tx, key, r#"{"key":"TM-1"}"#, 1_000)?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    // Another connection, as another process would see it.
    let second = open(&layout);
    second
        .write("test", move |tx| {
            assert_eq!(
                remembered(tx, key, 2_000)?.as_deref(),
                Some(r#"{"key":"TM-1"}"#)
            );
            assert_eq!(remembered(tx, other, 2_000)?, None);
            assert_eq!(remembered(tx, key, 1_000 + REQUEST_WINDOW_MS)?, None);
            // A new answer purges the expired one.
            remember(tx, other, "{}", 1_000 + REQUEST_WINDOW_MS)?;
            let rows: i64 = tx.query_row("SELECT count(*) FROM request_log", [], |r| r.get(0))?;
            assert_eq!(rows, 1);
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

#[test]
fn wiki_page_names_are_unique_ignoring_case() {
    let (_dir, layout) = workspace();
    let store = open(&layout);
    let result = store.write("test", |tx| {
        let (project, _, _) = insert_project(tx, "WK")?;
        let page = |title: &str, slug: &str, deleted: Option<i64>| {
            tx.execute(
                "INSERT INTO wiki_pages (id, project_id, rank, title, title_fold, slug,
                                         created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, 'i', ?3, ?4, ?5, 0, 0, ?6)",
                params![
                    Id::generate().as_bytes(),
                    project.as_bytes(),
                    title,
                    fold(title),
                    slug,
                    deleted
                ],
            )
        };
        page("Setup", "setup", Some(1))?;
        page("SETUP", "setup-2", None)?;
        let clash = page("setup", "setup-3", None);
        assert!(clash.is_err(), "same name in another case");
        Ok::<_, StoreError>(())
    });
    result.unwrap();
}
