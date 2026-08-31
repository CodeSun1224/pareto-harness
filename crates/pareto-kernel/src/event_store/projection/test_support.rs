use std::{collections::BTreeMap, sync::Arc};

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, Digest, EventId, ExecutionMode, IsolationScope,
    ProtocolLimitsRef, ProtocolLimitsV1, RevisionId, RunId, RunManifest, RunState,
    SchemaAdmissionAuthorizer, SchemaDocument, SchemaRef, SchemaSet, SchemaSetManifest,
    SchemaSetRef, TaskId, TaskState, TenantId, UserId, ValidationError, WorkspaceId, digest_json,
    digest_schema, generate_schema_bundle,
};
use sqlx::Executor;
use tempfile::TempDir;

use super::{ProjectionRegistry, ProjectionTarget};
use crate::event_store::lifecycle::{
    CreateRunCommand, CreateTaskCommand, LifecycleTarget, TransitionRunCommand,
    TransitionTaskCommand, TrustedRunInputs,
};
use crate::event_store::{EventStore, SchemaRegistry};

pub(super) struct Fixture {
    pub(super) _temp: TempDir,
    pub(super) path: std::path::PathBuf,
    pub(super) set: Arc<SchemaSet>,
    pub(super) limits: ProtocolLimitsRef,
    pub(super) scope: IsolationScope,
    pub(super) manifest: RunManifest,
}

struct TestEvolutionAuthorizer;

impl SchemaAdmissionAuthorizer for TestEvolutionAuthorizer {
    fn authorize(
        &self,
        _parent: Option<&SchemaSetRef>,
        _candidate: &SchemaSetRef,
    ) -> Result<(), ValidationError> {
        Ok(())
    }
}

impl Fixture {
    pub(super) fn new(run: &str) -> Self {
        let bundle = generate_schema_bundle().unwrap();
        let set = Arc::new(
            SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference)
                .unwrap(),
        );
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };
        let scope = IsolationScope {
            tenant_id: TenantId::parse("tenant_local").unwrap(),
            user_id: Some(UserId::parse("user_alice").unwrap()),
            workspace_id: WorkspaceId::parse("workspace_repo").unwrap(),
            run_id: RunId::parse(run).unwrap(),
            agent_id: AgentId::parse("agent_owner").unwrap(),
        };
        let manifest = RunManifest {
            schema_ref: set.schema_ref("run-manifest").unwrap().clone(),
            scope: scope.clone(),
            revisions: revision_pins(),
            hook_registry_config_digest: Some(
                Digest::parse(format!("sha256:{}", "e".repeat(64))).unwrap(),
            ),
            effect_registry_config_digest: Some(
                Digest::parse(format!("sha256:{}", "d".repeat(64))).unwrap(),
            ),
            plan_revision: None,
            schema_set_ref: set.reference().clone(),
            budget_revision: RevisionId::parse("rev_budget").unwrap(),
            protocol_limits_ref: limits.clone(),
            boundary_recording_policy_ref: BoundaryRecordingPolicyRef {
                revision_id: RevisionId::parse("rev_recording-policy").unwrap(),
                digest: Digest::parse(format!("sha256:{}", "a".repeat(64))).unwrap(),
            },
            execution_mode: ExecutionMode::Live {},
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("projection.sqlite3");
        Self {
            _temp: temp,
            path,
            set,
            limits,
            scope,
            manifest,
        }
    }

    pub(super) fn trusted(&self) -> TrustedRunInputs {
        TrustedRunInputs {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
            schema_set: self.set.clone(),
            protocol_limits_ref: self.limits.clone(),
            revisions: self.manifest.revisions.clone(),
            hook_registry_config_digest: self.manifest.hook_registry_config_digest.clone(),
            effect_registry_config_digest: self.manifest.effect_registry_config_digest.clone(),
            plan_revision: self.manifest.plan_revision.clone(),
            budget_revision: self.manifest.budget_revision.clone(),
            boundary_recording_policy_ref: self.manifest.boundary_recording_policy_ref.clone(),
            execution_mode: self.manifest.execution_mode.clone(),
        }
    }

    pub(super) fn lifecycle_target(&self) -> LifecycleTarget {
        LifecycleTarget {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
        }
    }

    pub(super) fn projection_target(&self) -> ProjectionTarget {
        ProjectionTarget {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
        }
    }

    pub(super) fn source_registry(&self) -> SchemaRegistry {
        SchemaRegistry(vec![self.set.clone()])
    }

    pub(super) fn projection_registry(&self) -> ProjectionRegistry {
        ProjectionRegistry::retained(
            self.source_registry(),
            SchemaRegistry(vec![
                self.set.clone(),
                self.retained_projection_output_set(),
            ]),
            self.limits.clone(),
        )
        .unwrap()
    }

    pub(super) fn evolved_set_with_unrelated_member(&self) -> Arc<SchemaSet> {
        let mut bundle = generate_schema_bundle().unwrap();
        let document = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "urn:pareto-harness:schema:projection-test-marker:1.0",
            "type": "object",
            "additionalProperties": false
        });
        let schema_ref = SchemaRef {
            r#type: "projection-test-marker".to_owned(),
            major: 1,
            minor: 0,
            schema_digest: digest_schema(
                "urn:pareto-harness:schema:projection-test-marker:1.0",
                &document,
            )
            .unwrap(),
        };
        bundle.schemas.push(SchemaDocument {
            filename: "projection-test-marker-v1.0.schema.json".to_owned(),
            document,
        });
        bundle.manifest.schemas.push(schema_ref);
        bundle.manifest.schemas.sort();
        let manifest_schema_ref = bundle.reference.manifest_schema_ref;
        let reference = SchemaSetRef {
            manifest_digest: digest_json(
                "schema-set",
                &manifest_schema_ref,
                &serde_json::to_value(&bundle.manifest).unwrap(),
            )
            .unwrap(),
            manifest_schema_ref,
        };
        Arc::new(
            SchemaSet::admit_with(
                &TestEvolutionAuthorizer,
                Some(&self.set),
                bundle.manifest,
                bundle.schemas,
                &reference,
                Vec::new(),
            )
            .unwrap(),
        )
    }

    pub(super) fn retained_lifecycle_set(&self) -> Arc<SchemaSet> {
        const RETAINED_LIFECYCLE_SET: &str =
            "sha256-dae028a86b31c5ab341240a0768e5166ac36cd4104bfa7e8c759230add368a71";
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/sets")
            .join(RETAINED_LIFECYCLE_SET);
        let manifest: SchemaSetManifest = serde_json::from_slice(
            &std::fs::read(directory.join("schema-set-v1.0.manifest.json")).unwrap(),
        )
        .unwrap();
        let reference: SchemaSetRef = serde_json::from_slice(
            &std::fs::read(directory.join("schema-set-v1.0.ref.json")).unwrap(),
        )
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
        Arc::new(
            SchemaSet::admit_with(
                &TestEvolutionAuthorizer,
                None,
                manifest,
                documents,
                &reference,
                Vec::new(),
            )
            .unwrap(),
        )
    }

    pub(super) fn retained_projection_output_set(&self) -> Arc<SchemaSet> {
        const RETAINED_OUTPUT_SET: &str =
            "sha256-4ce3872926ce61209fdc5ed48deceeec9703ccfe94ea83be485eb8ef7512ff97";
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/sets")
            .join(RETAINED_OUTPUT_SET);
        let manifest: SchemaSetManifest = serde_json::from_slice(
            &std::fs::read(directory.join("schema-set-v1.0.manifest.json")).unwrap(),
        )
        .unwrap();
        let reference: SchemaSetRef = serde_json::from_slice(
            &std::fs::read(directory.join("schema-set-v1.0.ref.json")).unwrap(),
        )
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
        Arc::new(
            SchemaSet::admit_with(
                &TestEvolutionAuthorizer,
                None,
                manifest,
                documents,
                &reference,
                Vec::new(),
            )
            .unwrap(),
        )
    }

    pub(super) fn create_run(&self, event: &str) -> CreateRunCommand {
        CreateRunCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-25T01:00:00.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            manifest: self.manifest.clone(),
        }
    }

    pub(super) fn create_task(
        &self,
        event: &str,
        expected_sequence: i64,
        task: &str,
    ) -> CreateTaskCommand {
        CreateTaskCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-25T01:00:01.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            task_id: TaskId::parse(task).unwrap(),
            parent_task_id: None,
        }
    }

    pub(super) fn transition_task(
        &self,
        event: &str,
        expected_sequence: i64,
        task: &str,
        from: TaskState,
        to: TaskState,
    ) -> TransitionTaskCommand {
        TransitionTaskCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-25T01:00:02.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            task_id: TaskId::parse(task).unwrap(),
            expected_state: from,
            target_state: to,
            reason_code: "projection-test".to_owned(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn transition_run(
        &self,
        event: &str,
        expected_sequence: i64,
        from: RunState,
        to: RunState,
    ) -> TransitionRunCommand {
        TransitionRunCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-25T01:00:03.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            expected_state: from,
            target_state: to,
            reason_code: "projection-test".to_owned(),
        }
    }

    pub(super) async fn open_created(&self) -> EventStore {
        let store = EventStore::open(&self.path).await.unwrap();
        store
            .create_run(
                &self.trusted(),
                &self.create_run("event_projection-created"),
            )
            .await
            .unwrap();
        store
    }
}

fn revision_pins() -> BTreeMap<String, RevisionId> {
    [
        "task",
        "behavior",
        "workspace",
        "environment",
        "context_graph",
        "model_snapshot",
        "tool_set",
        "kernel",
        "hook_registry",
        "effect_registry",
    ]
    .into_iter()
    .map(|role| {
        (
            role.to_owned(),
            RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap(),
        )
    })
    .collect()
}

pub(super) async fn mutate_event_rows(store: &EventStore, statement: &str) {
    mutate_immutable_table(
        store,
        "DROP TRIGGER events_no_update",
        statement,
        super::super::UPDATE_TRIGGER,
    )
    .await;
}

pub(super) async fn mutate_snapshot_rows(store: &EventStore, statement: &str) {
    mutate_immutable_table(
        store,
        "DROP TRIGGER projection_snapshots_no_update",
        statement,
        super::super::SNAPSHOT_UPDATE_TRIGGER,
    )
    .await;
}

async fn mutate_immutable_table(
    store: &EventStore,
    drop_trigger: &str,
    statement: &str,
    restore_trigger: &str,
) {
    let mut connection = store.pool.acquire().await.unwrap();
    connection.execute("BEGIN EXCLUSIVE").await.unwrap();
    connection.execute(drop_trigger).await.unwrap();
    connection.execute(statement).await.unwrap();
    connection.execute(restore_trigger).await.unwrap();
    connection.execute("COMMIT").await.unwrap();
}
