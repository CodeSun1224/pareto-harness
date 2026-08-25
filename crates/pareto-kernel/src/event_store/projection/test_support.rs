use std::{collections::BTreeMap, sync::Arc};

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, Digest, EventId, ExecutionMode, IsolationScope,
    ProtocolLimitsRef, ProtocolLimitsV1, RevisionId, RunId, RunManifest, RunState, SchemaSet,
    TaskId, TaskState, TenantId, UserId, WorkspaceId, generate_schema_bundle,
};
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
            SchemaRegistry(vec![self.set.clone()]),
            self.limits.clone(),
        )
        .unwrap()
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
