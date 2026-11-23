// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! JSON Schemas of the catalog.
//!
//! - Inputs are described as deserialization reads them: a field with a
//!   default is optional.
//! - Outputs are described as serialization writes them: a field is required
//!   unless it is left out when absent.
//! - [`catalog_document`] is the whole catalog as one JSON document, with
//!   shared types under `$defs`. The TypeScript client and the reference
//!   pages are generated from it.
//! - [`portable`] rewrites a schema for MCP clients: sub-schemas inlined, no
//!   `null` branches, no Rust number formats.

use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, Schema};
use serde_json::{Map, Value, json};

use crate::catalog::{CATALOG, Exposure, OpKind};
use crate::error::OpError;
use crate::event::Changed;
use crate::mcp;

/// The schema of an input type.
#[must_use]
pub fn input<T: JsonSchema>() -> Schema {
    SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator()
        .into_root_schema_for::<T>()
}

/// The schema of an output type.
///
/// An optional field that is left out when absent loses its `null`: the
/// value is either there or missing, never `null`. An `Option` field that
/// is always written keeps it, and [`output_has_null`] reports it.
#[must_use]
pub fn output<T: JsonSchema>() -> Schema {
    let schema = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<T>();
    let mut value = schema.to_value();
    drop_null_from_optional_fields(&mut value);
    Schema::try_from(value).unwrap_or_else(|_| unreachable!("a schema stays an object"))
}

fn drop_null_from_optional_fields(value: &mut Value) {
    match value {
        Value::Object(o) => {
            let required: Vec<String> = o
                .get("required")
                .and_then(Value::as_array)
                .map(|r| {
                    r.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            if let Some(Value::Object(props)) = o.get_mut("properties") {
                for (name, prop) in props.iter_mut() {
                    if !required.contains(name) {
                        drop_null(prop);
                    }
                }
            }
            for v in o.values_mut() {
                drop_null_from_optional_fields(v);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(drop_null_from_optional_fields),
        _ => {}
    }
}

/// Removes `null` from the top level of one schema: from a `type` array and
/// from `anyOf` / `oneOf` branches.
fn drop_null(value: &mut Value) {
    let Value::Object(o) = value else { return };
    if let Some(Value::Array(types)) = o.get("type") {
        let kept: Vec<Value> = types.iter().filter(|t| *t != "null").cloned().collect();
        let new = if kept.len() == 1 {
            kept.into_iter().next().unwrap_or_default()
        } else {
            Value::Array(kept)
        };
        o.insert("type".into(), new);
    }
    if o.get("default").is_some_and(Value::is_null) {
        o.remove("default");
    }
    if let Some(Value::Array(e)) = o.get_mut("enum") {
        e.retain(|v| !v.is_null());
    }
    for key in ["anyOf", "oneOf"] {
        let Some(Value::Array(branches)) = o.get(key) else {
            continue;
        };
        if !branches.iter().any(is_null_schema) {
            continue;
        }
        let kept: Vec<Value> = branches
            .iter()
            .filter(|b| !is_null_schema(b))
            .cloned()
            .collect();
        o.remove(key);
        if kept.len() == 1 {
            if let Some(Value::Object(single)) = kept.into_iter().next() {
                for (k, v) in single {
                    o.entry(k).or_insert(v);
                }
            }
        } else {
            o.insert(key.into(), Value::Array(kept));
        }
    }
}

/// The whole catalog as one JSON document: every operation in catalog order
/// with its metadata and schemas, the error type and the change event.
#[must_use]
pub fn catalog_document() -> Value {
    let operations: Vec<Value> = CATALOG
        .iter()
        .map(|d| {
            let mut op = Map::new();
            op.insert("id".into(), json!(d.id));
            op.insert("summary".into(), json!(d.summary));
            op.insert(
                "kind".into(),
                json!(match d.kind {
                    OpKind::Query => "query",
                    OpKind::Command => "command",
                }),
            );
            op.insert("safety".into(), json!(d.safety.as_str()));
            match d.exposure {
                Exposure::Data => {
                    op.insert("exposure".into(), json!("data"));
                }
                Exposure::App { reason } => {
                    op.insert("exposure".into(), json!("app"));
                    op.insert("app_reason".into(), json!(reason));
                }
            }
            if let Some(tool) = mcp::tool_for(d.id) {
                op.insert("mcp_tool".into(), json!(tool.name));
            }
            if let Some(cli) = d.cli {
                op.insert("cli".into(), json!(cli));
            }
            op.insert("input_name".into(), json!(d.input_name));
            op.insert("output_name".into(), json!(d.output_name));
            op.insert("input".into(), (d.input_schema)().to_value());
            op.insert("output".into(), (d.output_schema)().to_value());
            Value::Object(op)
        })
        .collect();
    json!({
        "operations": operations,
        "error": output::<OpError>().to_value(),
        "event": output::<Changed>().to_value(),
    })
}

/// Rewrites `schema` for MCP clients:
///
/// - every `$ref` is replaced by the definition it points at, and `$defs`
///   and `$schema` are removed;
/// - `null` is removed from `type` arrays, and `null` branches from `anyOf`
///   and `oneOf` (a single branch left is merged into its parent);
/// - Rust number formats (`uint32`, `int64`, `double`, …) and `default:
///   null` are removed.
///
/// In inputs, a patch field that accepts `null` to clear a value loses the
/// `null` here; the engine still accepts it.
#[must_use]
pub fn portable(schema: &Value) -> Value {
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut out = inline(schema, &defs, 0);
    if let Some(o) = out.as_object_mut() {
        o.remove("$defs");
        o.remove("$schema");
    }
    strip(&mut out);
    out
}

fn inline(value: &Value, defs: &Map<String, Value>, depth: usize) -> Value {
    assert!(
        depth < 64,
        "a recursive type can't be inlined into a schema"
    );
    match value {
        Value::Object(o) => {
            if let Some(name) = o
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/$defs/"))
            {
                let target = defs
                    .get(name)
                    .unwrap_or_else(|| panic!("the schema refers to a missing type {name}"));
                let mut merged = match inline(target, defs, depth + 1) {
                    Value::Object(t) => t,
                    other => return other,
                };
                // Keys next to `$ref` (a field's description) win.
                for (k, v) in o {
                    if k != "$ref" {
                        merged.insert(k.clone(), inline(v, defs, depth + 1));
                    }
                }
                return Value::Object(merged);
            }
            Value::Object(
                o.iter()
                    .filter(|(k, _)| k.as_str() != "$defs")
                    .map(|(k, v)| (k.clone(), inline(v, defs, depth)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| inline(v, defs, depth)).collect()),
        other => other.clone(),
    }
}

fn is_null_schema(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("null")
        || v.get("const").is_some_and(Value::is_null)
}

fn strip(value: &mut Value) {
    match value {
        Value::Object(o) => {
            if o.get("format")
                .and_then(Value::as_str)
                .is_some_and(is_rust_format)
            {
                o.remove("format");
            }
            for v in o.values_mut() {
                strip(v);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
    drop_null(value);
}

fn is_rust_format(format: &str) -> bool {
    format.starts_with("int")
        || format.starts_with("uint")
        || format == "double"
        || format == "float"
}

/// Whether `schema` allows a `null` anywhere: a `null` type, a `null`
/// branch, or `null` in an `enum`. Outputs must not: absent values are left
/// out instead.
#[must_use]
pub fn output_has_null(schema: &Value) -> bool {
    match schema {
        Value::Object(o) => {
            let in_type = match o.get("type") {
                Some(Value::String(t)) => t == "null",
                Some(Value::Array(ts)) => ts.iter().any(|t| t == "null"),
                _ => false,
            };
            let in_enum = o
                .get("enum")
                .and_then(Value::as_array)
                .is_some_and(|e| e.iter().any(Value::is_null));
            in_type || in_enum || o.values().any(output_has_null)
        }
        Value::Array(a) => a.iter().any(output_has_null),
        _ => false,
    }
}

/// Whether `schema` still has anything [`portable`] removes.
#[must_use]
pub fn is_portable(schema: &Value) -> bool {
    match schema {
        Value::Object(o) => {
            let bad = o.contains_key("$ref")
                || o.contains_key("$defs")
                || o.get("format")
                    .and_then(Value::as_str)
                    .is_some_and(is_rust_format)
                || matches!(o.get("type"), Some(Value::Array(ts)) if ts.iter().any(|t| t == "null"))
                || ["anyOf", "oneOf"].iter().any(|k| {
                    o.get(*k)
                        .and_then(Value::as_array)
                        .is_some_and(|b| b.iter().any(is_null_schema))
                });
            !bad && o.values().all(is_portable)
        }
        Value::Array(a) => a.iter().all(is_portable),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct Patch {
        title: Option<String>,
        #[serde(default)]
        due: Option<Option<String>>,
        count: u32,
        nested: Inner,
    }

    #[derive(Serialize, Deserialize, JsonSchema)]
    enum Kind {
        A,
        B,
    }

    #[derive(Serialize, Deserialize, JsonSchema)]
    struct Inner {
        /// A kind.
        kind: Kind,
        other: Option<Kind>,
    }

    #[test]
    fn portable_inlines_and_drops_null_and_formats() {
        let raw = input::<Patch>().to_value();
        assert!(!is_portable(&raw), "the raw schema has refs and nulls");
        let p = portable(&raw);
        assert!(is_portable(&p), "{p:#}");
        assert_eq!(
            p["properties"]["count"],
            json!({ "type": "integer", "minimum": 0 })
        );
        assert_eq!(p["properties"]["title"]["type"], json!("string"));
        assert_eq!(
            p["properties"]["nested"]["properties"]["kind"]["enum"],
            json!(["A", "B"])
        );
        assert_eq!(
            p["properties"]["nested"]["properties"]["kind"]["description"],
            json!("A kind.")
        );
        assert!(!output_has_null(&p));
    }

    #[test]
    fn output_null_is_detected() {
        #[derive(Serialize, JsonSchema)]
        struct Out {
            due: Option<String>,
        }
        #[derive(Serialize, JsonSchema)]
        struct OutSkipped {
            #[serde(skip_serializing_if = "Option::is_none")]
            due: Option<String>,
        }
        assert!(output_has_null(&output::<Out>().to_value()));
        let skipped = output::<OutSkipped>().to_value();
        assert!(!output_has_null(&skipped));
        assert!(skipped.get("required").is_none());
    }

    #[test]
    fn every_output_leaves_absent_values_out() {
        for d in CATALOG {
            let schema = (d.output_schema)().to_value();
            assert!(
                !output_has_null(&schema),
                "{}: an output field can be null; add skip_serializing_if",
                d.id
            );
        }
        assert!(!output_has_null(&output::<OpError>().to_value()));
        assert!(!output_has_null(&output::<Changed>().to_value()));
    }

    #[test]
    fn every_input_refuses_unknown_fields() {
        for d in CATALOG {
            let schema = (d.input_schema)().to_value();
            assert_eq!(schema["type"], json!("object"), "{}", d.id);
            assert_eq!(
                schema["additionalProperties"],
                json!(false),
                "{}: add deny_unknown_fields",
                d.id
            );
        }
    }
}
