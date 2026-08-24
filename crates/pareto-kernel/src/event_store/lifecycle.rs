use std::collections::BTreeMap;
use std::sync::Arc;

use pareto_protocol::{
    AgentId, BoundaryRecordingPolicyRef, EventEnvelope, EventId, ExecutionMode, IsolationScope,
    ProtocolLimitsRef, RevisionId, RunCreatedPayload, RunManifest, RunState,
    RunStateTransitionedPayload, SchemaSet, StreamId, TaskCreatedPayload, TaskId, TaskState,
    TaskStateTransitionedPayload, ValidatedEvent, digest_json,
};
use sqlx::{Row, SqliteConnection};

use super::{
    AppendResult, ErrorKind, EventStore, EventStoreError, PreparedEvent, SchemaRegistry,
    check_prepared_idempotency, insert_prepared, user_key, validate_row,
};

const ROW_COLUMNS: &str = "envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleErrorKind {
    ManifestInvalid,
    Unauthorized,
    AggregateNotFound,
    AggregateCorrupt,
    InvalidTransition,
    ParentStateConflict,
    TerminalStateConflict,
    OptimisticConcurrencyConflict,
    IdempotencyConflict,
    SchemaUnavailable,
    Busy,
    Io,
}

#[derive(Debug)]
struct LifecycleError {
    kind: LifecycleErrorKind,
}

impl LifecycleError {
    fn new(kind: LifecycleErrorKind) -> Self {
        Self { kind }
    }
}

impl From<EventStoreError> for LifecycleError {
    fn from(error: EventStoreError) -> Self {
        let kind = match error.kind {
            ErrorKind::IdempotencyConflict => LifecycleErrorKind::IdempotencyConflict,
            ErrorKind::SequenceConflict => LifecycleErrorKind::OptimisticConcurrencyConflict,
            ErrorKind::Busy => LifecycleErrorKind::Busy,
            ErrorKind::Io => LifecycleErrorKind::Io,
            ErrorKind::ProtocolInvalid
            | ErrorKind::IsolationConflict
            | ErrorKind::CausationConflict
            | ErrorKind::DatabaseCorrupt
            | ErrorKind::Migration => LifecycleErrorKind::AggregateCorrupt,
        };
        Self::new(kind)
    }
}

impl From<sqlx::Error> for LifecycleError {
    fn from(error: sqlx::Error) -> Self {
        EventStoreError::from(error).into()
    }
}

#[derive(Clone)]
struct TrustedRunInputs {
    scope: IsolationScope,
    actor: AgentId,
    schema_set: Arc<SchemaSet>,
    protocol_limits_ref: ProtocolLimitsRef,
    revisions: BTreeMap<String, RevisionId>,
    plan_revision: Option<RevisionId>,
    budget_revision: RevisionId,
    boundary_recording_policy_ref: BoundaryRecordingPolicyRef,
    execution_mode: ExecutionMode,
}

#[derive(Clone)]
struct LifecycleTarget {
    scope: IsolationScope,
    actor: AgentId,
}

#[derive(Clone)]
struct CreateRunCommand {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    manifest: RunManifest,
}

#[derive(Clone)]
struct CreateTaskCommand {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    expected_sequence: i64,
    task_id: TaskId,
    parent_task_id: Option<TaskId>,
}

#[derive(Clone)]
struct TransitionRunCommand {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    expected_sequence: i64,
    expected_state: RunState,
    target_state: RunState,
    reason_code: String,
}

#[derive(Clone)]
struct TransitionTaskCommand {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    expected_sequence: i64,
    task_id: TaskId,
    expected_state: TaskState,
    target_state: TaskState,
    reason_code: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppliedState {
    Run(RunState),
    Task(TaskState),
}

#[derive(Debug, Eq, PartialEq)]
enum LifecycleResult {
    Applied {
        event_id: EventId,
        sequence: i64,
        state: AppliedState,
    },
    AlreadyApplied {
        event_id: EventId,
        sequence: i64,
        state: AppliedState,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskRecord {
    parent_task_id: Option<TaskId>,
    state: TaskState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LifecycleState {
    manifest: RunManifest,
    run_state: RunState,
    tasks: BTreeMap<TaskId, TaskRecord>,
    sequence: i64,
}

#[derive(Debug)]
struct EstablishedAggregate {
    state: LifecycleState,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
    stream_id: StreamId,
}

impl EventStore {
    async fn create_run(
        &self,
        trusted: &TrustedRunInputs,
        command: &CreateRunCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        validate_create_authority(trusted, &command.manifest)?;
        let stream_id = lifecycle_stream_id(&trusted.scope)?;
        let validated_manifest = trusted
            .schema_set
            .validate_run_manifest(command.manifest.clone(), &trusted.scope)
            .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))?;
        let payload = RunCreatedPayload {
            manifest: validated_manifest.into_inner(),
        };
        let event = lifecycle_event(
            &trusted.schema_set,
            &trusted.protocol_limits_ref,
            &trusted.scope,
            &trusted.actor,
            &stream_id,
            &command.event_id,
            1,
            &command.occurred_at,
            &command.correlation_id,
            "run-created",
            &payload,
        )?;
        let prepared =
            PreparedEvent::new(&event, &trusted.schema_set, &trusted.protocol_limits_ref)?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(result) = check_prepared_idempotency(&mut transaction, &prepared).await? {
            transaction.commit().await?;
            return Ok(result_for(result, AppliedState::Run(RunState::Created)));
        }
        let count = aggregate_event_count(&mut transaction, &trusted.scope, &stream_id).await?;
        if count != 0 {
            return Err(LifecycleError::new(
                LifecycleErrorKind::OptimisticConcurrencyConflict,
            ));
        }
        let result = insert_prepared(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        Ok(result_for(result, AppliedState::Run(RunState::Created)))
    }

    async fn create_task(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &CreateTaskCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let payload = TaskCreatedPayload {
            task_id: command.task_id.clone(),
            parent_task_id: command.parent_task_id.clone(),
            initial_state: TaskState::Created,
        };
        let event = lifecycle_event(
            &aggregate.schema_set,
            &aggregate.limits,
            &target.scope,
            &target.actor,
            &aggregate.stream_id,
            &command.event_id,
            command.expected_sequence + 1,
            &command.occurred_at,
            &command.correlation_id,
            "task-created",
            &payload,
        )?;
        let prepared = PreparedEvent::new(&event, &aggregate.schema_set, &aggregate.limits)?;
        if let Some(result) = check_prepared_idempotency(&mut transaction, &prepared).await? {
            transaction.commit().await?;
            return Ok(result_for(result, AppliedState::Task(TaskState::Created)));
        }
        if aggregate.state.sequence != command.expected_sequence {
            return Err(LifecycleError::new(
                LifecycleErrorKind::OptimisticConcurrencyConflict,
            ));
        }
        validate_task_creation(&aggregate.state, &payload)?;
        let result = insert_prepared(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        Ok(result_for(result, AppliedState::Task(TaskState::Created)))
    }

    async fn transition_run(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &TransitionRunCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let payload = RunStateTransitionedPayload {
            from: command.expected_state,
            to: command.target_state,
            reason_code: command.reason_code.clone(),
        };
        let event = lifecycle_event(
            &aggregate.schema_set,
            &aggregate.limits,
            &target.scope,
            &target.actor,
            &aggregate.stream_id,
            &command.event_id,
            command.expected_sequence + 1,
            &command.occurred_at,
            &command.correlation_id,
            "run-state-transitioned",
            &payload,
        )?;
        let prepared = PreparedEvent::new(&event, &aggregate.schema_set, &aggregate.limits)?;
        if let Some(result) = check_prepared_idempotency(&mut transaction, &prepared).await? {
            transaction.commit().await?;
            return Ok(result_for(result, AppliedState::Run(command.target_state)));
        }
        validate_expected(
            aggregate.state.sequence,
            command.expected_sequence,
            aggregate.state.run_state == command.expected_state,
        )?;
        validate_run_transition(
            &aggregate.state,
            command.expected_state,
            command.target_state,
        )?;
        let result = insert_prepared(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        Ok(result_for(result, AppliedState::Run(command.target_state)))
    }

    async fn transition_task(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &TransitionTaskCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let payload = TaskStateTransitionedPayload {
            task_id: command.task_id.clone(),
            from: command.expected_state,
            to: command.target_state,
            reason_code: command.reason_code.clone(),
        };
        let event = lifecycle_event(
            &aggregate.schema_set,
            &aggregate.limits,
            &target.scope,
            &target.actor,
            &aggregate.stream_id,
            &command.event_id,
            command.expected_sequence + 1,
            &command.occurred_at,
            &command.correlation_id,
            "task-state-transitioned",
            &payload,
        )?;
        let prepared = PreparedEvent::new(&event, &aggregate.schema_set, &aggregate.limits)?;
        if let Some(result) = check_prepared_idempotency(&mut transaction, &prepared).await? {
            transaction.commit().await?;
            return Ok(result_for(result, AppliedState::Task(command.target_state)));
        }
        let current = aggregate
            .state
            .tasks
            .get(&command.task_id)
            .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::InvalidTransition))?;
        validate_expected(
            aggregate.state.sequence,
            command.expected_sequence,
            current.state == command.expected_state,
        )?;
        validate_task_transition(
            &aggregate.state,
            &command.task_id,
            command.expected_state,
            command.target_state,
        )?;
        let result = insert_prepared(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        Ok(result_for(result, AppliedState::Task(command.target_state)))
    }
}

fn result_for(result: AppendResult, state: AppliedState) -> LifecycleResult {
    match result {
        AppendResult::Appended { event_id, sequence } => LifecycleResult::Applied {
            event_id,
            sequence,
            state,
        },
        AppendResult::AlreadyCommitted { event_id, sequence } => LifecycleResult::AlreadyApplied {
            event_id,
            sequence,
            state,
        },
    }
}

fn validate_create_authority(
    trusted: &TrustedRunInputs,
    manifest: &RunManifest,
) -> Result<(), LifecycleError> {
    if trusted.actor != trusted.scope.agent_id || manifest.scope != trusted.scope {
        return Err(LifecycleError::new(LifecycleErrorKind::Unauthorized));
    }
    let exact = manifest.revisions == trusted.revisions
        && manifest.plan_revision == trusted.plan_revision
        && manifest.schema_set_ref == *trusted.schema_set.reference()
        && manifest.budget_revision == trusted.budget_revision
        && manifest.protocol_limits_ref == trusted.protocol_limits_ref
        && manifest.boundary_recording_policy_ref == trusted.boundary_recording_policy_ref
        && manifest.execution_mode == trusted.execution_mode;
    if exact {
        Ok(())
    } else {
        Err(LifecycleError::new(LifecycleErrorKind::ManifestInvalid))
    }
}

fn lifecycle_stream_id(scope: &IsolationScope) -> Result<StreamId, LifecycleError> {
    let suffix = scope
        .run_id
        .as_str()
        .strip_prefix("run_")
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))?;
    StreamId::parse(format!("stream_lifecycle-{suffix}"))
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))
}

#[allow(clippy::too_many_arguments)]
fn lifecycle_event<T: serde::Serialize>(
    schema_set: &SchemaSet,
    limits: &ProtocolLimitsRef,
    scope: &IsolationScope,
    actor: &AgentId,
    stream_id: &StreamId,
    event_id: &EventId,
    sequence: i64,
    occurred_at: &str,
    correlation_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<ValidatedEvent, LifecycleError> {
    let binding = schema_set
        .event_type_binding(event_type, 1, 0)
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::SchemaUnavailable))?;
    let envelope_schema = schema_set
        .schema_ref("event-envelope")
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::SchemaUnavailable))?;
    let payload = serde_json::to_value(payload)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))?;
    let payload_digest = digest_json("event-payload", &binding.payload_schema_ref, &payload)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))?;
    let envelope = EventEnvelope {
        schema_ref: envelope_schema.clone(),
        scope: scope.clone(),
        event_id: event_id.clone(),
        stream_id: stream_id.clone(),
        run_id: scope.run_id.clone(),
        sequence: sequence.to_string(),
        causation_id: None,
        correlation_id: correlation_id.to_owned(),
        event_type: event_type.to_owned(),
        event_major: 1,
        event_minor: 0,
        occurred_at: occurred_at.to_owned(),
        actor: actor.clone(),
        payload_schema_ref: binding.payload_schema_ref.clone(),
        payload_digest,
        payload,
    };
    schema_set
        .validate_event_at_boundary(
            envelope,
            scope.clone(),
            actor.clone(),
            stream_id.clone(),
            limits.clone(),
        )
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))
}

async fn load_established(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    target: &LifecycleTarget,
) -> Result<EstablishedAggregate, LifecycleError> {
    let stream_id = lifecycle_stream_id(&target.scope)?;
    let (present, user) = user_key(&target.scope);
    let first_sql = format!(
        "SELECT {ROW_COLUMNS} FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? AND sequence_i64=1"
    );
    let first = sqlx::query(&first_sql)
        .bind(target.scope.tenant_id.as_str())
        .bind(present)
        .bind(user)
        .bind(target.scope.workspace_id.as_str())
        .bind(target.scope.run_id.as_str())
        .bind(target.scope.agent_id.as_str())
        .bind(stream_id.as_str())
        .fetch_optional(&mut *connection)
        .await?
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::Unauthorized))?;
    let schema_ref = serde_json::from_str(&first.get::<String, _>(2))
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    let limits: ProtocolLimitsRef = serde_json::from_str(&first.get::<String, _>(4))
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    let schema_set = registry
        .0
        .iter()
        .find(|set| set.reference() == &schema_ref)
        .cloned()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::SchemaUnavailable))?;
    let read = super::AdmittedRead {
        scope: target.scope.clone(),
        stream_id: Some(stream_id.clone()),
        schema_set: schema_set.clone(),
        limits: limits.clone(),
    };
    let first_event = validate_row(&first, &read)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    let first_payload = first_event
        .downcast_payload::<RunCreatedPayload>()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    if first_event.envelope().event_type != "run-created"
        || first_event.envelope().sequence != "1"
        || first_payload.manifest.scope != target.scope
        || first_payload.manifest.schema_set_ref != *schema_set.reference()
        || first_payload.manifest.protocol_limits_ref != limits
    {
        return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
    }
    schema_set
        .validate_run_manifest(first_payload.manifest.clone(), &target.scope)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    if target.actor != first_payload.manifest.scope.agent_id {
        return Err(LifecycleError::new(LifecycleErrorKind::Unauthorized));
    }

    let all_sql = format!(
        "SELECT {ROW_COLUMNS} FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? ORDER BY sequence_i64"
    );
    let rows = sqlx::query(&all_sql)
        .bind(target.scope.tenant_id.as_str())
        .bind(present)
        .bind(user)
        .bind(target.scope.workspace_id.as_str())
        .bind(target.scope.run_id.as_str())
        .bind(target.scope.agent_id.as_str())
        .bind(stream_id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        events.push(
            validate_row(&row, &read)
                .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?,
        );
    }
    let state = fold_lifecycle(&events)?;
    Ok(EstablishedAggregate {
        state,
        schema_set,
        limits,
        stream_id,
    })
}

async fn aggregate_event_count(
    connection: &mut SqliteConnection,
    scope: &IsolationScope,
    stream_id: &StreamId,
) -> Result<i64, LifecycleError> {
    let (present, user) = user_key(scope);
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=?")
        .bind(scope.tenant_id.as_str()).bind(present).bind(user)
        .bind(scope.workspace_id.as_str()).bind(scope.run_id.as_str())
        .bind(scope.agent_id.as_str()).bind(stream_id.as_str())
        .fetch_one(&mut *connection).await?)
}

fn fold_lifecycle(events: &[ValidatedEvent]) -> Result<LifecycleState, LifecycleError> {
    let first = events
        .first()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateNotFound))?;
    let created = first
        .downcast_payload::<RunCreatedPayload>()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    if first.envelope().event_type != "run-created" || first.envelope().sequence != "1" {
        return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
    }
    let mut state = LifecycleState {
        manifest: created.manifest.clone(),
        run_state: RunState::Created,
        tasks: BTreeMap::new(),
        sequence: 1,
    };
    for (index, event) in events.iter().enumerate().skip(1) {
        let expected_sequence = i64::try_from(index + 1)
            .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
        if event.envelope().sequence.parse::<i64>().ok() != Some(expected_sequence) {
            return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
        }
        match event.envelope().event_type.as_str() {
            "task-created" => {
                let payload = event
                    .downcast_payload::<TaskCreatedPayload>()
                    .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                validate_task_creation(&state, payload)
                    .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                state.tasks.insert(
                    payload.task_id.clone(),
                    TaskRecord {
                        parent_task_id: payload.parent_task_id.clone(),
                        state: TaskState::Created,
                    },
                );
            }
            "run-state-transitioned" => {
                let payload = event
                    .downcast_payload::<RunStateTransitionedPayload>()
                    .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                if state.run_state != payload.from {
                    return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
                }
                validate_run_transition(&state, payload.from, payload.to)
                    .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                state.run_state = payload.to;
            }
            "task-state-transitioned" => {
                let payload = event
                    .downcast_payload::<TaskStateTransitionedPayload>()
                    .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                let current = state
                    .tasks
                    .get(&payload.task_id)
                    .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                if current.state != payload.from {
                    return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
                }
                validate_task_transition(&state, &payload.task_id, payload.from, payload.to)
                    .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
                state
                    .tasks
                    .get_mut(&payload.task_id)
                    .expect("task checked")
                    .state = payload.to;
            }
            _ => return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt)),
        }
        state.sequence = expected_sequence;
    }
    Ok(state)
}

fn validate_expected(
    actual_sequence: i64,
    expected_sequence: i64,
    state_matches: bool,
) -> Result<(), LifecycleError> {
    if actual_sequence == expected_sequence && state_matches {
        Ok(())
    } else {
        Err(LifecycleError::new(
            LifecycleErrorKind::OptimisticConcurrencyConflict,
        ))
    }
}

fn validate_task_creation(
    state: &LifecycleState,
    payload: &TaskCreatedPayload,
) -> Result<(), LifecycleError> {
    if state.run_state != RunState::Created
        || payload.initial_state != TaskState::Created
        || state.tasks.contains_key(&payload.task_id)
    {
        return Err(LifecycleError::new(LifecycleErrorKind::InvalidTransition));
    }
    if let Some(parent) = &payload.parent_task_id {
        let parent = state
            .tasks
            .get(parent)
            .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::ParentStateConflict))?;
        if is_task_terminal(parent.state) {
            return Err(LifecycleError::new(LifecycleErrorKind::ParentStateConflict));
        }
    }
    Ok(())
}

fn validate_run_transition(
    state: &LifecycleState,
    from: RunState,
    to: RunState,
) -> Result<(), LifecycleError> {
    if is_run_terminal(from) {
        return Err(LifecycleError::new(
            LifecycleErrorKind::TerminalStateConflict,
        ));
    }
    if !is_run_edge(from, to) {
        return Err(LifecycleError::new(LifecycleErrorKind::InvalidTransition));
    }
    let tasks: Vec<_> = state.tasks.values().map(|task| task.state).collect();
    let all_terminal = tasks.iter().copied().all(is_task_terminal);
    let any_failed = tasks.contains(&TaskState::Failed);
    let any_cancelled = tasks.contains(&TaskState::Cancelled);
    let guard = match (from, to) {
        (RunState::Created, RunState::Running) => {
            !tasks.is_empty() && tasks.iter().all(|state| *state == TaskState::Ready)
        }
        (RunState::Created | RunState::Running | RunState::Paused, RunState::Failed) => {
            all_terminal && any_failed
        }
        (RunState::Created, RunState::Cancelled) => {
            tasks.is_empty() || (all_terminal && !any_failed && any_cancelled)
        }
        (RunState::Running | RunState::Paused, RunState::Cancelled) => {
            all_terminal && !any_failed && any_cancelled
        }
        (RunState::Running, RunState::Paused) => tasks.iter().all(|state| {
            is_task_terminal(*state) || matches!(state, TaskState::Ready | TaskState::Paused)
        }),
        (RunState::Running, RunState::Succeeded) => {
            !tasks.is_empty() && tasks.iter().all(|state| *state == TaskState::Succeeded)
        }
        (RunState::Paused, RunState::Running) => {
            tasks.iter().any(|state| !is_task_terminal(*state))
                && tasks.iter().all(|state| {
                    is_task_terminal(*state)
                        || matches!(state, TaskState::Ready | TaskState::Paused)
                })
        }
        _ => unreachable!("run edge was checked"),
    };
    if guard {
        Ok(())
    } else {
        Err(LifecycleError::new(LifecycleErrorKind::ParentStateConflict))
    }
}

fn validate_task_transition(
    state: &LifecycleState,
    task_id: &TaskId,
    from: TaskState,
    to: TaskState,
) -> Result<(), LifecycleError> {
    if is_task_terminal(from) || is_run_terminal(state.run_state) {
        return Err(LifecycleError::new(
            LifecycleErrorKind::TerminalStateConflict,
        ));
    }
    if !is_task_edge(from, to) {
        return Err(LifecycleError::new(LifecycleErrorKind::InvalidTransition));
    }
    let task = state
        .tasks
        .get(task_id)
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::InvalidTransition))?;
    let parent_state = task
        .parent_task_id
        .as_ref()
        .and_then(|parent| state.tasks.get(parent))
        .map(|parent| parent.state);
    let children: Vec<_> = state
        .tasks
        .values()
        .filter(|child| child.parent_task_id.as_ref() == Some(task_id))
        .map(|child| child.state)
        .collect();
    let all_children_terminal = children.iter().copied().all(is_task_terminal);
    let guard = match (from, to) {
        (TaskState::Created, TaskState::Ready) => {
            state.run_state == RunState::Created
                && parent_state.is_none_or(|parent| !is_task_terminal(parent))
        }
        (TaskState::Created | TaskState::Ready, TaskState::Failed | TaskState::Cancelled) => {
            all_children_terminal
        }
        (TaskState::Ready | TaskState::Paused, TaskState::Running) => {
            state.run_state == RunState::Running
                && parent_state.is_none_or(|parent| parent == TaskState::Running)
        }
        (TaskState::Running, TaskState::Paused) => {
            state.run_state == RunState::Running
                && children.iter().all(|child| *child != TaskState::Running)
        }
        (TaskState::Running, TaskState::Succeeded) => {
            children.iter().all(|child| *child == TaskState::Succeeded)
        }
        (TaskState::Running | TaskState::Paused, TaskState::Failed | TaskState::Cancelled) => {
            all_children_terminal
        }
        _ => unreachable!("task edge was checked"),
    };
    if guard {
        Ok(())
    } else {
        Err(LifecycleError::new(LifecycleErrorKind::ParentStateConflict))
    }
}

fn is_run_terminal(state: RunState) -> bool {
    matches!(
        state,
        RunState::Succeeded | RunState::Failed | RunState::Cancelled
    )
}

fn is_run_edge(from: RunState, to: RunState) -> bool {
    matches!(
        (from, to),
        (
            RunState::Created,
            RunState::Running | RunState::Failed | RunState::Cancelled
        ) | (
            RunState::Running,
            RunState::Paused | RunState::Succeeded | RunState::Failed | RunState::Cancelled
        ) | (
            RunState::Paused,
            RunState::Running | RunState::Failed | RunState::Cancelled
        )
    )
}

fn is_task_edge(from: TaskState, to: TaskState) -> bool {
    matches!(
        (from, to),
        (
            TaskState::Created,
            TaskState::Ready | TaskState::Failed | TaskState::Cancelled
        ) | (
            TaskState::Ready,
            TaskState::Running | TaskState::Failed | TaskState::Cancelled
        ) | (
            TaskState::Running,
            TaskState::Paused | TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
        ) | (
            TaskState::Paused,
            TaskState::Running | TaskState::Failed | TaskState::Cancelled
        )
    )
}

fn is_task_terminal(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
    )
}

#[cfg(test)]
include!("lifecycle/tests.rs");
