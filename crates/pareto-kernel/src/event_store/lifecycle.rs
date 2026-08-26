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
pub(super) enum LifecycleErrorKind {
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
pub(super) struct LifecycleError {
    pub(super) kind: LifecycleErrorKind,
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
            | ErrorKind::WriterEpochConflict
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
pub(super) struct TrustedRunInputs {
    pub(super) scope: IsolationScope,
    pub(super) actor: AgentId,
    pub(super) schema_set: Arc<SchemaSet>,
    pub(super) protocol_limits_ref: ProtocolLimitsRef,
    pub(super) revisions: BTreeMap<String, RevisionId>,
    pub(super) plan_revision: Option<RevisionId>,
    pub(super) budget_revision: RevisionId,
    pub(super) boundary_recording_policy_ref: BoundaryRecordingPolicyRef,
    pub(super) execution_mode: ExecutionMode,
}

#[derive(Clone)]
pub(super) struct LifecycleTarget {
    pub(super) scope: IsolationScope,
    pub(super) actor: AgentId,
}

#[derive(Clone)]
pub(super) struct CreateRunCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) manifest: RunManifest,
}

#[derive(Clone)]
pub(super) struct CreateTaskCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) expected_sequence: i64,
    pub(super) task_id: TaskId,
    pub(super) parent_task_id: Option<TaskId>,
}

#[derive(Clone)]
pub(super) struct TransitionRunCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) expected_sequence: i64,
    pub(super) expected_state: RunState,
    pub(super) target_state: RunState,
    pub(super) reason_code: String,
}

#[derive(Clone)]
pub(super) struct TransitionTaskCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) expected_sequence: i64,
    pub(super) task_id: TaskId,
    pub(super) expected_state: TaskState,
    pub(super) target_state: TaskState,
    pub(super) reason_code: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AppliedState {
    Run(RunState),
    Task(TaskState),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum LifecycleResult {
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct TaskRecord {
    pub(super) parent_task_id: Option<TaskId>,
    pub(super) state: TaskState,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct LifecycleState {
    pub(super) manifest: RunManifest,
    pub(super) run_state: RunState,
    pub(super) tasks: BTreeMap<TaskId, TaskRecord>,
    pub(super) sequence: i64,
}

#[derive(Debug)]
pub(super) struct EstablishedAggregate {
    pub(super) state: LifecycleState,
    pub(super) schema_set: Arc<SchemaSet>,
    pub(super) limits: ProtocolLimitsRef,
    pub(super) stream_id: StreamId,
}

impl EventStore {
    pub(super) async fn create_run(
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

    pub(super) async fn create_task(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &CreateTaskCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let sequence = command_sequence(
            &mut transaction,
            &command.event_id,
            command.expected_sequence,
        )
        .await?;
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
            sequence,
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

    pub(super) async fn transition_run(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &TransitionRunCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let sequence = command_sequence(
            &mut transaction,
            &command.event_id,
            command.expected_sequence,
        )
        .await?;
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
            sequence,
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
        if matches!(
            command.target_state,
            RunState::Paused | RunState::Succeeded | RunState::Failed | RunState::Cancelled
        ) {
            super::runtime_control::ensure_no_pending_for_run(
                &mut transaction,
                registry,
                &target.scope,
            )
            .await
            .map_err(|_| LifecycleError::new(LifecycleErrorKind::ParentStateConflict))?;
        }
        validate_run_transition(
            &aggregate.state,
            command.expected_state,
            command.target_state,
        )?;
        let result = insert_prepared(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        Ok(result_for(result, AppliedState::Run(command.target_state)))
    }

    pub(super) async fn transition_task(
        &self,
        registry: &SchemaRegistry,
        target: &LifecycleTarget,
        command: &TransitionTaskCommand,
    ) -> Result<LifecycleResult, LifecycleError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_established(&mut transaction, registry, target).await?;
        let sequence = command_sequence(
            &mut transaction,
            &command.event_id,
            command.expected_sequence,
        )
        .await?;
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
            sequence,
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
        if aggregate.state.sequence != command.expected_sequence {
            return Err(LifecycleError::new(
                LifecycleErrorKind::OptimisticConcurrencyConflict,
            ));
        }
        let current = aggregate
            .state
            .tasks
            .get(&command.task_id)
            .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::InvalidTransition))?;
        if current.state != command.expected_state {
            return Err(LifecycleError::new(
                LifecycleErrorKind::OptimisticConcurrencyConflict,
            ));
        }
        if matches!(
            command.target_state,
            TaskState::Paused | TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
        ) {
            super::runtime_control::ensure_no_pending_for_task(
                &mut transaction,
                registry,
                &target.scope,
                &command.task_id,
            )
            .await
            .map_err(|_| LifecycleError::new(LifecycleErrorKind::ParentStateConflict))?;
        }
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

async fn command_sequence(
    connection: &mut SqliteConnection,
    event_id: &EventId,
    expected_sequence: i64,
) -> Result<i64, LifecycleError> {
    if let Some(sequence) = expected_sequence
        .checked_add(1)
        .filter(|sequence| *sequence >= 2)
    {
        return Ok(sequence);
    }
    let id_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE event_id=?")
        .bind(event_id.as_str())
        .fetch_one(&mut *connection)
        .await?;
    let kind = if id_exists == 0 {
        LifecycleErrorKind::OptimisticConcurrencyConflict
    } else {
        LifecycleErrorKind::IdempotencyConflict
    };
    Err(LifecycleError::new(kind))
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

pub(super) fn lifecycle_stream_id(scope: &IsolationScope) -> Result<StreamId, LifecycleError> {
    let suffix = scope
        .run_id
        .as_str()
        .strip_prefix("run_")
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))?;
    StreamId::parse(format!("stream_lifecycle-{suffix}"))
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::ManifestInvalid))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lifecycle_event<T: serde::Serialize>(
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

pub(super) async fn load_established(
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
    let state = fold_lifecycle(&schema_set, &events)?;
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

pub(super) fn fold_lifecycle(
    schema_set: &SchemaSet,
    events: &[ValidatedEvent],
) -> Result<LifecycleState, LifecycleError> {
    let first = events
        .first()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateNotFound))?;
    let created = first
        .downcast_payload::<RunCreatedPayload>()
        .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    let first_envelope = first.envelope();
    schema_set
        .validate_run_manifest(created.manifest.clone(), &first_envelope.scope)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    let expected_stream = lifecycle_stream_id(&created.manifest.scope)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    if first_envelope.event_type != "run-created"
        || first_envelope.event_major != 1
        || first_envelope.event_minor != 0
        || first_envelope.sequence != "1"
        || first.variant_id() != "run-created-v1"
        || first.schema_set_ref() != schema_set.reference()
        || created.manifest.scope != first_envelope.scope
        || created.manifest.schema_set_ref != *first.schema_set_ref()
        || created.manifest.protocol_limits_ref != *first.protocol_limits_ref()
        || first_envelope.run_id != created.manifest.scope.run_id
        || first_envelope.actor != created.manifest.scope.agent_id
        || first_envelope.stream_id != expected_stream
    {
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
        apply_lifecycle_event(schema_set, &mut state, event, expected_sequence)?;
    }
    Ok(state)
}

pub(super) fn apply_lifecycle_event(
    schema_set: &SchemaSet,
    state: &mut LifecycleState,
    event: &ValidatedEvent,
    expected_sequence: i64,
) -> Result<(), LifecycleError> {
    let envelope = event.envelope();
    let expected_stream = lifecycle_stream_id(&state.manifest.scope)
        .map_err(|_| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
    if envelope.sequence.parse::<i64>().ok() != Some(expected_sequence)
        || expected_sequence != state.sequence + 1
        || envelope.event_major != 1
        || envelope.event_minor != 0
        || envelope.scope != state.manifest.scope
        || envelope.run_id != state.manifest.scope.run_id
        || envelope.actor != state.manifest.scope.agent_id
        || envelope.stream_id != expected_stream
        || event.schema_set_ref() != schema_set.reference()
        || event.schema_set_ref() != &state.manifest.schema_set_ref
        || event.protocol_limits_ref() != &state.manifest.protocol_limits_ref
    {
        return Err(LifecycleError::new(LifecycleErrorKind::AggregateCorrupt));
    }
    match envelope.event_type.as_str() {
        "task-created" => {
            let payload = event
                .downcast_payload::<TaskCreatedPayload>()
                .ok_or_else(|| LifecycleError::new(LifecycleErrorKind::AggregateCorrupt))?;
            validate_task_creation(state, payload)
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
            validate_run_transition(state, payload.from, payload.to)
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
            validate_task_transition(state, &payload.task_id, payload.from, payload.to)
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
    Ok(())
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
