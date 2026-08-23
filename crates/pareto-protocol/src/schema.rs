use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::Value;

use crate::{
    ArtifactManifest, BoundaryInventoryHashView, BoundaryInventoryRevision,
    BoundaryReconciliationHashView, BoundaryReconciliationRevision, EventEnvelope, EvidenceRecord,
    ProtocolLimitsProfileV1, RevisionHashView, RevisionMetadata, RunManifest, SchemaRef,
    SchemaSetManifest, SchemaSetRef, ValidationError, digest_json, digest_schema,
};

/// A generated public JSON Schema and its stable filename.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaDocument {
    /// Repository-relative filename.
    pub filename: String,
    /// JSON Schema Draft 2020-12 document.
    pub document: Value,
}

/// Deterministically generated schemas plus the manifest and reference that pin the set.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedSchemaBundle {
    /// Public schema documents.
    pub schemas: Vec<SchemaDocument>,
    /// Manifest listing exact member SchemaRefs.
    pub manifest: SchemaSetManifest,
    /// Digest-bearing reference to the manifest.
    pub reference: SchemaSetRef,
}

/// Generates every V1 top-level public schema in deterministic filename order.
pub fn generate_schema_set() -> Result<Vec<SchemaDocument>, ValidationError> {
    let mut schemas = vec![
        generate::<ArtifactManifest>("artifact-manifest", 1, 0)?,
        generate::<BoundaryInventoryHashView>("boundary-inventory-hash-view", 1, 0)?,
        generate::<BoundaryInventoryRevision>("boundary-inventory-revision", 1, 0)?,
        generate::<BoundaryReconciliationHashView>("boundary-reconciliation-hash-view", 1, 0)?,
        generate::<BoundaryReconciliationRevision>("boundary-reconciliation-revision", 1, 0)?,
        generate::<EventEnvelope>("event-envelope", 1, 0)?,
        generate::<EvidenceRecord>("evidence-record", 1, 0)?,
        generate::<ProtocolLimitsProfileV1>("protocol-limits-profile", 1, 0)?,
        generate::<RevisionHashView>("revision-hash-view", 1, 0)?,
        generate::<RevisionMetadata>("revision-metadata", 1, 0)?,
        generate::<RunManifest>("run-manifest", 1, 0)?,
        generate::<SchemaSetManifest>("schema-set-manifest", 1, 0)?,
    ];
    schemas.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(schemas)
}

/// Generates the complete initial schema bundle and immutable set identity.
pub fn generate_schema_bundle() -> Result<GeneratedSchemaBundle, ValidationError> {
    let schemas = generate_schema_set()?;
    let mut members = Vec::with_capacity(schemas.len());
    for schema in &schemas {
        let schema_id = schema.document["$id"]
            .as_str()
            .expect("generated schemas have IDs");
        let (name, major, minor) = parse_schema_id(schema_id)?;
        members.push(SchemaRef {
            r#type: name,
            major,
            minor,
            schema_digest: digest_schema(schema_id, &schema.document)?,
        });
    }
    members.sort();
    let event_envelope_schema_ref = members
        .iter()
        .find(|schema| schema.r#type == "event-envelope")
        .cloned()
        .expect("event envelope schema is generated");
    let manifest = SchemaSetManifest {
        schemas: members,
        event_envelope_schema_ref,
        event_bindings: Vec::new(),
    };
    let manifest_schema_ref = manifest
        .schemas
        .iter()
        .find(|schema| schema.r#type == "schema-set-manifest")
        .cloned()
        .expect("manifest schema is generated");
    let manifest_value = serde_json::to_value(&manifest).expect("manifest serializes");
    let reference = SchemaSetRef {
        manifest_digest: digest_json("schema-set", &manifest_schema_ref, &manifest_value)?,
        manifest_schema_ref,
    };
    Ok(GeneratedSchemaBundle {
        schemas,
        manifest,
        reference,
    })
}

fn parse_schema_id(schema_id: &str) -> Result<(String, u32, u32), ValidationError> {
    let suffix = schema_id
        .strip_prefix("urn:pareto-harness:schema:")
        .ok_or_else(schema_id_error)?;
    let (name, version) = suffix.rsplit_once(':').ok_or_else(schema_id_error)?;
    let (major, minor) = version.split_once('.').ok_or_else(schema_id_error)?;
    Ok((
        name.to_owned(),
        major.parse().map_err(|_| schema_id_error())?,
        minor.parse().map_err(|_| schema_id_error())?,
    ))
}

fn schema_id_error() -> ValidationError {
    crate::ValidationError {
        code: crate::ErrorCode::SchemaMismatch,
        path: "/$id".to_owned(),
        contract: "schema_id".to_owned(),
        detail: "invalid generated schema ID".to_owned(),
    }
}

fn generate<T: JsonSchema + Serialize>(
    name: &str,
    major: u32,
    minor: u32,
) -> Result<SchemaDocument, ValidationError> {
    let schema = schema_for!(T);
    let mut document = serde_json::to_value(schema).map_err(|_| crate::ValidationError {
        code: crate::ErrorCode::InvariantViolation,
        path: String::new(),
        contract: "json_schema".to_owned(),
        detail: "schema serialization failed".to_owned(),
    })?;
    let object = document
        .as_object_mut()
        .expect("schemars root is an object");
    object.insert(
        "$schema".to_owned(),
        Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
    );
    object.insert(
        "$id".to_owned(),
        Value::String(format!("urn:pareto-harness:schema:{name}:{major}.{minor}")),
    );
    object.insert("unevaluatedProperties".to_owned(), Value::Bool(false));
    harden_schema(name, &mut document);
    Ok(SchemaDocument {
        filename: format!("{name}-v{major}.{minor}.schema.json"),
        document,
    })
}

fn harden_schema(name: &str, document: &mut Value) {
    strip_optional_null(document);
    if let Some(defs) = document.get_mut("$defs").and_then(Value::as_object_mut) {
        for (definition, schema) in defs {
            if definition == "Digest" {
                set_string_contract(schema, r"^sha256:[0-9a-f]{64}$", 71, 71);
            } else if let Some(prefix) = id_prefix(definition) {
                set_string_contract(
                    schema,
                    &format!(r"^{prefix}[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"),
                    prefix.len() + 1,
                    128,
                );
            }
        }
    }
    harden_common(document);
    harden_named_properties(document);
    if name == "event-envelope" {
        if let Some(properties) = document
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            set_string_contract(
                properties.get_mut("sequence").expect("event sequence"),
                r"^[1-9][0-9]*$",
                1,
                128,
            );
            set_string_contract(
                properties.get_mut("occurred_at").expect("event time"),
                r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$",
                24,
                24,
            );
        }
    }
    if name == "run-manifest" {
        if let Some(revisions) = document
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .and_then(|properties| properties.get_mut("revisions"))
            .and_then(Value::as_object_mut)
        {
            let revision = serde_json::json!({"$ref":"#/$defs/RevisionId"});
            let roles = [
                "task",
                "behavior",
                "workspace",
                "environment",
                "context_graph",
                "model_snapshot",
                "tool_set",
                "kernel",
            ];
            revisions.clear();
            revisions.insert("type".to_owned(), Value::String("object".to_owned()));
            revisions.insert(
                "properties".to_owned(),
                Value::Object(
                    roles
                        .iter()
                        .map(|role| ((*role).to_owned(), revision.clone()))
                        .collect(),
                ),
            );
            revisions.insert(
                "required".to_owned(),
                Value::Array(
                    roles
                        .iter()
                        .map(|role| Value::String((*role).to_owned()))
                        .collect(),
                ),
            );
            revisions.insert("additionalProperties".to_owned(), Value::Bool(false));
        }
    }
}

fn harden_named_properties(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
                for (name, schema) in properties {
                    match name.as_str() {
                        "created_at" | "occurred_at" | "observed_at" => set_string_contract(
                            schema,
                            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$",
                            24,
                            24,
                        ),
                        "sequence" | "final_event_sequence" => {
                            set_string_contract(schema, r"^[1-9][0-9]*$", 1, 128)
                        }
                        "byte_length" => set_string_contract(schema, r"^(0|[1-9][0-9]*)$", 1, 128),
                        "type" => {
                            set_string_contract(schema, r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$", 1, 128)
                        }
                        "logical_id" | "revision_kind" | "artifact_kind" | "media_type"
                        | "event_type" | "variant_id" | "correlation_id" | "claim"
                        | "evidence_type" | "evidence_scope" | "freshness" | "source"
                        | "boundary_kind" | "reason_code" => {
                            if let Some(object) = schema.as_object_mut() {
                                object.insert("minLength".to_owned(), Value::Number(1_u64.into()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            for child in object.values_mut() {
                harden_named_properties(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                harden_named_properties(child);
            }
        }
        _ => {}
    }
}

fn strip_optional_null(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(options) = object.get_mut("anyOf").and_then(Value::as_array_mut) {
                if options.len() == 2
                    && options
                        .iter()
                        .any(|option| option.get("type") == Some(&Value::String("null".to_owned())))
                {
                    let replacement = options
                        .iter()
                        .find(|option| {
                            option.get("type") != Some(&Value::String("null".to_owned()))
                        })
                        .cloned()
                        .expect("non-null option");
                    let description = object.remove("description");
                    *object = replacement
                        .as_object()
                        .cloned()
                        .expect("optional schema object");
                    if let Some(description) = description {
                        object.insert("description".to_owned(), description);
                    }
                }
            }
            for child in object.values_mut() {
                strip_optional_null(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_optional_null(child);
            }
        }
        _ => {}
    }
}

fn harden_common(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.get("type") == Some(&Value::String("string".to_owned())) {
                object
                    .entry("maxLength")
                    .or_insert(Value::Number(262_144_u64.into()));
            }
            if object.get("type") == Some(&Value::String("array".to_owned())) {
                object
                    .entry("maxItems")
                    .or_insert(Value::Number(16_384_u64.into()));
            }
            for child in object.values_mut() {
                harden_common(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                harden_common(child);
            }
        }
        _ => {}
    }
}

fn set_string_contract(schema: &mut Value, pattern: &str, minimum: usize, maximum: usize) {
    let object = schema.as_object_mut().expect("string schema object");
    object.insert("type".to_owned(), Value::String("string".to_owned()));
    object.insert("pattern".to_owned(), Value::String(pattern.to_owned()));
    object.insert(
        "minLength".to_owned(),
        Value::Number((minimum as u64).into()),
    );
    object.insert(
        "maxLength".to_owned(),
        Value::Number((maximum as u64).into()),
    );
}

fn id_prefix(definition: &str) -> Option<&'static str> {
    match definition {
        "TenantId" => Some("tenant_"),
        "UserId" => Some("user_"),
        "WorkspaceId" => Some("workspace_"),
        "RunId" => Some("run_"),
        "AgentId" => Some("agent_"),
        "StreamId" => Some("stream_"),
        "EventId" => Some("event_"),
        "RequirementId" => Some("req_"),
        "RevisionId" => Some("rev_"),
        _ => None,
    }
}
