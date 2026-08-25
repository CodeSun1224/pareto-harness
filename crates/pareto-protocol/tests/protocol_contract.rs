use pareto_protocol::*;
use serde_json::json;
use std::{any::Any, path::Path, sync::Arc};

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
    bundle.manifest.event_bindings.sort();
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
    assert_eq!(first.len(), 26);
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
fn projection_snapshot_contract() {
    let bundle = generate_schema_bundle().unwrap();
    for required in [
        "event-cursor",
        "projection-history-seed",
        "projection-history-step",
        "projection-reducer-descriptor",
        "projection-reducer-ref",
        "run-task-projection-hash-view",
        "run-task-projection",
        "run-task-projection-snapshot-hash-view",
        "run-task-projection-snapshot",
        "source-reducer-key",
    ] {
        assert!(
            bundle
                .manifest
                .schemas
                .iter()
                .any(|schema| schema.r#type == required),
            "missing {required}"
        );
    }
    let snapshot = bundle
        .schemas
        .iter()
        .find(|schema| schema.filename == "run-task-projection-snapshot-v1.0.schema.json")
        .unwrap();
    assert_eq!(snapshot.document["unevaluatedProperties"], false);
    assert_eq!(bundle.manifest.event_bindings.len(), 4);
}

#[test]
fn projection_digest_golden() {
    let seed_schema = schema("projection-history-seed", '1');
    let step_schema = schema("projection-history-step", '2');
    let seed_value = json!({"algorithm":"run-task-history-chain-v1"});
    let seed = digest_json("projection-history-chain-seed", &seed_schema, &seed_value).unwrap();
    let one_value = json!({
        "algorithm":"run-task-history-chain-v1",
        "previous_digest":seed,
        "sequence":"1",
        "envelope":{"event_id":"event_one","sequence":"1"},
        "source_schema_set_ref":{"manifest_schema_ref":schema("schema-set-manifest", '3'),"manifest_digest":digest('4')},
        "source_protocol_limits_ref":{"profile":"protocol-limits-v1","digest":digest('5')}
    });
    let one = digest_json("projection-history-chain-step", &step_schema, &one_value).unwrap();
    let two_value = json!({
        "algorithm":"run-task-history-chain-v1",
        "previous_digest":one,
        "sequence":"2",
        "envelope":{"event_id":"event_two","sequence":"2"},
        "source_schema_set_ref":{"manifest_schema_ref":schema("schema-set-manifest", '3'),"manifest_digest":digest('4')},
        "source_protocol_limits_ref":{"profile":"protocol-limits-v1","digest":digest('5')}
    });
    let two_from_prefix =
        digest_json("projection-history-chain-step", &step_schema, &two_value).unwrap();
    let two_from_full =
        digest_json("projection-history-chain-step", &step_schema, &two_value).unwrap();
    assert_eq!(two_from_prefix, two_from_full);
    assert_eq!(
        seed.as_str(),
        "sha256:6aec642c6ef3ff04139f58e33029f332c26d41bf190cde93c3a2f24e28025366"
    );
    assert_eq!(
        one.as_str(),
        "sha256:c0191409887d68f019f8b7d9945c7fd6a11ac3365d0278046106ed648cf72672"
    );
    assert_eq!(
        two_from_full.as_str(),
        "sha256:fdea26aac4058215776669944abf577138b2c88255b21aa4c361fc31ed50d783"
    );
}

#[test]
fn lifecycle_manifest_contract() {
    let bundle = generate_schema_bundle().unwrap();
    let expected_bindings = [
        ("run-created", "run-created-payload", "run-created-v1"),
        (
            "run-state-transitioned",
            "run-state-transitioned-payload",
            "run-state-transitioned-v1",
        ),
        ("task-created", "task-created-payload", "task-created-v1"),
        (
            "task-state-transitioned",
            "task-state-transitioned-payload",
            "task-state-transitioned-v1",
        ),
    ];
    assert_eq!(
        bundle.manifest.event_bindings.len(),
        expected_bindings.len()
    );
    for (binding, expected) in bundle.manifest.event_bindings.iter().zip(expected_bindings) {
        assert_eq!(
            (
                binding.event_type.as_str(),
                binding.payload_schema_ref.r#type.as_str(),
                binding.variant_id.as_str(),
            ),
            expected
        );
        assert_eq!((binding.major, binding.minor), (1, 0));
    }

    let member = |name: &str| {
        bundle
            .manifest
            .schemas
            .iter()
            .find(|item| item.r#type == name)
            .unwrap()
            .clone()
    };
    let revisions = [
        "task",
        "behavior",
        "workspace",
        "environment",
        "context_graph",
        "model_snapshot",
        "tool_set",
        "kernel",
    ]
    .into_iter()
    .map(|role| {
        (
            role.to_owned(),
            RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap(),
        )
    })
    .collect();
    let manifest = RunManifest {
        schema_ref: member("run-manifest"),
        scope: scope(),
        revisions,
        plan_revision: None,
        schema_set_ref: bundle.reference.clone(),
        budget_revision: RevisionId::parse("rev_budget").unwrap(),
        protocol_limits_ref: ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        },
        boundary_recording_policy_ref: BoundaryRecordingPolicyRef {
            revision_id: RevisionId::parse("rev_policy").unwrap(),
            digest: digest('f'),
        },
        execution_mode: ExecutionMode::Live {},
    };
    let payload = RunCreatedPayload {
        manifest: manifest.clone(),
    };
    let payload_value = serde_json::to_value(&payload).unwrap();
    let payload_schema = member("run-created-payload");
    let event = EventEnvelope {
        schema_ref: bundle.manifest.event_envelope_schema_ref.clone(),
        scope: scope(),
        event_id: EventId::parse("event_run-created").unwrap(),
        stream_id: StreamId::parse("stream_lifecycle-one").unwrap(),
        run_id: RunId::parse("run_one").unwrap(),
        sequence: "1".to_owned(),
        causation_id: None,
        correlation_id: "corr-run-create".to_owned(),
        event_type: "run-created".to_owned(),
        event_major: 1,
        event_minor: 0,
        occurred_at: "2026-08-24T00:00:00.000Z".to_owned(),
        actor: AgentId::parse("agent_primary").unwrap(),
        payload_schema_ref: payload_schema.clone(),
        payload_digest: digest_json("event-payload", &payload_schema, &payload_value).unwrap(),
        payload: payload_value,
    };
    let set =
        SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference).unwrap();
    let validated = set
        .validate_event_at_boundary(
            event,
            scope(),
            AgentId::parse("agent_primary").unwrap(),
            StreamId::parse("stream_lifecycle-one").unwrap(),
            manifest.protocol_limits_ref.clone(),
        )
        .unwrap();
    assert_eq!(validated.variant_id(), "run-created-v1");
    assert_eq!(
        validated.downcast_payload::<RunCreatedPayload>().unwrap(),
        &payload
    );

    assert!(TaskId::parse("task_root").is_ok());
    assert!(TaskId::parse("run_root").is_err());
    assert!(serde_json::from_value::<RunState>(json!("created")).is_ok());
    assert!(serde_json::from_value::<RunState>(json!("ready")).is_err());
    assert!(serde_json::from_value::<TaskState>(json!("ready")).is_ok());
    assert!(
        serde_json::from_value::<TaskCreatedPayload>(json!({
            "task_id":"task_root", "initial_state":"created", "extra":true
        }))
        .is_err()
    );
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

fn verify_retained_set(directory: &Path) {
    let manifest_path = directory.join("schema-set-v1.0.manifest.json");
    let reference_path = directory.join("schema-set-v1.0.ref.json");
    let manifest: SchemaSetManifest =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let reference: SchemaSetRef =
        serde_json::from_slice(&std::fs::read(&reference_path).unwrap()).unwrap();
    assert_eq!(
        directory.file_name().unwrap().to_string_lossy(),
        reference.manifest_digest.as_str().replace(':', "-")
    );
    assert_eq!(
        digest_json(
            "schema-set",
            &reference.manifest_schema_ref,
            &serde_json::to_value(&manifest).unwrap()
        )
        .unwrap(),
        reference.manifest_digest
    );
    assert!(manifest.schemas.contains(&reference.manifest_schema_ref));

    let expected: std::collections::BTreeSet<_> = manifest
        .schemas
        .iter()
        .map(|schema| {
            format!(
                "{}-v{}.{}.schema.json",
                schema.r#type, schema.major, schema.minor
            )
        })
        .chain([
            "schema-set-v1.0.manifest.json".to_owned(),
            "schema-set-v1.0.ref.json".to_owned(),
        ])
        .collect();
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(actual, expected, "retained set has missing or extra files");

    for schema_ref in &manifest.schemas {
        let filename = format!(
            "{}-v{}.{}.schema.json",
            schema_ref.r#type, schema_ref.major, schema_ref.minor
        );
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join(filename)).unwrap()).unwrap();
        let id = document["$id"].as_str().unwrap();
        assert_eq!(
            id,
            format!(
                "urn:pareto-harness:schema:{}:{}.{}",
                schema_ref.r#type, schema_ref.major, schema_ref.minor
            )
        );
        assert_eq!(
            digest_schema(id, &document).unwrap(),
            schema_ref.schema_digest
        );
        jsonschema::validator_for(&document).unwrap();
    }
}

struct RetainedSetAdmission;
impl SchemaAdmissionAuthorizer for RetainedSetAdmission {
    fn authorize(&self, _: Option<&SchemaSetRef>, _: &SchemaSetRef) -> Result<(), ValidationError> {
        Ok(())
    }
}

fn load_retained_set(directory: &Path) -> SchemaSet {
    let manifest: SchemaSetManifest = serde_json::from_slice(
        &std::fs::read(directory.join("schema-set-v1.0.manifest.json")).unwrap(),
    )
    .unwrap();
    let reference: SchemaSetRef =
        serde_json::from_slice(&std::fs::read(directory.join("schema-set-v1.0.ref.json")).unwrap())
            .unwrap();
    let documents = manifest
        .schemas
        .iter()
        .map(|schema| {
            let filename = format!(
                "{}-v{}.{}.schema.json",
                schema.r#type, schema.major, schema.minor
            );
            SchemaDocument {
                document: serde_json::from_slice(
                    &std::fs::read(directory.join(&filename)).unwrap(),
                )
                .unwrap(),
                filename,
            }
        })
        .collect();
    SchemaSet::admit_with(
        &RetainedSetAdmission,
        None,
        manifest,
        documents,
        &reference,
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn checked_in_old_writer_manifests_use_their_exact_retained_reader() {
    let sets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/sets");
    let current_bundle = generate_schema_bundle().unwrap();
    let current = SchemaSet::bootstrap_initial(
        current_bundle.manifest,
        current_bundle.schemas,
        &current_bundle.reference,
    )
    .unwrap();
    for set_directory in [
        "sha256-68535bfc61b49a5bac4c8f9fd6c405bca32dc60b662196c6668a3de4c1badac3",
        "sha256-7adfe3b790d85e4bfb3440e739528c4fd33a47f99dabf0403888e09cc279a2e4",
    ] {
        let retained = load_retained_set(&sets.join(set_directory));
        assert_eq!(
            retained
                .reference()
                .manifest_digest
                .as_str()
                .replace(':', "-"),
            set_directory
        );
        let revisions = [
            "task",
            "behavior",
            "workspace",
            "environment",
            "context_graph",
            "model_snapshot",
            "tool_set",
            "kernel",
        ]
        .into_iter()
        .map(|role| {
            (
                role.to_owned(),
                RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap(),
            )
        })
        .collect();
        let manifest = RunManifest {
            schema_ref: retained.schema_ref("run-manifest").unwrap().clone(),
            scope: scope(),
            revisions,
            plan_revision: None,
            schema_set_ref: retained.reference().clone(),
            budget_revision: RevisionId::parse("rev_budget").unwrap(),
            protocol_limits_ref: ProtocolLimitsRef {
                profile: "protocol-limits-v1".to_owned(),
                digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
            },
            boundary_recording_policy_ref: BoundaryRecordingPolicyRef {
                revision_id: RevisionId::parse("rev_policy").unwrap(),
                digest: digest('f'),
            },
            execution_mode: ExecutionMode::Live {},
        };
        assert!(
            retained
                .validate_run_manifest(manifest.clone(), &scope())
                .is_ok()
        );
        assert!(
            current.validate_run_manifest(manifest, &scope()).is_err(),
            "the current reader must not substitute for retained {set_directory}"
        );
    }
}

#[test]
fn every_retained_schema_set_is_complete_and_content_addressed() {
    let sets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/sets");
    let mut count = 0;
    for entry in std::fs::read_dir(sets).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir()
            && entry.file_name().to_string_lossy().starts_with("sha256-")
        {
            verify_retained_set(&entry.path());
            count += 1;
        }
    }
    assert!(count >= 2, "historical published sets must be retained");
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
fn schema_publisher_handles_concurrency_and_stale_staging() {
    let temporary =
        std::env::temp_dir().join(format!("pareto-schema-concurrent-{}", std::process::id()));
    if temporary.exists() {
        std::fs::remove_dir_all(&temporary).unwrap();
    }
    std::fs::create_dir_all(temporary.join("sets/.staging-sha256-stale-dead-process")).unwrap();
    let executable = env!("CARGO_BIN_EXE_generate_schemas");
    let mut first = std::process::Command::new(executable)
        .arg(&temporary)
        .spawn()
        .unwrap();
    let mut second = std::process::Command::new(executable)
        .arg(&temporary)
        .spawn()
        .unwrap();
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    let bundle = generate_schema_bundle().unwrap();
    let published = temporary
        .join("sets")
        .join(bundle.reference.manifest_digest.as_str().replace(':', "-"));
    verify_retained_set(&published);
    assert!(
        temporary
            .join("sets/.staging-sha256-stale-dead-process")
            .is_dir()
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
fn boundary_record_admission_binds_exact_top_and_hash_schemas() {
    let bundle = generate_schema_bundle().unwrap();
    let member = |name: &str| {
        bundle
            .manifest
            .schemas
            .iter()
            .find(|item| item.r#type == name)
            .unwrap()
            .clone()
    };
    let inventory_top = member("boundary-inventory-revision");
    let inventory_hash = member("boundary-inventory-hash-view");
    let reconciliation_top = member("boundary-reconciliation-revision");
    let reconciliation_hash = member("boundary-reconciliation-hash-view");
    let revision_metadata = member("revision-metadata");
    let run_schema = member("run-manifest");
    let set_ref = bundle.reference.clone();
    let set =
        SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference).unwrap();

    let mut inventory = BoundaryInventoryRevision {
        metadata: RevisionMetadata {
            logical_id: "inventory/run_source".to_owned(),
            revision_id: RevisionId::parse("rev_placeholder").unwrap(),
            revision_kind: "boundary_inventory".to_owned(),
            parent_revision: None,
            schema_ref: inventory_top,
            content_digest: digest('0'),
            creator_actor: AgentId::parse("agent_primary").unwrap(),
            source: "finalized-event-range".to_owned(),
            created_at: "2026-08-22T00:00:00.000Z".to_owned(),
        },
        hash_schema_ref: inventory_hash.clone(),
        source_run_id: RunId::parse("run_source").unwrap(),
        final_event_sequence: "4".to_owned(),
        schema_set_ref: set_ref.clone(),
        recording_policy_ref: BoundaryRecordingPolicyRef {
            revision_id: RevisionId::parse("rev_policy").unwrap(),
            digest: digest('f'),
        },
        boundaries: vec![
            BoundaryRecord {
                boundary_kind: "tool".to_owned(),
                request_event_id: EventId::parse("event_received").unwrap(),
                outcome: BoundaryOutcome::Received {
                    receipt_digest: digest('1'),
                },
            },
            BoundaryRecord {
                boundary_kind: "provider".to_owned(),
                request_event_id: EventId::parse("event_partial-no-receipt").unwrap(),
                outcome: BoundaryOutcome::Failed {
                    reason_code: "partial_effect_no_receipt".to_owned(),
                },
            },
            BoundaryRecord {
                boundary_kind: "process".to_owned(),
                request_event_id: EventId::parse("event_cancelled").unwrap(),
                outcome: BoundaryOutcome::Cancelled,
            },
        ],
    };
    inventory.metadata.content_digest = inventory.content_digest().unwrap();
    inventory.metadata.revision_id = derive_revision_id(&inventory.metadata).unwrap();
    let mut source_scope = scope();
    source_scope.run_id = RunId::parse("run_source").unwrap();
    let revisions = [
        "task",
        "behavior",
        "workspace",
        "environment",
        "context_graph",
        "model_snapshot",
        "tool_set",
        "kernel",
    ]
    .into_iter()
    .map(|role| {
        (
            role.to_owned(),
            RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap(),
        )
    })
    .collect();
    let source_manifest = RunManifest {
        schema_ref: run_schema,
        scope: source_scope.clone(),
        revisions,
        plan_revision: None,
        schema_set_ref: set_ref.clone(),
        budget_revision: RevisionId::parse("rev_budget").unwrap(),
        protocol_limits_ref: ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        },
        boundary_recording_policy_ref: inventory.recording_policy_ref.clone(),
        execution_mode: ExecutionMode::Live {},
    };
    let validated_inventory = set
        .validate_boundary_inventory(inventory.clone(), source_manifest.clone(), &source_scope)
        .unwrap();
    assert!(
        ExecutionMode::RecordedReplay {
            source_run_id: inventory.source_run_id.clone(),
            boundary_inventory_revision: inventory.metadata.revision_id.clone(),
        }
        .validate_inventory(&validated_inventory)
        .is_ok()
    );
    assert!(
        ExecutionMode::RecordedReplay {
            source_run_id: RunId::parse("run_other").unwrap(),
            boundary_inventory_revision: inventory.metadata.revision_id.clone(),
        }
        .validate_inventory(&validated_inventory)
        .is_err()
    );

    let mut wrong_scope = source_scope.clone();
    wrong_scope.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    assert!(
        set.validate_boundary_inventory(inventory.clone(), source_manifest.clone(), &wrong_scope)
            .is_err()
    );
    let mut wrong_policy = inventory.clone();
    wrong_policy.recording_policy_ref = BoundaryRecordingPolicyRef {
        revision_id: RevisionId::parse("rev_other-policy").unwrap(),
        digest: digest('2'),
    };
    wrong_policy.metadata.content_digest = wrong_policy.content_digest().unwrap();
    wrong_policy.metadata.revision_id = derive_revision_id(&wrong_policy.metadata).unwrap();
    assert!(
        set.validate_boundary_inventory(wrong_policy, source_manifest.clone(), &source_scope)
            .is_err()
    );

    let mut wrong_hash = inventory.clone();
    wrong_hash.hash_schema_ref = schema("boundary-inventory-hash-view", '9');
    wrong_hash.metadata.content_digest = wrong_hash.content_digest().unwrap();
    wrong_hash.metadata.revision_id = derive_revision_id(&wrong_hash.metadata).unwrap();
    assert!(
        set.validate_boundary_inventory(wrong_hash, source_manifest.clone(), &source_scope)
            .is_err()
    );
    let mut mutated = inventory.clone();
    mutated.final_event_sequence = "5".to_owned();
    assert!(
        set.validate_boundary_inventory(mutated, source_manifest.clone(), &source_scope)
            .is_err()
    );
    let mut wrong_top = inventory.clone();
    wrong_top.metadata.schema_ref = revision_metadata;
    wrong_top.metadata.revision_id = derive_revision_id(&wrong_top.metadata).unwrap();
    assert!(
        set.validate_boundary_inventory(wrong_top, source_manifest, &source_scope)
            .is_err()
    );

    let mut reconciliation = BoundaryReconciliationRevision {
        metadata: RevisionMetadata {
            logical_id: "reconciliation/run_source".to_owned(),
            revision_id: RevisionId::parse("rev_placeholder").unwrap(),
            revision_kind: "boundary_reconciliation".to_owned(),
            parent_revision: None,
            schema_ref: reconciliation_top,
            content_digest: digest('0'),
            creator_actor: AgentId::parse("agent_primary").unwrap(),
            source: "late-result-audit".to_owned(),
            created_at: "2026-08-22T00:00:00.000Z".to_owned(),
        },
        hash_schema_ref: reconciliation_hash,
        inventory_revision: inventory.metadata.revision_id.clone(),
        late_result_events: vec![EventId::parse("event_late").unwrap()],
    };
    reconciliation.metadata.content_digest = reconciliation.content_digest().unwrap();
    reconciliation.metadata.revision_id = derive_revision_id(&reconciliation.metadata).unwrap();
    assert!(
        set.validate_boundary_reconciliation(reconciliation.clone(), &validated_inventory)
            .is_ok()
    );
    let mut wrong_inventory = reconciliation.clone();
    wrong_inventory.inventory_revision = RevisionId::parse("rev_other-inventory").unwrap();
    wrong_inventory.metadata.content_digest = wrong_inventory.content_digest().unwrap();
    wrong_inventory.metadata.revision_id = derive_revision_id(&wrong_inventory.metadata).unwrap();
    assert!(
        set.validate_boundary_reconciliation(wrong_inventory, &validated_inventory)
            .is_err()
    );
    reconciliation.hash_schema_ref = schema("boundary-reconciliation-hash-view", '8');
    reconciliation.metadata.content_digest = reconciliation.content_digest().unwrap();
    reconciliation.metadata.revision_id = derive_revision_id(&reconciliation.metadata).unwrap();
    assert!(
        set.validate_boundary_reconciliation(reconciliation, &validated_inventory)
            .is_err()
    );
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
