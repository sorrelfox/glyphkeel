// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Schema versions and migrations.
//!
//! The schema version is SQLite's `user_version`. Each migration is a
//! numbered SQL file in `migrations/`, compiled into the program; they run
//! forward only, in order, each in its own write transaction that also sets
//! the new version, so a crash leaves the database at the last complete
//! version. A version-0 file with no tables is a new workspace; version 0
//! with tables is some other database.
//!
//! A program refuses a database newer than it knows. An older one is
//! upgraded only when the caller allows it (the app does, after a backup);
//! otherwise opening fails with [`StoreError::NeedsUpgrade`].

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::{Result, StoreError};

/// The schema version this program is built for.
pub const SCHEMA_VERSION: i64 = 18;

/// One migration: the SQL that takes the schema from `version - 1` to
/// `version`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) sql: &'static str,
}

/// Every migration, in order. The last one's version is [`SCHEMA_VERSION`].
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_requests_and_names.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../migrations/0003_subtask_order.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../migrations/0004_time_and_reminders.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../migrations/0005_project_icons.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../migrations/0006_wiki_conflicts.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("../migrations/0007_personal_drafts.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("../migrations/0008_text_links.sql"),
    },
    Migration {
        version: 9,
        sql: include_str!("../migrations/0009_wiki_home.sql"),
    },
    Migration {
        version: 10,
        sql: include_str!("../migrations/0010_wiki_templates.sql"),
    },
    Migration {
        version: 11,
        sql: include_str!("../migrations/0011_custom_fields.sql"),
    },
    Migration {
        version: 12,
        sql: include_str!("../migrations/0012_versions.sql"),
    },
    Migration {
        version: 13,
        sql: include_str!("../migrations/0013_status_and_agent_rules.sql"),
    },
    Migration {
        version: 14,
        sql: include_str!("../migrations/0014_recurrence_and_templates.sql"),
    },
    Migration {
        version: 15,
        sql: include_str!("../migrations/0015_lag_wip_minimum_and_kinds_of_work.sql"),
    },
    Migration {
        version: 16,
        sql: include_str!("../migrations/0016_epic_and_story_glyphs.sql"),
    },
    Migration {
        version: 17,
        sql: include_str!("../migrations/0017_card_covers.sql"),
    },
    Migration {
        version: 18,
        sql: include_str!("../migrations/0018_sprints.sql"),
    },
];

/// The schema of a database file compared with this program's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    /// A new, empty file.
    Empty,
    /// The version this program is built for.
    Current,
    /// An older version, which needs migrating.
    Older(i64),
    /// A newer version, which this program must not touch.
    Newer(i64),
}

/// Reads the schema version of the database on `conn`.
pub(crate) fn state(conn: &Connection, current: i64) -> Result<SchemaState> {
    let version = user_version(conn)?;
    Ok(match version {
        0 => {
            let objects: i64 =
                conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
            if objects == 0 {
                SchemaState::Empty
            } else {
                return Err(StoreError::NotAWorkspace(
                    conn.path().unwrap_or_default().into(),
                ));
            }
        }
        v if v == current => SchemaState::Current,
        v if v < current => SchemaState::Older(v),
        v => SchemaState::Newer(v),
    })
}

/// The `user_version` of the database on `conn`.
pub(crate) fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// Runs every migration in `migrations` above the database's version.
/// `init` runs inside the transaction that creates the schema of a new
/// workspace (from version 0), to fill in its first rows. Returns the version
/// the database had before.
///
/// Safe against another process migrating at the same time: each step
/// re-reads the version after taking the write lock and skips what is
/// already done. Callers still hold the migrate lock so only one process does
/// the work.
pub(crate) fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    init: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<()>,
) -> Result<i64> {
    let before = user_version(conn)?;
    if let Some(last) = migrations.last()
        && before > last.version
    {
        return Err(StoreError::TooNew {
            found: before,
            supported: last.version,
        });
    }
    let mut init = Some(init);
    for migration in migrations {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = user_version(&tx)?;
        if version >= migration.version {
            continue;
        }
        if version != migration.version - 1 {
            return Err(StoreError::Corrupt(format!(
                "schema version {version} cannot be upgraded by migration {}",
                migration.version
            )));
        }
        let run_init = if version == 0 { init.take() } else { None };
        let step = (|| {
            tx.execute_batch(migration.sql)?;
            tx.pragma_update(None, "user_version", migration.version)?;
            if let Some(init) = run_init {
                init(&tx)?;
            }
            check_foreign_keys(&tx)
        })();
        step.map_err(|source| StoreError::Migration {
            version: migration.version,
            source,
        })?;
        tx.commit()?;
        tracing::info!(version = migration.version, "workspace schema upgraded");
    }
    Ok(before)
}

/// Fails if any row breaks a foreign key: a migration must leave none.
fn check_foreign_keys(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let table: String = row.get(0)?;
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
            Some(format!("a row of {table} breaks a foreign key")),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            sql: "CREATE TABLE a (x INTEGER) STRICT;",
        },
        Migration {
            version: 2,
            sql: "ALTER TABLE a ADD COLUMN y TEXT;",
        },
    ];

    fn memory() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn empty_then_current() {
        let mut conn = memory();
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Empty);
        let before = migrate(&mut conn, TEST_MIGRATIONS, |tx| {
            tx.execute("INSERT INTO a (x) VALUES (7)", []).map(|_| ())
        })
        .unwrap();
        assert_eq!(before, 0);
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Current);
        let x: i64 = conn.query_row("SELECT x FROM a", [], |r| r.get(0)).unwrap();
        assert_eq!(x, 7);
    }

    #[test]
    fn older_is_upgraded_without_init() {
        let mut conn = memory();
        migrate(&mut conn, &TEST_MIGRATIONS[..1], |_| Ok(())).unwrap();
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Older(1));
        let before = migrate(&mut conn, TEST_MIGRATIONS, |_| {
            panic!("init runs only for new workspaces")
        })
        .unwrap();
        assert_eq!(before, 1);
        assert_eq!(user_version(&conn).unwrap(), 2);
        conn.execute("INSERT INTO a (x, y) VALUES (1, 'b')", [])
            .unwrap();
    }

    #[test]
    fn newer_is_refused() {
        let mut conn = memory();
        migrate(&mut conn, TEST_MIGRATIONS, |_| Ok(())).unwrap();
        assert_eq!(state(&conn, 1).unwrap(), SchemaState::Newer(2));
        assert!(matches!(
            migrate(&mut conn, &TEST_MIGRATIONS[..1], |_| Ok(())),
            Err(StoreError::TooNew {
                found: 2,
                supported: 1
            })
        ));
    }

    #[test]
    fn a_failed_migration_keeps_nothing() {
        let mut conn = memory();
        let broken = [
            TEST_MIGRATIONS[0],
            Migration {
                version: 2,
                sql: "CREATE TABLE b (x INTEGER); THIS IS NOT SQL;",
            },
        ];
        let err = migrate(&mut conn, &broken, |_| Ok(())).unwrap_err();
        assert!(
            matches!(err, StoreError::Migration { version: 2, .. }),
            "{err}"
        );
        assert_eq!(user_version(&conn).unwrap(), 1);
        let b: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'b'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(b, 0);
    }

    #[test]
    fn a_foreign_database_is_not_a_workspace() {
        let conn = memory();
        conn.execute_batch("CREATE TABLE other (x)").unwrap();
        assert!(matches!(state(&conn, 1), Err(StoreError::NotAWorkspace(_))));
    }

    #[test]
    fn the_real_schema_applies_cleanly() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(MIGRATIONS.last().unwrap().version, SCHEMA_VERSION);
        assert!(
            MIGRATIONS
                .windows(2)
                .all(|w| w[1].version == w[0].version + 1)
        );
    }

    #[test]
    fn version_4_keeps_the_latest_reminder_and_the_running_timer() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        migrate(&mut conn, &MIGRATIONS[..3], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01000000000000000000000000000000', 'i', 'TM', 'TM', 'tm', 'blue',
                     '{}', 0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'02000000000000000000000000000000', x'01000000000000000000000000000000',
                     'To do', 'to do', 'todo', 'blue', 'i');
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
             VALUES (x'03000000000000000000000000000000', x'01000000000000000000000000000000',
                     'Task', 'task', 'task', 'blue', 'i', 1);
             INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                                status_entered_at, type_id, rank, created_at, created_by_kind,
                                created_by_name, edited_at, updated_at)
             VALUES (x'04000000000000000000000000000000', x'01000000000000000000000000000000',
                     1, 'A', 'a', x'02000000000000000000000000000000', 0,
                     x'03000000000000000000000000000000', 'i', 0, 'user', 'Me', 0, 0);
             INSERT INTO reminders (id, task_id, remind_at, fired_at)
             VALUES (x'05000000000000000000000000000000', x'04000000000000000000000000000000',
                     1000, 1500),
                    (x'06000000000000000000000000000000', x'04000000000000000000000000000000',
                     2000, NULL);
             INSERT INTO timer (id, task_id, started_at, actor_kind, actor_name)
             VALUES (1, x'04000000000000000000000000000000', 777, 'user', 'Me');",
        )
        .unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        let kept: (i64, Option<i64>) = conn
            .query_row("SELECT remind_at, acked_at FROM reminders", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(kept, (2000, None));
        let timer: (i64, i64) = conn
            .query_row("SELECT running_since, elapsed_ms FROM timer", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(timer, (777, 0));
        let both = conn.execute(
            "INSERT INTO reminders (id, task_id, remind_at, days_before_due, actor_kind,
                                    actor_name, created_at)
             VALUES (x'07000000000000000000000000000000', x'04000000000000000000000000000000',
                     1, 1, 'user', 'Me', 0)",
            [],
        );
        assert!(both.is_err(), "a moment and days before due together");
    }

    #[test]
    fn version_5_keeps_known_project_icons_and_drops_the_rest() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        migrate(&mut conn, &MIGRATIONS[..4], |_| Ok(())).unwrap();
        let project = |n: u8, key: &str, icon: Option<&str>| {
            conn.execute(
                "INSERT INTO projects (id, rank, key, name, name_fold, hue, icon, modules,
                                       created_at, updated_at)
                 VALUES (?1, 'i', ?2, ?2, lower(?2), 'blue', ?3, '{}', 0, 0)",
                rusqlite::params![[n; 16].to_vec(), key, icon],
            )
            .unwrap();
        };
        project(1, "AA", Some("rocket"));
        project(2, "BB", Some("Rocket"));
        project(3, "CC", Some(""));
        project(4, "DD", Some("my-own-icon"));
        project(5, "EE", None);
        project(6, "FF", Some("graduation-cap"));
        conn.execute_batch(
            "INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'10000000000000000000000000000000', x'01010101010101010101010101010101',
                     'To do', 'to do', 'todo', 'blue', 'i');
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
             VALUES (x'11000000000000000000000000000000', x'01010101010101010101010101010101',
                     'Task', 'task', 'task', 'blue', 'i', 1);
             INSERT INTO labels (id, project_id, name, name_fold, hue, rank)
             VALUES (x'12000000000000000000000000000000', x'01010101010101010101010101010101',
                     'ui', 'ui', 'teal', 'i');
             INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                                status_entered_at, type_id, rank, created_at, created_by_kind,
                                created_by_name, edited_at, updated_at)
             VALUES (x'13000000000000000000000000000000', x'01010101010101010101010101010101',
                     1, 'A', 'a', x'10000000000000000000000000000000', 0,
                     x'11000000000000000000000000000000', 'i', 0, 'user', 'Me', 0, 0);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..5], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 5);
        let icons: Vec<(String, Option<String>)> = conn
            .prepare("SELECT key, icon FROM projects ORDER BY key")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let expected: Vec<(String, Option<String>)> = [
            ("AA", Some("rocket")),
            ("BB", None),
            ("CC", None),
            ("DD", None),
            ("EE", None),
            ("FF", Some("graduation-cap")),
        ]
        .iter()
        .map(|(k, i)| ((*k).to_owned(), i.map(str::to_owned)))
        .collect();
        assert_eq!(icons, expected);
        let rows: (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM tasks), (SELECT count(*) FROM labels),
                        (SELECT count(*) FROM statuses)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(rows, (1, 1, 1), "nothing else changes");
    }

    #[test]
    fn version_6_keeps_wiki_revisions_and_leaves_them_unresolved() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..5], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'WK', 'WK', 'wk', 'blue', '{}',
                     0, 0);
             INSERT INTO wiki_pages (id, project_id, rank, title, title_fold, slug, created_at,
                                     updated_at)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'i', 'Plan', 'plan', 'plan', 0, 0);
             INSERT INTO wiki_revisions (id, page_id, body, kind, actor_kind, actor_name,
                                         created_at)
             VALUES (x'03030303030303030303030303030303', x'02020202020202020202020202020202',
                     'Text', 'conflict', 'user', 'Me', 0);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..6], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 6);
        let row: (String, Option<i64>) = conn
            .query_row("SELECT body, resolved_at FROM wiki_revisions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(row, ("Text".to_owned(), None));
    }

    #[test]
    fn version_7_gives_drafts_to_the_person_and_keeps_one_per_person_and_editor() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..6], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO drafts (owner_kind, owner_id, title, body, base_rev, updated_at)
             VALUES ('task', x'01010101010101010101010101010101', NULL, 'Unsaved', NULL, 5),
                    ('new_comment', x'01010101010101010101010101010101', NULL, 'Hi', NULL, 6);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..7], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 7);
        let kept: Vec<(String, String, String, String, i64)> = conn
            .prepare(
                "SELECT actor_kind, actor_name, owner_kind, body, updated_at FROM drafts
                  ORDER BY updated_at",
            )
            .unwrap()
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let you = |kind: &str, body: &str, at: i64| {
            (
                "user".to_owned(),
                "You".to_owned(),
                kind.to_owned(),
                body.to_owned(),
                at,
            )
        };
        assert_eq!(
            kept,
            [you("task", "Unsaved", 5), you("new_comment", "Hi", 6)]
        );

        // Another person keeps a draft of the same editor beside it.
        conn.execute(
            "INSERT INTO drafts (actor_kind, actor_name, owner_kind, owner_id, body, updated_at)
             VALUES ('user', 'Robin', 'task', x'01010101010101010101010101010101', 'Mine', 7)",
            [],
        )
        .unwrap();
        let again = conn.execute(
            "INSERT INTO drafts (actor_kind, actor_name, owner_kind, owner_id, body, updated_at)
             VALUES ('user', 'You', 'task', x'01010101010101010101010101010101', 'Twice', 8)",
            [],
        );
        assert!(again.is_err(), "one draft per person and editor");
        let short = conn.execute(
            "INSERT INTO drafts (actor_kind, actor_name, owner_kind, owner_id, body, updated_at)
             VALUES ('user', 'You', 'project', x'01', 'Short', 8)",
            [],
        );
        assert!(short.is_err(), "the owner is an ID");
    }

    #[test]
    fn version_5_knows_every_project_icon() {
        let sql = MIGRATIONS[4].sql;
        let list = &sql[sql.find("NOT IN (").unwrap()..];
        let named: Vec<&str> = list.split('\'').skip(1).step_by(2).collect();
        let known: Vec<&str> = gk_core::ProjectIcon::ALL
            .iter()
            .map(|i| i.as_str())
            .collect();
        assert_eq!(named, known);
    }
    #[test]
    fn version_9_keeps_pages_and_allows_one_home_page_per_project() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..8], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'WK', 'WK', 'wk', 'blue', '{}',
                     0, 0);
             INSERT INTO wiki_pages (id, project_id, rank, title, title_fold, slug, created_at,
                                     updated_at)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'i', 'Plan', 'plan', 'plan', 0, 0),
                    (x'03030303030303030303030303030303', x'01010101010101010101010101010101',
                     'j', 'Home', 'home', 'home', 0, 0);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..9], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 9);
        let homes: i64 = conn
            .query_row("SELECT sum(is_home) FROM wiki_pages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(homes, 0);
        conn.execute("UPDATE wiki_pages SET is_home = 1 WHERE slug = 'home'", [])
            .unwrap();
        let second = conn.execute("UPDATE wiki_pages SET is_home = 1 WHERE slug = 'plan'", []);
        assert!(second.is_err(), "two home pages in one project");
    }

    #[test]
    fn version_10_marks_no_page_as_a_template() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..9], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'WK', 'WK', 'wk', 'blue', '{}',
                     0, 0);
             INSERT INTO wiki_pages (id, project_id, rank, title, title_fold, slug, created_at,
                                     updated_at)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'i', 'Plan', 'plan', 'plan', 0, 0);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..10], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 10);
        let marked: i64 = conn
            .query_row("SELECT sum(is_template) FROM wiki_pages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(marked, 0);
        let bad = conn.execute("UPDATE wiki_pages SET is_template = 2", []);
        assert!(bad.is_err(), "the mark is 0 or 1");
    }

    #[test]
    fn version_14_keeps_types_and_tasks_and_adds_series_and_templates() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..13], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'RT', 'RT', 'rt', 'blue', '{}',
                     0, 0);
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'Task', 'task', 'task', 'grey', 'i');",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..14], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 14);
        let offset: Option<i64> = conn
            .query_row("SELECT due_offset_days FROM task_types", [], |r| r.get(0))
            .unwrap();
        assert_eq!(offset, None, "existing types keep no due date");
        conn.execute("UPDATE task_types SET due_offset_days = 3", [])
            .unwrap();
        assert!(
            conn.execute("UPDATE task_types SET due_offset_days = -1", [])
                .is_err(),
            "the offset is not negative"
        );
        let insert = |kind: &str, project: &str, name: &str| {
            conn.execute(
                &format!(
                    "INSERT INTO templates (id, project_id, kind, name, name_fold, content, rank,
                                            created_at, created_by_kind, created_by_name,
                                            updated_at)
                     VALUES (randomblob(16), {project}, '{kind}', '{name}', lower('{name}'),
                             '{{}}', 'i', 0, 'user', 'You', 0)"
                ),
                [],
            )
        };
        let project = "x'01010101010101010101010101010101'";
        insert("task", project, "Bug").unwrap();
        assert!(insert("task", project, "bug").is_err(), "names are unique");
        assert!(
            insert("task", "NULL", "Loose").is_err(),
            "task templates have a project"
        );
        insert("project", "NULL", "Software").unwrap();
        assert!(insert("project", "NULL", "software").is_err());
        assert!(insert("project", project, "Inside").is_err());
        conn.execute(
            "INSERT INTO task_series (id, project_id, rule, pattern, created_at, created_by_kind,
                                      created_by_name, updated_at)
             VALUES (randomblob(16), x'01010101010101010101010101010101',
                     '{\"every\":\"week\"}', '{}', 0, 'agent', 'Helper', 0)",
            [],
        )
        .unwrap();
        assert!(
            conn.execute("UPDATE task_series SET rule = 'not json'", [])
                .is_err()
        );
    }

    #[test]
    fn version_15_adds_wip_minimums_and_kinds_of_work() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..14], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'RT', 'RT', 'rt', 'blue', '{}',
                     0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank,
                                   wip_limit)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'Doing', 'doing', 'in_progress', 'blue', 'i', 3);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..15], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 15);
        let (min, max): (Option<i64>, Option<i64>) = conn
            .query_row("SELECT wip_min, wip_limit FROM statuses", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            (min, max),
            (None, Some(3)),
            "columns keep their limit and no minimum"
        );
        conn.execute("UPDATE statuses SET wip_min = 1", []).unwrap();
        assert!(conn.execute("UPDATE statuses SET wip_min = 0", []).is_err());
        let kind = |name: &str| {
            conn.execute(
                "INSERT INTO work_kinds (id, name, name_fold, created_at, created_by_kind,
                                         created_by_name, updated_at)
                 VALUES (randomblob(16), ?1, lower(?1), 0, 'user', 'You', 0)",
                [name],
            )
        };
        kind("Design").unwrap();
        assert!(kind("design").is_err(), "names are unique");
        assert!(kind("").is_err(), "names are not empty");
    }

    #[test]
    fn version_16_keeps_types_and_their_tasks_and_allows_epics_and_stories() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..15], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'RT', 'RT', 'rt', 'blue', '{}',
                     0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'To do', 'to do', 'todo', 'blue', 'i');
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank,
                                     is_default, due_offset_days)
             VALUES (x'03030303030303030303030303030303', x'01010101010101010101010101010101',
                     'Bug', 'bug', 'bug', 'red', 'i', 1, 4);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id, type_id,
                                rank, status_entered_at, created_at, created_by_kind,
                                created_by_name, edited_at, updated_at)
             VALUES (x'04040404040404040404040404040404', x'01010101010101010101010101010101',
                     1, 'Crash', 'crash', x'02020202020202020202020202020202',
                     x'03030303030303030303030303030303', 'i', 0, 0, 'user', 'You', 0, 0)",
            [],
        )
        .unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        assert!(user_version(&conn).unwrap() >= 16);
        let kept: (String, i64, Option<i64>) = conn
            .query_row(
                "SELECT y.glyph, y.is_default, y.due_offset_days
                   FROM tasks t JOIN task_types y ON y.id = t.type_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(kept, ("bug".to_owned(), 1, Some(4)));
        let add = |glyph: &str, name: &str| {
            conn.execute(
                "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank)
                 VALUES (randomblob(16), x'01010101010101010101010101010101', ?2, lower(?2),
                         ?1, 'purple', 'j')",
                [glyph, name],
            )
        };
        add("epic", "Epic").unwrap();
        add("story", "Story").unwrap();
        assert!(
            add("saga", "Saga").is_err(),
            "the shapes are still a closed list"
        );
        assert!(add("story", "story").is_err(), "names stay unique");
        assert!(
            conn.execute(
                "INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank,
                                         is_default)
                 VALUES (randomblob(16), x'01010101010101010101010101010101', 'Two', 'two',
                         'task', 'grey', 'k', 1)",
                [],
            )
            .is_err(),
            "one default type per project"
        );
        assert!(
            conn.execute(
                "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                                    type_id, rank, status_entered_at, created_at,
                                    created_by_kind, created_by_name, edited_at, updated_at)
                 VALUES (randomblob(16), x'01010101010101010101010101010101', 2, 'X', 'x',
                         x'02020202020202020202020202020202', randomblob(16), 'j', 0, 0,
                         'user', 'You', 0, 0)",
                [],
            )
            .is_err(),
            "tasks still need a type that exists"
        );
    }

    #[test]
    fn card_covers_keep_tasks_and_take_an_id_or_nothing() {
        let mut conn = memory();
        let before = MIGRATIONS
            .iter()
            .position(|m| m.sql.contains("ADD COLUMN cover_id"))
            .unwrap();
        migrate(&mut conn, &MIGRATIONS[..before], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'CV', 'CV', 'cv', 'blue', '{}',
                     0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'To do', 'to do', 'todo', 'blue', 'i');
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank)
             VALUES (x'03030303030303030303030303030303', x'01010101010101010101010101010101',
                     'Task', 'task', 'task', 'blue', 'i');
             INSERT INTO tasks (id, project_id, number, title, title_fold, status_id, type_id,
                                rank, status_entered_at, created_at, created_by_kind,
                                created_by_name, edited_at, updated_at)
             VALUES (x'04040404040404040404040404040404', x'01010101010101010101010101010101',
                     1, 'Crash', 'crash', x'02020202020202020202020202020202',
                     x'03030303030303030303030303030303', 'i', 0, 0, 'user', 'You', 0, 0);",
        )
        .unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        let cover: Option<Vec<u8>> = conn
            .query_row("SELECT cover_id FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cover, None);
        let set = |value: &str| conn.execute(&format!("UPDATE tasks SET cover_id = {value}"), []);
        set("randomblob(16)").unwrap();
        assert!(set("randomblob(3)").is_err(), "a cover is an ID");
        set("NULL").unwrap();
    }

    #[test]
    fn version_12_adds_versions_and_plans_no_task_for_one() {
        let mut conn = memory();
        let before = MIGRATIONS.iter().position(|m| m.version == 12).unwrap();
        migrate(&mut conn, &MIGRATIONS[..before], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'WK', 'WK', 'wk', 'blue', '{}',
                     0, 0);",
        )
        .unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        assert!(user_version(&conn).unwrap() >= 12);
        let add = |id: u8, name: &str, status: &str| {
            conn.execute(
                "INSERT INTO versions (id, project_id, name, name_fold, status, rank, created_at,
                                       updated_at)
                 VALUES (?1, x'01010101010101010101010101010101', ?2, lower(?2), ?3, 'i', 0, 0)",
                rusqlite::params![vec![id; 16], name, status],
            )
        };
        add(2, "1.0", "open").unwrap();
        assert!(
            add(3, "1.0", "open").is_err(),
            "names are unique per project"
        );
        assert!(add(4, "2.0", "done").is_err(), "open, locked or closed");
        add(5, "2.0", "locked").unwrap();
        let columns: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name = 'fix_version_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(columns, 1);
    }

    #[test]
    fn version_11_keeps_values_with_their_task_and_field() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..10], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'CF', 'CF', 'cf', 'blue', '{}',
                     0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'To do', 'to do', 'todo', 'grey', 'i');
             INSERT INTO task_types (id, project_id, name, name_fold, glyph, hue, rank, is_default)
             VALUES (x'03030303030303030303030303030303', x'01010101010101010101010101010101',
                     'Task', 'task', 'task', 'grey', 'i', 1);
             INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                                status_entered_at, type_id, rank, created_at, created_by_kind,
                                created_by_name, edited_at, updated_at)
             VALUES (x'04040404040404040404040404040404', x'01010101010101010101010101010101',
                     1, 'A', 'a', x'02020202020202020202020202020202', 0,
                     x'03030303030303030303030303030303', 'i', 0, 'user', 'me', 0, 0);",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..11], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 11);
        conn.execute_batch(
            "INSERT INTO custom_fields (id, project_id, name, name_fold, query_name, kind, rank,
                                        created_at)
             VALUES (x'05050505050505050505050505050505', x'01010101010101010101010101010101',
                     'Points', 'points', 'points', 'integer', 'i', 0);
             INSERT INTO custom_fields (id, project_id, name, name_fold, query_name, kind, rank,
                                        created_at)
             VALUES (x'06060606060606060606060606060606', NULL, 'Points', 'points', 'points',
                     'integer', 'j', 0);
             INSERT INTO custom_values (task_id, field_id, value)
             VALUES (x'04040404040404040404040404040404', x'05050505050505050505050505050505', 3);",
        )
        .unwrap();
        let twice = conn.execute(
            "INSERT INTO custom_fields (id, project_id, name, name_fold, query_name, kind, rank,
                                        created_at)
             VALUES (x'07070707070707070707070707070707', NULL, 'POINTS', 'points', 'points', 'text', 'k',
                     0)",
            [],
        );
        assert!(twice.is_err(), "one name among the fields of every project");
        let blob = conn.execute(
            "INSERT INTO custom_values (task_id, field_id, value)
             VALUES (x'04040404040404040404040404040404', x'05050505050505050505050505050505', x'00')",
            [],
        );
        assert!(blob.is_err(), "a value is text or a number");
        conn.execute(
            "DELETE FROM tasks WHERE id = x'04040404040404040404040404040404'",
            [],
        )
        .unwrap();
        let left: i64 = conn
            .query_row("SELECT count(*) FROM custom_values", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "a task's values go with it");
    }

    #[test]
    fn version_13_gives_statuses_and_projects_no_rules() {
        let mut conn = memory();
        migrate(&mut conn, &MIGRATIONS[..12], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'RU', 'RU', 'ru', 'blue', '{}',
                     0, 0);
             INSERT INTO statuses (id, project_id, name, name_fold, category, hue, rank)
             VALUES (x'02020202020202020202020202020202', x'01010101010101010101010101010101',
                     'Done', 'done', 'done', 'green', 'i');",
        )
        .unwrap();
        migrate(&mut conn, &MIGRATIONS[..13], |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 13);
        let none: i64 = conn
            .query_row(
                "SELECT (SELECT count(*) FROM statuses WHERE rules IS NULL)
                      + (SELECT count(*) FROM projects WHERE agent_rules IS NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(none, 2);
        conn.execute(
            "UPDATE statuses SET rules = '{\"required\":[\"estimate\"]}'",
            [],
        )
        .unwrap();
        for bad in [
            "UPDATE statuses SET rules = '[1]'",
            "UPDATE statuses SET rules = 'not json'",
            "UPDATE projects SET agent_rules = '\"text\"'",
        ] {
            assert!(conn.execute(bad, []).is_err(), "{bad} must be refused");
        }
    }

    #[test]
    fn version_17_adds_sprints_with_one_active_per_project() {
        let mut conn = memory();
        let before = MIGRATIONS.iter().position(|m| m.version == 17).unwrap();
        migrate(&mut conn, &MIGRATIONS[..before], |_| Ok(())).unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, rank, key, name, name_fold, hue, modules, created_at,
                                   updated_at)
             VALUES (x'01010101010101010101010101010101', 'i', 'SP', 'SP', 'sp', 'blue', '{}',
                     0, 0);",
        )
        .unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        assert!(user_version(&conn).unwrap() >= 17);
        let add = |id: u8, name: &str, state: &str| {
            conn.execute(
                "INSERT INTO sprints (id, project_id, name, name_fold, state, rank, created_at,
                                      updated_at)
                 VALUES (?1, x'01010101010101010101010101010101', ?2, lower(?2), ?3, 'i', 0, 0)",
                rusqlite::params![vec![id; 16], name, state],
            )
        };
        add(2, "Sprint 1", "active").unwrap();
        assert!(
            add(3, "sprint 1", "planned").is_err(),
            "names are unique per project"
        );
        assert!(
            add(4, "Sprint 2", "active").is_err(),
            "one active sprint per project"
        );
        assert!(
            add(5, "Sprint 3", "done").is_err(),
            "planned, active or closed"
        );
        add(6, "Sprint 4", "planned").unwrap();
        let columns: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name = 'sprint_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(columns, 1);
    }
}
