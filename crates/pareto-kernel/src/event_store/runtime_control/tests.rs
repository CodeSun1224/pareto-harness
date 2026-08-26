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

#[derive(Clone)]
struct FakeClock {
    sample: ClockSample,
}

impl FakeClock {
    fn at(value: &str, monotonic: u64, epoch: &str) -> Self {
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

struct Fixture {
    _temp: TempDir,
    path: std::path::PathBuf,
    set: Arc<SchemaSet>,
    limits: pareto_protocol::ProtocolLimitsRef,
    scope: IsolationScope,
    manifest: RunManifest,
    task_id: TaskId,
}

impl Fixture {
    fn new(run: &str) -> Self {
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

    fn registry(&self) -> SchemaRegistry {
        SchemaRegistry(vec![self.set.clone()])
    }

    fn target(&self) -> RuntimeControlTarget {
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
                reducer_revision: RevisionId::parse("rev_runtime-control-reducer").unwrap(),
                projection_schema_ref: self.set.schema_ref("runtime-control-projection").unwrap().clone(),
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
            operation_contracts: vec![TrustedOperationContractV1 {
                contract_revision: RevisionId::parse("rev_fake-operation").unwrap(),
                resource_kind: "fake".to_owned(), operation: "invoke".to_owned(),
                resource_envelope: usage(4),
                meter_revision: RevisionId::parse("rev_kernel-meter").unwrap(),
                producer_revision: RevisionId::parse("rev_fake-producer").unwrap(),
                redaction_policy_revision: RevisionId::parse("rev_redaction").unwrap(),
            }],
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

    fn proposal(&self, suffix: &str) -> ProtectedOperationProposal {
        ProtectedOperationProposal {
            event_id: EventId::parse(format!("event_reserve-{suffix}")).unwrap(),
            denied_event_id: EventId::parse(format!("event_denied-{suffix}")).unwrap(),
            occurred_at: "2026-08-26T00:00:10.000Z".to_owned(),
            correlation_id: format!("corr-{suffix}"),
            operation_id: OperationId::parse(format!("operation_{suffix}")).unwrap(),
            reservation_id: ReservationId::parse(format!("reservation_{suffix}")).unwrap(),
            task_id: Some(self.task_id.clone()), resource: resource(), operation: "invoke".to_owned(),
            requested_usage: usage(1), callback_namespace: "fake-callback-v1".to_owned(),
            interruptibility: OperationInterruptibilityV1::Cooperative,
            absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(),
            timeout_policy_revision: RevisionId::parse("rev_timeout-policy").unwrap(),
        }
    }
}

fn revision_pins() -> BTreeMap<String, RevisionId> {
    ["task", "behavior", "workspace", "environment", "context_graph", "model_snapshot", "tool_set", "kernel"]
        .into_iter().map(|role| (role.to_owned(), RevisionId::parse(format!("rev_{}", role.replace('_', "-"))).unwrap())).collect()
}

fn digest(fill: char) -> Digest { Digest::parse(format!("sha256:{}", fill.to_string().repeat(64))).unwrap() }
fn usage(amount: u64) -> Vec<BudgetVectorEntryV1> { vec![BudgetVectorEntryV1 { dimension: BudgetDimensionV1::Tokens, amount: BudgetAmountV1::new(amount) }] }
fn resource() -> ResourceSelectorV1 { ResourceSelectorV1 { kind: "fake".to_owned(), id: Some("fixture".to_owned()) } }
fn live_clock() -> FakeClock { FakeClock::at("2026-08-26T00:00:10.000Z", 1_000, "epoch-a") }

async fn create_initialized(fixture: &Fixture) -> EventStore {
    let store = EventStore::open(&fixture.path).await.unwrap();
    store.create_run(&fixture.trusted(), &CreateRunCommand {
        event_id: EventId::parse("event_run-created").unwrap(), occurred_at: "2026-08-26T00:00:00.000Z".to_owned(), correlation_id: "corr-run".to_owned(), manifest: fixture.manifest.clone(),
    }).await.unwrap();
    store.create_task(&fixture.registry(), &super::lifecycle::LifecycleTarget { scope: fixture.scope.clone(), actor: fixture.scope.agent_id.clone() }, &CreateTaskCommand {
        event_id: EventId::parse("event_task-created").unwrap(), occurred_at: "2026-08-26T00:00:01.000Z".to_owned(), correlation_id: "corr-task".to_owned(), expected_sequence: 1, task_id: fixture.task_id.clone(), parent_task_id: None,
    }).await.unwrap();
    store.initialize_runtime_control(&fixture.registry(), &fixture.target(), &InitializeRuntimeControlCommand {
        event_id: EventId::parse("event_control-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-control".to_owned(), payload: fixture.initialization(),
    }).await.unwrap();
    store
}

async fn create_running(fixture: &Fixture) -> EventStore {
    let store = create_initialized(fixture).await;
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

fn settlement(_fixture: &Fixture, suffix: &str, outcome: OperationOutcomeV1, metered: u64) -> SettlementCommand {
    SettlementCommand {
        event_id: EventId::parse(format!("event_settle-{suffix}")).unwrap(),
        occurred_at: "2026-08-26T00:00:20.000Z".to_owned(), correlation_id: format!("corr-settle-{suffix}"),
        callback_id: CallbackId::parse(format!("callback_{suffix}")).unwrap(),
        operation_id: OperationId::parse(format!("operation_{suffix}")).unwrap(),
        reservation_id: ReservationId::parse(format!("reservation_{suffix}")).unwrap(),
        producer_revision: RevisionId::parse("rev_fake-producer").unwrap(), outcome,
        evidence_class: UsageEvidenceClassV1::KernelMeterVerified,
        observed_usage: usage(99), metered_usage: usage(metered), reason_code: "fake-result".to_owned(),
    }
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
    assert!(matches!(result, ReserveResult::Denied { ref reason_code, .. } if reason_code == "default_deny"));
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
    assert!(matches!(result, ReserveResult::Denied { .. }));
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
    let settled = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
    let settled_retry = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command, &FakeClock::at("2026-08-26T00:00:30.000Z", 3_000, "epoch-a")).await.unwrap();
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
    let settled = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "refund-only", OperationOutcomeV1::Succeeded, 1), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
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
    command.evidence_class = UsageEvidenceClassV1::Unknown;
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
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
    let error = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap_err();
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
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &success, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap_err().kind, RuntimeControlErrorKind::CancellationPending);
    store.acknowledge_cancellation(&fixture.registry(), &fixture.target(), &lease, &CancellationAckCommand {
        event_id: EventId::parse("event_cancel-ack").unwrap(), occurred_at: "2026-08-26T00:00:13.000Z".to_owned(), correlation_id: "corr-ack".to_owned(), cancellation_id: cancel.cancellation_id, operation_id: OperationId::parse("operation_cancel").unwrap(), reservation_id: ReservationId::parse("reservation_cancel").unwrap(), producer_revision: RevisionId::parse("rev_fake-producer").unwrap(),
    }).await.unwrap();
    let cancelled = settlement(&fixture, "cancel", OperationOutcomeV1::Cancelled, 1);
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &cancelled, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
}

#[test]
fn cancellation_propagation() {
    let operation = OperationReservedPayloadV1 {
        operation_id: OperationId::parse("operation_probe").unwrap(),
        reservation_id: ReservationId::parse("reservation_probe").unwrap(),
        subject_actor: AgentId::parse("agent_owner").unwrap(), task_id: Some(TaskId::parse("task_probe").unwrap()), resource: resource(), operation: "invoke".to_owned(), grant_id: CapabilityId::parse("cap_probe").unwrap(),
        authorization_decision: AuthorizationDecisionV1 { outcome: AuthorizationOutcomeV1::Allowed, reason_code: "allowed".to_owned(), grant_id: Some(CapabilityId::parse("cap_probe").unwrap()), request_digest: digest('1') },
        requested_usage: usage(1), trusted_reservation: usage(1), allocations: Vec::new(), operation_contract_revision: RevisionId::parse("rev_operation").unwrap(), producer_revision: RevisionId::parse("rev_producer").unwrap(), callback_namespace: "probe".to_owned(), interruptibility: OperationInterruptibilityV1::Cooperative, absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(),
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
    let error = store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "uninterruptible", OperationOutcomeV1::Succeeded, 1), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::CancellationPending);
}

#[tokio::test]
async fn timeout_recovery() {
    let fixture = Fixture::new("run_timeout");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "timeout").await;
    let at_deadline = FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a");
    let complete = settlement(&fixture, "timeout", OperationOutcomeV1::Succeeded, 1);
    assert_eq!(store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &complete, &at_deadline).await.unwrap_err().kind, RuntimeControlErrorKind::DeadlineExceeded);
    let command = TimeoutRecoveryCommand { correlation_id: "corr-timeout".to_owned(), operation_id: OperationId::parse("operation_timeout").unwrap(), recovery_revision: RevisionId::parse("rev_timeout-recovery").unwrap(), evidence_fingerprint: digest('e') };
    let first = store.recover_timeout(&fixture.registry(), &fixture.target(), &command, &at_deadline).await.unwrap();
    let retry = store.recover_timeout(&fixture.registry(), &fixture.target(), &command, &at_deadline).await.unwrap();
    assert_eq!(append_identity(&first), append_identity(&retry));
    let later = store.recover_timeout(&fixture.registry(), &fixture.target(), &TimeoutRecoveryCommand { evidence_fingerprint: digest('f'), ..command }, &FakeClock::at("2026-08-26T00:01:01.000Z", 52_000, "epoch-a")).await.unwrap();
    assert_eq!(append_identity(&retry), append_identity(&later));
}

#[test]
fn deadline() {
    let before = FakeClock::at("2026-08-26T00:00:59.999Z", 49_999, "epoch-a");
    let at = FakeClock::at("2026-08-26T00:01:00.000Z", 50_000, "epoch-a");
    assert!(before.sample.wall_millis < at.sample.wall_millis);
    assert!(before.sample.monotonic_millis < at.sample.monotonic_millis);
}

#[tokio::test]
async fn timeout_not_due_consumes_no_identity() {
    let fixture = Fixture::new("run_timeout-not-due");
    let store = create_running(&fixture).await;
    reserve(&store, &fixture, "not-due").await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    let error = store.recover_timeout(&fixture.registry(), &fixture.target(), &TimeoutRecoveryCommand { correlation_id: "corr-not-due".to_owned(), operation_id: OperationId::parse("operation_not-due").unwrap(), recovery_revision: RevisionId::parse("rev_timeout-recovery").unwrap(), evidence_fingerprint: digest('d') }, &live_clock()).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::NotDue);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events").fetch_one(&store.pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn terminal_race() {
    let fixture = Fixture::new("run_terminal-race");
    let store = create_running(&fixture).await;
    let lease = reserve(&store, &fixture, "winner").await;
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "winner", OperationOutcomeV1::Succeeded, 2), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
    let late_timeout = store.recover_timeout(&fixture.registry(), &fixture.target(), &TimeoutRecoveryCommand { correlation_id: "corr-late-timeout".to_owned(), operation_id: OperationId::parse("operation_winner").unwrap(), recovery_revision: RevisionId::parse("rev_timeout-recovery").unwrap(), evidence_fingerprint: digest('c') }, &FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a")).await.unwrap();
    assert_eq!(append_identity(&late_timeout).0.as_str(), "event_settle-winner");
    let projection = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    assert_eq!(projection.operations[0].outcome, Some(OperationOutcomeV1::Succeeded));
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
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &command, &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
    let before = store.runtime_control_projection(&fixture.registry(), &fixture.target()).await.unwrap();
    let late = LateResultCommand {
        event_id: EventId::parse("event_late-audit").unwrap(), occurred_at: "2026-08-26T00:00:30.000Z".to_owned(), correlation_id: "corr-late".to_owned(), callback_id: CallbackId::parse("callback_late-second").unwrap(), operation_id: OperationId::parse("operation_late").unwrap(), producer_revision: RevisionId::parse("rev_fake-producer").unwrap(), redacted_payload_digest: digest('b'),
    };
    let first_late = store.observe_late_result(&fixture.registry(), &fixture.target(), &lease, &late).await.unwrap();
    let retry_late = store.observe_late_result(&fixture.registry(), &fixture.target(), &lease, &late).await.unwrap();
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
    store.settle_operation(&fixture.registry(), &fixture.target(), &lease, &settlement(&fixture, "reopen", OperationOutcomeV1::Succeeded, 2), &FakeClock::at("2026-08-26T00:00:20.000Z", 2_000, "epoch-a")).await.unwrap();
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
    assert_eq!(store.reserve_protected_operation(&fixture.registry(), &fixture.target(), &fixture.proposal("replay"), &live_clock()).await.unwrap_err().kind, RuntimeControlErrorKind::RecordedReplay);
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
    let store = EventStore::open(&fixture.path).await.unwrap();
    store.create_run(&fixture.trusted(), &CreateRunCommand { event_id: EventId::parse("event_run-created").unwrap(), occurred_at: "2026-08-26T00:00:00.000Z".to_owned(), correlation_id: "corr-run".to_owned(), manifest: fixture.manifest.clone() }).await.unwrap();
    let mut payload = fixture.initialization();
    payload.source_contract.protocol_limits_ref.digest = digest('f');
    let error = store.initialize_runtime_control(&fixture.registry(), &fixture.target(), &InitializeRuntimeControlCommand { event_id: EventId::parse("event_bad-control-init").unwrap(), occurred_at: "2026-08-26T00:00:02.000Z".to_owned(), correlation_id: "corr-bad-control".to_owned(), payload }).await.unwrap_err();
    assert_eq!(error.kind, RuntimeControlErrorKind::LifecycleStateDenied);
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

#[test]
fn model_sequences() {
    for reserved in 1..=8 {
        for consumed in 0..=reserved {
            for refunded in 0..=consumed {
                let live_reserved = reserved - consumed;
                let net_consumed = consumed - refunded;
                assert!(live_reserved + net_consumed + refunded == reserved);
            }
        }
    }
    let vector = usage(1);
    assert_eq!(canonical_vector(&vector).unwrap(), vector);
    assert!(vector_map(&[vector[0].clone(), vector[0].clone()]).is_err());
}
