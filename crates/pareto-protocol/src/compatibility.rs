use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::{ErrorCode, ValidationError};

/// Conservatively proves the supported old-writer to new-reader schema evolution subset.
///
/// The V1 proof accepts byte-identical contracts (apart from `$id`) and recursively added
/// non-required object properties. Every other change fails closed and requires a major bump or
/// a future reviewed proof rule.
pub fn prove_old_writer_new_reader(old: &Value, new: &Value) -> Result<(), ValidationError> {
    let (old_name, old_major, old_minor) = schema_identity(old)?;
    let (new_name, new_major, new_minor) = schema_identity(new)?;
    if old == new {
        return Ok(());
    }
    if old_name != new_name || old_major != new_major || new_minor <= old_minor {
        return Err(incompatible(
            "/$id",
            "schema type and major must match and minor must strictly increase",
        ));
    }
    if protected_graph(old, "") != protected_graph(new, "") {
        return Err(incompatible(
            "",
            "composition or reference change cannot be proven by V1",
        ));
    }
    prove(old, new, "")
}

fn schema_identity(schema: &Value) -> Result<(String, u32, u32), ValidationError> {
    let id = schema
        .get("$id")
        .and_then(Value::as_str)
        .ok_or_else(|| incompatible("/$id", "schema ID is required"))?;
    let suffix = id
        .strip_prefix("urn:pareto-harness:schema:")
        .ok_or_else(|| incompatible("/$id", "schema ID prefix is invalid"))?;
    let (name, version) = suffix
        .rsplit_once(':')
        .ok_or_else(|| incompatible("/$id", "schema ID lacks version"))?;
    let (major, minor) = version
        .split_once('.')
        .ok_or_else(|| incompatible("/$id", "schema ID lacks major/minor"))?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return Err(incompatible("/$id", "schema type is invalid"));
    }
    let parse_component = |component: &str, label: &str| {
        if component.is_empty()
            || (component.len() > 1 && component.starts_with('0'))
            || !component.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(incompatible(
                "/$id",
                &format!("invalid canonical {label} version"),
            ));
        }
        component
            .parse()
            .map_err(|_| incompatible("/$id", &format!("invalid {label} version")))
    };
    Ok((
        name.to_owned(),
        parse_component(major, "major")?,
        parse_component(minor, "minor")?,
    ))
}

fn protected_graph(value: &Value, path: &str) -> Vec<(String, Value)> {
    let mut graph = Vec::new();
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}/{key}");
                if matches!(
                    key.as_str(),
                    "$ref" | "$dynamicRef" | "oneOf" | "anyOf" | "allOf" | "not"
                ) || (key == "$id" && !path.is_empty())
                {
                    graph.push((child_path.clone(), child.clone()));
                }
                graph.extend(protected_graph(child, &child_path));
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                graph.extend(protected_graph(child, &format!("{path}/{index}")));
            }
        }
        _ => {}
    }
    graph
}

fn prove(old: &Value, new: &Value, path: &str) -> Result<(), ValidationError> {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => prove_object(old, new, path),
        (Value::Array(old), Value::Array(new)) if old.len() == new.len() => {
            for (index, (old, new)) in old.iter().zip(new).enumerate() {
                prove(old, new, &format!("{path}/{index}"))?;
            }
            Ok(())
        }
        _ if old == new => Ok(()),
        _ => Err(incompatible(
            path,
            "change is outside the conservative compatibility proof",
        )),
    }
}

fn prove_object(
    old: &Map<String, Value>,
    new: &Map<String, Value>,
    path: &str,
) -> Result<(), ValidationError> {
    let old_required = required(old)?;
    let new_required = required(new)?;
    if !new_required.is_subset(&old_required) {
        return Err(incompatible(path, "new reader adds a required field"));
    }

    for (key, old_value) in old {
        if key == "$id" || key == "required" || key == "properties" {
            continue;
        }
        let new_value = new
            .get(key)
            .ok_or_else(|| incompatible(&format!("{path}/{key}"), "keyword removed"))?;
        prove(old_value, new_value, &format!("{path}/{key}"))?;
    }
    for key in new.keys() {
        if key == "$id" || key == "required" || key == "properties" {
            continue;
        }
        if !old.contains_key(key) {
            return Err(incompatible(
                &format!("{path}/{key}"),
                "new keyword is not in the V1 whitelist",
            ));
        }
    }

    let empty = Map::new();
    let old_properties = old
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let new_properties = new
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for (name, old_schema) in old_properties {
        let new_schema = new_properties.get(name).ok_or_else(|| {
            incompatible(&format!("{path}/properties/{name}"), "property removed")
        })?;
        prove(old_schema, new_schema, &format!("{path}/properties/{name}"))?;
    }
    for name in new_properties.keys() {
        if !old_properties.contains_key(name) && new_required.contains(name) {
            return Err(incompatible(
                &format!("{path}/properties/{name}"),
                "added property is required",
            ));
        }
    }
    Ok(())
}

fn required(object: &Map<String, Value>) -> Result<BTreeSet<String>, ValidationError> {
    object.get("required").map_or_else(
        || Ok(BTreeSet::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| incompatible("/required", "required must be an array"))?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        incompatible("/required", "required entries must be strings")
                    })
                })
                .collect()
        },
    )
}

fn incompatible(path: &str, detail: &str) -> ValidationError {
    ValidationError {
        code: ErrorCode::SchemaMismatch,
        path: path.to_owned(),
        contract: "old_writer_new_reader".to_owned(),
        detail: detail.to_owned(),
    }
}
