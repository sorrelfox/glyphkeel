// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Translation checks for the MCP server's own messages: every message the
//! code names is in `mcp.ftl` and every message there is named.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use gk_engine::Messages;

const MCP: Messages = Messages::new(include_str!("../i18n/en/mcp.ftl"));

#[test]
fn every_message_the_server_names_exists_and_every_message_is_named() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut texts = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            texts.push(std::fs::read_to_string(path).unwrap());
        }
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    // Client IDs in the table of known MCP clients look like message IDs.
    let (missing, unused) = MCP.check_usage(&refs, &["mcp-inspector"]);
    assert!(
        missing.is_empty(),
        "named in the code but not in mcp.ftl: {missing:?}"
    );
    assert!(
        unused.is_empty(),
        "in mcp.ftl but named nowhere: {unused:?}"
    );
}
