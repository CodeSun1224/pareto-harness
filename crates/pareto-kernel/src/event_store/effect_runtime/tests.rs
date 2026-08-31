use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, CallbackId, CancellationId, CancellationTargetV1, Digest,
    EffectAttemptId, EffectExecutorDescriptorHashViewV1, EffectExecutorDescriptorV1, EffectId,
    EffectIdempotencyPolicyV1, EffectLimitsV1, EffectPairId, EffectReceiptOutcomeClassV1,
    EffectRecoveryBaseKeyV1, EffectRegistrationV1, EffectRegistryRevisionV1, EffectRequestV1,
    EffectUnknownOutcomePolicyV1, ExecutionMode, IsolationScope, OperationOutcomeV1,
    ProtocolLimitsRef, ProtocolLimitsV1, RevisionId, RevisionMetadata, RunId, RunManifest,
    RunState, SchemaSet, TaskState, TenantId, UserId, WorkspaceId, derive_revision_id,
    generate_schema_bundle,
};
use tempfile::TempDir;

use super::*;
use crate::event_store::lifecycle::{
    CreateRunCommand, LifecycleErrorKind, LifecycleTarget, TransitionRunCommand,
    TransitionTaskCommand, TrustedRunInputs,
};
use crate::event_store::runtime_control::{self as control, RuntimeClock};

fn digest(hex: char) -> Digest {
    Digest::parse(format!("sha256:{}", hex.to_string().repeat(64))).unwrap()
}

struct Fixture {
    _temp: TempDir,
    path: std::path::PathBuf,
    set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
    scope: IsolationScope,
    manifest: RunManifest,
}

impl Fixture {
    fn new(run_id: &str) -> Self {
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
            run_id: RunId::parse(run_id).unwrap(),
            agent_id: AgentId::parse("agent_owner").unwrap(),
        };
        let revisions: BTreeMap<_, _> = [
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
        .collect();
        let manifest = RunManifest {
            schema_ref: set.schema_ref("run-manifest").unwrap().clone(),
            scope: scope.clone(),
            revisions,
            hook_registry_config_digest: Some(digest('e')),
            effect_registry_config_digest: Some(digest('d')),
            plan_revision: None,
            schema_set_ref: set.reference().clone(),
            budget_revision: RevisionId::parse("rev_budget").unwrap(),
            protocol_limits_ref: limits.clone(),
            boundary_recording_policy_ref: BoundaryRecordingPolicyRef {
                revision_id: RevisionId::parse("rev_recording-policy").unwrap(),
                digest: digest('a'),
            },
            execution_mode: ExecutionMode::Live {},
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("effect-runtime.sqlite3");
        Self {
            _temp: temp,
            path,
            set,
            limits,
            scope,
            manifest,
        }
    }

    fn registry(&self) -> SchemaRegistry {
        SchemaRegistry(vec![self.set.clone()])
    }

    fn target(&self) -> EffectTarget {
        EffectTarget {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
        }
    }

    fn trusted(&self) -> TrustedRunInputs {
        TrustedRunInputs {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
            schema_set: self.set.clone(),
            protocol_limits_ref: self.limits.clone(),
            revisions: self.manifest.revisions.clone(),
            hook_registry_config_digest: self.manifest.hook_registry_config_digest.clone(),
            effect_registry_config_digest: self.manifest.effect_registry_config_digest.clone(),
            plan_revision: None,
            budget_revision: self.manifest.budget_revision.clone(),
            boundary_recording_policy_ref: self.manifest.boundary_recording_policy_ref.clone(),
            execution_mode: self.manifest.execution_mode.clone(),
        }
    }

    fn initialize_command(&self) -> InitializeEffectStream {
        InitializeEffectStream {
            event_id: EventId::parse("event_effect-stream-init").unwrap(),
            occurred_at: "2026-08-30T00:00:01.000Z".to_owned(),
            correlation_id: "corr-effect-init".to_owned(),
            effect_registry_revision: self.manifest.revisions["effect_registry"].clone(),
            effect_registry_config_digest: self
                .manifest
                .effect_registry_config_digest
                .clone()
                .unwrap(),
        }
    }

    async fn created_store(&self) -> EventStore {
        let store = EventStore::open(&self.path).await.unwrap();
        store
            .create_run(
                &self.trusted(),
                &CreateRunCommand {
                    event_id: EventId::parse("event_run-created").unwrap(),
                    occurred_at: "2026-08-30T00:00:00.000Z".to_owned(),
                    correlation_id: "corr-run".to_owned(),
                    manifest: self.manifest.clone(),
                },
            )
            .await
            .unwrap();
        store
    }
}

#[tokio::test]
async fn fold_contract() {
    let fixture = Fixture::new("run_effect-fold");
    let store = fixture.created_store().await;
    let cursor = store
        .initialize_effect_stream(
            &fixture.registry(),
            &fixture.target(),
            &fixture.initialize_command(),
        )
        .await
        .unwrap();
    let projection = store
        .effect_projection_at(&fixture.registry(), &fixture.target(), &cursor)
        .await
        .unwrap();
    assert_eq!(projection.inclusive_cursor, cursor);
    assert!(projection.effects.is_empty());
    assert_eq!(
        projection.effect_registry_revision,
        fixture.manifest.revisions["effect_registry"]
    );
    let wrong = EventCursor {
        sequence: "1".to_owned(),
        event_id: EventId::parse("event_wrong").unwrap(),
    };
    assert_eq!(
        store
            .effect_projection_at(&fixture.registry(), &fixture.target(), &wrong)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::CursorMismatch
    );
}

#[tokio::test]
async fn projection_recovery() {
    let fixture = Fixture::new("run_effect-recovery");
    let store = fixture.created_store().await;
    let cursor = store
        .initialize_effect_stream(
            &fixture.registry(),
            &fixture.target(),
            &fixture.initialize_command(),
        )
        .await
        .unwrap();
    let before = store
        .effect_projection_at(&fixture.registry(), &fixture.target(), &cursor)
        .await
        .unwrap();
    let store_id = store.store_id.clone();
    store.pool.close().await;
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let after = reopened
        .effect_projection_at(&fixture.registry(), &fixture.target(), &cursor)
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn projection_reopens_losslessly_for_unclaimed_and_partial_effects() {
    let (fixture, store, target, registry, _) =
        admission_harness("run_effect-projection-unclaimed").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let before = store
        .effect_projection_at(&fixture.registry(), &target, &admitted.cursor)
        .await
        .unwrap();
    let entry = &before.effects[0];
    assert_eq!(entry.subject_actor, fixture.scope.agent_id);
    assert_eq!(entry.task_id.as_ref(), Some(&fixture.task_id));
    assert_eq!(entry.recovery_base_key.effect_id, request.effect_id);
    assert!(entry.recovery_key.is_none());
    assert_eq!(
        entry.intent_pair.operation_id,
        request.proposal.operation_id
    );
    assert!(!entry.reserved_usage.is_empty());
    assert_eq!(
        before.source_protocol_limits_ref,
        fixture.manifest.protocol_limits_ref
    );
    let store_id = store.store_id.clone();
    store.pool.close().await;
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    assert_eq!(
        before,
        reopened
            .effect_projection_at(&fixture.registry(), &target, &admitted.cursor)
            .await
            .unwrap()
    );

    let harness = receipt_harness(
        "run_effect-projection-partial",
        EffectReceiptOutcomeClassV1::Partial,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let cursor = EventCursor {
        sequence: "4".to_owned(),
        event_id: harness.command.effect_event_id.clone(),
    };
    let before = harness
        .store
        .effect_projection_at(&harness.fixture.registry(), &harness.target, &cursor)
        .await
        .unwrap();
    assert_eq!(before.effects[0].limitations, ["fake-limitation"]);
    assert!(before.effects[0].confirmed_components_digest.is_some());
    assert!(before.effects[0].unknown_components_digest.is_some());
    assert_eq!(
        before.effects[0].accounted_usage,
        before.effects[0].reserved_usage
    );
    let store_id = harness.store.store_id.clone();
    harness.store.pool.close().await;
    let reopened = EventStore::open_pinned(&harness.fixture.path, &store_id)
        .await
        .unwrap();
    assert_eq!(
        before,
        reopened
            .effect_projection_at(&harness.fixture.registry(), &harness.target, &cursor)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn compatibility() {
    let fixture = Fixture::new("run_effect-compatibility");
    let store = fixture.created_store().await;
    let mut wrong = fixture.initialize_command();
    wrong.effect_registry_config_digest = digest('f');
    assert_eq!(
        store
            .initialize_effect_stream(&fixture.registry(), &fixture.target(), &wrong)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::ManifestInvalid
    );
    let mut other = fixture.target();
    other.scope.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    assert_eq!(
        store
            .initialize_effect_stream(&fixture.registry(), &other, &fixture.initialize_command())
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
}

async fn reserve_intent_harness(
    run: &str,
) -> (
    control::Fixture,
    EventStore,
    EffectTarget,
    EffectReserveIntentCommandV1,
) {
    let fixture = control::Fixture::new(run);
    let store = control::create_running(&fixture).await;
    let target = EffectTarget {
        scope: fixture.scope.clone(),
        actor: fixture.scope.agent_id.clone(),
    };
    let effect_cursor = EventCursor {
        sequence: "1".to_owned(),
        event_id: EventId::parse("event_effect-stream-init").unwrap(),
    };
    let proposal = fixture.proposal("effect-intent");
    let clock = control::live_clock().sample();
    let mut transaction = store.pool.begin().await.unwrap();
    let planned = super::super::runtime_control::plan_hook_reservation(
        &mut transaction,
        &fixture.registry(),
        &fixture.target(),
        &proposal,
        &clock,
    )
    .await
    .unwrap();
    transaction.rollback().await.unwrap();
    let effect_id = EffectId::parse("effect_fixture").unwrap();
    let attempt_id = EffectAttemptId::parse("effect_attempt_fixture").unwrap();
    let pair = EffectPairBindingV1 {
        pair_id: EffectPairId::parse("effect_pair_fixture").unwrap(),
        pair_kind: EffectPairKindV1::ReserveIntent,
        pair_fingerprint: digest('0'),
        control_event_id: proposal.event_id.clone(),
        effect_event_id: EventId::parse("event_effect-intended").unwrap(),
        operation_id: proposal.operation_id.clone(),
        reservation_id: proposal.reservation_id.clone(),
        effect_id: effect_id.clone(),
        attempt_id: attempt_id.clone(),
        control_prepared_digest: digest('0'),
        effect_prepared_digest: digest('0'),
    };
    let executor_revision = RevisionId::parse("rev_fake-effect-executor-v1").unwrap();
    let executor_descriptor_digest = digest('6');
    let executor_config_digest = digest('7');
    let effect_payload = EffectIntendedPayloadV1 {
        effect_id: effect_id.clone(),
        attempt_id: attempt_id.clone(),
        effect_kind: "fake-effect".to_owned(),
        subject_actor: fixture.scope.agent_id.clone(),
        task_id: Some(fixture.task_id.clone()),
        request_digest: digest('8'),
        idempotency_key_digest: digest('9'),
        effect_registry_revision: fixture.manifest.revisions["effect_registry"].clone(),
        effect_registry_config_digest: fixture
            .manifest
            .effect_registry_config_digest
            .clone()
            .unwrap(),
        effect_revision: RevisionId::parse("rev_fake-effect-v1").unwrap(),
        executor_revision: executor_revision.clone(),
        executor_descriptor_digest: executor_descriptor_digest.clone(),
        executor_config_digest: executor_config_digest.clone(),
        pair: pair.clone(),
        reserved_usage: planned.payload.trusted_reservation.clone(),
        recovery_base_key: EffectRecoveryBaseKeyV1 {
            scope: fixture.scope.clone(),
            effect_id,
            attempt_id,
            operation_id: proposal.operation_id,
            reservation_id: proposal.reservation_id,
            executor_revision,
            executor_descriptor_digest,
            executor_config_digest,
            source_schema_set_ref: fixture.set.reference().clone(),
            meter_contract_revision: planned.payload.timeout_key.meter_revision.clone(),
            recovery_contract_revision: planned.payload.timeout_key.recovery_revision.clone(),
            initial_process_epoch_digest: digest('a'),
            deadline_at: planned.payload.absolute_deadline_utc.clone(),
        },
    };
    let command = EffectReserveIntentCommandV1 {
        scope: fixture.scope.clone(),
        owner: fixture.scope.agent_id.clone(),
        control_stream_id: runtime_control_stream_id(&fixture.scope).unwrap(),
        effect_stream_id: effect_stream_id(&fixture.scope).unwrap(),
        expected_control_cursor: planned.expected_cursor,
        expected_effect_cursor: effect_cursor,
        control_sequence: 0,
        effect_sequence: 0,
        pair,
        occurred_at: proposal.occurred_at,
        correlation_id: proposal.correlation_id,
        control_payload: planned.payload,
        effect_payload,
        clock,
    };
    (fixture, store, target, command)
}

#[tokio::test]
async fn intent_before_dispatch() {
    for (run, fault) in [
        (
            "run_effect-pair-after-first",
            AtomicPairFault::AfterFirstInsert,
        ),
        (
            "run_effect-pair-before-commit",
            AtomicPairFault::BeforeCommit,
        ),
    ] {
        let (fixture, store, target, command) = reserve_intent_harness(run).await;
        assert_eq!(
            store
                .append_effect_reserve_intent_pair(
                    &fixture.registry(),
                    &target,
                    &fixture.target(),
                    command.clone(),
                    fault,
                )
                .await
                .unwrap_err()
                .kind,
            EffectErrorKind::Store
        );
        let control = store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap();
        assert!(control.operations.is_empty());
        let projection = store
            .effect_projection_at(
                &fixture.registry(),
                &target,
                &command.expected_effect_cursor,
            )
            .await
            .unwrap();
        assert!(projection.effects.is_empty());
    }

    let (fixture, store, target, command) = reserve_intent_harness("run_effect-pair-success").await;
    let first = store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            command.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(!first.already_committed);
    assert!(matches!(first.control, AppendResult::Appended { .. }));
    assert!(matches!(first.effect, AppendResult::Appended { .. }));
    let final_cursor = EventCursor {
        sequence: "2".to_owned(),
        event_id: command.pair.effect_event_id.clone(),
    };
    let projection = store
        .effect_projection_at(&fixture.registry(), &target, &final_cursor)
        .await
        .unwrap();
    assert_eq!(projection.effects.len(), 1);
    assert_eq!(
        projection.effects[0].dispatch_state,
        EffectDispatchStateV1::Intended
    );
    let retry = store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            command.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(retry.already_committed);
    let mut mutated = command;
    mutated.effect_payload.request_digest = digest('f');
    assert_eq!(
        store
            .append_effect_reserve_intent_pair(
                &fixture.registry(),
                &target,
                &fixture.target(),
                mutated,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::IdempotencyConflict
    );
}

#[tokio::test]
async fn pair_counterpart_loss_fails_effect_and_control_reads_closed() {
    let (fixture, store, target, command) =
        reserve_intent_harness("run_effect-pair-missing-control").await;
    store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            command.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER events_no_delete")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM events WHERE event_id=?")
        .bind(command.pair.control_event_id.as_str())
        .execute(&store.pool)
        .await
        .unwrap();
    let cursor = EventCursor {
        sequence: "2".to_owned(),
        event_id: command.pair.effect_event_id,
    };
    assert_eq!(
        store
            .effect_projection_at(&fixture.registry(), &target, &cursor)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::PartialPair
    );

    let (fixture, store, target, command) =
        reserve_intent_harness("run_effect-pair-missing-effect").await;
    store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            command.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER events_no_delete")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM events WHERE event_id=?")
        .bind(command.pair.effect_event_id.as_str())
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        control::RuntimeControlErrorKind::AggregateCorrupt
    );

    let (fixture, store, target, command) =
        reserve_intent_harness("run_effect-pair-resealed-effect").await;
    store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            command.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let envelope_json: String =
        sqlx::query_scalar("SELECT envelope_json FROM events WHERE event_id=?")
            .bind(command.pair.effect_event_id.as_str())
            .fetch_one(&store.pool)
            .await
            .unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&envelope_json).unwrap();
    let mut payload: EffectIntendedPayloadV1 =
        serde_json::from_value(envelope["payload"].clone()).unwrap();
    payload.pair.pair_fingerprint = digest('f');
    let resealed = lifecycle_event(
        &fixture.set,
        &fixture.manifest.protocol_limits_ref,
        &fixture.scope,
        &fixture.scope.agent_id,
        &effect_stream_id(&fixture.scope).unwrap(),
        &command.pair.effect_event_id,
        2,
        &command.occurred_at,
        &command.correlation_id,
        "effect-intended",
        &payload,
    )
    .unwrap();
    let prepared = PreparedEvent::new(
        &resealed,
        &fixture.set,
        &fixture.manifest.protocol_limits_ref,
    )
    .unwrap();
    sqlx::query("DROP TRIGGER events_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET envelope_json=?,envelope_fingerprint=? WHERE event_id=?")
        .bind(&prepared.envelope_json)
        .bind(&prepared.envelope_fingerprint)
        .bind(command.pair.effect_event_id.as_str())
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .effect_projection_at(
                &fixture.registry(),
                &target,
                &EventCursor {
                    sequence: "2".to_owned(),
                    event_id: command.pair.effect_event_id,
                },
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::AggregateCorrupt
    );
}

async fn terminal_command(
    fixture: &control::Fixture,
    store: &EventStore,
    reserve: &EffectReserveIntentCommandV1,
    lease: &OperationLease,
) -> EffectTerminalConclusionCommandV1 {
    let clock = control::live_clock().sample();
    let mut transaction = store.pool.begin().await.unwrap();
    let planned = super::super::runtime_control::plan_hook_settlement(
        &mut transaction,
        &fixture.registry(),
        &fixture.target(),
        lease,
        EventId::parse("event_effect-settled").unwrap(),
        CallbackId::parse("callback_fake-effect").unwrap(),
        "corr-effect-terminal".to_owned(),
        OperationOutcomeV1::Failed,
        "effect-not-applied".to_owned(),
        digest('b'),
        &clock,
    )
    .await
    .unwrap();
    transaction.rollback().await.unwrap();
    let pair = EffectPairBindingV1 {
        pair_id: EffectPairId::parse("effect_pair_terminal").unwrap(),
        pair_kind: EffectPairKindV1::TerminalConclusion,
        pair_fingerprint: digest('0'),
        control_event_id: EventId::parse("event_effect-settled").unwrap(),
        effect_event_id: EventId::parse("event_effect-concluded").unwrap(),
        operation_id: reserve.pair.operation_id.clone(),
        reservation_id: reserve.pair.reservation_id.clone(),
        effect_id: reserve.pair.effect_id.clone(),
        attempt_id: reserve.pair.attempt_id.clone(),
        control_prepared_digest: digest('0'),
        effect_prepared_digest: digest('0'),
    };
    EffectTerminalConclusionCommandV1 {
        scope: fixture.scope.clone(),
        owner: fixture.scope.agent_id.clone(),
        control_stream_id: runtime_control_stream_id(&fixture.scope).unwrap(),
        effect_stream_id: effect_stream_id(&fixture.scope).unwrap(),
        expected_control_cursor: planned.expected_cursor,
        expected_effect_cursor: EventCursor {
            sequence: "2".to_owned(),
            event_id: reserve.pair.effect_event_id.clone(),
        },
        control_sequence: 0,
        effect_sequence: 0,
        pair: pair.clone(),
        occurred_at: clock.canonical_utc,
        correlation_id: "corr-effect-terminal".to_owned(),
        effect_payload: EffectAttemptConcludedPayloadV1 {
            effect_id: pair.effect_id.clone(),
            attempt_id: pair.attempt_id.clone(),
            external_conclusion: EffectExternalConclusionV1::NotApplied,
            reason_code: "effect-not-applied".to_owned(),
            accounted_usage: planned.payload.accounted_usage.clone(),
            pair,
        },
        control_payload: planned.payload,
    }
}

#[tokio::test]
async fn atomic_settlement() {
    let (fixture, store, target, reserve) =
        reserve_intent_harness("run_effect-atomic-settlement").await;
    let reserved = store
        .append_effect_reserve_intent_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            reserve.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let terminal = terminal_command(&fixture, &store, &reserve, &reserved.lease).await;
    assert_eq!(
        store
            .append_effect_terminal_conclusion_pair(
                &fixture.registry(),
                &target,
                &fixture.target(),
                terminal.clone(),
                AtomicPairFault::BeforeCommit,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Store
    );
    let before = store
        .effect_projection_at(
            &fixture.registry(),
            &target,
            &terminal.expected_effect_cursor,
        )
        .await
        .unwrap();
    assert_eq!(
        before.effects[0].dispatch_state,
        EffectDispatchStateV1::Intended
    );
    let settled = store
        .append_effect_terminal_conclusion_pair(
            &fixture.registry(),
            &target,
            &fixture.target(),
            terminal.clone(),
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(!settled.already_committed);
    assert!(matches!(settled.control, AppendResult::Appended { .. }));
    assert!(matches!(settled.effect, AppendResult::Appended { .. }));
    let final_cursor = EventCursor {
        sequence: "3".to_owned(),
        event_id: terminal.pair.effect_event_id.clone(),
    };
    let after = store
        .effect_projection_at(&fixture.registry(), &target, &final_cursor)
        .await
        .unwrap();
    assert_eq!(
        after.effects[0].dispatch_state,
        EffectDispatchStateV1::Concluded
    );
    assert_eq!(
        after.effects[0].external_conclusion,
        EffectExternalConclusionV1::NotApplied
    );
    assert!(
        store
            .append_effect_terminal_conclusion_pair(
                &fixture.registry(),
                &target,
                &fixture.target(),
                terminal,
                AtomicPairFault::None,
            )
            .await
            .unwrap()
            .already_committed
    );
}

fn install_effect_registry(
    fixture: &mut control::Fixture,
) -> (EffectRegistryRevisionV1, EffectExecutorDescriptorV1) {
    let content = EffectExecutorDescriptorHashViewV1 {
        adapter_revision: RevisionId::parse("rev_fake-adapter-v1").unwrap(),
        producer_revision: RevisionId::parse("rev_fake-producer-v1").unwrap(),
        request_schema_ref: fixture
            .set
            .schema_ref("projection-history-seed")
            .unwrap()
            .clone(),
        receipt_schema_ref: fixture
            .set
            .schema_ref("effect-receipt-observation")
            .unwrap()
            .clone(),
        config_digest: digest('7'),
        resource_contract_revision: RevisionId::parse("rev_fake-operation-v1").unwrap(),
        meter_contract_revision: RevisionId::parse("rev_kernel-meter-v1").unwrap(),
        recovery_contract_revision: RevisionId::parse("rev_timeout-recovery").unwrap(),
        implementation_compatibility_digest: digest('4'),
    };
    let mut descriptor = EffectExecutorDescriptorV1 {
        metadata: RevisionMetadata {
            logical_id: "fake-effect-executor".to_owned(),
            revision_id: RevisionId::parse("rev_placeholder").unwrap(),
            revision_kind: "effect_executor".to_owned(),
            parent_revision: None,
            schema_ref: fixture
                .set
                .schema_ref("effect-executor-descriptor")
                .unwrap()
                .clone(),
            content_digest: digest('0'),
            creator_actor: fixture.scope.agent_id.clone(),
            source: "effect-test-fixture".to_owned(),
            created_at: "2026-08-26T00:00:00.000Z".to_owned(),
        },
        hash_schema_ref: fixture
            .set
            .schema_ref("effect-executor-descriptor-hash-view")
            .unwrap()
            .clone(),
        content,
    };
    descriptor.metadata.content_digest = descriptor.content_digest().unwrap();
    descriptor.metadata.revision_id = derive_revision_id(&descriptor.metadata).unwrap();
    assert!(descriptor.validate().is_ok());
    let registration = EffectRegistrationV1 {
        effect_kind: "fake-effect".to_owned(),
        effect_revision: RevisionId::parse("rev_fake-effect-v1").unwrap(),
        executor_revision: descriptor.metadata.revision_id.clone(),
        executor_descriptor_digest: descriptor.metadata.content_digest.clone(),
        executor_config_digest: descriptor.content.config_digest.clone(),
        adapter_revision: RevisionId::parse("rev_fake-adapter-v1").unwrap(),
        producer_revision: RevisionId::parse("rev_fake-producer-v1").unwrap(),
        operation_contract_revision: RevisionId::parse("rev_fake-operation-v1").unwrap(),
        request_schema_ref: fixture
            .set
            .schema_ref("projection-history-seed")
            .unwrap()
            .clone(),
        receipt_schema_ref: fixture
            .set
            .schema_ref("effect-receipt-observation")
            .unwrap()
            .clone(),
        idempotency_policy: EffectIdempotencyPolicyV1::Keyed,
        unknown_outcome_policy: EffectUnknownOutcomePolicyV1::ReconcileOnly,
        reconciliation_policy_revision: RevisionId::parse("rev_effect-reconcile-v1").unwrap(),
        redaction_policy_revision: RevisionId::parse("rev_effect-redaction-v1").unwrap(),
        limits: EffectLimitsV1 {
            max_request_bytes: 4096,
            max_receipt_bytes: 4096,
            max_result_summary_bytes: 1024,
            max_limitations: 16,
        },
    };
    let registrations = vec![registration];
    let config_digest = effect_registry_config_digest(&registrations).unwrap();
    let mut metadata = RevisionMetadata {
        logical_id: "effect-registry-test".to_owned(),
        revision_id: RevisionId::parse("rev_placeholder").unwrap(),
        revision_kind: "effect_registry".to_owned(),
        parent_revision: None,
        schema_ref: fixture
            .set
            .schema_ref("effect-registry-revision")
            .unwrap()
            .clone(),
        content_digest: digest('5'),
        creator_actor: fixture.scope.agent_id.clone(),
        source: "effect-test-fixture".to_owned(),
        created_at: "2026-08-26T00:00:00.000Z".to_owned(),
    };
    metadata.revision_id = derive_revision_id(&metadata).unwrap();
    fixture
        .manifest
        .revisions
        .insert("effect_registry".to_owned(), metadata.revision_id.clone());
    fixture.manifest.effect_registry_config_digest = Some(config_digest.clone());
    (
        EffectRegistryRevisionV1 {
            metadata,
            config_digest,
            registrations,
        },
        descriptor,
    )
}

fn request_command(fixture: &control::Fixture) -> RequestEffectCommandV1 {
    let proposal = fixture.proposal("effect-admission");
    let request = EffectRequestV1 {
        effect_kind: "fake-effect".to_owned(),
        subject_actor: fixture.scope.agent_id.clone(),
        task_id: Some(fixture.task_id.clone()),
        request_schema_ref: fixture
            .set
            .schema_ref("projection-history-seed")
            .unwrap()
            .clone(),
        request: serde_json::json!({"algorithm": "fake-effect-request-v1"}),
        client_idempotency_key_digest: digest('9'),
        deadline_at: proposal.absolute_deadline_utc.clone(),
        correlation_id: proposal.correlation_id.clone(),
    };
    let effect_revision = RevisionId::parse("rev_fake-effect-v1").unwrap();
    let registry_revision = fixture.manifest.revisions["effect_registry"].clone();
    let effect_id = expected_effect_id(
        &fixture.scope,
        &registry_revision,
        fixture
            .manifest
            .effect_registry_config_digest
            .as_ref()
            .unwrap(),
        &effect_revision,
        &request.effect_kind,
        &request.client_idempotency_key_digest,
    )
    .unwrap();
    let request_digest = expected_request_digest(
        &request,
        &registry_revision,
        fixture
            .manifest
            .effect_registry_config_digest
            .as_ref()
            .unwrap(),
        &effect_revision,
        &RevisionId::parse("rev_fake-operation-v1").unwrap(),
        &proposal.timeout_policy_revision,
    )
    .unwrap();
    RequestEffectCommandV1 {
        occurred_at: proposal.occurred_at.clone(),
        correlation_id: proposal.correlation_id.clone(),
        proposal,
        request,
        effect_id,
        attempt_id: EffectAttemptId::parse("effect_attempt_admission").unwrap(),
        pair_id: EffectPairId::parse("effect_pair_admission").unwrap(),
        effect_event_id: EventId::parse("event_effect-admission-intent").unwrap(),
        effect_kind: "fake-effect".to_owned(),
        request_digest,
        idempotency_key_digest: digest('9'),
        clock: control::live_clock().sample(),
    }
}

async fn admission_harness(
    run: &str,
) -> (
    control::Fixture,
    EventStore,
    EffectTarget,
    EffectRegistryRevisionV1,
    EffectExecutorDescriptorV1,
) {
    let mut fixture = control::Fixture::new(run);
    let (effect_registry, descriptor) = install_effect_registry(&mut fixture);
    let store = control::create_running(&fixture).await;
    let target = EffectTarget {
        scope: fixture.scope.clone(),
        actor: fixture.scope.agent_id.clone(),
    };
    (fixture, store, target, effect_registry, descriptor)
}

#[tokio::test]
async fn default_deny() {
    let (fixture, store, target, registry, _) = admission_harness("run_effect-default-deny").await;
    let mut command = request_command(&fixture);
    command.effect_kind = "unregistered-effect".to_owned();
    assert_eq!(
        store
            .request_effect(
                &fixture.registry(),
                &registry,
                &target,
                &fixture.target(),
                &command,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
    assert!(
        store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap()
            .operations
            .is_empty()
    );
}

#[tokio::test]
async fn idempotency() {
    let (fixture, store, target, registry, _) = admission_harness("run_effect-idempotency").await;
    let command = request_command(&fixture);
    let first = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &command,
        )
        .await
        .unwrap();
    assert!(!first.already_committed);
    assert!(first.lease.is_some());
    assert_eq!(first.cursor.sequence, "2");
    let retry = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &command,
        )
        .await
        .unwrap();
    assert!(retry.already_committed);
    assert!(retry.lease.is_none());
    let mut mutation = command;
    mutation.request.request = serde_json::json!({"algorithm": "mutated-request-v1"});
    mutation.request_digest = expected_request_digest(
        &mutation.request,
        &registry.metadata.revision_id,
        &registry.config_digest,
        &registry.registrations[0].effect_revision,
        &registry.registrations[0].operation_contract_revision,
        &mutation.proposal.timeout_policy_revision,
    )
    .unwrap();
    assert_eq!(
        store
            .request_effect(
                &fixture.registry(),
                &registry,
                &target,
                &fixture.target(),
                &mutation,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::IdempotencyConflict
    );
}

#[tokio::test]
async fn isolation() {
    let (fixture, store, target, registry, _) = admission_harness("run_effect-isolation").await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let mut scopes = Vec::new();
    let mut tenant = target.scope.clone();
    tenant.tenant_id = TenantId::parse("tenant_other").unwrap();
    scopes.push(tenant);
    let mut absent_user = target.scope.clone();
    absent_user.user_id = None;
    scopes.push(absent_user);
    let mut user = target.scope.clone();
    user.user_id = Some(UserId::parse("user_other").unwrap());
    scopes.push(user);
    let mut workspace = target.scope.clone();
    workspace.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    scopes.push(workspace);
    let mut run = target.scope.clone();
    run.run_id = RunId::parse("run_effect-isolation-other").unwrap();
    scopes.push(run);
    let mut agent = target.scope.clone();
    agent.agent_id = AgentId::parse("agent_other").unwrap();
    scopes.push(agent);
    for scope in scopes {
        let other = EffectTarget {
            actor: scope.agent_id.clone(),
            scope: scope.clone(),
        };
        let mut other_control = fixture.target();
        other_control.scope = scope;
        other_control.principal = other.actor.clone();
        assert_eq!(
            store
                .request_effect(
                    &fixture.registry(),
                    &registry,
                    &other,
                    &other_control,
                    &request_command(&fixture),
                )
                .await
                .unwrap_err()
                .kind,
            EffectErrorKind::Unauthorized
        );
    }
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn dispatch_lease() {
    let (fixture, store, target, registry, descriptor) =
        admission_harness("run_effect-dispatch-lease").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let claim = ClaimEffectCommandV1 {
        event_id: EventId::parse("event_effect-dispatch-claimed").unwrap(),
        effect_id: request.effect_id.clone(),
        attempt_id: request.attempt_id.clone(),
        expected_effect_cursor: admitted.cursor,
        occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
        correlation_id: "corr-effect-dispatch-claim".to_owned(),
        clock: control::FakeClock::at("2026-08-26T00:00:11.000Z", 2_000, "epoch-a").sample(),
        claim_policy_revision: RevisionId::parse("rev_effect-claim-v1").unwrap(),
    };
    let first = store
        .claim_effect_dispatch(&fixture.registry(), &descriptor, &target, &claim)
        .await
        .unwrap();
    assert!(!first.already_committed);
    assert_eq!(first.cursor.sequence, "3");
    assert_eq!(first.lease.as_ref().unwrap().effect_id, request.effect_id);
    let projection = store
        .effect_projection_at(&fixture.registry(), &target, &first.cursor)
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].dispatch_state,
        EffectDispatchStateV1::Claimed
    );
    assert!(projection.effects[0].recovery_key.is_some());
    let calls = Arc::new(AtomicUsize::new(0));
    let wrong_implementation = CountingFakeExecutor {
        mode: FakeMode::Applied,
        calls: calls.clone(),
        producer_revision: descriptor.content.producer_revision.clone(),
        adapter_revision: descriptor.content.adapter_revision.clone(),
        implementation_compatibility_digest: digest('f'),
    };
    assert_eq!(
        execute_fake_effect(
            &descriptor,
            first.lease.as_ref().unwrap(),
            &wrong_implementation,
        )
        .unwrap_err()
        .kind,
        EffectErrorKind::Unauthorized
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let retry = store
        .claim_effect_dispatch(&fixture.registry(), &descriptor, &target, &claim)
        .await
        .unwrap();
    assert!(retry.already_committed);
    assert!(retry.lease.is_none());
    let mut forged = descriptor;
    forged.content.config_digest = digest('f');
    assert_eq!(
        store
            .claim_effect_dispatch(&fixture.registry(), &forged, &target, &claim)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
}

fn claim_command(
    request: &RequestEffectCommandV1,
    cursor: EventCursor,
    suffix: &str,
    clock: ClockSample,
) -> ClaimEffectCommandV1 {
    ClaimEffectCommandV1 {
        event_id: EventId::parse(format!("event_effect-claim-{suffix}")).unwrap(),
        effect_id: request.effect_id.clone(),
        attempt_id: request.attempt_id.clone(),
        expected_effect_cursor: cursor,
        occurred_at: clock.canonical_utc.clone(),
        correlation_id: format!("corr-effect-claim-{suffix}"),
        clock,
        claim_policy_revision: RevisionId::parse("rev_effect-claim-v1").unwrap(),
    }
}

#[tokio::test]
async fn claim_revalidates_cancellation_and_deadline_under_writer_lock() {
    let (fixture, store, target, registry, descriptor) =
        admission_harness("run_effect-claim-cancelled").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    store
        .request_cancellation(
            &fixture.registry(),
            &fixture.target(),
            &control::CancellationCommand {
                event_id: EventId::parse("event_effect-claim-cancel").unwrap(),
                occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
                correlation_id: "corr-effect-claim-cancel".to_owned(),
                cancellation_id: CancellationId::parse("cancel_effect-claim").unwrap(),
                target: CancellationTargetV1::Operation {
                    operation_id: request.proposal.operation_id.clone(),
                },
                reason_code: "cancel-before-dispatch".to_owned(),
            },
        )
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let cancelled_claim = claim_command(
        &request,
        admitted.cursor.clone(),
        "cancelled",
        control::FakeClock::at("2026-08-26T00:00:12.000Z", 3_000, "epoch-a").sample(),
    );
    assert_eq!(
        store
            .claim_effect_dispatch(&fixture.registry(), &descriptor, &target, &cancelled_claim)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(before, after);

    let (fixture, store, target, registry, descriptor) =
        admission_harness("run_effect-claim-deadline").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let deadline_claim = claim_command(
        &request,
        admitted.cursor.clone(),
        "at-deadline",
        control::FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a").sample(),
    );
    assert_eq!(
        store
            .claim_effect_dispatch(&fixture.registry(), &descriptor, &target, &deadline_claim)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
    let projection = store
        .effect_projection_at(&fixture.registry(), &target, &admitted.cursor)
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].dispatch_state,
        EffectDispatchStateV1::Intended
    );
}

#[derive(Clone, Copy)]
enum FakeMode {
    Applied,
    BusinessRejected,
    FailedBeforeApply,
    ResponseLost,
    Partial,
    CrashAfterReturn,
}

struct CountingFakeExecutor {
    mode: FakeMode,
    calls: Arc<AtomicUsize>,
    producer_revision: RevisionId,
    adapter_revision: RevisionId,
    implementation_compatibility_digest: Digest,
}

impl FakeEffectExecutor for CountingFakeExecutor {
    fn implementation_compatibility_digest(&self) -> &Digest {
        &self.implementation_compatibility_digest
    }

    fn invoke(&self, lease: &EffectDispatchLease) -> FakeEffectExecution {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let observation = |outcome_class| EffectReceiptObservationV1 {
            effect_id: lease.effect_id.clone(),
            attempt_id: lease.attempt_id.clone(),
            external_key_digest: lease.external_key_digest.clone(),
            producer_revision: self.producer_revision.clone(),
            adapter_revision: self.adapter_revision.clone(),
            outcome_class,
            observed_at: "2026-08-26T00:00:12.000Z".to_owned(),
            receipt_digest: digest('c'),
            result_digest: digest('d'),
            result_summary_bytes: 32,
            observed_usage: Vec::new(),
            limitations: Vec::new(),
        };
        match self.mode {
            FakeMode::Applied => {
                FakeEffectExecution::Observation(observation(EffectReceiptOutcomeClassV1::Applied))
            }
            FakeMode::BusinessRejected => FakeEffectExecution::Observation(observation(
                EffectReceiptOutcomeClassV1::RejectedBeforeApply,
            )),
            FakeMode::FailedBeforeApply => FakeEffectExecution::FailedBeforeApply {
                reason_code: "fake-before-apply".to_owned(),
            },
            FakeMode::ResponseLost => FakeEffectExecution::ResponseLost,
            FakeMode::Partial => {
                FakeEffectExecution::Observation(observation(EffectReceiptOutcomeClassV1::Partial))
            }
            FakeMode::CrashAfterReturn => FakeEffectExecution::CrashedAfterReturn(observation(
                EffectReceiptOutcomeClassV1::Unknown,
            )),
        }
    }
}

#[tokio::test]
async fn fake_outcomes() {
    let cases = [
        ("applied", FakeMode::Applied),
        ("business-rejected", FakeMode::BusinessRejected),
        ("before-apply", FakeMode::FailedBeforeApply),
        ("response-lost", FakeMode::ResponseLost),
        ("partial", FakeMode::Partial),
        ("crash-after-return", FakeMode::CrashAfterReturn),
    ];
    for (suffix, mode) in cases {
        let run = format!("run_effect-fake-{suffix}");
        let (fixture, store, target, registry, descriptor) = admission_harness(&run).await;
        let request = request_command(&fixture);
        let admitted = store
            .request_effect(
                &fixture.registry(),
                &registry,
                &target,
                &fixture.target(),
                &request,
            )
            .await
            .unwrap();
        let operation_lease = admitted.lease.unwrap();
        let claim = ClaimEffectCommandV1 {
            event_id: EventId::parse(format!("event_effect-claim-{suffix}")).unwrap(),
            effect_id: request.effect_id.clone(),
            attempt_id: request.attempt_id.clone(),
            expected_effect_cursor: admitted.cursor,
            occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
            correlation_id: format!("corr-effect-claim-{suffix}"),
            clock: control::FakeClock::at("2026-08-26T00:00:11.000Z", 2_000, "epoch-a").sample(),
            claim_policy_revision: RevisionId::parse("rev_effect-claim-v1").unwrap(),
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let executor = CountingFakeExecutor {
            mode,
            calls: calls.clone(),
            producer_revision: descriptor.content.producer_revision.clone(),
            adapter_revision: descriptor.content.adapter_revision.clone(),
            implementation_compatibility_digest: descriptor
                .content
                .implementation_compatibility_digest
                .clone(),
        };
        let receipt_command = AdmitEffectReceiptCommandV1 {
            control_event_id: EventId::parse(format!("event_effect-settled-{suffix}")).unwrap(),
            effect_event_id: EventId::parse(format!("event_effect-terminal-{suffix}")).unwrap(),
            pair_id: EffectPairId::parse(format!("effect_pair_terminal-{suffix}")).unwrap(),
            callback_id: CallbackId::parse(format!("callback_fake-effect-{suffix}")).unwrap(),
            occurred_at: "2026-08-26T00:00:12.000Z".to_owned(),
            correlation_id: format!("corr-effect-terminal-{suffix}"),
            clock: control::FakeClock::at("2026-08-26T00:00:12.000Z", 3_000, "epoch-a").sample(),
        };
        let first = store
            .execute_effect_to_terminal(
                &fixture.registry(),
                &registry,
                &descriptor,
                &target,
                &fixture.target(),
                &operation_lease,
                &claim,
                &receipt_command,
                &executor,
            )
            .await
            .unwrap();
        match first {
            EffectOrchestrationResult::Terminal(result) => {
                assert!(!result.already_committed);
            }
            EffectOrchestrationResult::AlreadyClaimed { .. } => panic!("first claim was reused"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let retry = store
            .execute_effect_to_terminal(
                &fixture.registry(),
                &registry,
                &descriptor,
                &target,
                &fixture.target(),
                &operation_lease,
                &claim,
                &receipt_command,
                &executor,
            )
            .await
            .unwrap();
        match retry {
            EffectOrchestrationResult::AlreadyClaimed { cursor } => {
                assert_eq!(cursor.sequence, "4");
            }
            EffectOrchestrationResult::Terminal(_) => panic!("retry executed"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let projection = store
            .effect_projection_at(
                &fixture.registry(),
                &target,
                &EventCursor {
                    sequence: "4".to_owned(),
                    event_id: receipt_command.effect_event_id,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            projection.effects[0].dispatch_state,
            EffectDispatchStateV1::Concluded
        );
        assert!(
            store
                .runtime_control_projection(&fixture.registry(), &fixture.target())
                .await
                .unwrap()
                .operations[0]
                .settlement
                .is_some()
        );
    }
}

struct ReceiptHarness {
    fixture: control::Fixture,
    store: EventStore,
    target: EffectTarget,
    registry: EffectRegistryRevisionV1,
    descriptor: EffectExecutorDescriptorV1,
    operation_lease: OperationLease,
    dispatch_lease: EffectDispatchLease,
    observation: EffectReceiptObservationV1,
    command: AdmitEffectReceiptCommandV1,
}

async fn receipt_harness(run: &str, outcome_class: EffectReceiptOutcomeClassV1) -> ReceiptHarness {
    let (fixture, store, target, registry, descriptor) = admission_harness(run).await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let operation_lease = admitted.lease.unwrap();
    let claimed = store
        .claim_effect_dispatch(
            &fixture.registry(),
            &descriptor,
            &target,
            &ClaimEffectCommandV1 {
                event_id: EventId::parse("event_effect-receipt-claim").unwrap(),
                effect_id: request.effect_id.clone(),
                attempt_id: request.attempt_id.clone(),
                expected_effect_cursor: admitted.cursor,
                occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
                correlation_id: "corr-effect-receipt-claim".to_owned(),
                clock: control::FakeClock::at("2026-08-26T00:00:11.000Z", 2_000, "epoch-a")
                    .sample(),
                claim_policy_revision: RevisionId::parse("rev_effect-claim-v1").unwrap(),
            },
        )
        .await
        .unwrap();
    let observation = EffectReceiptObservationV1 {
        effect_id: request.effect_id,
        attempt_id: request.attempt_id,
        external_key_digest: claimed.lease.as_ref().unwrap().external_key_digest.clone(),
        producer_revision: descriptor.content.producer_revision.clone(),
        adapter_revision: descriptor.content.adapter_revision.clone(),
        outcome_class,
        observed_at: "2026-08-26T00:00:12.000Z".to_owned(),
        receipt_digest: digest('c'),
        result_digest: digest('d'),
        result_summary_bytes: 32,
        observed_usage: Vec::new(),
        limitations: vec!["fake-limitation".to_owned()],
    };
    ReceiptHarness {
        fixture,
        store,
        target,
        registry,
        descriptor,
        operation_lease,
        dispatch_lease: claimed.lease.unwrap(),
        observation,
        command: AdmitEffectReceiptCommandV1 {
            control_event_id: EventId::parse("event_effect-receipt-settled").unwrap(),
            effect_event_id: EventId::parse("event_effect-receipt-terminal").unwrap(),
            pair_id: EffectPairId::parse("effect_pair_receipt-terminal").unwrap(),
            callback_id: CallbackId::parse("callback_fake-effect-receipt").unwrap(),
            occurred_at: "2026-08-26T00:00:12.000Z".to_owned(),
            correlation_id: "corr-effect-receipt-terminal".to_owned(),
            clock: control::FakeClock::at("2026-08-26T00:00:12.000Z", 3_000, "epoch-a").sample(),
        },
    }
}

#[tokio::test]
async fn receipt_admission() {
    let harness = receipt_harness(
        "run_effect-receipt-admission",
        EffectReceiptOutcomeClassV1::Applied,
    )
    .await;
    let mut forged = harness.observation.clone();
    forged.producer_revision = RevisionId::parse("rev_wrong-producer").unwrap();
    let mut rejected_command = harness.command.clone();
    rejected_command.effect_event_id =
        EventId::parse("event_effect-receipt-message-rejected").unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .admit_effect_receipt(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.descriptor,
                &harness.target,
                &harness.fixture.target(),
                &harness.operation_lease,
                &harness.dispatch_lease,
                &forged,
                &rejected_command,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn authenticated_invalid_receipt_settles_unknown_and_is_audited() {
    let harness = receipt_harness(
        "run_effect-invalid-receipt",
        EffectReceiptOutcomeClassV1::Applied,
    )
    .await;
    let mut invalid = harness.observation.clone();
    invalid.result_summary_bytes = harness.registry.registrations[0]
        .limits
        .max_result_summary_bytes
        + 1;
    let input_bytes = canonical(&invalid).unwrap();
    let input_digest = digest_bytes(
        "pareto.effect-receipt-rejected-input.v1",
        input_bytes.as_bytes(),
    )
    .unwrap();
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &invalid,
            &harness.command,
        )
        .await
        .unwrap();
    let audit_id = EventId::parse(format!(
        "event_effect-rejected-{}",
        &input_digest.as_str()[7..39]
    ))
    .unwrap();
    let projection = harness
        .store
        .effect_projection_at(
            &harness.fixture.registry(),
            &harness.target,
            &EventCursor {
                sequence: "5".to_owned(),
                event_id: audit_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(projection.rejected_count, 1);
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::Unknown
    );
    assert_eq!(
        projection.effects[0].reconciliation_state,
        EffectReconciliationStateV1::Required
    );
    assert_eq!(
        projection.effects[0].accounted_usage,
        projection.effects[0].reserved_usage
    );
    assert_eq!(
        projection.effects[0].limitations,
        ["receipt-admission-rejected"]
    );
    let control = harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &harness.fixture.target())
        .await
        .unwrap();
    assert!(control.operations[0].settlement.is_some());
}

#[tokio::test]
async fn state_model() {
    let harness = receipt_harness(
        "run_effect-state-model",
        EffectReceiptOutcomeClassV1::RejectedBeforeApply,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let projection = harness
        .store
        .effect_projection_at(
            &harness.fixture.registry(),
            &harness.target,
            &EventCursor {
                sequence: "4".to_owned(),
                event_id: harness.command.effect_event_id.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].dispatch_state,
        EffectDispatchStateV1::Concluded
    );
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::NotApplied
    );
    assert_eq!(
        projection.effects[0].reconciliation_state,
        EffectReconciliationStateV1::NotRequired
    );
}

#[tokio::test]
async fn partial_success() {
    for (suffix, outcome, conclusion) in [
        (
            "partial",
            EffectReceiptOutcomeClassV1::Partial,
            EffectExternalConclusionV1::Partial,
        ),
        (
            "unknown",
            EffectReceiptOutcomeClassV1::Unknown,
            EffectExternalConclusionV1::Unknown,
        ),
    ] {
        let harness = receipt_harness(&format!("run_effect-{suffix}"), outcome).await;
        harness
            .store
            .admit_effect_receipt(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.descriptor,
                &harness.target,
                &harness.fixture.target(),
                &harness.operation_lease,
                &harness.dispatch_lease,
                &harness.observation,
                &harness.command,
            )
            .await
            .unwrap();
        let projection = harness
            .store
            .effect_projection_at(
                &harness.fixture.registry(),
                &harness.target,
                &EventCursor {
                    sequence: "4".to_owned(),
                    event_id: harness.command.effect_event_id.clone(),
                },
            )
            .await
            .unwrap();
        assert_eq!(projection.effects[0].external_conclusion, conclusion);
        assert_eq!(
            projection.effects[0].reconciliation_state,
            EffectReconciliationStateV1::Required
        );
    }
}

#[tokio::test]
async fn late_receipts() {
    let harness = receipt_harness(
        "run_effect-late-receipts",
        EffectReceiptOutcomeClassV1::Applied,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let mut late = harness.observation.clone();
    late.receipt_digest = digest('e');
    let command = ObserveLateReceiptCommandV1 {
        event_id: EventId::parse("event_effect-late-receipt").unwrap(),
        occurred_at: "2026-08-26T00:00:13.000Z".to_owned(),
        correlation_id: "corr-effect-late-receipt".to_owned(),
    };
    assert!(matches!(
        harness
            .store
            .observe_late_effect_receipt(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &late,
                &command,
            )
            .await
            .unwrap(),
        AppendResult::Appended { .. }
    ));
    assert!(matches!(
        harness
            .store
            .observe_late_effect_receipt(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &late,
                &command,
            )
            .await
            .unwrap(),
        AppendResult::AlreadyCommitted { .. }
    ));
    let projection = harness
        .store
        .effect_projection_at(
            &harness.fixture.registry(),
            &harness.target,
            &EventCursor {
                sequence: "5".to_owned(),
                event_id: command.event_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(projection.late_receipt_count, 1);
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::Applied
    );
}

fn seal_recovery(mut command: RecoverEffectCommandV1) -> RecoverEffectCommandV1 {
    command.current_process_epoch_digest = digest_bytes(
        "effect-current-process-epoch-v1",
        command.clock.process_epoch.as_bytes(),
    )
    .unwrap();
    command.command_fingerprint = recovery_command_fingerprint(&command).unwrap();
    command
}

#[tokio::test]
async fn crash_recovery() {
    let (fixture, store, target, registry, _) =
        admission_harness("run_effect-recovery-unclaimed").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let old_epoch = digest_bytes("effect-initial-process-epoch-v1", b"epoch-a").unwrap();
    let command = seal_recovery(RecoverEffectCommandV1 {
        effect_id: request.effect_id.clone(),
        attempt_id: request.attempt_id.clone(),
        cause: EffectRecoveryCauseV1::ProcessEpochLost,
        expected_effect_cursor: admitted.cursor,
        control_event_id: EventId::parse("event_effect-recovery-unclaimed-control").unwrap(),
        effect_event_id: EventId::parse("event_effect-recovery-unclaimed-effect").unwrap(),
        pair_id: EffectPairId::parse("effect_pair_recovery-unclaimed").unwrap(),
        lost_process_epoch_digest: old_epoch,
        current_process_epoch_digest: digest('f'),
        occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
        correlation_id: "corr-effect-recovery-unclaimed".to_owned(),
        clock: control::FakeClock::at("2026-08-26T00:00:20.000Z", 10_000, "epoch-b").sample(),
        command_fingerprint: digest('0'),
    });
    assert!(
        !store
            .recover_effect(&fixture.registry(), &target, &fixture.target(), &command)
            .await
            .unwrap()
            .already_committed
    );
    assert!(
        store
            .recover_effect(&fixture.registry(), &target, &fixture.target(), &command)
            .await
            .unwrap()
            .already_committed
    );
    let mut mutation = command.clone();
    mutation.clock.process_epoch = "epoch-c".to_owned();
    let mutation = seal_recovery(mutation);
    assert_eq!(
        store
            .recover_effect(&fixture.registry(), &target, &fixture.target(), &mutation)
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::IdempotencyConflict
    );
    let mut new_sample = command.clone();
    new_sample.control_event_id =
        EventId::parse("event_effect-recovery-unclaimed-control-new-sample").unwrap();
    new_sample.effect_event_id =
        EventId::parse("event_effect-recovery-unclaimed-effect-new-sample").unwrap();
    new_sample.pair_id = EffectPairId::parse("effect_pair_recovery-unclaimed-new-sample").unwrap();
    new_sample.clock.process_epoch = "epoch-c".to_owned();
    new_sample.occurred_at = "2026-08-26T00:00:21.000Z".to_owned();
    new_sample.clock =
        control::FakeClock::at("2026-08-26T00:00:21.000Z", 11_000, "epoch-c").sample();
    let new_sample = seal_recovery(new_sample);
    assert!(
        store
            .recover_effect(&fixture.registry(), &target, &fixture.target(), &new_sample,)
            .await
            .unwrap()
            .already_committed
    );
    let projection = store
        .effect_projection_at(
            &fixture.registry(),
            &target,
            &EventCursor {
                sequence: "3".to_owned(),
                event_id: command.effect_event_id.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::NotApplied
    );

    let harness = receipt_harness(
        "run_effect-recovery-claimed",
        EffectReceiptOutcomeClassV1::Unknown,
    )
    .await;
    let claimed_command = seal_recovery(RecoverEffectCommandV1 {
        effect_id: harness.observation.effect_id.clone(),
        attempt_id: harness.observation.attempt_id.clone(),
        cause: EffectRecoveryCauseV1::ProcessEpochLost,
        expected_effect_cursor: EventCursor {
            sequence: "3".to_owned(),
            event_id: harness.dispatch_lease.claim_event_id.clone(),
        },
        control_event_id: EventId::parse("event_effect-recovery-claimed-control").unwrap(),
        effect_event_id: EventId::parse("event_effect-recovery-claimed-effect").unwrap(),
        pair_id: EffectPairId::parse("effect_pair_recovery-claimed").unwrap(),
        lost_process_epoch_digest: harness.dispatch_lease.process_epoch_digest.clone(),
        current_process_epoch_digest: digest('e'),
        occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
        correlation_id: "corr-effect-recovery-claimed".to_owned(),
        clock: control::FakeClock::at("2026-08-26T00:00:20.000Z", 10_000, "epoch-b").sample(),
        command_fingerprint: digest('0'),
    });
    harness
        .store
        .recover_effect(
            &harness.fixture.registry(),
            &harness.target,
            &harness.fixture.target(),
            &claimed_command,
        )
        .await
        .unwrap();
    let projection = harness
        .store
        .effect_projection_at(
            &harness.fixture.registry(),
            &harness.target,
            &EventCursor {
                sequence: "4".to_owned(),
                event_id: claimed_command.effect_event_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::Unknown
    );
    assert_eq!(
        projection.effects[0].reconciliation_state,
        EffectReconciliationStateV1::Required
    );
}

#[tokio::test]
async fn cancellation_timeout() {
    let (fixture, store, target, registry, _) =
        admission_harness("run_effect-deadline-recovery").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    let command = seal_recovery(RecoverEffectCommandV1 {
        effect_id: request.effect_id,
        attempt_id: request.attempt_id,
        cause: EffectRecoveryCauseV1::DeadlineDue,
        expected_effect_cursor: admitted.cursor,
        control_event_id: EventId::parse("event_effect-deadline-control").unwrap(),
        effect_event_id: EventId::parse("event_effect-deadline-effect").unwrap(),
        pair_id: EffectPairId::parse("effect_pair_deadline").unwrap(),
        lost_process_epoch_digest: digest('1'),
        current_process_epoch_digest: digest('2'),
        occurred_at: "2026-08-26T00:01:00.000Z".to_owned(),
        correlation_id: "corr-effect-deadline".to_owned(),
        clock: control::FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a").sample(),
        command_fingerprint: digest('0'),
    });
    store
        .recover_effect(&fixture.registry(), &target, &fixture.target(), &command)
        .await
        .unwrap();

    let (fixture, store, target, registry, _) =
        admission_harness("run_effect-cancellation-recovery").await;
    let request = request_command(&fixture);
    let admitted = store
        .request_effect(
            &fixture.registry(),
            &registry,
            &target,
            &fixture.target(),
            &request,
        )
        .await
        .unwrap();
    store
        .request_cancellation(
            &fixture.registry(),
            &fixture.target(),
            &control::CancellationCommand {
                event_id: EventId::parse("event_effect-cancel-requested").unwrap(),
                occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
                correlation_id: "corr-effect-cancel".to_owned(),
                cancellation_id: CancellationId::parse("cancel_effect-recovery").unwrap(),
                target: CancellationTargetV1::Operation {
                    operation_id: request.proposal.operation_id.clone(),
                },
                reason_code: "user-request".to_owned(),
            },
        )
        .await
        .unwrap();
    let command = seal_recovery(RecoverEffectCommandV1 {
        effect_id: request.effect_id,
        attempt_id: request.attempt_id,
        cause: EffectRecoveryCauseV1::CancellationEffective,
        expected_effect_cursor: admitted.cursor,
        control_event_id: EventId::parse("event_effect-cancel-control").unwrap(),
        effect_event_id: EventId::parse("event_effect-cancel-effect").unwrap(),
        pair_id: EffectPairId::parse("effect_pair_cancel").unwrap(),
        lost_process_epoch_digest: digest('1'),
        current_process_epoch_digest: digest('2'),
        occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
        correlation_id: "corr-effect-cancel-terminal".to_owned(),
        clock: control::FakeClock::at("2026-08-26T00:00:20.000Z", 10_000, "epoch-a").sample(),
        command_fingerprint: digest('0'),
    });
    store
        .recover_effect(&fixture.registry(), &target, &fixture.target(), &command)
        .await
        .unwrap();
}

fn seal_reconciliation(mut command: ReconcileEffectCommandV1) -> ReconcileEffectCommandV1 {
    command.command_fingerprint = reconciliation_command_fingerprint(&command).unwrap();
    command
}

#[tokio::test]
async fn reconciliation() {
    let harness = receipt_harness(
        "run_effect-reconciliation",
        EffectReceiptOutcomeClassV1::Partial,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let command = seal_reconciliation(ReconcileEffectCommandV1 {
        effect_id: harness.observation.effect_id.clone(),
        attempt_id: harness.observation.attempt_id.clone(),
        expected_effect_cursor: EventCursor {
            sequence: "4".to_owned(),
            event_id: harness.command.effect_event_id.clone(),
        },
        observation_event_id: EventId::parse("event_effect-reconciliation-observed").unwrap(),
        reconciled_event_id: EventId::parse("event_effect-reconciled").unwrap(),
        producer_revision: harness.registry.registrations[0]
            .reconciliation_policy_revision
            .clone(),
        source_observation_event_ids: vec![harness.command.effect_event_id.clone()],
        resolution: EffectReconciliationStateV1::ResolvedPartial,
        occurred_at: "2026-08-26T00:00:30.000Z".to_owned(),
        correlation_id: "corr-effect-reconciliation".to_owned(),
        command_fingerprint: digest('0'),
    });
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    let mut wrong_source = command.clone();
    wrong_source.source_observation_event_ids =
        vec![EventId::parse("event_effect-nonexistent-observation").unwrap()];
    wrong_source.observation_event_id =
        EventId::parse("event_effect-wrong-source-observed").unwrap();
    wrong_source.reconciled_event_id =
        EventId::parse("event_effect-wrong-source-reconciled").unwrap();
    let wrong_source = seal_reconciliation(wrong_source);
    assert_eq!(
        harness
            .store
            .reconcile_effect(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &wrong_source,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        harness
            .store
            .reconcile_effect(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &command,
                AtomicPairFault::BeforeCommit,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Store
    );
    assert!(
        !harness
            .store
            .reconcile_effect(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &command,
                AtomicPairFault::None,
            )
            .await
            .unwrap()
    );
    assert!(
        harness
            .store
            .reconcile_effect(
                &harness.fixture.registry(),
                &harness.registry,
                &harness.target,
                &command,
                AtomicPairFault::None,
            )
            .await
            .unwrap()
    );
    let projection = harness
        .store
        .effect_projection_at(
            &harness.fixture.registry(),
            &harness.target,
            &EventCursor {
                sequence: "6".to_owned(),
                event_id: command.reconciled_event_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        projection.effects[0].reconciliation_state,
        EffectReconciliationStateV1::ResolvedPartial
    );
    assert_eq!(
        projection.effects[0].external_conclusion,
        EffectExternalConclusionV1::Partial
    );
}

#[tokio::test]
async fn lifecycle_success_guard() {
    let harness = receipt_harness(
        "run_effect-success-guard",
        EffectReceiptOutcomeClassV1::Partial,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let mut connection = harness.store.pool.acquire().await.unwrap();
    assert!(
        ensure_effects_complete_for_task(
            &mut connection,
            &harness.fixture.registry(),
            &harness.fixture.scope,
            &TaskId::parse("task_unrelated").unwrap(),
        )
        .await
        .is_ok()
    );
    drop(connection);
    let lifecycle_target = LifecycleTarget {
        scope: harness.fixture.scope.clone(),
        actor: harness.fixture.scope.agent_id.clone(),
    };
    let task_success = TransitionTaskCommand {
        event_id: EventId::parse("event_effect-guard-task-succeeded").unwrap(),
        occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
        correlation_id: "corr-effect-guard-task".to_owned(),
        expected_sequence: 5,
        task_id: harness.fixture.task_id.clone(),
        expected_state: TaskState::Running,
        target_state: TaskState::Succeeded,
        reason_code: "effect-work-finished".to_owned(),
    };
    assert_eq!(
        harness
            .store
            .transition_task(
                &harness.fixture.registry(),
                &lifecycle_target,
                &task_success,
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    let success = TransitionRunCommand {
        event_id: EventId::parse("event_effect-guard-run-succeeded").unwrap(),
        occurred_at: "2026-08-26T00:00:21.000Z".to_owned(),
        correlation_id: "corr-effect-guard-run".to_owned(),
        expected_sequence: 6,
        expected_state: RunState::Running,
        target_state: RunState::Succeeded,
        reason_code: "complete".to_owned(),
    };
    let reconcile = seal_reconciliation(ReconcileEffectCommandV1 {
        effect_id: harness.observation.effect_id.clone(),
        attempt_id: harness.observation.attempt_id.clone(),
        expected_effect_cursor: EventCursor {
            sequence: "4".to_owned(),
            event_id: harness.command.effect_event_id.clone(),
        },
        observation_event_id: EventId::parse("event_effect-guard-reconciliation-observed").unwrap(),
        reconciled_event_id: EventId::parse("event_effect-guard-reconciled").unwrap(),
        producer_revision: harness.registry.registrations[0]
            .reconciliation_policy_revision
            .clone(),
        source_observation_event_ids: vec![harness.command.effect_event_id.clone()],
        resolution: EffectReconciliationStateV1::ResolvedPartial,
        occurred_at: "2026-08-26T00:00:22.000Z".to_owned(),
        correlation_id: "corr-effect-guard-reconciliation".to_owned(),
        command_fingerprint: digest('0'),
    });
    harness
        .store
        .reconcile_effect(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.target,
            &reconcile,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    harness
        .store
        .transition_task(
            &harness.fixture.registry(),
            &lifecycle_target,
            &task_success,
        )
        .await
        .unwrap();
    harness
        .store
        .transition_run(&harness.fixture.registry(), &lifecycle_target, &success)
        .await
        .unwrap();
}

#[tokio::test]
async fn recorded_replay() {
    let harness = receipt_harness(
        "run_effect-recorded-replay",
        EffectReceiptOutcomeClassV1::Applied,
    )
    .await;
    harness
        .store
        .admit_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.descriptor,
            &harness.target,
            &harness.fixture.target(),
            &harness.operation_lease,
            &harness.dispatch_lease,
            &harness.observation,
            &harness.command,
        )
        .await
        .unwrap();
    let cursor = EventCursor {
        sequence: "4".to_owned(),
        event_id: harness.command.effect_event_id.clone(),
    };
    let inventory = harness
        .store
        .effect_boundary_inventory_v2(
            &harness.fixture.registry(),
            &harness.target,
            &cursor,
            "inventory_effect-recorded-replay",
            "2026-08-26T00:00:20.000Z",
        )
        .await
        .unwrap();
    let mode = ExecutionMode::RecordedReplay {
        source_run_id: harness.fixture.scope.run_id.clone(),
        boundary_inventory_revision: inventory.metadata.revision_id.clone(),
    };
    let records = harness
        .store
        .recorded_effect_replay(
            &harness.fixture.registry(),
            &harness.fixture.set,
            &harness.fixture.manifest,
            &harness.target,
            &mode,
            &inventory,
        )
        .await
        .unwrap();
    assert_eq!(records, inventory.effects);
    harness
        .store
        .observe_late_effect_receipt(
            &harness.fixture.registry(),
            &harness.registry,
            &harness.target,
            &harness.observation,
            &ObserveLateReceiptCommandV1 {
                event_id: EventId::parse("event_effect-recorded-replay-late").unwrap(),
                occurred_at: "2026-08-26T00:00:30.000Z".to_owned(),
                correlation_id: "corr-effect-recorded-replay-late".to_owned(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .recorded_effect_replay(
                &harness.fixture.registry(),
                &harness.fixture.set,
                &harness.fixture.manifest,
                &harness.target,
                &mode,
                &inventory,
            )
            .await
            .unwrap(),
        records
    );
    let reexecute = ExecutionMode::Reexecute {
        source_run_id: harness.fixture.scope.run_id.clone(),
        boundary_inventory_revision: inventory.metadata.revision_id.clone(),
    };
    assert_eq!(
        harness
            .store
            .recorded_effect_replay(
                &harness.fixture.registry(),
                &harness.fixture.set,
                &harness.fixture.manifest,
                &harness.target,
                &reexecute,
                &inventory,
            )
            .await
            .unwrap_err()
            .kind,
        EffectErrorKind::Unauthorized
    );
}
