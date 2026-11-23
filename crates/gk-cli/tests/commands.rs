// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel` as a program against a temporary workspace: its output for
//! people and as JSON (snapshots), and its exit codes.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gk_engine::{Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::Value;
use tempfile::TempDir;

struct Workspace {
    dir: TempDir,
}

impl Workspace {
    /// A new, empty workspace in a temporary folder.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            &WorkspaceLayout::new(dir.path().join("Personal")),
            EngineConfig::new(Role::Cli),
        )
        .unwrap();
        engine.close();
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Personal")
    }

    /// Runs `glyphkeel --workspace <this> <args>`.
    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = glyphkeel(self.dir.path());
        cmd.arg("--workspace").arg(self.path()).args(args);
        cmd.output().unwrap()
    }

    /// Runs a command that must succeed and returns its standard output.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert_eq!(code(&out), 0, "{args:?}: {}", stderr(&out));
        stdout(&out)
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&self.ok(&all)).unwrap()
    }
}

/// The program, with its data folder inside `home` so a test never touches
/// the real one.
fn glyphkeel(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_glyphkeel"));
    cmd.env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env_remove("GLYPHKEEL_WORKSPACE")
        .env_remove("RUST_LOG");
    cmd
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

/// IDs, times and the program version vary between runs.
macro_rules! snapshot {
    ($value:expr) => {
        insta::with_settings!({
            filters => vec![
                (r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", "[id]"),
                (r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z", "[time]"),
                (r#"("?program_version"?: )"?[0-9][^"\n]*"?"#, "$1[version]"),
            ],
        }, {
            insta::assert_snapshot!($value);
        });
    };
}

#[test]
fn status_for_people_and_as_json() {
    let ws = Workspace::new();
    snapshot!(ws.ok(&["status"]));
    snapshot!(ws.ok(&["status", "--json"]));
}

#[test]
fn settings_change_shows_in_the_activity_log_and_reverts() {
    let ws = Workspace::new();
    snapshot!(ws.ok(&["settings", "get"]));
    snapshot!(ws.ok(&[
        "settings",
        "set",
        "--hours-per-day",
        "6",
        "--week-start",
        "sunday"
    ]));
    snapshot!(ws.ok(&["activity"]));
    snapshot!(ws.ok(&["activity", "--json"]));

    let group = ws.json(&["activity"])["items"][0]["group"]
        .as_str()
        .unwrap()
        .to_owned();
    let dry = ws.json(&["revert", &group, "--dry-run"]);
    assert_eq!(dry["reverted"][0], group.as_str());
    assert!(dry.get("group").is_none(), "a dry run changes nothing");

    let done = ws.json(&["revert", &group]);
    assert_eq!(done["reverted"][0], group.as_str());
    assert_eq!(ws.json(&["settings", "get"])["hours_per_day"], 8);

    snapshot!(ws.ok(&["revert", &group]));
}

#[test]
fn a_task_is_created_and_moved_on_the_board() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    snapshot!(ws.ok(&[
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Write the release notes",
        "--priority",
        "high",
        "--estimate",
        "1d 2h",
    ]));
    ws.ok(&["task", "create", "--project", "gk", "--title", "Second"]);
    snapshot!(ws.ok(&[
        "task",
        "move",
        "gk-1",
        "--status",
        "in progress",
        "--at",
        "top"
    ]));
    let board = ws.json(&["board", "GK"]);
    assert_eq!(board["columns"][0]["cards"][0]["key"], "GK-2");
    assert_eq!(board["columns"][2]["cards"][0]["key"], "GK-1");
    let shown = ws.json(&["task", "show", "GK-1", "GK-2"]);
    assert_eq!(shown["total"], 2);
    assert_eq!(shown["items"][0]["status"]["name"], "In progress");
    let out = ws.run(&["task", "move", "GK-1", "--status", "Nowhere"]);
    assert_eq!(code(&out), 2);
    snapshot!(stderr(&out));
}

#[test]
fn subtasks_are_reordered_and_one_tasks_activity_is_listed() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Parent"]);
    for title in ["First", "Second"] {
        ws.ok(&[
            "task",
            "create",
            "--project",
            "GK",
            "--title",
            title,
            "--parent",
            "GK-1",
        ]);
    }
    let moved = ws.json(&["task", "reorder-subtask", "GK-3", "--at", "top"]);
    assert_eq!(moved["task"]["subtasks"][0]["key"], "GK-3");
    assert_eq!(moved["task"]["subtasks"][1]["key"], "GK-2");
    let feed = ws.json(&["activity", "--task", "GK-3"]);
    assert_eq!(feed["total"], 2);
    assert_eq!(feed["items"][0]["op"], "task.reorder_subtask");
}

#[test]
fn time_is_logged_edited_and_reported() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Notes"]);
    let logged = ws.json(&[
        "time",
        "log",
        "GK-1",
        "--duration",
        "+30m",
        "--date",
        "2026-11-23",
        "--note",
        "Draft",
    ]);
    assert_eq!(logged["spent_minutes"], 30);
    let id = logged["entry"]["id"].as_str().unwrap().to_owned();
    let edited = ws.json(&["time", "edit", &id, "--duration", "1h 15m"]);
    assert_eq!(edited["entry"]["duration"], "1h 15m");
    let report = ws.json(&[
        "time",
        "report",
        "--from",
        "2026-11-23",
        "--to",
        "2026-11-29",
        "--group-by",
        "day",
    ]);
    assert_eq!(report["items"][0]["key"], "2026-11-23");
    assert_eq!(report["total_duration"], "1h 15m");
    snapshot!(ws.ok(&[
        "time",
        "report",
        "--from",
        "2026-11-23",
        "--to",
        "2026-11-29"
    ]));
    let gone = ws.json(&["time", "delete", &id]);
    assert_eq!(gone["spent_minutes"], 0);
    let out = ws.run(&["time", "log", "GK-1", "--duration", "25h"]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
}

/// The timer lives in the workspace, so each command is its own process:
/// one starts it, others see it, pause it and stop it.
#[test]
fn the_timer_is_shared_between_processes() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Notes"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Review"]);
    let none = ws.json(&["timer", "status"]);
    assert!(none.get("timer").is_none());

    let started = ws.json(&["timer", "start", "GK-1", "--note", "Writing"]);
    assert_eq!(started["timer"]["state"], "running");
    let seen = ws.json(&["timer", "status"]);
    assert_eq!(seen["timer"]["task"]["key"], "GK-1");
    assert_eq!(seen["timer"]["note"], "Writing");
    let overview = ws.json(&["status"]);
    assert_eq!(overview["timer"]["task"]["key"], "GK-1");

    let paused = ws.json(&["timer", "pause"]);
    assert_eq!(paused["timer"]["state"], "paused");
    assert_eq!(ws.json(&["timer", "status"])["timer"]["state"], "paused");

    // Starting on another task stops the paused one; it ran for under half
    // a minute, so nothing is logged.
    let switched = ws.json(&["timer", "start", "GK-2"]);
    assert_eq!(switched["timer"]["task"]["key"], "GK-2");
    assert!(switched.get("entry").is_none());
    assert_eq!(switched["warnings"].as_array().unwrap().len(), 1);

    let stopped = ws.json(&["timer", "stop", "--duration", "20m", "--note", "Read it"]);
    assert_eq!(stopped["entry"]["task"], "GK-2");
    assert_eq!(stopped["entry"]["minutes"], 20);
    assert!(ws.json(&["timer", "status"]).get("timer").is_none());
    let out = ws.run(&["timer", "stop"]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));
}

#[test]
fn reminders_are_set_listed_and_acknowledged() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Call back"]);
    let set = ws.json(&[
        "reminder",
        "set",
        "GK-1",
        "--at",
        "2020-01-01T09:00:00Z",
        "--note",
        "Ring Anna",
    ]);
    assert_eq!(set["reminder"]["at"], "2020-01-01T09:00:00.000Z");
    let due = ws.json(&["reminder", "due"]);
    assert_eq!(due["total"], 1);
    assert_eq!(due["items"][0]["kind"], "reminder");
    assert_eq!(due["items"][0]["missed"], true);
    let id = due["items"][0]["id"].as_str().unwrap().to_owned();
    let ack = ws.json(&["reminder", "ack", &id]);
    assert_eq!(ack["acknowledged"][0], id.as_str());
    assert_eq!(ws.json(&["reminder", "due"])["total"], 0);
    let cleared = ws.json(&["reminder", "clear", "GK-1"]);
    assert!(cleared.get("reminder").is_none());
}

#[test]
fn input_as_json_and_options_together() {
    let ws = Workspace::new();
    let out = ws.json(&[
        "settings",
        "set",
        "--input",
        r#"{"trash_days": 7, "hours_per_day": 4}"#,
        "--hours-per-day",
        "5",
    ]);
    assert_eq!(out["settings"]["trash_days"], 7);
    assert_eq!(out["settings"]["hours_per_day"], 5, "the option wins");
}

#[test]
fn agent_policy_and_sessions() {
    let ws = Workspace::new();
    let out = ws.json(&["agents", "policy", "--max-profile", "read-only"]);
    assert_eq!(out["settings"]["agent_max_profile"], "read-only");
    snapshot!(ws.ok(&["agents", "sessions"]));
}

#[test]
fn quiet_prints_nothing() {
    let ws = Workspace::new();
    assert_eq!(
        ws.ok(&["--quiet", "settings", "set", "--trash-days", "9"]),
        ""
    );
    assert_eq!(ws.json(&["settings", "get"])["trash_days"], 9);
}

#[test]
fn help_and_version_succeed() {
    let ws = Workspace::new();
    assert!(ws.ok(&["--help"]).contains("Exit codes"));
    assert_eq!(
        ws.ok(&["--version"]),
        format!("glyphkeel {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(ws.ok(&["completions", "bash"]).contains("_glyphkeel"));
    assert!(ws.ok(&["man"]).contains(".TH glyphkeel 1"));
}

#[test]
fn usage_errors_exit_1() {
    let ws = Workspace::new();
    for args in [
        &["settings", "set", "--bogus"][..],
        &["settings", "set", "--hours-per-day", "many"],
        &["settings", "set", "--week-start", "friday"],
        &["no-such-command"],
        &["agents"],
    ] {
        assert_eq!(code(&ws.run(args)), 1, "{args:?}");
    }
}

#[test]
fn invalid_input_exits_1_with_the_field() {
    let ws = Workspace::new();
    let out = ws.run(&["settings", "set", "--hours-per-day", "99"]);
    assert_eq!(code(&out), 1);
    assert_eq!(stdout(&out), "");
    snapshot!(stderr(&out));

    let out = ws.run(&["--json", "settings", "set", "--hours-per-day", "99"]);
    assert_eq!(code(&out), 1);
    snapshot!(stderr(&out));

    let out = ws.run(&["settings", "set", "--input", "[1, 2]"]);
    assert_eq!(code(&out), 1);
    let out = ws.run(&["revert", "not-an-id"]);
    assert_eq!(code(&out), 1);
}

#[test]
fn a_missing_workspace_exits_2_and_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nowhere");
    let out = glyphkeel(dir.path())
        .arg("--workspace")
        .arg(&missing)
        .arg("status")
        .output()
        .unwrap();
    assert_eq!(code(&out), 2, "{}", stderr(&out));
    assert!(!missing.exists());

    let out = glyphkeel(dir.path())
        .args(["--workspace", "Elsewhere", "status"])
        .output()
        .unwrap();
    assert_eq!(code(&out), 2);
}

#[test]
fn the_default_workspace_is_created_in_the_data_folder() {
    let dir = tempfile::tempdir().unwrap();
    let out = glyphkeel(dir.path())
        .args(["--json", "status"])
        .output()
        .unwrap();
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let info: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(info["name"], "Personal");
    assert!(
        dir.path()
            .join("data/glyphkeel/workspaces/Personal/workspace.db")
            .is_file()
    );
}

#[test]
fn a_broken_database_exits_4() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("Broken");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("workspace.db"), "this is not a database at all").unwrap();
    let out = glyphkeel(dir.path())
        .arg("--workspace")
        .arg(&ws)
        .args(["--json", "status"])
        .output()
        .unwrap();
    assert_eq!(code(&out), 4);
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["code"], "storage");
}

/// `glyphkeel open` with the app next to the program: copies the program
/// into a folder with a stand-in app that records its arguments and keeps
/// running, so the test sees that `glyphkeel` doesn't wait for it.
///
/// The programs these tests run are never written by the test process
/// itself. Tests run on several threads, and a child started by another
/// thread while this one has a program open for writing inherits that open
/// file until it starts its own program; running the file then fails with
/// "Text file busy". `cp` writes them in a process of its own instead, which
/// has closed the file when it exits.
#[cfg(unix)]
mod app {
    use std::time::{Duration, Instant};

    use super::*;

    /// Copies `from` to `to` as an executable, in other processes.
    fn install(from: &Path, to: &Path) {
        let copied = Command::new("cp").arg(from).arg(to).status().unwrap();
        assert!(copied.success(), "cp {} failed", to.display());
        let marked = Command::new("chmod").arg("755").arg(to).status().unwrap();
        assert!(marked.success(), "chmod {} failed", to.display());
    }

    /// A copy of `glyphkeel` at `to`.
    fn copy_program(to: &Path) {
        install(Path::new(env!("CARGO_BIN_EXE_glyphkeel")), to);
    }

    fn stand_in(dir: &Path, record: &Path) {
        let script = format!(
            "#!/bin/sh\necho $$ > '{pid}.tmp'\nprintf '%s\\n' \"$@\" > '{args}'\nmv '{pid}.tmp' '{pid}'\nexec sleep 30\n",
            pid = record.with_extension("pid").display(),
            args = record.display(),
        );
        // Written here but never run itself: only its copy is.
        let source = record.with_extension("sh");
        std::fs::write(&source, script).unwrap();
        install(&source, &dir.join("glyphkeel-app"));
    }

    /// Waits for the stand-in to start, stops it and returns its
    /// arguments.
    fn arguments_of_stand_in(record: &Path) -> Vec<String> {
        let pid_file = record.with_extension("pid");
        let start = Instant::now();
        while !pid_file.is_file() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the app never started"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let _ = Command::new("kill").arg(pid.trim()).status();
        std::fs::read_to_string(record)
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn open_starts_the_app_next_to_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let program = bin.join("glyphkeel");
        copy_program(&program);
        let record = dir.path().join("args");
        stand_in(&bin, &record);

        let start = Instant::now();
        let out = Command::new(&program)
            .args(["open", "TM-41", "glyphkeel://open/TM-2"])
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert_eq!(code(&out), 0, "{}", stderr(&out));
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "glyphkeel waited for the app"
        );
        assert_eq!(
            arguments_of_stand_in(&record),
            ["glyphkeel://open/TM-41", "glyphkeel://open/TM-2"]
        );
    }

    #[test]
    fn no_command_starts_the_app_from_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let program = bin.join("glyphkeel");
        copy_program(&program);
        let record = dir.path().join("args");
        stand_in(&elsewhere, &record);

        let out = Command::new(&program)
            .env("PATH", format!("{}:/usr/bin:/bin", elsewhere.display()))
            .output()
            .unwrap();
        assert_eq!(code(&out), 0, "{}", stderr(&out));
        assert!(arguments_of_stand_in(&record).is_empty());
    }

    #[test]
    fn without_the_app_open_exits_2() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("glyphkeel");
        copy_program(&program);
        let out = Command::new(&program)
            .arg("open")
            .env("PATH", dir.path().join("empty"))
            .output()
            .unwrap();
        assert_eq!(code(&out), 2);
        assert!(stderr(&out).contains("glyphkeel-app"));

        let out = Command::new(&program)
            .args(["open", "https://example.org"])
            .output()
            .unwrap();
        assert_eq!(code(&out), 1);
    }
}

/// The day a quarter of an hour logged now falls on, as `time log` reports it.
fn logged_day(entry: &Value) -> String {
    entry["entry"]["date"].as_str().unwrap().to_owned()
}

#[test]
fn the_command_line_and_the_engine_agree_on_today_in_the_personal_zone() {
    let ws = Workspace::new();
    // UTC+14 and UTC-12 are 26 hours apart: their dates always differ.
    let settings = ws.dir.path().join("config").join("glyphkeel");
    std::fs::create_dir_all(&settings).unwrap();
    std::fs::write(
        settings.join(gk_store::USER_SETTINGS_FILE),
        br#"{"time_zone": "Pacific/Kiritimati"}"#,
    )
    .unwrap();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Work"]);

    let engine_day = |zone: &str| {
        let mut config = EngineConfig::new(Role::Cli);
        config.time_zone = Some(zone.to_owned());
        let engine = Engine::open(&WorkspaceLayout::new(ws.path()), config).unwrap();
        let ctx = gk_engine::Context::user(gk_api::ops::common::Via::Cli);
        let entry = engine
            .call(
                &ctx,
                "time.log",
                serde_json::json!({ "key": "GK-1", "duration": "15m" }),
            )
            .unwrap();
        engine.close();
        logged_day(&entry)
    };

    let cli = logged_day(&ws.json(&["time", "log", "GK-1", "--duration", "15m"]));
    assert_eq!(
        cli,
        engine_day("Pacific/Kiritimati"),
        "the personal setting"
    );

    let behind = logged_day(&ws.json(&[
        "--tz",
        "Etc/GMT+12",
        "time",
        "log",
        "GK-1",
        "--duration",
        "15m",
    ]));
    assert_eq!(behind, engine_day("Etc/GMT+12"), "--tz for one command");
    assert_ne!(behind, cli);

    let out = ws.run(&[
        "--tz",
        "Mars/Olympus",
        "time",
        "log",
        "GK-1",
        "--duration",
        "15m",
    ]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("Mars/Olympus"), "{}", stderr(&out));
}
