use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, CancellationId, CancellationTargetV1, Digest, EventCursor,
    ExecutionMode, GateDecisionV1, HookDecisionId, HookId, HookInvocationKeyV1,
    HookInvocationReservedPayloadV1, HookInvocationTerminalPayloadV1,
    HookInvocationTerminalStateV1, HookKindV1, HookLimitsV1, HookPairBindingV1, HookPairId,
    HookPairKindV1, HookPhaseV1, HookPointV1, HookReasonCodeV1, HookRegistrationV1,
    HookRegistryRevisionV1, ObserverFailurePolicyV1, OperationOutcomeV1, ProposalId,
    ProtocolLimitsRef, ProtocolLimitsV1, RevisionId, RevisionMetadata, RunId, RunManifest,
    SchemaSet, TenantId, TransformContractV1, UsageEvidenceClassV1, UserId, WorkspaceId,
    derive_revision_id, generate_schema_bundle,
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
    hook_registry: Option<HookRegistryRevisionV1>,
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
        let path = temp.path().join("hook-runtime.sqlite3");
        let mut fixture = Self {
            _temp: temp,
            path,
            set,
            limits,
            scope,
            manifest,
            hook_registry: None,
        };
        let (manifest, hook_registry) = registry_fixture(&fixture);
        fixture.manifest = manifest;
        fixture.hook_registry = Some(hook_registry);
        fixture
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

    fn hook_registry(&self) -> &HookRegistryRevisionV1 {
        self.hook_registry.as_ref().unwrap()
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

fn digest(fill: char) -> Digest {
    Digest::parse(format!("sha256:{}", fill.to_string().repeat(64))).unwrap()
}

fn registration(
    fixture: &Fixture,
    kind: HookKindV1,
    id: &str,
    priority: i32,
) -> HookRegistrationV1 {
    let input_schema = fixture.set.schema_ref("hook-request-view").unwrap().clone();
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
            field_schema_ref: fixture
                .set
                .schema_ref("transform-field-value")
                .unwrap()
                .clone(),
            protected_hash_view_schema_ref: fixture
                .set
                .schema_ref("protected-proposal-hash-view")
                .unwrap()
                .clone(),
        }),
        resource_contract_revision: RevisionId::parse(FAKE_CONTRACT_REVISION).unwrap(),
        input_schema_ref: input_schema,
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
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry, &fixture.set).unwrap();
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
        ResolvedHookRegistry::resolve(&substituted, &registry, &fixture.set)
            .unwrap_err()
            .kind,
        HookErrorKind::ManifestInvalid
    );
}

pub(super) fn phase_order_lineage_case() {
    let fixture = Fixture::new("run_hook-lineage");
    let (manifest, registry) = registry_fixture(&fixture);
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry, &fixture.set).unwrap();
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
            request: &HookRequestViewV1,
        ) -> Result<UntrustedHookOutput, HookReasonCodeV1> {
            assert!(lease.narrowed);
            assert_eq!(lease.input_digest, request.input_digest);
            Ok(UntrustedHookOutput::Gate(GateDecisionV1::Allow {}))
        }
    }
    let fixture = Fixture::new("run_hook-handler-boundary");
    let input = digest('7');
    let output = Gate.invoke(
        &HookInvocationLease {
            invocation_id: pareto_protocol::HookInvocationId::parse("invocation_gate-a").unwrap(),
            hook_id: pareto_protocol::HookId::parse("hook_gate-a").unwrap(),
            input_digest: input.clone(),
            scope: fixture.scope.clone(),
            narrowed: true,
        },
        &HookRequestViewV1 {
            hook_point: HookPointV1::BeforeProposalAdmission,
            phase: HookPhaseV1::Gate,
            input_digest: input,
            proposal: TransformProposalV1 {
                proposal_id: ProposalId::parse("proposal_handler-boundary").unwrap(),
                schema_ref: fixture
                    .set
                    .schema_ref("transform-proposal")
                    .unwrap()
                    .clone(),
                fields: serde_json::json!({"content":"input"}),
            },
            fixed_business_decision: None,
        },
    );
    assert_eq!(
        output,
        Ok(UntrustedHookOutput::Gate(GateDecisionV1::Allow {}))
    );
}

struct PairHarness {
    fixture: control::Fixture,
    store: EventStore,
    hook_registry: HookRegistryRevisionV1,
    reserve: HookReservePairCommandV1,
    terminal: HookTerminalPairCommandV1,
}

fn install_pair_registry(fixture: &mut control::Fixture) -> HookRegistryRevisionV1 {
    let registration = HookRegistrationV1 {
        hook_id: HookId::parse("hook_gate-pair").unwrap(),
        hook_revision: RevisionId::parse("rev_gate-pair-v1").unwrap(),
        config_digest: digest('1'),
        kind: HookKindV1::Gate,
        hook_points: vec![HookPointV1::BeforeProposalAdmission],
        priority: 0,
        required: Some(true),
        observer_failure_policy: None,
        transform_contract: None,
        resource_contract_revision: RevisionId::parse(FAKE_CONTRACT_REVISION).unwrap(),
        input_schema_ref: fixture.set.schema_ref("hook-request-view").unwrap().clone(),
        output_schema_ref: fixture.set.schema_ref("gate-decision").unwrap().clone(),
        limits: HookLimitsV1 {
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_depth: 16,
            max_collection_items: 128,
        },
        redaction_policy_revision: RevisionId::parse("rev_redaction-v1").unwrap(),
        handler_compatibility_digest: digest('2'),
    };
    let registrations = vec![registration];
    let config_digest = registry_config_digest(&registrations).unwrap();
    let mut metadata = RevisionMetadata {
        logical_id: "hook-registry-pair".to_owned(),
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
        source: "pair-test-fixture".to_owned(),
        created_at: "2026-08-28T00:00:00.000Z".to_owned(),
    };
    metadata.revision_id = derive_revision_id(&metadata).unwrap();
    fixture
        .manifest
        .revisions
        .insert("hook_registry".to_owned(), metadata.revision_id.clone());
    fixture.manifest.hook_registry_config_digest = Some(config_digest.clone());
    HookRegistryRevisionV1 {
        metadata,
        config_digest,
        registrations,
    }
}

fn install_execution_registry(fixture: &mut control::Fixture) -> HookRegistryRevisionV1 {
    let make = |kind: HookKindV1, id: &str, priority: i32| HookRegistrationV1 {
        hook_id: HookId::parse(id).unwrap(),
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
            field_schema_ref: fixture
                .set
                .schema_ref("transform-field-value")
                .unwrap()
                .clone(),
            protected_hash_view_schema_ref: fixture
                .set
                .schema_ref("protected-proposal-hash-view")
                .unwrap()
                .clone(),
        }),
        resource_contract_revision: RevisionId::parse(FAKE_CONTRACT_REVISION).unwrap(),
        input_schema_ref: fixture.set.schema_ref("hook-request-view").unwrap().clone(),
        output_schema_ref: fixture
            .set
            .schema_ref(match kind {
                HookKindV1::Transform => "transform-proposal",
                HookKindV1::Gate => "gate-decision",
                HookKindV1::Observer => "observer-result",
            })
            .unwrap()
            .clone(),
        limits: HookLimitsV1 {
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_depth: 16,
            max_collection_items: 128,
        },
        redaction_policy_revision: RevisionId::parse("rev_redaction-v1").unwrap(),
        handler_compatibility_digest: digest('2'),
    };
    let registrations = vec![
        make(HookKindV1::Transform, "hook_transform-exec", 0),
        make(HookKindV1::Gate, "hook_gate-first", 0),
        make(HookKindV1::Gate, "hook_gate-second", 1),
        make(HookKindV1::Observer, "hook_observer-exec", 0),
    ];
    let config_digest = registry_config_digest(&registrations).unwrap();
    let mut metadata = RevisionMetadata {
        logical_id: "hook-registry-execution".to_owned(),
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
        source: "execution-test-fixture".to_owned(),
        created_at: "2026-08-28T00:00:00.000Z".to_owned(),
    };
    metadata.revision_id = derive_revision_id(&metadata).unwrap();
    fixture
        .manifest
        .revisions
        .insert("hook_registry".to_owned(), metadata.revision_id.clone());
    fixture.manifest.hook_registry_config_digest = Some(config_digest.clone());
    HookRegistryRevisionV1 {
        metadata,
        config_digest,
        registrations,
    }
}

struct ExecutionHandler {
    hook_id: HookId,
    output: UntrustedHookOutput,
    calls: Arc<Mutex<Vec<HookId>>>,
}

#[derive(Clone)]
struct AdvancingClock(Arc<Mutex<ClockSample>>);

impl RuntimeClock for AdvancingClock {
    fn sample(&self) -> ClockSample {
        self.0.lock().unwrap().clone()
    }
}

struct TimeoutTransformHandler {
    hook_id: HookId,
    output: UntrustedHookOutput,
    calls: Arc<Mutex<Vec<HookId>>>,
    clock: AdvancingClock,
}

struct CancellingTransformHandler {
    hook_id: HookId,
    output: UntrustedHookOutput,
    calls: Arc<Mutex<Vec<HookId>>>,
    path: std::path::PathBuf,
    store_id: String,
    registry: SchemaRegistry,
    target: RuntimeControlTarget,
}

impl FakeHookHandler for CancellingTransformHandler {
    fn invoke(
        &self,
        lease: &HookInvocationLease,
        request: &HookRequestViewV1,
    ) -> Result<UntrustedHookOutput, HookReasonCodeV1> {
        assert_eq!(lease.hook_id, self.hook_id);
        assert_eq!(lease.input_digest, request.input_digest);
        self.calls.lock().unwrap().push(self.hook_id.clone());
        let path = self.path.clone();
        let store_id = self.store_id.clone();
        let registry = self.registry.clone();
        let target = self.target.clone();
        let operation_id = operation_id_for(&lease.invocation_id).unwrap();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let store = EventStore::open_pinned(&path, &store_id).await.unwrap();
                    store
                        .request_cancellation(
                            &registry,
                            &target,
                            &control::CancellationCommand {
                                event_id: EventId::parse("event_cancel-kernel-hook").unwrap(),
                                occurred_at: "2026-08-26T00:00:20.000Z".to_owned(),
                                correlation_id: "corr-cancel-kernel-hook".to_owned(),
                                cancellation_id: CancellationId::parse("cancel_kernel-hook")
                                    .unwrap(),
                                target: CancellationTargetV1::Operation { operation_id },
                                reason_code: "user-request".to_owned(),
                            },
                        )
                        .await
                        .unwrap();
                });
        })
        .join()
        .unwrap();
        Ok(self.output.clone())
    }
}

impl FakeHookHandler for TimeoutTransformHandler {
    fn invoke(
        &self,
        lease: &HookInvocationLease,
        request: &HookRequestViewV1,
    ) -> Result<UntrustedHookOutput, HookReasonCodeV1> {
        assert_eq!(lease.hook_id, self.hook_id);
        assert_eq!(lease.input_digest, request.input_digest);
        self.calls.lock().unwrap().push(self.hook_id.clone());
        *self.clock.0.lock().unwrap() =
            control::FakeClock::at("2026-08-26T00:01:00.000Z", 51_000, "epoch-a").sample();
        Ok(self.output.clone())
    }
}

impl FakeHookHandler for ExecutionHandler {
    fn invoke(
        &self,
        lease: &HookInvocationLease,
        request: &HookRequestViewV1,
    ) -> Result<UntrustedHookOutput, HookReasonCodeV1> {
        assert_eq!(lease.hook_id, self.hook_id);
        assert_eq!(lease.input_digest, request.input_digest);
        self.calls.lock().unwrap().push(self.hook_id.clone());
        Ok(self.output.clone())
    }
}

fn execution_handlers(
    fixture: &control::Fixture,
    registry: &HookRegistryRevisionV1,
    deny_first_gate: bool,
    calls: Arc<Mutex<Vec<HookId>>>,
) -> FakeHookHandlers {
    let proposal = TransformProposalV1 {
        proposal_id: ProposalId::parse("proposal_execution").unwrap(),
        schema_ref: fixture
            .set
            .schema_ref("transform-proposal")
            .unwrap()
            .clone(),
        fields: serde_json::json!({"content":"transformed","authority":"fixed"}),
    };
    let mut handlers = FakeHookHandlers::default();
    for registration in &registry.registrations {
        let output = match registration.kind {
            HookKindV1::Transform => UntrustedHookOutput::Transform(Box::new(proposal.clone())),
            HookKindV1::Gate if registration.hook_id.as_str() == "hook_gate-first" => {
                if deny_first_gate {
                    UntrustedHookOutput::Gate(GateDecisionV1::Deny {
                        reason_code: HookReasonCodeV1::PolicyDenied,
                    })
                } else {
                    UntrustedHookOutput::Gate(GateDecisionV1::Allow {})
                }
            }
            HookKindV1::Gate => UntrustedHookOutput::Gate(GateDecisionV1::Allow {}),
            HookKindV1::Observer => UntrustedHookOutput::Observer(ObserverResultV1::Observed {
                annotation_digest: digest('9'),
            }),
        };
        handlers.bindings.insert(
            registration.hook_id.clone(),
            FakeHookHandlerBinding {
                hook_revision: registration.hook_revision.clone(),
                compatibility_digest: registration.handler_compatibility_digest.clone(),
                handler: Arc::new(ExecutionHandler {
                    hook_id: registration.hook_id.clone(),
                    output,
                    calls: calls.clone(),
                }),
            },
        );
    }
    handlers
}

async fn execution_harness(
    run: &str,
    deny_first_gate: bool,
) -> (
    control::Fixture,
    EventStore,
    HookRegistryRevisionV1,
    HookTarget,
    FakeHookHandlers,
    Arc<Mutex<Vec<HookId>>>,
) {
    let mut fixture = control::Fixture::new(run);
    let registry = install_execution_registry(&mut fixture);
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
                hook_registry_revision: registry.metadata.revision_id.clone(),
                hook_registry_config_digest: registry.config_digest.clone(),
            },
        )
        .await
        .unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let handlers = execution_handlers(&fixture, &registry, deny_first_gate, calls.clone());
    (fixture, store, registry, target, handlers, calls)
}

fn execution_command(fixture: &control::Fixture) -> ExecuteHookPointCommand {
    ExecuteHookPointCommand {
        point: HookPointV1::BeforeProposalAdmission,
        task_id: Some(fixture.task_id.clone()),
        source_cursor: EventCursor {
            sequence: "5".to_owned(),
            event_id: EventId::parse("event_task-running").unwrap(),
        },
        proposal: TransformProposalV1 {
            proposal_id: ProposalId::parse("proposal_execution").unwrap(),
            schema_ref: fixture
                .set
                .schema_ref("transform-proposal")
                .unwrap()
                .clone(),
            fields: serde_json::json!({"content":"initial","authority":"fixed"}),
        },
        occurred_at: "2026-08-26T00:00:10.000Z".to_owned(),
        correlation_id: "corr-hook-execution".to_owned(),
        absolute_deadline_utc: "2026-08-26T00:01:00.000Z".to_owned(),
        attempt: 1,
    }
}

fn reseal_hook_event<T: Serialize>(
    fixture: &control::Fixture,
    original: &ValidatedEvent,
    payload: &T,
) -> ValidatedEvent {
    let envelope = original.envelope();
    lifecycle_event(
        &fixture.set,
        &ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        },
        &fixture.scope,
        &fixture.scope.agent_id,
        &envelope.stream_id,
        &envelope.event_id,
        envelope.sequence.parse().unwrap(),
        &envelope.occurred_at,
        &envelope.correlation_id,
        &envelope.event_type,
        payload,
    )
    .unwrap()
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

    let mut fixture = control::Fixture::new(run);
    let hook_registry = install_pair_registry(&mut fixture);
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
    let point_id = HookDecisionId::parse("decision_pair-point").unwrap();
    let invocation_id = invocation_id_for(&point_id, &hook_registry.registrations[0], 0).unwrap();
    let reserve_pair = HookPairBindingV1 {
        pair_id: HookPairId::parse("pair_reserve-hook").unwrap(),
        pair_kind: HookPairKindV1::Reserve,
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
    store
        .append_hook_fact(
            &fixture.registry(),
            &target,
            None,
            &HookFactCommand {
                expected_cursor: EventCursor {
                    sequence: "1".to_owned(),
                    event_id: EventId::parse("event_hook-initialized").unwrap(),
                },
                event_id: EventId::parse("event_hook-point-started").unwrap(),
                occurred_at: "2026-08-26T00:00:06.500Z".to_owned(),
                correlation_id: "corr-hook-point".to_owned(),
                event_type: "hook-point-started",
                payload: HookPointStartedPayloadV1 {
                    point_id,
                    hook_point: HookPointV1::BeforeProposalAdmission,
                    subject_proposal_id: key.subject_proposal_id.clone(),
                    source_cursor: key.source_cursor.clone(),
                    initial_input_digest: key.input_digest.clone(),
                    ordered_invocations: vec![invocation_id.clone()],
                },
            },
        )
        .await
        .unwrap();
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
            sequence: "2".to_owned(),
            event_id: EventId::parse("event_hook-point-started").unwrap(),
        },
        control_sequence: 0,
        hook_sequence: 0,
        prepared_control_event_bytes: String::new(),
        prepared_hook_event_bytes: String::new(),
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
        pair_kind: HookPairKindV1::Terminal,
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
            sequence: "3".to_owned(),
            event_id: reserve.pair.hook_event_id.clone(),
        },
        control_sequence: 0,
        hook_sequence: 0,
        prepared_control_event_bytes: String::new(),
        prepared_hook_event_bytes: String::new(),
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
            reason_code: HookReasonCodeV1::Allowed,
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
        hook_registry,
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
            ("hook-point-started-v1", "hook-point-started"),
            ("hook-invocation-reserved-v1", "hook-invocation-reserved"),
        ]
    );
    fold_hook_events(&source.0, &hook_events, None).unwrap();
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

    let mut sequence_mutation = harness.reserve.clone();
    sequence_mutation.hook_sequence += 1;
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &sequence_mutation,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::IdempotencyConflict
    );

    let mut prepared_mutation = harness.reserve.clone();
    prepared_mutation.prepared_hook_event_bytes.push(' ');
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &prepared_mutation,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::IdempotencyConflict
    );

    let mut reused_pair = harness.reserve.clone();
    reused_pair.pair.control_event_id = EventId::parse("event_reused-pair-control").unwrap();
    reused_pair.pair.hook_event_id = EventId::parse("event_reused-pair-hook").unwrap();
    let reused_pair = seal_reserve_pair_command(reused_pair).unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_reserve_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &reused_pair,
                AtomicPairFault::None,
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::IdempotencyConflict
    );

    let mut cross_kind = harness.terminal.clone();
    cross_kind.pair.pair_id = harness.reserve.pair.pair_id.clone();
    let cross_kind = seal_terminal_pair_command(cross_kind).unwrap();
    assert_eq!(
        harness
            .store
            .append_hook_terminal_pair(
                &harness.fixture.registry(),
                &hook_target,
                &control_target,
                &cross_kind,
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
    let (hook_target, control_target) = pair_targets(&harness);
    let mut targets = Vec::new();
    let mut actor = hook_target.clone();
    actor.actor = AgentId::parse("agent_intruder").unwrap();
    targets.push(actor);
    let mut tenant = hook_target.clone();
    tenant.scope.tenant_id = TenantId::parse("tenant_other").unwrap();
    targets.push(tenant);
    let mut user_presence = hook_target.clone();
    user_presence.scope.user_id = None;
    targets.push(user_presence);
    let mut user_value = hook_target.clone();
    user_value.scope.user_id = Some(UserId::parse("user_other").unwrap());
    targets.push(user_value);
    let mut workspace = hook_target.clone();
    workspace.scope.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    targets.push(workspace);
    let mut run = hook_target.clone();
    run.scope.run_id = RunId::parse("run_other").unwrap();
    targets.push(run);
    let mut agent_scope = hook_target.clone();
    agent_scope.scope.agent_id = AgentId::parse("agent_other").unwrap();
    targets.push(agent_scope);
    for unauthorized_target in targets {
        assert_eq!(
            harness
                .store
                .append_hook_reserve_pair(
                    &harness.fixture.registry(),
                    &unauthorized_target,
                    &control_target,
                    &harness.reserve,
                    AtomicPairFault::None,
                )
                .await
                .unwrap_err()
                .kind,
            HookErrorKind::Unauthorized
        );
    }
    let mut task = harness.reserve.clone();
    task.hook_payload.key.task_id = Some(TaskId::parse("task_other").unwrap());
    let mut subject = harness.reserve.clone();
    subject.hook_payload.key.subject_proposal_id = ProposalId::parse("proposal_other").unwrap();
    let mut hook = harness.reserve.clone();
    hook.hook_payload.key.hook_id = HookId::parse("hook_other").unwrap();
    let resolved = ResolvedHookRegistry::resolve(
        &harness.fixture.manifest,
        &harness.hook_registry,
        &harness.fixture.set,
    )
    .unwrap();
    for (name, mutation) in [("task", task), ("subject", subject), ("hook", hook)] {
        let mutation = seal_reserve_pair_command(mutation).unwrap();
        assert!(
            harness
                .store
                .append_hook_reserve_pair_with_registry(
                    &harness.fixture.registry(),
                    &hook_target,
                    &control_target,
                    Some(&resolved),
                    &mutation,
                    AtomicPairFault::None,
                )
                .await
                .is_err(),
            "{name} identity mutation must fail closed"
        );
    }
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
    for reverse in [false, true] {
        let harness = pair_harness(if reverse {
            "run_hook-budget-concurrency-reverse"
        } else {
            "run_hook-budget-concurrency"
        })
        .await;
        let (hook_target, control_target) = pair_targets(&harness);
        let registry = harness.fixture.registry();
        let mut alternate = harness.reserve.clone();
        alternate.pair.pair_id = HookPairId::parse("pair_reserve-hook-alternate").unwrap();
        alternate.pair.control_event_id = EventId::parse("event_reserve-hook-alternate").unwrap();
        alternate.pair.hook_event_id = EventId::parse("event_hook-reserved-alternate").unwrap();
        alternate.control_payload.hook_pair = Some(alternate.pair.clone());
        alternate.hook_payload.pair = alternate.pair.clone();
        alternate.correlation_id = "corr-hook-reserve-alternate".to_owned();
        let alternate = seal_reserve_pair_command(alternate).unwrap();
        let (left, right) = if reverse {
            tokio::join!(
                harness.store.append_hook_reserve_pair(
                    &registry,
                    &hook_target,
                    &control_target,
                    &alternate,
                    AtomicPairFault::None,
                ),
                harness.store.append_hook_reserve_pair(
                    &registry,
                    &hook_target,
                    &control_target,
                    &harness.reserve,
                    AtomicPairFault::None,
                )
            )
        } else {
            tokio::join!(
                harness.store.append_hook_reserve_pair(
                    &registry,
                    &hook_target,
                    &control_target,
                    &harness.reserve,
                    AtomicPairFault::None,
                ),
                harness.store.append_hook_reserve_pair(
                    &registry,
                    &hook_target,
                    &control_target,
                    &alternate,
                    AtomicPairFault::None,
                )
            )
        };
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
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
        .hook_projection(
            &harness.fixture.registry(),
            &hook_target,
            &harness.hook_registry,
        )
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
    let resolved = ResolvedHookRegistry::resolve(&manifest, &registry, &fixture.set).unwrap();
    let proposal = TransformProposalV1 {
        proposal_id: ProposalId::parse("proposal_evaluation").unwrap(),
        schema_ref: fixture
            .set
            .schema_ref("transform-proposal")
            .unwrap()
            .clone(),
        fields: serde_json::json!({"content":"initial","authority":"fixed"}),
    };
    let protected = kernel_protected_view(
        &fixture.set,
        &resolved,
        &fixture.scope,
        &EventCursor {
            sequence: "5".to_owned(),
            event_id: EventId::parse("event_source-evaluation").unwrap(),
        },
        &proposal,
        &["/content".to_owned()],
    )
    .unwrap();
    (fixture, resolved, proposal, protected)
}

fn successful_outputs(
    proposal: &TransformProposalV1,
    _protected: &ProtectedProposalHashViewV1,
) -> BTreeMap<HookId, Result<UntrustedHookOutput, HookReasonCodeV1>> {
    let mut first = proposal.clone();
    first.fields["content"] = serde_json::json!("first");
    let mut final_proposal = proposal.clone();
    final_proposal.fields["content"] = serde_json::json!("final");
    BTreeMap::from([
        (
            HookId::parse("hook_transform-b").unwrap(),
            Ok(UntrustedHookOutput::Transform(Box::new(first))),
        ),
        (
            HookId::parse("hook_transform-a").unwrap(),
            Ok(UntrustedHookOutput::Transform(Box::new(final_proposal))),
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
            reason_code: HookReasonCodeV1::PolicyDenied,
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
    assert_eq!(denied.reason_code, HookReasonCodeV1::RequiredGateEmpty);
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
            reason_code: HookReasonCodeV1::ObserverFailedClosed,
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
            reason_code: HookReasonCodeV1::HookKindMismatch,
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
    let before = serde_json::json!({
        "nested": {"labels": ["first", "second"], "a/b": "fixed"}
    });
    let after = serde_json::json!({
        "nested": {"labels": ["first", "changed"], "a/b": "fixed"}
    });
    assert_eq!(
        changed_json_pointers(&before, &after, ""),
        vec!["/nested/labels/1"]
    );
    let escaped_after = serde_json::json!({
        "nested": {"labels": ["first", "second"], "a/b": "changed"}
    });
    assert_eq!(
        changed_json_pointers(&before, &escaped_after, ""),
        vec!["/nested/a~1b"]
    );

    let (fixture, resolved, proposal, protected) = evaluation_fixture();
    let mut outputs = successful_outputs(&proposal, &protected);
    let mut candidate = proposal.clone();
    candidate.fields["content"] = serde_json::json!("attempted");
    candidate.fields["authority"] = serde_json::json!("changed");
    outputs.insert(
        HookId::parse("hook_transform-b").unwrap(),
        Ok(UntrustedHookOutput::Transform(Box::new(candidate))),
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
        Ok(UntrustedHookOutput::Transform(Box::new(oversized))),
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
    assert_eq!(result.reason_code, HookReasonCodeV1::TransformOutputInvalid);
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
    terminal.hook_payload.reason_code = HookReasonCodeV1::TimedOut;
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
        .hook_projection(&registry, &hook_target, &harness.hook_registry)
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
        .hook_projection(&registry, &hook_target, &harness.hook_registry)
        .await
        .unwrap();
    assert_eq!(
        projection.invocations[0].terminal_state,
        Some(HookInvocationTerminalStateV1::TimedOut)
    );
}

pub(super) async fn kernel_owned_execution_case() {
    let (fixture, store, registry, target, handlers, calls) =
        execution_harness("run_hook-kernel-execution", false).await;
    let outcome = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await;
    if let Err(error) = &outcome {
        let rows: Vec<(String, String, i64)> =
            sqlx::query_as(
                "SELECT stream_id,json_extract(envelope_json,'$.event_type'),CAST(json_extract(envelope_json,'$.sequence') AS INTEGER) FROM events ORDER BY rowid",
            )
                .fetch_all(&store.pool)
                .await
                .unwrap();
        panic!(
            "execution failed: {error:?}; calls={:?}; rows={rows:?}",
            calls.lock().unwrap()
        );
    }
    let result = outcome.unwrap();
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Allow);
    assert_eq!(result.execution_status, HookExecutionStatusV1::Completed);
    assert_eq!(result.reason_code, HookReasonCodeV1::Completed);
    assert!(!result.already_committed);
    assert_eq!(
        result.proposal.as_ref().unwrap().fields["content"],
        "transformed"
    );
    assert_eq!(
        result.proposal.as_ref().unwrap().fields["authority"],
        "fixed"
    );
    assert_eq!(result.projection.invocations.len(), 4);
    assert_eq!(result.projection.finalized_points.len(), 1);
    assert_eq!(result.projection.skipped_count, 0);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            HookId::parse("hook_transform-exec").unwrap(),
            HookId::parse("hook_gate-first").unwrap(),
            HookId::parse("hook_gate-second").unwrap(),
            HookId::parse("hook_observer-exec").unwrap(),
        ]
    );

    let event_count_after_execution: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let retry = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await
        .unwrap();
    assert!(retry.already_committed);
    assert_eq!(retry.point_id, result.point_id);
    assert_eq!(retry.proposal, None);
    assert_eq!(retry.projection, result.projection);
    assert_eq!(calls.lock().unwrap().len(), 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        event_count_after_execution
    );
    let mut mutations = Vec::new();
    let mut task_mutation = execution_command(&fixture);
    task_mutation.task_id = Some(TaskId::parse("task_mutated").unwrap());
    mutations.push(task_mutation);
    let mut occurred_at_mutation = execution_command(&fixture);
    occurred_at_mutation.occurred_at = "2026-08-26T00:00:11.000Z".to_owned();
    mutations.push(occurred_at_mutation);
    let mut correlation_mutation = execution_command(&fixture);
    correlation_mutation.correlation_id = "corr-hook-mutated".to_owned();
    mutations.push(correlation_mutation);
    let mut deadline_mutation = execution_command(&fixture);
    deadline_mutation.absolute_deadline_utc = "2026-08-26T00:02:00.000Z".to_owned();
    mutations.push(deadline_mutation);
    for mutation in mutations {
        assert_eq!(
            store
                .execute_hook_point(
                    &fixture.registry(),
                    &target,
                    &fixture.target(),
                    &registry,
                    &handlers,
                    &mutation,
                    &control::live_clock(),
                )
                .await
                .unwrap_err()
                .kind,
            HookErrorKind::IdempotencyConflict
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        event_count_after_execution
    );

    let before_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let before_control = store
        .runtime_control_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    let replayed = store
        .recorded_hook_projection(
            &fixture.registry(),
            &target,
            &registry,
            &ExecutionMode::RecordedReplay {
                source_run_id: fixture.scope.run_id.clone(),
                boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
            },
        )
        .await
        .unwrap();
    let after_control = store
        .runtime_control_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    let after_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(replayed, result.projection);
    assert_eq!(before_events, after_events);
    assert_eq!(before_control.accounts, after_control.accounts);
    assert_eq!(before_control.operations, after_control.operations);
}

pub(super) async fn kernel_owned_start_recovery_case() {
    let (fixture, store, registry, target, handlers, calls) =
        execution_harness("run_hook-kernel-start-recovery", false).await;
    let command = execution_command(&fixture);
    let resolved =
        ResolvedHookRegistry::resolve(&fixture.manifest, &registry, &fixture.set).unwrap();
    let point_id = point_id_for(&target.scope, &command).unwrap();
    let ordered_invocations = resolved
        .ordered_for_point(command.point)
        .iter()
        .enumerate()
        .map(|(ordinal, registration)| {
            invocation_id_for(&point_id, registration, ordinal as u32).unwrap()
        })
        .collect();
    let initial_input_digest = proposal_digest(&fixture.set, &command.proposal).unwrap();
    let projection = store
        .hook_projection(&fixture.registry(), &target, &registry)
        .await
        .unwrap();
    store
        .append_hook_fact(
            &fixture.registry(),
            &target,
            Some(&resolved),
            &HookFactCommand {
                expected_cursor: projection.inclusive_cursor,
                event_id: point_start_event_id(&point_id, &command).unwrap(),
                occurred_at: command.occurred_at.clone(),
                correlation_id: command.correlation_id.clone(),
                event_type: "hook-point-started",
                payload: HookPointStartedPayloadV1 {
                    point_id: point_id.clone(),
                    hook_point: command.point,
                    subject_proposal_id: command.proposal.proposal_id.clone(),
                    source_cursor: command.source_cursor.clone(),
                    initial_input_digest,
                    ordered_invocations,
                },
            },
        )
        .await
        .unwrap();
    let result = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &command,
            &control::live_clock(),
        )
        .await
        .unwrap();
    assert_eq!(result.point_id, point_id);
    assert_eq!(result.execution_status, HookExecutionStatusV1::Completed);
    assert!(!result.already_committed);
    assert_eq!(result.projection.finalized_points, vec![point_id]);
    assert_eq!(calls.lock().unwrap().len(), 4);
    let started_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM events WHERE json_extract(envelope_json,'$.event_type')='hook-point-started'",
    )
    .fetch_one(&store.pool)
    .await
    .unwrap();
    assert_eq!(started_count, 1);
}

pub(super) async fn kernel_owned_gate_short_circuit_case() {
    let (fixture, store, registry, target, handlers, calls) =
        execution_harness("run_hook-kernel-deny", true).await;
    let result = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await
        .unwrap();
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(result.execution_status, HookExecutionStatusV1::GateDenied);
    assert_eq!(result.reason_code, HookReasonCodeV1::GateDenied);
    assert_eq!(result.projection.invocations.len(), 2);
    assert_eq!(result.projection.skipped_count, 2);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            HookId::parse("hook_transform-exec").unwrap(),
            HookId::parse("hook_gate-first").unwrap(),
        ]
    );
    assert_eq!(result.projection.finalized_points, vec![result.point_id]);
}

pub(super) async fn kernel_owned_timeout_case() {
    let (fixture, store, registry, target, mut handlers, calls) =
        execution_harness("run_hook-kernel-timeout", false).await;
    let clock = AdvancingClock(Arc::new(Mutex::new(control::live_clock().sample())));
    let registration = registry
        .registrations
        .iter()
        .find(|registration| registration.kind == HookKindV1::Transform)
        .unwrap();
    handlers.bindings.insert(
        registration.hook_id.clone(),
        FakeHookHandlerBinding {
            hook_revision: registration.hook_revision.clone(),
            compatibility_digest: registration.handler_compatibility_digest.clone(),
            handler: Arc::new(TimeoutTransformHandler {
                hook_id: registration.hook_id.clone(),
                output: UntrustedHookOutput::Transform(Box::new(TransformProposalV1 {
                    proposal_id: ProposalId::parse("proposal_execution").unwrap(),
                    schema_ref: fixture
                        .set
                        .schema_ref("transform-proposal")
                        .unwrap()
                        .clone(),
                    fields: serde_json::json!({"content":"too-late","authority":"fixed"}),
                })),
                calls: calls.clone(),
                clock: clock.clone(),
            }),
        },
    );
    let result = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &clock,
        )
        .await
        .unwrap();
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.reason_code, HookReasonCodeV1::TimedOut);
    assert_eq!(
        result.proposal.as_ref().unwrap().fields["content"],
        "initial"
    );
    assert_eq!(result.projection.invocations.len(), 1);
    assert_eq!(result.projection.skipped_count, 3);
    assert_eq!(result.projection.late_result_count, 1);
    assert_eq!(
        result.projection.invocations[0].terminal_state,
        Some(HookInvocationTerminalStateV1::TimedOut)
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    let control_projection = store
        .runtime_control_projection(&fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(
        control_projection.operations[0]
            .settlement
            .as_ref()
            .unwrap()
            .outcome,
        OperationOutcomeV1::TimedOut
    );
}

pub(super) async fn kernel_owned_cancellation_case() {
    let (fixture, store, registry, target, mut handlers, calls) =
        execution_harness("run_hook-kernel-cancel", false).await;
    let transform = registry
        .registrations
        .iter()
        .find(|registration| registration.kind == HookKindV1::Transform)
        .unwrap();
    let output = UntrustedHookOutput::Transform(Box::new(TransformProposalV1 {
        proposal_id: ProposalId::parse("proposal_execution").unwrap(),
        schema_ref: fixture
            .set
            .schema_ref("transform-proposal")
            .unwrap()
            .clone(),
        fields: serde_json::json!({"content":"cancelled-output","authority":"fixed"}),
    }));
    handlers.bindings.insert(
        transform.hook_id.clone(),
        FakeHookHandlerBinding {
            hook_revision: transform.hook_revision.clone(),
            compatibility_digest: transform.handler_compatibility_digest.clone(),
            handler: Arc::new(CancellingTransformHandler {
                hook_id: transform.hook_id.clone(),
                output,
                calls: calls.clone(),
                path: fixture.path.clone(),
                store_id: store.store_id.clone(),
                registry: fixture.registry(),
                target: fixture.target(),
            }),
        },
    );
    let result = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await
        .unwrap();
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.reason_code, HookReasonCodeV1::Cancelled);
    assert_eq!(
        result.proposal.as_ref().unwrap().fields["content"],
        "initial"
    );
    assert_eq!(result.projection.invocations.len(), 1);
    assert_eq!(result.projection.skipped_count, 3);
    assert_eq!(result.projection.late_result_count, 1);
    assert_eq!(
        result.projection.invocations[0].terminal_state,
        Some(HookInvocationTerminalStateV1::Cancelled)
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

pub(super) async fn kernel_owned_rejection_case() {
    let (fixture, store, registry, target, mut handlers, calls) =
        execution_harness("run_hook-kernel-rejection", false).await;
    let registration = registry
        .registrations
        .iter()
        .find(|registration| registration.kind == HookKindV1::Transform)
        .unwrap();
    handlers.bindings.insert(
        registration.hook_id.clone(),
        FakeHookHandlerBinding {
            hook_revision: registration.hook_revision.clone(),
            compatibility_digest: registration.handler_compatibility_digest.clone(),
            handler: Arc::new(ExecutionHandler {
                hook_id: registration.hook_id.clone(),
                output: UntrustedHookOutput::Gate(GateDecisionV1::Allow {}),
                calls: calls.clone(),
            }),
        },
    );
    let result = store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await
        .unwrap();
    assert_eq!(result.business_decision, HookBusinessDecisionV1::Deny);
    assert_eq!(
        result.execution_status,
        HookExecutionStatusV1::TransformFailed
    );
    assert_eq!(result.reason_code, HookReasonCodeV1::TransformOutputInvalid);
    assert_eq!(result.projection.rejected_count, 1);
    assert_eq!(result.projection.skipped_count, 3);
    assert_eq!(calls.lock().unwrap().len(), 1);

    let limits = ProtocolLimitsRef {
        profile: "protocol-limits-v1".to_owned(),
        digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
    };
    let events = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    let rejection_index = events
        .iter()
        .position(|event| {
            event
                .downcast_payload::<HookMessageRejectedPayloadV1>()
                .is_some()
        })
        .unwrap();
    let rejection = events[rejection_index]
        .downcast_payload::<HookMessageRejectedPayloadV1>()
        .unwrap();
    let mut mutations = Vec::new();
    let mut wrong_point = rejection.clone();
    wrong_point.hook_point = HookPointV1::BeforeAuthoritativeCommit;
    mutations.push(wrong_point);
    let mut wrong_hook = rejection.clone();
    wrong_hook.hook_id = Some(HookId::parse("hook_unknown-rejection").unwrap());
    mutations.push(wrong_hook);
    let mut wrong_revision = rejection.clone();
    wrong_revision.hook_revision = Some(RevisionId::parse("rev_wrong-rejection").unwrap());
    mutations.push(wrong_revision);
    let mut wrong_source = rejection.clone();
    wrong_source.source_cursor.event_id = EventId::parse("event_wrong-source").unwrap();
    mutations.push(wrong_source);
    let mut wrong_input = rejection.clone();
    wrong_input.input_digest = digest('7');
    mutations.push(wrong_input);
    let mut wrong_redaction = rejection.clone();
    wrong_redaction.redaction_policy_revision = RevisionId::parse("rev_wrong-redaction").unwrap();
    mutations.push(wrong_redaction);
    let mut wrong_decision = rejection.clone();
    wrong_decision.decision_id = HookDecisionId::parse("decision_wrong-rejection").unwrap();
    mutations.push(wrong_decision);
    let resolved =
        ResolvedHookRegistry::resolve(&fixture.manifest, &registry, &fixture.set).unwrap();
    for mutation in mutations {
        let mut history = store
            .read_hook_events(&target, fixture.set.clone(), limits.clone())
            .await
            .unwrap();
        history[rejection_index] = reseal_hook_event(&fixture, &events[rejection_index], &mutation);
        assert_eq!(
            fold_hook_events(&fixture.set, &history, Some(&resolved))
                .unwrap_err()
                .kind,
            HookErrorKind::AggregateCorrupt
        );
    }
}

pub(super) async fn resealed_history_rejection_case() {
    let (fixture, store, registry, target, handlers, _) =
        execution_harness("run_hook-resealed-history", false).await;
    store
        .execute_hook_point(
            &fixture.registry(),
            &target,
            &fixture.target(),
            &registry,
            &handlers,
            &execution_command(&fixture),
            &control::live_clock(),
        )
        .await
        .unwrap();
    let limits = ProtocolLimitsRef {
        profile: "protocol-limits-v1".to_owned(),
        digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
    };
    let events = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    let resolved =
        ResolvedHookRegistry::resolve(&fixture.manifest, &registry, &fixture.set).unwrap();

    let reserve_index = events
        .iter()
        .position(|event| {
            event
                .downcast_payload::<HookInvocationReservedPayloadV1>()
                .is_some()
        })
        .unwrap();
    let mut wrong_point = events[reserve_index]
        .downcast_payload::<HookInvocationReservedPayloadV1>()
        .unwrap()
        .clone();
    wrong_point.key.hook_point = HookPointV1::BeforeAuthoritativeCommit;
    let mut wrong_point_history = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    wrong_point_history[reserve_index] =
        reseal_hook_event(&fixture, &events[reserve_index], &wrong_point);
    assert_eq!(
        fold_hook_events(&fixture.set, &wrong_point_history, Some(&resolved))
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
    );

    let final_index = events
        .iter()
        .position(|event| {
            event
                .downcast_payload::<HookPointFinalizedPayloadV1>()
                .is_some()
        })
        .unwrap();
    let mut wrong_final = events[final_index]
        .downcast_payload::<HookPointFinalizedPayloadV1>()
        .unwrap()
        .clone();
    wrong_final.final_input_digest = digest('8');
    let mut wrong_final_history = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    wrong_final_history[final_index] =
        reseal_hook_event(&fixture, &events[final_index], &wrong_final);
    assert_eq!(
        fold_hook_events(&fixture.set, &wrong_final_history, Some(&resolved))
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
    );

    let terminal_index = events
        .iter()
        .position(|event| {
            event
                .downcast_payload::<HookInvocationTerminalPayloadV1>()
                .is_some()
        })
        .unwrap();
    let mut wrong_pair = events[terminal_index]
        .downcast_payload::<HookInvocationTerminalPayloadV1>()
        .unwrap()
        .clone();
    wrong_pair.pair.pair_id = HookPairId::parse("pair_resealed-cross-stream").unwrap();
    let mut wrong_pair_history = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    wrong_pair_history[terminal_index] =
        reseal_hook_event(&fixture, &events[terminal_index], &wrong_pair);
    let mut transaction = store.pool.begin().await.unwrap();
    let control_events = read_stream_events_in_transaction(
        &mut transaction,
        &target,
        runtime_control_stream_id(&fixture.scope).unwrap(),
        fixture.set.clone(),
        limits.clone(),
    )
    .await
    .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        validate_cross_stream_pairs(&wrong_pair_history, &control_events)
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
    );

    let mut counterpart_hook_history = store
        .read_hook_events(&target, fixture.set.clone(), limits.clone())
        .await
        .unwrap();
    let mut transaction = store.pool.begin().await.unwrap();
    let mut counterpart_control_history = read_stream_events_in_transaction(
        &mut transaction,
        &target,
        runtime_control_stream_id(&fixture.scope).unwrap(),
        fixture.set.clone(),
        limits,
    )
    .await
    .unwrap();
    transaction.rollback().await.unwrap();
    let replacement_hook_event_id = EventId::parse("event_resealed-hook-counterpart").unwrap();
    let mut hook_terminal = counterpart_hook_history[terminal_index]
        .downcast_payload::<HookInvocationTerminalPayloadV1>()
        .unwrap()
        .clone();
    let terminal_pair_id = hook_terminal.pair.pair_id.clone();
    hook_terminal.pair.hook_event_id = replacement_hook_event_id.clone();
    counterpart_hook_history[terminal_index] = reseal_hook_event(
        &fixture,
        &counterpart_hook_history[terminal_index],
        &hook_terminal,
    );
    let control_terminal_index = counterpart_control_history
        .iter()
        .position(|event| {
            event
                .downcast_payload::<OperationSettledPayloadV1>()
                .and_then(|payload| payload.hook_pair.as_ref())
                .is_some_and(|pair| pair.pair_id == terminal_pair_id)
        })
        .unwrap();
    let mut control_terminal = counterpart_control_history[control_terminal_index]
        .downcast_payload::<OperationSettledPayloadV1>()
        .unwrap()
        .clone();
    control_terminal.hook_pair.as_mut().unwrap().hook_event_id = replacement_hook_event_id;
    counterpart_control_history[control_terminal_index] = reseal_hook_event(
        &fixture,
        &counterpart_control_history[control_terminal_index],
        &control_terminal,
    );
    assert_eq!(
        validate_cross_stream_pairs(&counterpart_hook_history, &counterpart_control_history,)
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
    );
}

pub(super) async fn recorded_vertical_case() {
    struct CountingGate(Arc<AtomicUsize>);
    impl FakeHookHandler for CountingGate {
        fn invoke(
            &self,
            lease: &HookInvocationLease,
            request: &HookRequestViewV1,
        ) -> Result<UntrustedHookOutput, HookReasonCodeV1> {
            assert!(lease.narrowed);
            assert_eq!(lease.input_digest, request.input_digest);
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(UntrustedHookOutput::Gate(GateDecisionV1::Allow {}))
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
    let request = HookRequestViewV1 {
        hook_point: harness.reserve.hook_payload.key.hook_point,
        phase: harness.reserve.hook_payload.key.phase,
        input_digest: harness.reserve.hook_payload.key.input_digest.clone(),
        proposal: TransformProposalV1 {
            proposal_id: harness.reserve.hook_payload.key.subject_proposal_id.clone(),
            schema_ref: harness
                .fixture
                .set
                .schema_ref("transform-proposal")
                .unwrap()
                .clone(),
            fields: serde_json::json!({"content":"recorded"}),
        },
        fixed_business_decision: None,
    };
    assert_eq!(
        handler.invoke(&invocation_lease, &request),
        Ok(UntrustedHookOutput::Gate(GateDecisionV1::Allow {}))
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
        .hook_projection(&registry, &hook_target, &harness.hook_registry)
        .await
        .unwrap();
    let recorded_counter = Arc::new(AtomicUsize::new(0));
    let recorded = harness
        .store
        .recorded_hook_projection(
            &registry,
            &hook_target,
            &harness.hook_registry,
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
                .recorded_hook_projection(
                    &fixture.registry(),
                    &fixture.target(),
                    fixture.hook_registry(),
                    &mode,
                )
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
    let aggregate = fold_hook_events(&fixture.set, &events, None).unwrap();
    assert_eq!(aggregate.inclusive_cursor.sequence, "1");
    assert!(aggregate.invocations.is_empty());
    assert_eq!(
        store
            .hook_projection(
                &fixture.registry(),
                &fixture.target(),
                fixture.hook_registry(),
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt,
        "a Hook stream without its Runtime Control source must fail closed"
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
        fold_hook_events(&fixture.set, &invalid, None)
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
    );
}

#[tokio::test]
async fn recovery() {
    let fixture = Fixture::new("run_hook-recovery");
    let store = fixture.open_initialized().await;
    let store_id = store.store_id.clone();
    let before = store
        .hook_projection(
            &fixture.registry(),
            &fixture.target(),
            fixture.hook_registry(),
        )
        .await
        .unwrap_err()
        .kind;
    store.pool.close().await;
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let after = reopened
        .hook_projection(
            &fixture.registry(),
            &fixture.target(),
            fixture.hook_registry(),
        )
        .await
        .unwrap_err()
        .kind;
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
            .hook_projection(
                &fixture.registry(),
                &fixture.target(),
                fixture.hook_registry(),
            )
            .await
            .unwrap_err()
            .kind,
        HookErrorKind::AggregateCorrupt
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
        .hook_projection(
            &fixture.registry(),
            &fixture.target(),
            fixture.hook_registry(),
        )
        .await
        .unwrap_err()
        .kind;
    let recorded = store
        .recorded_hook_projection(
            &fixture.registry(),
            &fixture.target(),
            fixture.hook_registry(),
            &ExecutionMode::RecordedReplay {
                source_run_id: RunId::parse("run_source").unwrap(),
                boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
            },
        )
        .await
        .unwrap_err()
        .kind;
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
                .recorded_hook_projection(
                    &fixture.registry(),
                    &fixture.target(),
                    fixture.hook_registry(),
                    &unsupported,
                )
                .await
                .unwrap_err()
                .kind,
            HookErrorKind::UnsupportedMode
        );
    }
}
