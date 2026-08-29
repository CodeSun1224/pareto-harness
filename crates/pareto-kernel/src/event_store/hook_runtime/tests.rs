use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, CancellationId, CancellationTargetV1, Digest, EventCursor,
    ExecutionMode, GateDecisionV1, HookDecisionId, HookId, HookInvocationId, HookInvocationKeyV1,
    HookInvocationReservedPayloadV1, HookInvocationTerminalPayloadV1,
    HookInvocationTerminalStateV1, HookKindV1, HookLimitsV1, HookPairBindingV1, HookPairId,
    HookPhaseV1, HookPointV1, HookRegistrationV1, HookRegistryRevisionV1, ObserverFailurePolicyV1,
    OperationOutcomeV1, ProposalId, ProtocolLimitsRef, ProtocolLimitsV1, RevisionId,
    RevisionMetadata, RunId, RunManifest, SchemaSet, TenantId, TransformContractV1,
    UsageEvidenceClassV1, UserId, WorkspaceId, derive_revision_id, generate_schema_bundle,
};
use tempfile::TempDir;

use super::*;
use crate::event_store::lifecycle::{CreateRunCommand, TrustedRunInputs};
use crate::event_store::runtime_control::{self as control, ReserveResult};

struct Fixture {
    _temp: TempDir,
    path: std::path::PathBuf,
    set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
    scope: IsolationScope,
    manifest: RunManifest,
}

impl Fixture {
    fn new(run: &str) -> Self {
        let bundle = generate_schema_bundle().unwrap();
        let set = Arc::new(
            SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference)
                .unwrap(),
        );
        let scope = IsolationScope {
            tenant_id: TenantId::parse("tenant_local").unwrap(),
            user_id: Some(UserId::parse("user_alice").unwrap()),
            workspace_id: WorkspaceId::parse("workspace_repo").unwrap(),
            run_id: RunId::parse(run).unwrap(),
            agent_id: AgentId::parse("agent_owner").unwrap(),
        };
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };
        let manifest = RunManifest {
            schema_ref: set.schema_ref("run-manifest").unwrap().clone(),
            scope: scope.clone(),
            revisions: revision_pins(),
            hook_registry_config_digest: Some(digest('e')),
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
        let path = temp.path().join("hook-runtime.sqlite3");
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

    fn target(&self) -> HookTarget {
        HookTarget {
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
            plan_revision: None,
            budget_revision: self.manifest.budget_revision.clone(),
            boundary_recording_policy_ref: self.manifest.boundary_recording_policy_ref.clone(),
            execution_mode: self.manifest.execution_mode.clone(),
        }
    }

    fn initialization(&self) -> InitializeHookStream {
        InitializeHookStream {
            event_id: EventId::parse("event_hook-initialized").unwrap(),
            occurred_at: "2026-08-28T00:00:01.000Z".to_owned(),
            correlation_id: "corr-hook-init".to_owned(),
            hook_registry_revision: self.manifest.revisions["hook_registry"].clone(),
            hook_registry_config_digest: self.manifest.hook_registry_config_digest.clone().unwrap(),
        }
    }

    async fn open_initialized(&self) -> EventStore {
        let store = EventStore::open(&self.path).await.unwrap();
        store
            .create_run(
                &self.trusted(),
                &CreateRunCommand {
                    event_id: EventId::parse("event_run-created").unwrap(),
                    occurred_at: "2026-08-28T00:00:00.000Z".to_owned(),
                    correlation_id: "corr-run-create".to_owned(),
                    manifest: self.manifest.clone(),
                },
            )
            .await
            .unwrap();
        store
            .initialize_hook_stream(&self.registry(), &self.target(), &self.initialization())
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

fn digest(fill: char) -> Digest {
    Digest::parse(format!("sha256:{}", fill.to_string().repeat(64))).unwrap()
}

fn registration(
    fixture: &Fixture,
    kind: HookKindV1,
    id: &str,
    priority: i32,
) -> HookRegistrationV1 {
    let schema = fixture
        .set
        .schema_ref("hook-registry-revision")
        .unwrap()
        .clone();
    let output_schema = fixture
        .set
        .schema_ref(match kind {
            HookKindV1::Transform => "transform-proposal",
            HookKindV1::Gate => "gate-decision",
            HookKindV1::Observer => "observer-result",
        })
        .unwrap()
        .clone();
    HookRegistrationV1 {
        hook_id: pareto_protocol::HookId::parse(id).unwrap(),
        hook_revision: RevisionId::parse(format!("rev_{}-v1", id.replace("hook_", ""))).unwrap(),
        config_digest: digest('1'),
        kind,
        hook_points: vec![HookPointV1::BeforeProposalAdmission],
        priority,
        required: (kind == HookKindV1::Gate).then_some(true),
        observer_failure_policy: (kind == HookKindV1::Observer)
            .then_some(ObserverFailurePolicyV1::WarnAndContinue),
        transform_contract: (kind == HookKindV1::Transform).then_some(TransformContractV1 {
            allowed_fields: vec!["/content".to_owned()],
            field_schema_ref: schema.clone(),
            protected_hash_view_schema_ref: fixture
                .set
                .schema_ref("protected-proposal-hash-view")
                .unwrap()
                .clone(),
        }),
        resource_contract_revision: RevisionId::parse("rev_hook-resource-v1").unwrap(),
        input_schema_ref: schema.clone(),
        output_schema_ref: output_schema,
        limits: HookLimitsV1 {
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_depth: 16,
            max_collection_items: 128,
        },
        redaction_policy_revision: RevisionId::parse("rev_redaction-v1").unwrap(),
        handler_compatibility_digest: digest('2'),
    }
}

fn registry_fixture(fixture: &Fixture) -> (RunManifest, HookRegistryRevisionV1) {
    let registrations = vec![
        registration(fixture, HookKindV1::Transform, "hook_transform-b", -100),
        registration(fixture, HookKindV1::Transform, "hook_transform-a", 100),
        registration(fixture, HookKindV1::Gate, "hook_gate-a", -500),
        registration(fixture, HookKindV1::Observer, "hook_observer-a", -900),
    ];
    let config_digest = registry_config_digest(&registrations).unwrap();
    let mut metadata = RevisionMetadata {
        logical_id: "hook-registry-default".to_owned(),
        revision_id: RevisionId::parse("rev_placeholder").unwrap(),
        revision_kind: "hook_registry".to_owned(),
        parent_revision: None,
        schema_ref: fixture
            .set
            .schema_ref("hook-registry-revision")
            .unwrap()
            .clone(),
        content_digest: digest('3'),
        creator_actor: fixture.scope.agent_id.clone(),
        source: "test-fixture".to_owned(),
        created_at: "2026-08-28T00:00:00.000Z".to_owned(),
    };
    metadata.revision_id = derive_revision_id(&metadata).unwrap();
    let registry = HookRegistryRevisionV1 {
        metadata,
        config_digest: config_digest.clone(),
        registrations,
    };
    let mut manifest = fixture.manifest.clone();
    manifest.revisions.insert(
        "hook_registry".to_owned(),
        registry.metadata.revision_id.clone(),
    );
    manifest.hook_registry_config_digest = Some(config_digest);
    (manifest, registry)
}

pub(super) fn kind_point_table_case() {
    use HookKindV1::{Gate, Observer, Transform};
    use HookPointV1::{
        AfterAuthoritativeCommit, AfterProposalAdmission, BeforeAuthoritativeCommit,
        BeforeProposalAdmission,
    };
    for (point, kind, expected) in [
        (
            BeforeProposalAdmission,
            Transform,
            Some(HookPhaseV1::Transform),
        ),
        (BeforeProposalAdmission, Gate, Some(HookPhaseV1::Gate)),
        (
            BeforeProposalAdmission,
            Observer,
            Some(HookPhaseV1::Observer),
        ),
        (AfterProposalAdmission, Transform, None),
        (AfterProposalAdmission, Gate, None),
        (
            AfterProposalAdmission,
            Observer,
            Some(HookPhaseV1::Observer),
        ),
        (BeforeAuthoritativeCommit, Transform, None),
        (BeforeAuthoritativeCommit, Gate, Some(HookPhaseV1::Gate)),
        (
            BeforeAuthoritativeCommit,
            Observer,
            Some(HookPhaseV1::Observer),
        ),
        (AfterAuthoritativeCommit, Transform, None),
        (AfterAuthoritativeCommit, Gate, None),
        (
            AfterAuthoritativeCommit,
            Observer,
            Some(HookPhaseV1::Observer),
        ),
    ] {
        assert_eq!(phase_for(point, kind), expected);
    }
}

pub(super) fn ordering_case() {
    let fixture = Fixture::new("run_hook-ordering");
    let (manifest, registry) = registry_fixture(&fixture);
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry).unwrap();
    assert_eq!(resolved.revision, registry.metadata.revision_id);
    assert_eq!(resolved.config_digest, registry.config_digest);
    let identities: Vec<_> = resolved
        .ordered_for_point(HookPointV1::BeforeProposalAdmission)
        .iter()
        .map(|registration| (registration.kind, registration.hook_id.as_str()))
        .collect();
    assert_eq!(
        identities,
        vec![
            (HookKindV1::Transform, "hook_transform-b"),
            (HookKindV1::Transform, "hook_transform-a"),
            (HookKindV1::Gate, "hook_gate-a"),
            (HookKindV1::Observer, "hook_observer-a"),
        ]
    );
    let mut substituted = manifest;
    substituted.revisions.insert(
        "hook_registry".to_owned(),
        RevisionId::parse("rev_current").unwrap(),
    );
    assert_eq!(
        ResolvedHookRegistry::resolve(&substituted, &registry)
            .unwrap_err()
            .kind,
        HookErrorKind::ManifestInvalid
    );
}

pub(super) fn phase_order_lineage_case() {
    let fixture = Fixture::new("run_hook-lineage");
    let (manifest, registry) = registry_fixture(&fixture);
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry).unwrap();
    let initial = digest('4');
    let first_output = digest('5');
    let final_output = digest('6');
    let outputs = BTreeMap::from([
        (
            pareto_protocol::HookId::parse("hook_transform-b").unwrap(),
            first_output.clone(),
        ),
        (
            pareto_protocol::HookId::parse("hook_transform-a").unwrap(),
            final_output.clone(),
        ),
    ]);
    let lineage = planned_lineage(
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &initial,
        &outputs,
    )
    .unwrap();
    assert_eq!(lineage[0].input_digest, initial);
    assert_eq!(lineage[0].predecessor_output_digest, None);
    assert_eq!(lineage[1].input_digest, first_output.clone());
    assert_eq!(lineage[1].predecessor_output_digest, Some(first_output));
    assert_eq!(lineage[2].input_digest, final_output);
    assert_eq!(lineage[2].input_digest, lineage[3].input_digest);
    assert_eq!(lineage[2].phase, HookPhaseV1::Gate);
    assert_eq!(lineage[3].phase, HookPhaseV1::Observer);
}

#[test]
fn fake_handler_boundary_is_bounded() {
    struct Gate;
    impl FakeHookHandler for Gate {
        fn invoke(
            &self,
            lease: &HookInvocationLease,
            request: &HookRequestView,
        ) -> UntrustedHookOutput {
            assert!(lease.narrowed);
            assert_eq!(lease.input_digest, request.input_digest);
            UntrustedHookOutput::Gate(GateDecisionV1::Allow {})
        }
    }
    let fixture = Fixture::new("run_hook-handler-boundary");
    let input = digest('7');
    let output = Gate.invoke(
        &HookInvocationLease {
            invocation_id: pareto_protocol::HookInvocationId::parse("invocation_gate-a").unwrap(),
            hook_id: pareto_protocol::HookId::parse("hook_gate-a").unwrap(),
            input_digest: input.clone(),
            scope: fixture.scope,
            narrowed: true,
        },
        &HookRequestView {
            hook_point: HookPointV1::BeforeProposalAdmission,
            phase: HookPhaseV1::Gate,
            input_digest: input,
            fixed_business_decision: None,
        },
    );
    assert_eq!(output, UntrustedHookOutput::Gate(GateDecisionV1::Allow {}));
}

struct PairHarness {
    fixture: control::Fixture,
    store: EventStore,
    reserve: HookReservePairCommandV1,
    terminal: HookTerminalPairCommandV1,
}

async fn pair_harness(run: &str) -> PairHarness {
    let template = control::Fixture::new(run);
    let template_store = control::create_running(&template).await;
    let proposal = template.proposal("hook");
    let lease = match template_store
        .reserve_protected_operation(
            &template.registry(),
            &template.target(),
            &proposal,
            &control::live_clock(),
        )
        .await
        .unwrap()
    {
        ReserveResult::Reserved { lease, .. } => *lease,
        other => panic!("expected template reservation, got {other:?}"),
    };
    let reservation = template_store
        .runtime_control_projection(&template.registry(), &template.target())
        .await
        .unwrap()
        .operations
        .into_iter()
        .find(|operation| operation.operation_id == proposal.operation_id)
        .unwrap()
        .reservation;
    template_store
        .settle_operation(
            &template.registry(),
            &template.target(),
            &lease,
            &control::settlement(&template, "hook", OperationOutcomeV1::Succeeded, 1),
        )
        .await
        .unwrap();
    let settlement = template_store
        .runtime_control_projection(&template.registry(), &template.target())
        .await
        .unwrap()
        .operations
        .into_iter()
        .find(|operation| operation.operation_id == proposal.operation_id)
        .unwrap()
        .settlement
        .unwrap();

    let fixture = control::Fixture::new(run);
    let store = control::create_running(&fixture).await;
    let target = HookTarget {
        scope: fixture.scope.clone(),
        actor: fixture.scope.agent_id.clone(),
    };
    store
        .initialize_hook_stream(
            &fixture.registry(),
            &target,
            &InitializeHookStream {
                event_id: EventId::parse("event_hook-initialized").unwrap(),
                occurred_at: "2026-08-26T00:00:06.000Z".to_owned(),
                correlation_id: "corr-hook-init".to_owned(),
                hook_registry_revision: fixture.manifest.revisions["hook_registry"].clone(),
                hook_registry_config_digest: fixture
                    .manifest
                    .hook_registry_config_digest
                    .clone()
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    let invocation_id = HookInvocationId::parse("invocation_pair-hook").unwrap();
    let reserve_pair = HookPairBindingV1 {
        pair_id: HookPairId::parse("pair_reserve-hook").unwrap(),
        pair_fingerprint: digest('0'),
        control_event_id: proposal.event_id.clone(),
        hook_event_id: EventId::parse("event_hook-reserved").unwrap(),
        operation_id: proposal.operation_id.clone(),
        reservation_id: proposal.reservation_id.clone(),
        invocation_id: invocation_id.clone(),
    };
    let key = HookInvocationKeyV1 {
        scope: fixture.scope.clone(),
        task_id: Some(fixture.task_id.clone()),
        hook_point: HookPointV1::BeforeProposalAdmission,
        phase: HookPhaseV1::Gate,
        hook_id: HookId::parse("hook_gate-pair").unwrap(),
        hook_revision: RevisionId::parse("rev_gate-pair-v1").unwrap(),
        subject_proposal_id: ProposalId::parse("proposal_pair-hook").unwrap(),
        ordinal: 0,
        source_cursor: EventCursor {
            sequence: "5".to_owned(),
            event_id: EventId::parse("event_task-running").unwrap(),
        },
        input_digest: digest('4'),
        predecessor_output_digest: None,
        attempt: 1,
    };
    let trusted_reservation = reservation.trusted_reservation.clone();
    let reserve = seal_reserve_pair_command(HookReservePairCommandV1 {
        scope: fixture.scope.clone(),
        owner: fixture.scope.agent_id.clone(),
        control_stream_id: runtime_control_stream_id(&fixture.scope).unwrap(),
        hook_stream_id: hook_stream_id(&fixture.scope).unwrap(),
        expected_control_cursor: EventCursor {
            sequence: "1".to_owned(),
            event_id: EventId::parse("event_control-init").unwrap(),
        },
        expected_hook_cursor: EventCursor {
            sequence: "1".to_owned(),
            event_id: EventId::parse("event_hook-initialized").unwrap(),
        },
        pair: reserve_pair.clone(),
        occurred_at: reservation.reserved_at_utc.clone(),
        correlation_id: proposal.correlation_id.clone(),
        control_payload: reservation,
        hook_payload: HookInvocationReservedPayloadV1 {
            invocation_id: invocation_id.clone(),
            key,
            pair: reserve_pair,
            reserved_usage: trusted_reservation,
        },
        clock: control::RuntimeClock::sample(&control::live_clock()),
    })
    .unwrap();

    let terminal_pair = HookPairBindingV1 {
        pair_id: HookPairId::parse("pair_terminal-hook").unwrap(),
        pair_fingerprint: digest('0'),
        control_event_id: EventId::parse("event_settle-hook").unwrap(),
        hook_event_id: EventId::parse("event_hook-terminal").unwrap(),
        operation_id: proposal.operation_id,
        reservation_id: proposal.reservation_id,
        invocation_id: invocation_id.clone(),
    };
    let live_lease_fingerprint = settlement
        .callback_authority
        .as_ref()
        .unwrap()
        .lease_fingerprint
        .clone();
    let terminal = seal_terminal_pair_command(HookTerminalPairCommandV1 {
        scope: fixture.scope.clone(),
        owner: fixture.scope.agent_id.clone(),
        control_stream_id: runtime_control_stream_id(&fixture.scope).unwrap(),
        hook_stream_id: hook_stream_id(&fixture.scope).unwrap(),
        expected_control_cursor: EventCursor {
            sequence: "2".to_owned(),
            event_id: reserve.pair.control_event_id.clone(),
        },
        expected_hook_cursor: EventCursor {
            sequence: "2".to_owned(),
            event_id: reserve.pair.hook_event_id.clone(),
        },
        pair: terminal_pair.clone(),
        occurred_at: settlement.settled_at_utc.clone(),
        correlation_id: "corr-settle-hook".to_owned(),
        hook_payload: HookInvocationTerminalPayloadV1 {
            invocation_id,
            decision_id: HookDecisionId::parse("decision_pair-hook").unwrap(),
            terminal_state: HookInvocationTerminalStateV1::Succeeded,
            pair: terminal_pair,
            output_digest: Some(digest('5')),
            gate_decision: Some(GateDecisionV1::Allow {}),
            observer_result: None,
            accounted_usage: settlement.accounted_usage.clone(),
            reason_code: "allowed".to_owned(),
        },
        control_payload: settlement,
        authority: HookTerminalAuthorityV1::LiveLease {
            lease_fingerprint: live_lease_fingerprint,
        },
    })
    .unwrap();
    PairHarness {
        fixture,
        store,
        reserve,
        terminal,
    }
}

fn pair_targets(harness: &PairHarness) -> (HookTarget, control::RuntimeControlTarget) {
    (
        HookTarget {
            scope: harness.fixture.scope.clone(),
            actor: harness.fixture.scope.agent_id.clone(),
        },
        harness.fixture.target(),
    )
}

async fn event_count(store: &EventStore, event_id: &EventId) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE event_id=?")
        .bind(event_id.as_str())
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

fn pair_append_identity(result: &AppendResult) -> (EventId, i64) {
    match result {
        AppendResult::Appended { event_id, sequence }
        | AppendResult::AlreadyCommitted { event_id, sequence } => (event_id.clone(), *sequence),
    }
}

pub(super) async fn reserve_pair_atomicity_case() {
    let harness = pair_harness("run_hook-reserve-pair").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let first = harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(!first.already_committed);
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.control_event_id).await,
        1
    );
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.hook_event_id).await,
        1
    );
    let retry = harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(retry.already_committed);
    assert_eq!(
        pair_append_identity(&first.control),
        pair_append_identity(&retry.control)
    );
    assert_eq!(
        pair_append_identity(&first.hook),
        pair_append_identity(&retry.hook)
    );
    assert_eq!(first.lease, retry.lease);
}

pub(super) async fn pair_fault_injection_case() {
    for (run, fault) in [
        ("run_hook-fault-first", AtomicPairFault::AfterFirstInsert),
        ("run_hook-fault-commit", AtomicPairFault::BeforeCommit),
    ] {
        let harness = pair_harness(run).await;
        let (hook_target, control_target) = pair_targets(&harness);
        assert!(
            harness
                .store
                .append_hook_reserve_pair(
                    &harness.fixture.registry(),
                    &hook_target,
                    &control_target,
                    &harness.reserve,
                    fault,
                )
                .await
                .is_err()
        );
        assert_eq!(
            event_count(&harness.store, &harness.reserve.pair.control_event_id).await,
            0
        );
        assert_eq!(
            event_count(&harness.store, &harness.reserve.pair.hook_event_id).await,
            0
        );
    }
}

pub(super) async fn terminal_pair_atomicity_case() {
    let harness = pair_harness("run_hook-terminal-pair").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let reserve = harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let source = harness
        .store
        .hook_source(&harness.fixture.registry(), &hook_target)
        .await
        .unwrap();
    let hook_events = harness
        .store
        .read_hook_events(&hook_target, source.0.clone(), source.1)
        .await
        .unwrap();
    assert_eq!(
        hook_events
            .iter()
            .map(|event| (event.variant_id(), event.envelope().event_type.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("hook-stream-initialized-v1", "hook-stream-initialized"),
            ("hook-invocation-reserved-v1", "hook-invocation-reserved"),
        ]
    );
    fold_hook_events(&source.0, &hook_events).unwrap();
    harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &control_target)
        .await
        .unwrap();
    let first = harness
        .store
        .append_hook_terminal_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(!first.already_committed);
    let retry = harness
        .store
        .append_hook_terminal_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert!(retry.already_committed);
    assert_eq!(
        pair_append_identity(&first.control),
        pair_append_identity(&retry.control)
    );
    assert_eq!(
        pair_append_identity(&first.hook),
        pair_append_identity(&retry.hook)
    );
    let generic = control::settlement(&harness.fixture, "hook", OperationOutcomeV1::Succeeded, 1);
    assert_eq!(
        harness
            .store
            .settle_operation(
                &harness.fixture.registry(),
                &control_target,
                &reserve.lease,
                &generic,
            )
            .await
            .unwrap_err()
            .kind,
        control::RuntimeControlErrorKind::ProducerUnauthorized
    );
}

pub(super) async fn idempotency_case() {
    let harness = pair_harness("run_hook-pair-idempotency").await;
    let (hook_target, control_target) = pair_targets(&harness);
    harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let mut mutated = harness.reserve.clone();
    mutated.correlation_id = "corr-mutated".to_owned();
    let mutated = seal_reserve_pair_command(mutated).unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &mutated,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::IdempotencyConflict
    );
}

pub(super) async fn pair_corruption_case() {
    let harness = pair_harness("run_hook-pair-corruption").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let mut transaction = harness
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    let lifecycle = load_established(
        &mut transaction,
        &harness.fixture.registry(),
        &LifecycleTarget {
            scope: harness.fixture.scope.clone(),
            actor: harness.fixture.scope.agent_id.clone(),
        },
    )
    .await
    .unwrap();
    let event = control_event(
        &lifecycle,
        &harness.reserve.control_stream_id,
        &harness.reserve.pair.control_event_id,
        2,
        &harness.reserve.occurred_at,
        &harness.reserve.correlation_id,
        "operation-reserved",
        &harness.reserve.control_payload,
    )
    .unwrap();
    let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits).unwrap();
    crate::event_store::insert_prepared(&mut transaction, &prepared)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &control_target)
        .await
        .unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &harness.reserve,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::PartialPair
    );
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.control_event_id).await,
        1
    );
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.hook_event_id).await,
        0
    );
}

pub(super) async fn authority_case() {
    let harness = pair_harness("run_hook-authority").await;
    let (hook_target, control_target) = pair_targets(&harness);
    harness
        .store
        .revoke_capability(
            &harness.fixture.registry(),
            &control_target,
            &control::RevokeCapabilityCommand {
                event_id: EventId::parse("event_revoke-hook").unwrap(),
                occurred_at: "2026-08-26T00:00:09.000Z".to_owned(),
                correlation_id: "corr-revoke-hook".to_owned(),
                grant_id: harness.reserve.control_payload.grant_id.clone(),
                reason_code: "revoked-before-hook".to_owned(),
            },
        )
        .await
        .unwrap();
    let mut revoked = harness.reserve.clone();
    revoked.expected_control_cursor = EventCursor {
        sequence: "2".to_owned(),
        event_id: EventId::parse("event_revoke-hook").unwrap(),
    };
    let revoked = seal_reserve_pair_command(revoked).unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &revoked,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::Unauthorized
    );

    let narrowed_harness = pair_harness("run_hook-authority-narrowed").await;
    let (hook_target, control_target) = pair_targets(&narrowed_harness);
    let mut narrowed = narrowed_harness.reserve.clone();
    narrowed.control_payload.trusted_reservation[0].amount =
        pareto_protocol::BudgetAmountV1::new(1);
    narrowed.hook_payload.reserved_usage = narrowed.control_payload.trusted_reservation.clone();
    let narrowed = seal_reserve_pair_command(narrowed).unwrap();
    assert!(
        narrowed_harness
            .store
            .append_hook_reserve_pair(
                &narrowed_harness.fixture.registry(),
                &hook_target,
                &control_target,
                &narrowed,
                AtomicPairFault::None,
            )
            .await
            .is_err()
    );
}

pub(super) async fn isolation_case() {
    let harness = pair_harness("run_hook-isolation").await;
    let (mut hook_target, control_target) = pair_targets(&harness);
    hook_target.actor = AgentId::parse("agent_intruder").unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &harness.reserve,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::Unauthorized
    );
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.control_event_id).await,
        0
    );
    assert_eq!(
        event_count(&harness.store, &harness.reserve.pair.hook_event_id).await,
        0
    );
}

pub(super) async fn budget_reserve_case() {
    let harness = pair_harness("run_hook-budget-reserve").await;
    let (hook_target, control_target) = pair_targets(&harness);
    harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let projection = harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &control_target)
        .await
        .unwrap();
    let reservation = &harness.reserve.control_payload;
    for allocation in &reservation.allocations {
        let account = projection
            .accounts
            .iter()
            .find(|account| account.account.account_id == allocation.account_id)
            .unwrap();
        assert_eq!(account.reserved.as_u64(), allocation.amount.as_u64());
    }
    assert_eq!(projection.operations.len(), 1);
    assert_eq!(
        projection.operations[0].reservation.hook_pair,
        Some(harness.reserve.pair.clone())
    );
}

pub(super) async fn budget_concurrency_case() {
    let harness = pair_harness("run_hook-budget-concurrency").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    let first = harness.store.append_hook_reserve_pair(
        &registry,
        &hook_target,
        &control_target,
        &harness.reserve,
        AtomicPairFault::None,
    );
    let second = harness.store.append_hook_reserve_pair(
        &registry,
        &hook_target,
        &control_target,
        &harness.reserve,
        AtomicPairFault::None,
    );
    let (first, second) = tokio::join!(first, second);
    let first = first.unwrap();
    let second = second.unwrap();
    assert_ne!(first.already_committed, second.already_committed);
    let projection = harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &control_target)
        .await
        .unwrap();
    assert_eq!(projection.operations.len(), 1);
    for allocation in &harness.reserve.control_payload.allocations {
        let account = projection
            .accounts
            .iter()
            .find(|account| account.account.account_id == allocation.account_id)
            .unwrap();
        assert_eq!(account.reserved.as_u64(), allocation.amount.as_u64());
    }
}

pub(super) async fn settlement_case() {
    let harness = pair_harness("run_hook-settlement").await;
    let (hook_target, control_target) = pair_targets(&harness);
    harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    harness
        .store
        .append_hook_terminal_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let control = harness
        .store
        .runtime_control_projection(&harness.fixture.registry(), &control_target)
        .await
        .unwrap();
    assert_eq!(
        control.operations[0].accounted_usage,
        harness.terminal.control_payload.accounted_usage
    );
    assert!(
        control
            .accounts
            .iter()
            .all(|account| account.reserved.as_u64() == 0)
    );
    let hook = harness
        .store
        .hook_projection(&harness.fixture.registry(), &hook_target)
        .await
        .unwrap();
    assert_eq!(
        hook.invocations[0].terminal_state,
        Some(HookInvocationTerminalStateV1::Succeeded)
    );
}

fn evaluation_fixture() -> (
    Fixture,
    ResolvedHookRegistry,
    TransformProposalV1,
    ProtectedProposalHashViewV1,
) {
    let fixture = Fixture::new("run_hook-evaluation");
    let (manifest, registry) = registry_fixture(&fixture);
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry).unwrap();
    let proposal = TransformProposalV1 {
        proposal_id: ProposalId::parse("proposal_evaluation").unwrap(),
        schema_ref: fixture
            .set
            .schema_ref("transform-proposal")
            .unwrap()
            .clone(),
        fields: serde_json::json!({"content":"initial","authority":"fixed"}),
    };
    let protected = ProtectedProposalHashViewV1 {
        scope: fixture.scope.clone(),
        proposal_id: proposal.proposal_id.clone(),
        schema_set_ref: fixture.set.reference().clone(),
        hook_registry_revision: resolved.revision.clone(),
        authority_digest: digest('a'),
        unknown_fields_digest: digest('b'),
    };
    (fixture, resolved, proposal, protected)
}

fn successful_outputs(
    proposal: &TransformProposalV1,
    protected: &ProtectedProposalHashViewV1,
) -> BTreeMap<HookId, Result<UntrustedHookOutput, String>> {
    let mut first = proposal.clone();
    first.fields["content"] = serde_json::json!("first");
    let mut final_proposal = proposal.clone();
    final_proposal.fields["content"] = serde_json::json!("final");
    BTreeMap::from([
        (
            HookId::parse("hook_transform-b").unwrap(),
            Ok(UntrustedHookOutput::Transform {
                proposal: Box::new(first),
                protected: Box::new(protected.clone()),
            }),
        ),
        (
            HookId::parse("hook_transform-a").unwrap(),
            Ok(UntrustedHookOutput::Transform {
                proposal: Box::new(final_proposal),
                protected: Box::new(protected.clone()),
            }),
        ),
        (
            HookId::parse("hook_gate-a").unwrap(),
            Ok(UntrustedHookOutput::Gate(GateDecisionV1::Allow {})),
        ),
        (
            HookId::parse("hook_observer-a").unwrap(),
            Ok(UntrustedHookOutput::Observer(ObserverResultV1::Observed {
                annotation_digest: digest('c'),
            })),
        ),
    ])
}

pub(super) fn gate_composition_case() {
    let (fixture, resolved, proposal, protected) = evaluation_fixture();
    let mut outputs = successful_outputs(&proposal, &protected);
    let allowed = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(allowed.business_decision, HookBusinessDecisionV1::Allow);
    outputs.insert(
        HookId::parse("hook_gate-a").unwrap(),
        Ok(UntrustedHookOutput::Gate(GateDecisionV1::Deny {
            reason_code: "policy_denied".to_owned(),
        })),
    );
    let denied = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(denied.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(denied.execution_status, HookExecutionStatusV1::GateDenied);
}

pub(super) fn default_deny_case() {
    let (fixture, mut resolved, proposal, protected) = evaluation_fixture();
    for registration in &mut resolved.registrations {
        if registration.kind == HookKindV1::Gate {
            registration.required = Some(false);
        }
    }
    let denied = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &successful_outputs(&proposal, &protected),
    );
    assert_eq!(denied.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(denied.reason_code, "required_gate_empty");
}

pub(super) fn failure_policy_case() {
    let (fixture, mut resolved, proposal, protected) = evaluation_fixture();
    for registration in &mut resolved.registrations {
        if registration.kind == HookKindV1::Observer {
            registration.observer_failure_policy = Some(ObserverFailurePolicyV1::FailClosed);
        }
    }
    let mut outputs = successful_outputs(&proposal, &protected);
    outputs.insert(
        HookId::parse("hook_observer-a").unwrap(),
        Ok(UntrustedHookOutput::Observer(ObserverResultV1::Failure {
            reason_code: "observer_failed".to_owned(),
        })),
    );
    let result = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Allow);
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::ObserverFailed
    );
}

pub(super) fn observer_non_authority_case() {
    let (fixture, resolved, proposal, protected) = evaluation_fixture();
    let mut outputs = successful_outputs(&proposal, &protected);
    outputs.insert(
        HookId::parse("hook_observer-a").unwrap(),
        Ok(UntrustedHookOutput::Observer(ObserverResultV1::Failure {
            reason_code: "untrusted_deny_attempt".to_owned(),
        })),
    );
    let result = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Allow);
    assert_eq!(result.execution_status, HookExecutionStatusV1::Completed);
}

pub(super) fn transform_chain_failure_case() {
    let (fixture, resolved, proposal, protected) = evaluation_fixture();
    let mut outputs = successful_outputs(&proposal, &protected);
    outputs.remove(&HookId::parse("hook_transform-a").unwrap());
    let result = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.proposal, proposal);
}

pub(super) fn transform_protected_fields_case() {
    let (fixture, resolved, proposal, protected) = evaluation_fixture();
    let mut outputs = successful_outputs(&proposal, &protected);
    let mut changed = protected.clone();
    changed.authority_digest = digest('d');
    let mut candidate = proposal.clone();
    candidate.fields["content"] = serde_json::json!("attempted");
    outputs.insert(
        HookId::parse("hook_transform-b").unwrap(),
        Ok(UntrustedHookOutput::Transform {
            proposal: Box::new(candidate),
            protected: Box::new(changed),
        }),
    );
    let result = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.proposal, proposal);
}

pub(super) fn output_security_case() {
    let (fixture, mut resolved, proposal, protected) = evaluation_fixture();
    for registration in &mut resolved.registrations {
        if registration.kind == HookKindV1::Transform {
            registration.limits.max_output_bytes = 64;
        }
    }
    let mut outputs = successful_outputs(&proposal, &protected);
    let mut oversized = proposal.clone();
    oversized.fields["content"] = serde_json::json!("x".repeat(4096));
    outputs.insert(
        HookId::parse("hook_transform-b").unwrap(),
        Ok(UntrustedHookOutput::Transform {
            proposal: Box::new(oversized),
            protected: Box::new(protected.clone()),
        }),
    );
    let result = evaluate_point(
        &fixture.set,
        &resolved,
        HookPointV1::BeforeProposalAdmission,
        &proposal,
        &protected,
        &outputs,
    );
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.reason_code, "transform_output_invalid");
}

fn timeout_terminal(harness: &PairHarness, suffix: &str) -> HookTerminalPairCommandV1 {
    let mut terminal = harness.terminal.clone();
    terminal.pair.pair_id = HookPairId::parse(format!("pair_timeout-{suffix}")).unwrap();
    terminal.pair.control_event_id = EventId::parse(format!("event_timeout-{suffix}")).unwrap();
    terminal.pair.hook_event_id = EventId::parse(format!("event_hook-timeout-{suffix}")).unwrap();
    terminal.control_payload.callback_id = None;
    terminal.control_payload.callback_fingerprint = None;
    terminal.control_payload.callback_authority = None;
    terminal.control_payload.outcome = OperationOutcomeV1::TimedOut;
    terminal.control_payload.evidence_class = UsageEvidenceClassV1::Unknown;
    terminal.control_payload.kernel_meter_evidence = None;
    terminal.control_payload.observed_usage.clear();
    terminal.control_payload.accounted_usage =
        harness.reserve.control_payload.trusted_reservation.clone();
    terminal.control_payload.released_usage.clear();
    terminal.control_payload.reason_code = "deadline_elapsed".to_owned();
    terminal.control_payload.timeout_command_fingerprint = Some(digest('f'));
    terminal.control_payload.settled_at_utc = "2026-08-26T00:02:00.000Z".to_owned();
    terminal.hook_payload.terminal_state = HookInvocationTerminalStateV1::TimedOut;
    terminal.hook_payload.output_digest = None;
    terminal.hook_payload.gate_decision = None;
    terminal.hook_payload.observer_result = None;
    terminal.hook_payload.accounted_usage = terminal.control_payload.accounted_usage.clone();
    terminal.hook_payload.reason_code = "deadline_elapsed".to_owned();
    terminal.occurred_at = terminal.control_payload.settled_at_utc.clone();
    terminal.correlation_id = format!("corr-timeout-{suffix}");
    terminal.authority = HookTerminalAuthorityV1::TimeoutRecovery {
        timeout_key: Box::new(harness.reserve.control_payload.timeout_key.clone()),
    };
    seal_terminal_pair_command(terminal).unwrap()
}

pub(super) async fn cancellation_deadline_case() {
    let harness = pair_harness("run_hook-cancel-deadline").await;
    let (hook_target, control_target) = pair_targets(&harness);
    harness
        .store
        .append_hook_reserve_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    harness
        .store
        .request_cancellation(
            &harness.fixture.registry(),
            &control_target,
            &control::CancellationCommand {
                event_id: EventId::parse("event_cancel-hook").unwrap(),
                occurred_at: "2026-08-26T00:00:15.000Z".to_owned(),
                correlation_id: "corr-cancel-hook".to_owned(),
                cancellation_id: CancellationId::parse("cancel_hook").unwrap(),
                target: CancellationTargetV1::Operation {
                    operation_id: harness.reserve.pair.operation_id.clone(),
                },
                reason_code: "cancelled".to_owned(),
            },
        )
        .await
        .unwrap();
    let mut invalid_success = harness.terminal.clone();
    invalid_success.expected_control_cursor = EventCursor {
        sequence: "3".to_owned(),
        event_id: EventId::parse("event_cancel-hook").unwrap(),
    };
    let invalid_success = seal_terminal_pair_command(invalid_success).unwrap();
    assert!(
        harness
            .store
            .append_hook_terminal_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &invalid_success,
                AtomicPairFault::None,
            )
            .await
            .is_err()
    );
    let mut timeout = timeout_terminal(&harness, "cancelled");
    timeout.expected_control_cursor = EventCursor {
        sequence: "3".to_owned(),
        event_id: EventId::parse("event_cancel-hook").unwrap(),
    };
    let timeout = seal_terminal_pair_command(timeout).unwrap();
    harness
        .store
        .append_hook_terminal_pair(
            &harness.fixture.registry(),
            &hook_target,
            &control_target,
            &timeout,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
}

pub(super) async fn terminal_race_case() {
    let harness = pair_harness("run_hook-terminal-race").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    harness
        .store
        .append_hook_reserve_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let timeout = timeout_terminal(&harness, "race");
    let success = harness.store.append_hook_terminal_pair(
        &registry,
        &hook_target,
        &control_target,
        &harness.terminal,
        AtomicPairFault::None,
    );
    let timed_out = harness.store.append_hook_terminal_pair(
        &registry,
        &hook_target,
        &control_target,
        &timeout,
        AtomicPairFault::None,
    );
    let (success, timed_out) = tokio::join!(success, timed_out);
    assert_eq!(
        usize::from(success.is_ok()) + usize::from(timed_out.is_ok()),
        1
    );
    let hook = harness
        .store
        .hook_projection(&registry, &hook_target)
        .await
        .unwrap();
    assert!(hook.invocations[0].terminal_state.is_some());
    assert_eq!(hook.invocations.len(), 1);
}

pub(super) async fn model_sequences_case() {
    let harness = pair_harness("run_hook-model-sequences").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    assert!(
        harness
            .store
            .append_hook_terminal_pair(
                &registry,
                &hook_target,
                &control_target,
                &harness.terminal,
                AtomicPairFault::None,
            )
            .await
            .is_err()
    );
    for expected_retry in [false, true] {
        let result = harness
            .store
            .append_hook_reserve_pair(
                &registry,
                &hook_target,
                &control_target,
                &harness.reserve,
                AtomicPairFault::None,
            )
            .await
            .unwrap();
        assert_eq!(result.already_committed, expected_retry);
    }
    for expected_retry in [false, true] {
        let result = harness
            .store
            .append_hook_terminal_pair(
                &registry,
                &hook_target,
                &control_target,
                &harness.terminal,
                AtomicPairFault::None,
            )
            .await
            .unwrap();
        assert_eq!(result.already_committed, expected_retry);
    }
}

pub(super) async fn late_and_duplicate_case() {
    let harness = pair_harness("run_hook-late-duplicate").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    harness
        .store
        .append_hook_reserve_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    harness
        .store
        .append_hook_terminal_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    harness
        .store
        .append_hook_terminal_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let late = timeout_terminal(&harness, "late");
    assert!(
        harness
            .store
            .append_hook_terminal_pair(
                &registry,
                &hook_target,
                &control_target,
                &late,
                AtomicPairFault::None,
            )
            .await
            .is_err()
    );
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}

pub(super) async fn pair_recovery_case() {
    let harness = pair_harness("run_hook-pair-recovery").await;
    let timeout = timeout_terminal(&harness, "recovery");
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    harness
        .store
        .append_hook_reserve_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let store_id = harness.store.store_id.clone();
    drop(harness.store);
    let reopened = EventStore::open_pinned(&harness.fixture.path, &store_id)
        .await
        .unwrap();
    reopened
        .append_hook_terminal_pair(
            &registry,
            &hook_target,
            &control_target,
            &timeout,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let projection = reopened
        .hook_projection(&registry, &hook_target)
        .await
        .unwrap();
    assert_eq!(
        projection.invocations[0].terminal_state,
        Some(HookInvocationTerminalStateV1::TimedOut)
    );
}

pub(super) async fn recorded_vertical_case() {
    struct CountingGate(Arc<AtomicUsize>);
    impl FakeHookHandler for CountingGate {
        fn invoke(
            &self,
            lease: &HookInvocationLease,
            request: &HookRequestView,
        ) -> UntrustedHookOutput {
            assert!(lease.narrowed);
            assert_eq!(lease.input_digest, request.input_digest);
            self.0.fetch_add(1, Ordering::SeqCst);
            UntrustedHookOutput::Gate(GateDecisionV1::Allow {})
        }
    }

    let harness = pair_harness("run_hook-recorded-vertical").await;
    let (hook_target, control_target) = pair_targets(&harness);
    let registry = harness.fixture.registry();
    harness
        .store
        .append_hook_reserve_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.reserve,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    let live_counter = Arc::new(AtomicUsize::new(0));
    let handler = CountingGate(live_counter.clone());
    let invocation_lease = HookInvocationLease {
        invocation_id: harness.reserve.pair.invocation_id.clone(),
        hook_id: harness.reserve.hook_payload.key.hook_id.clone(),
        input_digest: harness.reserve.hook_payload.key.input_digest.clone(),
        scope: harness.fixture.scope.clone(),
        narrowed: true,
    };
    let request = HookRequestView {
        hook_point: harness.reserve.hook_payload.key.hook_point,
        phase: harness.reserve.hook_payload.key.phase,
        input_digest: harness.reserve.hook_payload.key.input_digest.clone(),
        fixed_business_decision: None,
    };
    assert_eq!(
        handler.invoke(&invocation_lease, &request),
        UntrustedHookOutput::Gate(GateDecisionV1::Allow {})
    );
    harness
        .store
        .append_hook_terminal_pair(
            &registry,
            &hook_target,
            &control_target,
            &harness.terminal,
            AtomicPairFault::None,
        )
        .await
        .unwrap();
    assert_eq!(live_counter.load(Ordering::SeqCst), 1);
    let before_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    let before_control = harness
        .store
        .runtime_control_projection(&registry, &control_target)
        .await
        .unwrap();
    let normal = harness
        .store
        .hook_projection(&registry, &hook_target)
        .await
        .unwrap();
    let recorded_counter = Arc::new(AtomicUsize::new(0));
    let recorded = harness
        .store
        .recorded_hook_projection(
            &registry,
            &hook_target,
            &ExecutionMode::RecordedReplay {
                source_run_id: harness.fixture.scope.run_id.clone(),
                boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
            },
        )
        .await
        .unwrap();
    let after_control = harness
        .store
        .runtime_control_projection(&registry, &control_target)
        .await
        .unwrap();
    let after_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&harness.store.pool)
        .await
        .unwrap();
    assert_eq!(recorded_counter.load(Ordering::SeqCst), 0);
    assert_eq!(normal, recorded);
    assert_eq!(before_events, after_events);
    assert_eq!(before_control.accounts, after_control.accounts);
    assert_eq!(before_control.operations, after_control.operations);
}

pub(super) async fn unsupported_modes_case() {
    let fixture = Fixture::new("run_hook-unsupported-modes");
    let store = fixture.open_initialized().await;
    for mode in [
        ExecutionMode::Live {},
        ExecutionMode::Reexecute {
            source_run_id: RunId::parse("run_source").unwrap(),
            boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
        },
    ] {
        assert_eq!(
            store
                .recorded_hook_projection(&fixture.registry(), &fixture.target(), &mode)
                .await
                .unwrap_err()
                .kind,
            HookErrorKind::UnsupportedMode
        );
    }
}

pub(super) async fn fold_contract_case() {
    let fixture = Fixture::new("run_hook-fold");
    let store = fixture.open_initialized().await;
    let events = store
        .read_hook_events(
            &fixture.target(),
            fixture.set.clone(),
            fixture.limits.clone(),
        )
        .await
        .unwrap();
    let aggregate = fold_hook_events(&fixture.set, &events).unwrap();
    assert_eq!(aggregate.inclusive_cursor.sequence, "1");
    assert!(aggregate.invocations.is_empty());
    let projection = store
        .hook_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(projection.inclusive_cursor.sequence, "1");
    assert_eq!(
        projection.hook_registry_revision,
        RevisionId::parse("rev_hook-registry").unwrap()
    );

    let duplicate = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &hook_stream_id(&fixture.scope).unwrap(),
        &EventId::parse("event_hook-invalid-second-init").unwrap(),
        2,
        "2026-08-28T00:00:02.000Z",
        "corr-invalid",
        "hook-stream-initialized",
        &aggregate.initialization,
    )
    .unwrap();
    let mut invalid = events;
    invalid.push(duplicate);
    assert_eq!(
        fold_hook_events(&fixture.set, &invalid).unwrap_err().kind,
        HookErrorKind::AggregateCorrupt
    );
}

#[tokio::test]
async fn recovery() {
    let fixture = Fixture::new("run_hook-recovery");
    let store = fixture.open_initialized().await;
    let store_id = store.store_id.clone();
    let before = store
        .hook_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    store.pool.close().await;
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let after = reopened
        .hook_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(before, after);
}

pub(super) async fn compatibility_case() {
    let fixture = Fixture::new("run_hook-compatibility");
    let store = EventStore::open(&fixture.path).await.unwrap();
    store
        .create_run(
            &fixture.trusted(),
            &CreateRunCommand {
                event_id: EventId::parse("event_run-created").unwrap(),
                occurred_at: "2026-08-28T00:00:00.000Z".to_owned(),
                correlation_id: "corr-run-create".to_owned(),
                manifest: fixture.manifest.clone(),
            },
        )
        .await
        .unwrap();
    let mut wrong_revision = fixture.initialization();
    wrong_revision.hook_registry_revision = RevisionId::parse("rev_current-substitution").unwrap();
    assert_eq!(
        store
            .initialize_hook_stream(&fixture.registry(), &fixture.target(), &wrong_revision)
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::ManifestInvalid
    );
    assert_eq!(
        store
            .hook_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateNotFound
    );
}

#[tokio::test]
async fn recorded_replay() {
    let fixture = Fixture::new("run_hook-recorded");
    let store = fixture.open_initialized().await;
    let before_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let normal = store
        .hook_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    let recorded = store
        .recorded_hook_projection(
            &fixture.registry(),
            &fixture.target(),
            &ExecutionMode::RecordedReplay {
                source_run_id: RunId::parse("run_source").unwrap(),
                boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
            },
        )
        .await
        .unwrap();
    let after_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(normal, recorded);
    assert_eq!(before_count, after_count);
    for unsupported in [
        ExecutionMode::Live {},
        ExecutionMode::Reexecute {
            source_run_id: RunId::parse("run_source").unwrap(),
            boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
        },
    ] {
        assert_eq!(
            store
                .recorded_hook_projection(&fixture.registry(), &fixture.target(), &unsupported)
                .await
                .unwrap_err()
                .kind,
            HookErrorKind::UnsupportedMode
        );
    }
}
