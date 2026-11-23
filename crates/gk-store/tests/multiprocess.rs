// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Several processes writing one workspace at once, one of them killed in
//! the middle of its work, while this process watches for their changes.
//!
//! The writers are this test binary started again with an environment
//! variable that makes `child_writer` do the work. Each commit bumps a
//! counter by read-modify-write and appends three history rows for one
//! group, then prints the group's ID as an acknowledgement.
//!
//! Afterwards: the database passes `integrity_check`; every acknowledged
//! commit is there in full; no commit is there in part; the counter equals
//! the number of commits (no lost updates); the history sequence has no
//! gaps; and the watcher saw every row, ending on the highest sequence
//! number.

// Helpers outside `#[test]` functions unwrap too; a failure is a test failure.
#![allow(clippy::unwrap_used)]

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, params};
use gk_store::{Store, StoreConfig, StoreError, WorkspaceLayout, open_raw};

const CHILD: &str = "GK_STORE_TEST_CHILD";
const ROWS_PER_COMMIT: i64 = 3;

/// The writer process. Does nothing unless started by the test below.
#[test]
fn child_writer() {
    let Ok(spec) = std::env::var(CHILD) else {
        return;
    };
    let (root, commits) = spec.rsplit_once('|').unwrap();
    let commits: u32 = commits.parse().unwrap();
    let store = Store::open(&WorkspaceLayout::new(root), StoreConfig::default()).unwrap();
    let mut out = std::io::stdout().lock();
    for _ in 0..commits {
        let group = Id::generate();
        store
            .write("test.commit", move |tx| {
                let counter: i64 = tx
                    .query_row(
                        "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'test_counter'",
                        [],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or(0);
                tx.execute(
                    "INSERT INTO meta (key, value) VALUES ('test_counter', ?1)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    [(counter + 1).to_string()],
                )?;
                for _ in 0..ROWS_PER_COMMIT {
                    tx.execute(
                        "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via,
                                               entity_kind, entity_id, changes)
                         VALUES (?1, 0, 'test.commit', 'agent', 'Writer', 'mcp', 'task', ?2, '{}')",
                        params![group.as_bytes(), Id::generate().as_bytes()],
                    )?;
                }
                Ok::<_, StoreError>(())
            })
            .unwrap();
        writeln!(out, "A {group}").unwrap();
        out.flush().unwrap();
    }
}

/// Starts a writer process; its acknowledgements arrive on the returned
/// channel.
fn spawn_writer(root: &std::path::Path, commits: u32) -> (Child, mpsc::Receiver<Id>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_writer", "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("{}|{commits}", root.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (acks, received) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(id) = line.strip_prefix("A ") {
                let _ = acks.send(id.parse().unwrap());
            }
        }
    });
    (child, received)
}

#[test]
fn processes_share_the_workspace_safely() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Shared"));
    let store = Store::open(&layout, StoreConfig::default()).unwrap();

    // The app's poller.
    let (seen_tx, seen) = mpsc::channel();
    let watcher = store
        .watch_changes(None)
        .unwrap()
        .spawn(Duration::from_millis(25), move |batch| {
            for record in batch.unwrap().records {
                let _ = seen_tx.send((record.seq, record.group_id));
            }
        })
        .unwrap();

    let (mut a, acks_a) = spawn_writer(layout.root(), 40);
    let (mut b, acks_b) = spawn_writer(layout.root(), 40);
    let (mut victim, acks_victim) = spawn_writer(layout.root(), 100_000);

    // Kill the victim once it is busy writing.
    acks_victim.recv_timeout(Duration::from_secs(20)).unwrap();
    thread::sleep(Duration::from_millis(150));
    victim.kill().unwrap();
    victim.wait().unwrap();
    assert!(a.wait().unwrap().success());
    assert!(b.wait().unwrap().success());

    let mut acked: Vec<Id> = Vec::new();
    for acks in [&acks_a, &acks_b, &acks_victim] {
        acked.extend(acks.try_iter());
    }
    // `acks_victim` lost the one consumed above; it is in the database too.
    assert!(acked.len() >= 80, "{}", acked.len());

    // Let the poller catch up, then stop it.
    let conn = open_raw(&layout.database()).unwrap();
    let max_seq: i64 = conn
        .query_row("SELECT max(seq) FROM activity", [], |r| r.get(0))
        .unwrap();
    let mut seen_seqs = BTreeSet::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen_seqs.last() != Some(&max_seq) && Instant::now() < deadline {
        if let Ok((seq, _)) = seen.recv_timeout(Duration::from_millis(100)) {
            seen_seqs.insert(seq);
        }
    }
    drop(watcher);
    seen_seqs.extend(seen.try_iter().map(|(seq, _)| seq));

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");

    let mut rows_per_group: HashMap<Id, i64> = HashMap::new();
    let mut stmt = conn.prepare("SELECT group_id FROM activity").unwrap();
    for group in stmt.query_map([], |r| r.get::<_, Vec<u8>>(0)).unwrap() {
        *rows_per_group
            .entry(Id::from_slice(&group.unwrap()).unwrap())
            .or_default() += 1;
    }
    for group in &acked {
        assert!(
            rows_per_group.contains_key(group),
            "acknowledged commit {group} is missing"
        );
    }
    assert!(
        rows_per_group.values().all(|&n| n == ROWS_PER_COMMIT),
        "a commit is there only in part"
    );

    let counter: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'test_counter'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(counter, rows_per_group.len() as i64, "lost updates");

    let (count, min_seq): (i64, i64) = conn
        .query_row("SELECT count(*), min(seq) FROM activity", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((min_seq, count), (1, max_seq), "the sequence has gaps");

    assert_eq!(seen_seqs.len() as i64, max_seq, "the watcher missed rows");
    assert_eq!(seen_seqs.last(), Some(&max_seq));
}
