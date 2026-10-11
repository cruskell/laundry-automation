//! Runtime validation of generated OpenAPI request schemas (business guards live in endpoints).
use anyhow::{Result, ensure};
use serde_json::Value;
use std::sync::OnceLock;

pub fn validate(name: &str, value: &Value) -> Result<()> {
    static SCHEMAS: OnceLock<Value> = OnceLock::new();
    let schemas = SCHEMAS.get_or_init(|| {
        serde_json::from_str(include_str!("generated_api_schemas.json")).expect("generated schemas")
    });
    check(&schemas[name], value, schemas)
}

fn check(schema: &Value, value: &Value, schemas: &Value) -> Result<()> {
    if let Some(reference) = schema["$ref"].as_str() {
        return check(
            &schemas[reference
                .strip_prefix("#/components/schemas/")
                .expect("local schema")],
            value,
            schemas,
        );
    }
    if value.is_null() && schema["nullable"] == true {
        return Ok(());
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(choices) = schema[keyword].as_array() {
            let matches = choices
                .iter()
                .filter(|s| check(s, value, schemas).is_ok())
                .count();
            ensure!(
                matches > 0 && (keyword != "oneOf" || matches == 1),
                "Body does not match {keyword} request schema"
            );
        }
    }
    if let Some(choices) = schema["enum"].as_array() {
        ensure!(choices.contains(value), "Value is outside schema enum");
    }
    if let Some(kind) = schema["type"].as_str() {
        ensure!(
            match kind {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                _ => false,
            },
            "Expected schema type {kind}"
        );
    }
    if let Some(text) = value.as_str() {
        if let Some(min) = schema["minLength"].as_u64() {
            ensure!(text.chars().count() >= min as usize, "String is too short");
        }
        if let Some(pattern) = schema["pattern"].as_str() {
            ensure!(
                regex::Regex::new(pattern)?.is_match(text),
                "String does not match schema pattern"
            );
        }
    }
    if let Some(number) = value.as_f64() {
        if let Some(min) = schema["minimum"].as_f64() {
            ensure!(number >= min, "Number is below schema minimum");
        }
        if let Some(max) = schema["maximum"].as_f64() {
            ensure!(number <= max, "Number exceeds schema maximum");
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(min) = schema["minItems"].as_u64() {
            ensure!(items.len() >= min as usize, "Array is too short");
        }
        if let Some(max) = schema["maxItems"].as_u64() {
            ensure!(items.len() <= max as usize, "Array is too long");
        }
        if !schema["items"].is_null() {
            for item in items {
                check(&schema["items"], item, schemas)?;
            }
        }
    }
    if let Some(fields) = value.as_object() {
        if let Some(min) = schema["minProperties"].as_u64() {
            ensure!(
                fields.len() >= min as usize,
                "Object has too few properties"
            );
        }
        if let Some(max) = schema["maxProperties"].as_u64() {
            ensure!(
                fields.len() <= max as usize,
                "Object has too many properties"
            );
        }
        if let Some(required) = schema["required"].as_array() {
            for key in required {
                ensure!(
                    fields.contains_key(key.as_str().expect("schema key")),
                    "Missing required field {key}"
                );
            }
        }
        for (key, item) in fields {
            if let Some(property) = schema["properties"].get(key) {
                check(property, item, schemas)?;
            } else if schema["additionalProperties"].is_object() {
                check(&schema["additionalProperties"], item, schemas)?;
            } else {
                ensure!(
                    schema["additionalProperties"] != false,
                    "Unknown field {key}"
                );
            }
        }
    }
    Ok(())
}
