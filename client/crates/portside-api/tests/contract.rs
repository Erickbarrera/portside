//! Contract test: the Rust models must accept every file shape the static catalog's document
//! allows, and serialize back to the same JSON (so no field is dropped or invented). The
//! document is `docs/specs/static-catalog/v1.json`, written by `pnpm contract:write` in
//! `catalog/` from the builder's schemas; CI checks it is current.

use std::collections::BTreeSet;
use std::fmt::Debug;

use portside_api::models::{CatalogSnapshot, Game, PortDetail, Release};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

const OPENAPI: &str = include_str!("../../../../docs/specs/static-catalog/v1.json");

/// How many examples to generate per schema. Each choice point (union branch, enum value,
/// optional field present or not) rotates, so this covers every enum value (the largest
/// enum has 8) and both sides of every optional/nullable field.
const VARIANTS: usize = 16;

fn doc() -> Value {
    serde_json::from_str(OPENAPI).expect("openapi document parses")
}

fn ok_schema<'a>(doc: &'a Value, path: &str) -> &'a Value {
    let schema =
        &doc["paths"][path]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
    assert!(
        !schema.is_null(),
        "{path}: no 200 application/json schema in the document"
    );
    schema
}

/// Generates example values for a JSON Schema (the subset TypeBox emits).
struct Gen<'a> {
    doc: &'a Value,
    variant: usize,
    choice: usize,
}

impl Gen<'_> {
    fn pick(&mut self, n: usize) -> usize {
        self.choice += 1;
        (self.variant + self.choice) % n
    }

    fn value(&mut self, schema: &Value) -> Value {
        if let Some(r) = schema["$ref"].as_str() {
            let name = r.trim_start_matches("#/components/schemas/");
            let target = self.doc["components"]["schemas"][name].clone();
            return self.value(&target);
        }
        if let Some(branches) = schema["anyOf"].as_array().or(schema["oneOf"].as_array()) {
            let i = self.pick(branches.len());
            return self.value(&branches[i]);
        }
        if let Some(c) = schema.get("const") {
            return c.clone();
        }
        if let Some(values) = schema["enum"].as_array() {
            let i = self.pick(values.len());
            return values[i].clone();
        }
        match schema["type"].as_str() {
            Some("object") => self.object(schema),
            Some("array") => {
                let item = self.value(&schema["items"]);
                Value::Array(vec![item])
            }
            Some("string") => match schema["format"].as_str() {
                Some("date-time") => json!("2026-01-02T03:04:05.000Z"),
                _ => json!("synthetic"),
            },
            Some("integer") => json!(schema["minimum"].as_i64().unwrap_or(1).max(1)),
            Some("number") => json!(1.5),
            Some("boolean") => json!(self.pick(2) == 0),
            Some("null") => Value::Null,
            other => panic!("schema type {other:?} not handled by the generator: {schema}"),
        }
    }

    fn object(&mut self, schema: &Value) -> Value {
        let required: BTreeSet<&str> = schema["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut out = Map::new();
        if let Some(props) = schema["properties"].as_object() {
            for (name, prop) in props {
                if required.contains(name.as_str()) || self.pick(2) == 0 {
                    out.insert(name.clone(), self.value(prop));
                }
            }
        }
        // Records (`Type.Record`): one synthetic entry.
        if let Some(extra) = schema.get("additionalProperties").filter(|v| v.is_object()) {
            out.insert("KEY".into(), self.value(extra));
        }
        if let Some(patterns) = schema["patternProperties"].as_object() {
            for prop in patterns.values() {
                out.insert("KEY".into(), self.value(prop));
            }
        }
        Value::Object(out)
    }
}

fn assert_round_trips<T: DeserializeOwned + Serialize + Debug>(path: &str) {
    let doc = doc();
    let schema = ok_schema(&doc, path);
    for variant in 0..VARIANTS {
        let example = Gen {
            doc: &doc,
            variant,
            choice: 0,
        }
        .value(schema);
        let parsed: T = serde_json::from_value(example.clone()).unwrap_or_else(|e| {
            panic!(
                "{path}: Rust model rejects a file the catalog may publish: {e}\n{}",
                serde_json::to_string_pretty(&example).unwrap_or_default()
            )
        });
        let back = serde_json::to_value(&parsed).expect("serialize");
        assert_eq!(
            back, example,
            "{path}: Rust model does not round-trip (a field is missing or extra in the Rust model)"
        );
    }
}

#[test]
fn port_detail() {
    assert_round_trips::<PortDetail>("/v1/ports/{id}.json");
}

#[test]
fn releases() {
    assert_round_trips::<Vec<Release>>("/v1/ports/{id}/releases.json");
}

#[test]
fn game() {
    assert_round_trips::<Game>("/v1/games/{game_key}.json");
}

#[test]
fn catalog_snapshot() {
    assert_round_trips::<CatalogSnapshot>("/v1/snapshot.json");
}

/// Every file the Rust client reads must exist in the document.
#[test]
fn client_files_exist() {
    let doc = doc();
    for path in [
        "/v1/ports/{id}.json",
        "/v1/ports/{id}/releases.json",
        "/v1/games/{game_key}.json",
        "/v1/snapshot.json",
    ] {
        assert!(
            doc["paths"][path]["get"].is_object(),
            "{path} is not in the document"
        );
    }
}
