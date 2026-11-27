// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Every tool that takes a path on disk, under the standard and the full
//! profile: a plain name is a file in the workspace's `exports` folder, a
//! path inside a folder the person added works too, and everything else is
//! refused with the folders named: other absolute paths, `..`, hidden
//! files, and links that lead outside.

#![allow(clippy::unwrap_used)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{connect, lifecycles, open_as_person, text, workspace};
use gk_api::Profile;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

/// The tools that name a file or folder, with the input around the path
/// and whether they write it.
const TOOLS: &[(&str, &str, bool)] = &[
    ("attach_file", r#"{"task": "TM-1"}"#, false),
    ("import_data", r#"{"dry_run": true}"#, false),
    ("import_csv", r#"{"project": "TM", "dry_run": true}"#, false),
    (
        "import_wiki_pages",
        r#"{"project": "TM", "dry_run": true}"#,
        false,
    ),
    ("export_data", r#"{"projects": ["TM"]}"#, true),
    ("export_wiki_page", r#"{"project": "TM"}"#, true),
];

/// The file each reading tool reads, by name.
fn source(tool: &str) -> &'static str {
    match tool {
        "attach_file" => "note.txt",
        "import_data" => "tm.zip",
        "import_csv" => "rows.csv",
        "import_wiki_pages" => "notes",
        _ => "unused",
    }
}

fn input(tool: &str, path: &str) -> Value {
    let (_, around, _) = TOOLS.iter().find(|(t, _, _)| *t == tool).unwrap();
    let mut v: Value = serde_json::from_str(around).unwrap();
    v["path"] = json!(path);
    v
}

/// Puts each tool's source in `folder`.
fn fill(folder: &Path, archive: &Path) {
    fs::create_dir_all(folder.join("notes")).unwrap();
    fs::write(folder.join("note.txt"), "A note.\n").unwrap();
    fs::write(folder.join("rows.csv"), "Title\nFrom CSV\n").unwrap();
    fs::write(folder.join("notes").join("Page.md"), "# Page\n\nText.\n").unwrap();
    fs::copy(archive, folder.join("tm.zip")).unwrap();
}

/// An absolute path outside every folder agents may use, written as this
/// system writes paths.
const ELSEWHERE: &str = if cfg!(windows) {
    r"C:\Windows\win.ini"
} else {
    "/etc/passwd"
};

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

struct Setup {
    _dir: tempfile::TempDir,
    options: gk_mcp::Options,
    exports: PathBuf,
    shared: PathBuf,
    secret: PathBuf,
}

fn setup() -> Setup {
    let (dir, mut options) = workspace();
    let root = gk_store::canonical_path(dir.path()).unwrap();
    let shared = root.join("shared");
    let secret = root.join("secret");
    let exports;
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call("task.create", json!({ "project": "TM", "title": "Ship" }));
        call(
            "wiki.create",
            json!({ "project": "TM", "title": "Home", "body": "Hi.\n" }),
        );
        common::set_agent_cap(&engine, "full");
        // The person's own export, anywhere.
        let archive = root.join("tm.zip");
        call("transfer.export", json!({ "path": s(&archive) }));
        fs::create_dir_all(engine.store().layout().exports_dir()).unwrap();
        exports = gk_store::canonical_path(&engine.store().layout().exports_dir()).unwrap();
        for folder in [&exports, &shared, &secret] {
            fill(folder, &archive);
        }
        engine.close();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(secret.join("note.txt"), exports.join("leak.txt")).unwrap();
        symlink(&secret, exports.join("out")).unwrap();
    }
    options.toolsets = Some(gk_api::Toolset::ALL.to_vec());
    options.agent_folders = Some(vec![shared.clone()]);
    Setup {
        _dir: dir,
        options,
        exports,
        shared,
        secret,
    }
}

fn refused(r: &CallToolResult, code: &str, folders: &[&Path]) -> String {
    assert_eq!(r.is_error, Some(true), "{r:?}");
    let t = text(r);
    assert!(t.starts_with(&format!("Error ({code})")), "{t}");
    for folder in folders {
        assert!(t.contains(&s(folder)), "{t} names {}", folder.display());
    }
    t
}

fn ok(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

#[tokio::test]
async fn every_path_taking_tool_is_limited_to_the_folders_agents_may_use() {
    for profile in [Profile::Standard, Profile::Full] {
        let mut s_ = setup();
        s_.options.profile = profile;
        let (_, mode, version, _) = lifecycles().remove(0);
        let c = connect(&s_.options, mode, version, "Path Tester").await;
        let listed = c.tool_names().await;
        let folders = [s_.exports.as_path(), s_.shared.as_path()];
        // Both profiles have every one of these tools, with all toolsets.
        for (tool, _, _) in TOOLS {
            assert!(listed.iter().any(|l| l == tool), "{profile} has {tool}");
        }

        // get_workspace says where files may be.
        let info = ok(&c.call("get_workspace", json!({})).await);
        assert_eq!(
            info["agent_folders"],
            json!([s(&s_.exports), s(&s_.shared)])
        );

        for (tool, _, writes) in TOOLS {
            let (inside, there, outside) = if *writes {
                (
                    format!("{tool}-{profile}.zip"),
                    s_.shared.join(format!("{tool}-{profile}.zip")),
                    s_.secret.join(format!("{tool}-{profile}.zip")),
                )
            } else {
                (
                    source(tool).to_owned(),
                    s_.shared.join(source(tool)),
                    s_.secret.join(source(tool)),
                )
            };
            // A plain name: the exports folder.
            let done = ok(&c.call(tool, input(tool, &inside)).await);
            if *writes {
                assert_eq!(done["path"], json!(s(&s_.exports.join(&inside))), "{tool}");
            }
            // A path inside a folder the person added.
            let done = ok(&c.call(tool, input(tool, &s(&there))).await);
            if *writes {
                assert_eq!(done["path"], json!(s(&there)), "{tool}");
            }
            // Elsewhere, refused with the folders named.
            let t = refused(
                &c.call(tool, input(tool, &s(&outside))).await,
                "forbidden",
                &folders,
            );
            assert!(t.contains("outside the folders agents may use"), "{t}");
            refused(
                &c.call(tool, input(tool, ELSEWHERE)).await,
                "forbidden",
                &folders,
            );
            refused(
                &c.call(tool, input(tool, &format!("../../secret/{}", source(tool))))
                    .await,
                "invalid",
                &folders,
            );
            refused(
                &c.call(
                    tool,
                    input(
                        tool,
                        &s(&s_.shared.join("..").join("secret").join(source(tool))),
                    ),
                )
                .await,
                "invalid",
                &folders,
            );
            refused(
                &c.call(tool, input(tool, ".hidden")).await,
                "invalid",
                &folders,
            );
            #[cfg(unix)]
            {
                let through = if *writes { "out/new.zip" } else { "leak.txt" };
                let t = refused(
                    &c.call(tool, input(tool, through)).await,
                    "forbidden",
                    &folders,
                );
                assert!(t.contains("through a link"), "{t}");
            }
            if *writes {
                assert!(!outside.exists(), "{tool} wrote outside");
            }
        }
        c.close().await;
        // The secret folder is as it was.
        let mut names: Vec<String> = fs::read_dir(&s_.secret)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["note.txt", "notes", "rows.csv", "tm.zip"],
            "{profile}"
        );
    }
}

#[tokio::test]
async fn a_file_attached_by_path_comes_from_where_it_was_checked() {
    let s_ = setup();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&s_.options, mode, version, "Attacher").await;
    let attached = ok(&c
        .call("attach_file", json!({ "task": "TM-1", "path": "note.txt" }))
        .await);
    assert_eq!(attached["file"]["name"], "note.txt", "{attached}");
    let id = attached["file"]["id"].as_str().unwrap();
    let read = ok(&c.call("read_attachment", json!({ "attachment": id })).await);
    assert_eq!(read["text"], "A note.\n", "{read}");
    c.close().await;
}
