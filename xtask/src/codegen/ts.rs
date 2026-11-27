// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The TypeScript client, generated from the catalog's JSON Schemas.
//!
//! It handles the subset of JSON Schema the catalog uses: objects, arrays,
//! maps, string enums, constants, nullable types, `$ref`, `oneOf` and
//! `anyOf`. Types shared between operations are written once; two different
//! types with the same name stop the generator.

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};

use super::sorted;

/// The generated file.
pub(crate) fn generate(catalog: &Value) -> Result<String> {
    let ops = catalog["operations"]
        .as_array()
        .context("the catalog has no operations")?;
    let mut types = Types::default();
    for op in ops {
        let id = str_of(op, "id")?;
        types
            .add(str_of(op, "input_name")?, &op["input"])
            .with_context(|| format!("input of {id}"))?;
        types
            .add(str_of(op, "output_name")?, &op["output"])
            .with_context(|| format!("output of {id}"))?;
    }
    types.add("OpError", &catalog["error"])?;
    types.add("Changed", &catalog["event"])?;

    let mut s = String::new();
    s += "// SPDX-License-Identifier: GPL-3.0-or-later\n";
    s += "// Copyright (C) 2026 Glyphkeel contributors\n\n";
    s += "// Generated from the operation catalog by `cargo xtask gen`. Do not edit:\n";
    s += "// change the catalog in crates/gk-api and run `cargo xtask gen` again.\n\n";

    for (name, schema) in &types.0 {
        s += &doc(schema, "");
        writeln!(s, "export type {name} = {};\n", ts(schema, "")?)?;
    }

    s += "/** An error from the IPC layer itself, before the engine saw the call. */\n";
    s += "export interface IpcError {\n  code: \"ipc\";\n  message: string;\n}\n\n";
    s += "/** Every error a call can end with. */\n";
    s += "export type CallError = OpError | IpcError;\n\n";

    s += "/** Every operation: its input, its output, its safety class and kind. */\n";
    s += "export interface OpMap {\n";
    for op in ops {
        writeln!(
            s,
            "  {:?}: {{ input: {}; output: {}; safety: {:?}; kind: {:?} }};",
            str_of(op, "id")?,
            str_of(op, "input_name")?,
            str_of(op, "output_name")?,
            str_of(op, "safety")?,
            str_of(op, "kind")?,
        )?;
    }
    s += "}\n\n";
    s += "/** The name of an operation. */\n";
    s += "export type OpId = keyof OpMap;\n\n";
    s += "/** The safety class of every operation. */\n";
    s += "export const OP_SAFETY = {\n";
    for op in ops {
        writeln!(s, "  {:?}: {:?},", str_of(op, "id")?, str_of(op, "safety")?)?;
    }
    s += "} as const satisfies { [K in OpId]: OpMap[K][\"safety\"] };\n\n";
    s +=
        "/** Sends one call; the transport (Tauri IPC or the development bridge) supplies it. */\n";
    s += "export type Call = <K extends OpId>(op: K, input: OpMap[K][\"input\"]) => Promise<OpMap[K][\"output\"]>;\n\n";

    s += "/** One typed function per operation, over any transport. */\n";
    s += "export function makeOps(call: Call) {\n  return {\n";
    let mut areas: BTreeMap<&str, Vec<(String, &Value)>> = BTreeMap::new();
    for op in ops {
        let id = str_of(op, "id")?;
        let (area, verb) = id
            .split_once('.')
            .ok_or_else(|| anyhow!("{id} is not area.verb"))?;
        areas.entry(area).or_default().push((camel(verb), op));
    }
    for (area, verbs) in areas {
        writeln!(s, "    {area}: {{")?;
        for (verb, op) in verbs {
            let id = str_of(op, "id")?;
            writeln!(
                s,
                "      /** {} ({}) */",
                comment(str_of(op, "summary")?),
                str_of(op, "safety")?
            )?;
            let default = if has_required(&op["input"]) {
                ""
            } else {
                " = {}"
            };
            writeln!(
                s,
                "      {verb}: (input: OpMap[{id:?}][\"input\"]{default}) => call({id:?}, input),"
            )?;
        }
        s += "    },\n";
    }
    s += "  };\n}\n\n";
    s += "/** The typed operations. */\n";
    s += "export type Ops = ReturnType<typeof makeOps>;\n";
    Ok(s)
}

/// Named types, in name order.
#[derive(Default)]
struct Types(BTreeMap<String, Value>);

impl Types {
    fn add(&mut self, name: &str, schema: &Value) -> Result<()> {
        let mut root = sorted(schema);
        let Some(o) = root.as_object_mut() else {
            bail!("the schema of {name} is not an object");
        };
        if let Some(Value::Object(defs)) = o.remove("$defs") {
            for (n, s) in defs {
                self.insert(&n, s)?;
            }
        }
        o.remove("$schema");
        o.remove("title");
        self.insert(name, root)
    }

    fn insert(&mut self, name: &str, schema: Value) -> Result<()> {
        if let Some(prev) = self.0.get(name)
            && *prev != schema
        {
            bail!(
                "two different types are named {name}; give one of them another name \
                 (a type used in both inputs and outputs must not differ between them)"
            );
        }
        self.0.insert(name.to_owned(), schema);
        Ok(())
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| anyhow!("the catalog entry has no {key}"))
}

fn has_required(schema: &Value) -> bool {
    schema["required"].as_array().is_some_and(|r| !r.is_empty())
}

/// `snake_case` to `camelCase`.
fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in s.chars() {
        if c == '_' || c == '.' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn comment(text: &str) -> String {
    text.replace("*/", "* /").replace('\n', " ")
}

fn doc(schema: &Value, indent: &str) -> String {
    match schema.get("description").and_then(Value::as_str) {
        Some(d) => format!("{indent}/** {} */\n", comment(d)),
        None => String::new(),
    }
}

fn ident_ok(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn prop_name(s: &str) -> String {
    if ident_ok(s) {
        s.to_owned()
    } else {
        format!("{s:?}")
    }
}

fn ts(schema: &Value, indent: &str) -> Result<String> {
    let Some(o) = schema.as_object() else {
        return Ok(if *schema == Value::Bool(false) {
            "never".into()
        } else {
            "unknown".into()
        });
    };
    if let Some(r) = o.get("$ref").and_then(Value::as_str) {
        return r
            .strip_prefix("#/$defs/")
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("unsupported reference {r}"));
    }
    if let Some(c) = o.get("const") {
        return Ok(serde_json::to_string(c)?);
    }
    if let Some(e) = o.get("enum").and_then(Value::as_array) {
        let parts: Result<Vec<String>> = e.iter().map(|v| Ok(serde_json::to_string(v)?)).collect();
        return Ok(parts?.join(" | "));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(alts) = o.get(key).and_then(Value::as_array) {
            let mut parts: Vec<String> = Vec::new();
            for alt in alts {
                let t = ts(alt, indent)?;
                if !parts.contains(&t) {
                    parts.push(t);
                }
            }
            return Ok(parts.join(" | "));
        }
    }
    if let Some(all) = o.get("allOf").and_then(Value::as_array) {
        let parts: Result<Vec<String>> = all.iter().map(|a| ts(a, indent)).collect();
        return Ok(parts?.join(" & "));
    }
    match o.get("type") {
        Some(Value::Array(types)) => {
            let mut parts = Vec::new();
            for t in types {
                let mut single = o.clone();
                single.insert("type".into(), t.clone());
                parts.push(ts(&Value::Object(single), indent)?);
            }
            Ok(parts.join(" | "))
        }
        Some(Value::String(t)) => Ok(match t.as_str() {
            "string" => "string".into(),
            "integer" | "number" => "number".into(),
            "boolean" => "boolean".into(),
            "null" => "null".into(),
            "array" => {
                let item = match o.get("items") {
                    Some(i) => ts(i, indent)?,
                    None => "unknown".into(),
                };
                if item.contains(' ') {
                    format!("({item})[]")
                } else {
                    format!("{item}[]")
                }
            }
            "object" => object(o, indent)?,
            other => bail!("unsupported type {other}"),
        }),
        _ => Ok("unknown".into()),
    }
}

fn object(o: &Map<String, Value>, indent: &str) -> Result<String> {
    let required: Vec<&str> = o
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    match o.get("properties").and_then(Value::as_object) {
        Some(props) if !props.is_empty() => {
            let inner = format!("{indent}  ");
            let mut s = String::from("{\n");
            for (k, v) in props {
                s += &doc(v, &inner);
                let optional = if required.contains(&k.as_str()) {
                    ""
                } else {
                    "?"
                };
                writeln!(s, "{inner}{}{optional}: {};", prop_name(k), ts(v, &inner)?)?;
            }
            s += indent;
            s += "}";
            Ok(s)
        }
        _ => Ok(match o.get("additionalProperties") {
            Some(Value::Bool(false)) => "Record<string, never>".into(),
            Some(extra @ Value::Object(_)) => format!("Record<string, {}>", ts(extra, indent)?),
            _ => "Record<string, unknown>".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn types_for_the_schema_subset() {
        let schema = json!({
            "type": "object",
            "required": ["a", "kind"],
            "additionalProperties": false,
            "properties": {
                "a": { "type": "string", "description": "An a." },
                "b": { "type": ["integer", "null"] },
                "kind": { "oneOf": [ { "const": "x" }, { "const": "y" } ] },
                "tags": { "type": "array", "items": { "$ref": "#/$defs/Tag" } },
                "map": { "type": "object", "additionalProperties": { "type": "boolean" } },
                "any": true,
                "odd-name": { "anyOf": [ { "$ref": "#/$defs/Tag" }, { "type": "null" } ] }
            }
        });
        let out = ts(&schema, "").expect("generates");
        assert!(out.contains("/** An a. */\n  a: string;"), "{out}");
        assert!(out.contains("b?: number | null;"), "{out}");
        assert!(out.contains("kind: \"x\" | \"y\";"), "{out}");
        assert!(out.contains("tags?: Tag[];"), "{out}");
        assert!(out.contains("map?: Record<string, boolean>;"), "{out}");
        assert!(out.contains("any?: unknown;"), "{out}");
        assert!(out.contains("\"odd-name\"?: Tag | null;"), "{out}");
    }

    #[test]
    fn conflicting_names_stop_the_generator() {
        let mut types = Types::default();
        types.add("A", &json!({ "type": "string" })).expect("first");
        types
            .add("A", &json!({ "type": "string" }))
            .expect("same again");
        assert!(types.add("A", &json!({ "type": "number" })).is_err());
    }

    #[test]
    fn verbs_become_camel_case() {
        assert_eq!(camel("policy.update"), "policyUpdate");
        assert_eq!(camel("move_to_project"), "moveToProject");
    }
}
