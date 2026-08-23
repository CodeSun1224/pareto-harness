use pareto_protocol::*;
use serde_json::json;
use std::{any::Any, sync::Arc};

fn digest(hex: char) -> Digest {
    Digest::parse(format!("sha256:{}", hex.to_string().repeat(64))).unwrap()
}
fn schema(name: &str, hex: char) -> SchemaRef {
    SchemaRef {
        r#type: name.to_owned(),
        major: 1,
        minor: 0,
        schema_digest: digest(hex),
    }
}
fn scope() -> IsolationScope {
    IsolationScope {
        tenant_id: TenantId::parse("tenant_local").unwrap(),
        user_id: Some(UserId::parse("user_alice").unwrap()),
        workspace_id: WorkspaceId::parse("workspace_repo").unwrap(),
        run_id: RunId::parse("run_one").unwrap(),
        agent_id: AgentId::parse("agent_primary").unwrap(),
    }
}

struct AdmissionPolicy(bool);
impl SchemaAdmissionAuthorizer for AdmissionPolicy {
    fn authorize(
        &self,
        parent: Option<&SchemaSetRef>,
        _candidate: &SchemaSetRef,
    ) -> Result<(), ValidationError> {
        if self.0 && parent.is_some() {
            Ok(())
        } else {
            Err(ValidationError {
                code: ErrorCode::InvariantViolation,
                path: String::new(),
                contract: "schema_admission_policy".to_owned(),
                detail: "transition denied".to_owned(),
            })
        }
    }
}

struct IntegrationDecoder(SchemaRef);
impl EventVariantDecoder for IntegrationDecoder {
    fn variant_id(&self) -> &str {
        "integration-payload-v1"
    }
    fn payload_schema_ref(&self) -> &SchemaRef {
        &self.0
    }
    fn decode(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Box<dyn Any + Send + Sync>, ValidationError> {
        serde_json::from_value::<std::collections::BTreeMap<String, String>>(payload.clone())
            .map(|value| Box::new(value) as Box<dyn Any + Send + Sync>)
            .map_err(|_| ValidationError {
                code: ErrorCode::SchemaMismatch,
                path: "/payload".to_owned(),
                contract: "typed_decoder".to_owned(),
                detail: "decode failed".to_owned(),
            })
    }
}

#[test]
fn external_kernel_can_authorize_evolved_event_schema_set() {
    let initial_bundle = generate_schema_bundle().unwrap();
    let initial = SchemaSet::bootstrap_initial(
        initial_bundle.manifest,
        initial_bundle.schemas,
        &initial_bundle.reference,
    )
    .unwrap();
    let id = "urn:pareto-harness:schema:integration-payload:1.0";
    let document = json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema", "$id":id,
        "type":"object", "properties":{"message":{"type":"string"}},
        "required":["message"], "unevaluatedProperties":false
    });
    let payload_ref = SchemaRef {
        r#type: "integration-payload".to_owned(),
        major: 1,
        minor: 0,
        schema_digest: digest_schema(id, &document).unwrap(),
    };
    let mut bundle = generate_schema_bundle().unwrap();
    bundle.manifest.schemas.push(payload_ref.clone());
    bundle.manifest.schemas.sort();
    bundle.manifest.event_bindings.push(EventTypeBinding {
        event_type: "integration".to_owned(),
        major: 1,
        minor: 0,
        payload_schema_ref: payload_ref.clone(),
        variant_id: "integration-payload-v1".to_owned(),
    });
    bundle.schemas.push(SchemaDocument {
        filename: "integration-payload-v1.0.schema.json".to_owned(),
        document,
    });
    let candidate = SchemaSetRef {
        manifest_schema_ref: bundle.reference.manifest_schema_ref.clone(),
        manifest_digest: digest_json(
            "schema-set",
            &bundle.reference.manifest_schema_ref,
            &serde_json::to_value(&bundle.manifest).unwrap(),
        )
        .unwrap(),
    };
    let make_decoders =
        || vec![Arc::new(IntegrationDecoder(payload_ref.clone())) as Arc<dyn EventVariantDecoder>];
    assert!(
        SchemaSet::admit_with(
            &AdmissionPolicy(false),
            Some(&initial),
            bundle.manifest.clone(),
            bundle.schemas.clone(),
            &candidate,
            make_decoders()
        )
        .is_err()
    );
    assert!(
        SchemaSet::admit_with(
            &AdmissionPolicy(true),
            Some(&initial),
            bundle.manifest,
            bundle.schemas,
            &candidate,
            make_decoders()
        )
        .is_ok()
    );
}

#[test]
fn identifiers_and_digests_fail_closed_during_deserialization() {
    assert!(RunId::parse("workspace_wrong").is_err());
    assert!(RunId::parse("run_UPPER").is_err());
    assert!(RunId::parse("run_a_b").is_err());
    assert!(serde_json::from_str::<Digest>("\"sha256:ABC\"").is_err());
    assert!(serde_json::from_str::<RunId>("\"run_good\"").is_ok());
    assert!(
        serde_json::from_value::<SchemaRef>(json!({
            "type":"Bad_Type", "major":1, "minor":0,
            "schema_digest":digest('a')
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ArtifactManifest>(json!({
            "schema_ref":schema("artifact-manifest", 'a'), "artifact_kind":"test",
            "media_type":"application/octet-stream", "byte_length":"01",
            "raw_bytes_digest":digest('b')
        }))
        .is_err()
    );
}

#[test]
fn closed_types_reject_unknown_fields_duplicate_keys_and_floats() {
    assert!(parse_bounded::<IsolationScope>(br#"{"tenant_id":"tenant_local","workspace_id":"workspace_repo","run_id":"run_one","agent_id":"agent_primary","extra":true}"#).is_err());
    assert!(parse_bounded::<serde_json::Value>(br#"{"x":1,"x":2}"#).is_err());
    assert!(parse_bounded::<serde_json::Value>(br#"{"x":1.5}"#).is_err());
}

#[test]
fn schema_generation_is_deterministic_closed_and_versioned() {
    let first = generate_schema_set().unwrap();
    let second = generate_schema_set().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 12);
    for schema in first {
        assert_eq!(
            schema.document["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert!(
            schema.document["$id"]
                .as_str()
                .unwrap()
                .starts_with("urn:pareto-harness:schema:")
        );
        assert_eq!(schema.document["unevaluatedProperties"], false);
    }
}

#[test]
fn checked_in_schemas_equal_deterministic_generation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bundle = generate_schema_bundle().unwrap();
    let set_name = bundle.reference.manifest_digest.as_str().replace(':', "-");
    let published = root.join("schemas").join("sets").join(set_name);
    let expected: std::collections::BTreeSet<_> = bundle
        .schemas
        .iter()
        .map(|schema| schema.filename.clone())
        .chain([
            "schema-set-v1.0.manifest.json".to_owned(),
            "schema-set-v1.0.ref.json".to_owned(),
        ])
        .collect();
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir(&published)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        actual, expected,
        "schema directory contains missing or stale files"
    );
    for schema in bundle.schemas {
        let checked_in = std::fs::read_to_string(published.join(&schema.filename)).unwrap();
        assert_eq!(
            checked_in,
            format!("{}\n", canonical_json(&schema.document).unwrap())
        );
    }
    assert_eq!(
        std::fs::read_to_string(published.join("schema-set-v1.0.manifest.json")).unwrap(),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(bundle.manifest).unwrap()).unwrap()
        )
    );
    assert_eq!(
        std::fs::read_to_string(published.join("schema-set-v1.0.ref.json")).unwrap(),
        format!(
            "{}\n",
            canonical_json(&serde_json::to_value(bundle.reference).unwrap()).unwrap()
        )
    );
}

#[test]
fn schema_publisher_is_idempotent_and_rejects_existing_byte_drift() {
    let temporary =
        std::env::temp_dir().join(format!("pareto-schema-publish-{}", std::process::id()));
    if temporary.exists() {
        std::fs::remove_dir_all(&temporary).unwrap();
    }
    let executable = env!("CARGO_BIN_EXE_generate_schemas");
    assert!(
        std::process::Command::new(executable)
            .arg(&temporary)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new(executable)
            .arg(&temporary)
            .status()
            .unwrap()
            .success()
    );
    let bundle = generate_schema_bundle().unwrap();
    let published = temporary
        .join("sets")
        .join(bundle.reference.manifest_digest.as_str().replace(':', "-"));
    std::fs::write(published.join("schema-set-v1.0.ref.json"), b"corrupt\n").unwrap();
    assert!(
        !std::process::Command::new(executable)
            .arg(&temporary)
            .status()
            .unwrap()
            .success()
    );
    std::fs::remove_dir_all(temporary).unwrap();
}

#[test]
fn compatibility_proof_allows_only_optional_property_addition() {
    let old = json!({"$id":"urn:pareto-harness:schema:test:1.0","type":"object","properties":{"name":{"type":"string"}},"required":["name"],"unevaluatedProperties":false});
    let compatible = json!({"$id":"urn:pareto-harness:schema:test:1.1","type":"object","properties":{"name":{"type":"string"},"note":{"type":"string"}},"required":["name"],"unevaluatedProperties":false});
    assert!(prove_old_writer_new_reader(&old, &compatible).is_ok());

    for breaking in [
        json!({"$id":"urn:pareto-harness:schema:test:1.1","type":"object","properties":{"name":{"type":"string","maxLength":3}},"required":["name"],"unevaluatedProperties":false}),
        json!({"$id":"urn:pareto-harness:schema:test:1.1","type":"object","properties":{"name":{"type":"string"},"note":{"type":"string"}},"required":["name","note"],"unevaluatedProperties":false}),
        json!({"$id":"urn:pareto-harness:schema:test:1.1","oneOf":[{"type":"string"},{"type":"number"}]}),
        json!({"$id":"urn:pareto-harness:schema:test:2.0","type":"object","properties":{"name":{"type":"string"}},"required":["name"],"unevaluatedProperties":false}),
        json!({"$id":"urn:pareto-harness:schema:test:1.0","type":"object","properties":{"name":{"type":"string"},"note":{"type":"string"}},"required":["name"],"unevaluatedProperties":false}),
    ] {
        assert!(prove_old_writer_new_reader(&old, &breaking).is_err());
    }

    let composed_old = json!({"$id":"urn:pareto-harness:schema:test:1.0","oneOf":[{"type":"object","properties":{"kind":{"const":"x"}},"required":["kind"],"unevaluatedProperties":false},{"type":"object","properties":{"kind":{"const":"x"},"extra":{"type":"string"}},"required":["kind","extra"],"unevaluatedProperties":false}]});
    let composed_new = json!({"$id":"urn:pareto-harness:schema:test:1.1","oneOf":[{"type":"object","properties":{"kind":{"const":"x"},"extra":{"type":"string"}},"required":["kind"],"unevaluatedProperties":false},{"type":"object","properties":{"kind":{"const":"x"},"extra":{"type":"string"}},"required":["kind","extra"],"unevaluatedProperties":false}]});
    assert!(prove_old_writer_new_reader(&composed_old, &composed_new).is_err());

    let mut generated_old = generate_schema_set()
        .unwrap()
        .into_iter()
        .find(|document| document.filename.starts_with("event-envelope-"))
        .unwrap()
        .document;
    let mut generated_new = generated_old.clone();
    generated_new["$id"] = json!("urn:pareto-harness:schema:event-envelope:1.1");
    generated_new["properties"]["compatible_note"] = json!({"type":"string"});
    assert!(prove_old_writer_new_reader(&generated_old, &generated_new).is_ok());
    generated_old["$id"] = json!("urn:pareto-harness:schema:event-envelope:1.1");
    assert!(prove_old_writer_new_reader(&generated_old, &generated_new).is_err());

    let malformed = json!({"$id":"urn:pareto-harness:schema:test:01.0","type":"object"});
    assert!(prove_old_writer_new_reader(&malformed, &malformed).is_err());
}

#[test]
fn execution_modes_are_explicit_and_closed() {
    let live: ExecutionMode = serde_json::from_value(json!({"mode":"live"})).unwrap();
    assert_eq!(live, ExecutionMode::Live {});
    assert!(
        serde_json::from_value::<ExecutionMode>(json!({"mode":"live","source_run_id":"run_old"}))
            .is_err()
    );
    assert!(serde_json::from_value::<ExecutionMode>(json!({"mode":"recorded_replay"})).is_err());
    let simulated: ExecutionMode = serde_json::from_value(json!({
        "mode":"simulated","fixture_revisions":[],"simulation_origin":"standalone"
    }))
    .unwrap();
    assert!(
        simulated
            .validate(&RunId::parse("run_new").unwrap())
            .is_err()
    );
}

#[test]
fn manifest_requires_all_version_budget_limit_and_policy_pins() {
    let incomplete =
        json!({"schema_ref": schema("run-manifest", 'a'), "scope": scope(), "revisions": {}});
    assert!(serde_json::from_value::<RunManifest>(incomplete).is_err());
}

#[test]
fn artifact_and_json_digest_domains_do_not_collide() {
    let payload_schema = schema("payload", 'a');
    let artifact_schema = schema("artifact-manifest", 'b');
    let json_digest =
        digest_json("event-payload", &payload_schema, &json!({"value":"abc"})).unwrap();
    let (manifest, artifact_digest) = digest_artifact(
        artifact_schema,
        "test-output",
        "application/json",
        br#"{"value":"abc"}"#,
    )
    .unwrap();
    assert_eq!(manifest.byte_length, "15");
    assert_eq!(
        manifest.raw_bytes_digest.as_str(),
        "sha256:afef793fc69ce78450c4c66b8d52dd7c7779bfa4871c521469741f22d5dde564"
    );
    assert_eq!(
        artifact_digest.as_str(),
        "sha256:12e2962246ec15febb6adbb74b37404d5cc49a6fa0721bcc207871b85fb19b80"
    );
    assert_ne!(json_digest, artifact_digest);
    assert_ne!(
        digest_json("revision:task", &payload_schema, &json!({"value":"abc"})).unwrap(),
        json_digest
    );
}

#[test]
fn published_schemas_compile_and_match_serde_presence_rules() {
    let schemas = generate_schema_set().unwrap();
    for document in &schemas {
        jsonschema::validator_for(&document.document)
            .unwrap_or_else(|error| panic!("{} does not compile: {error}", document.filename));
    }

    let event = schemas
        .iter()
        .find(|document| document.filename.starts_with("event-envelope-"))
        .unwrap();
    let scope_schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/IsolationScope",
        "$defs": event.document["$defs"].clone()
    });
    let validator = jsonschema::validator_for(&scope_schema).unwrap();
    let valid = json!({
        "tenant_id":"tenant_local", "workspace_id":"workspace_repo",
        "run_id":"run_one", "agent_id":"agent_primary"
    });
    assert!(validator.is_valid(&valid));
    assert!(serde_json::from_value::<IsolationScope>(valid).is_ok());

    let explicit_null = json!({
        "tenant_id":"tenant_local", "user_id":null, "workspace_id":"workspace_repo",
        "run_id":"run_one", "agent_id":"agent_primary"
    });
    assert!(!validator.is_valid(&explicit_null));
    assert!(serde_json::from_value::<IsolationScope>(explicit_null).is_err());
}

#[test]
fn digest_golden_vectors_bind_schema_type_and_revision_metadata() {
    let payload_schema = schema("payload", 'a');
    let payload = json!({"alpha":"one","nested":{"count":2}});
    assert_eq!(
        digest_json("event-payload", &payload_schema, &payload)
            .unwrap()
            .as_str(),
        "sha256:21c157ffa1c9ea40dd9647ad26e47a4afa3ade83d6202604e4c6c58c262f4a0f"
    );
    let revision_view = RevisionHashView {
        revision_kind: "behavior".to_owned(),
        content: json!({"strategy":"default","retry_limit":"2"}),
    };
    assert_eq!(
        digest_revision_content(&schema("revision-hash-view", '9'), &revision_view)
            .unwrap()
            .as_str(),
        "sha256:a7d7fd40bad36075ae2a084a23ea0631a27d77c8229c05ada50326537d507b01"
    );

    let mut metadata = RevisionMetadata {
        logical_id: "behavior/default".to_owned(),
        revision_id: RevisionId::parse("rev_placeholder").unwrap(),
        revision_kind: "behavior".to_owned(),
        parent_revision: None,
        schema_ref: schema("revision-metadata", 'b'),
        content_digest: digest('c'),
        creator_actor: AgentId::parse("agent_primary").unwrap(),
        source: "checked-in".to_owned(),
        created_at: "2026-08-22T00:00:00.000Z".to_owned(),
    };
    metadata.revision_id = derive_revision_id(&metadata).unwrap();
    assert_eq!(
        derive_revision_id(&metadata).unwrap().as_str(),
        "rev_d660eb14f881d60f15ab7a6bdb8bb18f68ae67ff54ce186c5d6c0688a76e2fe2"
    );
    let mut changed = metadata.clone();
    changed.source = "generated".to_owned();
    assert_ne!(derive_revision_id(&metadata), derive_revision_id(&changed));
    let mut with_parent = metadata.clone();
    with_parent.parent_revision = Some(RevisionId::parse("rev_parent").unwrap());
    assert_ne!(
        derive_revision_id(&metadata),
        derive_revision_id(&with_parent)
    );
    let mut valid_metadata = metadata.clone();
    valid_metadata.revision_id = derive_revision_id(&valid_metadata).unwrap();
    valid_metadata.validate_identity().unwrap();

    let document = json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema",
        "$id":"urn:pareto-harness:schema:test:1.0", "type":"object"
    });
    assert!(digest_schema("urn:pareto-harness:schema:other:1.0", &document).is_err());
}

#[test]
fn replay_lineage_and_boundary_finalization_fail_closed() {
    let derived = RunId::parse("run_derived").unwrap();
    let source = RunId::parse("run_source").unwrap();
    let inventory = RevisionId::parse("rev_inventory").unwrap();
    assert!(
        ExecutionMode::RecordedReplay {
            source_run_id: source.clone(),
            boundary_inventory_revision: inventory.clone(),
        }
        .validate(&derived)
        .is_ok()
    );
    assert!(
        ExecutionMode::Reexecute {
            source_run_id: derived.clone(),
            boundary_inventory_revision: inventory,
        }
        .validate(&derived)
        .is_err()
    );
    assert!(
        ExecutionMode::Simulated {
            fixture_revisions: vec![RevisionId::parse("rev_fixture").unwrap()],
            simulation_origin: SimulationOrigin::Derived,
            source_run_id: Some(source),
        }
        .validate(&derived)
        .is_ok()
    );

    let metadata = RevisionMetadata {
        logical_id: "inventory/run_source".to_owned(),
        revision_id: RevisionId::parse("rev_inventory-metadata").unwrap(),
        revision_kind: "boundary_inventory".to_owned(),
        parent_revision: None,
        schema_ref: schema("revision-metadata", 'b'),
        content_digest: digest('c'),
        creator_actor: AgentId::parse("agent_primary").unwrap(),
        source: "finalized-event-range".to_owned(),
        created_at: "2026-08-22T00:00:00.000Z".to_owned(),
    };
    let mut empty_finalized = BoundaryInventoryRevision {
        metadata: metadata.clone(),
        hash_schema_ref: schema("boundary-inventory-hash-view", '7'),
        source_run_id: RunId::parse("run_source").unwrap(),
        final_event_sequence: "1".to_owned(),
        schema_set_ref: SchemaSetRef {
            manifest_schema_ref: schema("schema-set-manifest", 'd'),
            manifest_digest: digest('e'),
        },
        recording_policy_ref: BoundaryRecordingPolicyRef {
            revision_id: RevisionId::parse("rev_policy").unwrap(),
            digest: digest('f'),
        },
        boundaries: Vec::new(),
    };
    empty_finalized.metadata.content_digest = empty_finalized.content_digest().unwrap();
    empty_finalized.metadata.revision_id = derive_revision_id(&empty_finalized.metadata).unwrap();
    empty_finalized.validate().unwrap();
    let mut changed_inventory = empty_finalized.clone();
    changed_inventory.source_run_id = RunId::parse("run_changed").unwrap();
    assert!(changed_inventory.validate().is_err());
    let mut invalid_sequence = empty_finalized;
    invalid_sequence.final_event_sequence = "00".to_owned();
    assert!(invalid_sequence.validate().is_err());

    let mut reconciliation_metadata = metadata;
    reconciliation_metadata.revision_kind = "boundary_reconciliation".to_owned();
    let mut reconciliation = BoundaryReconciliationRevision {
        metadata: reconciliation_metadata,
        hash_schema_ref: schema("boundary-reconciliation-hash-view", '8'),
        inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
        late_result_events: vec![EventId::parse("event_late").unwrap()],
    };
    reconciliation.metadata.content_digest = reconciliation.content_digest().unwrap();
    reconciliation.metadata.revision_id = derive_revision_id(&reconciliation.metadata).unwrap();
    reconciliation.validate().unwrap();
    let mut changed_reconciliation = reconciliation;
    changed_reconciliation
        .late_result_events
        .push(EventId::parse("event_later").unwrap());
    assert!(changed_reconciliation.validate().is_err());
}

#[test]
fn limits_reject_depth_at_n_plus_one_and_accept_n() {
    assert_eq!(
        ProtocolLimitsV1::computed_digest().unwrap(),
        ProtocolLimitsV1::DIGEST
    );
    fn nested(depth: usize) -> serde_json::Value {
        let mut value = json!(true);
        for _ in 1..depth {
            value = json!([value]);
        }
        value
    }
    let at_limit = serde_json::to_vec(&nested(ProtocolLimitsV1::DEPTH)).unwrap();
    assert!(parse_bounded::<serde_json::Value>(&at_limit).is_ok());
    let over_limit = serde_json::to_vec(&nested(ProtocolLimitsV1::DEPTH + 1)).unwrap();
    assert_eq!(
        parse_bounded::<serde_json::Value>(&over_limit)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let string_at_limit = serde_json::to_vec(&"x".repeat(ProtocolLimitsV1::STRING_BYTES)).unwrap();
    assert!(parse_bounded::<String>(&string_at_limit).is_ok());
    let string_over_limit =
        serde_json::to_vec(&"x".repeat(ProtocolLimitsV1::STRING_BYTES + 1)).unwrap();
    assert_eq!(
        parse_bounded::<String>(&string_over_limit)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let minified = br#"{}"#;
    assert!(parse_bounded::<serde_json::Value>(minified).is_ok());
    let mut pretty_transport = vec![b' '; ProtocolLimitsV1::RAW_RECORD_BYTES + 1];
    pretty_transport[0] = b'{';
    let last = pretty_transport.len() - 1;
    pretty_transport[last] = b'}';
    assert_eq!(
        parse_bounded::<serde_json::Value>(&pretty_transport)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let array_at_limit = vec![json!(null); ProtocolLimitsV1::COLLECTION];
    assert!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&array_at_limit).unwrap()).is_ok()
    );
    let array_over_limit = vec![json!(null); ProtocolLimitsV1::COLLECTION + 1];
    assert_eq!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&array_over_limit).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let object_at_limit: serde_json::Map<_, _> = (0..ProtocolLimitsV1::COLLECTION)
        .map(|index| (format!("k{index}"), json!(null)))
        .collect();
    assert!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&object_at_limit).unwrap()).is_ok()
    );
    let object_over_limit: serde_json::Map<_, _> = (0..=ProtocolLimitsV1::COLLECTION)
        .map(|index| (format!("k{index}"), json!(null)))
        .collect();
    assert_eq!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&object_over_limit).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let object_name_at_limit = "x".repeat(ProtocolLimitsV1::STRING_BYTES);
    let object_at_limit = json!({object_name_at_limit: null});
    assert!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&object_at_limit).unwrap()).is_ok()
    );
    let object_name_over_limit = "x".repeat(ProtocolLimitsV1::STRING_BYTES + 1);
    let object_over_limit = json!({object_name_over_limit: null});
    assert_eq!(
        parse_bounded::<serde_json::Value>(&serde_json::to_vec(&object_over_limit).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );

    let escaped = br#""\u0078""#;
    assert_eq!(parse_bounded::<String>(escaped).unwrap(), "x");
}
