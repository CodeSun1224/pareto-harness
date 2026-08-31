//! Kernel-owned Effect event stream, fixed-horizon fold, and projection.

use std::{collections::BTreeMap, sync::Arc};

use super::lifecycle::{LifecycleTarget, lifecycle_event, load_established};
use super::runtime_control::{
    ClockSample, EffectSettlementAccountingV1, HookControlEventContext, OperationLease,
    ProtectedOperationProposal, RuntimeControlTarget, control_event,
    effect_cancellation_is_effective, plan_effect_conservative_settlement,
    plan_effect_recovery_settlement, plan_hook_reservation, plan_hook_settlement,
    prepare_effect_reservation_event, prepare_effect_settlement_event, runtime_control_stream_id,
    validate_runtime_control_history,
};
use super::{
    AdmittedAppend, AdmittedRead, AppendResult, AtomicPairFault, ErrorKind, EventStore,
    EventStoreError, KernelAuthority, PreparedEvent, SchemaRegistry, append_atomic_pair, canonical,
    check_prepared_idempotency, insert_prepared, user_key, validate_row,
};
use pareto_protocol::{
    AgentId, BoundaryInventoryRevisionV2, CallbackId, Digest, EffectAttemptConcludedPayloadV1,
    EffectBoundaryOutcomeV2, EffectBoundaryRecordV2, EffectDispatchClaimedPayloadV1,
    EffectDispatchStateV1, EffectExecutorDescriptorV1, EffectExternalConclusionV1, EffectId,
    EffectIntendedPayloadV1, EffectLateReceiptObservedPayloadV1, EffectMessageRejectedPayloadV1,
    EffectPairBindingV1, EffectPairId, EffectPairKindV1, EffectProjectionEntryV1,
    EffectProjectionHashViewV1, EffectProjectionV1, EffectReceiptAdmittedPayloadV1,
    EffectReceiptObservationV1, EffectReceiptOutcomeClassV1, EffectReconciledPayloadV1,
    EffectReconciliationBindingV2, EffectReconciliationObservedPayloadV1,
    EffectReconciliationRequiredPayloadV1, EffectReconciliationStateV1, EffectRecoveryCauseV1,
    EffectRegistryRevisionV1, EffectRequestV1, EffectStreamInitializedPayloadV1, EventCursor,
    EventId, IsolationScope, OperationOutcomeV1, OperationReservedPayloadV1,
    OperationSettledPayloadV1, ProtocolLimitsRef, RevisionId, RevisionMetadata, SchemaSet,
    StreamId, ValidatedEvent, derive_revision_id, digest_json,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

const EFFECT_REDUCER_REVISION: &str = "rev_effect-reducer-v1";
const EFFECT_OUTPUT_READER_REVISION: &str = "rev_effect-projection-reader-v1";
const EFFECT_HISTORY_REVISION: &str = "rev_effect-history-chain-v1";
const ROW_COLUMNS: &str = "envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectErrorKind {
    Unauthorized,
    ManifestInvalid,
    AggregateNotFound,
    AggregateCorrupt,
    SchemaUnavailable,
    CursorMismatch,
    IdempotencyConflict,
    PartialPair,
    Store,
}

#[derive(Debug)]
pub(super) struct EffectError {
    kind: EffectErrorKind,
}

impl EffectError {
    fn new(kind: EffectErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone)]
pub(super) struct EffectTarget {
    pub(super) scope: IsolationScope,
    pub(super) actor: AgentId,
}

#[derive(Clone)]
pub(super) struct InitializeEffectStream {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) effect_registry_revision: RevisionId,
    pub(super) effect_registry_config_digest: Digest,
}

#[derive(Clone)]
struct EffectAggregate {
    initialization: EffectStreamInitializedPayloadV1,
    effects: BTreeMap<EffectId, EffectProjectionEntryV1>,
    intents: BTreeMap<EffectId, EffectIntendedPayloadV1>,
    claims: BTreeMap<EffectId, EffectDispatchClaimedPayloadV1>,
    late_receipt_count: u64,
    rejected_count: u64,
    reconciliation_observations: BTreeMap<EventId, Digest>,
    terminals: BTreeMap<EffectId, EffectTerminalFact>,
    reconciled: BTreeMap<EffectId, EffectReconciledPayloadV1>,
    inclusive_cursor: EventCursor,
    history_digest: Digest,
}

#[derive(Clone)]
enum EffectTerminalFact {
    Attempt(EffectAttemptConcludedPayloadV1),
    Receipt(EffectReceiptAdmittedPayloadV1),
    Reconciliation(EffectReconciliationRequiredPayloadV1),
}

#[derive(Clone)]
struct RequestEffectCommandV1 {
    proposal: ProtectedOperationProposal,
    request: EffectRequestV1,
    effect_id: EffectId,
    attempt_id: pareto_protocol::EffectAttemptId,
    pair_id: EffectPairId,
    effect_event_id: EventId,
    effect_kind: String,
    request_digest: Digest,
    idempotency_key_digest: Digest,
    occurred_at: String,
    correlation_id: String,
    clock: ClockSample,
}

#[derive(Debug)]
struct EffectIntentAdmissionResult {
    cursor: EventCursor,
    lease: Option<OperationLease>,
    already_committed: bool,
}

#[derive(Clone)]
struct ClaimEffectCommandV1 {
    event_id: EventId,
    effect_id: EffectId,
    attempt_id: pareto_protocol::EffectAttemptId,
    expected_effect_cursor: EventCursor,
    occurred_at: String,
    correlation_id: String,
    clock: ClockSample,
    claim_policy_revision: RevisionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct EffectDispatchLease {
    scope: IsolationScope,
    effect_id: EffectId,
    attempt_id: pareto_protocol::EffectAttemptId,
    claim_event_id: EventId,
    executor_revision: RevisionId,
    executor_descriptor_digest: Digest,
    executor_config_digest: Digest,
    external_key_digest: Digest,
    process_epoch_digest: Digest,
    seal: Digest,
}

#[derive(Debug)]
struct EffectClaimResult {
    cursor: EventCursor,
    lease: EffectDispatchLease,
    already_committed: bool,
}

#[derive(Debug)]
struct EffectDispatchResult {
    cursor: EventCursor,
    execution: Option<FakeEffectExecution>,
    already_claimed: bool,
}

#[derive(Clone)]
struct AdmitEffectReceiptCommandV1 {
    control_event_id: EventId,
    effect_event_id: EventId,
    pair_id: EffectPairId,
    callback_id: CallbackId,
    occurred_at: String,
    correlation_id: String,
    clock: ClockSample,
}

#[derive(Clone)]
struct ObserveLateReceiptCommandV1 {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
}

#[derive(Clone, Serialize)]
struct RecoverEffectCommandV1 {
    effect_id: EffectId,
    attempt_id: pareto_protocol::EffectAttemptId,
    cause: EffectRecoveryCauseV1,
    expected_effect_cursor: EventCursor,
    control_event_id: EventId,
    effect_event_id: EventId,
    pair_id: EffectPairId,
    lost_process_epoch_digest: Digest,
    current_process_epoch_digest: Digest,
    occurred_at: String,
    correlation_id: String,
    clock: ClockSample,
    command_fingerprint: Digest,
}

#[derive(Debug)]
struct EffectRecoveryResult {
    already_committed: bool,
}

#[derive(Clone, Serialize)]
struct ReconcileEffectCommandV1 {
    effect_id: EffectId,
    attempt_id: pareto_protocol::EffectAttemptId,
    expected_effect_cursor: EventCursor,
    observation_event_id: EventId,
    reconciled_event_id: EventId,
    producer_revision: RevisionId,
    source_observation_event_ids: Vec<EventId>,
    resolution: EffectReconciliationStateV1,
    occurred_at: String,
    correlation_id: String,
    command_fingerprint: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FakeEffectExecution {
    Observation(EffectReceiptObservationV1),
    FailedBeforeApply { reason_code: String },
    ResponseLost,
    CrashedAfterReturn(EffectReceiptObservationV1),
}

trait FakeEffectExecutor {
    fn invoke(&self, lease: &EffectDispatchLease) -> FakeEffectExecution;
}

#[derive(Clone, Serialize)]
struct EffectReserveIntentCommandV1 {
    scope: IsolationScope,
    owner: AgentId,
    control_stream_id: StreamId,
    effect_stream_id: StreamId,
    expected_control_cursor: EventCursor,
    expected_effect_cursor: EventCursor,
    control_sequence: i64,
    effect_sequence: i64,
    pair: EffectPairBindingV1,
    occurred_at: String,
    correlation_id: String,
    control_payload: OperationReservedPayloadV1,
    effect_payload: EffectIntendedPayloadV1,
    clock: ClockSample,
}

#[derive(Debug)]
struct EffectReserveIntentResult {
    control: AppendResult,
    effect: AppendResult,
    lease: OperationLease,
    already_committed: bool,
}

trait EffectTerminalPayload: Clone + Serialize {
    const EVENT_TYPE: &'static str;
    fn effect_id(&self) -> &EffectId;
    fn attempt_id(&self) -> &pareto_protocol::EffectAttemptId;
    fn conclusion(&self) -> EffectExternalConclusionV1;
    fn accounted_usage(&self) -> &[pareto_protocol::BudgetVectorEntryV1];
    fn pair(&self) -> &EffectPairBindingV1;
    fn set_pair(&mut self, pair: EffectPairBindingV1);
}

macro_rules! terminal_payload {
    ($type:ty, $event_type:literal) => {
        impl EffectTerminalPayload for $type {
            const EVENT_TYPE: &'static str = $event_type;
            fn effect_id(&self) -> &EffectId {
                &self.effect_id
            }
            fn attempt_id(&self) -> &pareto_protocol::EffectAttemptId {
                &self.attempt_id
            }
            fn conclusion(&self) -> EffectExternalConclusionV1 {
                self.external_conclusion
            }
            fn accounted_usage(&self) -> &[pareto_protocol::BudgetVectorEntryV1] {
                &self.accounted_usage
            }
            fn pair(&self) -> &EffectPairBindingV1 {
                &self.pair
            }
            fn set_pair(&mut self, pair: EffectPairBindingV1) {
                self.pair = pair;
            }
        }
    };
}

terminal_payload!(EffectAttemptConcludedPayloadV1, "effect-attempt-concluded");
terminal_payload!(EffectReceiptAdmittedPayloadV1, "effect-receipt-admitted");
terminal_payload!(
    EffectReconciliationRequiredPayloadV1,
    "effect-reconciliation-required"
);

#[derive(Clone, Serialize)]
struct EffectTerminalPairCommandV1<T> {
    scope: IsolationScope,
    owner: AgentId,
    control_stream_id: StreamId,
    effect_stream_id: StreamId,
    expected_control_cursor: EventCursor,
    expected_effect_cursor: EventCursor,
    control_sequence: i64,
    effect_sequence: i64,
    pair: EffectPairBindingV1,
    occurred_at: String,
    correlation_id: String,
    control_payload: OperationSettledPayloadV1,
    effect_payload: T,
}

type EffectTerminalConclusionCommandV1 =
    EffectTerminalPairCommandV1<EffectAttemptConcludedPayloadV1>;

#[derive(Debug)]
struct EffectTerminalConclusionResult {
    control: AppendResult,
    effect: AppendResult,
    already_committed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairPresence {
    Zero,
    Two,
}

fn zero_digest() -> Result<Digest, EffectError> {
    Digest::parse(format!("sha256:{}", "0".repeat(64)))
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))
}

fn digest_bytes(domain: &str, bytes: &[u8]) -> Result<Digest, EffectError> {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes);
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))
}

fn map_store_error(error: EventStoreError) -> EffectError {
    let kind = match error.kind {
        ErrorKind::IdempotencyConflict => EffectErrorKind::IdempotencyConflict,
        ErrorKind::DatabaseCorrupt => EffectErrorKind::PartialPair,
        _ => EffectErrorKind::Store,
    };
    EffectError::new(kind)
}

fn next_sequence(cursor: &EventCursor) -> Result<i64, EffectError> {
    cursor
        .sequence
        .parse::<i64>()
        .ok()
        .and_then(|sequence| sequence.checked_add(1))
        .ok_or_else(|| EffectError::new(EffectErrorKind::IdempotencyConflict))
}

fn pair_command_fingerprint(command: &EffectReserveIntentCommandV1) -> Result<Digest, EffectError> {
    let mut normalized = command.clone();
    normalized.pair.pair_fingerprint = zero_digest()?;
    normalized.control_payload.effect_pair = Some(normalized.pair.clone());
    normalized.effect_payload.pair = normalized.pair.clone();
    let bytes =
        canonical(&normalized).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    digest_bytes("effect-reserve-intent-command-v1", bytes.as_bytes())
}

fn terminal_pair_command_fingerprint<T: EffectTerminalPayload>(
    command: &EffectTerminalPairCommandV1<T>,
) -> Result<Digest, EffectError> {
    let mut normalized = command.clone();
    normalized.pair.pair_fingerprint = zero_digest()?;
    normalized.control_payload.effect_pair = Some(normalized.pair.clone());
    normalized.effect_payload.set_pair(normalized.pair.clone());
    let bytes =
        canonical(&normalized).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    digest_bytes("effect-terminal-conclusion-command-v1", bytes.as_bytes())
}

fn external_key_digest(intent: &EffectIntendedPayloadV1) -> Result<Digest, EffectError> {
    let bytes = canonical(&serde_json::json!({
        "scope": intent.recovery_base_key.scope,
        "effect_id": intent.effect_id,
        "attempt_id": intent.attempt_id,
        "idempotency_key_digest": intent.idempotency_key_digest,
        "executor_revision": intent.executor_revision,
        "executor_descriptor_digest": intent.executor_descriptor_digest,
        "executor_config_digest": intent.executor_config_digest,
    }))
    .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    digest_bytes("effect-external-key-v1", bytes.as_bytes())
}

fn make_dispatch_lease(
    scope: &IsolationScope,
    payload: &EffectDispatchClaimedPayloadV1,
) -> Result<EffectDispatchLease, EffectError> {
    let mut lease = EffectDispatchLease {
        scope: scope.clone(),
        effect_id: payload.effect_id.clone(),
        attempt_id: payload.attempt_id.clone(),
        claim_event_id: payload.recovery_key.claim_event_id.clone(),
        executor_revision: payload.executor_revision.clone(),
        executor_descriptor_digest: payload.executor_descriptor_digest.clone(),
        executor_config_digest: payload.executor_config_digest.clone(),
        external_key_digest: payload.external_key_digest.clone(),
        process_epoch_digest: payload.recovery_key.claim_process_epoch_digest.clone(),
        seal: zero_digest()?,
    };
    let bytes =
        canonical(&lease).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    lease.seal = digest_bytes("effect-dispatch-lease-v1", bytes.as_bytes())?;
    Ok(lease)
}

fn verify_dispatch_lease(
    descriptor: &EffectExecutorDescriptorV1,
    lease: &EffectDispatchLease,
) -> Result<(), EffectError> {
    let mut normalized = lease.clone();
    normalized.seal = zero_digest()?;
    let bytes =
        canonical(&normalized).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    let expected = digest_bytes("effect-dispatch-lease-v1", bytes.as_bytes())?;
    if lease.seal != expected
        || descriptor.validate().is_err()
        || descriptor.metadata.revision_id != lease.executor_revision
        || descriptor.metadata.content_digest != lease.executor_descriptor_digest
        || descriptor.content.config_digest != lease.executor_config_digest
    {
        return Err(EffectError::new(EffectErrorKind::Unauthorized));
    }
    Ok(())
}

fn execute_fake_effect(
    descriptor: &EffectExecutorDescriptorV1,
    lease: &EffectDispatchLease,
    executor: &dyn FakeEffectExecutor,
) -> Result<FakeEffectExecution, EffectError> {
    verify_dispatch_lease(descriptor, lease)?;
    Ok(executor.invoke(lease))
}

fn recovery_command_fingerprint(command: &RecoverEffectCommandV1) -> Result<Digest, EffectError> {
    let mut normalized = command.clone();
    normalized.command_fingerprint = zero_digest()?;
    let bytes =
        canonical(&normalized).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    digest_bytes("pareto.effect-recovery.command.v1", bytes.as_bytes())
}

fn expected_effect_id(
    scope: &IsolationScope,
    registry_revision: &RevisionId,
    effect_revision: &RevisionId,
    effect_kind: &str,
    idempotency_key_digest: &Digest,
) -> Result<EffectId, EffectError> {
    let bytes = canonical(&serde_json::json!({
        "scope": scope,
        "registry_revision": registry_revision,
        "effect_revision": effect_revision,
        "effect_kind": effect_kind,
        "idempotency_key_digest": idempotency_key_digest,
    }))
    .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))?;
    let digest = digest_bytes("pareto.effect-id.v1", bytes.as_bytes())?;
    EffectId::parse(format!("effect_{}", &digest.as_str()[7..39]))
        .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))
}

fn expected_request_digest(
    request: &EffectRequestV1,
    registry_revision: &RevisionId,
    registry_config_digest: &Digest,
    effect_revision: &RevisionId,
    operation_contract_revision: &RevisionId,
    timeout_policy_revision: &RevisionId,
) -> Result<Digest, EffectError> {
    let bytes = canonical(&serde_json::json!({
        "request": request,
        "registry_revision": registry_revision,
        "registry_config_digest": registry_config_digest,
        "effect_revision": effect_revision,
        "operation_contract_revision": operation_contract_revision,
        "timeout_policy_revision": timeout_policy_revision,
    }))
    .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))?;
    digest_bytes("pareto.effect-request.v1", bytes.as_bytes())
}

fn reconciliation_command_fingerprint(
    command: &ReconcileEffectCommandV1,
) -> Result<Digest, EffectError> {
    let mut normalized = command.clone();
    normalized.command_fingerprint = zero_digest()?;
    let bytes =
        canonical(&normalized).map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    digest_bytes("pareto.effect-reconciliation.command.v1", bytes.as_bytes())
}

async fn pair_presence(
    connection: &mut sqlx::SqliteConnection,
    control: &PreparedEvent,
    effect: &PreparedEvent,
) -> Result<PairPresence, EffectError> {
    let control = check_prepared_idempotency(connection, control)
        .await
        .map_err(map_store_error)?;
    let effect = check_prepared_idempotency(connection, effect)
        .await
        .map_err(map_store_error)?;
    match (control, effect) {
        (None, None) => Ok(PairPresence::Zero),
        (Some(_), Some(_)) => Ok(PairPresence::Two),
        _ => Err(EffectError::new(EffectErrorKind::PartialPair)),
    }
}

impl EventStore {
    pub(super) async fn initialize_effect_stream(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        command: &InitializeEffectStream,
    ) -> Result<EventCursor, EffectError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let manifest = &lifecycle.state.manifest;
        if manifest.schema_ref.major != 3
            || manifest.revisions.get("effect_registry") != Some(&command.effect_registry_revision)
            || manifest.effect_registry_config_digest.as_ref()
                != Some(&command.effect_registry_config_digest)
        {
            return Err(EffectError::new(EffectErrorKind::ManifestInvalid));
        }
        let checkpoint = lifecycle
            .checkpoints
            .get(&lifecycle.state.sequence)
            .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let stream_id = effect_stream_id(&target.scope)?;
        let payload = EffectStreamInitializedPayloadV1 {
            source_run_id: target.scope.run_id.clone(),
            lifecycle_cursor: EventCursor {
                sequence: lifecycle.state.sequence.to_string(),
                event_id: checkpoint.event_id.clone(),
            },
            effect_registry_revision: command.effect_registry_revision.clone(),
            effect_registry_config_digest: command.effect_registry_config_digest.clone(),
            boundary_recording_policy_revision: manifest
                .boundary_recording_policy_ref
                .revision_id
                .clone(),
            source_schema_set_ref: manifest.schema_set_ref.clone(),
            protocol_limits_digest: lifecycle.limits.digest.clone(),
            reducer_revision: RevisionId::parse(EFFECT_REDUCER_REVISION)
                .map_err(|_| EffectError::new(EffectErrorKind::SchemaUnavailable))?,
            output_reader_revision: RevisionId::parse(EFFECT_OUTPUT_READER_REVISION)
                .map_err(|_| EffectError::new(EffectErrorKind::SchemaUnavailable))?,
            history_digest_revision: RevisionId::parse(EFFECT_HISTORY_REVISION)
                .map_err(|_| EffectError::new(EffectErrorKind::SchemaUnavailable))?,
        };
        let event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &stream_id,
            &command.event_id,
            1,
            &command.occurred_at,
            &command.correlation_id,
            "effect-stream-initialized",
            &payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))?;
        let authority = KernelAuthority::authenticated(
            target.scope.clone(),
            target.actor.clone(),
            Some(stream_id),
            lifecycle.schema_set.reference().clone(),
            lifecycle.limits.clone(),
        );
        let admitted =
            AdmittedAppend::admit(&authority, event, lifecycle.schema_set, lifecycle.limits)
                .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        self.append(admitted)
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(EventCursor {
            sequence: "1".to_owned(),
            event_id: command.event_id.clone(),
        })
    }

    async fn request_effect(
        &self,
        registry: &SchemaRegistry,
        effect_registry: &EffectRegistryRevisionV1,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        command: &RequestEffectCommandV1,
    ) -> Result<EffectIntentAdmissionResult, EffectError> {
        if target.scope != control_target.scope
            || target.actor != control_target.principal
            || effect_registry.validate().is_err()
            || effect_registry_config_digest(&effect_registry.registrations)?
                != effect_registry.config_digest
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let registration = effect_registry
            .registrations
            .binary_search_by(|candidate| candidate.effect_kind.cmp(&command.effect_kind))
            .ok()
            .and_then(|index| effect_registry.registrations.get(index))
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        if lifecycle.state.manifest.schema_ref.major != 3
            || lifecycle.state.manifest.revisions.get("effect_registry")
                != Some(&effect_registry.metadata.revision_id)
            || lifecycle
                .state
                .manifest
                .effect_registry_config_digest
                .as_ref()
                != Some(&effect_registry.config_digest)
        {
            return Err(EffectError::new(EffectErrorKind::ManifestInvalid));
        }
        let request_bytes = canonical(&command.request)
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        if request_bytes.len() as u64 > registration.limits.max_request_bytes
            || lifecycle
                .schema_set
                .parse_record::<EffectRequestV1>(request_bytes.as_bytes())
                .is_err()
            || command.request.request_schema_ref != registration.request_schema_ref
            || lifecycle
                .schema_set
                .validate_value_against(
                    &command.request.request_schema_ref,
                    &command.request.request,
                )
                .is_err()
            || command.request.effect_kind != command.effect_kind
            || command.request.subject_actor != control_target.principal
            || command.request.task_id != command.proposal.task_id
            || command.request.deadline_at != command.proposal.absolute_deadline_utc
            || command.request.correlation_id != command.correlation_id
            || command.request.client_idempotency_key_digest != command.idempotency_key_digest
            || command.effect_id
                != expected_effect_id(
                    &target.scope,
                    &effect_registry.metadata.revision_id,
                    &registration.effect_revision,
                    &command.effect_kind,
                    &command.idempotency_key_digest,
                )?
            || command.request_digest
                != expected_request_digest(
                    &command.request,
                    &effect_registry.metadata.revision_id,
                    &effect_registry.config_digest,
                    &registration.effect_revision,
                    &registration.operation_contract_revision,
                    &command.proposal.timeout_policy_revision,
                )?
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let effect_cursor = current_effect_cursor(&mut transaction, target).await?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &effect_cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        if let Some(existing) = aggregate.intents.values().find(|intent| {
            intent.idempotency_key_digest == command.idempotency_key_digest
                || intent.effect_id == command.effect_id
        }) {
            let exact = existing.effect_id == command.effect_id
                && existing.attempt_id == command.attempt_id
                && existing.effect_kind == command.effect_kind
                && existing.request_digest == command.request_digest
                && existing.idempotency_key_digest == command.idempotency_key_digest
                && existing.pair.operation_id == command.proposal.operation_id
                && existing.pair.reservation_id == command.proposal.reservation_id
                && existing.effect_revision == registration.effect_revision
                && existing.executor_revision == registration.executor_revision
                && existing.executor_descriptor_digest == registration.executor_descriptor_digest
                && existing.executor_config_digest == registration.executor_config_digest;
            transaction
                .rollback()
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            if exact {
                return Ok(EffectIntentAdmissionResult {
                    cursor: aggregate.inclusive_cursor,
                    lease: None,
                    already_committed: true,
                });
            }
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let planned = plan_hook_reservation(
            &mut transaction,
            registry,
            control_target,
            &command.proposal,
            &command.clock,
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        if registration.operation_contract_revision != planned.payload.operation_contract_revision
            || registration.adapter_revision != planned.payload.adapter_revision
            || registration.producer_revision != planned.payload.producer_revision
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let pair = EffectPairBindingV1 {
            pair_id: command.pair_id.clone(),
            pair_kind: EffectPairKindV1::ReserveIntent,
            pair_fingerprint: zero_digest()?,
            control_event_id: command.proposal.event_id.clone(),
            effect_event_id: command.effect_event_id.clone(),
            operation_id: command.proposal.operation_id.clone(),
            reservation_id: command.proposal.reservation_id.clone(),
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            control_prepared_digest: zero_digest()?,
            effect_prepared_digest: zero_digest()?,
        };
        let effect_payload = EffectIntendedPayloadV1 {
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            effect_kind: command.effect_kind.clone(),
            subject_actor: control_target.principal.clone(),
            task_id: command.proposal.task_id.clone(),
            request_digest: command.request_digest.clone(),
            idempotency_key_digest: command.idempotency_key_digest.clone(),
            effect_registry_revision: effect_registry.metadata.revision_id.clone(),
            effect_registry_config_digest: effect_registry.config_digest.clone(),
            effect_revision: registration.effect_revision.clone(),
            executor_revision: registration.executor_revision.clone(),
            executor_descriptor_digest: registration.executor_descriptor_digest.clone(),
            executor_config_digest: registration.executor_config_digest.clone(),
            pair: pair.clone(),
            reserved_usage: planned.payload.trusted_reservation.clone(),
            recovery_base_key: pareto_protocol::EffectRecoveryBaseKeyV1 {
                scope: target.scope.clone(),
                effect_id: command.effect_id.clone(),
                attempt_id: command.attempt_id.clone(),
                operation_id: command.proposal.operation_id.clone(),
                reservation_id: command.proposal.reservation_id.clone(),
                executor_revision: registration.executor_revision.clone(),
                executor_descriptor_digest: registration.executor_descriptor_digest.clone(),
                executor_config_digest: registration.executor_config_digest.clone(),
                source_schema_set_ref: lifecycle.schema_set.reference().clone(),
                meter_contract_revision: planned.payload.timeout_key.meter_revision.clone(),
                recovery_contract_revision: planned.payload.timeout_key.recovery_revision.clone(),
                initial_process_epoch_digest: digest_bytes(
                    "effect-initial-process-epoch-v1",
                    command.clock.process_epoch.as_bytes(),
                )?,
                deadline_at: planned.payload.absolute_deadline_utc.clone(),
            },
        };
        let result = self
            .append_effect_reserve_intent_pair(
                registry,
                target,
                control_target,
                EffectReserveIntentCommandV1 {
                    scope: target.scope.clone(),
                    owner: target.actor.clone(),
                    control_stream_id: runtime_control_stream_id(&target.scope)
                        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?,
                    effect_stream_id: effect_stream_id(&target.scope)?,
                    expected_control_cursor: planned.expected_cursor,
                    expected_effect_cursor: effect_cursor.clone(),
                    control_sequence: 0,
                    effect_sequence: 0,
                    pair,
                    occurred_at: command.occurred_at.clone(),
                    correlation_id: command.correlation_id.clone(),
                    control_payload: planned.payload,
                    effect_payload,
                    clock: command.clock.clone(),
                },
                AtomicPairFault::None,
            )
            .await?;
        Ok(EffectIntentAdmissionResult {
            cursor: EventCursor {
                sequence: next_sequence(&effect_cursor)?.to_string(),
                event_id: command.effect_event_id.clone(),
            },
            lease: Some(result.lease),
            already_committed: result.already_committed,
        })
    }

    async fn claim_effect_dispatch(
        &self,
        registry: &SchemaRegistry,
        descriptor: &EffectExecutorDescriptorV1,
        target: &EffectTarget,
        command: &ClaimEffectCommandV1,
    ) -> Result<EffectClaimResult, EffectError> {
        if descriptor.validate().is_err() {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let current_cursor = current_effect_cursor(&mut transaction, target).await?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &current_cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        let intent = aggregate
            .intents
            .get(&command.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        if intent.attempt_id != command.attempt_id
            || descriptor.metadata.revision_id != intent.executor_revision
            || descriptor.metadata.content_digest != intent.executor_descriptor_digest
            || descriptor.content.config_digest != intent.executor_config_digest
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let claim_process_epoch_digest = digest_bytes(
            "effect-claim-process-epoch-v1",
            command.clock.process_epoch.as_bytes(),
        )?;
        let clock_bytes = canonical(&command.clock)
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let claim_clock_digest = digest_bytes("effect-claim-clock-v1", clock_bytes.as_bytes())?;
        let external_key_digest = external_key_digest(intent)?;
        if let Some(existing) = aggregate.claims.get(&command.effect_id) {
            let exact = existing.attempt_id == command.attempt_id
                && existing.recovery_key.claim_event_id == command.event_id
                && existing.recovery_key.claim_process_epoch_digest == claim_process_epoch_digest
                && existing.recovery_key.claim_clock_digest == claim_clock_digest
                && existing.recovery_key.claim_policy_revision == command.claim_policy_revision
                && existing.external_key_digest == external_key_digest;
            transaction
                .rollback()
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            if !exact {
                return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
            }
            return Ok(EffectClaimResult {
                cursor: aggregate.inclusive_cursor,
                lease: make_dispatch_lease(&target.scope, existing)?,
                already_committed: true,
            });
        }
        if current_cursor != command.expected_effect_cursor
            || aggregate
                .effects
                .get(&command.effect_id)
                .is_none_or(|entry| entry.dispatch_state != EffectDispatchStateV1::Intended)
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let mut payload = EffectDispatchClaimedPayloadV1 {
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            request_digest: intent.request_digest.clone(),
            external_key_digest: external_key_digest.clone(),
            executor_revision: intent.executor_revision.clone(),
            executor_descriptor_digest: intent.executor_descriptor_digest.clone(),
            executor_config_digest: intent.executor_config_digest.clone(),
            recovery_key: pareto_protocol::EffectRecoveryKeyV1 {
                base: intent.recovery_base_key.clone(),
                claim_event_id: command.event_id.clone(),
                claim_event_digest: zero_digest()?,
                claim_process_epoch_digest,
                claim_clock_digest,
                external_key_digest,
                claim_policy_revision: command.claim_policy_revision.clone(),
            },
        };
        let sequence = next_sequence(&current_cursor)?;
        let normalized = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &effect_stream_id(&target.scope)?,
            &command.event_id,
            sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-dispatch-claimed",
            &payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        payload.recovery_key.claim_event_digest = digest_bytes(
            "effect-claim-event-v1",
            canonical(normalized.envelope())
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
                .as_bytes(),
        )?;
        let event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &effect_stream_id(&target.scope)?,
            &command.event_id,
            sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-dispatch-claimed",
            &payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits)
            .map_err(map_store_error)?;
        insert_prepared(&mut transaction, &prepared)
            .await
            .map_err(map_store_error)?;
        let final_cursor = EventCursor {
            sequence: sequence.to_string(),
            event_id: command.event_id.clone(),
        };
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &final_cursor,
        )
        .await?;
        fold_effect_events(&lifecycle.schema_set, &events)?;
        transaction
            .commit()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(EffectClaimResult {
            cursor: final_cursor,
            lease: make_dispatch_lease(&target.scope, &payload)?,
            already_committed: false,
        })
    }

    async fn dispatch_effect(
        &self,
        registry: &SchemaRegistry,
        descriptor: &EffectExecutorDescriptorV1,
        target: &EffectTarget,
        command: &ClaimEffectCommandV1,
        executor: &dyn FakeEffectExecutor,
    ) -> Result<EffectDispatchResult, EffectError> {
        let claimed = self
            .claim_effect_dispatch(registry, descriptor, target, command)
            .await?;
        let execution = if claimed.already_committed {
            None
        } else {
            Some(execute_fake_effect(descriptor, &claimed.lease, executor)?)
        };
        Ok(EffectDispatchResult {
            cursor: claimed.cursor,
            execution,
            already_claimed: claimed.already_committed,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn admit_effect_receipt(
        &self,
        registry: &SchemaRegistry,
        effect_registry: &EffectRegistryRevisionV1,
        descriptor: &EffectExecutorDescriptorV1,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        operation_lease: &OperationLease,
        dispatch_lease: &EffectDispatchLease,
        observation: &EffectReceiptObservationV1,
        command: &AdmitEffectReceiptCommandV1,
    ) -> Result<EffectTerminalConclusionResult, EffectError> {
        verify_dispatch_lease(descriptor, dispatch_lease)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let cursor = current_effect_cursor(&mut transaction, target).await?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        let intent = aggregate
            .intents
            .get(&dispatch_lease.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let registration = effect_registry
            .registrations
            .binary_search_by(|candidate| candidate.effect_kind.cmp(&intent.effect_kind))
            .ok()
            .and_then(|index| effect_registry.registrations.get(index))
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let observation_bytes = canonical(observation)
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        if effect_registry.validate().is_err()
            || effect_registry.metadata.revision_id != intent.effect_registry_revision
            || effect_registry.config_digest != intent.effect_registry_config_digest
            || registration.executor_revision != dispatch_lease.executor_revision
            || registration.executor_descriptor_digest != dispatch_lease.executor_descriptor_digest
            || registration.executor_config_digest != dispatch_lease.executor_config_digest
            || observation.effect_id != dispatch_lease.effect_id
            || observation.attempt_id != dispatch_lease.attempt_id
            || observation.external_key_digest != dispatch_lease.external_key_digest
            || aggregate
                .effects
                .get(&observation.effect_id)
                .is_none_or(|entry| entry.dispatch_state != EffectDispatchStateV1::Claimed)
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let receipt_valid = observation.producer_revision == registration.producer_revision
            && observation.adapter_revision == registration.adapter_revision
            && observation_bytes.len() as u64 <= registration.limits.max_receipt_bytes
            && observation.limitations.len() as u32 <= registration.limits.max_limitations
            && observation
                .limitations
                .windows(2)
                .all(|items| items[0] < items[1])
            && lifecycle
                .schema_set
                .parse_record::<EffectReceiptObservationV1>(observation_bytes.as_bytes())
                .is_ok();
        if !receipt_valid {
            let payload = EffectMessageRejectedPayloadV1 {
                effect_id: Some(observation.effect_id.clone()),
                attempt_id: Some(observation.attempt_id.clone()),
                effect_kind: Some(intent.effect_kind.clone()),
                reason_code: "receipt-admission-rejected".to_owned(),
                input_digest: digest_bytes(
                    "pareto.effect-receipt-rejected-input.v1",
                    observation_bytes.as_bytes(),
                )?,
                effect_registry_revision: intent.effect_registry_revision.clone(),
                redaction_policy_revision: registration.redaction_policy_revision.clone(),
            };
            let event = lifecycle_event(
                &lifecycle.schema_set,
                &lifecycle.limits,
                &target.scope,
                &target.actor,
                &effect_stream_id(&target.scope)?,
                &command.effect_event_id,
                next_sequence(&cursor)?,
                &command.occurred_at,
                &command.correlation_id,
                "effect-message-rejected",
                &payload,
            )
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
            let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
            if check_prepared_idempotency(&mut transaction, &prepared)
                .await
                .map_err(map_store_error)?
                .is_none()
            {
                insert_prepared(&mut transaction, &prepared)
                    .await
                    .map_err(map_store_error)?;
            }
            transaction
                .commit()
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let (runtime_outcome, reason_code) = match observation.outcome_class {
            EffectReceiptOutcomeClassV1::Applied => {
                (OperationOutcomeV1::Succeeded, "effect-applied")
            }
            EffectReceiptOutcomeClassV1::RejectedBeforeApply => {
                (OperationOutcomeV1::Failed, "effect-rejected-before-apply")
            }
            EffectReceiptOutcomeClassV1::Partial => (OperationOutcomeV1::Failed, "effect-partial"),
            EffectReceiptOutcomeClassV1::Unknown => (OperationOutcomeV1::Failed, "effect-unknown"),
        };
        let planned = match observation.outcome_class {
            EffectReceiptOutcomeClassV1::Applied => {
                plan_hook_settlement(
                    &mut transaction,
                    registry,
                    control_target,
                    operation_lease,
                    command.control_event_id.clone(),
                    command.callback_id.clone(),
                    command.correlation_id.clone(),
                    runtime_outcome,
                    reason_code.to_owned(),
                    observation.receipt_digest.clone(),
                    &command.clock,
                )
                .await
            }
            EffectReceiptOutcomeClassV1::RejectedBeforeApply => {
                plan_effect_conservative_settlement(
                    &mut transaction,
                    registry,
                    control_target,
                    operation_lease,
                    command.control_event_id.clone(),
                    command.callback_id.clone(),
                    command.correlation_id.clone(),
                    runtime_outcome,
                    reason_code.to_owned(),
                    observation.receipt_digest.clone(),
                    EffectSettlementAccountingV1::VerifiedZero,
                    &command.clock,
                )
                .await
            }
            EffectReceiptOutcomeClassV1::Partial | EffectReceiptOutcomeClassV1::Unknown => {
                plan_effect_conservative_settlement(
                    &mut transaction,
                    registry,
                    control_target,
                    operation_lease,
                    command.control_event_id.clone(),
                    command.callback_id.clone(),
                    command.correlation_id.clone(),
                    runtime_outcome,
                    reason_code.to_owned(),
                    observation.receipt_digest.clone(),
                    EffectSettlementAccountingV1::UnknownConservative,
                    &command.clock,
                )
                .await
            }
        }
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let pair = EffectPairBindingV1 {
            pair_id: command.pair_id.clone(),
            pair_kind: EffectPairKindV1::TerminalConclusion,
            pair_fingerprint: zero_digest()?,
            control_event_id: command.control_event_id.clone(),
            effect_event_id: command.effect_event_id.clone(),
            operation_id: intent.pair.operation_id.clone(),
            reservation_id: intent.pair.reservation_id.clone(),
            effect_id: observation.effect_id.clone(),
            attempt_id: observation.attempt_id.clone(),
            control_prepared_digest: zero_digest()?,
            effect_prepared_digest: zero_digest()?,
        };
        let base = EffectTerminalPairCommandV1 {
            scope: target.scope.clone(),
            owner: target.actor.clone(),
            control_stream_id: runtime_control_stream_id(&target.scope)
                .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?,
            effect_stream_id: effect_stream_id(&target.scope)?,
            expected_control_cursor: planned.expected_cursor,
            expected_effect_cursor: cursor,
            control_sequence: 0,
            effect_sequence: 0,
            pair: pair.clone(),
            occurred_at: command.occurred_at.clone(),
            correlation_id: command.correlation_id.clone(),
            control_payload: planned.payload,
            effect_payload: (),
        };
        match observation.outcome_class {
            EffectReceiptOutcomeClassV1::Applied => {
                self.append_effect_terminal_pair(
                    registry,
                    target,
                    control_target,
                    EffectTerminalPairCommandV1 {
                        effect_payload: EffectReceiptAdmittedPayloadV1 {
                            effect_id: observation.effect_id.clone(),
                            attempt_id: observation.attempt_id.clone(),
                            producer_revision: observation.producer_revision.clone(),
                            adapter_revision: observation.adapter_revision.clone(),
                            external_conclusion: EffectExternalConclusionV1::Applied,
                            receipt_digest: observation.receipt_digest.clone(),
                            result_digest: observation.result_digest.clone(),
                            accounted_usage: base.control_payload.accounted_usage.clone(),
                            limitations: observation.limitations.clone(),
                            pair,
                        },
                        scope: base.scope,
                        owner: base.owner,
                        control_stream_id: base.control_stream_id,
                        effect_stream_id: base.effect_stream_id,
                        expected_control_cursor: base.expected_control_cursor,
                        expected_effect_cursor: base.expected_effect_cursor,
                        control_sequence: base.control_sequence,
                        effect_sequence: base.effect_sequence,
                        pair: base.pair,
                        occurred_at: base.occurred_at,
                        correlation_id: base.correlation_id,
                        control_payload: base.control_payload,
                    },
                    AtomicPairFault::None,
                )
                .await
            }
            EffectReceiptOutcomeClassV1::RejectedBeforeApply => {
                self.append_effect_terminal_pair(
                    registry,
                    target,
                    control_target,
                    EffectTerminalPairCommandV1 {
                        effect_payload: EffectAttemptConcludedPayloadV1 {
                            effect_id: observation.effect_id.clone(),
                            attempt_id: observation.attempt_id.clone(),
                            external_conclusion: EffectExternalConclusionV1::NotApplied,
                            reason_code: reason_code.to_owned(),
                            accounted_usage: base.control_payload.accounted_usage.clone(),
                            pair,
                        },
                        scope: base.scope,
                        owner: base.owner,
                        control_stream_id: base.control_stream_id,
                        effect_stream_id: base.effect_stream_id,
                        expected_control_cursor: base.expected_control_cursor,
                        expected_effect_cursor: base.expected_effect_cursor,
                        control_sequence: base.control_sequence,
                        effect_sequence: base.effect_sequence,
                        pair: base.pair,
                        occurred_at: base.occurred_at,
                        correlation_id: base.correlation_id,
                        control_payload: base.control_payload,
                    },
                    AtomicPairFault::None,
                )
                .await
            }
            EffectReceiptOutcomeClassV1::Partial | EffectReceiptOutcomeClassV1::Unknown => {
                let conclusion =
                    if observation.outcome_class == EffectReceiptOutcomeClassV1::Partial {
                        EffectExternalConclusionV1::Partial
                    } else {
                        EffectExternalConclusionV1::Unknown
                    };
                self.append_effect_terminal_pair(
                    registry,
                    target,
                    control_target,
                    EffectTerminalPairCommandV1 {
                        effect_payload: EffectReconciliationRequiredPayloadV1 {
                            effect_id: observation.effect_id.clone(),
                            attempt_id: observation.attempt_id.clone(),
                            external_conclusion: conclusion,
                            reason_code: reason_code.to_owned(),
                            accounted_usage: base.control_payload.accounted_usage.clone(),
                            receipt_digest: Some(observation.receipt_digest.clone()),
                            limitations: observation.limitations.clone(),
                            confirmed_components_digest: (conclusion
                                == EffectExternalConclusionV1::Partial)
                                .then(|| observation.result_digest.clone()),
                            unknown_components_digest: observation.receipt_digest.clone(),
                            pair,
                        },
                        scope: base.scope,
                        owner: base.owner,
                        control_stream_id: base.control_stream_id,
                        effect_stream_id: base.effect_stream_id,
                        expected_control_cursor: base.expected_control_cursor,
                        expected_effect_cursor: base.expected_effect_cursor,
                        control_sequence: base.control_sequence,
                        effect_sequence: base.effect_sequence,
                        pair: base.pair,
                        occurred_at: base.occurred_at,
                        correlation_id: base.correlation_id,
                        control_payload: base.control_payload,
                    },
                    AtomicPairFault::None,
                )
                .await
            }
        }
    }

    async fn observe_late_effect_receipt(
        &self,
        registry: &SchemaRegistry,
        effect_registry: &EffectRegistryRevisionV1,
        target: &EffectTarget,
        observation: &EffectReceiptObservationV1,
        command: &ObserveLateReceiptCommandV1,
    ) -> Result<AppendResult, EffectError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let cursor = current_effect_cursor(&mut transaction, target).await?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        let intent = aggregate
            .intents
            .get(&observation.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let claim = aggregate
            .claims
            .get(&observation.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let registration = effect_registry
            .registrations
            .binary_search_by(|candidate| candidate.effect_kind.cmp(&intent.effect_kind))
            .ok()
            .and_then(|index| effect_registry.registrations.get(index))
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        if observation.attempt_id != intent.attempt_id
            || observation.external_key_digest != claim.external_key_digest
            || observation.producer_revision != registration.producer_revision
            || observation.adapter_revision != registration.adapter_revision
            || aggregate
                .effects
                .get(&observation.effect_id)
                .is_none_or(|entry| entry.dispatch_state != EffectDispatchStateV1::Concluded)
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let existing_sequence: Option<i64> =
            sqlx::query_scalar("SELECT sequence_i64 FROM events WHERE event_id=?")
                .bind(command.event_id.as_str())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let sequence = existing_sequence.unwrap_or(next_sequence(&cursor)?);
        let payload = EffectLateReceiptObservedPayloadV1 {
            effect_id: observation.effect_id.clone(),
            attempt_id: observation.attempt_id.clone(),
            receipt_digest: observation.receipt_digest.clone(),
            producer_revision: Some(observation.producer_revision.clone()),
            reason_code: "late-after-terminal".to_owned(),
            redaction_policy_revision: registration.redaction_policy_revision.clone(),
        };
        let event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &effect_stream_id(&target.scope)?,
            &command.event_id,
            sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-late-receipt-observed",
            &payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits)
            .map_err(map_store_error)?;
        let result = if let Some(existing) = check_prepared_idempotency(&mut transaction, &prepared)
            .await
            .map_err(map_store_error)?
        {
            existing
        } else {
            insert_prepared(&mut transaction, &prepared)
                .await
                .map_err(map_store_error)?
        };
        let final_cursor = EventCursor {
            sequence: sequence.to_string(),
            event_id: command.event_id.clone(),
        };
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &final_cursor,
        )
        .await?;
        fold_effect_events(&lifecycle.schema_set, &events)?;
        transaction
            .commit()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(result)
    }

    async fn recover_effect(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        command: &RecoverEffectCommandV1,
    ) -> Result<EffectRecoveryResult, EffectError> {
        if command.command_fingerprint != recovery_command_fingerprint(command)?
            || target.scope != control_target.scope
            || target.actor != control_target.principal
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let cursor = current_effect_cursor(&mut transaction, target).await?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        let intent = aggregate
            .intents
            .get(&command.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let entry = aggregate
            .effects
            .get(&command.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        if intent.attempt_id != command.attempt_id {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        if entry.dispatch_state == EffectDispatchStateV1::Concluded {
            let control_json: Option<String> =
                sqlx::query_scalar("SELECT envelope_json FROM events WHERE event_id=?")
                    .bind(command.control_event_id.as_str())
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            let effect_json: Option<String> =
                sqlx::query_scalar("SELECT envelope_json FROM events WHERE event_id=?")
                    .bind(command.effect_event_id.as_str())
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            let exact = control_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                .is_some_and(|event| {
                    event["event_type"] == "operation-settled"
                        && event["payload"]["timeout_command_fingerprint"]
                            == command.command_fingerprint.as_str()
                        && event["payload"]["effect_pair"]["effect_id"]
                            == command.effect_id.as_str()
                        && event["payload"]["effect_pair"]["attempt_id"]
                            == command.attempt_id.as_str()
                })
                && effect_json
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                    .is_some_and(|event| {
                        matches!(
                            event["event_type"].as_str(),
                            Some("effect-attempt-concluded")
                                | Some("effect-reconciliation-required")
                        ) && event["payload"]["effect_id"] == command.effect_id.as_str()
                            && event["payload"]["attempt_id"] == command.attempt_id.as_str()
                    });
            transaction
                .rollback()
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
            if exact {
                return Ok(EffectRecoveryResult {
                    already_committed: true,
                });
            }
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        if cursor != command.expected_effect_cursor {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let claimed = entry.dispatch_state == EffectDispatchStateV1::Claimed;
        let expected_lost_epoch = if claimed {
            aggregate
                .claims
                .get(&command.effect_id)
                .map(|claim| &claim.recovery_key.claim_process_epoch_digest)
                .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?
        } else {
            &intent.recovery_base_key.initial_process_epoch_digest
        };
        let eligible = match command.cause {
            EffectRecoveryCauseV1::ProcessEpochLost => {
                &command.lost_process_epoch_digest == expected_lost_epoch
                    && command.current_process_epoch_digest != command.lost_process_epoch_digest
            }
            EffectRecoveryCauseV1::DeadlineDue => {
                command.clock.canonical_utc >= intent.recovery_base_key.deadline_at
            }
            EffectRecoveryCauseV1::CancellationEffective => effect_cancellation_is_effective(
                &mut transaction,
                registry,
                control_target,
                &intent.pair.operation_id,
            )
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?,
        };
        if !eligible {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let runtime_outcome = match command.cause {
            EffectRecoveryCauseV1::DeadlineDue => OperationOutcomeV1::TimedOut,
            EffectRecoveryCauseV1::ProcessEpochLost => OperationOutcomeV1::Failed,
            EffectRecoveryCauseV1::CancellationEffective => OperationOutcomeV1::Cancelled,
        };
        let accounting = if claimed {
            EffectSettlementAccountingV1::UnknownConservative
        } else {
            EffectSettlementAccountingV1::VerifiedZero
        };
        let planned = plan_effect_recovery_settlement(
            &mut transaction,
            registry,
            control_target,
            &intent.pair.operation_id,
            runtime_outcome,
            if claimed {
                "effect-recovery-after-claim"
            } else {
                "effect-recovery-before-claim"
            }
            .to_owned(),
            command.command_fingerprint.clone(),
            accounting,
            &command.clock,
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let pair = EffectPairBindingV1 {
            pair_id: command.pair_id.clone(),
            pair_kind: EffectPairKindV1::TerminalConclusion,
            pair_fingerprint: zero_digest()?,
            control_event_id: command.control_event_id.clone(),
            effect_event_id: command.effect_event_id.clone(),
            operation_id: intent.pair.operation_id.clone(),
            reservation_id: intent.pair.reservation_id.clone(),
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            control_prepared_digest: zero_digest()?,
            effect_prepared_digest: zero_digest()?,
        };
        let base = EffectTerminalPairCommandV1 {
            scope: target.scope.clone(),
            owner: target.actor.clone(),
            control_stream_id: runtime_control_stream_id(&target.scope)
                .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?,
            effect_stream_id: effect_stream_id(&target.scope)?,
            expected_control_cursor: planned.expected_cursor,
            expected_effect_cursor: cursor,
            control_sequence: 0,
            effect_sequence: 0,
            pair: pair.clone(),
            occurred_at: command.occurred_at.clone(),
            correlation_id: command.correlation_id.clone(),
            control_payload: planned.payload,
            effect_payload: (),
        };
        let result = if claimed {
            self.append_effect_terminal_pair(
                registry,
                target,
                control_target,
                EffectTerminalPairCommandV1 {
                    effect_payload: EffectReconciliationRequiredPayloadV1 {
                        effect_id: command.effect_id.clone(),
                        attempt_id: command.attempt_id.clone(),
                        external_conclusion: EffectExternalConclusionV1::Unknown,
                        reason_code: "effect-recovery-after-claim".to_owned(),
                        accounted_usage: base.control_payload.accounted_usage.clone(),
                        receipt_digest: None,
                        limitations: Vec::new(),
                        confirmed_components_digest: None,
                        unknown_components_digest: command.command_fingerprint.clone(),
                        pair,
                    },
                    scope: base.scope,
                    owner: base.owner,
                    control_stream_id: base.control_stream_id,
                    effect_stream_id: base.effect_stream_id,
                    expected_control_cursor: base.expected_control_cursor,
                    expected_effect_cursor: base.expected_effect_cursor,
                    control_sequence: base.control_sequence,
                    effect_sequence: base.effect_sequence,
                    pair: base.pair,
                    occurred_at: base.occurred_at,
                    correlation_id: base.correlation_id,
                    control_payload: base.control_payload,
                },
                AtomicPairFault::None,
            )
            .await?
        } else {
            self.append_effect_terminal_pair(
                registry,
                target,
                control_target,
                EffectTerminalPairCommandV1 {
                    effect_payload: EffectAttemptConcludedPayloadV1 {
                        effect_id: command.effect_id.clone(),
                        attempt_id: command.attempt_id.clone(),
                        external_conclusion: EffectExternalConclusionV1::NotApplied,
                        reason_code: "effect-recovery-before-claim".to_owned(),
                        accounted_usage: base.control_payload.accounted_usage.clone(),
                        pair,
                    },
                    scope: base.scope,
                    owner: base.owner,
                    control_stream_id: base.control_stream_id,
                    effect_stream_id: base.effect_stream_id,
                    expected_control_cursor: base.expected_control_cursor,
                    expected_effect_cursor: base.expected_effect_cursor,
                    control_sequence: base.control_sequence,
                    effect_sequence: base.effect_sequence,
                    pair: base.pair,
                    occurred_at: base.occurred_at,
                    correlation_id: base.correlation_id,
                    control_payload: base.control_payload,
                },
                AtomicPairFault::None,
            )
            .await?
        };
        Ok(EffectRecoveryResult {
            already_committed: result.already_committed,
        })
    }

    async fn reconcile_effect(
        &self,
        registry: &SchemaRegistry,
        effect_registry: &EffectRegistryRevisionV1,
        target: &EffectTarget,
        command: &ReconcileEffectCommandV1,
        fault: AtomicPairFault,
    ) -> Result<bool, EffectError> {
        if command.command_fingerprint != reconciliation_command_fingerprint(command)?
            || command.source_observation_event_ids.is_empty()
            || command
                .source_observation_event_ids
                .windows(2)
                .any(|ids| ids[0] >= ids[1])
            || !matches!(
                command.resolution,
                EffectReconciliationStateV1::ResolvedApplied
                    | EffectReconciliationStateV1::ResolvedNotApplied
                    | EffectReconciliationStateV1::ResolvedPartial
            )
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let current = current_effect_cursor(&mut transaction, target).await?;
        let current_events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &current,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &current_events)?;
        let intent = aggregate
            .intents
            .get(&command.effect_id)
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        let registration = effect_registry
            .registrations
            .binary_search_by(|candidate| candidate.effect_kind.cmp(&intent.effect_kind))
            .ok()
            .and_then(|index| effect_registry.registrations.get(index))
            .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
        if command.attempt_id != intent.attempt_id
            || command.producer_revision != registration.reconciliation_policy_revision
        {
            return Err(EffectError::new(EffectErrorKind::Unauthorized));
        }
        let first_sequence = next_sequence(&command.expected_effect_cursor)?;
        let second_sequence = first_sequence
            .checked_add(1)
            .ok_or_else(|| EffectError::new(EffectErrorKind::IdempotencyConflict))?;
        let observed_payload = EffectReconciliationObservedPayloadV1 {
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            producer_revision: command.producer_revision.clone(),
            source_observation_event_ids: command.source_observation_event_ids.clone(),
            evidence_fingerprint: command.command_fingerprint.clone(),
        };
        let reconciled_payload = EffectReconciledPayloadV1 {
            effect_id: command.effect_id.clone(),
            attempt_id: command.attempt_id.clone(),
            reconciliation_state: command.resolution,
            source_observation_event_id: command.observation_event_id.clone(),
            evidence_fingerprint: command.command_fingerprint.clone(),
        };
        let stream_id = effect_stream_id(&target.scope)?;
        let observed_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &stream_id,
            &command.observation_event_id,
            first_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-reconciliation-observed",
            &observed_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let reconciled_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &stream_id,
            &command.reconciled_event_id,
            second_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-reconciled",
            &reconciled_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let observed_prepared =
            PreparedEvent::new(&observed_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let reconciled_prepared =
            PreparedEvent::new(&reconciled_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let presence =
            pair_presence(&mut transaction, &observed_prepared, &reconciled_prepared).await?;
        if presence == PairPresence::Zero
            && (current != command.expected_effect_cursor
                || aggregate
                    .effects
                    .get(&command.effect_id)
                    .is_none_or(|entry| {
                        entry.reconciliation_state != EffectReconciliationStateV1::Required
                    }))
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        let pair = append_atomic_pair(
            &mut transaction,
            &observed_prepared,
            &reconciled_prepared,
            fault,
        )
        .await
        .map_err(map_store_error)?;
        let final_cursor = EventCursor {
            sequence: second_sequence.to_string(),
            event_id: command.reconciled_event_id.clone(),
        };
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &final_cursor,
        )
        .await?;
        fold_effect_events(&lifecycle.schema_set, &events)?;
        transaction
            .commit()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(pair.already_committed)
    }

    async fn append_effect_reserve_intent_pair(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        mut command: EffectReserveIntentCommandV1,
        fault: AtomicPairFault,
    ) -> Result<EffectReserveIntentResult, EffectError> {
        if target.scope != control_target.scope
            || target.actor != control_target.principal
            || command.scope != target.scope
            || command.owner != target.actor
            || command.control_stream_id
                != runtime_control_stream_id(&command.scope)
                    .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?
            || command.effect_stream_id != effect_stream_id(&command.scope)?
            || command.pair.pair_kind != EffectPairKindV1::ReserveIntent
            || command.pair.control_event_id == command.pair.effect_event_id
            || command.pair.operation_id != command.control_payload.operation_id
            || command.pair.reservation_id != command.control_payload.reservation_id
            || command.pair.effect_id != command.effect_payload.effect_id
            || command.pair.attempt_id != command.effect_payload.attempt_id
            || command.effect_payload.subject_actor != command.owner
            || command.effect_payload.task_id != command.control_payload.task_id
            || command.effect_payload.reserved_usage != command.control_payload.trusted_reservation
            || command.effect_payload.recovery_base_key.scope != command.scope
            || command.effect_payload.recovery_base_key.effect_id != command.pair.effect_id
            || command.effect_payload.recovery_base_key.attempt_id != command.pair.attempt_id
            || command.effect_payload.recovery_base_key.operation_id != command.pair.operation_id
            || command.effect_payload.recovery_base_key.reservation_id
                != command.pair.reservation_id
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        command.control_sequence = next_sequence(&command.expected_control_cursor)?;
        command.effect_sequence = next_sequence(&command.expected_effect_cursor)?;
        command.pair.pair_fingerprint = zero_digest()?;
        command.pair.control_prepared_digest = zero_digest()?;
        command.pair.effect_prepared_digest = zero_digest()?;
        command.control_payload.hook_pair = None;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.pair = command.pair.clone();

        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        if lifecycle.state.manifest.schema_ref.major != 3
            || command.effect_payload.effect_registry_revision
                != lifecycle.state.manifest.revisions["effect_registry"]
            || Some(&command.effect_payload.effect_registry_config_digest)
                != lifecycle
                    .state
                    .manifest
                    .effect_registry_config_digest
                    .as_ref()
            || command
                .effect_payload
                .recovery_base_key
                .source_schema_set_ref
                != *lifecycle.schema_set.reference()
        {
            return Err(EffectError::new(EffectErrorKind::ManifestInvalid));
        }

        let normalized_control = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            command.control_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "operation-reserved",
            &command.control_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let normalized_effect = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.effect_stream_id,
            &command.pair.effect_event_id,
            command.effect_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-intended",
            &command.effect_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        command.pair.control_prepared_digest = digest_bytes(
            "effect-control-prepared-v1",
            canonical(normalized_control.envelope())
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
                .as_bytes(),
        )?;
        command.pair.effect_prepared_digest = digest_bytes(
            "effect-event-prepared-v1",
            canonical(normalized_effect.envelope())
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
                .as_bytes(),
        )?;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.pair = command.pair.clone();
        command.pair.pair_fingerprint = pair_command_fingerprint(&command)?;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.pair = command.pair.clone();

        let control_event = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            command.control_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "operation-reserved",
            &command.control_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let effect_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.effect_stream_id,
            &command.pair.effect_event_id,
            command.effect_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "effect-intended",
            &command.effect_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let control_prepared =
            PreparedEvent::new(&control_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let effect_prepared =
            PreparedEvent::new(&effect_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let presence = pair_presence(&mut transaction, &control_prepared, &effect_prepared).await?;
        let lease = if presence == PairPresence::Zero {
            let events = read_effect_events_at(
                &mut transaction,
                target,
                lifecycle.schema_set.clone(),
                lifecycle.limits.clone(),
                &command.expected_effect_cursor,
            )
            .await?;
            let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
            if aggregate.inclusive_cursor != command.expected_effect_cursor {
                return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
            }
            let (admitted_control, lease) = prepare_effect_reservation_event(
                &mut transaction,
                registry,
                control_target,
                &HookControlEventContext {
                    expected_cursor: &command.expected_control_cursor,
                    event_id: &command.pair.control_event_id,
                    occurred_at: &command.occurred_at,
                    correlation_id: &command.correlation_id,
                },
                &command.control_payload,
                &command.clock,
            )
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
            if admitted_control.envelope_json != control_prepared.envelope_json {
                return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
            }
            lease
        } else {
            validate_runtime_control_history(&mut transaction, registry, control_target)
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
            let final_cursor = EventCursor {
                sequence: command.effect_sequence.to_string(),
                event_id: command.pair.effect_event_id.clone(),
            };
            let events = read_effect_events_at(
                &mut transaction,
                target,
                lifecycle.schema_set.clone(),
                lifecycle.limits.clone(),
                &final_cursor,
            )
            .await?;
            fold_effect_events(&lifecycle.schema_set, &events)?;
            super::runtime_control::make_lease(
                control_target,
                &command.control_payload,
                &command.clock,
            )
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?
        };
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &effect_prepared, fault)
            .await
            .map_err(map_store_error)?;
        let final_cursor = EventCursor {
            sequence: command.effect_sequence.to_string(),
            event_id: command.pair.effect_event_id.clone(),
        };
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &final_cursor,
        )
        .await?;
        fold_effect_events(&lifecycle.schema_set, &events)?;
        transaction
            .commit()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(EffectReserveIntentResult {
            control: pair.first,
            effect: pair.second,
            lease,
            already_committed: pair.already_committed,
        })
    }

    async fn append_effect_terminal_pair<T: EffectTerminalPayload>(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        mut command: EffectTerminalPairCommandV1<T>,
        fault: AtomicPairFault,
    ) -> Result<EffectTerminalConclusionResult, EffectError> {
        if target.scope != control_target.scope
            || target.actor != control_target.principal
            || command.scope != target.scope
            || command.owner != target.actor
            || command.control_stream_id
                != runtime_control_stream_id(&command.scope)
                    .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?
            || command.effect_stream_id != effect_stream_id(&command.scope)?
            || command.pair.pair_kind != EffectPairKindV1::TerminalConclusion
            || command.pair.control_event_id == command.pair.effect_event_id
            || command.pair.operation_id != command.control_payload.operation_id
            || command.pair.reservation_id != command.control_payload.reservation_id
            || command.pair.effect_id != *command.effect_payload.effect_id()
            || command.pair.attempt_id != *command.effect_payload.attempt_id()
            || command.effect_payload.accounted_usage() != command.control_payload.accounted_usage
            || !matches!(
                command.effect_payload.conclusion(),
                EffectExternalConclusionV1::Applied
                    | EffectExternalConclusionV1::NotApplied
                    | EffectExternalConclusionV1::Partial
                    | EffectExternalConclusionV1::Unknown
            )
        {
            return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
        }
        command.control_sequence = next_sequence(&command.expected_control_cursor)?;
        command.effect_sequence = next_sequence(&command.expected_effect_cursor)?;
        command.pair.pair_fingerprint = zero_digest()?;
        command.pair.control_prepared_digest = zero_digest()?;
        command.pair.effect_prepared_digest = zero_digest()?;
        command.control_payload.hook_pair = None;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.set_pair(command.pair.clone());

        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let normalized_control = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            command.control_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "operation-settled",
            &command.control_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let normalized_effect = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.effect_stream_id,
            &command.pair.effect_event_id,
            command.effect_sequence,
            &command.occurred_at,
            &command.correlation_id,
            T::EVENT_TYPE,
            &command.effect_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        command.pair.control_prepared_digest = digest_bytes(
            "effect-control-prepared-v1",
            canonical(normalized_control.envelope())
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
                .as_bytes(),
        )?;
        command.pair.effect_prepared_digest = digest_bytes(
            "effect-event-prepared-v1",
            canonical(normalized_effect.envelope())
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
                .as_bytes(),
        )?;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.set_pair(command.pair.clone());
        command.pair.pair_fingerprint = terminal_pair_command_fingerprint(&command)?;
        command.control_payload.effect_pair = Some(command.pair.clone());
        command.effect_payload.set_pair(command.pair.clone());
        let control_event = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            command.control_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "operation-settled",
            &command.control_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let effect_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.effect_stream_id,
            &command.pair.effect_event_id,
            command.effect_sequence,
            &command.occurred_at,
            &command.correlation_id,
            T::EVENT_TYPE,
            &command.effect_payload,
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        let control_prepared =
            PreparedEvent::new(&control_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let effect_prepared =
            PreparedEvent::new(&effect_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let presence = pair_presence(&mut transaction, &control_prepared, &effect_prepared).await?;
        if presence == PairPresence::Zero {
            let events = read_effect_events_at(
                &mut transaction,
                target,
                lifecycle.schema_set.clone(),
                lifecycle.limits.clone(),
                &command.expected_effect_cursor,
            )
            .await?;
            let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
            let entry = aggregate
                .effects
                .get(&command.pair.effect_id)
                .ok_or_else(|| EffectError::new(EffectErrorKind::IdempotencyConflict))?;
            let state_allowed = match command.effect_payload.conclusion() {
                EffectExternalConclusionV1::NotApplied => matches!(
                    entry.dispatch_state,
                    EffectDispatchStateV1::Intended | EffectDispatchStateV1::Claimed
                ),
                _ => entry.dispatch_state == EffectDispatchStateV1::Claimed,
            };
            if entry.attempt_id != command.pair.attempt_id || !state_allowed {
                return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
            }
            let admitted = prepare_effect_settlement_event(
                &mut transaction,
                registry,
                control_target,
                &HookControlEventContext {
                    expected_cursor: &command.expected_control_cursor,
                    event_id: &command.pair.control_event_id,
                    occurred_at: &command.occurred_at,
                    correlation_id: &command.correlation_id,
                },
                &command.control_payload,
            )
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
            if admitted.envelope_json != control_prepared.envelope_json {
                return Err(EffectError::new(EffectErrorKind::IdempotencyConflict));
            }
        } else {
            validate_runtime_control_history(&mut transaction, registry, control_target)
                .await
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        }
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &effect_prepared, fault)
            .await
            .map_err(map_store_error)?;
        let final_cursor = EventCursor {
            sequence: command.effect_sequence.to_string(),
            event_id: command.pair.effect_event_id.clone(),
        };
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            &final_cursor,
        )
        .await?;
        fold_effect_events(&lifecycle.schema_set, &events)?;
        transaction
            .commit()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        Ok(EffectTerminalConclusionResult {
            control: pair.first,
            effect: pair.second,
            already_committed: pair.already_committed,
        })
    }

    async fn append_effect_terminal_conclusion_pair(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        control_target: &RuntimeControlTarget,
        command: EffectTerminalConclusionCommandV1,
        fault: AtomicPairFault,
    ) -> Result<EffectTerminalConclusionResult, EffectError> {
        self.append_effect_terminal_pair(registry, target, control_target, command, fault)
            .await
    }

    async fn effect_projection_at(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        inclusive_cursor: &EventCursor,
    ) -> Result<EffectProjectionV1, EffectError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        if lifecycle.state.manifest.schema_ref.major != 3 {
            return Err(EffectError::new(EffectErrorKind::ManifestInvalid));
        }
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            inclusive_cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        if aggregate.initialization.effect_registry_revision
            != *lifecycle
                .state
                .manifest
                .revisions
                .get("effect_registry")
                .ok_or_else(|| EffectError::new(EffectErrorKind::ManifestInvalid))?
            || Some(&aggregate.initialization.effect_registry_config_digest)
                != lifecycle
                    .state
                    .manifest
                    .effect_registry_config_digest
                    .as_ref()
        {
            return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
        }
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        build_projection(
            &self.store_id,
            &target.scope,
            &target.actor,
            &lifecycle.schema_set,
            aggregate,
        )
    }

    async fn effect_boundary_inventory_v2(
        &self,
        registry: &SchemaRegistry,
        target: &EffectTarget,
        inclusive_cursor: &EventCursor,
        logical_id: &str,
        created_at: &str,
    ) -> Result<BoundaryInventoryRevisionV2, EffectError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let events = read_effect_events_at(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
            inclusive_cursor,
        )
        .await?;
        let aggregate = fold_effect_events(&lifecycle.schema_set, &events)?;
        let mut records = Vec::with_capacity(aggregate.effects.len());
        for (effect_id, entry) in &aggregate.effects {
            let intent = aggregate
                .intents
                .get(effect_id)
                .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
            let terminal = aggregate
                .terminals
                .get(effect_id)
                .ok_or_else(|| EffectError::new(EffectErrorKind::Unauthorized))?;
            let reconciliation_binding = || {
                aggregate.reconciled.get(effect_id).map_or_else(
                    || EffectReconciliationBindingV2::Open {
                        evidence_digest: match terminal {
                            EffectTerminalFact::Reconciliation(payload) => {
                                payload.unknown_components_digest.clone()
                            }
                            _ => unreachable!("only reconciliation facts request binding"),
                        },
                    },
                    |resolved| EffectReconciliationBindingV2::Resolved {
                        source_reconciliation_event_id: resolved
                            .source_observation_event_id
                            .clone(),
                        evidence_digest: resolved.evidence_fingerprint.clone(),
                    },
                )
            };
            let outcome = match terminal {
                EffectTerminalFact::Receipt(payload) => EffectBoundaryOutcomeV2::Applied {
                    receipt_digest: payload.receipt_digest.clone(),
                    result_digest: payload.result_digest.clone(),
                    limitations: payload.limitations.clone(),
                },
                EffectTerminalFact::Attempt(payload)
                    if payload.external_conclusion == EffectExternalConclusionV1::NotApplied =>
                {
                    EffectBoundaryOutcomeV2::NotApplied {
                        reason_code: payload.reason_code.clone(),
                        limitations: Vec::new(),
                    }
                }
                EffectTerminalFact::Reconciliation(payload)
                    if payload.external_conclusion == EffectExternalConclusionV1::Partial =>
                {
                    EffectBoundaryOutcomeV2::Partial {
                        receipt_digest: payload.receipt_digest.clone(),
                        confirmed_components_digest: payload
                            .confirmed_components_digest
                            .clone()
                            .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?,
                        unknown_components_digest: payload.unknown_components_digest.clone(),
                        limitations: payload.limitations.clone(),
                        reconciliation_binding: reconciliation_binding(),
                    }
                }
                EffectTerminalFact::Reconciliation(payload)
                    if payload.external_conclusion == EffectExternalConclusionV1::Unknown =>
                {
                    EffectBoundaryOutcomeV2::Unknown {
                        limitations: payload.limitations.clone(),
                        reconciliation_binding: reconciliation_binding(),
                    }
                }
                _ => return Err(EffectError::new(EffectErrorKind::AggregateCorrupt)),
            };
            records.push(EffectBoundaryRecordV2 {
                effect_id: effect_id.clone(),
                request_digest: entry.request_digest.clone(),
                attempt_id: entry.attempt_id.clone(),
                external_key_digest: aggregate
                    .claims
                    .get(effect_id)
                    .map(|claim| claim.external_key_digest.clone()),
                executor_revision: entry.executor_revision.clone(),
                executor_descriptor_digest: entry.executor_descriptor_digest.clone(),
                operation_id: intent.pair.operation_id.clone(),
                reservation_id: intent.pair.reservation_id.clone(),
                outcome,
            });
        }
        transaction
            .rollback()
            .await
            .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
        let schema_ref = lifecycle
            .schema_set
            .schema_ref("boundary-inventory-revision")
            .filter(|schema| schema.major == 2)
            .ok_or_else(|| EffectError::new(EffectErrorKind::SchemaUnavailable))?
            .clone();
        let hash_schema_ref = lifecycle
            .schema_set
            .schema_ref("boundary-inventory-hash-view")
            .filter(|schema| schema.major == 2)
            .ok_or_else(|| EffectError::new(EffectErrorKind::SchemaUnavailable))?
            .clone();
        let mut inventory = BoundaryInventoryRevisionV2 {
            metadata: RevisionMetadata {
                logical_id: logical_id.to_owned(),
                revision_id: RevisionId::parse("rev_placeholder")
                    .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?,
                revision_kind: "boundary_inventory_v2".to_owned(),
                parent_revision: None,
                schema_ref,
                content_digest: zero_digest()?,
                creator_actor: target.actor.clone(),
                source: "kernel-effect-finalization".to_owned(),
                created_at: created_at.to_owned(),
            },
            hash_schema_ref,
            source_run_id: target.scope.run_id.clone(),
            schema_set_ref: lifecycle.schema_set.reference().clone(),
            effect_stream_id: effect_stream_id(&target.scope)?,
            effect_inclusive_cursor: inclusive_cursor.clone(),
            effect_history_digest: aggregate.history_digest,
            recording_policy_revision: lifecycle
                .state
                .manifest
                .boundary_recording_policy_ref
                .revision_id
                .clone(),
            effects: records,
        };
        inventory.metadata.content_digest = inventory
            .content_digest()
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        inventory.metadata.revision_id = derive_revision_id(&inventory.metadata)
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        inventory
            .validate()
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
        Ok(inventory)
    }

    async fn recorded_effect_replay(
        &self,
        registry: &SchemaRegistry,
        schema_set: &SchemaSet,
        source_manifest: &pareto_protocol::RunManifest,
        target: &EffectTarget,
        execution_mode: &pareto_protocol::ExecutionMode,
        inventory: &BoundaryInventoryRevisionV2,
    ) -> Result<Vec<EffectBoundaryRecordV2>, EffectError> {
        let validated = schema_set
            .validate_boundary_inventory_v2(
                inventory.clone(),
                source_manifest.clone(),
                &target.scope,
            )
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        execution_mode
            .validate_inventory_v2(&validated)
            .map_err(|_| EffectError::new(EffectErrorKind::Unauthorized))?;
        let rebuilt = self
            .effect_boundary_inventory_v2(
                registry,
                target,
                &inventory.effect_inclusive_cursor,
                "recorded-replay-verification",
                &inventory.metadata.created_at,
            )
            .await?;
        if rebuilt.effect_stream_id != inventory.effect_stream_id
            || rebuilt.effect_history_digest != inventory.effect_history_digest
            || rebuilt.effects != inventory.effects
            || rebuilt.schema_set_ref != inventory.schema_set_ref
        {
            return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
        }
        Ok(inventory.effects.clone())
    }
}

fn effect_stream_id(scope: &IsolationScope) -> Result<StreamId, EffectError> {
    let suffix = scope
        .run_id
        .as_str()
        .strip_prefix("run_")
        .ok_or_else(|| EffectError::new(EffectErrorKind::ManifestInvalid))?;
    StreamId::parse(format!("stream_effect-{suffix}"))
        .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))
}

fn effect_registry_config_digest(
    registrations: &[pareto_protocol::EffectRegistrationV1],
) -> Result<Digest, EffectError> {
    let bytes = canonical(&registrations)
        .map_err(|_| EffectError::new(EffectErrorKind::ManifestInvalid))?;
    digest_bytes("effect-registry-config-v1", bytes.as_bytes())
}

async fn current_effect_cursor(
    connection: &mut sqlx::SqliteConnection,
    target: &EffectTarget,
) -> Result<EventCursor, EffectError> {
    let stream_id = effect_stream_id(&target.scope)?;
    let (present, user) = user_key(&target.scope);
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT sequence_i64,event_id FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? ORDER BY sequence_i64 DESC,event_id DESC LIMIT 1",
    )
    .bind(target.scope.tenant_id.as_str())
    .bind(present)
    .bind(user)
    .bind(target.scope.workspace_id.as_str())
    .bind(target.scope.run_id.as_str())
    .bind(target.scope.agent_id.as_str())
    .bind(stream_id.as_str())
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
    let (sequence, event_id) =
        row.ok_or_else(|| EffectError::new(EffectErrorKind::AggregateNotFound))?;
    Ok(EventCursor {
        sequence: sequence.to_string(),
        event_id: EventId::parse(event_id)
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?,
    })
}

pub(super) async fn ensure_effects_complete_for_run(
    connection: &mut sqlx::SqliteConnection,
    registry: &SchemaRegistry,
    scope: &IsolationScope,
) -> Result<(), ()> {
    let target = EffectTarget {
        scope: scope.clone(),
        actor: scope.agent_id.clone(),
    };
    let lifecycle = load_established(
        connection,
        registry,
        &LifecycleTarget {
            scope: scope.clone(),
            actor: scope.agent_id.clone(),
        },
    )
    .await
    .map_err(|_| ())?;
    let cursor = current_effect_cursor(connection, &target)
        .await
        .map_err(|_| ())?;
    let events = read_effect_events_at(
        connection,
        &target,
        lifecycle.schema_set.clone(),
        lifecycle.limits.clone(),
        &cursor,
    )
    .await
    .map_err(|_| ())?;
    let aggregate = fold_effect_events(&lifecycle.schema_set, &events).map_err(|_| ())?;
    if aggregate.effects.values().all(|entry| {
        entry.dispatch_state == EffectDispatchStateV1::Concluded
            && entry.reconciliation_state != EffectReconciliationStateV1::Required
    }) {
        Ok(())
    } else {
        Err(())
    }
}

async fn read_effect_events_at(
    connection: &mut sqlx::SqliteConnection,
    target: &EffectTarget,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
    inclusive_cursor: &EventCursor,
) -> Result<Vec<ValidatedEvent>, EffectError> {
    let horizon = inclusive_cursor
        .sequence
        .parse::<i64>()
        .map_err(|_| EffectError::new(EffectErrorKind::CursorMismatch))?;
    if horizon <= 0 {
        return Err(EffectError::new(EffectErrorKind::CursorMismatch));
    }
    let stream_id = effect_stream_id(&target.scope)?;
    let admitted = AdmittedRead {
        scope: target.scope.clone(),
        stream_id: Some(stream_id.clone()),
        schema_set,
        limits,
    };
    let (present, user) = user_key(&target.scope);
    let rows = sqlx::query(&format!(
        "SELECT {ROW_COLUMNS} FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? AND sequence_i64<=? ORDER BY sequence_i64,event_id"
    ))
    .bind(target.scope.tenant_id.as_str())
    .bind(present)
    .bind(user)
    .bind(target.scope.workspace_id.as_str())
    .bind(target.scope.run_id.as_str())
    .bind(target.scope.agent_id.as_str())
    .bind(stream_id.as_str())
    .bind(horizon)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| EffectError::new(EffectErrorKind::Store))?;
    let events: Result<Vec<_>, _> = rows
        .iter()
        .map(|row| {
            validate_row(row, &admitted)
                .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))
        })
        .collect();
    let events = events?;
    let last = events
        .last()
        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateNotFound))?;
    if last.envelope().sequence != inclusive_cursor.sequence
        || last.envelope().event_id != inclusive_cursor.event_id
    {
        return Err(EffectError::new(EffectErrorKind::CursorMismatch));
    }
    Ok(events)
}

fn fold_effect_events(
    schema_set: &SchemaSet,
    events: &[ValidatedEvent],
) -> Result<EffectAggregate, EffectError> {
    let first = events
        .first()
        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateNotFound))?;
    let initialization = first
        .downcast_payload::<EffectStreamInitializedPayloadV1>()
        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?
        .clone();
    if first.variant_id() != "effect-stream-initialized-v1"
        || first.envelope().sequence != "1"
        || initialization.source_run_id != first.envelope().scope.run_id
        || initialization.source_schema_set_ref != *schema_set.reference()
    {
        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
    }
    let history_schema = schema_set
        .schema_ref("effect-projection-hash-view")
        .ok_or_else(|| EffectError::new(EffectErrorKind::SchemaUnavailable))?;
    let mut history_digest = digest_json(
        "effect-history-seed",
        history_schema,
        &serde_json::json!({"algorithm":"effect-history-chain-v1"}),
    )
    .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    let mut effects = BTreeMap::new();
    let mut intents = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut late_receipt_count = 0_u64;
    let mut rejected_count = 0_u64;
    let mut reconciliation_observations = BTreeMap::new();
    let mut terminals = BTreeMap::new();
    let mut reconciled = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if event.envelope().sequence != (index + 1).to_string()
            || event.schema_set_ref() != schema_set.reference()
        {
            return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
        }
        if index > 0 {
            match event.variant_id() {
                "effect-intended-v1" => {
                    let payload = event
                        .downcast_payload::<EffectIntendedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.subject_actor != event.envelope().scope.agent_id
                        || payload.pair.pair_kind != EffectPairKindV1::ReserveIntent
                        || payload.pair.effect_event_id != event.envelope().event_id
                        || payload.pair.effect_id != payload.effect_id
                        || payload.pair.attempt_id != payload.attempt_id
                        || payload.pair.operation_id != payload.recovery_base_key.operation_id
                        || payload.pair.reservation_id != payload.recovery_base_key.reservation_id
                        || payload.recovery_base_key.scope != event.envelope().scope
                        || payload.recovery_base_key.effect_id != payload.effect_id
                        || payload.recovery_base_key.attempt_id != payload.attempt_id
                        || payload.recovery_base_key.executor_revision != payload.executor_revision
                        || payload.recovery_base_key.executor_descriptor_digest
                            != payload.executor_descriptor_digest
                        || payload.recovery_base_key.executor_config_digest
                            != payload.executor_config_digest
                        || payload.recovery_base_key.source_schema_set_ref
                            != *schema_set.reference()
                        || effects.contains_key(&payload.effect_id)
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    effects.insert(
                        payload.effect_id.clone(),
                        EffectProjectionEntryV1 {
                            effect_id: payload.effect_id.clone(),
                            attempt_id: payload.attempt_id.clone(),
                            effect_kind: payload.effect_kind.clone(),
                            request_digest: payload.request_digest.clone(),
                            idempotency_key_digest: payload.idempotency_key_digest.clone(),
                            operation_id: payload.pair.operation_id.clone(),
                            reservation_id: payload.pair.reservation_id.clone(),
                            executor_revision: payload.executor_revision.clone(),
                            executor_descriptor_digest: payload.executor_descriptor_digest.clone(),
                            dispatch_state: EffectDispatchStateV1::Intended,
                            external_conclusion: EffectExternalConclusionV1::Pending,
                            reconciliation_state: EffectReconciliationStateV1::NotRequired,
                            recovery_key: None,
                            receipt_digest: None,
                        },
                    );
                    intents.insert(payload.effect_id.clone(), payload.clone());
                }
                "effect-attempt-concluded-v1" => {
                    let payload = event
                        .downcast_payload::<EffectAttemptConcludedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get_mut(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.pair.pair_kind != EffectPairKindV1::TerminalConclusion
                        || payload.pair.effect_event_id != event.envelope().event_id
                        || payload.pair.effect_id != payload.effect_id
                        || payload.pair.attempt_id != payload.attempt_id
                        || payload.pair.operation_id != entry.operation_id
                        || payload.pair.reservation_id != entry.reservation_id
                        || payload.attempt_id != entry.attempt_id
                        || !matches!(
                            entry.dispatch_state,
                            EffectDispatchStateV1::Intended | EffectDispatchStateV1::Claimed
                        )
                        || !matches!(
                            payload.external_conclusion,
                            EffectExternalConclusionV1::Applied
                                | EffectExternalConclusionV1::NotApplied
                        )
                        || payload.reason_code.is_empty()
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    entry.dispatch_state = EffectDispatchStateV1::Concluded;
                    entry.external_conclusion = payload.external_conclusion;
                    terminals.insert(
                        payload.effect_id.clone(),
                        EffectTerminalFact::Attempt(payload.clone()),
                    );
                }
                "effect-dispatch-claimed-v1" => {
                    let payload = event
                        .downcast_payload::<EffectDispatchClaimedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get_mut(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let intent = intents
                        .get(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.attempt_id != entry.attempt_id
                        || payload.request_digest != entry.request_digest
                        || payload.executor_revision != entry.executor_revision
                        || payload.executor_descriptor_digest != entry.executor_descriptor_digest
                        || payload.executor_config_digest != intent.executor_config_digest
                        || payload.recovery_key.base != intent.recovery_base_key
                        || payload.recovery_key.claim_event_id != event.envelope().event_id
                        || payload.recovery_key.external_key_digest != payload.external_key_digest
                        || entry.dispatch_state != EffectDispatchStateV1::Intended
                        || claims.contains_key(&payload.effect_id)
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    entry.dispatch_state = EffectDispatchStateV1::Claimed;
                    entry.recovery_key = Some(payload.recovery_key.clone());
                    claims.insert(payload.effect_id.clone(), payload.clone());
                }
                "effect-receipt-admitted-v1" => {
                    let payload = event
                        .downcast_payload::<EffectReceiptAdmittedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get_mut(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if !claims.contains_key(&payload.effect_id) {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    if payload.pair.pair_kind != EffectPairKindV1::TerminalConclusion
                        || payload.pair.effect_event_id != event.envelope().event_id
                        || payload.pair.effect_id != payload.effect_id
                        || payload.pair.attempt_id != payload.attempt_id
                        || payload.pair.operation_id != entry.operation_id
                        || payload.pair.reservation_id != entry.reservation_id
                        || payload.attempt_id != entry.attempt_id
                        || payload.external_conclusion != EffectExternalConclusionV1::Applied
                        || entry.dispatch_state != EffectDispatchStateV1::Claimed
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    entry.dispatch_state = EffectDispatchStateV1::Concluded;
                    entry.external_conclusion = EffectExternalConclusionV1::Applied;
                    entry.receipt_digest = Some(payload.receipt_digest.clone());
                    terminals.insert(
                        payload.effect_id.clone(),
                        EffectTerminalFact::Receipt(payload.clone()),
                    );
                }
                "effect-reconciliation-required-v1" => {
                    let payload = event
                        .downcast_payload::<EffectReconciliationRequiredPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get_mut(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.pair.pair_kind != EffectPairKindV1::TerminalConclusion
                        || payload.pair.effect_event_id != event.envelope().event_id
                        || payload.pair.effect_id != payload.effect_id
                        || payload.pair.attempt_id != payload.attempt_id
                        || payload.pair.operation_id != entry.operation_id
                        || payload.pair.reservation_id != entry.reservation_id
                        || payload.attempt_id != entry.attempt_id
                        || !matches!(
                            payload.external_conclusion,
                            EffectExternalConclusionV1::Partial
                                | EffectExternalConclusionV1::Unknown
                        )
                        || !payload
                            .limitations
                            .windows(2)
                            .all(|items| items[0] < items[1])
                        || entry.dispatch_state != EffectDispatchStateV1::Claimed
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    entry.dispatch_state = EffectDispatchStateV1::Concluded;
                    entry.external_conclusion = payload.external_conclusion;
                    entry.reconciliation_state = EffectReconciliationStateV1::Required;
                    terminals.insert(
                        payload.effect_id.clone(),
                        EffectTerminalFact::Reconciliation(payload.clone()),
                    );
                }
                "effect-late-receipt-observed-v1" => {
                    let payload = event
                        .downcast_payload::<EffectLateReceiptObservedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.attempt_id != entry.attempt_id
                        || entry.dispatch_state != EffectDispatchStateV1::Concluded
                        || payload.reason_code.is_empty()
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    late_receipt_count = late_receipt_count
                        .checked_add(1)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                }
                "effect-message-rejected-v1" => {
                    let payload = event
                        .downcast_payload::<EffectMessageRejectedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.reason_code.is_empty()
                        || payload.effect_registry_revision
                            != initialization.effect_registry_revision
                        || payload.effect_id.as_ref().is_some_and(|effect_id| {
                            effects.get(effect_id).is_none_or(|entry| {
                                payload.attempt_id.as_ref() != Some(&entry.attempt_id)
                            })
                        })
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    rejected_count = rejected_count
                        .checked_add(1)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                }
                "effect-reconciliation-observed-v1" => {
                    let payload = event
                        .downcast_payload::<EffectReconciliationObservedPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.attempt_id != entry.attempt_id
                        || entry.reconciliation_state != EffectReconciliationStateV1::Required
                        || payload
                            .source_observation_event_ids
                            .windows(2)
                            .any(|ids| ids[0] >= ids[1])
                        || reconciliation_observations
                            .insert(
                                event.envelope().event_id.clone(),
                                payload.evidence_fingerprint.clone(),
                            )
                            .is_some()
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                }
                "effect-reconciled-v1" => {
                    let payload = event
                        .downcast_payload::<EffectReconciledPayloadV1>()
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    let entry = effects
                        .get_mut(&payload.effect_id)
                        .ok_or_else(|| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
                    if payload.attempt_id != entry.attempt_id
                        || entry.reconciliation_state != EffectReconciliationStateV1::Required
                        || !matches!(
                            payload.reconciliation_state,
                            EffectReconciliationStateV1::ResolvedApplied
                                | EffectReconciliationStateV1::ResolvedNotApplied
                                | EffectReconciliationStateV1::ResolvedPartial
                        )
                        || reconciliation_observations.get(&payload.source_observation_event_id)
                            != Some(&payload.evidence_fingerprint)
                    {
                        return Err(EffectError::new(EffectErrorKind::AggregateCorrupt));
                    }
                    entry.reconciliation_state = payload.reconciliation_state;
                    reconciled.insert(payload.effect_id.clone(), payload.clone());
                }
                _ => return Err(EffectError::new(EffectErrorKind::AggregateCorrupt)),
            }
        }
        history_digest = digest_json(
            "effect-history-step",
            history_schema,
            &serde_json::json!({
                "previous_digest": history_digest,
                "sequence": event.envelope().sequence,
                "event_id": event.envelope().event_id,
                "payload_digest": event.envelope().payload_digest,
                "variant_id": event.variant_id(),
            }),
        )
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    }
    Ok(EffectAggregate {
        initialization,
        inclusive_cursor: EventCursor {
            sequence: events
                .last()
                .expect("non-empty checked")
                .envelope()
                .sequence
                .clone(),
            event_id: events
                .last()
                .expect("non-empty checked")
                .envelope()
                .event_id
                .clone(),
        },
        effects,
        intents,
        claims,
        late_receipt_count,
        rejected_count,
        reconciliation_observations,
        terminals,
        reconciled,
        history_digest,
    })
}

fn build_projection(
    store_id: &str,
    scope: &IsolationScope,
    actor: &AgentId,
    schema_set: &SchemaSet,
    aggregate: EffectAggregate,
) -> Result<EffectProjectionV1, EffectError> {
    let view = EffectProjectionHashViewV1 {
        source_store_id: store_id.to_owned(),
        scope: scope.clone(),
        owner_actor: actor.clone(),
        effect_stream_id: effect_stream_id(scope)?,
        inclusive_cursor: aggregate.inclusive_cursor,
        source_schema_set_ref: schema_set.reference().clone(),
        effect_registry_revision: aggregate.initialization.effect_registry_revision,
        effect_registry_config_digest: aggregate.initialization.effect_registry_config_digest,
        reducer_revision: aggregate.initialization.reducer_revision,
        history_digest: aggregate.history_digest,
        effects: aggregate.effects.into_values().collect(),
        late_receipt_count: aggregate.late_receipt_count,
        rejected_count: aggregate.rejected_count,
    };
    let hash_schema = schema_set
        .schema_ref("effect-projection-hash-view")
        .ok_or_else(|| EffectError::new(EffectErrorKind::SchemaUnavailable))?;
    let projection_digest = digest_json(
        "effect-projection",
        hash_schema,
        &serde_json::to_value(&view)
            .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?,
    )
    .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    let projection = EffectProjectionV1 {
        source_store_id: view.source_store_id,
        scope: view.scope,
        owner_actor: view.owner_actor,
        effect_stream_id: view.effect_stream_id,
        inclusive_cursor: view.inclusive_cursor,
        source_schema_set_ref: view.source_schema_set_ref,
        effect_registry_revision: view.effect_registry_revision,
        effect_registry_config_digest: view.effect_registry_config_digest,
        reducer_revision: view.reducer_revision,
        history_digest: view.history_digest,
        effects: view.effects,
        late_receipt_count: view.late_receipt_count,
        rejected_count: view.rejected_count,
        projection_digest,
    };
    let bytes = canonical(&projection)
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?
        .into_bytes();
    schema_set
        .parse_record::<EffectProjectionV1>(&bytes)
        .map_err(|_| EffectError::new(EffectErrorKind::AggregateCorrupt))?;
    Ok(projection)
}

#[cfg(test)]
mod tests;
