// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Translation checks for the command line's own text: every message the
//! code names is in `cli.ftl`, every message there is named, and every one
//! renders. And the language it shows: `--lang`, then `GLYPHKEEL_LANG`,
//! then the language chosen in the app, then the system's, with machine
//! text (`--json`) staying English.

#![allow(clippy::unwrap_used)]

mod common;

use std::path::Path;

use common::{Workspace, code, glyphkeel, stderr, stdout};
use gk_engine::Messages;

const CLI: Messages = Messages::new(include_str!("../i18n/en/cli.ftl"));

#[test]
fn every_message_the_command_line_names_exists_and_every_message_is_named() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut texts = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            texts.push(std::fs::read_to_string(path).unwrap());
        }
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let (missing, unused) = CLI.check_usage(&refs, &[]);
    assert!(
        missing.is_empty(),
        "named in the code but not in cli.ftl: {missing:?}"
    );
    assert!(
        unused.is_empty(),
        "in cli.ftl but named nowhere: {unused:?}"
    );
    for id in CLI.ids() {
        let shown = CLI.text(id, &[]);
        assert!(!shown.is_empty() && shown != id, "{id}");
    }
}

/// The pseudo-locale's form of the program's description.
const PSEUDO_ABOUT: &str = "[Ĝĺýýþĥķééééĺ, áá ĺóóçááĺ ţáášķ ɱááñááĝééŕ";
const ENGLISH_ABOUT: &str = "Glyphkeel, a local task manager";

/// The help with `env` set and `args` given.
fn help(home: &Path, env: &[(&str, &str)], args: &[&str]) -> String {
    let out = glyphkeel(home)
        .envs(env.iter().copied())
        .args(args)
        .arg("--help")
        .output()
        .unwrap();
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    stdout(&out)
}

fn is_pseudo(text: &str) -> bool {
    text.starts_with(PSEUDO_ABOUT)
}

fn is_english(text: &str) -> bool {
    text.starts_with(ENGLISH_ABOUT)
}

#[test]
fn the_language_comes_from_the_option_the_environment_the_app_then_the_system() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    assert!(is_english(&help(home, &[], &[])));
    assert!(is_pseudo(&help(home, &[], &["--lang", "en-XA"])));
    // A global option: also after the command's words.
    let task = help(home, &[], &["task", "show", "--lang=en-XA"]);
    assert!(task.starts_with("[Óóñéé ţóó 20 ţáášķš"), "{task}");

    // The system's language, as gettext reads it. Windows has its own
    // display language, which comes before these variables there.
    if cfg!(not(windows)) {
        assert!(is_pseudo(&help(home, &[("LANG", "en_XA.UTF-8")], &[])));
        assert!(is_pseudo(&help(
            home,
            &[("LANGUAGE", "en_XA:de"), ("LANG", "de_DE.UTF-8")],
            &[]
        )));
        assert!(is_english(&help(
            home,
            &[("LANGUAGE", "en_XA"), ("LC_ALL", "C")],
            &[]
        )));
    }

    // The language chosen in the app's Preferences comes before the
    // system's.
    let config = home.join("config").join("glyphkeel");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("app.json"), r#"{"ui": {"language": "en-XA"}}"#).unwrap();
    assert!(is_pseudo(&help(home, &[("LANG", "en_US.UTF-8")], &[])));

    // The environment variable comes before the app's choice, and the
    // option before both.
    assert!(is_english(&help(home, &[("GLYPHKEEL_LANG", "en")], &[])));
    std::fs::write(config.join("app.json"), r#"{"ui": {"language": "system"}}"#).unwrap();
    assert!(is_pseudo(&help(home, &[("GLYPHKEEL_LANG", "en-XA")], &[])));
    assert!(is_english(&help(
        home,
        &[("GLYPHKEEL_LANG", "en-XA")],
        &["--lang", "en"]
    )));
    // A value that names no language is passed over.
    assert!(is_pseudo(&help(
        home,
        &[("GLYPHKEEL_LANG", "en-XA")],
        &["--lang", "system"]
    )));
}

/// Words in plain English left in `text`: runs of ASCII letters, other
/// than `allowed` and placeholders in capitals.
fn plain_words(text: &str, allowed: &[&str]) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .filter(|w| !w.chars().all(|c| c.is_ascii_uppercase()))
        .filter(|w| !allowed.contains(w))
        .map(str::to_owned)
        .collect()
}

#[test]
fn output_and_errors_in_the_pseudo_locale_have_no_plain_english() {
    let ws = Workspace::new();
    let pseudo = |args: &[&str]| {
        let mut all = vec!["--lang", "en-XA"];
        all.extend_from_slice(args);
        ws.run(&all)
    };

    // An engine error, with its hint and choices.
    let out = pseudo(&["task", "show", "ZZ-1"]);
    assert_eq!(code(&out), 2);
    let err = stderr(&out);
    assert!(err.starts_with("[ĝĺýýþĥķééééĺ: ["), "{err}");
    assert_eq!(plain_words(&err, &["ZZ"]), Vec::<String>::new(), "{err}");
    // As JSON the message stays English, with its ID for translating it.
    let out = ws.run(&["--lang", "en-XA", "--json", "task", "show", "ZZ-1"]);
    let json: serde_json::Value = serde_json::from_str(&stderr(&out)).unwrap();
    assert!(json["message"].as_str().unwrap().is_ascii(), "{json}");

    // A command line mistake, told by the program rather than clap.
    let out = pseudo(&["tsk"]);
    assert_eq!(code(&out), 1);
    let err = stderr(&out);
    assert_eq!(
        plain_words(&err, &["tsk", "task", "glyphkeel"]),
        Vec::<String>::new(),
        "{err}"
    );
    let out = pseudo(&["settings", "set", "--week-start", "someday"]);
    let err = stderr(&out);
    assert_eq!(
        plain_words(
            &err,
            &["week", "start", "someday", "monday", "saturday", "sunday"]
        ),
        Vec::<String>::new(),
        "{err}"
    );

    // Output for people: a table's labels, yes and no, and a plain line.
    ws.ok(&["project", "create", "--key", "ZZ", "--name", "Zed"]);
    let out = pseudo(&["project", "list"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let list = stdout(&out);
    assert!(list.contains("Zed"), "{list}");
    let out = pseudo(&["workspace", "list"]);
    let lines = stdout(&out);
    assert!(lines.starts_with('['), "{lines}");
}

#[test]
fn numbers_and_days_follow_the_language() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--key", "ZZ", "--name", "Zed"]);
    ws.ok(&[
        "task",
        "create",
        "--project",
        "ZZ",
        "--title",
        "Due",
        "--due-date",
        "2026-11-27",
    ]);
    let english = ws.ok(&["task", "show", "ZZ-1"]);
    assert!(english.contains("Nov 27, 2026"), "{english}");
    let german = ws.ok(&["--lang", "de", "task", "show", "ZZ-1"]);
    assert!(german.contains("27.11.2026"), "{german}");
    let json = ws.json(&["--lang", "de", "task", "show", "ZZ-1"]);
    assert_eq!(json["items"][0]["due_date"], "2026-11-27", "{json}");
}
