//! IPC reference (markdown) and JSON Schema, both from `rc_ipc::method` and `rc_ipc::schema`.

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::Result;
use rc_ipc::method::MethodSpec;
use rc_ipc::schema::Settings;
use schemars::SchemaGenerator;
use serde_json::{json, Map, Value};

use crate::docs::HEADER;

/// Everything the IPC docs need, from one generator run.
struct Ipc {
    specs: Vec<MethodSpec>,
    defs: BTreeMap<String, Value>,
}

fn build() -> Ipc {
    let mut generator = SchemaGenerator::default();
    let specs = rc_ipc::method::specs(&mut generator);
    // Not used by any method, but it is a data type the docs must cover.
    generator.subschema_for::<Settings>();
    let defs = generator
        .definitions()
        .iter()
        .map(|(name, schema)| (name.clone(), schema.clone()))
        .collect();
    Ipc { specs, defs }
}

pub fn markdown() -> String {
    let ipc = build();
    let mut out = String::new();
    let _ = write!(out, "<!-- {HEADER} -->\n\n# IPC reference\n\n");
    out.push_str(WIRE);
    out.push_str("\n## Methods\n\n| Method | Params | Result | Errors |\n|---|---|---|---|\n");
    for spec in &ipc.specs {
        let params = spec
            .params
            .as_ref()
            .map_or_else(|| "none".to_string(), |p| label(p.as_value()));
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | `error` string |",
            spec.method.name(),
            params,
            label(spec.result.as_value())
        );
    }
    let _ = write!(
        out,
        "\nTauri GUI commands mirror these names with camelCase args. The GUI exposes `ping` as `daemon_info` and `shutdown` as `stop_daemon`.\n\n## Types\n\n"
    );
    for (name, schema) in &ipc.defs {
        out.push_str(&render_def(name, schema));
    }
    out
}

pub fn json_schema() -> Result<String> {
    let ipc = build();
    let methods: Map<String, Value> = ipc
        .specs
        .iter()
        .map(|spec| {
            let params = spec
                .params
                .as_ref()
                .map_or(Value::Null, |p| p.as_value().clone());
            let entry = json!({ "params": params, "result": spec.result.as_value() });
            (spec.method.name().to_string(), entry)
        })
        .collect();
    let defs: Map<String, Value> = ipc.defs.into_iter().collect();
    let doc = json!({
        "$comment": HEADER,
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "rc-keeper IPC",
        "description": "Params and results of each daemon method. Methods: see the methods object.",
        "methods": methods,
        "$defs": defs,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&doc)?))
}

const WIRE: &str = "\
The daemon listens on a local socket. Each request and each response is one JSON object on one line.

Request: `{\"id\": 1, \"method\": \"<name>\", \"params\": {...}}`

Response: `{\"id\": 1, \"result\": <value>}` on success, or `{\"id\": 1, \"error\": \"<message>\"}` on failure.

Errors are plain strings. Unknown methods return `unknown method: <name>`. Bad params return `bad params: <reason>`. Other messages come from the daemon manager.
";

/// Type label for a JSON Schema node. Named types link to their section.
fn label(v: &Value) -> String {
    if let Some(reference) = v.get("$ref").and_then(Value::as_str) {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        return format!("[{name}](#{})", name.to_lowercase());
    }
    if let Some(values) = v.get("enum").and_then(Value::as_array) {
        return values.iter().map(code).collect::<Vec<_>>().join(" \\| ");
    }
    match v.get("type") {
        Some(Value::String(t)) if t == "array" => {
            let items = v.get("items").map_or_else(|| "any".to_string(), label);
            format!("{items}[]")
        }
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(types)) => types
            .iter()
            .map(|t| t.as_str().unwrap_or("any").to_string())
            .collect::<Vec<_>>()
            .join(" \\| "),
        _ => {
            let variants = v.get("anyOf").or_else(|| v.get("oneOf"));
            match variants.and_then(Value::as_array) {
                Some(list) => list.iter().map(label).collect::<Vec<_>>().join(" \\| "),
                None => "any".to_string(),
            }
        }
    }
}

fn code(v: &Value) -> String {
    match v.as_str() {
        Some(s) => format!("`{s}`"),
        None => format!("`{v}`"),
    }
}

fn render_def(name: &str, schema: &Value) -> String {
    let mut out = format!("### {name}\n\n");
    if let Some(desc) = schema.get("description").and_then(Value::as_str) {
        let _ = write!(out, "{}\n\n", one_line(desc));
    }
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        let required: Vec<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        out.push_str("| Field | Type | Required | Description |\n|---|---|---|---|\n");
        for (field, prop) in props {
            let req = if required.contains(&field.as_str()) {
                "yes"
            } else {
                "no"
            };
            let desc = prop
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let _ = writeln!(
                out,
                "| `{field}` | {} | {req} | {} |",
                label(prop),
                one_line(desc).replace('|', "\\|")
            );
        }
    } else {
        let _ = writeln!(out, "Type: {}", label(schema));
    }
    out.push('\n');
    out
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
