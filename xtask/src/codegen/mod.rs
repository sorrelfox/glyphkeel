// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask gen`: the files generated from the operation catalog.
//!
//! - `docs/reference/catalog.schema.json`: every operation with its JSON
//!   Schemas, the error type and the change event;
//! - `docs/reference/mcp-tools.json`: the golden MCP tool list, as agents
//!   see it, and which tools each profile gets;
//! - `docs/reference/operations.md` and `docs/reference/cli.md`: the
//!   reference pages;
//! - `ui/src/generated/ops.ts`: the TypeScript client.
//!
//! `--check` writes nothing and fails when a file differs from what the
//! catalog generates now.

mod docs;
mod ts;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

/// One generated file: its path from the repository root and its content.
pub(crate) struct Output {
    pub(crate) path: &'static str,
    pub(crate) content: String,
}

/// Every generated file.
pub(crate) fn outputs() -> Result<Vec<Output>> {
    let catalog = gk_api::schema::catalog_document();
    Ok(vec![
        Output {
            path: "docs/reference/catalog.schema.json",
            content: json_file(&catalog)?,
        },
        Output {
            path: "docs/reference/mcp-tools.json",
            content: json_file(&gk_api::mcp::golden_document())?,
        },
        Output {
            path: "docs/reference/operations.md",
            content: docs::operations(),
        },
        Output {
            path: "docs/reference/cli.md",
            content: docs::cli(),
        },
        Output {
            path: "ui/src/generated/ops.ts",
            content: ts::generate(&catalog)?,
        },
    ])
}

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        other => bail!("`gen` takes only --check (got: {})", other.join(" ")),
    };
    let mut stale = Vec::new();
    for out in outputs()? {
        let path = root.join(out.path);
        let current = fs::read_to_string(&path).ok();
        if current.as_deref() == Some(out.content.as_str()) {
            continue;
        }
        if check {
            stale.push(out.path);
        } else {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
            }
            fs::write(&path, &out.content)
                .with_context(|| format!("cannot write {}", path.display()))?;
            eprintln!("gen: wrote {}", out.path);
        }
    }
    if stale.is_empty() {
        eprintln!("gen: the generated files match the catalog");
        Ok(true)
    } else {
        for path in &stale {
            eprintln!("stale: {path}");
        }
        eprintln!("gen: run `cargo xtask gen` to regenerate");
        Ok(false)
    }
}

/// Pretty JSON with keys sorted at every level, so the file doesn't depend
/// on how `serde_json` was built, and a final newline.
fn json_file(value: &Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(&sorted(value))?;
    text.push('\n');
    Ok(text)
}

pub(crate) fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted(&o[k]));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed files match the catalog: the same check as
    /// `cargo xtask gen --check`.
    #[test]
    fn generated_files_are_up_to_date() {
        let root = crate::workspace_root();
        for out in outputs().expect("generates") {
            let current = fs::read_to_string(root.join(out.path)).unwrap_or_default();
            assert!(
                current == out.content,
                "{} is stale: run `cargo xtask gen`",
                out.path
            );
        }
    }

    #[test]
    fn every_operation_reaches_every_output() {
        let outputs = outputs().expect("generates");
        let text = |path: &str| {
            outputs
                .iter()
                .find(|o| o.path == path)
                .map(|o| o.content.clone())
                .unwrap_or_default()
        };
        let schema = text("docs/reference/catalog.schema.json");
        let ts = text("ui/src/generated/ops.ts");
        let reference = text("docs/reference/operations.md");
        let cli = text("docs/reference/cli.md");
        let tools = text("docs/reference/mcp-tools.json");
        for d in gk_api::CATALOG {
            let quoted = format!("\"{}\"", d.id);
            assert!(
                schema.contains(&quoted),
                "{} missing from the schemas",
                d.id
            );
            assert!(ts.contains(&quoted), "{} missing from the client", d.id);
            assert!(
                reference.contains(&format!("`{}`", d.id)),
                "{} missing from the reference",
                d.id
            );
            if let Some(command) = d.cli {
                assert!(
                    cli.contains(&format!("glyphkeel {command}")),
                    "{command} missing"
                );
            }
            if !d.is_app_only() {
                assert!(
                    tools.contains(&quoted),
                    "{} not covered in the tool list",
                    d.id
                );
            }
        }
        for tool in gk_api::TOOLS {
            assert!(tools.contains(&format!("\"{}\"", tool.name)));
        }
    }

    #[test]
    fn json_keys_are_sorted() {
        let v: Value = serde_json::json!({ "b": 1, "a": { "d": 2, "c": [ { "f": 1, "e": 2 } ] } });
        let text = json_file(&v).expect("writes");
        let a = text.find("\"a\"").expect("a");
        let b = text.find("\"b\"").expect("b");
        assert!(a < b);
        assert!(text.find("\"e\"") < text.find("\"f\""));
        assert!(text.ends_with("}\n"));
    }
}
