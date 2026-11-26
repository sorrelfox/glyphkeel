// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent imports a Jira export from the workspace's `exports` folder:
//! a dry run that lists only the rows with problems, the import with those
//! rows left out, and a revert; it keeps presets of its own.

#![allow(clippy::unwrap_used)]

mod common;

use std::path::Path;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

#[tokio::test]
async fn an_agent_imports_a_jira_export_and_takes_it_back() {
    let (_dir, mut options) = workspace();
    options.toolsets = Some(gk_api::Toolset::ALL.to_vec());
    {
        let (engine, person) = open_as_person(&options);
        common::set_agent_cap(&engine, "full");
        engine
            .call(
                &person,
                "project.create",
                json!({ "name": "Web", "key": "SHOP" }),
            )
            .unwrap();
        let exports = engine.store().layout().exports_dir();
        std::fs::create_dir_all(&exports).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gk-engine/tests/csv/jira.csv");
        std::fs::copy(fixture, exports.join("jira.csv")).unwrap();
        engine.close();
    }
    options.profile = gk_api::Profile::Full;
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Importer").await;

    let presets = output(&c.call("list_csv_presets", json!({})).await);
    assert_eq!(presets["items"][0]["name"], "jira");

    let check = output(
        &c.call(
            "import_csv",
            json!({ "project": "SHOP", "path": "jira.csv", "dry_run": true, "only_problems": true }),
        )
        .await,
    );
    assert_eq!(check["preset"], "jira");
    assert_eq!(check["blocked"], true);
    assert_eq!(check["rows"].as_array().unwrap().len(), 1);
    assert_eq!(check["rows"][0]["problems"][0]["code"], "no_title");

    let outside = c
        .call(
            "import_csv",
            json!({ "project": "SHOP", "path": "/etc/passwd", "dry_run": true }),
        )
        .await;
    assert_eq!(outside.is_error, Some(true));

    let done = output(
        &c.call(
            "import_csv",
            json!({ "project": "SHOP", "path": "jira.csv", "skip_invalid": true }),
        )
        .await,
    );
    assert_eq!(done["counts"]["created"], 3);
    output(
        &c.call("revert_changes", json!({ "groups": [done["group"]] }))
            .await,
    );

    let saved = output(
        &c.call(
            "save_csv_preset",
            json!({ "name": "Ours", "columns": [{ "column": "Summary", "to": "title" }] }),
        )
        .await,
    );
    assert_eq!(saved["items"].as_array().unwrap().len(), 3);
    let gone = output(&c.call("delete_csv_preset", json!({ "name": "Ours" })).await);
    assert_eq!(gone["items"].as_array().unwrap().len(), 2);
    c.close().await;
}
