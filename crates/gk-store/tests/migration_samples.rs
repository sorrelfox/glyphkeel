// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Workspaces written by earlier releases, upgraded by this program.
//!
//! Each file in `samples/` is the database of a workspace that a released
//! `glyphkeel` wrote, as SQL. `cargo xtask migration-samples` makes them by
//! running a fixed set of commands with that release's program and dumping
//! the result with [`write_sample`] below. They are never edited or
//! regenerated: they stand for the workspaces people have.
//!
//! Every sample must upgrade to the current schema and keep its data: the
//! database stays intact, no row is lost, and the upgraded schema is the one
//! a new workspace gets.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use gk_store::rusqlite::{Connection, types::ValueRef};
use gk_store::{SCHEMA_VERSION, SchemaState, Store, StoreConfig, StoreError, WorkspaceLayout};

/// A sample file: the release that wrote it, its schema version and its SQL.
#[derive(Debug)]
struct Sample {
    file: PathBuf,
    release: String,
    schema: i64,
    sql: String,
}

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples")
}

/// Every sample, oldest release first.
fn samples() -> Vec<Sample> {
    let mut found: Vec<Sample> = fs::read_dir(samples_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "sql"))
        .map(|file| {
            let sql = fs::read_to_string(&file).unwrap();
            let field = |name: &str| {
                sql.lines()
                    .find_map(|line| line.strip_prefix(&format!("-- {name}: ")))
                    .unwrap_or_else(|| panic!("{} has no `-- {name}:` line", file.display()))
                    .trim()
                    .to_owned()
            };
            let release = field("release");
            let schema = field("schema").parse().unwrap();
            Sample {
                file,
                release,
                schema,
                sql,
            }
        })
        .collect();
    found.sort_by_key(|s| version_key(&s.release));
    found
}

fn version_key(version: &str) -> Vec<u64> {
    version.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Restores `sample` as the database of a workspace in `dir`.
fn restore(sample: &Sample, dir: &Path) -> WorkspaceLayout {
    let layout = WorkspaceLayout::new(dir.join("Sample"));
    layout.create_dirs().unwrap();
    let conn = Connection::open(layout.database()).unwrap();
    conn.execute_batch(&sample.sql)
        .unwrap_or_else(|e| panic!("{}: {e}", sample.file.display()));
    drop(conn);
    layout
}

/// The schema's tables, indexes, triggers and views, by name.
fn schema_of(conn: &Connection) -> BTreeMap<String, (String, Option<String>)> {
    let mut stmt = conn
        .prepare("SELECT name, type, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The row count of every ordinary table.
fn row_counts(conn: &Connection) -> BTreeMap<String, i64> {
    ordinary_tables(conn)
        .into_iter()
        .map(|table| {
            let n = conn
                .query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |r| {
                    r.get(0)
                })
                .unwrap();
            (table, n)
        })
        .collect()
}

/// Tables that hold rows of their own: not virtual tables, not their
/// shadow tables and not SQLite's own.
fn ordinary_tables(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, sql FROM sqlite_schema WHERE type = 'table'
              AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
        )
        .unwrap();
    let all: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let virtual_tables: Vec<&str> = all
        .iter()
        .filter(|(_, sql)| sql.starts_with("CREATE VIRTUAL TABLE"))
        .map(|(name, _)| name.as_str())
        .collect();
    all.iter()
        .filter(|(name, sql)| {
            !sql.starts_with("CREATE VIRTUAL TABLE")
                && !virtual_tables
                    .iter()
                    .any(|v| name.starts_with(&format!("{v}_")))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

#[test]
fn there_is_a_sample_of_every_released_schema_up_to_this_one() {
    let samples = samples();
    assert!(
        !samples.is_empty(),
        "no samples in {}",
        samples_dir().display()
    );
    for sample in &samples {
        assert!(
            sample.schema <= SCHEMA_VERSION,
            "{} has schema {}, newer than this program's {SCHEMA_VERSION}",
            sample.file.display(),
            sample.schema
        );
        let name = sample.file.file_stem().unwrap().to_string_lossy();
        assert_eq!(name, sample.release, "a sample is named after its release");
    }
}

#[test]
fn every_sample_upgrades_and_keeps_its_data() {
    let fresh_dir = tempfile::tempdir().unwrap();
    let fresh = WorkspaceLayout::new(fresh_dir.path().join("New"));
    drop(Store::open(&fresh, StoreConfig::default()).unwrap());
    let new_schema = schema_of(&Connection::open(fresh.database()).unwrap());

    for sample in samples() {
        let name = sample.file.display().to_string();
        let dir = tempfile::tempdir().unwrap();
        let layout = restore(&sample, dir.path());

        let before = {
            let conn = Connection::open(layout.database()).unwrap();
            let version: i64 = conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, sample.schema, "{name}: the header's schema");
            row_counts(&conn)
        };
        let older = sample.schema < SCHEMA_VERSION;
        assert_eq!(
            Store::probe(&layout).unwrap(),
            if older {
                SchemaState::Older(sample.schema)
            } else {
                SchemaState::Current
            },
            "{name}"
        );
        if older {
            // Programs that may not upgrade leave it alone.
            assert!(
                matches!(
                    Store::open(&layout, StoreConfig::default()),
                    Err(StoreError::NeedsUpgrade { found, current })
                        if found == sample.schema && current == SCHEMA_VERSION
                ),
                "{name}"
            );
        }

        let config = StoreConfig {
            allow_upgrade: true,
            ..StoreConfig::default()
        };
        let store = Store::open(&layout, config).unwrap_or_else(|e| panic!("{name}: {e}"));
        drop(store);

        let conn = Connection::open(layout.database()).unwrap();
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok", "{name}");
        let broken: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0, "{name}: rows breaking a foreign key");
        assert_eq!(
            schema_of(&conn),
            new_schema,
            "{name}: the upgraded schema differs from a new workspace's"
        );

        let after = row_counts(&conn);
        let mut lost = String::new();
        for (table, count) in &before {
            if let Some(now) = after.get(table)
                && now < count
            {
                let _ = writeln!(lost, "{table}: {count} rows before, {now} after");
            }
        }
        assert!(lost.is_empty(), "{name} lost rows:\n{lost}");
        for table in ["projects", "tasks", "comments", "time_entries", "activity"] {
            assert!(
                before.get(table).copied().unwrap_or(0) > 0,
                "{name}: the sample has no rows in {table}"
            );
        }

        // The search index was rebuilt from its documents and finds them.
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM fts_words WHERE fts_words MATCH 'garden'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(found > 0, "{name}: the search index finds nothing");
    }
}

/// Writes the sample of a workspace an earlier release made. Run by
/// `cargo xtask migration-samples`, which sets the variables; it does nothing
/// on its own.
#[test]
#[ignore = "a tool for `cargo xtask migration-samples`"]
fn write_sample() {
    let (Ok(database), Ok(out), Ok(release)) = (
        std::env::var("GK_SAMPLE_DB"),
        std::env::var("GK_SAMPLE_OUT"),
        std::env::var("GK_SAMPLE_RELEASE"),
    ) else {
        eprintln!("GK_SAMPLE_DB, GK_SAMPLE_OUT and GK_SAMPLE_RELEASE are not set");
        return;
    };
    let conn = Connection::open(&database).unwrap();
    let sql = dump(&conn, &release);
    fs::write(&out, sql).unwrap();
}

/// The database on `conn` as SQL that recreates it: the tables, their rows,
/// then the indexes and triggers (so no trigger runs while the rows go in),
/// and the full-text indexes rebuilt from their content tables.
fn dump(conn: &Connection, release: &str) -> String {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- SPDX-License-Identifier: GPL-3.0-or-later\n\
         -- Copyright (C) 2026 Glyphkeel contributors\n\
         --\n\
         -- A Glyphkeel workspace written by glyphkeel {release}.\n\
         -- release: {release}\n\
         -- schema: {version}\n\
         -- Made by `cargo xtask migration-samples`. Never edit or regenerate it:\n\
         -- it stands for the workspaces this release left on people's disks."
    );
    out.push_str("PRAGMA foreign_keys = OFF;\nBEGIN;\n");

    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
              WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
        )
        .unwrap();
    let objects: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let ordinary = ordinary_tables(conn);
    let virtual_tables: Vec<&str> = objects
        .iter()
        .filter(|(kind, _, sql)| kind == "table" && sql.starts_with("CREATE VIRTUAL TABLE"))
        .map(|(_, name, _)| name.as_str())
        .collect();

    for (kind, name, sql) in &objects {
        if kind == "table" && (ordinary.contains(name) || virtual_tables.contains(&name.as_str())) {
            let _ = writeln!(out, "{sql};");
        }
    }
    for table in &ordinary {
        let mut rows = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
        let columns = rows.column_count();
        let mut cursor = rows.query([]).unwrap();
        while let Some(row) = cursor.next().unwrap() {
            let values: Vec<String> = (0..columns)
                .map(|i| literal(row.get_ref(i).unwrap()))
                .collect();
            let _ = writeln!(out, "INSERT INTO \"{table}\" VALUES({});", values.join(","));
        }
    }
    for (kind, _, sql) in &objects {
        if kind != "table" {
            let _ = writeln!(out, "{sql};");
        }
    }
    for table in &virtual_tables {
        let _ = writeln!(
            out,
            "INSERT INTO \"{table}\"(\"{table}\") VALUES('rebuild');"
        );
    }
    let _ = writeln!(out, "PRAGMA user_version = {version};\nCOMMIT;");
    out
}

fn literal(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => format!("{f:?}"),
        ValueRef::Text(t) => format!("'{}'", String::from_utf8_lossy(t).replace('\'', "''")),
        ValueRef::Blob(b) => {
            let mut hex = String::with_capacity(b.len() * 2 + 3);
            hex.push_str("x'");
            for byte in b {
                let _ = write!(hex, "{byte:02x}");
            }
            hex.push('\'');
            hex
        }
    }
}
