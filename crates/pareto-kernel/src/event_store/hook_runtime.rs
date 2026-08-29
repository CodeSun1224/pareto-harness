use std::{collections::BTreeMap, sync::Arc};

use pareto_protocol::{
    AgentId, Digest, EventCursor, EventId, ExecutionMode, GateDecisionV1, HookBusinessDecisionV1,
    HookDecisionId, HookExecutionStatusV1, HookId, HookInvocationId,
    HookInvocationProjectionEntryV1, HookInvocationReservedPayloadV1,
    HookInvocationSkippedPayloadV1, HookInvocationTerminalPayloadV1, HookKindV1,
    HookLateResultObservedPayloadV1, HookMessageRejectedPayloadV1, HookPairBindingV1, HookPhaseV1,
    HookPointFinalizedPayloadV1, HookPointStartedPayloadV1, HookPointV1, HookProjectionHashViewV1,
    HookProjectionV1, HookRegistrationV1, HookRegistryRevisionV1, HookStreamInitializedPayloadV1,
    IsolationScope, ObserverResultV1, OperationReservedPayloadV1, OperationSettledPayloadV1,
    ProtectedProposalHashViewV1, ProtocolLimitsRef, RevisionId, RunManifest, SchemaSet, StreamId,
    TimeoutKeyV1, TransformProposalV1, ValidatedEvent, digest_json,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::SqliteConnection;

use super::lifecycle::{LifecycleTarget, lifecycle_event, load_established};
use super::runtime_control::{
    ClockSample, HookControlEventContext, OperationLease, RuntimeControlTarget, control_event,
    make_lease, prepare_hook_reservation_event, prepare_hook_settlement_event,
    runtime_control_stream_id, validate_runtime_control_history,
};
use super::{
    AdmittedAppend, AdmittedRead, AppendResult, AtomicPairFault, ErrorKind, EventStore,
    EventStoreError, KernelAuthority, PreparedEvent, SchemaRegistry, append_atomic_pair, canonical,
    check_prepared_idempotency,
};

const HOOK_REDUCER_REVISION: &str = "rev_hook-projection-reducer-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HookErrorKind {
    Unauthorized,
    ManifestInvalid,
    SchemaUnavailable,
    AggregateNotFound,
    AggregateCorrupt,
    UnsupportedMode,
    IdempotencyConflict,
    PartialPair,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairPresence {
    Zero,
    Two,
}

async fn pair_presence(
    connection: &mut SqliteConnection,
    control: &PreparedEvent,
    hook: &PreparedEvent,
) -> Result<PairPresence, HookError> {
    let control = check_prepared_idempotency(connection, control)
        .await
        .map_err(map_store_error)?;
    let hook = check_prepared_idempotency(connection, hook)
        .await
        .map_err(map_store_error)?;
    match (control, hook) {
        (None, None) => Ok(PairPresence::Zero),
        (Some(_), Some(_)) => Ok(PairPresence::Two),
        _ => Err(HookError::new(HookErrorKind::PartialPair)),
    }
}

fn validate_reserve_pair_command(
    target: &HookTarget,
    control_target: &RuntimeControlTarget,
    command: &HookReservePairCommandV1,
) -> Result<(), HookError> {
    let fingerprint = reserve_pair_fingerprint(command)?;
    if command.scope != target.scope
        || command.scope != control_target.scope
        || command.owner != target.actor
        || command.owner != control_target.principal
        || command.control_stream_id
            != runtime_control_stream_id(&command.scope)
                .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?
        || command.hook_stream_id != hook_stream_id(&command.scope)?
    {
        return Err(HookError::new(HookErrorKind::Unauthorized));
    }
    if command.pair.pair_fingerprint != fingerprint
        || command.control_payload.hook_pair.as_ref() != Some(&command.pair)
        || command.hook_payload.pair != command.pair
        || command.hook_payload.invocation_id != command.pair.invocation_id
        || command.control_payload.operation_id != command.pair.operation_id
        || command.control_payload.reservation_id != command.pair.reservation_id
        || command.hook_payload.reserved_usage != command.control_payload.trusted_reservation
    {
        return Err(HookError::new(HookErrorKind::IdempotencyConflict));
    }
    Ok(())
}

fn validate_terminal_pair_command(
    target: &HookTarget,
    control_target: &RuntimeControlTarget,
    command: &HookTerminalPairCommandV1,
) -> Result<(), HookError> {
    let fingerprint = terminal_pair_fingerprint(command)?;
    if command.scope != target.scope
        || command.scope != control_target.scope
        || command.owner != target.actor
        || command.owner != control_target.principal
        || command.control_stream_id
            != runtime_control_stream_id(&command.scope)
                .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?
        || command.hook_stream_id != hook_stream_id(&command.scope)?
    {
        return Err(HookError::new(HookErrorKind::Unauthorized));
    }
    if command.pair.pair_fingerprint != fingerprint
        || command.control_payload.hook_pair.as_ref() != Some(&command.pair)
        || command.hook_payload.pair != command.pair
        || command.hook_payload.invocation_id != command.pair.invocation_id
        || command.control_payload.operation_id != command.pair.operation_id
        || command.control_payload.reservation_id != command.pair.reservation_id
        || command.hook_payload.accounted_usage != command.control_payload.accounted_usage
        || match (&command.authority, command.control_payload.outcome) {
            (HookTerminalAuthorityV1::LiveLease { lease_fingerprint }, outcome)
                if outcome != pareto_protocol::OperationOutcomeV1::TimedOut =>
            {
                command
                    .control_payload
                    .callback_authority
                    .as_ref()
                    .is_none_or(|authority| &authority.lease_fingerprint != lease_fingerprint)
            }
            (
                HookTerminalAuthorityV1::TimeoutRecovery { .. },
                pareto_protocol::OperationOutcomeV1::TimedOut,
            ) => false,
            _ => true,
        }
    {
        return Err(HookError::new(HookErrorKind::IdempotencyConflict));
    }
    Ok(())
}

fn next_sequence(cursor: &EventCursor) -> Result<i64, HookError> {
    cursor
        .sequence
        .parse::<i64>()
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn map_store_error(error: EventStoreError) -> HookError {
    let kind = match error.kind {
        ErrorKind::IdempotencyConflict => HookErrorKind::IdempotencyConflict,
        ErrorKind::DatabaseCorrupt => HookErrorKind::PartialPair,
        _ => HookErrorKind::Store,
    };
    HookError::new(kind)
}

#[derive(Debug, Eq, PartialEq)]
struct HookError {
    kind: HookErrorKind,
}

impl HookError {
    fn new(kind: HookErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone)]
struct HookTarget {
    scope: IsolationScope,
    actor: pareto_protocol::AgentId,
}

#[derive(Clone)]
struct InitializeHookStream {
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    hook_registry_revision: RevisionId,
    hook_registry_config_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HookAggregate {
    initialization: HookStreamInitializedPayloadV1,
    invocations: BTreeMap<pareto_protocol::HookInvocationId, HookInvocationProjectionEntryV1>,
    finalized_points: Vec<HookDecisionId>,
    skipped_count: u64,
    late_result_count: u64,
    rejected_count: u64,
    history_digest: Digest,
    inclusive_cursor: EventCursor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedHookRegistry {
    revision: RevisionId,
    config_digest: Digest,
    registrations: Vec<HookRegistrationV1>,
}

impl ResolvedHookRegistry {
    fn resolve(
        manifest: &RunManifest,
        registry: &HookRegistryRevisionV1,
    ) -> Result<Self, HookError> {
        registry
            .validate()
            .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?;
        let expected_revision = manifest
            .revisions
            .get("hook_registry")
            .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
        let expected_config = manifest
            .hook_registry_config_digest
            .as_ref()
            .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
        let computed_config = registry_config_digest(&registry.registrations)?;
        if &registry.metadata.revision_id != expected_revision
            || &registry.config_digest != expected_config
            || registry.config_digest != computed_config
        {
            return Err(HookError::new(HookErrorKind::ManifestInvalid));
        }
        Ok(Self {
            revision: registry.metadata.revision_id.clone(),
            config_digest: registry.config_digest.clone(),
            registrations: registry.registrations.clone(),
        })
    }

    fn ordered_for_point(&self, point: HookPointV1) -> Vec<&HookRegistrationV1> {
        let mut registrations: Vec<_> = self
            .registrations
            .iter()
            .filter(|registration| registration.hook_points.contains(&point))
            .filter(|registration| phase_for(point, registration.kind).is_some())
            .collect();
        registrations.sort_by(|left, right| {
            phase_for(point, left.kind)
                .cmp(&phase_for(point, right.kind))
                .then(left.priority.cmp(&right.priority))
                .then(left.hook_id.cmp(&right.hook_id))
                .then(left.hook_revision.cmp(&right.hook_revision))
        });
        registrations
    }
}

fn phase_for(point: HookPointV1, kind: HookKindV1) -> Option<HookPhaseV1> {
    match (point, kind) {
        (HookPointV1::BeforeProposalAdmission, HookKindV1::Transform) => {
            Some(HookPhaseV1::Transform)
        }
        (
            HookPointV1::BeforeProposalAdmission | HookPointV1::BeforeAuthoritativeCommit,
            HookKindV1::Gate,
        ) => Some(HookPhaseV1::Gate),
        (_, HookKindV1::Observer) => Some(HookPhaseV1::Observer),
        _ => None,
    }
}

fn registry_config_digest(registrations: &[HookRegistrationV1]) -> Result<Digest, HookError> {
    let canonical =
        canonical(&registrations).map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?;
    let mut hasher = Sha256::new();
    hasher.update(b"hook-registry-config-v1\0");
    hasher.update(canonical.as_bytes());
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HookInvocationLease {
    invocation_id: HookInvocationId,
    hook_id: HookId,
    input_digest: Digest,
    scope: IsolationScope,
    narrowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HookRequestView {
    hook_point: HookPointV1,
    phase: HookPhaseV1,
    input_digest: Digest,
    fixed_business_decision: Option<pareto_protocol::HookBusinessDecisionV1>,
}

#[derive(Clone, Debug, PartialEq)]
enum UntrustedHookOutput {
    Observer(ObserverResultV1),
    Gate(GateDecisionV1),
    Transform {
        proposal: Box<TransformProposalV1>,
        protected: Box<ProtectedProposalHashViewV1>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct HookPointEvaluation {
    proposal: TransformProposalV1,
    business_decision: HookBusinessDecisionV1,
    execution_status: HookExecutionStatusV1,
    reason_code: String,
    observer_results: Vec<(HookId, ObserverResultV1)>,
}

fn evaluate_point(
    schema_set: &SchemaSet,
    registry: &ResolvedHookRegistry,
    point: HookPointV1,
    initial: &TransformProposalV1,
    protected: &ProtectedProposalHashViewV1,
    outputs: &BTreeMap<HookId, Result<UntrustedHookOutput, String>>,
) -> HookPointEvaluation {
    let mut proposal = initial.clone();
    let mut required_gate_count = 0_u32;
    let mut required_gate_allows = 0_u32;
    let mut gate_denied = false;
    let mut observer_results = Vec::new();
    let gate_bearing = matches!(
        point,
        HookPointV1::BeforeProposalAdmission | HookPointV1::BeforeAuthoritativeCommit
    );
    let mut business_decision = if gate_bearing {
        HookBusinessDecisionV1::Allow
    } else {
        HookBusinessDecisionV1::ObserveOnly
    };
    let mut execution_status = HookExecutionStatusV1::Completed;
    let mut reason_code = "completed".to_owned();
    for registration in registry.ordered_for_point(point) {
        let Some(output) = outputs.get(&registration.hook_id) else {
            if registration.kind == HookKindV1::Transform {
                execution_status = HookExecutionStatusV1::TransformFailed;
                business_decision = HookBusinessDecisionV1::Deny;
                reason_code = "transform_missing".to_owned();
                break;
            }
            if registration.kind == HookKindV1::Gate && registration.required == Some(true) {
                required_gate_count += 1;
                gate_denied = true;
                reason_code = "required_gate_missing".to_owned();
            }
            continue;
        };
        let output = match output {
            Ok(output) if output_is_bounded(schema_set, registration, output) => output,
            _ if registration.kind == HookKindV1::Transform => {
                execution_status = HookExecutionStatusV1::TransformFailed;
                business_decision = HookBusinessDecisionV1::Deny;
                reason_code = "transform_output_invalid".to_owned();
                break;
            }
            _ if registration.kind == HookKindV1::Gate => {
                if registration.required == Some(true) {
                    required_gate_count += 1;
                }
                gate_denied = true;
                reason_code = "gate_output_invalid".to_owned();
                continue;
            }
            _ => {
                if registration.observer_failure_policy
                    == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed)
                {
                    execution_status = HookExecutionStatusV1::ObserverFailed;
                    reason_code = "observer_failed_closed".to_owned();
                }
                continue;
            }
        };
        match (registration.kind, output) {
            (
                HookKindV1::Transform,
                UntrustedHookOutput::Transform {
                    proposal: candidate,
                    protected: candidate_protected,
                },
            ) => {
                let contract = registration.transform_contract.as_ref();
                if candidate_protected.as_ref() != protected
                    || candidate.proposal_id != proposal.proposal_id
                    || candidate.schema_ref != proposal.schema_ref
                    || contract.is_none_or(|contract| {
                        !transform_changes_allowed(&proposal, candidate, &contract.allowed_fields)
                    })
                {
                    execution_status = HookExecutionStatusV1::TransformFailed;
                    business_decision = HookBusinessDecisionV1::Deny;
                    reason_code = "transform_protected_field".to_owned();
                    break;
                }
                proposal = candidate.as_ref().clone();
            }
            (HookKindV1::Gate, UntrustedHookOutput::Gate(decision)) => {
                if registration.required == Some(true) {
                    required_gate_count += 1;
                }
                match decision {
                    GateDecisionV1::Allow {} if registration.required == Some(true) => {
                        required_gate_allows += 1;
                    }
                    GateDecisionV1::Deny { .. } => {
                        gate_denied = true;
                        reason_code = "gate_denied".to_owned();
                    }
                    GateDecisionV1::Abstain {} if registration.required == Some(true) => {
                        gate_denied = true;
                        reason_code = "required_gate_abstained".to_owned();
                    }
                    _ => {}
                }
            }
            (HookKindV1::Observer, UntrustedHookOutput::Observer(result)) => {
                if matches!(result, ObserverResultV1::Failure { .. })
                    && registration.observer_failure_policy
                        == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed)
                {
                    execution_status = HookExecutionStatusV1::ObserverFailed;
                    reason_code = "observer_failed_closed".to_owned();
                }
                observer_results.push((registration.hook_id.clone(), result.clone()));
            }
            (HookKindV1::Transform, _) | (HookKindV1::Gate, _) | (HookKindV1::Observer, _) => {
                if registration.kind == HookKindV1::Transform {
                    execution_status = HookExecutionStatusV1::TransformFailed;
                    business_decision = HookBusinessDecisionV1::Deny;
                    reason_code = "hook_kind_mismatch".to_owned();
                    break;
                }
                gate_denied |= registration.kind == HookKindV1::Gate;
            }
        }
    }
    if gate_bearing
        && execution_status != HookExecutionStatusV1::TransformFailed
        && (required_gate_count == 0 || required_gate_allows != required_gate_count || gate_denied)
    {
        business_decision = HookBusinessDecisionV1::Deny;
        if execution_status == HookExecutionStatusV1::Completed {
            execution_status = HookExecutionStatusV1::GateDenied;
        }
        if required_gate_count == 0 {
            reason_code = "required_gate_empty".to_owned();
        }
    }
    if execution_status == HookExecutionStatusV1::TransformFailed {
        proposal = initial.clone();
    }
    HookPointEvaluation {
        proposal,
        business_decision,
        execution_status,
        reason_code,
        observer_results,
    }
}

fn output_is_bounded(
    schema_set: &SchemaSet,
    registration: &HookRegistrationV1,
    output: &UntrustedHookOutput,
) -> bool {
    let matches_kind = matches!(
        (registration.kind, output),
        (HookKindV1::Observer, UntrustedHookOutput::Observer(_))
            | (HookKindV1::Gate, UntrustedHookOutput::Gate(_))
            | (HookKindV1::Transform, UntrustedHookOutput::Transform { .. })
    );
    if !matches_kind {
        return false;
    }
    let value = match output {
        UntrustedHookOutput::Observer(value) => serde_json::to_value(value),
        UntrustedHookOutput::Gate(value) => serde_json::to_value(value),
        UntrustedHookOutput::Transform { proposal, .. } => serde_json::to_value(proposal),
    };
    let Ok(value) = value else { return false };
    let Ok(bytes) = canonical(&value) else {
        return false;
    };
    let (depth, items) = json_shape(&value);
    bytes.len() <= registration.limits.max_output_bytes as usize
        && depth <= registration.limits.max_depth as usize
        && items <= registration.limits.max_collection_items as usize
        && schema_set
            .validate_value_against(&registration.output_schema_ref, &value)
            .is_ok()
}

fn json_shape(value: &serde_json::Value) -> (usize, usize) {
    match value {
        serde_json::Value::Array(values) => values.iter().fold((1, values.len()), |acc, value| {
            let child = json_shape(value);
            (acc.0.max(child.0 + 1), acc.1.saturating_add(child.1))
        }),
        serde_json::Value::Object(values) => {
            values.values().fold((1, values.len()), |acc, value| {
                let child = json_shape(value);
                (acc.0.max(child.0 + 1), acc.1.saturating_add(child.1))
            })
        }
        _ => (1, 0),
    }
}

fn transform_changes_allowed(
    before: &TransformProposalV1,
    after: &TransformProposalV1,
    allowed: &[String],
) -> bool {
    let (Some(before), Some(after)) = (before.fields.as_object(), after.fields.as_object()) else {
        return before.fields == after.fields;
    };
    before.keys().chain(after.keys()).all(|key| {
        before.get(key) == after.get(key) || allowed.binary_search(&format!("/{key}")).is_ok()
    })
}

trait FakeHookHandler {
    fn invoke(&self, lease: &HookInvocationLease, request: &HookRequestView)
    -> UntrustedHookOutput;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvocationLineage {
    hook_id: HookId,
    phase: HookPhaseV1,
    input_digest: Digest,
    predecessor_output_digest: Option<Digest>,
}

#[derive(Clone, Serialize)]
struct HookReservePairCommandV1 {
    scope: IsolationScope,
    owner: AgentId,
    control_stream_id: StreamId,
    hook_stream_id: StreamId,
    expected_control_cursor: EventCursor,
    expected_hook_cursor: EventCursor,
    pair: HookPairBindingV1,
    occurred_at: String,
    correlation_id: String,
    control_payload: OperationReservedPayloadV1,
    hook_payload: HookInvocationReservedPayloadV1,
    clock: ClockSample,
}

#[derive(Clone, Serialize)]
struct HookTerminalPairCommandV1 {
    scope: IsolationScope,
    owner: AgentId,
    control_stream_id: StreamId,
    hook_stream_id: StreamId,
    expected_control_cursor: EventCursor,
    expected_hook_cursor: EventCursor,
    pair: HookPairBindingV1,
    occurred_at: String,
    correlation_id: String,
    control_payload: OperationSettledPayloadV1,
    hook_payload: HookInvocationTerminalPayloadV1,
    authority: HookTerminalAuthorityV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
enum HookTerminalAuthorityV1 {
    LiveLease { lease_fingerprint: Digest },
    TimeoutRecovery { timeout_key: Box<TimeoutKeyV1> },
}

#[derive(Debug)]
struct HookReservePairResult {
    control: AppendResult,
    hook: AppendResult,
    lease: OperationLease,
    already_committed: bool,
}

#[derive(Debug)]
struct HookTerminalPairResult {
    control: AppendResult,
    hook: AppendResult,
    already_committed: bool,
}

fn seal_reserve_pair_command(
    mut command: HookReservePairCommandV1,
) -> Result<HookReservePairCommandV1, HookError> {
    let fingerprint = reserve_pair_fingerprint(&command)?;
    command.pair.pair_fingerprint = fingerprint.clone();
    command.control_payload.hook_pair = Some(command.pair.clone());
    command.hook_payload.pair = command.pair.clone();
    Ok(command)
}

fn seal_terminal_pair_command(
    mut command: HookTerminalPairCommandV1,
) -> Result<HookTerminalPairCommandV1, HookError> {
    let fingerprint = terminal_pair_fingerprint(&command)?;
    command.pair.pair_fingerprint = fingerprint.clone();
    command.control_payload.hook_pair = Some(command.pair.clone());
    command.hook_payload.pair = command.pair.clone();
    Ok(command)
}

fn reserve_pair_fingerprint(command: &HookReservePairCommandV1) -> Result<Digest, HookError> {
    let mut normalized = command.clone();
    clear_pair_fingerprint(&mut normalized.pair)?;
    normalized.control_payload.hook_pair = Some(normalized.pair.clone());
    normalized.hook_payload.pair = normalized.pair.clone();
    pair_digest("hook-reserve-pair-command-v1", &normalized)
}

fn terminal_pair_fingerprint(command: &HookTerminalPairCommandV1) -> Result<Digest, HookError> {
    let mut normalized = command.clone();
    clear_pair_fingerprint(&mut normalized.pair)?;
    normalized.control_payload.hook_pair = Some(normalized.pair.clone());
    normalized.hook_payload.pair = normalized.pair.clone();
    pair_digest("hook-terminal-pair-command-v1", &normalized)
}

fn clear_pair_fingerprint(pair: &mut HookPairBindingV1) -> Result<(), HookError> {
    pair.pair_fingerprint = Digest::parse(format!("sha256:{}", "0".repeat(64)))
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
    Ok(())
}

fn pair_digest<T: Serialize>(domain: &str, value: &T) -> Result<Digest, HookError> {
    let bytes = canonical(value).map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes.as_bytes());
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn planned_lineage(
    registry: &ResolvedHookRegistry,
    point: HookPointV1,
    initial_input_digest: &Digest,
    transform_outputs: &BTreeMap<HookId, Digest>,
) -> Result<Vec<InvocationLineage>, HookError> {
    let ordered = registry.ordered_for_point(point);
    let mut final_input = initial_input_digest.clone();
    let mut predecessor = None;
    let mut lineage = Vec::with_capacity(ordered.len());
    for registration in ordered {
        let phase = phase_for(point, registration.kind)
            .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
        lineage.push(InvocationLineage {
            hook_id: registration.hook_id.clone(),
            phase,
            input_digest: final_input.clone(),
            predecessor_output_digest: if phase == HookPhaseV1::Transform {
                predecessor.clone()
            } else {
                None
            },
        });
        if phase == HookPhaseV1::Transform {
            let output = transform_outputs
                .get(&registration.hook_id)
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?
                .clone();
            predecessor = Some(output.clone());
            final_input = output;
        }
    }
    Ok(lineage)
}

impl EventStore {
    async fn initialize_hook_stream(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        command: &InitializeHookStream,
    ) -> Result<(), HookError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        transaction
            .rollback()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        let manifest = &lifecycle.state.manifest;
        validate_hook_manifest(manifest, command)?;
        let stream_id = hook_stream_id(&target.scope)?;
        let payload = HookStreamInitializedPayloadV1 {
            source_run_id: target.scope.run_id.clone(),
            hook_registry_revision: command.hook_registry_revision.clone(),
            hook_registry_config_digest: command.hook_registry_config_digest.clone(),
            source_schema_set_ref: manifest.schema_set_ref.clone(),
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
            "hook-stream-initialized",
            &payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?;
        let authority = KernelAuthority::authenticated(
            target.scope.clone(),
            target.actor.clone(),
            Some(stream_id),
            lifecycle.schema_set.reference().clone(),
            lifecycle.limits.clone(),
        );
        let admitted =
            AdmittedAppend::admit(&authority, event, lifecycle.schema_set, lifecycle.limits)
                .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        self.append(admitted)
            .await
            .map(|_| ())
            .map_err(|_| HookError::new(HookErrorKind::Store))
    }

    async fn append_hook_reserve_pair(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        command: &HookReservePairCommandV1,
        fault: AtomicPairFault,
    ) -> Result<HookReservePairResult, HookError> {
        validate_reserve_pair_command(target, control_target, command)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        let control_sequence = next_sequence(&command.expected_control_cursor)?;
        let hook_sequence = next_sequence(&command.expected_hook_cursor)?;
        let control_event = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            control_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "operation-reserved",
            &command.control_payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let hook_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.hook_stream_id,
            &command.pair.hook_event_id,
            hook_sequence,
            &command.occurred_at,
            &command.correlation_id,
            "hook-invocation-reserved",
            &command.hook_payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
        let control_prepared =
            PreparedEvent::new(&control_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let hook_prepared =
            PreparedEvent::new(&hook_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let presence = pair_presence(&mut transaction, &control_prepared, &hook_prepared).await?;
        if presence == PairPresence::Zero {
            let hook_aggregate = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            let hook_aggregate = fold_hook_events(&lifecycle.schema_set, &hook_aggregate)?;
            if hook_aggregate.inclusive_cursor != command.expected_hook_cursor {
                return Err(HookError::new(HookErrorKind::IdempotencyConflict));
            }
            let (admitted_control, _) = prepare_hook_reservation_event(
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
            .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
            if admitted_control.envelope_json != control_prepared.envelope_json {
                return Err(HookError::new(HookErrorKind::IdempotencyConflict));
            }
        } else {
            validate_runtime_control_history(&mut transaction, registry, control_target)
                .await
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let hook_events = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            fold_hook_events(&lifecycle.schema_set, &hook_events)?;
        }
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &hook_prepared, fault)
            .await
            .map_err(map_store_error)?;
        let lease = make_lease(control_target, &command.control_payload, &command.clock)
            .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        transaction
            .commit()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        Ok(HookReservePairResult {
            control: pair.first,
            hook: pair.second,
            lease,
            already_committed: pair.already_committed,
        })
    }

    async fn append_hook_terminal_pair(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        command: &HookTerminalPairCommandV1,
        fault: AtomicPairFault,
    ) -> Result<HookTerminalPairResult, HookError> {
        validate_terminal_pair_command(target, control_target, command)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        let control_event = control_event(
            &lifecycle,
            &command.control_stream_id,
            &command.pair.control_event_id,
            next_sequence(&command.expected_control_cursor)?,
            &command.occurred_at,
            &command.correlation_id,
            "operation-settled",
            &command.control_payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        let hook_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.hook_stream_id,
            &command.pair.hook_event_id,
            next_sequence(&command.expected_hook_cursor)?,
            &command.occurred_at,
            &command.correlation_id,
            "hook-invocation-terminal",
            &command.hook_payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let control_prepared =
            PreparedEvent::new(&control_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let hook_prepared =
            PreparedEvent::new(&hook_event, &lifecycle.schema_set, &lifecycle.limits)
                .map_err(map_store_error)?;
        let presence = pair_presence(&mut transaction, &control_prepared, &hook_prepared).await?;
        if presence == PairPresence::Zero {
            let hook_events = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            let aggregate = fold_hook_events(&lifecycle.schema_set, &hook_events)?;
            if aggregate.inclusive_cursor != command.expected_hook_cursor
                || !aggregate
                    .invocations
                    .contains_key(&command.pair.invocation_id)
            {
                return Err(HookError::new(HookErrorKind::IdempotencyConflict));
            }
            let admitted_control = prepare_hook_settlement_event(
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
                match &command.authority {
                    HookTerminalAuthorityV1::TimeoutRecovery { timeout_key } => {
                        Some(timeout_key.as_ref())
                    }
                    HookTerminalAuthorityV1::LiveLease { .. } => None,
                },
            )
            .await
            .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
            if admitted_control.envelope_json != control_prepared.envelope_json {
                return Err(HookError::new(HookErrorKind::IdempotencyConflict));
            }
        } else {
            validate_runtime_control_history(&mut transaction, registry, control_target)
                .await
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let hook_events = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            fold_hook_events(&lifecycle.schema_set, &hook_events)?;
        }
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &hook_prepared, fault)
            .await
            .map_err(map_store_error)?;
        transaction
            .commit()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        Ok(HookTerminalPairResult {
            control: pair.first,
            hook: pair.second,
            already_committed: pair.already_committed,
        })
    }

    async fn hook_projection(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
    ) -> Result<HookProjectionV1, HookError> {
        let (schema_set, limits, manifest) = self.hook_source(registry, target).await?;
        let events = self
            .read_hook_events(target, schema_set.clone(), limits)
            .await?;
        let aggregate = fold_hook_events(&schema_set, &events)?;
        if aggregate.initialization.hook_registry_revision
            != *manifest
                .revisions
                .get("hook_registry")
                .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?
            || Some(&aggregate.initialization.hook_registry_config_digest)
                != manifest.hook_registry_config_digest.as_ref()
        {
            return Err(HookError::new(HookErrorKind::AggregateCorrupt));
        }
        build_projection(
            &self.store_id,
            &target.scope,
            &target.actor,
            &schema_set,
            aggregate,
        )
    }

    async fn recorded_hook_projection(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        mode: &ExecutionMode,
    ) -> Result<HookProjectionV1, HookError> {
        if !matches!(mode, ExecutionMode::RecordedReplay { .. }) {
            return Err(HookError::new(HookErrorKind::UnsupportedMode));
        }
        self.hook_projection(registry, target).await
    }

    async fn hook_source(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
    ) -> Result<(Arc<SchemaSet>, ProtocolLimitsRef, RunManifest), HookError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        let lifecycle = load_established(
            &mut transaction,
            registry,
            &LifecycleTarget {
                scope: target.scope.clone(),
                actor: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
        transaction
            .rollback()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        if lifecycle.state.manifest.schema_ref.major != 2 {
            return Err(HookError::new(HookErrorKind::ManifestInvalid));
        }
        Ok((
            lifecycle.schema_set,
            lifecycle.limits,
            lifecycle.state.manifest,
        ))
    }

    async fn read_hook_events(
        &self,
        target: &HookTarget,
        schema_set: Arc<SchemaSet>,
        limits: ProtocolLimitsRef,
    ) -> Result<Vec<ValidatedEvent>, HookError> {
        let admitted = AdmittedRead {
            scope: target.scope.clone(),
            stream_id: Some(hook_stream_id(&target.scope)?),
            schema_set,
            limits,
        };
        let mut cursor = None;
        let mut events = Vec::new();
        loop {
            let page = self
                .read(&admitted, cursor.as_ref(), 256)
                .await
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
            events.extend(page.events);
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        if events.is_empty() {
            Err(HookError::new(HookErrorKind::AggregateNotFound))
        } else {
            Ok(events)
        }
    }
}

fn validate_hook_manifest(
    manifest: &RunManifest,
    command: &InitializeHookStream,
) -> Result<(), HookError> {
    if manifest.schema_ref.major != 2
        || manifest.revisions.get("hook_registry") != Some(&command.hook_registry_revision)
        || manifest.hook_registry_config_digest.as_ref()
            != Some(&command.hook_registry_config_digest)
    {
        Err(HookError::new(HookErrorKind::ManifestInvalid))
    } else {
        Ok(())
    }
}

fn hook_stream_id(scope: &IsolationScope) -> Result<StreamId, HookError> {
    let suffix = scope
        .run_id
        .as_str()
        .strip_prefix("run_")
        .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
    StreamId::parse(format!("stream_hooks-{suffix}"))
        .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))
}

fn fold_hook_events(
    schema_set: &SchemaSet,
    events: &[ValidatedEvent],
) -> Result<HookAggregate, HookError> {
    let first = events
        .first()
        .ok_or_else(|| HookError::new(HookErrorKind::AggregateNotFound))?;
    let initialization = first
        .downcast_payload::<HookStreamInitializedPayloadV1>()
        .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?
        .clone();
    if first.variant_id() != "hook-stream-initialized-v1"
        || first.envelope().sequence != "1"
        || initialization.source_run_id != first.envelope().scope.run_id
        || initialization.source_schema_set_ref != *schema_set.reference()
    {
        return Err(HookError::new(HookErrorKind::AggregateCorrupt));
    }
    let history_schema = schema_set
        .schema_ref("hook-projection-hash-view")
        .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
    let mut history_digest = digest_json(
        "hook-history-seed",
        history_schema,
        &serde_json::json!({"algorithm":"hook-history-chain-v1"}),
    )
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
    let mut invocations = BTreeMap::new();
    let mut finalized_points = Vec::new();
    let mut skipped_count = 0_u64;
    let mut late_result_count = 0_u64;
    let mut rejected_count = 0_u64;
    for (index, event) in events.iter().enumerate() {
        let sequence = index + 1;
        if event.envelope().sequence != sequence.to_string()
            || event.schema_set_ref() != schema_set.reference()
        {
            return Err(HookError::new(HookErrorKind::AggregateCorrupt));
        }
        history_digest = digest_json(
            "hook-history-step",
            history_schema,
            &serde_json::json!({
                "previous_digest": history_digest,
                "sequence": event.envelope().sequence,
                "event_id": event.envelope().event_id,
                "payload_digest": event.envelope().payload_digest,
                "variant_id": event.variant_id(),
            }),
        )
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
        if index == 0 {
            continue;
        }
        if let Some(payload) = event.downcast_payload::<HookInvocationReservedPayloadV1>() {
            if payload.invocation_id != payload.pair.invocation_id
                || invocations.contains_key(&payload.invocation_id)
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            invocations.insert(
                payload.invocation_id.clone(),
                HookInvocationProjectionEntryV1 {
                    invocation_id: payload.invocation_id.clone(),
                    key: payload.key.clone(),
                    operation_id: payload.pair.operation_id.clone(),
                    reservation_id: payload.pair.reservation_id.clone(),
                    terminal_state: None,
                    decision_id: None,
                },
            );
        } else if let Some(payload) = event.downcast_payload::<HookInvocationTerminalPayloadV1>() {
            let entry = invocations
                .get_mut(&payload.invocation_id)
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            if entry.terminal_state.is_some()
                || payload.pair.invocation_id != payload.invocation_id
                || entry.operation_id != payload.pair.operation_id
                || entry.reservation_id != payload.pair.reservation_id
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            entry.terminal_state = Some(payload.terminal_state);
            entry.decision_id = Some(payload.decision_id.clone());
        } else if event
            .downcast_payload::<HookPointStartedPayloadV1>()
            .is_some()
        {
        } else if event
            .downcast_payload::<HookInvocationSkippedPayloadV1>()
            .is_some()
        {
            skipped_count += 1;
        } else if let Some(payload) = event.downcast_payload::<HookPointFinalizedPayloadV1>() {
            if finalized_points.contains(&payload.point_id) {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            finalized_points.push(payload.point_id.clone());
        } else if event
            .downcast_payload::<HookLateResultObservedPayloadV1>()
            .is_some()
        {
            late_result_count += 1;
        } else if event
            .downcast_payload::<HookMessageRejectedPayloadV1>()
            .is_some()
        {
            rejected_count += 1;
        } else {
            return Err(HookError::new(HookErrorKind::AggregateCorrupt));
        }
    }
    let last = events.last().expect("non-empty Hook history");
    finalized_points.sort();
    Ok(HookAggregate {
        initialization,
        invocations,
        finalized_points,
        skipped_count,
        late_result_count,
        rejected_count,
        history_digest,
        inclusive_cursor: EventCursor {
            sequence: last.envelope().sequence.clone(),
            event_id: last.envelope().event_id.clone(),
        },
    })
}

fn build_projection(
    store_id: &str,
    scope: &IsolationScope,
    actor: &pareto_protocol::AgentId,
    schema_set: &SchemaSet,
    aggregate: HookAggregate,
) -> Result<HookProjectionV1, HookError> {
    let reducer_revision = RevisionId::parse(HOOK_REDUCER_REVISION)
        .map_err(|_| HookError::new(HookErrorKind::SchemaUnavailable))?;
    let invocations: Vec<_> = aggregate.invocations.into_values().collect();
    let view = HookProjectionHashViewV1 {
        source_store_id: store_id.to_owned(),
        scope: scope.clone(),
        owner_actor: actor.clone(),
        hook_stream_id: hook_stream_id(scope)?,
        inclusive_cursor: aggregate.inclusive_cursor.clone(),
        source_schema_set_ref: schema_set.reference().clone(),
        hook_registry_revision: aggregate.initialization.hook_registry_revision.clone(),
        hook_registry_config_digest: aggregate.initialization.hook_registry_config_digest.clone(),
        reducer_revision: reducer_revision.clone(),
        history_digest: aggregate.history_digest.clone(),
        invocations: invocations.clone(),
        finalized_points: aggregate.finalized_points.clone(),
        skipped_count: aggregate.skipped_count,
        late_result_count: aggregate.late_result_count,
        rejected_count: aggregate.rejected_count,
    };
    let hash_schema = schema_set
        .schema_ref("hook-projection-hash-view")
        .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
    let projection_digest = digest_json(
        "hook-projection",
        hash_schema,
        &serde_json::to_value(&view)
            .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
    )
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
    let projection = HookProjectionV1 {
        source_store_id: view.source_store_id,
        scope: view.scope,
        owner_actor: view.owner_actor,
        hook_stream_id: view.hook_stream_id,
        inclusive_cursor: view.inclusive_cursor,
        source_schema_set_ref: view.source_schema_set_ref,
        hook_registry_revision: view.hook_registry_revision,
        hook_registry_config_digest: view.hook_registry_config_digest,
        reducer_revision: view.reducer_revision,
        history_digest: view.history_digest,
        invocations: view.invocations,
        finalized_points: view.finalized_points,
        skipped_count: view.skipped_count,
        late_result_count: view.late_result_count,
        rejected_count: view.rejected_count,
        projection_digest,
    };
    let bytes = canonical(&projection)
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?
        .into_bytes();
    schema_set
        .parse_record::<HookProjectionV1>(&bytes)
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
    Ok(projection)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[tokio::test]
async fn reserve_pair_atomicity() {
    tests::reserve_pair_atomicity_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn pair_fault_injection() {
    tests::pair_fault_injection_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn terminal_pair_atomicity() {
    tests::terminal_pair_atomicity_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn idempotency() {
    tests::idempotency_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn pair_corruption() {
    tests::pair_corruption_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn authority() {
    tests::authority_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn isolation() {
    tests::isolation_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn budget_reserve() {
    tests::budget_reserve_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn budget_concurrency() {
    tests::budget_concurrency_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn settlement() {
    tests::settlement_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn cancellation_deadline() {
    tests::cancellation_deadline_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn terminal_race() {
    tests::terminal_race_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn model_sequences() {
    tests::model_sequences_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn late_and_duplicate() {
    tests::late_and_duplicate_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn recovery() {
    tests::pair_recovery_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn recorded_replay() {
    tests::recorded_vertical_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn unsupported_modes() {
    tests::unsupported_modes_case().await;
}

#[cfg(test)]
#[test]
fn kind_point_table() {
    tests::kind_point_table_case();
}

#[cfg(test)]
#[test]
fn ordering() {
    tests::ordering_case();
}

#[cfg(test)]
#[test]
fn phase_order_lineage() {
    tests::phase_order_lineage_case();
}

#[cfg(test)]
#[tokio::test]
async fn fold_contract() {
    tests::fold_contract_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn compatibility() {
    tests::compatibility_case().await;
}

#[cfg(test)]
#[test]
fn gate_composition() {
    tests::gate_composition_case();
}

#[cfg(test)]
#[test]
fn default_deny() {
    tests::default_deny_case();
}

#[cfg(test)]
#[test]
fn failure_policy() {
    tests::failure_policy_case();
}

#[cfg(test)]
#[test]
fn observer_non_authority() {
    tests::observer_non_authority_case();
}

#[cfg(test)]
#[test]
fn transform_chain_failure() {
    tests::transform_chain_failure_case();
}

#[cfg(test)]
#[test]
fn transform_protected_fields() {
    tests::transform_protected_fields_case();
}

#[cfg(test)]
#[test]
fn output_security() {
    tests::output_security_case();
}
