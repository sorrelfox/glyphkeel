// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Translation checks for the command line's own text: every message the
//! code names is in `cli.ftl`, every message there is named, and every one
//! renders.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use gk_engine::Messages;

const CLI: Messages = Messages(include_str!("../i18n/en/cli.ftl"));

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
