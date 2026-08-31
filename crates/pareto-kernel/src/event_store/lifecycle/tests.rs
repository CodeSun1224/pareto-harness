use pareto_protocol::{
    ProtocolLimitsV1, TenantId, UserId, WorkspaceId, generate_schema_bundle,
};
use sqlx::Connection;
use std::str::FromStr;
use tempfile::TempDir;

use super::{canonical, fingerprint};
use crate::event_store::effect_runtime::{EffectTarget, InitializeEffectStream};

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
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };
        let scope = IsolationScope {
            tenant_id: TenantId::parse("tenant_local").unwrap(),
            user_id: Some(UserId::parse("user_alice").unwrap()),
            workspace_id: WorkspaceId::parse("workspace_repo").unwrap(),
            run_id: pareto_protocol::RunId::parse(run).unwrap(),
            agent_id: AgentId::parse("agent_owner").unwrap(),
        };
        let revisions = revision_pins();
        let manifest = RunManifest {
            schema_ref: set.schema_ref("run-manifest").unwrap().clone(),
            scope: scope.clone(),
            revisions,
            hook_registry_config_digest: Some(Digest::parse(format!("sha256:{}", "e".repeat(64))).unwrap()),
            effect_registry_config_digest: Some(Digest::parse(format!("sha256:{}", "d".repeat(64))).unwrap()),
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
        let path = temp.path().join("lifecycle.sqlite3");
        Self {
            _temp: temp,
            path,
            set,
            limits,
            scope,
            manifest,
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
            plan_revision: self.manifest.plan_revision.clone(),
            budget_revision: self.manifest.budget_revision.clone(),
            boundary_recording_policy_ref: self.manifest.boundary_recording_policy_ref.clone(),
            execution_mode: self.manifest.execution_mode.clone(),
        }
    }

    fn target(&self) -> LifecycleTarget {
        LifecycleTarget {
            scope: self.scope.clone(),
            actor: self.scope.agent_id.clone(),
        }
    }

    fn registry(&self) -> SchemaRegistry {
        SchemaRegistry(vec![self.set.clone()])
    }

    fn create_run(&self, event: &str) -> CreateRunCommand {
        CreateRunCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-24T01:00:00.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            manifest: self.manifest.clone(),
        }
    }

    fn create_task(
        &self,
        event: &str,
        expected_sequence: i64,
        task: &str,
        parent: Option<&str>,
    ) -> CreateTaskCommand {
        CreateTaskCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-24T01:00:01.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            task_id: TaskId::parse(task).unwrap(),
            parent_task_id: parent.map(|value| TaskId::parse(value).unwrap()),
        }
    }

    fn transition_run(
        &self,
        event: &str,
        expected_sequence: i64,
        from: RunState,
        to: RunState,
    ) -> TransitionRunCommand {
        TransitionRunCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-24T01:00:02.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            expected_state: from,
            target_state: to,
            reason_code: "test-transition".to_owned(),
        }
    }

    fn transition_task(
        &self,
        event: &str,
        expected_sequence: i64,
        task: &str,
        from: TaskState,
        to: TaskState,
    ) -> TransitionTaskCommand {
        TransitionTaskCommand {
            event_id: EventId::parse(event).unwrap(),
            occurred_at: "2026-08-24T01:00:03.000Z".to_owned(),
            correlation_id: format!("corr-{event}"),
            expected_sequence,
            task_id: TaskId::parse(task).unwrap(),
            expected_state: from,
            target_state: to,
            reason_code: "test-transition".to_owned(),
        }
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

async fn open_created(fixture: &Fixture, event: &str) -> EventStore {
    let store = EventStore::open(&fixture.path).await.unwrap();
    store
        .create_run(&fixture.trusted(), &fixture.create_run(event))
        .await
        .unwrap();
    store
}

async fn event_count(store: &EventStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn manifest() {
    let fixture = Fixture::new("run_manifest");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let mut invalid_commands = Vec::new();
    for (index, role) in [
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
    .enumerate()
    {
        let mut command = fixture.create_run(&format!("event_manifest-pin-{index}"));
        command.manifest.revisions.insert(
            role.to_owned(),
            RevisionId::parse(format!("rev_other-{}", role.replace('_', "-"))).unwrap(),
        );
        invalid_commands.push(command);
    }
    let mut plan = fixture.create_run("event_manifest-plan");
    plan.manifest.plan_revision = Some(RevisionId::parse("rev_untrusted-plan").unwrap());
    invalid_commands.push(plan);
    let mut schema_set = fixture.create_run("event_manifest-schema-set");
    schema_set.manifest.schema_set_ref.manifest_digest =
        Digest::parse(format!("sha256:{}", "b".repeat(64))).unwrap();
    invalid_commands.push(schema_set);
    let mut budget = fixture.create_run("event_manifest-budget");
    budget.manifest.budget_revision = RevisionId::parse("rev_other-budget").unwrap();
    invalid_commands.push(budget);
    let mut limits = fixture.create_run("event_manifest-limits");
    limits.manifest.protocol_limits_ref.digest =
        Digest::parse(format!("sha256:{}", "c".repeat(64))).unwrap();
    invalid_commands.push(limits);
    let mut policy = fixture.create_run("event_manifest-policy");
    policy.manifest.boundary_recording_policy_ref.revision_id =
        RevisionId::parse("rev_other-policy").unwrap();
    invalid_commands.push(policy);
    let mut mode = fixture.create_run("event_manifest-mode");
    mode.manifest.execution_mode = ExecutionMode::Simulated {
        fixture_revisions: vec![RevisionId::parse("rev_fixture").unwrap()],
        simulation_origin: pareto_protocol::SimulationOrigin::Standalone,
        source_run_id: None,
    };
    invalid_commands.push(mode);
    let mut schema = fixture.create_run("event_manifest-schema");
    schema.manifest.schema_ref.r#type = "evidence-record".to_owned();
    invalid_commands.push(schema);
    for command in invalid_commands {
        assert_eq!(
            store
                .create_run(&fixture.trusted(), &command)
                .await
                .unwrap_err()
                .kind,
            LifecycleErrorKind::ManifestInvalid
        );
    }
    assert_eq!(event_count(&store).await, 0);
    let result = store
        .create_run(&fixture.trusted(), &fixture.create_run("event_run-create"))
        .await
        .unwrap();
    assert!(matches!(
        result,
        LifecycleResult::Applied {
            sequence: 1,
            state: AppliedState::Run(RunState::Created),
            ..
        }
    ));
    let mut transaction = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_established(&mut transaction, &fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(aggregate.state.manifest, fixture.manifest);
    assert_eq!(aggregate.state.run_state, RunState::Created);
    assert_eq!(aggregate.state.sequence, 1);
    drop(transaction);
    let user_version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let extra_authority_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND (name LIKE '%manifest%' OR name LIKE '%state%')",
    )
    .fetch_one(&store.pool)
    .await
    .unwrap();
    assert_eq!(user_version, 2);
    assert_eq!(extra_authority_tables, 0);
}

#[test]
fn state_machine() {
    let fixture = Fixture::new("run_state-machine");
    let mut state = LifecycleState {
        manifest: fixture.manifest,
        run_state: RunState::Created,
        tasks: BTreeMap::new(),
        sequence: 1,
    };
    assert_eq!(
        validate_run_transition(&state, RunState::Created, RunState::Running)
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    state.tasks.insert(
        TaskId::parse("task_root").unwrap(),
        TaskRecord {
            parent_task_id: None,
            state: TaskState::Ready,
        },
    );
    validate_run_transition(&state, RunState::Created, RunState::Running).unwrap();
    for terminal in [RunState::Succeeded, RunState::Failed, RunState::Cancelled] {
        assert_eq!(
            validate_run_transition(&state, terminal, RunState::Running)
                .unwrap_err()
                .kind,
            LifecycleErrorKind::TerminalStateConflict
        );
    }

    let run_states = [
        RunState::Created,
        RunState::Running,
        RunState::Paused,
        RunState::Succeeded,
        RunState::Failed,
        RunState::Cancelled,
    ];
    let expected_run_edges = [
        (RunState::Created, RunState::Running),
        (RunState::Created, RunState::Failed),
        (RunState::Created, RunState::Cancelled),
        (RunState::Running, RunState::Paused),
        (RunState::Running, RunState::Succeeded),
        (RunState::Running, RunState::Failed),
        (RunState::Running, RunState::Cancelled),
        (RunState::Paused, RunState::Running),
        (RunState::Paused, RunState::Failed),
        (RunState::Paused, RunState::Cancelled),
    ];
    for from in run_states {
        for to in run_states {
            assert_eq!(
                is_run_edge(from, to),
                expected_run_edges.contains(&(from, to)),
                "unexpected Run edge {from:?}->{to:?}"
            );
        }
    }

    let task_states = [
        TaskState::Created,
        TaskState::Ready,
        TaskState::Running,
        TaskState::Paused,
        TaskState::Succeeded,
        TaskState::Failed,
        TaskState::Cancelled,
    ];
    let expected_task_edges = [
        (TaskState::Created, TaskState::Ready),
        (TaskState::Created, TaskState::Failed),
        (TaskState::Created, TaskState::Cancelled),
        (TaskState::Ready, TaskState::Running),
        (TaskState::Ready, TaskState::Failed),
        (TaskState::Ready, TaskState::Cancelled),
        (TaskState::Running, TaskState::Paused),
        (TaskState::Running, TaskState::Succeeded),
        (TaskState::Running, TaskState::Failed),
        (TaskState::Running, TaskState::Cancelled),
        (TaskState::Paused, TaskState::Running),
        (TaskState::Paused, TaskState::Failed),
        (TaskState::Paused, TaskState::Cancelled),
    ];
    for from in task_states {
        for to in task_states {
            assert_eq!(
                is_task_edge(from, to),
                expected_task_edges.contains(&(from, to)),
                "unexpected Task edge {from:?}->{to:?}"
            );
        }
    }

    for (from, to, task_state) in [
        (RunState::Created, RunState::Running, TaskState::Ready),
        (RunState::Created, RunState::Failed, TaskState::Failed),
        (RunState::Created, RunState::Cancelled, TaskState::Cancelled),
        (RunState::Running, RunState::Paused, TaskState::Ready),
        (RunState::Running, RunState::Succeeded, TaskState::Succeeded),
        (RunState::Running, RunState::Failed, TaskState::Failed),
        (RunState::Running, RunState::Cancelled, TaskState::Cancelled),
        (RunState::Paused, RunState::Running, TaskState::Ready),
        (RunState::Paused, RunState::Failed, TaskState::Failed),
        (RunState::Paused, RunState::Cancelled, TaskState::Cancelled),
    ] {
        let mut guarded = state.clone();
        guarded.run_state = from;
        guarded.tasks.get_mut(&TaskId::parse("task_root").unwrap()).unwrap().state = task_state;
        validate_run_transition(&guarded, from, to).unwrap();
    }

    for (from, to, run_state) in [
        (TaskState::Created, TaskState::Ready, RunState::Created),
        (TaskState::Created, TaskState::Failed, RunState::Created),
        (TaskState::Created, TaskState::Cancelled, RunState::Created),
        (TaskState::Ready, TaskState::Running, RunState::Running),
        (TaskState::Ready, TaskState::Failed, RunState::Created),
        (TaskState::Ready, TaskState::Cancelled, RunState::Created),
        (TaskState::Running, TaskState::Paused, RunState::Running),
        (TaskState::Running, TaskState::Succeeded, RunState::Running),
        (TaskState::Running, TaskState::Failed, RunState::Running),
        (TaskState::Running, TaskState::Cancelled, RunState::Running),
        (TaskState::Paused, TaskState::Running, RunState::Running),
        (TaskState::Paused, TaskState::Failed, RunState::Running),
        (TaskState::Paused, TaskState::Cancelled, RunState::Running),
    ] {
        let mut guarded = state.clone();
        guarded.run_state = run_state;
        guarded.tasks.get_mut(&TaskId::parse("task_root").unwrap()).unwrap().state = from;
        validate_task_transition(
            &guarded,
            &TaskId::parse("task_root").unwrap(),
            from,
            to,
        )
        .unwrap();
    }
}

#[tokio::test]
async fn creation_atomicity() {
    let fixture = Fixture::new("run_atomicity");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let mut invalid = fixture.create_run("event_invalid-create");
    invalid.manifest.revisions.remove("kernel");
    assert_eq!(
        store
            .create_run(&fixture.trusted(), &invalid)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ManifestInvalid
    );
    assert_eq!(event_count(&store).await, 0);
    store
        .create_run(
            &fixture.trusted(),
            &fixture.create_run("event_atomic-create"),
        )
        .await
        .unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::from_str(&format!(
        "sqlite://{}",
        fixture.path.display()
    ))
    .unwrap();
    let mut fresh = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&mut fresh)
        .await
        .unwrap();
    assert_eq!(visible, 1);
}

#[tokio::test]
async fn hierarchy() {
    let fixture = Fixture::new("run_hierarchy");
    let store = open_created(&fixture, "event_hierarchy-create").await;
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_parent-create", 1, "task_parent", None),
        )
        .await
        .unwrap();
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task(
                "event_child-create",
                2,
                "task_child",
                Some("task_parent"),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .create_task(
                &fixture.registry(),
                &fixture.target(),
                &fixture.create_task(
                    "event_orphan-create",
                    3,
                    "task_orphan",
                    Some("task_missing"),
                ),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_child-ready",
                3,
                "task_child",
                TaskState::Created,
                TaskState::Ready,
            ),
        )
        .await
        .unwrap();
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_parent-ready",
                4,
                "task_parent",
                TaskState::Created,
                TaskState::Ready,
            ),
        )
        .await
        .unwrap();
    store
        .initialize_effect_stream(
            &fixture.registry(),
            &EffectTarget {
                scope: fixture.scope.clone(),
                actor: fixture.scope.agent_id.clone(),
            },
            &InitializeEffectStream {
                event_id: EventId::parse("event_hierarchy-effect-stream-init").unwrap(),
                occurred_at: "2026-08-24T01:00:03.500Z".to_owned(),
                correlation_id: "corr-hierarchy-effect-stream-init".to_owned(),
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
    store
        .transition_run(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_run(
                "event_run-start",
                5,
                RunState::Created,
                RunState::Running,
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .transition_task(
                &fixture.registry(),
                &fixture.target(),
                &fixture.transition_task(
                    "event_child-start-too-early",
                    6,
                    "task_child",
                    TaskState::Ready,
                    TaskState::Running,
                ),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_parent-start",
                6,
                "task_parent",
                TaskState::Ready,
                TaskState::Running,
            ),
        )
        .await
        .unwrap();
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_child-start",
                7,
                "task_child",
                TaskState::Ready,
                TaskState::Running,
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .transition_task(
                &fixture.registry(),
                &fixture.target(),
                &fixture.transition_task(
                    "event_parent-finish-too-early",
                    8,
                    "task_parent",
                    TaskState::Running,
                    TaskState::Succeeded,
                ),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_child-finish",
                8,
                "task_child",
                TaskState::Running,
                TaskState::Succeeded,
            ),
        )
        .await
        .unwrap();
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_parent-finish",
                9,
                "task_parent",
                TaskState::Running,
                TaskState::Succeeded,
            ),
        )
        .await
        .unwrap();
    store
        .transition_run(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_run(
                "event_run-finish",
                10,
                RunState::Running,
                RunState::Succeeded,
            ),
        )
        .await
        .unwrap();
    assert_eq!(event_count(&store).await, 12);
}

#[tokio::test]
async fn idempotency() {
    let fixture = Fixture::new("run_idempotency");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let create = fixture.create_run("event_idempotent-create");
    store.create_run(&fixture.trusted(), &create).await.unwrap();
    assert!(matches!(
        store.create_run(&fixture.trusted(), &create).await.unwrap(),
        LifecycleResult::AlreadyApplied { sequence: 1, .. }
    ));
    let mut mutated = create.clone();
    mutated.occurred_at = "2026-08-24T01:00:09.000Z".to_owned();
    assert_eq!(
        store
            .create_run(&fixture.trusted(), &mutated)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::IdempotencyConflict
    );
    let task = fixture.create_task("event_idempotent-task", 1, "task_one", None);
    store
        .create_task(&fixture.registry(), &fixture.target(), &task)
        .await
        .unwrap();
    assert!(matches!(
        store
            .create_task(&fixture.registry(), &fixture.target(), &task)
            .await
            .unwrap(),
        LifecycleResult::AlreadyApplied { sequence: 2, .. }
    ));
    let stale = fixture.create_task("event_stale-task", 1, "task_two", None);
    assert_eq!(
        store
            .create_task(&fixture.registry(), &fixture.target(), &stale)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::OptimisticConcurrencyConflict
    );

    let other = Fixture::new("run_other-idempotency");
    let other_store = EventStore::open(&other.path).await.unwrap();
    let mut other_create = other.create_run("event_idempotent-create");
    other_create.correlation_id = "corr-other".to_owned();
    assert!(other_store
        .create_run(&other.trusted(), &other_create)
        .await
        .is_ok());
    // Cross-aggregate reuse is tested in one database by targeting a second exact Run below.
    let mut second_scope = fixture.scope.clone();
    second_scope.run_id = pareto_protocol::RunId::parse("run_second").unwrap();
    let mut second_manifest = fixture.manifest.clone();
    second_manifest.scope = second_scope.clone();
    let mut second_trusted = fixture.trusted();
    second_trusted.scope = second_scope;
    let second_command = CreateRunCommand {
        event_id: task.event_id.clone(),
        occurred_at: "2026-08-24T01:00:10.000Z".to_owned(),
        correlation_id: "corr-cross-aggregate".to_owned(),
        manifest: second_manifest,
    };
    assert_eq!(
        store
            .create_run(&second_trusted, &second_command)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::IdempotencyConflict
    );
}

#[tokio::test]
async fn conflict_priority() {
    let fixture = Fixture::new("run_conflict-priority");
    let store = open_created(&fixture, "event_priority-create").await;
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_priority-task", 1, "task_existing", None),
        )
        .await
        .unwrap();

    for (event, task) in [
        ("event_priority-stale-existing", "task_existing"),
        ("event_priority-stale-missing", "task_missing"),
    ] {
        assert_eq!(
            store
                .transition_task(
                    &fixture.registry(),
                    &fixture.target(),
                    &fixture.transition_task(
                        event,
                        1,
                        task,
                        TaskState::Created,
                        TaskState::Ready,
                    ),
                )
                .await
                .unwrap_err()
                .kind,
            LifecycleErrorKind::OptimisticConcurrencyConflict
        );
    }
    assert_eq!(
        store
            .transition_task(
                &fixture.registry(),
                &fixture.target(),
                &fixture.transition_task(
                    "event_priority-current-missing",
                    2,
                    "task_missing",
                    TaskState::Created,
                    TaskState::Ready,
                ),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::InvalidTransition
    );
    assert_eq!(event_count(&store).await, 2);
}

#[tokio::test]
async fn sequence_boundaries() {
    let fixture = Fixture::new("run_sequence-boundaries");
    let store = open_created(&fixture, "event_sequence-create").await;
    let task = fixture.create_task("event_sequence-task", 1, "task_boundary", None);
    store
        .create_task(&fixture.registry(), &fixture.target(), &task)
        .await
        .unwrap();
    assert!(matches!(
        store
            .create_task(&fixture.registry(), &fixture.target(), &task)
            .await
            .unwrap(),
        LifecycleResult::AlreadyApplied { sequence: 2, .. }
    ));

    for (index, expected) in [i64::MAX, -1, 0].into_iter().enumerate() {
        let create = fixture.create_task(
            &format!("event_sequence-invalid-create-{index}"),
            expected,
            &format!("task_invalid-{index}"),
            None,
        );
        let run = fixture.transition_run(
            &format!("event_sequence-invalid-run-{index}"),
            expected,
            RunState::Created,
            RunState::Running,
        );
        let task = fixture.transition_task(
            &format!("event_sequence-invalid-task-{index}"),
            expected,
            "task_boundary",
            TaskState::Created,
            TaskState::Ready,
        );
        for result in [
            store
                .create_task(&fixture.registry(), &fixture.target(), &create)
                .await,
            store
                .transition_run(&fixture.registry(), &fixture.target(), &run)
                .await,
            store
                .transition_task(&fixture.registry(), &fixture.target(), &task)
                .await,
        ] {
            assert_eq!(
                result.unwrap_err().kind,
                LifecycleErrorKind::OptimisticConcurrencyConflict
            );
        }
    }

    for collision in [
        fixture.create_task("event_sequence-create", i64::MAX, "task_collision", None),
        fixture.create_task("event_sequence-task", -1, "task_collision", None),
    ] {
        assert_eq!(
            store
                .create_task(&fixture.registry(), &fixture.target(), &collision)
                .await
                .unwrap_err()
                .kind,
            LifecycleErrorKind::IdempotencyConflict
        );
    }
    assert_eq!(event_count(&store).await, 2);
}

#[tokio::test]
async fn terminal_and_late() {
    let fixture = Fixture::new("run_terminal");
    let store = open_created(&fixture, "event_terminal-create").await;
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_terminal-task", 1, "task_terminal", None),
        )
        .await
        .unwrap();
    let cancel_task = fixture.transition_task(
        "event_task-cancel",
        2,
        "task_terminal",
        TaskState::Created,
        TaskState::Cancelled,
    );
    store
        .transition_task(&fixture.registry(), &fixture.target(), &cancel_task)
        .await
        .unwrap();
    let cancel_run = fixture.transition_run(
        "event_run-cancel",
        3,
        RunState::Created,
        RunState::Cancelled,
    );
    store
        .transition_run(&fixture.registry(), &fixture.target(), &cancel_run)
        .await
        .unwrap();
    assert!(matches!(
        store
            .transition_task(&fixture.registry(), &fixture.target(), &cancel_task)
            .await
            .unwrap(),
        LifecycleResult::AlreadyApplied { sequence: 3, .. }
    ));
    assert!(matches!(
        store
            .transition_run(&fixture.registry(), &fixture.target(), &cancel_run)
            .await
            .unwrap(),
        LifecycleResult::AlreadyApplied { sequence: 4, .. }
    ));
    let late = fixture.transition_task(
        "event_late-success",
        4,
        "task_terminal",
        TaskState::Cancelled,
        TaskState::Succeeded,
    );
    assert_eq!(
        store
            .transition_task(&fixture.registry(), &fixture.target(), &late)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::TerminalStateConflict
    );
    assert_eq!(event_count(&store).await, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrency() {
    let fixture = Fixture::new("run_concurrency");
    let first = open_created(&fixture, "event_concurrency-create").await;
    first
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_concurrency-task", 1, "task_race", None),
        )
        .await
        .unwrap();
    first
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_concurrency-ready",
                2,
                "task_race",
                TaskState::Created,
                TaskState::Ready,
            ),
        )
        .await
        .unwrap();
    let second = EventStore::open_pinned(&fixture.path, &first.store_id)
        .await
        .unwrap();
    let stores = [Arc::new(first), Arc::new(second)];
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for (index, event) in ["event_race-left", "event_race-right"]
        .into_iter()
        .enumerate()
    {
        let store = stores[index].clone();
        let registry = fixture.registry();
        let target = fixture.target();
        let command = fixture.transition_run(
            event,
            3,
            RunState::Created,
            RunState::Running,
        );
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            store.transition_run(&registry, &target, &command).await
        }));
    }
    let contention_started = std::time::Instant::now();
    barrier.wait().await;
    let mut applied = 0;
    let mut stale = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(LifecycleResult::Applied { .. }) => applied += 1,
            Err(LifecycleError {
                kind: LifecycleErrorKind::OptimisticConcurrencyConflict,
            }) => stale += 1,
            other => panic!("unexpected race result: {other:?}"),
        }
    }
    assert_eq!((applied, stale), (1, 1));
    assert_eq!(event_count(&stores[0]).await, 4);
    eprintln!(
        "REQ-0005 observation: two-writer lifecycle contention={:?}",
        contention_started.elapsed()
    );
}

#[tokio::test]
async fn authority() {
    let fixture = Fixture::new("run_authority");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let mut wrong_actor = fixture.trusted();
    wrong_actor.actor = AgentId::parse("agent_intruder").unwrap();
    assert_eq!(
        store
            .create_run(
                &wrong_actor,
                &fixture.create_run("event_unauthorized-create"),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::Unauthorized
    );
    let mut wrong_pin = fixture.create_run("event_wrong-pin");
    wrong_pin.manifest.budget_revision = RevisionId::parse("rev_other-budget").unwrap();
    assert_eq!(
        store
            .create_run(&fixture.trusted(), &wrong_pin)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ManifestInvalid
    );
    store
        .create_run(
            &fixture.trusted(),
            &fixture.create_run("event_authorized-create"),
        )
        .await
        .unwrap();
    let mut target = fixture.target();
    target.actor = AgentId::parse("agent_intruder").unwrap();
    assert_eq!(
        store
            .create_task(
                &fixture.registry(),
                &target,
                &fixture.create_task("event_intruder-task", 1, "task_intruder", None),
            )
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::Unauthorized
    );
    assert_eq!(event_count(&store).await, 1);
}

#[tokio::test]
async fn isolation() {
    let fixture = Fixture::new("run_isolation");
    let store = open_created(&fixture, "event_isolation-create").await;
    let mut scopes = Vec::new();
    let mut tenant = fixture.scope.clone();
    tenant.tenant_id = TenantId::parse("tenant_other").unwrap();
    scopes.push(tenant);
    let mut user_presence = fixture.scope.clone();
    user_presence.user_id = None;
    scopes.push(user_presence);
    let mut user_value = fixture.scope.clone();
    user_value.user_id = Some(UserId::parse("user_other").unwrap());
    scopes.push(user_value);
    let mut workspace = fixture.scope.clone();
    workspace.workspace_id = WorkspaceId::parse("workspace_other").unwrap();
    scopes.push(workspace);
    let mut run = fixture.scope.clone();
    run.run_id = pareto_protocol::RunId::parse("run_other").unwrap();
    scopes.push(run);
    let mut agent = fixture.scope.clone();
    agent.agent_id = AgentId::parse("agent_other").unwrap();
    scopes.push(agent);
    for (index, scope) in scopes.into_iter().enumerate() {
        let target = LifecycleTarget {
            actor: scope.agent_id.clone(),
            scope,
        };
        assert_eq!(
            store
                .create_task(
                    &fixture.registry(),
                    &target,
                    &fixture.create_task(
                        &format!("event_isolation-{index}"),
                        1,
                        &format!("task_isolation-{index}"),
                        None,
                    ),
                )
                .await
                .unwrap_err()
                .kind,
            LifecycleErrorKind::Unauthorized
        );
    }
    assert_eq!(event_count(&store).await, 1);
}

#[tokio::test]
async fn transaction() {
    let fixture = Fixture::new("run_transaction");
    let store = open_created(&fixture, "event_transaction-create").await;
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_transaction-task", 1, "task_tx", None),
        )
        .await
        .unwrap();
    let invalid = fixture.transition_run(
        "event_transaction-invalid",
        2,
        RunState::Created,
        RunState::Running,
    );
    assert_eq!(
        store
            .transition_run(&fixture.registry(), &fixture.target(), &invalid)
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::ParentStateConflict
    );
    assert_eq!(event_count(&store).await, 2);
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_established(&mut tx, &fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(aggregate.state.sequence, 2);
    assert_eq!(aggregate.state.run_state, RunState::Created);
    assert_eq!(aggregate.state.tasks[&TaskId::parse("task_tx").unwrap()].state, TaskState::Created);
}

#[tokio::test]
async fn recovery() {
    let fixture = Fixture::new("run_recovery");
    let store = open_created(&fixture, "event_recovery-create").await;
    let store_id = store.store_id.clone();
    store
        .create_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.create_task("event_recovery-task", 1, "task_recovered", None),
        )
        .await
        .unwrap();
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_recovery-ready",
                2,
                "task_recovered",
                TaskState::Created,
                TaskState::Ready,
            ),
        )
        .await
        .unwrap();
    drop(store);
    let reopened = EventStore::open_pinned(&fixture.path, &store_id)
        .await
        .unwrap();
    let mut tx = reopened.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_established(&mut tx, &fixture.registry(), &fixture.target())
        .await
        .unwrap();
    assert_eq!(aggregate.state.manifest, fixture.manifest);
    assert_eq!(aggregate.state.sequence, 3);
    assert_eq!(aggregate.state.tasks[&TaskId::parse("task_recovered").unwrap()].state, TaskState::Ready);
}

#[tokio::test]
async fn compatibility() {
    let fixture = Fixture::new("run_compatibility");
    let store = open_created(&fixture, "event_compatibility-create").await;
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert_eq!(
        load_established(&mut tx, &SchemaRegistry(Vec::new()), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::SchemaUnavailable
    );
    drop(tx);

    let payload = RunStateTransitionedPayload {
        from: RunState::Created,
        to: RunState::Succeeded,
        reason_code: "corrupt-illegal-history".to_owned(),
    };
    let event = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &lifecycle_stream_id(&fixture.scope).unwrap(),
        &EventId::parse("event_illegal-history").unwrap(),
        2,
        "2026-08-24T01:00:20.000Z",
        "corr-illegal-history",
        "run-state-transitioned",
        &payload,
    )
    .unwrap();
    let prepared = PreparedEvent::new(&event, &fixture.set, &fixture.limits).unwrap();
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    insert_prepared(&mut tx, &prepared).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    assert_eq!(
        load_established(&mut tx, &fixture.registry(), &fixture.target())
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::AggregateCorrupt
    );

    let unknown = Fixture::new("run_unknown-major");
    let unknown_store = open_created(&unknown, "event_unknown-create").await;
    let payload = RunStateTransitionedPayload {
        from: RunState::Created,
        to: RunState::Cancelled,
        reason_code: "unknown-major".to_owned(),
    };
    let validated = lifecycle_event(
        &unknown.set,
        &unknown.limits,
        &unknown.scope,
        &unknown.scope.agent_id,
        &lifecycle_stream_id(&unknown.scope).unwrap(),
        &EventId::parse("event_unknown-major").unwrap(),
        2,
        "2026-08-24T01:00:21.000Z",
        "corr-unknown-major",
        "run-state-transitioned",
        &payload,
    )
    .unwrap();
    let mut envelope = validated.envelope().clone();
    envelope.event_major = 2;
    let envelope_json = canonical(&envelope).unwrap();
    let schema_set_json = canonical(unknown.set.reference()).unwrap();
    let limits_json = canonical(&unknown.limits).unwrap();
    let prepared = PreparedEvent {
        envelope,
        envelope_fingerprint: fingerprint(envelope_json.as_bytes()),
        schema_set_fingerprint: fingerprint(schema_set_json.as_bytes()),
        limits_fingerprint: fingerprint(limits_json.as_bytes()),
        envelope_json,
        schema_set_json,
        limits_json,
        sequence: 2,
    };
    let mut tx = unknown_store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    insert_prepared(&mut tx, &prepared).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = unknown_store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    assert_eq!(
        load_established(&mut tx, &unknown.registry(), &unknown.target())
            .await
            .unwrap_err()
            .kind,
        LifecycleErrorKind::AggregateCorrupt
    );
}

fn base_fold_events(fixture: &Fixture) -> Vec<ValidatedEvent> {
    let stream = lifecycle_stream_id(&fixture.scope).unwrap();
    let created = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_fold-create").unwrap(),
        1,
        "2026-08-24T01:01:00.000Z",
        "corr-fold-create",
        "run-created",
        &RunCreatedPayload {
            manifest: fixture.manifest.clone(),
        },
    )
    .unwrap();
    let task = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_fold-task").unwrap(),
        2,
        "2026-08-24T01:01:01.000Z",
        "corr-fold-task",
        "task-created",
        &TaskCreatedPayload {
            task_id: TaskId::parse("task_model").unwrap(),
            parent_task_id: None,
            initial_state: TaskState::Created,
        },
    )
    .unwrap();
    vec![created, task]
}

#[test]
fn fold_contract() {
    let fixture = Fixture::new("run_fold-contract");
    let events = base_fold_events(&fixture);
    let first = fold_lifecycle(&fixture.set, &events).unwrap();
    let second = fold_lifecycle(&fixture.set, &events).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        fingerprint(canonical(&first).unwrap().as_bytes()),
        fingerprint(canonical(&second).unwrap().as_bytes())
    );
    assert_eq!(first.manifest, fixture.manifest);
    assert_eq!(first.sequence, 2);

    let stream = lifecycle_stream_id(&fixture.scope).unwrap();
    let gap = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_fold-gap").unwrap(),
        4,
        "2026-08-24T01:01:04.000Z",
        "corr-fold-gap",
        "task-state-transitioned",
        &TaskStateTransitionedPayload {
            task_id: TaskId::parse("task_model").unwrap(),
            from: TaskState::Created,
            to: TaskState::Ready,
            reason_code: "gap".to_owned(),
        },
    )
    .unwrap();
    let mut corrupt = events;
    corrupt.push(gap);
    assert_eq!(
        fold_lifecycle(&fixture.set, &corrupt).unwrap_err().kind,
        LifecycleErrorKind::AggregateCorrupt
    );
}

fn fold_task_event(
    fixture: &Fixture,
    scope: &IsolationScope,
    actor: &AgentId,
    stream: &StreamId,
    event_id: &str,
) -> ValidatedEvent {
    lifecycle_event(
        &fixture.set,
        &fixture.limits,
        scope,
        actor,
        stream,
        &EventId::parse(event_id).unwrap(),
        2,
        "2026-08-24T01:01:01.000Z",
        &format!("corr-{event_id}"),
        "task-created",
        &TaskCreatedPayload {
            task_id: TaskId::parse("task_mixed").unwrap(),
            parent_task_id: None,
            initial_state: TaskState::Created,
        },
    )
    .unwrap()
}

#[test]
fn fold_identity() {
    let fixture = Fixture::new("run_fold-identity");
    let mut first = base_fold_events(&fixture).remove(0);
    let mut variants = Vec::new();

    let mut tenant = fixture.scope.clone();
    tenant.tenant_id = TenantId::parse("tenant_mixed").unwrap();
    variants.push(tenant);
    let mut user_presence = fixture.scope.clone();
    user_presence.user_id = None;
    variants.push(user_presence);
    let mut user_value = fixture.scope.clone();
    user_value.user_id = Some(UserId::parse("user_mixed").unwrap());
    variants.push(user_value);
    let mut workspace = fixture.scope.clone();
    workspace.workspace_id = WorkspaceId::parse("workspace_mixed").unwrap();
    variants.push(workspace);
    let mut run = fixture.scope.clone();
    run.run_id = pareto_protocol::RunId::parse("run_mixed").unwrap();
    variants.push(run);
    let mut agent = fixture.scope.clone();
    agent.agent_id = AgentId::parse("agent_mixed").unwrap();
    variants.push(agent);

    for (index, scope) in variants.into_iter().enumerate() {
        let stream = lifecycle_stream_id(&scope).unwrap();
        let second = fold_task_event(
            &fixture,
            &scope,
            &scope.agent_id,
            &stream,
            &format!("event_fold-mixed-scope-{index}"),
        );
        assert_eq!(
            fold_lifecycle(&fixture.set, &[first, second])
                .unwrap_err()
                .kind,
            LifecycleErrorKind::AggregateCorrupt
        );
        first = base_fold_events(&fixture).remove(0);
    }

    let other_actor = AgentId::parse("agent_other-actor").unwrap();
    let stream = lifecycle_stream_id(&fixture.scope).unwrap();
    let actor_mixed = fold_task_event(
        &fixture,
        &fixture.scope,
        &other_actor,
        &stream,
        "event_fold-mixed-actor",
    );
    assert_eq!(
        fold_lifecycle(&fixture.set, &[first, actor_mixed])
            .unwrap_err()
            .kind,
        LifecycleErrorKind::AggregateCorrupt
    );

    let first = base_fold_events(&fixture).remove(0);
    let other_stream = StreamId::parse("stream_lifecycle-alternate").unwrap();
    let stream_mixed = fold_task_event(
        &fixture,
        &fixture.scope,
        &fixture.scope.agent_id,
        &other_stream,
        "event_fold-mixed-stream",
    );
    assert_eq!(
        fold_lifecycle(&fixture.set, &[first, stream_mixed])
            .unwrap_err()
            .kind,
        LifecycleErrorKind::AggregateCorrupt
    );

    let mut mismatched_manifest = fixture.manifest.clone();
    mismatched_manifest.scope.workspace_id = WorkspaceId::parse("workspace_payload").unwrap();
    let mismatched_first = lifecycle_event(
        &fixture.set,
        &fixture.limits,
        &fixture.scope,
        &fixture.scope.agent_id,
        &stream,
        &EventId::parse("event_fold-mismatched-manifest").unwrap(),
        1,
        "2026-08-24T01:01:00.000Z",
        "corr-fold-mismatched-manifest",
        "run-created",
        &RunCreatedPayload {
            manifest: mismatched_manifest,
        },
    )
    .unwrap();
    assert_eq!(
        fold_lifecycle(&fixture.set, &[mismatched_first])
            .unwrap_err()
            .kind,
        LifecycleErrorKind::AggregateCorrupt
    );

    let mut wrong_schema = fixture.manifest.clone();
    wrong_schema.schema_ref = fixture.set.schema_ref("evidence-record").unwrap().clone();
    let mut self_replay = fixture.manifest.clone();
    self_replay.execution_mode = ExecutionMode::RecordedReplay {
        source_run_id: fixture.scope.run_id.clone(),
        boundary_inventory_revision: RevisionId::parse("rev_inventory").unwrap(),
    };
    let mut invalid_derived = fixture.manifest.clone();
    invalid_derived.execution_mode = ExecutionMode::Simulated {
        fixture_revisions: vec![RevisionId::parse("rev_fixture").unwrap()],
        simulation_origin: pareto_protocol::SimulationOrigin::Derived,
        source_run_id: None,
    };
    for (index, manifest) in [wrong_schema, self_replay, invalid_derived]
        .into_iter()
        .enumerate()
    {
        let admitted_event = lifecycle_event(
            &fixture.set,
            &fixture.limits,
            &fixture.scope,
            &fixture.scope.agent_id,
            &stream,
            &EventId::parse(format!("event_fold-invalid-manifest-{index}")).unwrap(),
            1,
            "2026-08-24T01:01:00.000Z",
            &format!("corr-fold-invalid-manifest-{index}"),
            "run-created",
            &RunCreatedPayload { manifest },
        )
        .expect("event JSON admission intentionally precedes Manifest semantics");
        assert_eq!(
            fold_lifecycle(&fixture.set, &[admitted_event])
                .unwrap_err()
                .kind,
            LifecycleErrorKind::AggregateCorrupt
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum ModelAction {
    TaskReady,
    RunStart,
    TaskStart,
    TaskSuccess,
    RunSuccess,
    TaskCancel,
    RunCancel,
}

fn reference_apply(
    run: &mut RunState,
    task: &mut TaskState,
    action: ModelAction,
) -> bool {
    match action {
        ModelAction::TaskReady if *run == RunState::Created && *task == TaskState::Created => {
            *task = TaskState::Ready;
            true
        }
        ModelAction::RunStart if *run == RunState::Created && *task == TaskState::Ready => {
            *run = RunState::Running;
            true
        }
        ModelAction::TaskStart if *run == RunState::Running && *task == TaskState::Ready => {
            *task = TaskState::Running;
            true
        }
        ModelAction::TaskSuccess if *run == RunState::Running && *task == TaskState::Running => {
            *task = TaskState::Succeeded;
            true
        }
        ModelAction::RunSuccess if *run == RunState::Running && *task == TaskState::Succeeded => {
            *run = RunState::Succeeded;
            true
        }
        ModelAction::TaskCancel
            if !is_run_terminal(*run)
                && matches!(*task, TaskState::Created | TaskState::Ready | TaskState::Running) =>
        {
            *task = TaskState::Cancelled;
            true
        }
        ModelAction::RunCancel
            if matches!(*run, RunState::Created | RunState::Running | RunState::Paused)
                && *task == TaskState::Cancelled =>
        {
            *run = RunState::Cancelled;
            true
        }
        _ => false,
    }
}

#[test]
fn model_sequences() {
    let fixture = Fixture::new("run_model-sequences");
    let alphabet = [
        ModelAction::TaskReady,
        ModelAction::RunStart,
        ModelAction::TaskStart,
        ModelAction::TaskSuccess,
        ModelAction::RunSuccess,
        ModelAction::TaskCancel,
        ModelAction::RunCancel,
    ];
    let stream = lifecycle_stream_id(&fixture.scope).unwrap();
    for left in alphabet {
        for middle in alphabet {
            for right in alphabet {
                let mut events = base_fold_events(&fixture);
                let mut run = RunState::Created;
                let mut task = TaskState::Created;
                for (index, action) in [left, middle, right].into_iter().enumerate() {
                    let from_run = run;
                    let from_task = task;
                    let valid = reference_apply(&mut run, &mut task, action);
                    let sequence = i64::try_from(events.len() + 1).unwrap();
                    let suffix = format!("model-{}-{}-{}-{index}", left as u8, middle as u8, right as u8);
                    let event = match action {
                        ModelAction::RunStart | ModelAction::RunSuccess | ModelAction::RunCancel => {
                            let to = match action {
                                ModelAction::RunStart => RunState::Running,
                                ModelAction::RunSuccess => RunState::Succeeded,
                                ModelAction::RunCancel => RunState::Cancelled,
                                _ => unreachable!(),
                            };
                            lifecycle_event(
                                &fixture.set, &fixture.limits, &fixture.scope,
                                &fixture.scope.agent_id, &stream,
                                &EventId::parse(format!("event_{suffix}")).unwrap(), sequence,
                                "2026-08-24T01:02:00.000Z", &format!("corr-{suffix}"),
                                "run-state-transitioned",
                                &RunStateTransitionedPayload { from: from_run, to, reason_code: "model".to_owned() },
                            ).unwrap()
                        }
                        _ => {
                            let to = match action {
                                ModelAction::TaskReady => TaskState::Ready,
                                ModelAction::TaskStart => TaskState::Running,
                                ModelAction::TaskSuccess => TaskState::Succeeded,
                                ModelAction::TaskCancel => TaskState::Cancelled,
                                _ => unreachable!(),
                            };
                            lifecycle_event(
                                &fixture.set, &fixture.limits, &fixture.scope,
                                &fixture.scope.agent_id, &stream,
                                &EventId::parse(format!("event_{suffix}")).unwrap(), sequence,
                                "2026-08-24T01:02:00.000Z", &format!("corr-{suffix}"),
                                "task-state-transitioned",
                                &TaskStateTransitionedPayload { task_id: TaskId::parse("task_model").unwrap(), from: from_task, to, reason_code: "model".to_owned() },
                            ).unwrap()
                        }
                    };
                    events.push(event);
                    let folded = fold_lifecycle(&fixture.set, &events);
                    assert_eq!(folded.is_ok(), valid, "model divergence for {left:?}/{middle:?}/{right:?} at {index}");
                    if !valid {
                        break;
                    }
                    let folded = folded.unwrap();
                    assert_eq!(folded.run_state, run);
                    assert_eq!(folded.tasks[&TaskId::parse("task_model").unwrap()].state, task);
                }
            }
        }
    }
}

#[tokio::test]
async fn performance_observation() {
    let fixture = Fixture::new("run_performance");
    let store = EventStore::open(&fixture.path).await.unwrap();
    let create_started = std::time::Instant::now();
    store
        .create_run(
            &fixture.trusted(),
            &fixture.create_run("event_performance-create"),
        )
        .await
        .unwrap();
    let create_elapsed = create_started.elapsed();
    for index in 0..20 {
        store
            .create_task(
                &fixture.registry(),
                &fixture.target(),
                &fixture.create_task(
                    &format!("event_performance-task-{index}"),
                    i64::from(index) + 1,
                    &format!("task_performance-{index}"),
                    None,
                ),
            )
            .await
            .unwrap();
    }
    let transition_started = std::time::Instant::now();
    store
        .transition_task(
            &fixture.registry(),
            &fixture.target(),
            &fixture.transition_task(
                "event_performance-transition",
                21,
                "task_performance-0",
                TaskState::Created,
                TaskState::Ready,
            ),
        )
        .await
        .unwrap();
    let transition_elapsed = transition_started.elapsed();
    let fold_started = std::time::Instant::now();
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let aggregate = load_established(&mut tx, &fixture.registry(), &fixture.target())
        .await
        .unwrap();
    let fold_elapsed = fold_started.elapsed();
    assert_eq!(aggregate.state.sequence, 22);
    eprintln!(
        "REQ-0005 observation: create={create_elapsed:?}, transition_at_21_events={transition_elapsed:?}, exact_reader_fold_22_events={fold_elapsed:?}"
    );
}
