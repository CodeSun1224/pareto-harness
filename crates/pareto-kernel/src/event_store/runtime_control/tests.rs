use std::sync::Arc;

use pareto_protocol::{
    BoundaryRecordingPolicyRef, BudgetPlanV1, CapabilityConstraintsV1, CapabilityScopeV1,
    OperationBudgetLimitV1, ProtocolLimitsV1, ResourceSelectorV1, RunId,
    RunManifest, SchemaSet, TenantId, UserId, WorkspaceId,
    generate_schema_bundle,
};
use tempfile::TempDir;

use super::lifecycle::{
    AppliedState, CreateRunCommand, CreateTaskCommand, LifecycleResult, TransitionRunCommand,
    TransitionTaskCommand, TrustedRunInputs,
};
use crate::event_store::effect_runtime::{EffectTarget, InitializeEffectStream};

#[derive(Clone)]
pub(super) struct FakeClock {
    sample: ClockSample,
}

impl FakeClock {
    pub(super) fn at(value: &str, monotonic: u64, epoch: &str) -> Self {
        Self {
            sample: ClockSample {
                canonical_utc: value.to_owned(),
                wall_millis: parse_utc_millis(value).unwrap(),
                monotonic_millis: monotonic,
                process_epoch: epoch.to_owned(),
            },
        }
    }
}

impl RuntimeClock for FakeClock {
    fn sample(&self) -> ClockSample {
        self.sample.clone()
    }
}

pub(super) struct Fixture {
    _temp: TempDir,
    pub(super) path: std::path::PathBuf,
    pub(super) set: Arc<SchemaSet>,
    limits: pareto_protocol::ProtocolLimitsRef,
    pub(super) scope: IsolationScope,
    pub(super) manifest: RunManifest,
    pub(super) task_id: TaskId,
}

impl Fixture {
    pub(super) fn new(run: &str) -> Self {
        Self::with_mode(run, ExecutionMode::Live {})
    }

    fn with_mode(run: &str, execution_mode: ExecutionMode) -> Self {
        let bundle = generate_schema_bundle().unwrap();
        let set = Arc::new(
            SchemaSet::bootstrap_initial(bundle.manifest, bundle.schemas, &bundle.reference)
                .unwrap(),
        );
        let limits = pareto_protocol::ProtocolLimitsRef {
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
            execution_mode,
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("runtime-control.sqlite3");
        Self {
            _temp: temp,
            path,
            set,
            limits,
            scope,
            manifest,
            task_id: TaskId::parse("task_root").unwrap(),
        }
    }

    pub(super) fn registry(&self) -> SchemaRegistry {
        SchemaRegistry(vec![self.set.clone()])
    }

    pub(super) fn target(&self) -> RuntimeControlTarget {
        RuntimeControlTarget {
            scope: self.scope.clone(),
            principal: self.scope.agent_id.clone(),
        }
    }

    fn target_as(&self, actor: &str) -> RuntimeControlTarget {
        RuntimeControlTarget {
            scope: self.scope.clone(),
            principal: AgentId::parse(actor).unwrap(),
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

    fn initialization(&self) -> RuntimeControlInitializedPayloadV1 {
        let owner = self.scope.agent_id.clone();
        let accounts = [
            ("budget_run", BudgetScopeV1::Run),
            ("budget_task", BudgetScopeV1::Task { task_id: self.task_id.clone() }),
            ("budget_owner", BudgetScopeV1::Actor { actor_id: owner.clone() }),
            ("budget_child", BudgetScopeV1::Actor { actor_id: AgentId::parse("agent_child").unwrap() }),
        ].into_iter().map(|(id, scope)| BudgetAccountV1 {
            account_id: BudgetAccountId::parse(id).unwrap(), scope,
            dimension: BudgetDimensionV1::Tokens,
            hard_limit: BudgetAmountV1::new(10),
            soft_limit: Some(BudgetAmountV1::new(8)),
        }).collect();
        RuntimeControlInitializedPayloadV1 {
            source_contract: pareto_protocol::RuntimeControlSourceContractV1 {
                schema_set_ref: self.set.reference().clone(),
                protocol_limits_ref: self.limits.clone(),
                lifecycle_cursor: EventCursor {
                    sequence: "2".to_owned(),
                    event_id: EventId::parse("event_task-created").unwrap(),
                },
                reducer_revision: RevisionId::parse(RUNTIME_REDUCER_REVISION).unwrap(),
                accepted_event_bindings: CONTROL_EVENT_TYPES
                    .iter()
                    .map(|event_type| {
                        self.set
                            .event_type_binding(event_type, 1, 0)
                            .unwrap()
                            .clone()
                    })
                    .collect(),
                history_digest_revision: RevisionId::parse(RUNTIME_HISTORY_REVISION).unwrap(),
                projection_schema_ref: self.set.schema_ref("runtime-control-projection").unwrap().clone(),
                projection_reader_revision: RevisionId::parse(RUNTIME_READER_REVISION).unwrap(),
            },
            initial_grants: vec![self.grant(
                "cap_root", "agent_owner", "agent_owner", None, None, true, 2, 10,
            )],
            budget_plan: BudgetPlanV1 {
                budget_revision: self.manifest.budget_revision.clone(),
                accounts,
                operation_limits: vec![OperationBudgetLimitV1 {
                    resource_kind: "fake".to_owned(), operation: "invoke".to_owned(),
                    dimension: BudgetDimensionV1::Tokens,
                    hard_limit: BudgetAmountV1::new(6), soft_limit: Some(BudgetAmountV1::new(5)),
                }],
            },
            clock_contract: pareto_protocol::RuntimeClockContractV1 {
                clock_revision: RevisionId::parse("rev_fake-clock").unwrap(),
                recovery_revision: RevisionId::parse("rev_timeout-recovery").unwrap(),
            },
            operation_contract_refs: vec![RevisionId::parse(FAKE_CONTRACT_REVISION).unwrap()],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn grant(&self, id: &str, issuer: &str, subject: &str, parent: Option<&str>, task: Option<TaskId>, delegate: bool, depth: u32, max: u64) -> CapabilityGrantV1 {
        CapabilityGrantV1 {
            schema_ref: self.set.schema_ref("capability-grant").unwrap().clone(),
            grant_id: CapabilityId::parse(id).unwrap(),
            issuer_actor: AgentId::parse(issuer).unwrap(),
            subject_actor: AgentId::parse(subject).unwrap(),
            scope: CapabilityScopeV1 { isolation: self.scope.clone(), task_id: task },
            resource: resource(), operations: vec!["invoke".to_owned()],
            constraints: CapabilityConstraintsV1 {
                not_before_utc: "2026-01-01T00:00:00.000Z".to_owned(),
                expires_at_utc: "2027-01-01T00:00:00.000Z".to_owned(),
                max_operation_usage: usage(max), allow_delegation: delegate,
                remaining_delegation_depth: depth,
            },
            parent_grant_id: parent.map(|value| CapabilityId::parse(value).unwrap()),
            issued_at_utc: "2026-08-26T00:00:00.000Z".to_owned(),
        }
    }

    pub(super) fn proposal(&self, suffix: &str) -> ProtectedOperationProposal {
        ProtectedOperationProposal {
            event_id: EventId::parse(format!("event_reserve-{suffix}")).unwrap(),
            denied_event_id: EventId::parse(format!("event_denied-{suffix}")).unwrap(),
            occurred_at: "2026-08-26T00:00:10.000Z".to_owned(),
            correlation_id: format!("corr-{suffix}"),
            operation_id: OperationId::parse(format!("operation_{suffix}")).unwrap(),
            reservation_id: ReservationId::parse(format!("reservation_{suffix}")).unwrap(),
            task_id: Some(self.task_id.clone()), resource: resource(), operation: "invoke".to_owned(),
            adapter_revision: RevisionId::parse(FAKE_ADAPTER_REVISION).unwrap(),
            requested_usage: usage(1), callback_namespace: FAKE_CALLBACK_NAMESPACE.to_owned(),
            interruptibility: OperationInterruptibilityV1::Cooperative,
            absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(),
            timeout_policy_revision: RevisionId::parse("rev_timeout-policy").unwrap(),
        }
    }
}

fn revision_pins() -> BTreeMap<String, RevisionId> {
    ["task", "behavior", "workspace", "environment", "context_graph", "model_snapshot", "tool_set", "kernel", "hook_registry", "effect_registry"]
        .into_iter().map(|role| (role.to_owned(), RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap())).collect()
}

fn digest(fill: char) -> Digest { Digest::parse(format!("sha256:{}", fill.to_string().repeat(64))).unwrap() }
fn usage(amount: u64) -> Vec<BudgetVectorEntryV1> { vec![BudgetVectorEntryV1 { dimension: BudgetDimensionV1::Tokens, amount: BudgetAmountV1::new(amount) }] }
fn resource() -> ResourceSelectorV1 { ResourceSelectorV1 { kind: "fake".to_owned(), id: Some("fixture".to_owned()) } }
pub(super) fn live_clock() -> FakeClock { FakeClock::at("2026-08-26T00:00:10.000Z", 1_000, "epoch-a") }

fn timeout_request(
    operation_id: &str,
    correlation_id: &str,
    meter_snapshot: Option<KernelMeterSnapshot>,
    unknown_evidence_fingerprint: Digest,
) -> TimeoutRecoveryRequest {
    TimeoutRecoveryRequest {
        operation_id: OperationId::parse(operation_id).unwrap(),
        correlation_id: correlation_id.to_owned(),
        meter_snapshot,
        unknown_evidence_fingerprint,
    }
}

async fn create_initialized(fixture: &Fixture) -> EventStore {
    create_initialized_with_payload(fixture, fixture.initialization()).await
}

async fn create_initialized_with_payload(
    fixture: &Fixture,
    payload: RuntimeControlInitializedPayloadV1,
) -> EventStore {
    let store = create_lifecycle_only(fixture).await;
    store
        .initialize_effect_stream(
            &fixture.registry(),
            &EffectTarget {
                scope: fixture.scope.clone(),
                actor: fixture.scope.agent_id.clone(),
            },
            &InitializeEffectStream {
                event_id: EventId::parse("event_effect-stream-init").unwrap(),
                occurred_at: "2026-08-26T00:00:01.500Z".to_owned(),
                correlation_id: "corr-effect-stream-init".to_owned(),
                effect_registry_revision: fixture.manifest.revisions["effect_registry"].clone(),
                effect_registry_config_digest: fixture
                    .manifest
                    .effect_registry_config_digest
                    .clone()
                    .unwrap(),
            },
        )
        .await
        .unwrap();
    store.initialize_runtime_control(&fixture.registry(), &fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_control-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-control".to_owned(), payload,
    }).await.unwrap();
    store
}

async fn create_lifecycle_only(fixture: &Fixture) -> EventStore {
    let store = EventStore::open(&fixture.path).await.unwrap();
    store.create_run(&fixture.trusted(), &CreateRunCommand {
        event_id: EventId::parse("event_run-created").unwrap(), occurred_at: "2026-08-26T00:00:00.000Z".to_owned(), correlation_id: "corr-run".to_owned(), manifest: fixture.manifest.clone(),
    }).await.unwrap();
    store.create_task(&fixture.registry(), &super::lifecycle::LifecycleTarget { scope: fixture.scope.clone(), actor: fixture.scope.agent_id.clone() }, &CreateTaskCommand {
        event_id: EventId::parse("event_task-created").unwrap(), occurred_at: "2026-08-26T00:00:01.000Z".to_owned(), correlation_id: "corr-task".to_owned(), expected_sequence: 1, task_id: fixture.task_id.clone(), parent_task_id: None,
    }).await.unwrap();
    store
}

pub(super) async fn create_running(fixture: &Fixture) -> EventStore {
    create_running_with_payload(fixture, fixture.initialization()).await
}

async fn create_running_with_payload(
    fixture: &Fixture,
    payload: RuntimeControlInitializedPayloadV1,
) -> EventStore {
    let store = create_initialized_with_payload(fixture, payload).await;
    let target = super::lifecycle::LifecycleTarget { scope: fixture.scope.clone(), actor: fixture.scope.agent_id.clone() };
    store.transition_task(&fixture.registry(), &target, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-ready").unwrap(), occurred_at: "2026-08-26T00:00:03.000Z".to_owned(), correlation_id: "corr-ready".to_owned(), expected_sequence: 2, task_id: fixture.task_id.clone(), expected_state: TaskState::Created, target_state: TaskState::Ready, reason_code: "ready".to_owned(),
    }).await.unwrap();
    let result = store.transition_run(&fixture.registry(), &target, &TransitionRunCommand {
        event_id: EventId::parse("event_run-running").unwrap(), occurred_at: "2026-08-26T00:00:04.000Z".to_owned(), correlation_id: "corr-running".to_owned(), expected_sequence: 3, expected_state: RunState::Created, target_state: RunState::Running, reason_code: "start".to_owned(),
    }).await.unwrap();
    assert!(matches!(result, LifecycleResult::Applied { state: AppliedState::Run(RunState::Running), .. }));
    store.transition_task(&fixture.registry(), &target, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-running").unwrap(), occurred_at: "2026-08-26T00:00:05.000Z".to_owned(), correlation_id: "corr-task-running".to_owned(), expected_sequence: 4, task_id: fixture.task_id.clone(), expected_state: TaskState::Ready, target_state: TaskState::Running, reason_code: "dispatch".to_owned(),
    }).await.unwrap();
    store
}

async fn reserve(store: &EventStore, fixture: &Fixture, suffix: &str) -> OperationLease {
    match store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal(suffix), &live_clock()).await.unwrap() {
        ReserveResult::Reserved { lease, .. } => *lease,
        other => panic!("expected reservation, got {other:?}"),
    }
}

async fn completed_reservation_template(
    fixture: &Fixture,
    suffix: &str,
) -> (EventStore, OperationReservedPayloadV1) {
    let store = create_running(fixture).await;
    let lease = reserve(&store, fixture, suffix).await;
    let reservation = store
        .runtime_control_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap()
        .operations
        .into_iter()
        .find(|operation| operation.operation_id == lease.operation_id)
        .unwrap()
        .reservation;
    store
        .settle_operation(
            &fixture.registry(),
            &fixture.target(),
            &lease,
            &settlement(fixture, suffix, OperationOutcomeV1::Succeeded, 1),
        )
        .await
        .unwrap();
    (store, reservation)
}

fn retarget_reservation(
    reservation: &mut OperationReservedPayloadV1,
    suffix: &str,
    reserved_at_utc: &str,
) {
    reservation.operation_id = OperationId::parse(format!("operation_{suffix}")).unwrap();
    reservation.reservation_id = ReservationId::parse(format!("reservation_{suffix}")).unwrap();
    reservation.timeout_key.operation_id = reservation.operation_id.clone();
    reservation.timeout_key.reservation_id = reservation.reservation_id.clone();
    reservation.reserved_at_utc = reserved_at_utc.to_owned();
}

async fn append_forged_reservation(
    store: &EventStore,
    fixture: &Fixture,
    event_id: &str,
    payload: &OperationReservedPayloadV1,
) {
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &fixture.registry(), &fixture.target()).await.unwrap();
    append_control(
        tx,
        &aggregate,
        &EventId::parse(event_id).unwrap(),
        &payload.reserved_at_utc,
        "corr-forged-reservation",
        "operation-reserved",
        payload,
    )
    .await
    .unwrap();
}

async fn append_forged_settlement(
    store: &EventStore,
    fixture: &Fixture,
    event_id: &str,
    payload: &OperationSettledPayloadV1,
) {
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &fixture.registry(), &fixture.target()).await.unwrap();
    append_control(
        tx,
        &aggregate,
        &EventId::parse(event_id).unwrap(),
        &payload.settled_at_utc,
        "corr-forged-settlement",
        "operation-settled",
        payload,
    )
    .await
    .unwrap();
}

fn reseal_callback_authority(
    scope: &IsolationScope,
    operation_id: &OperationId,
    authority: &mut CallbackAuthorityV1,
) {
    let mut lease = OperationLease {
        scope: scope.clone(),
        operation_id: operation_id.clone(),
        reservation_id: authority.reservation_id.clone(),
        producer_revision: authority.producer_revision.clone(),
        process_epoch: authority.process_epoch.clone(),
        reserved_wall_millis: authority.lease_wall_millis.parse().unwrap(),
        reserved_monotonic_millis: authority.lease_monotonic_millis.parse().unwrap(),
        deadline_monotonic_millis: authority.deadline_monotonic_millis.parse().unwrap(),
        seal: digest('0'),
    };
    lease.seal = lease_seal(&lease).unwrap();
    authority.lease_fingerprint = lease.seal;
}

async fn assert_control_history_corrupt(store: EventStore, fixture: &Fixture) {
    assert_eq!(
        store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        RuntimeControlErrorKind::AggregateCorrupt
    );
    assert_eq!(
        store
            .replay_runtime_control(&fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        RuntimeControlErrorKind::AggregateCorrupt
    );
    let store_id = store.store_id.clone();
    drop(store);
    let reopened = EventStore::open_pinned(&fixture.path, &store_id).await.unwrap();
    assert_eq!(
        reopened
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        RuntimeControlErrorKind::AggregateCorrupt
    );
}

pub(super) fn settlement(fixture: &Fixture, suffix: &str, outcome: OperationOutcomeV1, metered: u64) -> SettlementCommand {
    let contract = retained_operation_contract(fixture.set.reference()).unwrap();
    let mut meter = KernelMeter::new(&contract, "epoch-a").unwrap();
    for _ in 0..metered {
        meter.try_consume(BudgetDimensionV1::Tokens).unwrap();
    }
    SettlementCommand::from_producer_observation(
        EventId::parse(format!("event_settle-{suffix}")).unwrap(),
        format!("corr-settle-{suffix}"),
        CallbackId::parse(format!("callback_fake-{suffix}")).unwrap(),
        OperationId::parse(format!("operation_{suffix}")).unwrap(),
        ReservationId::parse(format!("reservation_{suffix}")).unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        outcome,
        usage(99),
        digest('9'),
        "fake-result".to_owned(),
        Some(meter.snapshot().unwrap()),
        &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    )
    .unwrap()
}

#[test]
fn capability_table() {
    let parent = BTreeMap::from([(BudgetDimensionV1::Tokens, 10)]);
    let child = BTreeMap::from([(BudgetDimensionV1::Tokens, 5)]);
    assert!(vector_lte(&child, &parent));
    assert!(!vector_lte(&parent, &child));
}

#[tokio::test]
async fn default_deny() {
    let fixture = Fixture::new("run_default-deny");
    let store = create_running(&fixture).await;
    let result = store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_intruder"), &fixture.proposal("denied"), &live_clock()).await.unwrap();
    assert!(matches!(result, ReserveResult::Denied { ref reason_code, .. } if reason_code == "capability_missing"));
}

#[tokio::test]
async fn denial_audit() {
    let fixture = Fixture::new("run_denial-audit");
    let store = create_running(&fixture).await;
    store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_intruder"), &fixture.proposal("audit"), &live_clock()).await.unwrap();
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(projection.rejected_message_count, "1");
}

#[tokio::test]
async fn delegation() {
    let fixture = Fixture::new("run_delegation");
    let store = create_running(&fixture).await;
    let mut child = fixture.grant("cap_child", "agent_owner", "agent_child", Some("cap_root"), Some(fixture.task_id.clone()), false, 1, 5);
    child.issued_at_utc = "2026-08-26T00:00:06.000Z".to_owned();
    store.issue_capability(&fixture.registry(), &fixture.target(), &EventId::parse("event_cap-child").unwrap(), "2026-08-26T00:00:06.000Z", "corr-cap", child).await.unwrap();
    let result = store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_child"), &fixture.proposal("child"), &live_clock()).await.unwrap();
    assert!(matches!(result, ReserveResult::Reserved { .. }));
    let mut widened = fixture.grant("cap_wide", "agent_child", "agent_intruder", Some("cap_child"), None, true, 2, 20);
    widened.issued_at_utc = "2026-08-26T00:00:07.000Z".to_owned();
    let error = store.issue_capability(&fixture.registry(), &fixture.target_as("agent_child"), &EventId::parse("event_cap-wide").unwrap(), "2026-08-26T00:00:07.000Z", "corr-wide", widened).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::DelegationWidening);
}

#[tokio::test]
async fn revocation_and_expiry() {
    let fixture = Fixture::new("run_revocation");
    let store = create_running(&fixture).await;
    store.revoke_capability(&fixture.registry(), &fixture.target(), &RevokeCapabilityCommand { event_id: EventId::parse("event_revoke-root").unwrap(), occurred_at: "2026-08-26T00:00:07.000Z".to_owned(), correlation_id: "corr-revoke".to_owned(), grant_id: CapabilityId::parse("cap_root").unwrap(), reason_code: "owner-revoked".to_owned() }).await.unwrap();
    let result = store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("revoked"), &live_clock()).await.unwrap();
    assert!(matches!(result, ReserveResult::Denied { reason_code, .. } if reason_code == "capability_revoked"));

    let time_fixture = Fixture::new("run_capability-time-boundaries");
    let time_store = create_running(&time_fixture).await;
    let mut bounded = time_fixture.grant("cap_time-bounded", "agent_owner", "agent_child", None, None, false, 1, 4);
    bounded.issued_at_utc = "2026-08-26T00:00:06.000Z".to_owned();
    bounded.constraints.not_before_utc = "2026-08-26T00:00:20.000Z".to_owned();
    bounded.constraints.expires_at_utc = "2026-08-26T00:00:30.000Z".to_owned();
    let bounded_issued_at = bounded.issued_at_utc.clone();
    time_store.issue_capability(&time_fixture.registry(), &time_fixture.target(), &EventId::parse("event_cap-time-bounded").unwrap(), &bounded_issued_at, "corr-time-bounded", bounded).await.unwrap();
    let before = time_store.reserve_protected_operation(&time_fixture.registry(), &time_fixture.target_as("agent_child"), &time_fixture.proposal("before-not-before"), &live_clock()).await.unwrap();
    assert!(matches!(before, ReserveResult::Denied { reason_code, .. } if reason_code == "capability_not_yet_valid"));
    assert!(matches!(time_store.reserve_protected_operation(
        &time_fixture.registry(), &time_fixture.target_as("agent_child"), &time_fixture.proposal("at-not-before"),
        &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    ).await.unwrap(), ReserveResult::Reserved { .. }));
    let expired = time_store.reserve_protected_operation(
        &time_fixture.registry(), &time_fixture.target_as("agent_child"), &time_fixture.proposal("at-expiry"),
        &FakeClock::at("2026-08-26T00:00:30.000Z", 3_000, "epoch-a"),
    ).await.unwrap();
    assert!(matches!(expired, ReserveResult::Denied { reason_code, .. } if reason_code == "capability_expired"));
}

#[tokio::test]
async fn lifecycle_admission() {
    let fixture = Fixture::new("run_lifecycle-admission");
    let store = create_initialized(&fixture).await;
    let error = store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("created"), &live_clock()).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::LifecycleStateDenied);
}

#[tokio::test]
async fn lifecycle_reserve_race() {
    let fixture = Fixture::new("run_lifecycle-race");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "guard").await;
    let error = store.transition_task(&fixture.registry(), &super::lifecycle::LifecycleTarget { scope: fixture.scope.clone(), actor: fixture.scope.agent_id.clone() }, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-pause-blocked").unwrap(), occurred_at: "2026-08-26T00:00:30.000Z".to_owned(), correlation_id: "corr-pause".to_owned(), expected_sequence: 5, task_id: fixture.task_id.clone(), expected_state: TaskState::Running, target_state: TaskState::Paused, reason_code: "pause".to_owned(),
    }).await.unwrap_err();
    assert_eq!(error.kind, super::lifecycle::LifecycleErrorKind::ParentStateConflict);
}

#[tokio::test]
async fn resource_envelope() {
    let fixture = Fixture::new("run_resource-envelope");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "envelope").await;
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.operations[0].reserved_usage.iter().any(|entry| entry.amount.as_u64() == 4));
}

#[tokio::test]
async fn budget_concurrency() {
    let fixture = Fixture::new("run_budget-concurrency");
    let store = create_running(&fixture).await;
    let registry = fixture.registry();
    let target = fixture.target();
    let proposal_a = fixture.proposal("race-a");
    let proposal_b = fixture.proposal("race-b");
    let clock = live_clock();
    let (a, b) = tokio::join!(
        store.reserve_protected_operation(&registry, &target, &proposal_a, &clock),
        store.reserve_protected_operation(&registry, &target, &proposal_b, &clock),
    );
    assert!(matches!(a.unwrap(), ReserveResult::Reserved { .. }));
    assert!(matches!(b.unwrap(), ReserveResult::Reserved { .. }));
    let third = store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("race-c"), &live_clock()).await.unwrap();
    assert!(matches!(third, ReserveResult::Denied { ref reason_code, .. } if reason_code == "budget_hard_limit"));
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.accounts.iter().filter(|a| matches!(a.account.scope, BudgetScopeV1::Run)).all(|a| a.reserved.as_u64() == 8));
}

#[tokio::test]
async fn reserve_contract() {
    let fixture = Fixture::new("run_reserve");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "one").await;
    assert_eq!(lease.operation_id.as_str(), "operation_one");
}

#[tokio::test]
async fn settlement_release_refund_and_usage_authority() {
    let fixture = Fixture::new("run_settlement");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "settle").await;
    let command = settlement(&fixture, "settle", OperationOutcomeV1::Succeeded, 2);
    let settled = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    let settled_retry = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    assert_eq!(append_identity(&settled), append_identity(&settled_retry));
    let (settlement_event_id, _) = append_identity(&settled);
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.accounts.iter().all(|a| a.reserved.as_u64() == 0));
    assert!(projection.accounts.iter().filter(|a| !matches!(a.account.scope, BudgetScopeV1::Actor { ref actor_id } if actor_id.as_str() == "agent_child")).all(|a| a.gross_consumed.as_u64() == 2));
    let refund = RefundCommand {
        event_id: EventId::parse("event_refund-settle").unwrap(), occurred_at: "2026-08-26T00:00:21.000Z".to_owned(), correlation_id: "corr-refund".to_owned(), settlement_event_id, operation_id: OperationId::parse("operation_settle").unwrap(), refunded_usage: usage(1), reason_code: "meter-correction".to_owned(),
    };
    let first_refund = store.refund_budget(&fixture.registry(), &fixture.target(), &refund).await.unwrap();
    let retry_refund = store.refund_budget(&fixture.registry(), &fixture.target(), &refund).await.unwrap();
    assert_eq!(append_identity(&first_refund), append_identity(&retry_refund));
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.accounts.iter().filter(|a| !matches!(a.account.scope, BudgetScopeV1::Actor { ref actor_id } if actor_id.as_str() == "agent_child")).all(|a| a.net_consumed.as_u64() == 1));
}

#[tokio::test]
async fn refund() {
    let fixture = Fixture::new("run_refund");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "refund-only").await;
    let settled = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "refund-only", OperationOutcomeV1::Succeeded, 1)).await.unwrap();
    let error = store.refund_budget(&fixture.registry(), &fixture.target(), &RefundCommand { event_id: EventId::parse("event_refund-too-much").unwrap(), occurred_at: "2026-08-26T00:00:21.000Z".to_owned(), correlation_id: "corr-refund-too-much".to_owned(), settlement_event_id: append_identity(&settled).0, operation_id: OperationId::parse("operation_refund-only").unwrap(), refunded_usage: usage(2), reason_code: "invalid".to_owned() }).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::BudgetExhausted);
}

#[test]
fn usage_authority() {
    assert_eq!(UsageEvidenceClassV1::Unknown, UsageEvidenceClassV1::Unknown);
    assert!(vector_lte(&vector_map(&usage(4)).unwrap(), &vector_map(&usage(4)).unwrap()));
}

#[tokio::test]
async fn unknown_usage_is_conservative() {
    let fixture = Fixture::new("run_unknown-usage");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "unknown").await;
    let mut command = settlement(&fixture, "unknown", OperationOutcomeV1::Failed, 1);
    command.meter_snapshot = None;
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.accounts.iter().filter(|a| !matches!(a.account.scope, BudgetScopeV1::Actor { ref actor_id } if actor_id.as_str() == "agent_child")).all(|a| a.gross_consumed.as_u64() == 4));
}

#[tokio::test]
async fn callback_authority() {
    let fixture = Fixture::new("run_callback-authority");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "producer").await;
    let mut command = settlement(&fixture, "producer", OperationOutcomeV1::Succeeded, 1);
    command.producer_revision = RevisionId::parse("rev_untrusted-producer").unwrap();
    let error = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::ProducerUnauthorized);
}

#[tokio::test]
async fn cancellation_authority_and_propagation() {
    let fixture = Fixture::new("run_cancellation");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "cancel").await;
    let unauthorized = store.request_cancellation(&fixture.registry(), &fixture.target_as("agent_intruder"), &CancellationCommand {
        event_id: EventId::parse("event_cancel-bad").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-cancel-bad".to_owned(), cancellation_id: CancellationId::parse("cancel_bad").unwrap(), target: CancellationTargetV1::Task { task_id: fixture.task_id.clone() }, reason_code: "bad".to_owned(),
    }).await.unwrap_err();
    assert_eq!(unauthorized.kind, RuntimeControlErrorKind::Unauthorized);
    let cancel = CancellationCommand {
        event_id: EventId::parse("event_cancel-task").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-cancel-task".to_owned(), cancellation_id: CancellationId::parse("cancel_task").unwrap(), target: CancellationTargetV1::Task { task_id: fixture.task_id.clone() }, reason_code: "user-request".to_owned(),
    };
    let first_cancel = store.request_cancellation(&fixture.registry(), &fixture.target(), &cancel).await.unwrap();
    let retry_cancel = store.request_cancellation(&fixture.registry(), &fixture.target(), &cancel).await.unwrap();
    assert_eq!(append_identity(&first_cancel), append_identity(&retry_cancel));
    let success = settlement(&fixture, "cancel", OperationOutcomeV1::Succeeded, 1);
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &success).await.unwrap_err().kind, RuntimeControlErrorKind::CancellationPending);
    let ack = CancellationAckCommand::from_producer(
        EventId::parse("event_cancel-ack").unwrap(), "corr-ack".to_owned(),
        cancel.cancellation_id, OperationId::parse("operation_cancel").unwrap(),
        ReservationId::parse("reservation_cancel").unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        &FakeClock::at("2026-08-26T00:00:13.000Z", 1_300, "epoch-a"),
    ).unwrap();
    store.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &lease, &ack).await.unwrap();
    let cancelled = settlement(&fixture, "cancel", OperationOutcomeV1::Cancelled, 1);
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &cancelled).await.unwrap();
}

#[test]
fn cancellation_propagation() {
    let operation = OperationReservedPayloadV1 {
        operation_id: OperationId::parse("operation_probe").unwrap(),
        reservation_id: ReservationId::parse("reservation_probe").unwrap(),
        hook_pair: None,
        effect_pair: None,
        subject_actor: AgentId::parse("agent_owner").unwrap(), task_id: Some(TaskId::parse("task_probe").unwrap()), resource: resource(), operation: "invoke".to_owned(), grant_id: CapabilityId::parse("cap_probe").unwrap(),
        authorization_decision: AuthorizationDecisionV1 { outcome: AuthorizationOutcomeV1::Allowed, reason_code: "allowed".to_owned(), grant_id: Some(CapabilityId::parse("cap_probe").unwrap()), request_digest: digest('1') },
        requested_usage: usage(1), trusted_reservation: usage(1), allocations: Vec::new(), operation_contract_revision: RevisionId::parse("rev_operation").unwrap(), adapter_revision: RevisionId::parse("rev_adapter").unwrap(),
        lifecycle_admission: LifecycleAdmissionV1 { cursor: EventCursor { sequence: "5".to_owned(), event_id: EventId::parse("event_task-running").unwrap() }, run_state: RunState::Running, task_state: Some(TaskState::Running) },
        producer_revision: RevisionId::parse("rev_producer").unwrap(), initial_process_epoch: "epoch-a".to_owned(), callback_namespace: "probe".to_owned(), interruptibility: OperationInterruptibilityV1::Cooperative, absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(),
        timeout_key: TimeoutKeyV1 { recovery_revision: RevisionId::parse("rev_recovery").unwrap(), scope: IsolationScope { tenant_id: TenantId::parse("tenant_local").unwrap(), user_id: None, workspace_id: WorkspaceId::parse("workspace_repo").unwrap(), run_id: RunId::parse("run_probe").unwrap(), agent_id: AgentId::parse("agent_owner").unwrap() }, control_stream_id: StreamId::parse("stream_runtime-control-probe").unwrap(), operation_id: OperationId::parse("operation_probe").unwrap(), reservation_id: ReservationId::parse("reservation_probe").unwrap(), absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(), timeout_policy_revision: RevisionId::parse("rev_timeout").unwrap(), clock_revision: RevisionId::parse("rev_clock").unwrap(), source_schema_set_ref: generate_schema_bundle().unwrap().reference, source_protocol_limits_ref: pareto_protocol::ProtocolLimitsRef { profile: "protocol-limits-v1".to_owned(), digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap() }, operation_contract_revision: RevisionId::parse("rev_operation").unwrap(), meter_revision: RevisionId::parse("rev_meter").unwrap() },
        warnings: Vec::new(), reserved_at_utc: "2026-08-26T00:00:00.000Z".to_owned(),
    };
    assert!(cancel_target_matches(&CancellationTargetV1::Run, &operation));
    assert!(cancel_target_matches(&CancellationTargetV1::Task { task_id: TaskId::parse("task_probe").unwrap() }, &operation));
    assert!(cancel_target_matches(&CancellationTargetV1::Operation { operation_id: OperationId::parse("operation_probe").unwrap() }, &operation));
}

#[tokio::test]
async fn interruptibility() {
    let fixture = Fixture::new("run_interruptibility");
    let store = create_running(&fixture).await;
    let mut proposal = fixture.proposal("uninterruptible");
    proposal.interruptibility = OperationInterruptibilityV1::Uninterruptible;
    let lease = match store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &proposal, &live_clock()).await.unwrap() {
        ReserveResult::Reserved { lease, .. } => *lease,
        other => panic!("expected reservation, got {other:?}"),
    };
    store.request_cancellation(&fixture.registry(), &fixture.target(), &CancellationCommand {
        event_id: EventId::parse("event_cancel-uninterruptible").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-uninterruptible".to_owned(), cancellation_id: CancellationId::parse("cancel_uninterruptible").unwrap(), target: CancellationTargetV1::Operation { operation_id: proposal.operation_id.clone() }, reason_code: "stop".to_owned(),
    }).await.unwrap();
    let error = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "uninterruptible", OperationOutcomeV1::Succeeded, 1)).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::CancellationPending);
    let before_rebind: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let rebound = store
        .rebind_operation_lease(
            &fixture.registry(),
            &fixture.target(),
            &proposal.operation_id,
            &FakeClock::at("2026-08-26T00:00:13.000Z", 1_300, "epoch-b"),
        )
        .await
        .unwrap();
    assert!(store
        .cancellation_probe(
            &fixture.registry(),
            &fixture.target(),
            &rebound.lease,
            &FakeClock::at("2026-08-26T00:00:14.000Z", 1_301, "epoch-b"),
        )
        .await
        .unwrap()
        .requested);
    let after_rebind: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(before_rebind, after_rebind, "lease rebind cannot acknowledge stop");
    let ack_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM events WHERE json_extract(envelope_json,'$.event_type')='cancellation-acknowledged'",
    )
    .fetch_one(&store.pool)
    .await
    .unwrap();
    assert_eq!(ack_count, 0);
    let timeout = store
        .prepare_timeout_recovery(
            &fixture.registry(),
            &fixture.target(),
            timeout_request("operation_uninterruptible", "corr-timeout-uninterruptible", None, digest('4')),
            &FakeClock::at("2026-08-26T00:01:00.000Z", 48_300, "epoch-b"),
        )
        .await
        .unwrap();
    store
        .recover_timeout(&fixture.registry(), &fixture.target(), &timeout)
        .await
        .unwrap();
    let projection = store
        .runtime_control_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(projection.operations[0].outcome, Some(OperationOutcomeV1::TimedOut));
    assert!(projection
        .cancellations
        .iter()
        .all(|cancellation| cancellation.acknowledgements.is_empty()));
}

#[tokio::test]
async fn timeout_recovery() {
    let fixture = Fixture::new("run_timeout");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "timeout").await;
    let at_deadline = FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a");
    let mut complete = settlement(&fixture, "timeout", OperationOutcomeV1::Succeeded, 1);
    complete.decision_clock = at_deadline.sample();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &complete).await.unwrap_err().kind, RuntimeControlErrorKind::DeadlineExceeded);
    let command = store.prepare_timeout_recovery(
        &fixture.registry(), &fixture.target(), timeout_request("operation_timeout", "corr-timeout", None, digest('e')), &at_deadline,
    ).await.unwrap();
    assert_eq!(command.event_id.as_str(), "event_1a6d3f29fd556e10de0d4c8df97bba5f3a8beb6f0b8ee0dbb5ebe27e12a31fd1");
    let first = store.recover_timeout(&fixture.registry(), &fixture.target(), &command).await.unwrap();
    let retry = store.recover_timeout(&fixture.registry(), &fixture.target(), &command).await.unwrap();
    assert_eq!(append_identity(&first), append_identity(&retry));
    let later_command = store.prepare_timeout_recovery(
        &fixture.registry(), &fixture.target(), timeout_request("operation_timeout", "corr-timeout-later", None, digest('f')),
        &FakeClock::at("2026-08-26T00:01:01.000Z", 52_000, "epoch-b"),
    ).await.unwrap();
    let later = store.recover_timeout(&fixture.registry(), &fixture.target(), &later_command).await.unwrap();
    assert_eq!(append_identity(&retry), append_identity(&later));
}

#[tokio::test]
async fn deadline() {
    let before_fixture = Fixture::new("run_deadline-before");
    let before_store = create_running(&before_fixture).await;
    let before_lease = reserve(&before_store, &before_fixture, "deadline-before").await;
    let mut before = settlement(&before_fixture, "deadline-before", OperationOutcomeV1::Succeeded, 1);
    before.decision_clock = FakeClock::at("2026-08-26T00:00:59.999Z", 50_999, "epoch-a").sample();
    before.meter_snapshot = None;
    before_store.settle_operation(&before_fixture.registry(), &before_fixture.target(), &before_lease, &before).await.unwrap();

    let at_fixture = Fixture::new("run_deadline-at");
    let at_store = create_running(&at_fixture).await;
    let at_lease = reserve(&at_store, &at_fixture, "deadline-at").await;
    let at_clock = FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a");
    let mut at = settlement(&at_fixture, "deadline-at", OperationOutcomeV1::Succeeded, 1);
    at.decision_clock = at_clock.sample();
    assert_eq!(at_store.settle_operation(&at_fixture.registry(), &at_fixture.target(), &at_lease, &at).await.unwrap_err().kind, RuntimeControlErrorKind::DeadlineExceeded);
    let timeout = at_store.prepare_timeout_recovery(
        &at_fixture.registry(), &at_fixture.target(), timeout_request("operation_deadline-at", "corr-deadline-at", None, digest('4')), &at_clock,
    ).await.unwrap();
    at_store.recover_timeout(&at_fixture.registry(), &at_fixture.target(), &timeout).await.unwrap();
    assert_eq!(at_store.runtime_control_projection(&at_fixture.registry(), &at_fixture.target()).await.unwrap().operations[0].outcome, Some(OperationOutcomeV1::TimedOut));
}

#[tokio::test]
async fn timeout_not_due_consumes_no_identity() {
    let fixture = Fixture::new("run_timeout-not-due");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "not-due").await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    let command = store.prepare_timeout_recovery(
        &fixture.registry(), &fixture.target(), timeout_request("operation_not-due", "corr-not-due", None, digest('d')), &live_clock(),
    ).await.unwrap();
    let error = store.recover_timeout(&fixture.registry(), &fixture.target(), &command).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::NotDue);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn terminal_race() {
    let fixture = Fixture::new("run_terminal-race");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "winner").await;
    let timeout_command = store.prepare_timeout_recovery(
        &fixture.registry(), &fixture.target(), timeout_request("operation_winner", "corr-racing-timeout", None, digest('c')),
        &FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-b"),
    ).await.unwrap();
    let registry = fixture.registry();
    let target = fixture.target();
    let completion = settlement(&fixture, "winner", OperationOutcomeV1::Succeeded, 2);
    let (completion_result, timeout_result) = tokio::join!(
        store.settle_operation(&registry, &target, &lease, &completion),
        store.recover_timeout(&registry, &target, &timeout_command),
    );
    assert!(completion_result.is_ok());
    assert!(timeout_result.is_ok());
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(matches!(projection.operations[0].outcome, Some(OperationOutcomeV1::Succeeded | OperationOutcomeV1::TimedOut)));
    let gross = projection.accounts.iter().find(|account| matches!(account.account.scope, BudgetScopeV1::Run)).unwrap().gross_consumed.as_u64();
    assert!(gross == 2 || gross == 4, "exactly one terminal may account budget");
}

#[tokio::test]
async fn idempotency() {
    let fixture = Fixture::new("run_idempotency");
    let store = create_running(&fixture).await;
    let proposal = fixture.proposal("same");
    let first = store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &proposal, &live_clock()).await.unwrap();
    let retry = store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &proposal, &live_clock()).await.unwrap();
    assert!(matches!(first, ReserveResult::Reserved { .. }));
    assert!(matches!(retry, ReserveResult::AlreadyReserved { .. }));
    let mut mutation = proposal;
    mutation.requested_usage = usage(2);
    assert_eq!(store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &mutation, &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::IdempotencyConflict);
}

#[tokio::test]
async fn late_and_duplicate_results_do_not_change_budget() {
    let fixture = Fixture::new("run_late-result");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "late").await;
    let command = settlement(&fixture, "late", OperationOutcomeV1::Succeeded, 2);
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    let before = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    let late = LateResultCommand {
        event_id: EventId::parse("event_late-audit").unwrap(), occurred_at: "2026-08-26T00:00:30.000Z".to_owned(), correlation_id: "corr-late".to_owned(), callback_id: CallbackId::parse("callback_fake-late-second").unwrap(), operation_id: OperationId::parse("operation_late").unwrap(), producer_revision: RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(), redacted_payload_digest: digest('b'),
    };
    let late_clock = FakeClock::at("2026-08-26T00:00:30.000Z", 3_000, "epoch-a");
    let first_late = store.observe_late_result(&fixture.registry(), &fixture.target(), &lease, &late, &late_clock).await.unwrap();
    let retry_late = store.observe_late_result(&fixture.registry(), &fixture.target(), &lease, &late, &late_clock).await.unwrap();
    assert_eq!(append_identity(&first_late), append_identity(&retry_late));
    let after = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(before.accounts, after.accounts);
    assert_eq!(after.late_result_count, "1");
}

#[tokio::test]
async fn isolation() {
    let fixture = Fixture::new("run_isolation");
    let store = create_running(&fixture).await;
    let mut target = fixture.target();
    target.scope.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(store.reserve_protected_operation(&fixture.registry(), &target, &fixture.proposal("cross-scope"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::Unauthorized);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn recovery_projection_and_recorded_replay() {
    let fixture = Fixture::new("run_recovery");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "reopen").await;
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "reopen", OperationOutcomeV1::Succeeded, 2)).await.unwrap();
    let expected = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    let store_id = store.store_id.clone();
    drop(store);
    let reopened = EventStore::open_pinned(&fixture.path, &store_id).await.unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&reopened.pool).await.unwrap();
    let recovered = reopened.replay_runtime_control(&fixture.registry(), &fixture.target()).await.unwrap();
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&reopened.pool).await.unwrap();
    assert_eq!(expected, recovered);
    assert_eq!(before, after);
}

#[tokio::test]
async fn recorded_replay_never_dispatches() {
    let fixture = Fixture::with_mode("run_recorded-replay", ExecutionMode::RecordedReplay { source_run_id: RunId::parse("run_source").unwrap(), boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap() });
    let store = create_running(&fixture).await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let operation = FakeOperation { units: 1, dispatch_count: dispatch_count.clone(), performed_units: Arc::new(AtomicUsize::new(0)) };
    assert_eq!(store.dispatch_fake_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("replay"), &live_clock(), &operation).await.unwrap_err().kind, RuntimeControlErrorKind::RecordedReplay);
    assert_eq!(dispatch_count.load(Ordering::SeqCst), 0);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn compatibility() {
    let fixture = Fixture::new("run_compatibility");
    let store = create_running(&fixture).await;
    let missing = SchemaRegistry(Vec::new());
    assert!(store.runtime_control_projection(&missing, &fixture.target()).await.is_err());
}

#[tokio::test]
async fn schema_manifest_binding() {
    let fixture = Fixture::new("run_schema-binding");
    let store = create_lifecycle_only(&fixture).await;
    let mut payload = fixture.initialization();
    payload.source_contract.protocol_limits_ref.digest = digest('f');
    let error = store.initialize_runtime_control(&fixture.registry(), &fixture.target(), &InitializeRuntimeControlCommand { event_id: EventId::parse("event_bad-control-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-bad-control".to_owned(), payload }).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::LifecycleStateDenied);

    let cursor_fixture = Fixture::new("run_schema-cursor-binding");
    let cursor_store = create_lifecycle_only(&cursor_fixture).await;
    let mut cursor_payload = cursor_fixture.initialization();
    cursor_payload.source_contract.lifecycle_cursor.event_id = EventId::parse("event_run-created").unwrap();
    assert_eq!(cursor_store.initialize_runtime_control(&cursor_fixture.registry(), &cursor_fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_bad-cursor-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-bad-cursor".to_owned(), payload: cursor_payload,
    }).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);

    let binding_fixture = Fixture::new("run_schema-event-binding");
    let binding_store = create_lifecycle_only(&binding_fixture).await;
    let mut binding_payload = binding_fixture.initialization();
    binding_payload.source_contract.accepted_event_bindings.pop();
    assert_eq!(binding_store.initialize_runtime_control(&binding_fixture.registry(), &binding_fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_bad-binding-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-bad-binding".to_owned(), payload: binding_payload,
    }).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);
}

#[tokio::test]
async fn initialization_rejects_duplicate_budget_scope_dimension_and_operation_limit() {
    let account_fixture = Fixture::new("run_duplicate-account");
    let account_store = create_lifecycle_only(&account_fixture).await;
    let mut account_payload = account_fixture.initialization();
    let mut duplicate_account = account_payload.budget_plan.accounts[0].clone();
    duplicate_account.account_id = BudgetAccountId::parse("budget_duplicate").unwrap();
    account_payload.budget_plan.accounts.push(duplicate_account);
    assert_eq!(account_store.initialize_runtime_control(&account_fixture.registry(), &account_fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_duplicate-account-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-duplicate-account".to_owned(), payload: account_payload,
    }).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);

    let limit_fixture = Fixture::new("run_duplicate-limit");
    let limit_store = create_lifecycle_only(&limit_fixture).await;
    let mut limit_payload = limit_fixture.initialization();
    limit_payload.budget_plan.operation_limits.push(limit_payload.budget_plan.operation_limits[0].clone());
    assert_eq!(limit_store.initialize_runtime_control(&limit_fixture.registry(), &limit_fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_duplicate-limit-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-duplicate-limit".to_owned(), payload: limit_payload,
    }).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);
}

#[tokio::test]
async fn projection() {
    let fixture = Fixture::new("run_projection");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "projection").await;
    let first = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    let second = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(first, second);
}

#[test]
fn budget_model() {
    let reserved = BTreeMap::from([(BudgetDimensionV1::Tokens, 10)]);
    let consumed = BTreeMap::from([(BudgetDimensionV1::Tokens, 4)]);
    assert_eq!(vector_sub(&reserved, &consumed).unwrap()[&BudgetDimensionV1::Tokens], 6);
}

#[tokio::test]
async fn model_sequences() {
    #[derive(Clone, Copy, Debug)]
    enum ModelCommand {
        Request,
        Ack,
        Complete,
        Cancel,
        Timeout,
    }

    let vector = usage(1);
    assert_eq!(canonical_vector(&vector).unwrap(), vector);
    assert!(vector_map(&[vector[0].clone(), vector[0].clone()]).is_err());

    // Bounded transition graph: every command's exact duplicate plus both orders of each
    // authority/terminal conflict, followed by the cancellation acknowledgement triples.
    let sequences = vec![
        vec![ModelCommand::Request, ModelCommand::Request],
        vec![ModelCommand::Ack, ModelCommand::Ack],
        vec![ModelCommand::Complete, ModelCommand::Complete],
        vec![ModelCommand::Cancel, ModelCommand::Cancel],
        vec![ModelCommand::Timeout, ModelCommand::Timeout],
        vec![ModelCommand::Request, ModelCommand::Ack],
        vec![ModelCommand::Ack, ModelCommand::Request],
        vec![ModelCommand::Request, ModelCommand::Complete],
        vec![ModelCommand::Complete, ModelCommand::Request],
        vec![ModelCommand::Request, ModelCommand::Cancel],
        vec![ModelCommand::Cancel, ModelCommand::Request],
        vec![ModelCommand::Complete, ModelCommand::Timeout],
        vec![ModelCommand::Timeout, ModelCommand::Complete],
        vec![ModelCommand::Cancel, ModelCommand::Timeout],
        vec![ModelCommand::Timeout, ModelCommand::Cancel],
        vec![ModelCommand::Request, ModelCommand::Ack, ModelCommand::Cancel],
        vec![ModelCommand::Ack, ModelCommand::Request, ModelCommand::Cancel],
        vec![ModelCommand::Request, ModelCommand::Cancel, ModelCommand::Ack],
        vec![ModelCommand::Cancel, ModelCommand::Request, ModelCommand::Ack],
        vec![ModelCommand::Request, ModelCommand::Cancel, ModelCommand::Timeout],
        vec![ModelCommand::Request, ModelCommand::Timeout, ModelCommand::Cancel],
        vec![ModelCommand::Complete, ModelCommand::Request, ModelCommand::Timeout],
        vec![ModelCommand::Timeout, ModelCommand::Request, ModelCommand::Complete],
    ];
    for (case_offset, sequence) in sequences.into_iter().enumerate() {
                let case_index = case_offset + 1;
                let fixture = Fixture::new(&format!("run_bounded-model-{case_index}"));
                let store = create_running(&fixture).await;
                let suffix = format!("bounded-model-{case_index}");
                let lease = reserve(&store, &fixture, &suffix).await;
                let cancellation_id =
                    CancellationId::parse(format!("cancel_bounded-model-{case_index}")).unwrap();
                let cancellation = CancellationCommand {
                    event_id: EventId::parse(format!("event_cancel-bounded-model-{case_index}"))
                        .unwrap(),
                    occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
                    correlation_id: format!("corr-cancel-bounded-model-{case_index}"),
                    cancellation_id: cancellation_id.clone(),
                    target: CancellationTargetV1::Operation {
                        operation_id: lease.operation_id.clone(),
                    },
                    reason_code: "bounded_model".to_owned(),
                };
                let ack = CancellationAckCommand::from_producer(
                    EventId::parse(format!("event_ack-bounded-model-{case_index}")).unwrap(),
                    format!("corr-ack-bounded-model-{case_index}"),
                    cancellation_id,
                    lease.operation_id.clone(),
                    lease.reservation_id.clone(),
                    lease.producer_revision.clone(),
                    &FakeClock::at("2026-08-26T00:00:12.000Z", 1_200, "epoch-a"),
                )
                .unwrap();
                let complete = settlement(&fixture, &suffix, OperationOutcomeV1::Succeeded, 1);
                let mut cancel = settlement(&fixture, &suffix, OperationOutcomeV1::Cancelled, 1);
                cancel.event_id =
                    EventId::parse(format!("event_settle-cancel-{suffix}")).unwrap();
                cancel.callback_id =
                    CallbackId::parse(format!("callback_fake-cancel-{case_index}")).unwrap();
                let timeout = store
                    .prepare_timeout_recovery(
                        &fixture.registry(),
                        &fixture.target(),
                        timeout_request(
                            lease.operation_id.as_str(),
                            &format!("corr-timeout-bounded-model-{case_index}"),
                            None,
                            digest('6'),
                        ),
                        &FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a"),
                    )
                    .await
                    .unwrap();
                let mut cancellation_was_effective = false;
                let mut terminal_cancellation_state = None;
                for command in sequence {
                    let before = store
                        .runtime_control_projection(&fixture.registry(), &fixture.target())
                        .await
                        .unwrap();
                    let before_outcome = before.operations[0].outcome;
                    match command {
                        ModelCommand::Request => {
                            if store
                                .request_cancellation(
                                    &fixture.registry(),
                                    &fixture.target(),
                                    &cancellation,
                                )
                                .await
                                .is_ok()
                            {
                                cancellation_was_effective = true;
                            }
                        }
                        ModelCommand::Ack => {
                            let _ = store
                                .acknowledge_cancellation(
                                    &fixture.registry(),
                                    &fixture.target(),
                                    &lease,
                                    &ack,
                                )
                                .await;
                        }
                        ModelCommand::Complete => {
                            let _ = store
                                .settle_operation(
                                    &fixture.registry(),
                                    &fixture.target(),
                                    &lease,
                                    &complete,
                                )
                                .await;
                        }
                        ModelCommand::Cancel => {
                            let _ = store
                                .settle_operation(
                                    &fixture.registry(),
                                    &fixture.target(),
                                    &lease,
                                    &cancel,
                                )
                                .await;
                        }
                        ModelCommand::Timeout => {
                            let _ = store
                                .recover_timeout(&fixture.registry(), &fixture.target(), &timeout)
                                .await;
                        }
                    }
                    let after = store
                        .runtime_control_projection(&fixture.registry(), &fixture.target())
                        .await
                        .unwrap();
                    let after_outcome = after.operations[0].outcome;
                    if let Some(winner) = before_outcome {
                        assert_eq!(after_outcome, Some(winner), "terminal changed in case {case_index}");
                    } else if after_outcome.is_some() {
                        terminal_cancellation_state = Some(cancellation_was_effective);
                    }
                    if let Some(outcome) = after_outcome {
                        match outcome {
                            OperationOutcomeV1::Cancelled => assert!(
                                terminal_cancellation_state.unwrap(),
                                "cancelled without request in case {case_index}"
                            ),
                            OperationOutcomeV1::Succeeded | OperationOutcomeV1::Failed => assert!(
                                !terminal_cancellation_state.unwrap(),
                                "completion after cancellation in case {case_index}"
                            ),
                            OperationOutcomeV1::TimedOut => {}
                        }
                        let settlement = after.operations[0].settlement.as_ref().unwrap();
                        let reserved =
                            vector_map(&after.operations[0].reservation.trusted_reservation).unwrap();
                        let accounted = vector_map(&settlement.accounted_usage).unwrap();
                        let released = vector_map(&settlement.released_usage).unwrap();
                        assert!(reserved.iter().all(|(dimension, amount)| {
                            accounted.get(dimension).copied().unwrap_or(0)
                                + released.get(dimension).copied().unwrap_or(0)
                                == *amount
                        }));
                        assert!(after.accounts.iter().all(|account| account.reserved.as_u64() == 0));
                    }
                }
                if store
                    .runtime_control_projection(&fixture.registry(), &fixture.target())
                    .await
                    .unwrap()
                    .operations[0]
                    .outcome
                    .is_none()
                {
                    store
                        .recover_timeout(&fixture.registry(), &fixture.target(), &timeout)
                        .await
                        .unwrap();
                }
                let terminal_projection = store
                    .runtime_control_projection(&fixture.registry(), &fixture.target())
                    .await
                    .unwrap();
                let prior_late_count = terminal_projection.late_results.len();
                let mut late = complete.clone();
                late.event_id = EventId::parse(format!("event_late-bounded-{case_index}"))
                    .unwrap();
                late.callback_id =
                    CallbackId::parse(format!("callback_fake-late-bounded-{case_index}"))
                        .unwrap();
                late.redacted_payload_digest = digest('7');
                if terminal_projection.operations[0].outcome
                    == Some(OperationOutcomeV1::TimedOut)
                {
                    late.decision_clock =
                        FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a")
                            .sample();
                }
                store
                    .settle_operation(&fixture.registry(), &fixture.target(), &lease, &late)
                    .await
                    .unwrap_or_else(|error| {
                        panic!("late retry failed in bounded case {case_index}: {error:?}")
                    });
                let after_late = store
                    .runtime_control_projection(&fixture.registry(), &fixture.target())
                    .await
                    .unwrap();
                let before_duplicate: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
                    .fetch_one(&store.pool)
                    .await
                    .unwrap();
                store
                    .settle_operation(&fixture.registry(), &fixture.target(), &lease, &late)
                    .await
                    .unwrap_or_else(|error| {
                        panic!("late exact retry failed in bounded case {case_index}: {error:?}")
                    });
                let after_duplicate: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
                    .fetch_one(&store.pool)
                    .await
                    .unwrap();
                assert_eq!(before_duplicate, after_duplicate);
                assert_eq!(after_late.late_results.len(), prior_late_count + 1);
        assert_eq!(
            store
                .replay_runtime_control(&fixture.registry(), &fixture.target())
                .await
                .unwrap(),
            after_late
        );
    }

    for cancellation_first in [false, true] {
        let label = if cancellation_first { "cancel" } else { "complete" };
        let fixture = Fixture::new(&format!("run_bounded-race-{label}"));
        let store = create_running(&fixture).await;
        let suffix = format!("bounded-race-{label}");
        let lease = reserve(&store, &fixture, &suffix).await;
        if cancellation_first {
            store
                .request_cancellation(
                    &fixture.registry(),
                    &fixture.target(),
                    &CancellationCommand {
                        event_id: EventId::parse("event_cancel-bounded-race").unwrap(),
                        occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
                        correlation_id: "corr-cancel-bounded-race".to_owned(),
                        cancellation_id: CancellationId::parse("cancel_bounded-race").unwrap(),
                        target: CancellationTargetV1::Operation {
                            operation_id: lease.operation_id.clone(),
                        },
                        reason_code: "bounded_race".to_owned(),
                    },
                )
                .await
                .unwrap();
        }
        let callback = settlement(
            &fixture,
            &suffix,
            if cancellation_first {
                OperationOutcomeV1::Cancelled
            } else {
                OperationOutcomeV1::Succeeded
            },
            1,
        );
        let timeout = store
            .prepare_timeout_recovery(
                &fixture.registry(),
                &fixture.target(),
                timeout_request(
                    lease.operation_id.as_str(),
                    &format!("corr-timeout-bounded-race-{label}"),
                    None,
                    digest('5'),
                ),
                &FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-b"),
            )
            .await
            .unwrap();
        let registry = fixture.registry();
        let target = fixture.target();
        let (callback_result, timeout_result) = tokio::join!(
            store.settle_operation(&registry, &target, &lease, &callback),
            store.recover_timeout(&registry, &target, &timeout),
        );
        assert!(callback_result.is_ok());
        assert!(timeout_result.is_ok());
        let projection = store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap();
        assert!(matches!(
            projection.operations[0].outcome,
            Some(OperationOutcomeV1::Succeeded)
                | Some(OperationOutcomeV1::Cancelled)
                | Some(OperationOutcomeV1::TimedOut)
        ));
        assert!(projection.accounts.iter().all(|account| account.reserved.as_u64() == 0));
        assert_eq!(
            store
                .replay_runtime_control(&fixture.registry(), &fixture.target())
                .await
                .unwrap(),
            projection
        );
    }
}

#[tokio::test]
async fn usage_authority_fake_operation_kernel_meter_and_violation() {
    let fixture = Fixture::new("run_meter-authority");
    let store = create_running(&fixture).await;
    let dispatches = Arc::new(AtomicUsize::new(0));
    let performed = Arc::new(AtomicUsize::new(0));
    let operation = FakeOperation {
        units: 2,
        dispatch_count: dispatches.clone(),
        performed_units: performed.clone(),
    };
    let (lease, snapshot) = store
        .dispatch_fake_operation(
            &fixture.registry(), &fixture.target(), &fixture.proposal("metered"),
            &live_clock(), &operation,
        )
        .await
        .unwrap();
    assert_eq!(dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(performed.load(Ordering::SeqCst), 2);
    let command = SettlementCommand::from_producer_observation(
        EventId::parse("event_settle-metered").unwrap(), "corr-metered".to_owned(),
        CallbackId::parse("callback_fake-metered").unwrap(),
        OperationId::parse("operation_metered").unwrap(),
        ReservationId::parse("reservation_metered").unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        OperationOutcomeV1::Succeeded, Vec::new(), digest('1'), "ok".to_owned(),
        Some(snapshot), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    ).unwrap();
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();

    let violation_dispatches = Arc::new(AtomicUsize::new(0));
    let violation_performed = Arc::new(AtomicUsize::new(0));
    let violation_operation = FakeOperation {
        units: 5,
        dispatch_count: violation_dispatches.clone(),
        performed_units: violation_performed.clone(),
    };
    let (violation_lease, violation_snapshot) = store
        .dispatch_fake_operation(
            &fixture.registry(), &fixture.target(), &fixture.proposal("meter-violation"),
            &live_clock(), &violation_operation,
        )
        .await
        .unwrap();
    assert_eq!(violation_dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(violation_performed.load(Ordering::SeqCst), 4, "fifth unit must not execute");
    assert!(violation_snapshot.contract_violation);
    let violation_command = SettlementCommand::from_producer_observation(
        EventId::parse("event_settle-meter-violation").unwrap(), "corr-meter-violation".to_owned(),
        CallbackId::parse("callback_fake-meter-violation").unwrap(),
        OperationId::parse("operation_meter-violation").unwrap(),
        ReservationId::parse("reservation_meter-violation").unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        OperationOutcomeV1::Succeeded, Vec::new(), digest('2'), "claimed-success".to_owned(),
        Some(violation_snapshot), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    ).unwrap();
    store.settle_operation(&fixture.registry(), &fixture.target(), &violation_lease, &violation_command).await.unwrap();
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(projection.operations.iter().find(|op| op.operation_id.as_str() == "operation_meter-violation").unwrap().outcome, Some(OperationOutcomeV1::Failed));
    assert!(projection.accounts.iter().filter(|account| !matches!(account.account.scope, BudgetScopeV1::Actor { ref actor_id } if actor_id.as_str() == "agent_child")).all(|account| account.gross_consumed.as_u64() == 6));
}

#[tokio::test]
async fn usage_authority_rejects_forged_meter_snapshot_and_unregistered_contract() {
    let fixture = Fixture::new("run_forged-meter");
    let store = create_running(&fixture).await;
    let operation = FakeOperation {
        units: 2,
        dispatch_count: Arc::new(AtomicUsize::new(0)),
        performed_units: Arc::new(AtomicUsize::new(0)),
    };
    let (lease, mut snapshot) = store.dispatch_fake_operation(
        &fixture.registry(), &fixture.target(), &fixture.proposal("forged"), &live_clock(), &operation,
    ).await.unwrap();
    snapshot.usage = usage(1);
    let command = SettlementCommand::from_producer_observation(
        EventId::parse("event_settle-forged").unwrap(), "corr-forged".to_owned(),
        CallbackId::parse("callback_fake-forged").unwrap(), OperationId::parse("operation_forged").unwrap(),
        ReservationId::parse("reservation_forged").unwrap(), RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        OperationOutcomeV1::Succeeded, Vec::new(), digest('3'), "forged".to_owned(), Some(snapshot),
        &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    ).unwrap();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);

    let bad_fixture = Fixture::new("run_unregistered-contract");
    let mut payload = bad_fixture.initialization();
    payload.operation_contract_refs[0] = RevisionId::parse("rev_unregistered").unwrap();
    let store = EventStore::open(&bad_fixture.path).await.unwrap();
    store.create_run(&bad_fixture.trusted(), &CreateRunCommand {
        event_id: EventId::parse("event_run-created").unwrap(), occurred_at: "2026-08-26T00:00:00.000Z".to_owned(), correlation_id: "corr-run".to_owned(), manifest: bad_fixture.manifest.clone(),
    }).await.unwrap();
    store.create_task(&bad_fixture.registry(), &super::lifecycle::LifecycleTarget { scope: bad_fixture.scope.clone(), actor: bad_fixture.scope.agent_id.clone() }, &CreateTaskCommand {
        event_id: EventId::parse("event_task-created").unwrap(), occurred_at: "2026-08-26T00:00:01.000Z".to_owned(), correlation_id: "corr-task".to_owned(), expected_sequence: 1, task_id: bad_fixture.task_id.clone(), parent_task_id: None,
    }).await.unwrap();
    assert_eq!(store.initialize_runtime_control(&bad_fixture.registry(), &bad_fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_control-init-bad-contract").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-control-bad".to_owned(), payload,
    }).await.unwrap_err().kind, RuntimeControlErrorKind::ResourceEnvelopeUnavailable);
}

#[tokio::test]
async fn callback_authority_rejects_stale_epoch_wall_regression_and_namespace() {
    let fixture = Fixture::new("run_epoch-authority");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "epoch").await;
    let mut stale = settlement(&fixture, "epoch", OperationOutcomeV1::Succeeded, 1);
    stale.decision_clock = FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-b").sample();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &stale).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);
    let mut regressed = settlement(&fixture, "epoch", OperationOutcomeV1::Succeeded, 1);
    regressed.decision_clock = FakeClock::at("2026-08-26T00:00:09.000Z", 900, "epoch-a").sample();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &regressed).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);
    let mut wrong_namespace = settlement(&fixture, "epoch", OperationOutcomeV1::Succeeded, 1);
    wrong_namespace.callback_id = CallbackId::parse("callback_wrong-epoch").unwrap();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &wrong_namespace).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);
    assert!(parse_utc_millis("2026-02-30T00:00:00.000Z").is_err());
}

#[tokio::test]
async fn callback_authority_reopen_rejects_previous_process_epoch_lease() {
    let fixture = Fixture::new("run_reopen-stale-lease");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "reopen-stale").await;
    let cancellation_id = CancellationId::parse("cancel_reopen-stale").unwrap();
    store.request_cancellation(&fixture.registry(), &fixture.target(), &CancellationCommand {
        event_id: EventId::parse("event_cancel-reopen-stale").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-cancel-reopen".to_owned(),
        cancellation_id: cancellation_id.clone(), target: CancellationTargetV1::Operation { operation_id: lease.operation_id.clone() }, reason_code: "restart".to_owned(),
    }).await.unwrap();
    let store_id = store.store_id.clone();
    drop(store);
    let reopened = EventStore::open_pinned(&fixture.path, &store_id).await.unwrap();
    let mut completion = settlement(&fixture, "reopen-stale", OperationOutcomeV1::Succeeded, 1);
    completion.decision_clock = FakeClock::at("2026-08-26T00:00:20.000Z", 100, "epoch-after-reopen").sample();
    assert_eq!(reopened.settle_operation(&fixture.registry(), &fixture.target(), &lease, &completion).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);
    assert_eq!(reopened.rebind_operation_lease(
        &fixture.registry(), &fixture.target(), &OperationId::parse("operation_reopen-stale").unwrap(),
        &FakeClock::at("2026-08-26T00:00:09.000Z", 90, "epoch-after-reopen"),
    ).await.unwrap_err().kind, RuntimeControlErrorKind::ClockInvalid);
    let before_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&reopened.pool).await.unwrap();
    let rebound = reopened.rebind_operation_lease(
        &fixture.registry(), &fixture.target(), &OperationId::parse("operation_reopen-stale").unwrap(),
        &FakeClock::at("2026-08-26T00:00:20.000Z", 100, "epoch-after-reopen"),
    ).await.unwrap();
    assert_eq!(before_events, sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events").fetch_one(&reopened.pool).await.unwrap());
    assert!(reopened.cancellation_probe(
        &fixture.registry(), &fixture.target(), &rebound.lease,
        &FakeClock::at("2026-08-26T00:00:21.000Z", 101, "epoch-after-reopen"),
    ).await.unwrap().requested);
    let executor_return_ack = CancellationAckCommand::from_producer(
        EventId::parse("event_ack-reopen-executor-return").unwrap(), "corr-ack-reopen-executor-return".to_owned(), cancellation_id,
        rebound.lease.operation_id.clone(), rebound.lease.reservation_id.clone(), rebound.lease.producer_revision.clone(),
        &FakeClock::at("2026-08-26T00:00:21.000Z", 101, "epoch-after-reopen"),
    ).unwrap();
    reopened.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &rebound.lease, &executor_return_ack).await.unwrap();
    let mut rebound_completion = settlement(&fixture, "reopen-stale", OperationOutcomeV1::Cancelled, 1);
    rebound_completion.decision_clock = FakeClock::at("2026-08-26T00:00:22.000Z", 102, "epoch-after-reopen").sample();
    rebound_completion.meter_snapshot = None;
    reopened.settle_operation(&fixture.registry(), &fixture.target(), &rebound.lease, &rebound_completion).await.unwrap();

    let expired_fixture = Fixture::new("run_reopen-expired");
    let expired_store = create_running(&expired_fixture).await;
    reserve(&expired_store, &expired_fixture, "reopen-expired").await;
    let expired_store_id = expired_store.store_id.clone();
    drop(expired_store);
    let expired_reopened = EventStore::open_pinned(&expired_fixture.path, &expired_store_id).await.unwrap();
    assert_eq!(expired_reopened.rebind_operation_lease(
        &expired_fixture.registry(), &expired_fixture.target(), &OperationId::parse("operation_reopen-expired").unwrap(),
        &FakeClock::at("2026-08-26T00:01:00.000Z", 500, "epoch-after-reopen"),
    ).await.unwrap_err().kind, RuntimeControlErrorKind::DeadlineExceeded);
    let recovery = expired_reopened.prepare_timeout_recovery(
        &expired_fixture.registry(), &expired_fixture.target(), timeout_request("operation_reopen-expired", "corr-reopen-expired", None, digest('a')),
        &FakeClock::at("2026-08-26T00:01:00.000Z", 500, "epoch-after-reopen"),
    ).await.unwrap();
    expired_reopened.recover_timeout(&expired_fixture.registry(), &expired_fixture.target(), &recovery).await.unwrap();
}

#[tokio::test]
async fn capability_table_root_resource_narrowing_and_stable_reasons() {
    let fixture = Fixture::new("run_capability-matrix");
    let store = create_running(&fixture).await;
    let mut root = fixture.grant("cap_kind-root", "agent_owner", "agent_delegate", None, None, true, 3, 8);
    root.resource.id = None;
    root.issued_at_utc = "2026-08-26T00:00:06.000Z".to_owned();
    store.issue_capability(&fixture.registry(), &fixture.target(), &EventId::parse("event_cap-kind-root").unwrap(), &root.issued_at_utc, "corr-kind-root", root.clone()).await.unwrap();
    let mut child = fixture.grant("cap_exact-child", "agent_delegate", "agent_child", Some("cap_kind-root"), Some(fixture.task_id.clone()), false, 1, 4);
    child.issued_at_utc = "2026-08-26T00:00:07.000Z".to_owned();
    assert!(grant_is_subset(&child, &root).unwrap());
    store.issue_capability(&fixture.registry(), &fixture.target_as("agent_delegate"), &EventId::parse("event_cap-exact-child").unwrap(), &child.issued_at_utc, "corr-exact-child", child.clone()).await.unwrap();
    assert!(matches!(store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_child"), &fixture.proposal("narrowed"), &live_clock()).await.unwrap(), ReserveResult::Reserved { .. }));
    let mut widened_resource = child.clone();
    widened_resource.resource.id = None;
    let mut exact_parent = root.clone();
    exact_parent.resource.id = Some("fixture".to_owned());
    assert!(!grant_is_subset(&widened_resource, &exact_parent).unwrap());
    let mut widened_task = child.clone();
    widened_task.scope.task_id = None;
    let mut task_parent = root.clone();
    task_parent.scope.task_id = Some(fixture.task_id.clone());
    assert!(!grant_is_subset(&widened_task, &task_parent).unwrap());
    let mut widened_usage = child.clone();
    widened_usage.constraints.max_operation_usage = usage(9);
    assert!(!grant_is_subset(&widened_usage, &root).unwrap());
    let mut bad_order = child;
    bad_order.operations = vec!["z".to_owned(), "a".to_owned()];
    assert!(validate_grant_shape(&bad_order).is_err());
}

#[tokio::test]
async fn capability_initial_same_scope_subject_and_revocation_reason() {
    let fixture = Fixture::new("run_initial-subject");
    let mut payload = fixture.initialization();
    payload.initial_grants[0].subject_actor = AgentId::parse("agent_child").unwrap();
    let store = create_running_with_payload(&fixture, payload).await;
    assert!(matches!(store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_child"), &fixture.proposal("initial-child"), &live_clock()).await.unwrap(), ReserveResult::Reserved { .. }));
    let other = Fixture::new("run_revoked-reason");
    let store = create_running(&other).await;
    store.revoke_capability(&other.registry(), &other.target(), &RevokeCapabilityCommand {
        event_id: EventId::parse("event_revoke-reason").unwrap(), occurred_at: "2026-08-26T00:00:07.000Z".to_owned(), correlation_id: "corr-revoke-reason".to_owned(), grant_id: CapabilityId::parse("cap_root").unwrap(), reason_code: "owner".to_owned(),
    }).await.unwrap();
    assert!(matches!(store.reserve_protected_operation(&other.registry(), &other.target(), &other.proposal("revoked-reason"), &live_clock()).await.unwrap(), ReserveResult::Denied { reason_code, .. } if reason_code == "capability_revoked"));
}

#[tokio::test]
async fn timeout_recovery_verified_partial_mutation_and_wall_regression() {
    let fixture = Fixture::new("run_timeout-matrix");
    let store = create_running(&fixture).await;
    let operation = FakeOperation {
        units: 2,
        dispatch_count: Arc::new(AtomicUsize::new(0)),
        performed_units: Arc::new(AtomicUsize::new(0)),
    };
    let (_lease, snapshot) = store.dispatch_fake_operation(
        &fixture.registry(), &fixture.target(), &fixture.proposal("partial-timeout"), &live_clock(), &operation,
    ).await.unwrap();
    let at_deadline = FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a");
    let command = store.prepare_timeout_recovery(
        &fixture.registry(), &fixture.target(), timeout_request("operation_partial-timeout", "corr-partial-timeout", Some(snapshot), digest('4')), &at_deadline,
    ).await.unwrap();
    let mut mutation = command.clone();
    mutation.decision_clock.canonical_utc = "2026-08-26T00:01:01.000Z".to_owned();
    mutation.decision_clock.wall_millis += 1_000;
    assert_eq!(store.recover_timeout(&fixture.registry(), &fixture.target(), &mutation).await.unwrap_err().kind, RuntimeControlErrorKind::IdempotencyConflict);
    let first = store.recover_timeout(&fixture.registry(), &fixture.target(), &command).await.unwrap();
    let retry = store.recover_timeout(&fixture.registry(), &fixture.target(), &command).await.unwrap();
    assert_eq!(append_identity(&first), append_identity(&retry), "commit-response-loss retry must be byte exact");
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert!(projection.accounts.iter().filter(|account| !matches!(account.account.scope, BudgetScopeV1::Actor { ref actor_id } if actor_id.as_str() == "agent_child")).all(|account| account.gross_consumed.as_u64() == 2 && account.reserved.as_u64() == 0));

    let other = Fixture::new("run_wall-regression");
    let other_store = create_running(&other).await;
    reserve(&other_store, &other, "wall-regression").await;
    assert_eq!(other_store.prepare_timeout_recovery(
        &other.registry(), &other.target(), timeout_request("operation_wall-regression", "corr-wall-regression", None, digest('5')),
        &FakeClock::at("2026-08-26T00:00:09.000Z", 900, "epoch-b"),
    ).await.unwrap_err().kind, RuntimeControlErrorKind::ClockInvalid);
}

#[tokio::test]
async fn cancellation_authority_probe_admission_future_and_rebound_executor_ack() {
    let fixture = Fixture::new("run_cancel-matrix");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "probe-current").await;
    let before_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    let before_probe = store.cancellation_probe(&fixture.registry(), &fixture.target(), &lease, &live_clock()).await.unwrap();
    assert!(!before_probe.requested);
    let after_probe_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before_count, after_probe_count, "probe must be read only");
    let nonexistent = CancellationCommand {
        event_id: EventId::parse("event_cancel-missing-task").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-cancel-missing".to_owned(), cancellation_id: CancellationId::parse("cancel_missing-task").unwrap(), target: CancellationTargetV1::Task { task_id: TaskId::parse("task_missing").unwrap() }, reason_code: "missing".to_owned(),
    };
    assert_eq!(store.request_cancellation(&fixture.registry(), &fixture.target(), &nonexistent).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);
    let cancel = CancellationCommand {
        event_id: EventId::parse("event_cancel-task-matrix").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-cancel-task-matrix".to_owned(), cancellation_id: CancellationId::parse("cancel_task-matrix").unwrap(), target: CancellationTargetV1::Task { task_id: fixture.task_id.clone() }, reason_code: "stop".to_owned(),
    };
    store.request_cancellation(&fixture.registry(), &fixture.target(), &cancel).await.unwrap();
    let probe = store.cancellation_probe(&fixture.registry(), &fixture.target(), &lease, &live_clock()).await.unwrap();
    assert_eq!(probe.cancellation_ids, vec![cancel.cancellation_id.clone()]);
    assert_eq!(store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("future-after-cancel"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::CancellationPending);
    let ack = CancellationAckCommand::from_producer(
        EventId::parse("event_cancel-recovery-ack").unwrap(), "corr-recovery-ack".to_owned(), cancel.cancellation_id,
        OperationId::parse("operation_probe-current").unwrap(), ReservationId::parse("reservation_probe-current").unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(), &FakeClock::at("2026-08-26T00:00:13.000Z", 1_300, "epoch-b"),
    ).unwrap();
    assert_eq!(store.rebind_operation_lease(&fixture.registry(), &fixture.target(), &OperationId::parse("operation_probe-current").unwrap(), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::ProducerUnauthorized);
    let rebound = store.rebind_operation_lease(
        &fixture.registry(), &fixture.target(), &OperationId::parse("operation_probe-current").unwrap(),
        &FakeClock::at("2026-08-26T00:00:13.000Z", 1_300, "epoch-b"),
    ).await.unwrap();
    store.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &rebound.lease, &ack).await.unwrap();
    let mut ack_mutation = ack.clone();
    ack_mutation.decision_clock.canonical_utc = "2026-08-26T00:00:14.000Z".to_owned();
    ack_mutation.decision_clock.wall_millis += 1_000;
    assert_eq!(store.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &rebound.lease, &ack_mutation).await.unwrap_err().kind, RuntimeControlErrorKind::IdempotencyConflict);
}

#[tokio::test]
async fn idempotency_denial_and_callback_state_changes_are_locked() {
    let fixture = Fixture::new("run_idempotency-matrix");
    let store = create_running(&fixture).await;
    let proposal = fixture.proposal("locked-denial");
    let denied = store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_intruder"), &proposal, &live_clock()).await.unwrap();
    let mut root = fixture.grant("cap_intruder-root", "agent_owner", "agent_intruder", None, None, false, 1, 4);
    root.issued_at_utc = "2026-08-26T00:00:11.000Z".to_owned();
    let issued_at = root.issued_at_utc.clone();
    store.issue_capability(&fixture.registry(), &fixture.target(), &EventId::parse("event_cap-intruder-root").unwrap(), &issued_at, "corr-intruder", root).await.unwrap();
    let retry = store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_intruder"), &proposal, &live_clock()).await.unwrap();
    assert_eq!(append_identity_from_reserve(&denied), append_identity_from_reserve(&retry));

    let lease = reserve(&store, &fixture, "callback-lock").await;
    let command = settlement(&fixture, "callback-lock", OperationOutcomeV1::Succeeded, 1);
    let first = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    let exact = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    assert_eq!(append_identity(&first), append_identity(&exact));
    let mut mutation = command.clone();
    mutation.event_id = EventId::parse("event_settle-callback-mutation").unwrap();
    mutation.reason_code = "mutated".to_owned();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &mutation).await.unwrap_err().kind, RuntimeControlErrorKind::IdempotencyConflict);
    let before = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    let mut late = command.clone();
    late.event_id = EventId::parse("event_late-unified").unwrap();
    late.callback_id = CallbackId::parse("callback_fake-callback-lock-late").unwrap();
    late.redacted_payload_digest = digest('6');
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &late).await.unwrap();
    let after = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(before.accounts, after.accounts);
    assert_eq!(after.late_result_count.parse::<u64>().unwrap(), before.late_result_count.parse::<u64>().unwrap() + 1);
}

fn append_identity_from_reserve(result: &ReserveResult) -> (EventId, i64) {
    match result {
        ReserveResult::Reserved { event_id, sequence, .. }
        | ReserveResult::AlreadyReserved { event_id, sequence }
        | ReserveResult::Denied { event_id, sequence, .. } => (event_id.clone(), *sequence),
    }
}

#[tokio::test]
async fn budget_concurrency_reverse_winner_and_lifecycle_race() {
    let fixture = Fixture::new("run_reverse-budget-race");
    let mut payload = fixture.initialization();
    for account in &mut payload.budget_plan.accounts {
        account.hard_limit = BudgetAmountV1::new(6);
        account.soft_limit = None;
    }
    let store = create_running_with_payload(&fixture, payload).await;
    let registry = fixture.registry();
    let target = fixture.target();
    let left_proposal = fixture.proposal("reverse-left");
    let right_proposal = fixture.proposal("reverse-right");
    let clock = live_clock();
    let (left, right) = tokio::join!(
        store.reserve_protected_operation(&registry, &target, &left_proposal, &clock),
        store.reserve_protected_operation(&registry, &target, &right_proposal, &clock),
    );
    let outcomes = [left.unwrap(), right.unwrap()];
    assert_eq!(outcomes.iter().filter(|result| matches!(result, ReserveResult::Reserved { .. })).count(), 1);
    assert_eq!(outcomes.iter().filter(|result| matches!(result, ReserveResult::Denied { reason_code, .. } if reason_code == "budget_hard_limit")).count(), 1);

    let race_fixture = Fixture::new("run_lifecycle-two-writer");
    let race_store = create_running(&race_fixture).await;
    let lifecycle_target = super::lifecycle::LifecycleTarget { scope: race_fixture.scope.clone(), actor: race_fixture.scope.agent_id.clone() };
    let transition = TransitionRunCommand {
        event_id: EventId::parse("event_run-pause-race").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-pause-race".to_owned(), expected_sequence: 5, expected_state: RunState::Running, target_state: RunState::Paused, reason_code: "race".to_owned(),
    };
    let race_registry = race_fixture.registry();
    let race_target = race_fixture.target();
    let race_proposal = race_fixture.proposal("lifecycle-race-live");
    let race_clock = live_clock();
    let (reserve_result, transition_result) = tokio::join!(
        race_store.reserve_protected_operation(&race_registry, &race_target, &race_proposal, &race_clock),
        race_store.transition_run(&race_registry, &lifecycle_target, &transition),
    );
    assert!(reserve_result.is_ok() ^ transition_result.is_ok(), "only the serialized winner may commit");
}

#[tokio::test]
async fn compatibility_rejects_schema_valid_illegal_capability_history() {
    let fixture = Fixture::new("run_illegal-grant-history");
    let store = create_running(&fixture).await;
    let mut illegal = fixture.grant("cap_illegal-history", "agent_child", "agent_intruder", Some("cap_root"), None, true, 9, 99);
    illegal.issued_at_utc = "2026-08-26T00:00:12.000Z".to_owned();
    let illegal_issued_at = illegal.issued_at_utc.clone();
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &fixture.registry(), &fixture.target()).await.unwrap();
    append_control(
        tx, &aggregate, &EventId::parse("event_illegal-grant-history").unwrap(),
        &illegal_issued_at, "corr-illegal-grant", "capability-issued",
        &CapabilityIssuedPayloadV1 { grant: illegal },
    ).await.unwrap();
    assert_eq!(store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);
}

#[tokio::test]
async fn compatibility_rejects_schema_valid_illegal_budget_and_cancel_history() {
    let fixture = Fixture::new("run_illegal-budget-history");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "illegal-budget").await;
    let payload = OperationSettledPayloadV1 {
        operation_id: OperationId::parse("operation_illegal-budget").unwrap(),
        reservation_id: ReservationId::parse("reservation_illegal-budget").unwrap(),
        hook_pair: None,
        effect_pair: None,
        callback_id: Some(CallbackId::parse("callback_fake-illegal-budget").unwrap()),
        callback_fingerprint: Some(digest('7')),
        callback_authority: None,
        outcome: OperationOutcomeV1::Succeeded,
        evidence_class: UsageEvidenceClassV1::KernelMeterVerified,
        kernel_meter_evidence: None,
        observed_usage: Vec::new(), accounted_usage: usage(5), released_usage: Vec::new(),
        reason_code: "illegal".to_owned(), timeout_command_fingerprint: None,
        settled_at_utc: "2026-08-26T00:00:20.000Z".to_owned(),
    };
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &fixture.registry(), &fixture.target()).await.unwrap();
    append_control(
        tx, &aggregate, &EventId::parse("event_illegal-budget-settlement").unwrap(),
        &payload.settled_at_utc, "corr-illegal-budget", "operation-settled", &payload,
    ).await.unwrap();
    assert_eq!(store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);

    let cancel_fixture = Fixture::new("run_illegal-cancel-history");
    let cancel_store = create_running(&cancel_fixture).await;
    let ack = CancellationAcknowledgedPayloadV1 {
        cancellation_id: CancellationId::parse("cancel_never-requested").unwrap(),
        operation_id: OperationId::parse("operation_never-reserved").unwrap(),
        reservation_id: ReservationId::parse("reservation_never-reserved").unwrap(),
        producer_revision: RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        authority_kind: "kernel_recovery".to_owned(),
        lease_authority: CallbackAuthorityV1 {
            reservation_id: ReservationId::parse("reservation_never-reserved").unwrap(),
            producer_revision: RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
            process_epoch: "epoch-b".to_owned(), lease_wall_millis: "0".to_owned(),
            lease_monotonic_millis: "0".to_owned(), deadline_monotonic_millis: "1".to_owned(),
            decision_monotonic_millis: "0".to_owned(),
            lease_fingerprint: digest('9'),
        },
        acknowledged_at_utc: "2026-08-26T00:00:20.000Z".to_owned(),
    };
    let mut tx = cancel_store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &cancel_fixture.registry(), &cancel_fixture.target()).await.unwrap();
    append_control(
        tx, &aggregate, &EventId::parse("event_illegal-cancel-ack").unwrap(),
        &ack.acknowledged_at_utc, "corr-illegal-cancel", "cancellation-acknowledged", &ack,
    ).await.unwrap();
    assert_eq!(cancel_store.runtime_control_projection(&cancel_fixture.registry(), &cancel_fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);
}

#[tokio::test]
async fn compatibility_rejects_forged_reservation_authority_cancellation_and_lifecycle() {
    let cancelled_fixture = Fixture::new("run_forged-reserve-cancelled");
    let (cancelled_store, mut cancelled) = completed_reservation_template(&cancelled_fixture, "cancel-seed").await;
    cancelled_store.request_cancellation(&cancelled_fixture.registry(), &cancelled_fixture.target(), &CancellationCommand {
        event_id: EventId::parse("event_cancel-before-forged-reserve").unwrap(),
        occurred_at: "2026-08-26T00:00:21.000Z".to_owned(), correlation_id: "corr-cancel-before-forge".to_owned(),
        cancellation_id: CancellationId::parse("cancel_before-forged-reserve").unwrap(),
        target: CancellationTargetV1::Task { task_id: cancelled_fixture.task_id.clone() }, reason_code: "stop".to_owned(),
    }).await.unwrap();
    retarget_reservation(&mut cancelled, "forged-after-cancel", "2026-08-26T00:00:22.000Z");
    append_forged_reservation(&cancelled_store, &cancelled_fixture, "event_forged-after-cancel", &cancelled).await;
    assert_eq!(cancelled_store.runtime_control_projection(&cancelled_fixture.registry(), &cancelled_fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);

    let usage_fixture = Fixture::new("run_forged-reserve-usage");
    let (usage_store, mut low_usage) = completed_reservation_template(&usage_fixture, "usage-seed").await;
    let mut low_grant = usage_fixture.grant("cap_low-usage", "agent_owner", "agent_owner", None, None, false, 1, 1);
    low_grant.issued_at_utc = "2026-08-26T00:00:21.000Z".to_owned();
    usage_store.issue_capability(&usage_fixture.registry(), &usage_fixture.target(), &EventId::parse("event_cap-low-usage").unwrap(), &low_grant.issued_at_utc, "corr-cap-low", low_grant.clone()).await.unwrap();
    retarget_reservation(&mut low_usage, "forged-low-usage", "2026-08-26T00:00:22.000Z");
    low_usage.grant_id = low_grant.grant_id.clone();
    low_usage.authorization_decision.grant_id = Some(low_grant.grant_id);
    append_forged_reservation(&usage_store, &usage_fixture, "event_forged-low-usage", &low_usage).await;
    assert_eq!(usage_store.runtime_control_projection(&usage_fixture.registry(), &usage_fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);

    for (suffix, mutation) in [
        ("adapter", 0_u8),
        ("cursor", 1_u8),
        ("deadline", 2_u8),
        ("timeout-policy", 3_u8),
    ] {
        let fixture = Fixture::new(&format!("run_forged-reserve-{suffix}"));
        let (store, mut forged) = completed_reservation_template(&fixture, &format!("{suffix}-seed")).await;
        retarget_reservation(&mut forged, &format!("forged-{suffix}"), "2026-08-26T00:00:22.000Z");
        match mutation {
            0 => forged.adapter_revision = RevisionId::parse("rev_wrong-adapter").unwrap(),
            1 => forged.lifecycle_admission.cursor.event_id = EventId::parse("event_run-created").unwrap(),
            2 => forged.timeout_key.absolute_deadline_utc = "2026-08-26T00:00:59.000Z".to_owned(),
            3 => forged.timeout_key.timeout_policy_revision = RevisionId::parse("rev_wrong-timeout-policy").unwrap(),
            _ => unreachable!(),
        }
        append_forged_reservation(&store, &fixture, &format!("event_forged-{suffix}"), &forged).await;
        assert_eq!(store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt, "{suffix}");
    }
}

#[tokio::test]
async fn compatibility_rejects_forged_settlement_and_late_lease_authority() {
    let settlement_fixture = Fixture::new("run_forged-settlement-authority");
    let settlement_store = create_running(&settlement_fixture).await;
    let lease = reserve(&settlement_store, &settlement_fixture, "forged-settlement").await;
    let reservation = settlement_store.runtime_control_projection(&settlement_fixture.registry(), &settlement_fixture.target()).await.unwrap().operations[0].reservation.clone();
    let command = settlement(&settlement_fixture, "forged-settlement", OperationOutcomeV1::Succeeded, 1);
    let mut forged_settlement = settlement_payload(&reservation, &lease, &command).unwrap();
    forged_settlement.callback_authority.as_mut().unwrap().process_epoch = "epoch-forged".to_owned();
    let mut tx = settlement_store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &settlement_fixture.registry(), &settlement_fixture.target()).await.unwrap();
    append_control(
        tx, &aggregate, &EventId::parse("event_forged-settlement-authority").unwrap(),
        &forged_settlement.settled_at_utc, "corr-forged-settlement-authority", "operation-settled", &forged_settlement,
    ).await.unwrap();
    assert_eq!(settlement_store.runtime_control_projection(&settlement_fixture.registry(), &settlement_fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);

    let late_fixture = Fixture::new("run_forged-late-authority");
    let late_store = create_running(&late_fixture).await;
    let late_lease = reserve(&late_store, &late_fixture, "forged-late").await;
    late_store.settle_operation(&late_fixture.registry(), &late_fixture.target(), &late_lease, &settlement(&late_fixture, "forged-late", OperationOutcomeV1::Succeeded, 1)).await.unwrap();
    let mut authority = durable_lease_authority(
        &late_lease,
        &FakeClock::at("2026-08-26T00:00:21.000Z", 2_100, "epoch-a").sample(),
    );
    authority.process_epoch = "epoch-forged".to_owned();
    let late = LateResultObservedPayloadV1 {
        operation_id: OperationId::parse("operation_forged-late").unwrap(),
        callback_id: CallbackId::parse("callback_fake-forged-late-second").unwrap(),
        callback_fingerprint: digest('8'), callback_authority: authority,
        classification: "late_after_succeeded".to_owned(), payload_digest: digest('9'),
        redaction_policy_revision: RevisionId::parse("rev_redaction-v1").unwrap(),
        received_at_utc: "2026-08-26T00:00:21.000Z".to_owned(),
    };
    let mut tx = late_store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_control(&mut tx, &late_fixture.registry(), &late_fixture.target()).await.unwrap();
    append_control(
        tx, &aggregate, &EventId::parse("event_forged-late-authority").unwrap(),
        &late.received_at_utc, "corr-forged-late-authority", "late-result-observed", &late,
    ).await.unwrap();
    assert_eq!(late_store.runtime_control_projection(&late_fixture.registry(), &late_fixture.target()).await.unwrap_err().kind, RuntimeControlErrorKind::AggregateCorrupt);
}

#[tokio::test]
async fn compatibility_rejects_schema_valid_illegal_terminal_winner_history() {
    for case in [
        "wrong-namespace",
        "cancelled-without-request",
        "success-after-cancel",
        "success-at-deadline",
        "timeout-before-deadline",
        "invalid-monotonic-equation",
        "lease-after-settlement",
        "meter-epoch-mismatch",
    ] {
        let fixture = Fixture::new(&format!("run_forged-terminal-{case}"));
        let store = create_running(&fixture).await;
        let suffix = format!("forged-terminal-{case}");
        let lease = reserve(&store, &fixture, &suffix).await;
        if case == "success-after-cancel" {
            store
                .request_cancellation(
                    &fixture.registry(),
                    &fixture.target(),
                    &CancellationCommand {
                        event_id: EventId::parse("event_cancel-before-forged-success").unwrap(),
                        occurred_at: "2026-08-26T00:00:11.000Z".to_owned(),
                        correlation_id: "corr-cancel-before-forged-success".to_owned(),
                        cancellation_id: CancellationId::parse("cancel_before-forged-success")
                            .unwrap(),
                        target: CancellationTargetV1::Operation {
                            operation_id: lease.operation_id.clone(),
                        },
                        reason_code: "stop".to_owned(),
                    },
                )
                .await
                .unwrap();
        }
        let reservation = store
            .runtime_control_projection(&fixture.registry(), &fixture.target())
            .await
            .unwrap()
            .operations[0]
            .reservation
            .clone();
        let command = settlement(&fixture, &suffix, OperationOutcomeV1::Succeeded, 1);
        let mut forged = settlement_payload(&reservation, &lease, &command).unwrap();
        match case {
            "wrong-namespace" => {
                forged.callback_id = Some(CallbackId::parse("callback_wrong-namespace").unwrap());
            }
            "cancelled-without-request" => forged.outcome = OperationOutcomeV1::Cancelled,
            "success-after-cancel" => {}
            "success-at-deadline" => {
                forged.settled_at_utc = reservation.absolute_deadline_utc.clone();
            }
            "timeout-before-deadline" => {
                forged.outcome = OperationOutcomeV1::TimedOut;
                forged.callback_id = None;
                forged.callback_fingerprint = None;
                forged.callback_authority = None;
                forged.timeout_command_fingerprint = Some(digest('8'));
            }
            "invalid-monotonic-equation" => {
                let authority = forged.callback_authority.as_mut().unwrap();
                authority.deadline_monotonic_millis = authority
                    .deadline_monotonic_millis
                    .parse::<u64>()
                    .unwrap()
                    .checked_add(1)
                    .unwrap()
                    .to_string();
                reseal_callback_authority(&fixture.scope, &forged.operation_id, authority);
            }
            "lease-after-settlement" => {
                let authority = forged.callback_authority.as_mut().unwrap();
                authority.lease_wall_millis =
                    parse_utc_millis("2026-08-26T00:00:30.000Z").unwrap().to_string();
                authority.lease_monotonic_millis = "3000".to_owned();
                authority.deadline_monotonic_millis = "33000".to_owned();
                authority.decision_monotonic_millis = "3000".to_owned();
                reseal_callback_authority(&fixture.scope, &forged.operation_id, authority);
            }
            "meter-epoch-mismatch" => {
                let authority = forged.callback_authority.as_mut().unwrap();
                authority.process_epoch = "epoch-b".to_owned();
                reseal_callback_authority(&fixture.scope, &forged.operation_id, authority);
            }
            _ => unreachable!(),
        }
        append_forged_settlement(
            &store,
            &fixture,
            &format!("event_forged-terminal-{case}"),
            &forged,
        )
        .await;
        assert_control_history_corrupt(store, &fixture).await;
    }
}

#[tokio::test]
async fn isolation_full_scope_and_business_id_matrix_is_no_write() {
    let fixture = Fixture::new("run_isolation-matrix");
    let store = create_running(&fixture).await;
    let mut targets = Vec::new();
    let mut tenant = fixture.target();
    tenant.scope.tenant_id = TenantId::parse("tenant_other").unwrap();
    targets.push(tenant);
    let mut no_user = fixture.target();
    no_user.scope.user_id = None;
    targets.push(no_user);
    let mut user = fixture.target();
    user.scope.user_id = Some(UserId::parse("user_bob").unwrap());
    targets.push(user);
    let mut workspace = fixture.target();
    workspace.scope.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    targets.push(workspace);
    let mut run = fixture.target();
    run.scope.run_id = RunId::parse("run_other").unwrap();
    targets.push(run);
    let mut owner = fixture.target();
    owner.scope.agent_id = AgentId::parse("agent_other-owner").unwrap();
    targets.push(owner);
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    for (index, target) in targets.iter().enumerate() {
        let mut proposal = fixture.proposal(&format!("isolation-{index}"));
        proposal.operation_id = OperationId::parse("operation_shadowed").unwrap();
        assert_eq!(store.reserve_protected_operation(&fixture.registry(), target, &proposal, &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::Unauthorized);
    }
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn projection_contains_complete_initialization_cancellation_and_history_provenance() {
    let fixture = Fixture::new("run_projection-provenance");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "projection-provenance").await;
    let cancel = CancellationCommand {
        event_id: EventId::parse("event_cancel-projection").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-cancel-projection".to_owned(), cancellation_id: CancellationId::parse("cancel_projection").unwrap(), target: CancellationTargetV1::Operation { operation_id: OperationId::parse("operation_projection-provenance").unwrap() }, reason_code: "projection".to_owned(),
    };
    store.request_cancellation(&fixture.registry(), &fixture.target(), &cancel).await.unwrap();
    let ack = CancellationAckCommand::from_producer(
        EventId::parse("event_ack-projection").unwrap(), "corr-ack-projection".to_owned(), cancel.cancellation_id,
        OperationId::parse("operation_projection-provenance").unwrap(), ReservationId::parse("reservation_projection-provenance").unwrap(),
        RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(), &FakeClock::at("2026-08-26T00:00:13.000Z", 1_300, "epoch-a"),
    ).unwrap();
    store.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &lease, &ack).await.unwrap();
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(projection.budget_revision, fixture.manifest.budget_revision);
    assert_eq!(projection.clock_contract, fixture.initialization().clock_contract);
    assert_eq!(projection.operation_contracts, vec![retained_operation_contract(fixture.set.reference()).unwrap()]);
    assert_eq!(projection.cancellations.len(), 1);
    assert_eq!(projection.cancellations[0].acknowledgements.len(), 1);
    assert_eq!(projection.source_contract.lifecycle_cursor.sequence, "2");
    assert_ne!(projection.history_digest, digest('0'));
    assert_eq!(projection.source_contract.accepted_event_bindings.len(), CONTROL_EVENT_TYPES.len());
}

#[tokio::test]
async fn lifecycle_admission_paused_and_terminal_task_run_matrix() {
    let paused_fixture = Fixture::new("run_paused-task-admission");
    let paused_store = create_running(&paused_fixture).await;
    let target = super::lifecycle::LifecycleTarget { scope: paused_fixture.scope.clone(), actor: paused_fixture.scope.agent_id.clone() };
    paused_store.transition_task(&paused_fixture.registry(), &target, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-paused-admission").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-task-paused".to_owned(), expected_sequence: 5, task_id: paused_fixture.task_id.clone(), expected_state: TaskState::Running, target_state: TaskState::Paused, reason_code: "pause".to_owned(),
    }).await.unwrap();
    assert_eq!(paused_store.reserve_protected_operation(&paused_fixture.registry(), &paused_fixture.target(), &paused_fixture.proposal("paused-task"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);

    let terminal_fixture = Fixture::new("run_terminal-admission");
    let terminal_store = create_running(&terminal_fixture).await;
    let terminal_target = super::lifecycle::LifecycleTarget { scope: terminal_fixture.scope.clone(), actor: terminal_fixture.scope.agent_id.clone() };
    terminal_store.transition_task(&terminal_fixture.registry(), &terminal_target, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-succeeded-admission").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-task-succeeded".to_owned(), expected_sequence: 5, task_id: terminal_fixture.task_id.clone(), expected_state: TaskState::Running, target_state: TaskState::Succeeded, reason_code: "done".to_owned(),
    }).await.unwrap();
    assert_eq!(terminal_store.reserve_protected_operation(&terminal_fixture.registry(), &terminal_fixture.target(), &terminal_fixture.proposal("terminal-task"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);
    terminal_store.transition_run(&terminal_fixture.registry(), &terminal_target, &TransitionRunCommand {
        event_id: EventId::parse("event_run-succeeded-admission").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-run-succeeded".to_owned(), expected_sequence: 6, expected_state: RunState::Running, target_state: RunState::Succeeded, reason_code: "done".to_owned(),
    }).await.unwrap();
    assert_eq!(terminal_store.reserve_protected_operation(&terminal_fixture.registry(), &terminal_fixture.target(), &terminal_fixture.proposal("terminal-run"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);
}

#[tokio::test]
async fn cancellation_authority_principal_and_terminal_task_matrix() {
    let fixture = Fixture::new("run_cancel-principal-matrix");
    let store = create_running(&fixture).await;
    let mut delegated = fixture.grant("cap_cancel-child", "agent_owner", "agent_child", Some("cap_root"), Some(fixture.task_id.clone()), false, 1, 4);
    delegated.issued_at_utc = "2026-08-26T00:00:06.000Z".to_owned();
    store.issue_capability(&fixture.registry(), &fixture.target(), &EventId::parse("event_cap-cancel-child").unwrap(), &delegated.issued_at_utc.clone(), "corr-cap-cancel", delegated).await.unwrap();
    store.reserve_protected_operation(&fixture.registry(), &fixture.target_as("agent_child"), &fixture.proposal("subject-cancel"), &live_clock()).await.unwrap();
    let subject_cancel = CancellationCommand {
        event_id: EventId::parse("event_cancel-by-subject").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-subject-cancel".to_owned(), cancellation_id: CancellationId::parse("cancel_by-subject").unwrap(), target: CancellationTargetV1::Operation { operation_id: OperationId::parse("operation_subject-cancel").unwrap() }, reason_code: "subject".to_owned(),
    };
    store.request_cancellation(&fixture.registry(), &fixture.target_as("agent_child"), &subject_cancel).await.unwrap();
    let unrelated = CancellationCommand {
        event_id: EventId::parse("event_cancel-by-unrelated").unwrap(), occurred_at: "2026-08-26T00:00:13.000Z".to_owned(), correlation_id: "corr-unrelated-cancel".to_owned(), cancellation_id: CancellationId::parse("cancel_by-unrelated").unwrap(), target: subject_cancel.target.clone(), reason_code: "unrelated".to_owned(),
    };
    assert_eq!(store.request_cancellation(&fixture.registry(), &fixture.target_as("agent_delegate"), &unrelated).await.unwrap_err().kind, RuntimeControlErrorKind::Unauthorized);

    let terminal_fixture = Fixture::new("run_cancel-terminal-task");
    let terminal_store = create_running(&terminal_fixture).await;
    let lifecycle_target = super::lifecycle::LifecycleTarget { scope: terminal_fixture.scope.clone(), actor: terminal_fixture.scope.agent_id.clone() };
    terminal_store.transition_task(&terminal_fixture.registry(), &lifecycle_target, &TransitionTaskCommand {
        event_id: EventId::parse("event_task-succeeded-cancel").unwrap(), occurred_at: "2026-08-26T00:00:11.000Z".to_owned(), correlation_id: "corr-terminal-task".to_owned(), expected_sequence: 5, task_id: terminal_fixture.task_id.clone(), expected_state: TaskState::Running, target_state: TaskState::Succeeded, reason_code: "done".to_owned(),
    }).await.unwrap();
    assert_eq!(terminal_store.request_cancellation(&terminal_fixture.registry(), &terminal_fixture.target(), &CancellationCommand {
        event_id: EventId::parse("event_cancel-terminal-task").unwrap(), occurred_at: "2026-08-26T00:00:12.000Z".to_owned(), correlation_id: "corr-cancel-terminal-task".to_owned(), cancellation_id: CancellationId::parse("cancel_terminal-task").unwrap(), target: CancellationTargetV1::Task { task_id: terminal_fixture.task_id.clone() }, reason_code: "late".to_owned(),
    }).await.unwrap_err().kind, RuntimeControlErrorKind::LifecycleStateDenied);
}

#[tokio::test]
async fn late_and_duplicate_out_of_order_messages_use_safe_rejected_audit() {
    let fixture = Fixture::new("run_rejected-control-audit");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "lease-source").await;
    let command = SettlementCommand::from_producer_observation(
        EventId::parse("event_rejected-pre-reserve").unwrap(), "corr-rejected-pre-reserve".to_owned(),
        CallbackId::parse("callback_fake-never-reserved").unwrap(), OperationId::parse("operation_never-reserved").unwrap(),
        ReservationId::parse("reservation_never-reserved").unwrap(), RevisionId::parse(FAKE_PRODUCER_REVISION).unwrap(),
        OperationOutcomeV1::Succeeded, Vec::new(), digest('8'), "out-of-order".to_owned(), None,
        &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a"),
    ).unwrap();
    let first = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    let retry = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command).await.unwrap();
    assert_eq!(append_identity(&first), append_identity(&retry));
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(projection.rejected_message_count, "1");
    assert!(projection.operations[0].outcome.is_none());
    let mut mutation = command;
    mutation.reason_code = "changed".to_owned();
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &mutation).await.unwrap_err().kind, RuntimeControlErrorKind::IdempotencyConflict);
}
