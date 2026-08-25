use std::{collections::BTreeMap, sync::Arc};

use pareto_protocol::{
    AgentId, Digest, EventCursor, IsolationScope, ProjectionHistorySeedV1, ProjectionHistoryStepV1,
    ProjectionReducerDescriptorV1, ProjectionReducerRef, ProtocolLimitsRef, RevisionId,
    RunCreatedPayload, RunTaskProjection, RunTaskProjectionHashViewV1, RunTaskProjectionSnapshot,
    RunTaskProjectionSnapshotHashViewV1, RunTaskProjectionTask, SchemaRef, SchemaSet, SchemaSetRef,
    SourceReducerKeyV1, StreamId, ValidatedEvent, digest_json,
};
use sqlx::{Row, SqliteConnection};

use super::lifecycle::{
    LifecycleError, LifecycleErrorKind, LifecycleState, TaskRecord, apply_lifecycle_event,
    fold_lifecycle, lifecycle_stream_id,
};
use super::{
    AdmittedRead, ErrorKind, EventStore, EventStoreError, SchemaRegistry, canonical, fingerprint,
    user_key, validate_row,
};

const ROW_COLUMNS: &str = "envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id";
const HISTORY_ALGORITHM: &str = "run-task-history-chain-v1";
const LIFECYCLE_EVENTS: [&str; 4] = [
    "run-created",
    "run-state-transitioned",
    "task-created",
    "task-state-transitioned",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionErrorKind {
    Unauthorized,
    AggregateNotFound,
    AggregateCorrupt,
    SchemaUnavailable,
    ReducerUnavailable,
    UnsupportedEvent,
    InvalidSequence,
    HistoryMismatch,
    SnapshotIntegrity,
    SnapshotIncompatible,
    WriterEpochConflict,
    SimulationUnavailable,
    NotComparable,
    Busy,
    Io,
}

#[derive(Debug)]
struct ProjectionError {
    kind: ProjectionErrorKind,
}

impl ProjectionError {
    fn new(kind: ProjectionErrorKind) -> Self {
        Self { kind }
    }
}

impl From<EventStoreError> for ProjectionError {
    fn from(error: EventStoreError) -> Self {
        let kind = match error.kind {
            ErrorKind::WriterEpochConflict => ProjectionErrorKind::WriterEpochConflict,
            ErrorKind::Busy => ProjectionErrorKind::Busy,
            ErrorKind::Io => ProjectionErrorKind::Io,
            ErrorKind::ProtocolInvalid => ProjectionErrorKind::SchemaUnavailable,
            ErrorKind::IsolationConflict => ProjectionErrorKind::Unauthorized,
            ErrorKind::Migration
            | ErrorKind::DatabaseCorrupt
            | ErrorKind::IdempotencyConflict
            | ErrorKind::SequenceConflict
            | ErrorKind::CausationConflict => ProjectionErrorKind::AggregateCorrupt,
        };
        Self::new(kind)
    }
}

impl From<LifecycleError> for ProjectionError {
    fn from(error: LifecycleError) -> Self {
        let kind = match error.kind {
            LifecycleErrorKind::Unauthorized => ProjectionErrorKind::Unauthorized,
            LifecycleErrorKind::AggregateNotFound => ProjectionErrorKind::AggregateNotFound,
            LifecycleErrorKind::SchemaUnavailable => ProjectionErrorKind::SchemaUnavailable,
            LifecycleErrorKind::Busy => ProjectionErrorKind::Busy,
            LifecycleErrorKind::Io => ProjectionErrorKind::Io,
            LifecycleErrorKind::ManifestInvalid
            | LifecycleErrorKind::AggregateCorrupt
            | LifecycleErrorKind::InvalidTransition
            | LifecycleErrorKind::ParentStateConflict
            | LifecycleErrorKind::TerminalStateConflict
            | LifecycleErrorKind::OptimisticConcurrencyConflict
            | LifecycleErrorKind::IdempotencyConflict => ProjectionErrorKind::AggregateCorrupt,
        };
        Self::new(kind)
    }
}

impl From<sqlx::Error> for ProjectionError {
    fn from(error: sqlx::Error) -> Self {
        EventStoreError::from(error).into()
    }
}

#[derive(Clone, Debug)]
struct ProjectionTarget {
    scope: IsolationScope,
    actor: AgentId,
}

#[derive(Clone)]
struct ReducerRegistration {
    source_key: SourceReducerKeyV1,
    descriptor: ProjectionReducerDescriptorV1,
    reducer_ref: ProjectionReducerRef,
}

#[derive(Clone)]
struct ProjectionRegistry {
    sources: SchemaRegistry,
    outputs: SchemaRegistry,
    output_limits: ProtocolLimitsRef,
    reducers: Vec<ReducerRegistration>,
}

impl ProjectionRegistry {
    fn retained(
        sources: SchemaRegistry,
        outputs: SchemaRegistry,
        output_limits: ProtocolLimitsRef,
    ) -> Result<Self, ProjectionError> {
        let output = outputs
            .0
            .iter()
            .find(|set| set.schema_ref("run-task-projection").is_some())
            .cloned()
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::SchemaUnavailable))?;
        let mut reducers = Vec::new();
        for source in &sources.0 {
            let Ok(source_key) = source_reducer_key(source) else {
                continue;
            };
            if reducers
                .iter()
                .any(|item: &ReducerRegistration| item.source_key == source_key)
            {
                continue;
            }
            let descriptor = reducer_descriptor(&source_key, &output, &output_limits)?;
            let descriptor_bytes = canonical(&descriptor)?.into_bytes();
            output
                .parse_record::<ProjectionReducerDescriptorV1>(&descriptor_bytes)
                .map_err(|_| ProjectionError::new(ProjectionErrorKind::ReducerUnavailable))?;
            let contract_digest = digest_json(
                "projection-reducer-contract",
                &descriptor.schema_ref,
                &serde_json::to_value(&descriptor)
                    .map_err(|_| ProjectionError::new(ProjectionErrorKind::ReducerUnavailable))?,
            )
            .map_err(|_| ProjectionError::new(ProjectionErrorKind::ReducerUnavailable))?;
            let reducer_ref = ProjectionReducerRef {
                descriptor_schema_ref: descriptor.schema_ref.clone(),
                contract_digest,
            };
            reducers.push(ReducerRegistration {
                source_key,
                descriptor,
                reducer_ref,
            });
        }
        if reducers.is_empty() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::ReducerUnavailable,
            ));
        }
        Ok(Self {
            sources,
            outputs,
            output_limits,
            reducers,
        })
    }

    fn resolve_reducer(&self, source: &SchemaSet) -> Result<&ReducerRegistration, ProjectionError> {
        let key = source_reducer_key(source)?;
        self.reducers
            .iter()
            .find(|registration| registration.source_key == key)
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::ReducerUnavailable))
    }

    fn resolve_output(
        &self,
        reference: &SchemaSetRef,
        limits: &ProtocolLimitsRef,
    ) -> Result<Arc<SchemaSet>, ProjectionError> {
        if limits != &self.output_limits {
            return Err(ProjectionError::new(
                ProjectionErrorKind::SnapshotIncompatible,
            ));
        }
        self.outputs
            .0
            .iter()
            .find(|set| set.reference() == reference)
            .cloned()
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::SchemaUnavailable))
    }
}

fn exact_schema(set: &SchemaSet, name: &str) -> Result<SchemaRef, ProjectionError> {
    set.schema_ref(name)
        .cloned()
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::SchemaUnavailable))
}

fn source_reducer_key(source: &SchemaSet) -> Result<SourceReducerKeyV1, ProjectionError> {
    let mut event_bindings = Vec::with_capacity(LIFECYCLE_EVENTS.len());
    for event_type in LIFECYCLE_EVENTS {
        event_bindings.push(
            source
                .event_type_binding(event_type, 1, 0)
                .cloned()
                .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::ReducerUnavailable))?,
        );
    }
    event_bindings.sort();
    Ok(SourceReducerKeyV1 {
        run_manifest_schema_ref: exact_schema(source, "run-manifest")?,
        event_bindings,
    })
}

fn reducer_descriptor(
    source_key: &SourceReducerKeyV1,
    output: &SchemaSet,
    output_limits: &ProtocolLimitsRef,
) -> Result<ProjectionReducerDescriptorV1, ProjectionError> {
    Ok(ProjectionReducerDescriptorV1 {
        schema_ref: exact_schema(output, "projection-reducer-descriptor")?,
        reducer_kind: "run-task-lifecycle".to_owned(),
        major: 1,
        minor: 0,
        accepted_event_bindings: source_key.event_bindings.clone(),
        run_manifest_schema_ref: source_key.run_manifest_schema_ref.clone(),
        manifest_admission_contract: "validate-run-manifest-v1-before-state".to_owned(),
        run_transition_contract: vec![
            "created->running:all-tasks-ready".to_owned(),
            "created|running|paused->failed:all-terminal-and-any-failed".to_owned(),
            "created|running|paused->cancelled:all-terminal-no-failure-and-any-cancelled-or-empty-created".to_owned(),
            "running->paused:no-running-task".to_owned(),
            "running->succeeded:all-tasks-succeeded-nonempty".to_owned(),
            "paused->running:remaining-task-and-no-running-task".to_owned(),
        ],
        task_transition_contract: vec![
            "created->ready".to_owned(),
            "created|ready|running|paused->failed|cancelled:children-terminal".to_owned(),
            "ready|paused->running:run-running-and-parent-running".to_owned(),
            "running->paused:no-running-child".to_owned(),
            "running->succeeded:all-children-succeeded".to_owned(),
        ],
        parent_guard_contract: vec![
            "parent-created-earlier-same-stream".to_owned(),
            "terminal-states-have-no-outgoing-edge".to_owned(),
            "no-implicit-child-cascade".to_owned(),
        ],
        task_ordering: "task-id-ascending-v1".to_owned(),
        history_algorithm: HISTORY_ALGORITHM.to_owned(),
        history_seed_schema_ref: exact_schema(output, "projection-history-seed")?,
        history_step_schema_ref: exact_schema(output, "projection-history-step")?,
        projection_hash_schema_ref: exact_schema(output, "run-task-projection-hash-view")?,
        snapshot_hash_schema_ref: exact_schema(
            output,
            "run-task-projection-snapshot-hash-view",
        )?,
        output_schema_set_ref: output.reference().clone(),
        output_protocol_limits_ref: output_limits.clone(),
        projection_schema_ref: exact_schema(output, "run-task-projection")?,
        snapshot_schema_ref: exact_schema(output, "run-task-projection-snapshot")?,
    })
}

struct ProjectionSource {
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
    stream_id: StreamId,
    events: Vec<ValidatedEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotDisposition {
    Missing,
    Used,
    RejectedIntegrity,
    RejectedCursor,
    RejectedIncompatible,
}

#[derive(Debug)]
struct ProjectionLoad {
    projection: RunTaskProjection,
    snapshot_disposition: SnapshotDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionComparison {
    Equal,
    Divergent,
    NotComparable,
}

#[derive(Clone, Debug)]
struct SimulationRequest {
    source: ProjectionTarget,
    fixture_revisions: Vec<RevisionId>,
}

impl EventStore {
    async fn project_full(
        &self,
        registry: &ProjectionRegistry,
        target: &ProjectionTarget,
    ) -> Result<ProjectionLoad, ProjectionError> {
        let mut transaction = self.pool.begin().await?;
        let source = load_source(&mut transaction, &registry.sources, target).await?;
        let projection = full_projection(&self.store_id, registry, &source)?;
        transaction.rollback().await?;
        Ok(ProjectionLoad {
            projection,
            snapshot_disposition: SnapshotDisposition::Missing,
        })
    }

    async fn create_projection_snapshot(
        &self,
        registry: &ProjectionRegistry,
        target: &ProjectionTarget,
    ) -> Result<RunTaskProjectionSnapshot, ProjectionError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let source = load_source(&mut transaction, &registry.sources, target).await?;
        let projection = full_projection(&self.store_id, registry, &source)?;
        let reducer = registry.resolve_reducer(&source.schema_set)?;
        let output = registry.resolve_output(
            &reducer.descriptor.output_schema_set_ref,
            &reducer.descriptor.output_protocol_limits_ref,
        )?;
        let snapshot = build_snapshot(&projection, reducer, &output)?;
        insert_snapshot(&mut transaction, &snapshot).await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    async fn project_snapshot_assisted(
        &self,
        registry: &ProjectionRegistry,
        target: &ProjectionTarget,
    ) -> Result<ProjectionLoad, ProjectionError> {
        let mut transaction = self.pool.begin().await?;
        let source = load_source(&mut transaction, &registry.sources, target).await?;
        let reducer = registry.resolve_reducer(&source.schema_set)?;
        let candidate = select_snapshot(&mut transaction, &self.store_id, target, &source).await?;
        let result = match candidate {
            None => ProjectionLoad {
                projection: full_projection(&self.store_id, registry, &source)?,
                snapshot_disposition: SnapshotDisposition::Missing,
            },
            Some(row) => match validate_snapshot_candidate(
                &row,
                &self.store_id,
                registry,
                target,
                &source,
                reducer,
            ) {
                Ok(snapshot) => ProjectionLoad {
                    projection: apply_snapshot_suffix(
                        &self.store_id,
                        registry,
                        &source,
                        reducer,
                        &snapshot,
                    )?,
                    snapshot_disposition: SnapshotDisposition::Used,
                },
                Err(CandidateFailure::Integrity) => ProjectionLoad {
                    projection: full_projection(&self.store_id, registry, &source)?,
                    snapshot_disposition: SnapshotDisposition::RejectedIntegrity,
                },
                Err(CandidateFailure::Cursor) => ProjectionLoad {
                    projection: full_projection(&self.store_id, registry, &source)?,
                    snapshot_disposition: SnapshotDisposition::RejectedCursor,
                },
                Err(CandidateFailure::Incompatible) => ProjectionLoad {
                    projection: full_projection(&self.store_id, registry, &source)?,
                    snapshot_disposition: SnapshotDisposition::RejectedIncompatible,
                },
                Err(CandidateFailure::HistoryMismatch) => {
                    return Err(ProjectionError::new(ProjectionErrorKind::HistoryMismatch));
                }
            },
        };
        transaction.rollback().await?;
        Ok(result)
    }

    async fn recorded_replay(
        &self,
        registry: &ProjectionRegistry,
        target: &ProjectionTarget,
    ) -> Result<RunTaskProjection, ProjectionError> {
        Ok(self.project_full(registry, target).await?.projection)
    }

    fn simulated_replay(
        &self,
        request: &SimulationRequest,
    ) -> Result<RunTaskProjection, ProjectionError> {
        let _fixed_lineage = (
            &request.source.scope,
            &request.source.actor,
            &request.fixture_revisions,
        );
        Err(ProjectionError::new(
            ProjectionErrorKind::SimulationUnavailable,
        ))
    }
}

fn compare_projections(
    target: &ProjectionTarget,
    left: &RunTaskProjection,
    right: &RunTaskProjection,
) -> Result<ProjectionComparison, ProjectionError> {
    if target.actor != target.scope.agent_id
        || left.scope != target.scope
        || right.scope != target.scope
        || left.owner_actor != target.actor
        || right.owner_actor != target.actor
    {
        return Err(ProjectionError::new(ProjectionErrorKind::Unauthorized));
    }
    let comparable = left.source_store_id == right.source_store_id
        && left.scope == right.scope
        && left.owner_actor == right.owner_actor
        && left.stream_id == right.stream_id
        && left.cursor == right.cursor
        && left.source_schema_set_ref == right.source_schema_set_ref
        && left.source_protocol_limits_ref == right.source_protocol_limits_ref
        && left.history_chain_state == right.history_chain_state
        && left.reducer_ref == right.reducer_ref
        && left.output_schema_set_ref == right.output_schema_set_ref
        && left.output_protocol_limits_ref == right.output_protocol_limits_ref;
    if !comparable {
        return Ok(ProjectionComparison::NotComparable);
    }
    if left.projection_digest == right.projection_digest {
        Ok(ProjectionComparison::Equal)
    } else {
        Ok(ProjectionComparison::Divergent)
    }
}

async fn load_source(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    target: &ProjectionTarget,
) -> Result<ProjectionSource, ProjectionError> {
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
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::Unauthorized))?;
    let schema_ref: SchemaSetRef = serde_json::from_str(&first.get::<String, _>(2))
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?;
    let limits: ProtocolLimitsRef = serde_json::from_str(&first.get::<String, _>(4))
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?;
    let schema_set = registry
        .0
        .iter()
        .find(|set| set.reference() == &schema_ref)
        .cloned()
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::SchemaUnavailable))?;
    let read = AdmittedRead {
        scope: target.scope.clone(),
        stream_id: Some(stream_id.clone()),
        schema_set: schema_set.clone(),
        limits: limits.clone(),
    };
    let first_event = validate_row(&first, &read)
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?;
    let created = first_event
        .downcast_payload::<RunCreatedPayload>()
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?;
    if first_event.envelope().event_type != "run-created"
        || first_event.envelope().sequence != "1"
        || created.manifest.scope != target.scope
        || created.manifest.schema_set_ref != *schema_set.reference()
        || created.manifest.protocol_limits_ref != limits
    {
        return Err(ProjectionError::new(ProjectionErrorKind::AggregateCorrupt));
    }
    schema_set
        .validate_run_manifest(created.manifest.clone(), &target.scope)
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?;
    if target.actor != created.manifest.scope.agent_id {
        return Err(ProjectionError::new(ProjectionErrorKind::Unauthorized));
    }
    let all_sql = format!(
        "SELECT {ROW_COLUMNS} FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? ORDER BY sequence_i64,event_id"
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
                .map_err(|_| ProjectionError::new(ProjectionErrorKind::AggregateCorrupt))?,
        );
    }
    if events.is_empty() {
        return Err(ProjectionError::new(ProjectionErrorKind::AggregateNotFound));
    }
    Ok(ProjectionSource {
        schema_set,
        limits,
        stream_id,
        events,
    })
}

fn full_projection(
    store_id: &str,
    registry: &ProjectionRegistry,
    source: &ProjectionSource,
) -> Result<RunTaskProjection, ProjectionError> {
    let reducer = registry.resolve_reducer(&source.schema_set)?;
    let state = fold_lifecycle(&source.schema_set, &source.events)?;
    let history = history_chain(reducer, &source.events)?;
    build_projection(store_id, registry, source, reducer, state, history)
}

fn history_seed(reducer: &ReducerRegistration) -> Result<Digest, ProjectionError> {
    let value = serde_json::to_value(ProjectionHistorySeedV1 {
        algorithm: HISTORY_ALGORITHM.to_owned(),
    })
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::HistoryMismatch))?;
    digest_json(
        "projection-history-chain-seed",
        &reducer.descriptor.history_seed_schema_ref,
        &value,
    )
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::HistoryMismatch))
}

fn history_step(
    reducer: &ReducerRegistration,
    previous: Digest,
    event: &ValidatedEvent,
) -> Result<Digest, ProjectionError> {
    if event.envelope().sequence.parse::<i64>().ok().is_none() {
        return Err(ProjectionError::new(ProjectionErrorKind::InvalidSequence));
    }
    let value = serde_json::to_value(ProjectionHistoryStepV1 {
        algorithm: HISTORY_ALGORITHM.to_owned(),
        previous_digest: previous,
        sequence: event.envelope().sequence.clone(),
        envelope: event.envelope().clone(),
        source_schema_set_ref: event.schema_set_ref().clone(),
        source_protocol_limits_ref: event.protocol_limits_ref().clone(),
    })
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::HistoryMismatch))?;
    digest_json(
        "projection-history-chain-step",
        &reducer.descriptor.history_step_schema_ref,
        &value,
    )
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::HistoryMismatch))
}

fn history_chain(
    reducer: &ReducerRegistration,
    events: &[ValidatedEvent],
) -> Result<Digest, ProjectionError> {
    let mut current = history_seed(reducer)?;
    for event in events {
        current = history_step(reducer, current, event)?;
    }
    Ok(current)
}

fn build_projection(
    store_id: &str,
    registry: &ProjectionRegistry,
    source: &ProjectionSource,
    reducer: &ReducerRegistration,
    state: LifecycleState,
    history_chain_state: Digest,
) -> Result<RunTaskProjection, ProjectionError> {
    let output = registry.resolve_output(
        &reducer.descriptor.output_schema_set_ref,
        &reducer.descriptor.output_protocol_limits_ref,
    )?;
    let last = source
        .events
        .last()
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::AggregateNotFound))?;
    if state.sequence.to_string() != last.envelope().sequence {
        return Err(ProjectionError::new(ProjectionErrorKind::InvalidSequence));
    }
    let tasks = state
        .tasks
        .into_iter()
        .map(|(task_id, task)| RunTaskProjectionTask {
            task_id,
            parent_task_id: task.parent_task_id,
            state: task.state,
        })
        .collect();
    let mut projection = RunTaskProjection {
        schema_ref: reducer.descriptor.projection_schema_ref.clone(),
        source_store_id: store_id.to_owned(),
        scope: state.manifest.scope.clone(),
        owner_actor: state.manifest.scope.agent_id.clone(),
        stream_id: source.stream_id.clone(),
        cursor: EventCursor {
            sequence: last.envelope().sequence.clone(),
            event_id: last.envelope().event_id.clone(),
        },
        source_schema_set_ref: source.schema_set.reference().clone(),
        source_protocol_limits_ref: source.limits.clone(),
        reducer_ref: reducer.reducer_ref.clone(),
        output_schema_set_ref: output.reference().clone(),
        output_protocol_limits_ref: registry.output_limits.clone(),
        history_chain_state,
        manifest: state.manifest,
        run_state: state.run_state,
        tasks,
        projection_digest: history_seed(reducer)?,
    };
    projection.projection_digest = compute_projection_digest(&projection, reducer)?;
    validate_projection_record(&projection, reducer, &output)?;
    Ok(projection)
}

fn projection_hash_view(projection: &RunTaskProjection) -> RunTaskProjectionHashViewV1 {
    RunTaskProjectionHashViewV1 {
        projection_schema_ref: projection.schema_ref.clone(),
        source_store_id: projection.source_store_id.clone(),
        scope: projection.scope.clone(),
        owner_actor: projection.owner_actor.clone(),
        stream_id: projection.stream_id.clone(),
        cursor: projection.cursor.clone(),
        source_schema_set_ref: projection.source_schema_set_ref.clone(),
        source_protocol_limits_ref: projection.source_protocol_limits_ref.clone(),
        reducer_ref: projection.reducer_ref.clone(),
        output_schema_set_ref: projection.output_schema_set_ref.clone(),
        output_protocol_limits_ref: projection.output_protocol_limits_ref.clone(),
        history_chain_state: projection.history_chain_state.clone(),
        manifest: projection.manifest.clone(),
        run_state: projection.run_state,
        tasks: projection.tasks.clone(),
    }
}

fn compute_projection_digest(
    projection: &RunTaskProjection,
    reducer: &ReducerRegistration,
) -> Result<Digest, ProjectionError> {
    let value = serde_json::to_value(projection_hash_view(projection))
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))?;
    digest_json(
        "run-task-projection",
        &reducer.descriptor.projection_hash_schema_ref,
        &value,
    )
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))
}

fn validate_projection_record(
    projection: &RunTaskProjection,
    reducer: &ReducerRegistration,
    output: &SchemaSet,
) -> Result<(), ProjectionError> {
    let expected_stream = lifecycle_stream_id(&projection.scope)
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))?;
    if projection.schema_ref != reducer.descriptor.projection_schema_ref
        || projection.reducer_ref != reducer.reducer_ref
        || projection.output_schema_set_ref != *output.reference()
        || projection.output_protocol_limits_ref != reducer.descriptor.output_protocol_limits_ref
        || projection.manifest.scope != projection.scope
        || projection.manifest.schema_set_ref != projection.source_schema_set_ref
        || projection.manifest.protocol_limits_ref != projection.source_protocol_limits_ref
        || projection.owner_actor != projection.scope.agent_id
        || projection.stream_id != expected_stream
        || projection
            .cursor
            .sequence
            .parse::<i64>()
            .ok()
            .is_none_or(|value| value <= 0)
        || projection.projection_digest != compute_projection_digest(projection, reducer)?
        || projection
            .tasks
            .windows(2)
            .any(|pair| pair[0].task_id >= pair[1].task_id)
    {
        return Err(ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity));
    }
    let bytes = canonical(projection)?.into_bytes();
    output
        .parse_record::<RunTaskProjection>(&bytes)
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))?;
    Ok(())
}

fn build_snapshot(
    projection: &RunTaskProjection,
    reducer: &ReducerRegistration,
    output: &SchemaSet,
) -> Result<RunTaskProjectionSnapshot, ProjectionError> {
    let mut snapshot = RunTaskProjectionSnapshot {
        schema_ref: reducer.descriptor.snapshot_schema_ref.clone(),
        projection_schema_ref: reducer.descriptor.projection_schema_ref.clone(),
        output_schema_set_ref: output.reference().clone(),
        output_protocol_limits_ref: reducer.descriptor.output_protocol_limits_ref.clone(),
        projection: projection.clone(),
        projection_digest: projection.projection_digest.clone(),
        snapshot_digest: history_seed(reducer)?,
    };
    snapshot.snapshot_digest = compute_snapshot_digest(&snapshot, reducer)?;
    validate_snapshot_record(&snapshot, reducer, output)?;
    Ok(snapshot)
}

fn snapshot_hash_view(snapshot: &RunTaskProjectionSnapshot) -> RunTaskProjectionSnapshotHashViewV1 {
    RunTaskProjectionSnapshotHashViewV1 {
        snapshot_schema_ref: snapshot.schema_ref.clone(),
        projection_schema_ref: snapshot.projection_schema_ref.clone(),
        output_schema_set_ref: snapshot.output_schema_set_ref.clone(),
        output_protocol_limits_ref: snapshot.output_protocol_limits_ref.clone(),
        projection: snapshot.projection.clone(),
        projection_digest: snapshot.projection_digest.clone(),
    }
}

fn compute_snapshot_digest(
    snapshot: &RunTaskProjectionSnapshot,
    reducer: &ReducerRegistration,
) -> Result<Digest, ProjectionError> {
    let value = serde_json::to_value(snapshot_hash_view(snapshot))
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))?;
    digest_json(
        "run-task-projection-snapshot",
        &reducer.descriptor.snapshot_hash_schema_ref,
        &value,
    )
    .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))
}

fn validate_snapshot_record(
    snapshot: &RunTaskProjectionSnapshot,
    reducer: &ReducerRegistration,
    output: &SchemaSet,
) -> Result<(), ProjectionError> {
    if snapshot.schema_ref != reducer.descriptor.snapshot_schema_ref
        || snapshot.projection_schema_ref != reducer.descriptor.projection_schema_ref
        || snapshot.output_schema_set_ref != *output.reference()
        || snapshot.output_protocol_limits_ref != reducer.descriptor.output_protocol_limits_ref
        || snapshot.projection.schema_ref != snapshot.projection_schema_ref
        || snapshot.projection_digest != snapshot.projection.projection_digest
        || snapshot.snapshot_digest != compute_snapshot_digest(snapshot, reducer)?
    {
        return Err(ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity));
    }
    validate_projection_record(&snapshot.projection, reducer, output)?;
    let bytes = canonical(snapshot)?.into_bytes();
    output
        .parse_record::<RunTaskProjectionSnapshot>(&bytes)
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))?;
    Ok(())
}

async fn insert_snapshot(
    connection: &mut SqliteConnection,
    snapshot: &RunTaskProjectionSnapshot,
) -> Result<(), ProjectionError> {
    let projection = &snapshot.projection;
    let snapshot_json = canonical(snapshot)?;
    let snapshot_fingerprint = fingerprint(snapshot_json.as_bytes());
    let output_schema_set_json = canonical(&snapshot.output_schema_set_ref)?;
    let output_limits_json = canonical(&snapshot.output_protocol_limits_ref)?;
    let source_schema_set_json = canonical(&projection.source_schema_set_ref)?;
    let source_limits_json = canonical(&projection.source_protocol_limits_ref)?;
    let reducer_ref_json = canonical(&projection.reducer_ref)?;
    let reducer_ref_fingerprint = fingerprint(reducer_ref_json.as_bytes());
    let (present, user) = user_key(&projection.scope);
    let cursor_sequence = projection
        .cursor
        .sequence
        .parse::<i64>()
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::InvalidSequence))?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT snapshot_json FROM projection_snapshots WHERE source_store_id=? AND tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND owner_actor=? AND stream_id=? AND cursor_sequence=? AND reducer_ref_fingerprint=?",
    )
    .bind(&projection.source_store_id)
    .bind(projection.scope.tenant_id.as_str())
    .bind(present)
    .bind(user)
    .bind(projection.scope.workspace_id.as_str())
    .bind(projection.scope.run_id.as_str())
    .bind(projection.scope.agent_id.as_str())
    .bind(projection.owner_actor.as_str())
    .bind(projection.stream_id.as_str())
    .bind(cursor_sequence)
    .bind(&reducer_ref_fingerprint)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(existing) = existing {
        return if existing == snapshot_json {
            Ok(())
        } else {
            Err(ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity))
        };
    }
    sqlx::query(
        "INSERT INTO projection_snapshots(snapshot_json,snapshot_fingerprint,output_schema_set_json,output_schema_set_fingerprint,output_limits_json,output_limits_fingerprint,source_schema_set_json,source_schema_set_fingerprint,source_limits_json,source_limits_fingerprint,reducer_ref_json,reducer_ref_fingerprint,source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,cursor_sequence,cursor_event_id,projection_digest,snapshot_digest) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&snapshot_json)
    .bind(&snapshot_fingerprint)
    .bind(&output_schema_set_json)
    .bind(fingerprint(output_schema_set_json.as_bytes()))
    .bind(&output_limits_json)
    .bind(fingerprint(output_limits_json.as_bytes()))
    .bind(&source_schema_set_json)
    .bind(fingerprint(source_schema_set_json.as_bytes()))
    .bind(&source_limits_json)
    .bind(fingerprint(source_limits_json.as_bytes()))
    .bind(&reducer_ref_json)
    .bind(&reducer_ref_fingerprint)
    .bind(&projection.source_store_id)
    .bind(projection.scope.tenant_id.as_str())
    .bind(present)
    .bind(user)
    .bind(projection.scope.workspace_id.as_str())
    .bind(projection.scope.run_id.as_str())
    .bind(projection.scope.agent_id.as_str())
    .bind(projection.owner_actor.as_str())
    .bind(projection.stream_id.as_str())
    .bind(cursor_sequence)
    .bind(projection.cursor.event_id.as_str())
    .bind(projection.projection_digest.as_str())
    .bind(snapshot.snapshot_digest.as_str())
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn select_snapshot(
    connection: &mut SqliteConnection,
    store_id: &str,
    target: &ProjectionTarget,
    source: &ProjectionSource,
) -> Result<Option<sqlx::sqlite::SqliteRow>, ProjectionError> {
    let (present, user) = user_key(&target.scope);
    let horizon = source
        .events
        .last()
        .and_then(|event| event.envelope().sequence.parse::<i64>().ok())
        .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::InvalidSequence))?;
    Ok(sqlx::query(
        "SELECT snapshot_json,snapshot_fingerprint,output_schema_set_json,output_schema_set_fingerprint,output_limits_json,output_limits_fingerprint,source_schema_set_json,source_schema_set_fingerprint,source_limits_json,source_limits_fingerprint,reducer_ref_json,reducer_ref_fingerprint,source_store_id,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,owner_actor,stream_id,cursor_sequence,cursor_event_id,projection_digest,snapshot_digest FROM projection_snapshots WHERE source_store_id=? AND tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND owner_actor=? AND stream_id=? AND cursor_sequence<=? ORDER BY cursor_sequence DESC,snapshot_ordinal DESC LIMIT 1",
    )
    .bind(store_id)
    .bind(target.scope.tenant_id.as_str())
    .bind(present)
    .bind(user)
    .bind(target.scope.workspace_id.as_str())
    .bind(target.scope.run_id.as_str())
    .bind(target.scope.agent_id.as_str())
    .bind(target.actor.as_str())
    .bind(source.stream_id.as_str())
    .bind(horizon)
    .fetch_optional(&mut *connection)
    .await?)
}

enum CandidateFailure {
    Integrity,
    Cursor,
    Incompatible,
    HistoryMismatch,
}

fn validate_snapshot_candidate(
    row: &sqlx::sqlite::SqliteRow,
    store_id: &str,
    registry: &ProjectionRegistry,
    target: &ProjectionTarget,
    source: &ProjectionSource,
    reducer: &ReducerRegistration,
) -> Result<RunTaskProjectionSnapshot, CandidateFailure> {
    let snapshot_json: String = row.get(0);
    if fingerprint(snapshot_json.as_bytes()) != row.get::<String, _>(1) {
        return Err(CandidateFailure::Integrity);
    }
    let output_ref_json: String = row.get(2);
    let output_limits_json: String = row.get(4);
    if fingerprint(output_ref_json.as_bytes()) != row.get::<String, _>(3)
        || fingerprint(output_limits_json.as_bytes()) != row.get::<String, _>(5)
    {
        return Err(CandidateFailure::Integrity);
    }
    let output_ref: SchemaSetRef =
        serde_json::from_str(&output_ref_json).map_err(|_| CandidateFailure::Integrity)?;
    let output_limits: ProtocolLimitsRef =
        serde_json::from_str(&output_limits_json).map_err(|_| CandidateFailure::Integrity)?;
    if canonical(&output_ref).map_err(|_| CandidateFailure::Integrity)? != output_ref_json
        || canonical(&output_limits).map_err(|_| CandidateFailure::Integrity)? != output_limits_json
    {
        return Err(CandidateFailure::Integrity);
    }
    let output = registry
        .resolve_output(&output_ref, &output_limits)
        .map_err(|_| CandidateFailure::Incompatible)?;
    let snapshot = output
        .parse_record::<RunTaskProjectionSnapshot>(snapshot_json.as_bytes())
        .map_err(|_| CandidateFailure::Integrity)?
        .into_inner();
    if canonical(&snapshot).map_err(|_| CandidateFailure::Integrity)? != snapshot_json
        || validate_snapshot_record(&snapshot, reducer, &output).is_err()
    {
        return Err(CandidateFailure::Integrity);
    }
    let source_ref_json: String = row.get(6);
    let source_limits_json: String = row.get(8);
    let reducer_json: String = row.get(10);
    if fingerprint(source_ref_json.as_bytes()) != row.get::<String, _>(7)
        || fingerprint(source_limits_json.as_bytes()) != row.get::<String, _>(9)
        || fingerprint(reducer_json.as_bytes()) != row.get::<String, _>(11)
    {
        return Err(CandidateFailure::Integrity);
    }
    let exact_columns = row.get::<String, _>(12) == store_id
        && row.get::<String, _>(13) == target.scope.tenant_id.as_str()
        && row.get::<i64, _>(14) == user_key(&target.scope).0
        && row.get::<String, _>(15) == user_key(&target.scope).1
        && row.get::<String, _>(16) == target.scope.workspace_id.as_str()
        && row.get::<String, _>(17) == target.scope.run_id.as_str()
        && row.get::<String, _>(18) == target.scope.agent_id.as_str()
        && row.get::<String, _>(19) == target.actor.as_str()
        && row.get::<String, _>(20) == source.stream_id.as_str();
    if !exact_columns
        || snapshot.projection.source_store_id != store_id
        || snapshot.projection.scope != target.scope
        || snapshot.projection.owner_actor != target.actor
        || snapshot.projection.stream_id != source.stream_id
        || snapshot.projection.source_schema_set_ref != *source.schema_set.reference()
        || snapshot.projection.source_protocol_limits_ref != source.limits
        || snapshot.projection.reducer_ref != reducer.reducer_ref
        || snapshot.output_schema_set_ref != output_ref
        || snapshot.output_protocol_limits_ref != output_limits
        || canonical(&snapshot.projection.source_schema_set_ref)
            .map_err(|_| CandidateFailure::Integrity)?
            != source_ref_json
        || canonical(&snapshot.projection.source_protocol_limits_ref)
            .map_err(|_| CandidateFailure::Integrity)?
            != source_limits_json
        || canonical(&snapshot.projection.reducer_ref).map_err(|_| CandidateFailure::Integrity)?
            != reducer_json
    {
        return Err(CandidateFailure::Incompatible);
    }
    if row.get::<String, _>(23) != snapshot.projection_digest.as_str()
        || row.get::<String, _>(24) != snapshot.snapshot_digest.as_str()
    {
        return Err(CandidateFailure::Integrity);
    }
    let cursor = snapshot
        .projection
        .cursor
        .sequence
        .parse::<usize>()
        .map_err(|_| CandidateFailure::Cursor)?;
    if cursor == 0
        || row.get::<i64, _>(21) != i64::try_from(cursor).map_err(|_| CandidateFailure::Cursor)?
        || row.get::<String, _>(22) != snapshot.projection.cursor.event_id.as_str()
        || source.events.get(cursor - 1).map(|event| {
            (
                event.envelope().sequence.as_str(),
                event.envelope().event_id.as_str(),
            )
        }) != Some((
            snapshot.projection.cursor.sequence.as_str(),
            snapshot.projection.cursor.event_id.as_str(),
        ))
    {
        return Err(CandidateFailure::Cursor);
    }
    let authoritative_prefix = history_chain(reducer, &source.events[..cursor])
        .map_err(|_| CandidateFailure::HistoryMismatch)?;
    if authoritative_prefix != snapshot.projection.history_chain_state {
        return Err(CandidateFailure::HistoryMismatch);
    }
    Ok(snapshot)
}

fn apply_snapshot_suffix(
    store_id: &str,
    registry: &ProjectionRegistry,
    source: &ProjectionSource,
    reducer: &ReducerRegistration,
    snapshot: &RunTaskProjectionSnapshot,
) -> Result<RunTaskProjection, ProjectionError> {
    let cursor = snapshot
        .projection
        .cursor
        .sequence
        .parse::<usize>()
        .map_err(|_| ProjectionError::new(ProjectionErrorKind::InvalidSequence))?;
    let mut tasks = BTreeMap::new();
    for task in &snapshot.projection.tasks {
        if tasks
            .insert(
                task.task_id.clone(),
                TaskRecord {
                    parent_task_id: task.parent_task_id.clone(),
                    state: task.state,
                },
            )
            .is_some()
        {
            return Err(ProjectionError::new(ProjectionErrorKind::SnapshotIntegrity));
        }
    }
    let mut state = LifecycleState {
        manifest: snapshot.projection.manifest.clone(),
        run_state: snapshot.projection.run_state,
        tasks,
        sequence: i64::try_from(cursor)
            .map_err(|_| ProjectionError::new(ProjectionErrorKind::InvalidSequence))?,
    };
    let mut history = snapshot.projection.history_chain_state.clone();
    for (index, event) in source.events.iter().enumerate().skip(cursor) {
        let sequence = i64::try_from(index + 1)
            .map_err(|_| ProjectionError::new(ProjectionErrorKind::InvalidSequence))?;
        history = history_step(reducer, history, event)?;
        apply_lifecycle_event(&source.schema_set, &mut state, event, sequence)?;
    }
    build_projection(store_id, registry, source, reducer, state, history)
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod snapshot;

#[cfg(test)]
mod replay;

#[cfg(test)]
#[tokio::test]
async fn reducer() {
    let fixture = test_support::Fixture::new("run_projection-reducer");
    let store = fixture.open_created().await;
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_projection-task", 1, "task_projection"),
        )
        .await
        .unwrap();
    let registry = fixture.projection_registry();
    let first = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let second = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(first.projection, second.projection);
    assert_eq!(
        canonical(&first.projection).unwrap(),
        canonical(&second.projection).unwrap()
    );
}

#[cfg(test)]
#[tokio::test]
async fn reducer_resolution() {
    let fixture = test_support::Fixture::new("run_reducer-resolution");
    let store = fixture.open_created().await;
    let mut registry = fixture.projection_registry();
    registry.reducers.clear();
    assert_eq!(
        store
            .project_full(&registry, &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::ReducerUnavailable
    );
}

#[cfg(test)]
#[tokio::test]
async fn digest_golden() {
    let fixture = test_support::Fixture::new("run_projection-golden");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let mut transaction = store.pool.begin().await.unwrap();
    let source = load_source(
        &mut transaction,
        &registry.sources,
        &fixture.projection_target(),
    )
    .await
    .unwrap();
    let reducer = registry.resolve_reducer(&source.schema_set).unwrap();
    let seed = history_seed(reducer).unwrap();
    let projection =
        full_projection("00000000000000000000000000000000", &registry, &source).unwrap();
    let output = registry
        .resolve_output(
            &reducer.descriptor.output_schema_set_ref,
            &reducer.descriptor.output_protocol_limits_ref,
        )
        .unwrap();
    let snapshot = build_snapshot(&projection, reducer, &output).unwrap();
    assert_eq!(
        reducer.reducer_ref.contract_digest.as_str(),
        "sha256:18e293687a43d6c594681435650866cd65a3475362eac080140b29b66591f964"
    );
    assert_eq!(
        seed.as_str(),
        "sha256:c9e96f10a2d32a42f22d76829ba79feb1ad222825e7aceb72bd22df7479b79e8"
    );
    assert_eq!(
        projection.projection_digest.as_str(),
        "sha256:9afb6790070edc1c46444c0ba20cb4c97a7cbba2d65556b8e95b020ee4ce945e"
    );
    assert_eq!(
        snapshot.snapshot_digest.as_str(),
        "sha256:64207266735307494818b57da04d9c215a4bae025815a62789cc6d04e60f693a"
    );
}

#[cfg(test)]
#[tokio::test]
async fn full_history() {
    let fixture = test_support::Fixture::new("run_full-history");
    let store = fixture.open_created().await;
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_full-history-task", 1, "task_b"),
        )
        .await
        .unwrap();
    let projection = store
        .project_full(&fixture.projection_registry(), &fixture.projection_target())
        .await
        .unwrap()
        .projection;
    assert_eq!(projection.cursor.sequence, "2");
    assert_eq!(projection.manifest, fixture.manifest);
    assert_eq!(projection.tasks[0].task_id.as_str(), "task_b");
}

#[cfg(test)]
#[tokio::test]
async fn invalid_history() {
    let fixture = test_support::Fixture::new("run_invalid-history");
    let store = fixture.open_created().await;
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_invalid-gap", 1, "task_gap"),
        )
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER events_no_update")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET sequence_i64=3 WHERE sequence_i64=2")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query(super::UPDATE_TRIGGER)
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .project_full(&fixture.projection_registry(), &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::AggregateCorrupt
    );

    let unknown_fixture = test_support::Fixture::new("run_unknown-event");
    let unknown_store = unknown_fixture.open_created().await;
    unknown_store
        .create_task(
            &unknown_fixture.source_registry(),
            &unknown_fixture.lifecycle_target(),
            &unknown_fixture.create_task("event_unknown-event", 1, "task_unknown"),
        )
        .await
        .unwrap();
    let envelope_json: String =
        sqlx::query_scalar("SELECT envelope_json FROM events WHERE sequence_i64=2")
            .fetch_one(&unknown_store.pool)
            .await
            .unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(&envelope_json).unwrap();
    envelope["event_type"] = serde_json::Value::String("unknown-lifecycle".to_owned());
    let envelope_json = pareto_protocol::canonical_json(&envelope).unwrap();
    sqlx::query("DROP TRIGGER events_no_update")
        .execute(&unknown_store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET envelope_json=?,envelope_fingerprint=? WHERE sequence_i64=2")
        .bind(&envelope_json)
        .bind(fingerprint(envelope_json.as_bytes()))
        .execute(&unknown_store.pool)
        .await
        .unwrap();
    sqlx::query(super::UPDATE_TRIGGER)
        .execute(&unknown_store.pool)
        .await
        .unwrap();
    assert_eq!(
        unknown_store
            .project_full(
                &unknown_fixture.projection_registry(),
                &unknown_fixture.projection_target(),
            )
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::AggregateCorrupt
    );
}

#[cfg(test)]
#[tokio::test]
async fn no_snapshot() {
    let fixture = test_support::Fixture::new("run_no-snapshot");
    let store = fixture.open_created().await;
    let load = store
        .project_snapshot_assisted(&fixture.projection_registry(), &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(load.snapshot_disposition, SnapshotDisposition::Missing);
    assert_eq!(load.projection.cursor.sequence, "1");
}

#[cfg(test)]
#[tokio::test]
async fn isolation() {
    let fixture = test_support::Fixture::new("run_projection-isolation");
    let store = fixture.open_created().await;
    let mut targets = Vec::new();
    let mut actor = fixture.projection_target();
    actor.actor = AgentId::parse("agent_intruder").unwrap();
    targets.push(actor);
    let mut tenant = fixture.projection_target();
    tenant.scope.tenant_id = pareto_protocol::TenantId::parse("tenant_other").unwrap();
    targets.push(tenant);
    let mut user = fixture.projection_target();
    user.scope.user_id = Some(pareto_protocol::UserId::parse("user_other").unwrap());
    targets.push(user);
    let mut absent_user = fixture.projection_target();
    absent_user.scope.user_id = None;
    targets.push(absent_user);
    let mut workspace = fixture.projection_target();
    workspace.scope.workspace_id = pareto_protocol::WorkspaceId::parse("workspace_other").unwrap();
    targets.push(workspace);
    let mut run = fixture.projection_target();
    run.scope.run_id = pareto_protocol::RunId::parse("run_other").unwrap();
    targets.push(run);
    let mut agent = fixture.projection_target();
    agent.scope.agent_id = AgentId::parse("agent_other").unwrap();
    agent.actor = agent.scope.agent_id.clone();
    targets.push(agent);
    for target in targets {
        assert_eq!(
            store
                .project_full(&fixture.projection_registry(), &target)
                .await
                .unwrap_err()
                .kind,
            ProjectionErrorKind::Unauthorized
        );
    }
}

#[cfg(test)]
#[tokio::test]
async fn compatibility() {
    let fixture = test_support::Fixture::new("run_projection-compatibility");
    let store = fixture.open_created().await;
    let mut registry = fixture.projection_registry();
    registry.sources.0.clear();
    assert_eq!(
        store
            .project_full(&registry, &fixture.projection_target())
            .await
            .unwrap_err()
            .kind,
        ProjectionErrorKind::SchemaUnavailable
    );
}

#[cfg(test)]
#[tokio::test]
async fn concurrency() {
    let fixture = test_support::Fixture::new("run_projection-concurrency");
    let store = fixture.open_created().await;
    let registry = fixture.projection_registry();
    let projection = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    store
        .create_task(
            &fixture.source_registry(),
            &fixture.lifecycle_target(),
            &fixture.create_task("event_concurrent-after", 1, "task_after"),
        )
        .await
        .unwrap();
    assert_eq!(projection.projection.cursor.sequence, "1");
    let later = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    assert_eq!(later.projection.cursor.sequence, "2");
}

#[cfg(test)]
#[tokio::test]
#[ignore = "reproducible local SQLite observation; no performance threshold"]
async fn performance_observation() {
    use std::time::Instant;

    use pareto_protocol::{EventId, TaskCreatedPayload, TaskId, TaskState};

    use super::lifecycle::lifecycle_event;
    use super::{AdmittedAppend, AppendResult};

    let fixture = test_support::Fixture::new("run_projection-observation");
    let store = fixture.open_created().await;
    let stream = lifecycle_stream_id(&fixture.scope).unwrap();
    let registry = fixture.projection_registry();
    async fn append_range(
        fixture: &test_support::Fixture,
        store: &EventStore,
        stream: &StreamId,
        first: i64,
        last: i64,
    ) {
        for sequence in first..=last {
            let event = lifecycle_event(
                &fixture.set,
                &fixture.limits,
                &fixture.scope,
                &fixture.scope.agent_id,
                stream,
                &EventId::parse(format!("event_observation-{sequence}")).unwrap(),
                sequence,
                "2026-08-25T02:00:00.000Z",
                &format!("corr-observation-{sequence}"),
                "task-created",
                &TaskCreatedPayload {
                    task_id: TaskId::parse(format!("task_observation-{sequence:04}")).unwrap(),
                    parent_task_id: None,
                    initial_state: TaskState::Created,
                },
            )
            .unwrap();
            let result = store
                .append(AdmittedAppend {
                    event,
                    schema_set: fixture.set.clone(),
                    limits: fixture.limits.clone(),
                })
                .await
                .unwrap();
            assert!(matches!(result, AppendResult::Appended { .. }));
        }
    }

    let start = Instant::now();
    let full_1 = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full_1_elapsed = start.elapsed();
    let start = Instant::now();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let snapshot_1_elapsed = start.elapsed();

    append_range(&fixture, &store, &stream, 2, 2).await;
    let start = Instant::now();
    let suffix_1 = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let suffix_1_elapsed = start.elapsed();

    append_range(&fixture, &store, &stream, 3, 10).await;
    let start = Instant::now();
    let full_10 = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full_10_elapsed = start.elapsed();
    let start = Instant::now();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let snapshot_10_elapsed = start.elapsed();

    append_range(&fixture, &store, &stream, 11, 20).await;
    let start = Instant::now();
    let suffix_10 = store
        .project_snapshot_assisted(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let suffix_10_elapsed = start.elapsed();

    append_range(&fixture, &store, &stream, 21, 100).await;
    let start = Instant::now();
    let full_100 = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full_100_elapsed = start.elapsed();
    let start = Instant::now();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let snapshot_100_elapsed = start.elapsed();

    append_range(&fixture, &store, &stream, 101, 1000).await;
    let start = Instant::now();
    let full_1000 = store
        .project_full(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let full_1000_elapsed = start.elapsed();
    let start = Instant::now();
    let replay = store
        .recorded_replay(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let replay_1000_elapsed = start.elapsed();
    let start = Instant::now();
    store
        .create_projection_snapshot(&registry, &fixture.projection_target())
        .await
        .unwrap();
    let snapshot_1000_elapsed = start.elapsed();
    let snapshot_bytes: i64 =
        sqlx::query_scalar("SELECT SUM(length(snapshot_json)) FROM projection_snapshots")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    let database_bytes: i64 = sqlx::query_scalar(
        "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
    )
    .fetch_one(&store.pool)
    .await
    .unwrap();

    assert_eq!(full_1.projection.cursor.sequence, "1");
    assert_eq!(suffix_1.projection.cursor.sequence, "2");
    assert_eq!(full_10.projection.cursor.sequence, "10");
    assert_eq!(suffix_10.projection.cursor.sequence, "20");
    assert_eq!(full_100.projection.cursor.sequence, "100");
    assert_eq!(full_1000.projection, replay);
    eprintln!(
        "projection_observation full[1]={full_1_elapsed:?} snapshot[1]={snapshot_1_elapsed:?} suffix[1]={suffix_1_elapsed:?} full[10]={full_10_elapsed:?} snapshot[10]={snapshot_10_elapsed:?} suffix[10]={suffix_10_elapsed:?} full[100]={full_100_elapsed:?} snapshot[100]={snapshot_100_elapsed:?} full[1000]={full_1000_elapsed:?} snapshot[1000]={snapshot_1000_elapsed:?} recorded_replay[1000]={replay_1000_elapsed:?} snapshot_json_bytes={snapshot_bytes} database_bytes={database_bytes}"
    );
}
