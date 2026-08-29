use std::{collections::BTreeMap, sync::Arc};

use pareto_protocol::{
    AgentId, BudgetAmountV1, BudgetDimensionV1, BudgetVectorEntryV1, CallbackId, Digest,
    EventCursor, EventId, ExecutionMode, GateDecisionV1, HookBusinessDecisionV1, HookDecisionId,
    HookExecutionStatusV1, HookId, HookInvocationId, HookInvocationProjectionEntryV1,
    HookInvocationReservedPayloadV1, HookInvocationSkippedPayloadV1,
    HookInvocationTerminalPayloadV1, HookKindV1, HookLateResultObservedPayloadV1,
    HookMessageRejectedPayloadV1, HookPairBindingV1, HookPairKindV1, HookPhaseV1,
    HookPointFinalizedPayloadV1, HookPointStartedPayloadV1, HookPointV1, HookProjectionHashViewV1,
    HookProjectionV1, HookReasonCodeV1, HookRegistrationV1, HookRegistryRevisionV1,
    HookRequestViewV1, HookStreamInitializedPayloadV1, IsolationScope, ObserverResultV1,
    OperationId, OperationInterruptibilityV1, OperationOutcomeV1, OperationReservedPayloadV1,
    OperationSettledPayloadV1, ProtectedProposalHashViewV1, ProtocolLimitsRef, ReservationId,
    ResourceSelectorV1, RevisionId, RunManifest, SchemaSet, StreamId, TaskId, TimeoutKeyV1,
    TransformProposalV1, ValidatedEvent, digest_json,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::{Row, SqliteConnection};

use super::lifecycle::{LifecycleTarget, lifecycle_event, load_established};
use super::runtime_control::{
    ClockSample, FAKE_ADAPTER_REVISION, FAKE_CALLBACK_NAMESPACE, FAKE_CONTRACT_REVISION,
    FAKE_TIMEOUT_POLICY_REVISION, HookControlEventContext, OperationLease,
    ProtectedOperationProposal, RuntimeClock, RuntimeControlErrorKind, RuntimeControlTarget,
    control_event, make_lease, plan_hook_reservation, plan_hook_settlement,
    plan_hook_timeout_settlement, prepare_hook_reservation_event, prepare_hook_settlement_event,
    runtime_control_stream_id, validate_runtime_control_history,
};
use super::{
    AdmittedAppend, AdmittedRead, AppendResult, AtomicPairFault, ErrorKind, EventStore,
    EventStoreError, KernelAuthority, PreparedEvent, SchemaRegistry, append_atomic_pair, canonical,
    check_prepared_idempotency, insert_prepared, validate_row,
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
    scope: &IsolationScope,
    pair: &HookPairBindingV1,
    control: &PreparedEvent,
    hook: &PreparedEvent,
) -> Result<PairPresence, HookError> {
    let user_present = i64::from(scope.user_id.is_some());
    let user_id = scope
        .user_id
        .as_ref()
        .map_or_else(String::new, |value| value.as_str().to_owned());
    let rows = sqlx::query(
        r#"SELECT event_id,event_type FROM (
            SELECT event_id,json_extract(envelope_json,'$.event_type') AS event_type,
                   json_extract(envelope_json,'$.payload.hook_pair.pair_id') AS control_pair_id,
                   json_extract(envelope_json,'$.payload.hook_pair.pair_kind') AS control_pair_kind,
                   json_extract(envelope_json,'$.payload.pair.pair_id') AS hook_pair_id,
                   json_extract(envelope_json,'$.payload.pair.pair_kind') AS hook_pair_kind
            FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=?
              AND run_id=? AND agent_id=?
        ) WHERE control_pair_id=? OR hook_pair_id=?"#,
    )
    .bind(scope.tenant_id.as_str())
    .bind(user_present)
    .bind(user_id)
    .bind(scope.workspace_id.as_str())
    .bind(scope.run_id.as_str())
    .bind(scope.agent_id.as_str())
    .bind(pair.pair_id.as_str())
    .bind(pair.pair_id.as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| HookError::new(HookErrorKind::Store))?;
    if rows.len() == 1 {
        return Err(HookError::new(HookErrorKind::PartialPair));
    }
    if rows.len() > 2 {
        return Err(HookError::new(HookErrorKind::IdempotencyConflict));
    }
    if rows.len() == 2 {
        let mut identities: Vec<(String, String)> = rows
            .iter()
            .map(|row| (row.get::<String, _>(0), row.get::<String, _>(1)))
            .collect();
        identities.sort();
        let mut expected = vec![
            (
                pair.control_event_id.as_str().to_owned(),
                match pair.pair_kind {
                    HookPairKindV1::Reserve => "operation-reserved",
                    HookPairKindV1::Terminal => "operation-settled",
                }
                .to_owned(),
            ),
            (
                pair.hook_event_id.as_str().to_owned(),
                match pair.pair_kind {
                    HookPairKindV1::Reserve => "hook-invocation-reserved",
                    HookPairKindV1::Terminal => "hook-invocation-terminal",
                }
                .to_owned(),
            ),
        ];
        expected.sort();
        if identities != expected {
            return Err(HookError::new(HookErrorKind::IdempotencyConflict));
        }
    }
    let control = check_prepared_idempotency(connection, control)
        .await
        .map_err(map_store_error)?;
    let hook = check_prepared_idempotency(connection, hook)
        .await
        .map_err(map_store_error)?;
    match (control, hook) {
        (None, None) if rows.is_empty() => Ok(PairPresence::Zero),
        (Some(_), Some(_)) if rows.len() == 2 => Ok(PairPresence::Two),
        (None, None) | (Some(_), Some(_)) => {
            Err(HookError::new(HookErrorKind::IdempotencyConflict))
        }
        _ => Err(HookError::new(HookErrorKind::PartialPair)),
    }
}

fn validate_reserve_pair_command(
    target: &HookTarget,
    control_target: &RuntimeControlTarget,
    command: &HookReservePairCommandV1,
) -> Result<(), HookError> {
    let fingerprint = reserve_pair_fingerprint(command)?;
    let prepared = reserve_pair_event_bytes(command)?;
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
    if command.pair.pair_kind != HookPairKindV1::Reserve
        || command.control_sequence != next_sequence(&command.expected_control_cursor)?
        || command.hook_sequence != next_sequence(&command.expected_hook_cursor)?
        || (
            command.prepared_control_event_bytes.clone(),
            command.prepared_hook_event_bytes.clone(),
        ) != prepared
        || command.pair.pair_fingerprint != fingerprint
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
    let prepared = terminal_pair_event_bytes(command)?;
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
    if command.pair.pair_kind != HookPairKindV1::Terminal
        || command.control_sequence != next_sequence(&command.expected_control_cursor)?
        || command.hook_sequence != next_sequence(&command.expected_hook_cursor)?
        || (
            command.prepared_control_event_bytes.clone(),
            command.prepared_hook_event_bytes.clone(),
        ) != prepared
        || command.pair.pair_fingerprint != fingerprint
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

#[derive(Clone)]
struct HookFactCommand<T> {
    expected_cursor: EventCursor,
    event_id: EventId,
    occurred_at: String,
    correlation_id: String,
    event_type: &'static str,
    payload: T,
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

struct OpenPointFold {
    start: HookPointStartedPayloadV1,
    next_ordinal: usize,
    active_invocation: Option<HookInvocationId>,
    current_input_digest: Digest,
    last_transform_output: Option<Digest>,
    last_phase: Option<HookPhaseV1>,
    decisions: Vec<HookDecisionId>,
    skipped: Vec<HookInvocationId>,
    required_gate_total: usize,
    required_gate_allows: usize,
    terminal_outcome: Option<(
        HookBusinessDecisionV1,
        HookExecutionStatusV1,
        HookReasonCodeV1,
    )>,
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
        schema_set: &SchemaSet,
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
        let request_schema = schema_set
            .schema_ref("hook-request-view")
            .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let transform_field_schema = schema_set
            .schema_ref("transform-field-value")
            .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let protected_schema = schema_set
            .schema_ref("protected-proposal-hash-view")
            .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
        if &registry.metadata.revision_id != expected_revision
            || &registry.config_digest != expected_config
            || registry.config_digest != computed_config
            || registry.registrations.iter().any(|registration| {
                let output = schema_set.schema_ref(match registration.kind {
                    HookKindV1::Observer => "observer-result",
                    HookKindV1::Gate => "gate-decision",
                    HookKindV1::Transform => "transform-proposal",
                });
                registration.input_schema_ref != *request_schema
                    || output != Some(&registration.output_schema_ref)
                    || registration
                        .transform_contract
                        .as_ref()
                        .is_some_and(|contract| {
                            contract.field_schema_ref != *transform_field_schema
                                || contract.protected_hash_view_schema_ref != *protected_schema
                        })
            })
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

#[derive(Clone, Debug, PartialEq, Serialize)]
enum UntrustedHookOutput {
    Observer(ObserverResultV1),
    Gate(GateDecisionV1),
    Transform(Box<TransformProposalV1>),
}

#[derive(Clone, Debug, PartialEq)]
struct HookPointEvaluation {
    proposal: TransformProposalV1,
    business_decision: HookBusinessDecisionV1,
    execution_status: HookExecutionStatusV1,
    reason_code: HookReasonCodeV1,
    observer_results: Vec<(HookId, ObserverResultV1)>,
}

fn evaluate_point(
    schema_set: &SchemaSet,
    registry: &ResolvedHookRegistry,
    point: HookPointV1,
    initial: &TransformProposalV1,
    protected: &ProtectedProposalHashViewV1,
    outputs: &BTreeMap<HookId, Result<UntrustedHookOutput, HookReasonCodeV1>>,
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
    let mut reason_code = HookReasonCodeV1::Completed;
    for registration in registry.ordered_for_point(point) {
        let Some(output) = outputs.get(&registration.hook_id) else {
            if registration.kind == HookKindV1::Transform {
                execution_status = HookExecutionStatusV1::TransformFailed;
                business_decision = HookBusinessDecisionV1::Deny;
                reason_code = HookReasonCodeV1::TransformMissing;
                break;
            }
            if registration.kind == HookKindV1::Gate && registration.required == Some(true) {
                required_gate_count += 1;
                gate_denied = true;
                reason_code = HookReasonCodeV1::RequiredGateMissing;
            }
            continue;
        };
        let output = match output {
            Ok(output) if output_is_bounded(schema_set, registration, output) => output,
            _ if registration.kind == HookKindV1::Transform => {
                execution_status = HookExecutionStatusV1::TransformFailed;
                business_decision = HookBusinessDecisionV1::Deny;
                reason_code = HookReasonCodeV1::TransformOutputInvalid;
                break;
            }
            _ if registration.kind == HookKindV1::Gate => {
                if registration.required == Some(true) {
                    required_gate_count += 1;
                }
                gate_denied = true;
                reason_code = HookReasonCodeV1::GateOutputInvalid;
                break;
            }
            _ => {
                if registration.observer_failure_policy
                    == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed)
                {
                    execution_status = HookExecutionStatusV1::ObserverFailed;
                    reason_code = HookReasonCodeV1::ObserverFailedClosed;
                    break;
                }
                continue;
            }
        };
        match (registration.kind, output) {
            (HookKindV1::Transform, UntrustedHookOutput::Transform(candidate)) => {
                let contract = registration.transform_contract.as_ref();
                if candidate.proposal_id != proposal.proposal_id
                    || candidate.schema_ref != proposal.schema_ref
                    || contract.is_none_or(|contract| {
                        !transform_changes_allowed(
                            schema_set, &proposal, candidate, protected, contract,
                        )
                    })
                {
                    execution_status = HookExecutionStatusV1::TransformFailed;
                    business_decision = HookBusinessDecisionV1::Deny;
                    reason_code = HookReasonCodeV1::TransformProtectedField;
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
                        reason_code = HookReasonCodeV1::GateDenied;
                        break;
                    }
                    GateDecisionV1::Abstain {} if registration.required == Some(true) => {
                        gate_denied = true;
                        reason_code = HookReasonCodeV1::RequiredGateAbstained;
                        break;
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
                    reason_code = HookReasonCodeV1::ObserverFailedClosed;
                }
                observer_results.push((registration.hook_id.clone(), result.clone()));
                if execution_status == HookExecutionStatusV1::ObserverFailed {
                    break;
                }
            }
            (HookKindV1::Transform, _) | (HookKindV1::Gate, _) | (HookKindV1::Observer, _) => {
                if registration.kind == HookKindV1::Transform {
                    execution_status = HookExecutionStatusV1::TransformFailed;
                    business_decision = HookBusinessDecisionV1::Deny;
                    reason_code = HookReasonCodeV1::HookKindMismatch;
                    break;
                }
                gate_denied |= registration.kind == HookKindV1::Gate;
                if registration.kind == HookKindV1::Gate {
                    reason_code = HookReasonCodeV1::HookKindMismatch;
                    break;
                }
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
            reason_code = HookReasonCodeV1::RequiredGateEmpty;
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
            | (HookKindV1::Transform, UntrustedHookOutput::Transform(_))
    );
    if !matches_kind {
        return false;
    }
    let value = match output {
        UntrustedHookOutput::Observer(value) => serde_json::to_value(value),
        UntrustedHookOutput::Gate(value) => serde_json::to_value(value),
        UntrustedHookOutput::Transform(proposal) => serde_json::to_value(proposal),
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
    schema_set: &SchemaSet,
    before: &TransformProposalV1,
    after: &TransformProposalV1,
    protected: &ProtectedProposalHashViewV1,
    contract: &pareto_protocol::TransformContractV1,
) -> bool {
    if protected_view_for_candidate(after, protected, &contract.allowed_fields).as_ref()
        != Some(protected)
    {
        return false;
    }
    changed_json_pointers(&before.fields, &after.fields, "")
        .iter()
        .all(|pointer| {
            contract.allowed_fields.binary_search(pointer).is_ok()
                && after.fields.pointer(pointer).is_some_and(|value| {
                    schema_set
                        .validate_value_against(&contract.field_schema_ref, value)
                        .is_ok()
                })
        })
}

fn changed_json_pointers(
    before: &serde_json::Value,
    after: &serde_json::Value,
    prefix: &str,
) -> Vec<String> {
    if before == after {
        return Vec::new();
    }
    match (before.as_object(), after.as_object()) {
        (Some(before), Some(after)) => {
            let mut keys: std::collections::BTreeSet<&str> =
                before.keys().map(String::as_str).collect();
            keys.extend(after.keys().map(String::as_str));
            keys.into_iter()
                .flat_map(|key| {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    let pointer = format!("{prefix}/{escaped}");
                    match (before.get(key), after.get(key)) {
                        (Some(left), Some(right)) => changed_json_pointers(left, right, &pointer),
                        _ => vec![pointer],
                    }
                })
                .collect()
        }
        _ => match (before.as_array(), after.as_array()) {
            (Some(before), Some(after)) => (0..before.len().max(after.len()))
                .flat_map(|index| {
                    let pointer = format!("{prefix}/{index}");
                    match (before.get(index), after.get(index)) {
                        (Some(left), Some(right)) => changed_json_pointers(left, right, &pointer),
                        _ => vec![pointer],
                    }
                })
                .collect(),
            _ => vec![prefix.to_owned()],
        },
    }
}

fn protected_view_for_candidate(
    candidate: &TransformProposalV1,
    fixed: &ProtectedProposalHashViewV1,
    allowed: &[String],
) -> Option<ProtectedProposalHashViewV1> {
    let mut protected_fields = candidate.fields.clone();
    for pointer in allowed {
        remove_json_pointer(&mut protected_fields, pointer)?;
    }
    Some(ProtectedProposalHashViewV1 {
        scope: fixed.scope.clone(),
        proposal_id: candidate.proposal_id.clone(),
        schema_set_ref: fixed.schema_set_ref.clone(),
        hook_registry_revision: fixed.hook_registry_revision.clone(),
        authority_digest: fixed.authority_digest.clone(),
        unknown_fields_digest: pair_digest("hook-protected-fields-v1", &protected_fields).ok()?,
    })
}

fn remove_json_pointer(value: &mut serde_json::Value, pointer: &str) -> Option<()> {
    let (parent_pointer, leaf) = pointer.rsplit_once('/')?;
    let leaf = leaf.replace("~1", "/").replace("~0", "~");
    let parent = if parent_pointer.is_empty() {
        value
    } else {
        value.pointer_mut(parent_pointer)?
    };
    if let Some(object) = parent.as_object_mut() {
        object.remove(&leaf)?;
        Some(())
    } else {
        let index = leaf.parse::<usize>().ok()?;
        let array = parent.as_array_mut()?;
        (index < array.len()).then(|| {
            array.remove(index);
        })
    }
}

fn proposal_digest(
    schema_set: &SchemaSet,
    proposal: &TransformProposalV1,
) -> Result<Digest, HookError> {
    let schema = schema_set
        .schema_ref("transform-proposal")
        .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
    if &proposal.schema_ref != schema {
        return Err(HookError::new(HookErrorKind::SchemaUnavailable));
    }
    digest_json(
        "hook-transform-proposal-v1",
        schema,
        &serde_json::to_value(proposal)
            .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
    )
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn kernel_protected_view(
    schema_set: &SchemaSet,
    registry: &ResolvedHookRegistry,
    scope: &IsolationScope,
    source_cursor: &EventCursor,
    proposal: &TransformProposalV1,
    allowed: &[String],
) -> Result<ProtectedProposalHashViewV1, HookError> {
    let authority_digest = pair_digest(
        "hook-protected-authority-v1",
        &serde_json::json!({
            "scope": scope,
            "source_cursor": source_cursor,
            "schema_set": schema_set.reference(),
            "registry_revision": registry.revision,
            "registry_config_digest": registry.config_digest,
            "proposal_id": proposal.proposal_id,
        }),
    )?;
    let seed = ProtectedProposalHashViewV1 {
        scope: scope.clone(),
        proposal_id: proposal.proposal_id.clone(),
        schema_set_ref: schema_set.reference().clone(),
        hook_registry_revision: registry.revision.clone(),
        authority_digest,
        unknown_fields_digest: Digest::parse(format!("sha256:{}", "0".repeat(64)))
            .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
    };
    protected_view_for_candidate(proposal, &seed, allowed)
        .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))
}

trait FakeHookHandler: Send + Sync {
    fn invoke(
        &self,
        lease: &HookInvocationLease,
        request: &HookRequestViewV1,
    ) -> Result<UntrustedHookOutput, HookReasonCodeV1>;
}

struct FakeHookHandlerBinding {
    hook_revision: RevisionId,
    compatibility_digest: Digest,
    handler: Arc<dyn FakeHookHandler>,
}

#[derive(Default)]
struct FakeHookHandlers {
    bindings: BTreeMap<HookId, FakeHookHandlerBinding>,
}

impl FakeHookHandlers {
    fn resolve(
        &self,
        registration: &HookRegistrationV1,
    ) -> Result<&dyn FakeHookHandler, HookError> {
        let binding = self
            .bindings
            .get(&registration.hook_id)
            .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
        if binding.hook_revision != registration.hook_revision
            || binding.compatibility_digest != registration.handler_compatibility_digest
        {
            return Err(HookError::new(HookErrorKind::ManifestInvalid));
        }
        Ok(binding.handler.as_ref())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvocationLineage {
    hook_id: HookId,
    phase: HookPhaseV1,
    input_digest: Digest,
    predecessor_output_digest: Option<Digest>,
}

#[derive(Clone)]
struct ExecuteHookPointCommand {
    point: HookPointV1,
    task_id: Option<TaskId>,
    source_cursor: EventCursor,
    proposal: TransformProposalV1,
    occurred_at: String,
    correlation_id: String,
    absolute_deadline_utc: String,
    attempt: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct ExecuteHookPointResult {
    point_id: HookDecisionId,
    proposal: TransformProposalV1,
    business_decision: HookBusinessDecisionV1,
    execution_status: HookExecutionStatusV1,
    reason_code: HookReasonCodeV1,
    projection: HookProjectionV1,
}

fn stable_suffix<T: Serialize>(domain: &str, value: &T) -> Result<String, HookError> {
    Ok(pair_digest(domain, value)?
        .as_str()
        .strip_prefix("sha256:")
        .expect("validated digest")
        .chars()
        .take(32)
        .collect())
}

fn point_id_for(
    scope: &IsolationScope,
    command: &ExecuteHookPointCommand,
) -> Result<HookDecisionId, HookError> {
    HookDecisionId::parse(format!(
        "decision_point-{}",
        stable_suffix(
            "hook-point-id-v1",
            &serde_json::json!({
                "scope": scope,
                "point": command.point,
                "source": command.source_cursor,
                "proposal": command.proposal.proposal_id,
                "attempt": command.attempt,
            }),
        )?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn invocation_id_for(
    point_id: &HookDecisionId,
    registration: &HookRegistrationV1,
    ordinal: u32,
) -> Result<HookInvocationId, HookError> {
    HookInvocationId::parse(format!(
        "invocation_{}",
        stable_suffix(
            "hook-invocation-id-v1",
            &serde_json::json!({
                "point_id": point_id,
                "hook_id": registration.hook_id,
                "hook_revision": registration.hook_revision,
                "ordinal": ordinal,
            }),
        )?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn event_id_for(domain: &str, identity: &impl Serialize) -> Result<EventId, HookError> {
    EventId::parse(format!("event_{}", stable_suffix(domain, identity)?))
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn operation_id_for(invocation_id: &HookInvocationId) -> Result<OperationId, HookError> {
    OperationId::parse(format!(
        "operation_{}",
        stable_suffix("hook-operation-id-v1", invocation_id)?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn reservation_id_for(invocation_id: &HookInvocationId) -> Result<ReservationId, HookError> {
    ReservationId::parse(format!(
        "reservation_{}",
        stable_suffix("hook-reservation-id-v1", invocation_id)?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn pair_id_for(
    kind: HookPairKindV1,
    invocation_id: &HookInvocationId,
) -> Result<pareto_protocol::HookPairId, HookError> {
    pareto_protocol::HookPairId::parse(format!(
        "pair_{}",
        stable_suffix(
            "hook-pair-id-v1",
            &serde_json::json!({"kind":kind,"invocation_id":invocation_id}),
        )?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn decision_id_for(invocation_id: &HookInvocationId) -> Result<HookDecisionId, HookError> {
    HookDecisionId::parse(format!(
        "decision_{}",
        stable_suffix("hook-component-decision-v1", invocation_id)?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn callback_id_for(invocation_id: &HookInvocationId) -> Result<CallbackId, HookError> {
    CallbackId::parse(format!(
        "{FAKE_CALLBACK_NAMESPACE}{}",
        stable_suffix("hook-callback-id-v1", invocation_id)?
    ))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn append_cursor(result: &AppendResult) -> EventCursor {
    match result {
        AppendResult::Appended { event_id, sequence }
        | AppendResult::AlreadyCommitted { event_id, sequence } => EventCursor {
            sequence: sequence.to_string(),
            event_id: event_id.clone(),
        },
    }
}

#[derive(Clone, Serialize)]
struct HookReservePairCommandV1 {
    scope: IsolationScope,
    owner: AgentId,
    control_stream_id: StreamId,
    hook_stream_id: StreamId,
    expected_control_cursor: EventCursor,
    expected_hook_cursor: EventCursor,
    control_sequence: i64,
    hook_sequence: i64,
    prepared_control_event_bytes: String,
    prepared_hook_event_bytes: String,
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
    control_sequence: i64,
    hook_sequence: i64,
    prepared_control_event_bytes: String,
    prepared_hook_event_bytes: String,
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
    command.control_sequence = next_sequence(&command.expected_control_cursor)?;
    command.hook_sequence = next_sequence(&command.expected_hook_cursor)?;
    let fingerprint = reserve_pair_fingerprint(&command)?;
    command.pair.pair_fingerprint = fingerprint.clone();
    command.control_payload.hook_pair = Some(command.pair.clone());
    command.hook_payload.pair = command.pair.clone();
    (
        command.prepared_control_event_bytes,
        command.prepared_hook_event_bytes,
    ) = reserve_pair_event_bytes(&command)?;
    Ok(command)
}

fn seal_terminal_pair_command(
    mut command: HookTerminalPairCommandV1,
) -> Result<HookTerminalPairCommandV1, HookError> {
    command.control_sequence = next_sequence(&command.expected_control_cursor)?;
    command.hook_sequence = next_sequence(&command.expected_hook_cursor)?;
    let fingerprint = terminal_pair_fingerprint(&command)?;
    command.pair.pair_fingerprint = fingerprint.clone();
    command.control_payload.hook_pair = Some(command.pair.clone());
    command.hook_payload.pair = command.pair.clone();
    (
        command.prepared_control_event_bytes,
        command.prepared_hook_event_bytes,
    ) = terminal_pair_event_bytes(&command)?;
    Ok(command)
}

fn reserve_pair_fingerprint(command: &HookReservePairCommandV1) -> Result<Digest, HookError> {
    let mut normalized = command.clone();
    normalized.prepared_control_event_bytes.clear();
    normalized.prepared_hook_event_bytes.clear();
    clear_pair_fingerprint(&mut normalized.pair)?;
    normalized.control_payload.hook_pair = Some(normalized.pair.clone());
    normalized.hook_payload.pair = normalized.pair.clone();
    pair_digest("hook-reserve-pair-command-v1", &normalized)
}

fn terminal_pair_fingerprint(command: &HookTerminalPairCommandV1) -> Result<Digest, HookError> {
    let mut normalized = command.clone();
    normalized.prepared_control_event_bytes.clear();
    normalized.prepared_hook_event_bytes.clear();
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

#[allow(clippy::too_many_arguments)]
fn pair_event_bytes<T: Serialize>(
    scope: &IsolationScope,
    owner: &AgentId,
    stream_id: &StreamId,
    sequence: i64,
    event_id: &EventId,
    occurred_at: &str,
    correlation_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<String, HookError> {
    canonical(&serde_json::json!({
        "scope": scope,
        "owner": owner,
        "stream_id": stream_id,
        "sequence": sequence.to_string(),
        "event_id": event_id,
        "occurred_at": occurred_at,
        "correlation_id": correlation_id,
        "event_type": event_type,
        "event_major": 1,
        "event_minor": 0,
        "payload": payload,
    }))
    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))
}

fn reserve_pair_event_bytes(
    command: &HookReservePairCommandV1,
) -> Result<(String, String), HookError> {
    Ok((
        pair_event_bytes(
            &command.scope,
            &command.owner,
            &command.control_stream_id,
            command.control_sequence,
            &command.pair.control_event_id,
            &command.occurred_at,
            &command.correlation_id,
            "operation-reserved",
            &command.control_payload,
        )?,
        pair_event_bytes(
            &command.scope,
            &command.owner,
            &command.hook_stream_id,
            command.hook_sequence,
            &command.pair.hook_event_id,
            &command.occurred_at,
            &command.correlation_id,
            "hook-invocation-reserved",
            &command.hook_payload,
        )?,
    ))
}

fn terminal_pair_event_bytes(
    command: &HookTerminalPairCommandV1,
) -> Result<(String, String), HookError> {
    Ok((
        pair_event_bytes(
            &command.scope,
            &command.owner,
            &command.control_stream_id,
            command.control_sequence,
            &command.pair.control_event_id,
            &command.occurred_at,
            &command.correlation_id,
            "operation-settled",
            &command.control_payload,
        )?,
        pair_event_bytes(
            &command.scope,
            &command.owner,
            &command.hook_stream_id,
            command.hook_sequence,
            &command.pair.hook_event_id,
            &command.occurred_at,
            &command.correlation_id,
            "hook-invocation-terminal",
            &command.hook_payload,
        )?,
    ))
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

    async fn append_hook_fact<T: Serialize>(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        hook_registry: Option<&ResolvedHookRegistry>,
        command: &HookFactCommand<T>,
    ) -> Result<AppendResult, HookError> {
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
        let stream_id = hook_stream_id(&target.scope)?;
        let event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &target.scope,
            &target.actor,
            &stream_id,
            &command.event_id,
            next_sequence(&command.expected_cursor)?,
            &command.occurred_at,
            &command.correlation_id,
            command.event_type,
            &command.payload,
        )
        .map_err(|_| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits)
            .map_err(map_store_error)?;
        if let Some(existing) = check_prepared_idempotency(&mut transaction, &prepared)
            .await
            .map_err(map_store_error)?
        {
            transaction
                .commit()
                .await
                .map_err(|_| HookError::new(HookErrorKind::Store))?;
            return Ok(existing);
        }
        let events = read_hook_events_in_transaction(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
        )
        .await?;
        let aggregate = fold_hook_events(&lifecycle.schema_set, &events, None)?;
        if aggregate.inclusive_cursor != command.expected_cursor {
            return Err(HookError::new(HookErrorKind::IdempotencyConflict));
        }
        let result = insert_prepared(&mut transaction, &prepared)
            .await
            .map_err(map_store_error)?;
        let admitted_events = read_hook_events_in_transaction(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
        )
        .await?;
        fold_hook_events(&lifecycle.schema_set, &admitted_events, hook_registry)?;
        transaction
            .commit()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_hook_point<C: RuntimeClock>(
        &self,
        schema_registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        registry_revision: &HookRegistryRevisionV1,
        handlers: &FakeHookHandlers,
        command: &ExecuteHookPointCommand,
        clock: &C,
    ) -> Result<ExecuteHookPointResult, HookError> {
        if target.scope != control_target.scope
            || target.actor != control_target.principal
            || command.attempt == 0
        {
            return Err(HookError::new(HookErrorKind::Unauthorized));
        }
        let (schema_set, limits, manifest) = self.hook_source(schema_registry, target).await?;
        if !matches!(manifest.execution_mode, ExecutionMode::Live {}) {
            return Err(HookError::new(HookErrorKind::UnsupportedMode));
        }
        let resolved = ResolvedHookRegistry::resolve(&manifest, registry_revision, &schema_set)?;
        let ordered = resolved.ordered_for_point(command.point);
        if ordered
            .iter()
            .any(|registration| handlers.resolve(registration).is_err())
        {
            return Err(HookError::new(HookErrorKind::ManifestInvalid));
        }
        let initial_digest = proposal_digest(&schema_set, &command.proposal)?;
        let request_schema = schema_set
            .schema_ref("hook-request-view")
            .ok_or_else(|| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let current_events = self
            .read_hook_events(target, schema_set.clone(), limits.clone())
            .await?;
        let aggregate = fold_hook_events(&schema_set, &current_events, Some(&resolved))?;
        let point_id = point_id_for(&target.scope, command)?;
        let ordered_invocations: Vec<_> = ordered
            .iter()
            .enumerate()
            .map(|(ordinal, registration)| {
                invocation_id_for(&point_id, registration, ordinal as u32)
            })
            .collect::<Result<_, _>>()?;
        let start = HookPointStartedPayloadV1 {
            point_id: point_id.clone(),
            hook_point: command.point,
            subject_proposal_id: command.proposal.proposal_id.clone(),
            source_cursor: command.source_cursor.clone(),
            initial_input_digest: initial_digest.clone(),
            ordered_invocations: ordered_invocations.clone(),
        };
        let start_result = self
            .append_hook_fact(
                schema_registry,
                target,
                Some(&resolved),
                &HookFactCommand {
                    expected_cursor: aggregate.inclusive_cursor,
                    event_id: event_id_for("hook-point-start-event-v1", &point_id)?,
                    occurred_at: command.occurred_at.clone(),
                    correlation_id: command.correlation_id.clone(),
                    event_type: "hook-point-started",
                    payload: start,
                },
            )
            .await?;
        let mut hook_cursor = append_cursor(&start_result);
        let mut proposal = command.proposal.clone();
        let mut input_digest = initial_digest.clone();
        let gate_bearing = matches!(
            command.point,
            HookPointV1::BeforeProposalAdmission | HookPointV1::BeforeAuthoritativeCommit
        );
        let required_gate_total = ordered
            .iter()
            .filter(|registration| {
                registration.kind == HookKindV1::Gate && registration.required == Some(true)
            })
            .count();
        let mut required_gate_allows = 0_usize;
        let mut business_decision = if gate_bearing {
            HookBusinessDecisionV1::Allow
        } else {
            HookBusinessDecisionV1::ObserveOnly
        };
        let mut execution_status = HookExecutionStatusV1::Completed;
        let mut final_reason = HookReasonCodeV1::Completed;
        let mut stop_reason: Option<HookReasonCodeV1> = None;
        let mut decisions = Vec::new();
        let mut skipped = Vec::new();

        for (ordinal, registration) in ordered.iter().enumerate() {
            let phase = phase_for(command.point, registration.kind)
                .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
            if stop_reason.is_none() && phase == HookPhaseV1::Gate && required_gate_total == 0 {
                business_decision = HookBusinessDecisionV1::Deny;
                execution_status = HookExecutionStatusV1::GateDenied;
                final_reason = HookReasonCodeV1::RequiredGateEmpty;
                stop_reason = Some(HookReasonCodeV1::SkippedAfterGateDenial);
            }
            let invocation_id = ordered_invocations[ordinal].clone();
            if let Some(reason_code) = stop_reason {
                let skipped_result = self
                    .append_hook_fact(
                        schema_registry,
                        target,
                        Some(&resolved),
                        &HookFactCommand {
                            expected_cursor: hook_cursor,
                            event_id: event_id_for(
                                "hook-invocation-skipped-event-v1",
                                &invocation_id,
                            )?,
                            occurred_at: command.occurred_at.clone(),
                            correlation_id: command.correlation_id.clone(),
                            event_type: "hook-invocation-skipped",
                            payload: HookInvocationSkippedPayloadV1 {
                                invocation_id: invocation_id.clone(),
                                hook_point: command.point,
                                phase,
                                reason_code,
                                input_digest: input_digest.clone(),
                            },
                        },
                    )
                    .await?;
                hook_cursor = append_cursor(&skipped_result);
                skipped.push(invocation_id);
                continue;
            }

            if registration.resource_contract_revision
                != RevisionId::parse(FAKE_CONTRACT_REVISION)
                    .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?
            {
                return Err(HookError::new(HookErrorKind::ManifestInvalid));
            }
            let operation_id = operation_id_for(&invocation_id)?;
            let reservation_id = reservation_id_for(&invocation_id)?;
            let reserve_pair_id = pair_id_for(HookPairKindV1::Reserve, &invocation_id)?;
            let reserve_control_event =
                event_id_for("hook-reserve-control-event-v1", &reserve_pair_id)?;
            let reserve_hook_event = event_id_for("hook-reserve-hook-event-v1", &reserve_pair_id)?;
            let reserve_sample = clock.sample();
            let protected_operation = ProtectedOperationProposal {
                event_id: reserve_control_event.clone(),
                denied_event_id: event_id_for("hook-reserve-denied-event-v1", &reserve_pair_id)?,
                occurred_at: reserve_sample.canonical_utc.clone(),
                correlation_id: command.correlation_id.clone(),
                operation_id: operation_id.clone(),
                reservation_id: reservation_id.clone(),
                task_id: command.task_id.clone(),
                resource: ResourceSelectorV1 {
                    kind: "fake".to_owned(),
                    id: Some("fixture".to_owned()),
                },
                operation: "invoke".to_owned(),
                adapter_revision: RevisionId::parse(FAKE_ADAPTER_REVISION)
                    .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?,
                requested_usage: vec![BudgetVectorEntryV1 {
                    dimension: BudgetDimensionV1::Tokens,
                    amount: BudgetAmountV1::new(1),
                }],
                callback_namespace: FAKE_CALLBACK_NAMESPACE.to_owned(),
                interruptibility: OperationInterruptibilityV1::Cooperative,
                absolute_deadline_utc: command.absolute_deadline_utc.clone(),
                timeout_policy_revision: RevisionId::parse(FAKE_TIMEOUT_POLICY_REVISION)
                    .map_err(|_| HookError::new(HookErrorKind::ManifestInvalid))?,
            };
            let mut control_connection = self
                .pool
                .acquire()
                .await
                .map_err(|_| HookError::new(HookErrorKind::Store))?;
            let planned = plan_hook_reservation(
                &mut control_connection,
                schema_registry,
                control_target,
                &protected_operation,
                &reserve_sample,
            )
            .await
            .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
            if planned.payload.operation_contract_revision
                != registration.resource_contract_revision
            {
                return Err(HookError::new(HookErrorKind::ManifestInvalid));
            }
            let reserve_pair = HookPairBindingV1 {
                pair_id: reserve_pair_id,
                pair_kind: HookPairKindV1::Reserve,
                pair_fingerprint: Digest::parse(format!("sha256:{}", "0".repeat(64)))
                    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
                control_event_id: reserve_control_event,
                hook_event_id: reserve_hook_event,
                operation_id: operation_id.clone(),
                reservation_id: reservation_id.clone(),
                invocation_id: invocation_id.clone(),
            };
            let key = pareto_protocol::HookInvocationKeyV1 {
                scope: target.scope.clone(),
                task_id: command.task_id.clone(),
                hook_point: command.point,
                phase,
                hook_id: registration.hook_id.clone(),
                hook_revision: registration.hook_revision.clone(),
                subject_proposal_id: command.proposal.proposal_id.clone(),
                ordinal: ordinal as u32,
                source_cursor: command.source_cursor.clone(),
                input_digest: input_digest.clone(),
                predecessor_output_digest: (phase == HookPhaseV1::Transform && ordinal > 0)
                    .then_some(input_digest.clone()),
                attempt: command.attempt,
            };
            let reserve_command = seal_reserve_pair_command(HookReservePairCommandV1 {
                scope: target.scope.clone(),
                owner: target.actor.clone(),
                control_stream_id: runtime_control_stream_id(&target.scope)
                    .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?,
                hook_stream_id: hook_stream_id(&target.scope)?,
                expected_control_cursor: planned.expected_cursor,
                expected_hook_cursor: hook_cursor,
                control_sequence: 0,
                hook_sequence: 0,
                prepared_control_event_bytes: String::new(),
                prepared_hook_event_bytes: String::new(),
                pair: reserve_pair.clone(),
                occurred_at: reserve_sample.canonical_utc.clone(),
                correlation_id: command.correlation_id.clone(),
                hook_payload: HookInvocationReservedPayloadV1 {
                    invocation_id: invocation_id.clone(),
                    key,
                    pair: reserve_pair,
                    reserved_usage: planned.payload.trusted_reservation.clone(),
                },
                control_payload: planned.payload,
                clock: reserve_sample,
            })?;
            let reserve_result = self
                .append_hook_reserve_pair_with_registry(
                    schema_registry,
                    target,
                    control_target,
                    Some(&resolved),
                    &reserve_command,
                    AtomicPairFault::None,
                )
                .await?;
            hook_cursor = append_cursor(&reserve_result.hook);

            let fixed_business_decision =
                (phase == HookPhaseV1::Observer).then_some(business_decision);
            let request = HookRequestViewV1 {
                hook_point: command.point,
                phase,
                input_digest: input_digest.clone(),
                proposal: proposal.clone(),
                fixed_business_decision,
            };
            let request_value = serde_json::to_value(&request)
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let request_bytes = canonical(&request_value)
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
            if request_bytes.len() > registration.limits.max_input_bytes as usize
                || schema_set
                    .validate_value_against(request_schema, &request_value)
                    .is_err()
            {
                return Err(HookError::new(HookErrorKind::SchemaUnavailable));
            }
            let lease = HookInvocationLease {
                invocation_id: invocation_id.clone(),
                hook_id: registration.hook_id.clone(),
                input_digest: input_digest.clone(),
                scope: target.scope.clone(),
                narrowed: true,
            };
            let handler_result = handlers.resolve(registration)?.invoke(&lease, &request);
            let mut terminal_state = pareto_protocol::HookInvocationTerminalStateV1::Succeeded;
            let mut terminal_reason = match registration.kind {
                HookKindV1::Transform => HookReasonCodeV1::Transformed,
                HookKindV1::Gate => HookReasonCodeV1::Allowed,
                HookKindV1::Observer => HookReasonCodeV1::Observed,
            };
            let mut gate_decision = None;
            let mut observer_result = None;
            let mut output_digest = None;
            let mut late_result_digest = None;
            let mut message_rejected = false;
            let output = match handler_result {
                Ok(output) if output_is_bounded(&schema_set, registration, &output) => Some(output),
                Ok(_) => {
                    message_rejected = true;
                    terminal_state = pareto_protocol::HookInvocationTerminalStateV1::Failed;
                    terminal_reason = match registration.kind {
                        HookKindV1::Transform => HookReasonCodeV1::TransformOutputInvalid,
                        HookKindV1::Gate => HookReasonCodeV1::GateOutputInvalid,
                        HookKindV1::Observer => HookReasonCodeV1::HandlerFailed,
                    };
                    None
                }
                Err(reason) => {
                    terminal_state = pareto_protocol::HookInvocationTerminalStateV1::Failed;
                    terminal_reason = reason;
                    None
                }
            };
            if let Some(output) = output {
                output_digest = Some(pair_digest("hook-validated-output-v1", &output)?);
                match (registration.kind, output) {
                    (HookKindV1::Transform, UntrustedHookOutput::Transform(candidate)) => {
                        let contract = registration
                            .transform_contract
                            .as_ref()
                            .ok_or_else(|| HookError::new(HookErrorKind::ManifestInvalid))?;
                        let protected = kernel_protected_view(
                            &schema_set,
                            &resolved,
                            &target.scope,
                            &command.source_cursor,
                            &proposal,
                            &contract.allowed_fields,
                        )?;
                        if candidate.proposal_id != proposal.proposal_id
                            || candidate.schema_ref != proposal.schema_ref
                            || !transform_changes_allowed(
                                &schema_set,
                                &proposal,
                                &candidate,
                                &protected,
                                contract,
                            )
                        {
                            terminal_state = pareto_protocol::HookInvocationTerminalStateV1::Failed;
                            terminal_reason = HookReasonCodeV1::TransformProtectedField;
                            output_digest = None;
                        } else {
                            proposal = *candidate;
                            input_digest = proposal_digest(&schema_set, &proposal)?;
                            output_digest = Some(input_digest.clone());
                        }
                    }
                    (HookKindV1::Gate, UntrustedHookOutput::Gate(decision)) => {
                        if registration.required == Some(true)
                            && matches!(decision, GateDecisionV1::Allow {})
                        {
                            required_gate_allows += 1;
                        }
                        match &decision {
                            GateDecisionV1::Deny { .. } => {
                                terminal_reason = HookReasonCodeV1::GateDenied;
                                business_decision = HookBusinessDecisionV1::Deny;
                                execution_status = HookExecutionStatusV1::GateDenied;
                                final_reason = HookReasonCodeV1::GateDenied;
                                stop_reason = Some(HookReasonCodeV1::SkippedAfterGateDenial);
                            }
                            GateDecisionV1::Abstain {} if registration.required == Some(true) => {
                                terminal_reason = HookReasonCodeV1::RequiredGateAbstained;
                                business_decision = HookBusinessDecisionV1::Deny;
                                execution_status = HookExecutionStatusV1::GateDenied;
                                final_reason = HookReasonCodeV1::RequiredGateAbstained;
                                stop_reason = Some(HookReasonCodeV1::SkippedAfterGateDenial);
                            }
                            _ => {}
                        }
                        gate_decision = Some(decision);
                    }
                    (HookKindV1::Observer, UntrustedHookOutput::Observer(result)) => {
                        if matches!(result, ObserverResultV1::Failure { .. })
                            && registration.observer_failure_policy
                                == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed)
                        {
                            terminal_reason = HookReasonCodeV1::ObserverFailedClosed;
                            execution_status = HookExecutionStatusV1::ObserverFailed;
                            final_reason = HookReasonCodeV1::ObserverFailedClosed;
                            stop_reason = Some(HookReasonCodeV1::SkippedAfterObserverFailure);
                        }
                        observer_result = Some(result);
                    }
                    _ => {
                        terminal_state = pareto_protocol::HookInvocationTerminalStateV1::Failed;
                        terminal_reason = HookReasonCodeV1::HookKindMismatch;
                        output_digest = None;
                    }
                }
            }
            if terminal_state == pareto_protocol::HookInvocationTerminalStateV1::Failed {
                match registration.kind {
                    HookKindV1::Transform => {
                        proposal = command.proposal.clone();
                        input_digest = initial_digest.clone();
                        business_decision = HookBusinessDecisionV1::Deny;
                        execution_status = HookExecutionStatusV1::TransformFailed;
                        final_reason = terminal_reason;
                        stop_reason = Some(HookReasonCodeV1::SkippedAfterTransformFailure);
                    }
                    HookKindV1::Gate => {
                        business_decision = HookBusinessDecisionV1::Deny;
                        execution_status = HookExecutionStatusV1::GateDenied;
                        final_reason = terminal_reason;
                        stop_reason = Some(HookReasonCodeV1::SkippedAfterGateDenial);
                    }
                    HookKindV1::Observer
                        if registration.observer_failure_policy
                            == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed) =>
                    {
                        execution_status = HookExecutionStatusV1::ObserverFailed;
                        final_reason = HookReasonCodeV1::ObserverFailedClosed;
                        stop_reason = Some(HookReasonCodeV1::SkippedAfterObserverFailure);
                    }
                    HookKindV1::Observer => {}
                }
            }
            let terminal_pair_id = pair_id_for(HookPairKindV1::Terminal, &invocation_id)?;
            let live_terminal_control_event =
                event_id_for("hook-terminal-control-event-v1", &terminal_pair_id)?;
            let terminal_hook_event =
                event_id_for("hook-terminal-hook-event-v1", &terminal_pair_id)?;
            let terminal_sample = clock.sample();
            let mut control_connection = self
                .pool
                .acquire()
                .await
                .map_err(|_| HookError::new(HookErrorKind::Store))?;
            let live_plan = plan_hook_settlement(
                &mut control_connection,
                schema_registry,
                control_target,
                &reserve_result.lease,
                live_terminal_control_event.clone(),
                callback_id_for(&invocation_id)?,
                command.correlation_id.clone(),
                if terminal_state == pareto_protocol::HookInvocationTerminalStateV1::Succeeded {
                    OperationOutcomeV1::Succeeded
                } else {
                    OperationOutcomeV1::Failed
                },
                format!("{terminal_reason:?}").to_ascii_lowercase(),
                output_digest
                    .clone()
                    .unwrap_or_else(|| input_digest.clone()),
                &terminal_sample,
            )
            .await;
            let (expected_control_cursor, control_payload, authority, terminal_control_event) =
                match live_plan {
                    Ok(planned) => (
                        planned.expected_cursor,
                        planned.payload,
                        HookTerminalAuthorityV1::LiveLease {
                            lease_fingerprint: planned.lease_fingerprint,
                        },
                        live_terminal_control_event,
                    ),
                    Err(error) if error.kind == RuntimeControlErrorKind::DeadlineExceeded => {
                        terminal_state = pareto_protocol::HookInvocationTerminalStateV1::TimedOut;
                        terminal_reason = HookReasonCodeV1::TimedOut;
                        late_result_digest = output_digest.clone();
                        output_digest = None;
                        gate_decision = None;
                        observer_result = None;
                        match registration.kind {
                            HookKindV1::Transform => {
                                proposal = command.proposal.clone();
                                input_digest = initial_digest.clone();
                                business_decision = HookBusinessDecisionV1::Deny;
                                execution_status = HookExecutionStatusV1::TransformFailed;
                                stop_reason = Some(HookReasonCodeV1::SkippedAfterTransformFailure);
                            }
                            HookKindV1::Gate => {
                                business_decision = HookBusinessDecisionV1::Deny;
                                execution_status = HookExecutionStatusV1::GateDenied;
                                stop_reason = Some(HookReasonCodeV1::SkippedAfterGateDenial);
                            }
                            HookKindV1::Observer
                                if registration.observer_failure_policy
                                    == Some(
                                        pareto_protocol::ObserverFailurePolicyV1::FailClosed,
                                    ) =>
                            {
                                execution_status = HookExecutionStatusV1::ObserverFailed;
                                stop_reason = Some(HookReasonCodeV1::SkippedAfterObserverFailure);
                            }
                            HookKindV1::Observer => {}
                        }
                        final_reason = HookReasonCodeV1::TimedOut;
                        let planned = plan_hook_timeout_settlement(
                            &mut control_connection,
                            schema_registry,
                            control_target,
                            &operation_id,
                            command.correlation_id.clone(),
                            &terminal_sample,
                        )
                        .await
                        .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?;
                        (
                            planned.expected_cursor,
                            planned.payload,
                            HookTerminalAuthorityV1::TimeoutRecovery {
                                timeout_key: Box::new(planned.timeout_key),
                            },
                            planned.event_id,
                        )
                    }
                    Err(_) => return Err(HookError::new(HookErrorKind::Unauthorized)),
                };
            let decision_id = decision_id_for(&invocation_id)?;
            decisions.push(decision_id.clone());
            let terminal_pair = HookPairBindingV1 {
                pair_id: terminal_pair_id,
                pair_kind: HookPairKindV1::Terminal,
                pair_fingerprint: Digest::parse(format!("sha256:{}", "0".repeat(64)))
                    .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
                control_event_id: terminal_control_event,
                hook_event_id: terminal_hook_event,
                operation_id,
                reservation_id,
                invocation_id: invocation_id.clone(),
            };
            let terminal_command = seal_terminal_pair_command(HookTerminalPairCommandV1 {
                scope: target.scope.clone(),
                owner: target.actor.clone(),
                control_stream_id: runtime_control_stream_id(&target.scope)
                    .map_err(|_| HookError::new(HookErrorKind::Unauthorized))?,
                hook_stream_id: hook_stream_id(&target.scope)?,
                expected_control_cursor,
                expected_hook_cursor: hook_cursor,
                control_sequence: 0,
                hook_sequence: 0,
                prepared_control_event_bytes: String::new(),
                prepared_hook_event_bytes: String::new(),
                pair: terminal_pair.clone(),
                occurred_at: terminal_sample.canonical_utc.clone(),
                correlation_id: command.correlation_id.clone(),
                hook_payload: HookInvocationTerminalPayloadV1 {
                    invocation_id: invocation_id.clone(),
                    decision_id: decision_id.clone(),
                    terminal_state,
                    pair: terminal_pair,
                    output_digest,
                    gate_decision,
                    observer_result,
                    accounted_usage: control_payload.accounted_usage.clone(),
                    reason_code: terminal_reason,
                },
                control_payload,
                authority,
            })?;
            let terminal_result = self
                .append_hook_terminal_pair_with_registry(
                    schema_registry,
                    target,
                    control_target,
                    Some(&resolved),
                    &terminal_command,
                    AtomicPairFault::None,
                )
                .await?;
            hook_cursor = append_cursor(&terminal_result.hook);
            if let Some(output_digest) = late_result_digest {
                let late = self
                    .append_hook_fact(
                        schema_registry,
                        target,
                        Some(&resolved),
                        &HookFactCommand {
                            expected_cursor: hook_cursor,
                            event_id: event_id_for("hook-late-result-event-v1", &invocation_id)?,
                            occurred_at: terminal_sample.canonical_utc.clone(),
                            correlation_id: command.correlation_id.clone(),
                            event_type: "hook-late-result-observed",
                            payload: HookLateResultObservedPayloadV1 {
                                invocation_id: invocation_id.clone(),
                                hook_id: registration.hook_id.clone(),
                                hook_revision: registration.hook_revision.clone(),
                                attempt: command.attempt,
                                output_digest,
                                reason_code: HookReasonCodeV1::LateAfterTerminal,
                                redaction_policy_revision: registration
                                    .redaction_policy_revision
                                    .clone(),
                            },
                        },
                    )
                    .await?;
                hook_cursor = append_cursor(&late);
            }
            if message_rejected {
                let rejected = self
                    .append_hook_fact(
                        schema_registry,
                        target,
                        Some(&resolved),
                        &HookFactCommand {
                            expected_cursor: hook_cursor,
                            event_id: event_id_for(
                                "hook-message-rejected-event-v1",
                                &invocation_id,
                            )?,
                            occurred_at: terminal_sample.canonical_utc.clone(),
                            correlation_id: command.correlation_id.clone(),
                            event_type: "hook-message-rejected",
                            payload: HookMessageRejectedPayloadV1 {
                                decision_id,
                                hook_point: command.point,
                                hook_id: Some(registration.hook_id.clone()),
                                hook_revision: Some(registration.hook_revision.clone()),
                                reason_code: HookReasonCodeV1::MessageRejected,
                                safe_subject_id: command.proposal.proposal_id.clone(),
                                input_digest: input_digest.clone(),
                                hook_registry_revision: resolved.revision.clone(),
                                source_cursor: command.source_cursor.clone(),
                                redaction_policy_revision: registration
                                    .redaction_policy_revision
                                    .clone(),
                            },
                        },
                    )
                    .await?;
                hook_cursor = append_cursor(&rejected);
            }
        }

        if gate_bearing
            && execution_status == HookExecutionStatusV1::Completed
            && required_gate_allows != required_gate_total
        {
            business_decision = HookBusinessDecisionV1::Deny;
            execution_status = HookExecutionStatusV1::GateDenied;
            final_reason = HookReasonCodeV1::RequiredGateMissing;
        }
        let final_input_digest = proposal_digest(&schema_set, &proposal)?;
        let finalized = HookPointFinalizedPayloadV1 {
            point_id: point_id.clone(),
            hook_point: command.point,
            source_cursor: command.source_cursor.clone(),
            initial_input_digest: initial_digest,
            final_input_digest,
            ordered_invocations,
            ordered_component_decisions: decisions,
            skipped_invocations: skipped,
            business_decision,
            execution_status,
            reason_code: final_reason,
        };
        self.append_hook_fact(
            schema_registry,
            target,
            Some(&resolved),
            &HookFactCommand {
                expected_cursor: hook_cursor,
                event_id: event_id_for("hook-point-finalized-event-v1", &point_id)?,
                occurred_at: command.occurred_at.clone(),
                correlation_id: command.correlation_id.clone(),
                event_type: "hook-point-finalized",
                payload: finalized,
            },
        )
        .await?;
        let projection = self
            .hook_projection(schema_registry, target, registry_revision)
            .await?;
        Ok(ExecuteHookPointResult {
            point_id,
            proposal,
            business_decision,
            execution_status,
            reason_code: final_reason,
            projection,
        })
    }

    #[cfg(test)]
    async fn append_hook_reserve_pair(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        command: &HookReservePairCommandV1,
        fault: AtomicPairFault,
    ) -> Result<HookReservePairResult, HookError> {
        self.append_hook_reserve_pair_with_registry(
            registry,
            target,
            control_target,
            None,
            command,
            fault,
        )
        .await
    }

    async fn append_hook_reserve_pair_with_registry(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        hook_registry: Option<&ResolvedHookRegistry>,
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
        .map_err(|_| HookError::new(HookErrorKind::SchemaUnavailable))?;
        let hook_event = lifecycle_event(
            &lifecycle.schema_set,
            &lifecycle.limits,
            &command.scope,
            &command.owner,
            &command.hook_stream_id,
            &command.pair.hook_event_id,
            command.hook_sequence,
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
        let presence = pair_presence(
            &mut transaction,
            &command.scope,
            &command.pair,
            &control_prepared,
            &hook_prepared,
        )
        .await?;
        if presence == PairPresence::Zero {
            let hook_aggregate = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            let hook_aggregate =
                fold_hook_events(&lifecycle.schema_set, &hook_aggregate, hook_registry)?;
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
            fold_hook_events(&lifecycle.schema_set, &hook_events, hook_registry)?;
        }
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &hook_prepared, fault)
            .await
            .map_err(map_store_error)?;
        let admitted_hook_events = read_hook_events_in_transaction(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
        )
        .await?;
        fold_hook_events(&lifecycle.schema_set, &admitted_hook_events, hook_registry)?;
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

    #[cfg(test)]
    async fn append_hook_terminal_pair(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        command: &HookTerminalPairCommandV1,
        fault: AtomicPairFault,
    ) -> Result<HookTerminalPairResult, HookError> {
        self.append_hook_terminal_pair_with_registry(
            registry,
            target,
            control_target,
            None,
            command,
            fault,
        )
        .await
    }

    async fn append_hook_terminal_pair_with_registry(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        control_target: &RuntimeControlTarget,
        hook_registry: Option<&ResolvedHookRegistry>,
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
            command.control_sequence,
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
            command.hook_sequence,
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
        let presence = pair_presence(
            &mut transaction,
            &command.scope,
            &command.pair,
            &control_prepared,
            &hook_prepared,
        )
        .await?;
        if presence == PairPresence::Zero {
            let hook_events = self
                .read_hook_events(
                    target,
                    lifecycle.schema_set.clone(),
                    lifecycle.limits.clone(),
                )
                .await?;
            let aggregate = fold_hook_events(&lifecycle.schema_set, &hook_events, hook_registry)?;
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
            fold_hook_events(&lifecycle.schema_set, &hook_events, hook_registry)?;
        }
        let pair = append_atomic_pair(&mut transaction, &control_prepared, &hook_prepared, fault)
            .await
            .map_err(map_store_error)?;
        let admitted_hook_events = read_hook_events_in_transaction(
            &mut transaction,
            target,
            lifecycle.schema_set.clone(),
            lifecycle.limits.clone(),
        )
        .await?;
        fold_hook_events(&lifecycle.schema_set, &admitted_hook_events, hook_registry)?;
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
        hook_registry: &HookRegistryRevisionV1,
    ) -> Result<HookProjectionV1, HookError> {
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
        let schema_set = lifecycle.schema_set.clone();
        let limits = lifecycle.limits.clone();
        let manifest = lifecycle.state.manifest.clone();
        if manifest.schema_ref.major != 2 {
            return Err(HookError::new(HookErrorKind::ManifestInvalid));
        }
        let resolved = ResolvedHookRegistry::resolve(&manifest, hook_registry, &schema_set)?;
        validate_runtime_control_history(
            &mut transaction,
            registry,
            &RuntimeControlTarget {
                scope: target.scope.clone(),
                principal: target.actor.clone(),
            },
        )
        .await
        .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?;
        let events = read_hook_events_in_transaction(
            &mut transaction,
            target,
            schema_set.clone(),
            limits.clone(),
        )
        .await?;
        let control_events = read_stream_events_in_transaction(
            &mut transaction,
            target,
            runtime_control_stream_id(&target.scope)
                .map_err(|_| HookError::new(HookErrorKind::AggregateCorrupt))?,
            schema_set.clone(),
            limits,
        )
        .await?;
        validate_cross_stream_pairs(&events, &control_events)?;
        let aggregate = fold_hook_events(&schema_set, &events, Some(&resolved))?;
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
        let projection = build_projection(
            &self.store_id,
            &target.scope,
            &target.actor,
            &schema_set,
            aggregate,
        )?;
        transaction
            .commit()
            .await
            .map_err(|_| HookError::new(HookErrorKind::Store))?;
        Ok(projection)
    }

    async fn recorded_hook_projection(
        &self,
        registry: &SchemaRegistry,
        target: &HookTarget,
        hook_registry: &HookRegistryRevisionV1,
        mode: &ExecutionMode,
    ) -> Result<HookProjectionV1, HookError> {
        if !matches!(mode, ExecutionMode::RecordedReplay { .. }) {
            return Err(HookError::new(HookErrorKind::UnsupportedMode));
        }
        self.hook_projection(registry, target, hook_registry).await
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

async fn read_hook_events_in_transaction(
    connection: &mut SqliteConnection,
    target: &HookTarget,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
) -> Result<Vec<ValidatedEvent>, HookError> {
    read_stream_events_in_transaction(
        connection,
        target,
        hook_stream_id(&target.scope)?,
        schema_set,
        limits,
    )
    .await
}

async fn read_stream_events_in_transaction(
    connection: &mut SqliteConnection,
    target: &HookTarget,
    stream_id: StreamId,
    schema_set: Arc<SchemaSet>,
    limits: ProtocolLimitsRef,
) -> Result<Vec<ValidatedEvent>, HookError> {
    let admitted = AdmittedRead {
        scope: target.scope.clone(),
        stream_id: Some(stream_id),
        schema_set,
        limits,
    };
    let user_present = i64::from(target.scope.user_id.is_some());
    let user_id = target
        .scope
        .user_id
        .as_ref()
        .map_or_else(String::new, |value| value.as_str().to_owned());
    let rows = sqlx::query(
        "SELECT envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? ORDER BY sequence_i64,event_id",
    )
    .bind(target.scope.tenant_id.as_str())
    .bind(user_present)
    .bind(user_id)
    .bind(target.scope.workspace_id.as_str())
    .bind(target.scope.run_id.as_str())
    .bind(target.scope.agent_id.as_str())
    .bind(admitted.stream_id.as_ref().expect("bound Hook stream").as_str())
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| HookError::new(HookErrorKind::Store))?;
    let events: Result<Vec<_>, _> = rows
        .iter()
        .map(|row| validate_row(row, &admitted).map_err(map_store_error))
        .collect();
    let events = events?;
    if events.is_empty() {
        Err(HookError::new(HookErrorKind::AggregateNotFound))
    } else {
        Ok(events)
    }
}

fn validate_cross_stream_pairs(
    hook_events: &[ValidatedEvent],
    control_events: &[ValidatedEvent],
) -> Result<(), HookError> {
    #[derive(Clone)]
    enum ExpectedPair {
        Reserve(HookPairBindingV1, Vec<BudgetVectorEntryV1>),
        Terminal(HookPairBindingV1, Vec<BudgetVectorEntryV1>),
    }
    let mut expected = BTreeMap::new();
    for event in hook_events {
        let pair = if let Some(payload) =
            event.downcast_payload::<HookInvocationReservedPayloadV1>()
        {
            ExpectedPair::Reserve(payload.pair.clone(), payload.reserved_usage.clone())
        } else if let Some(payload) = event.downcast_payload::<HookInvocationTerminalPayloadV1>() {
            ExpectedPair::Terminal(payload.pair.clone(), payload.accounted_usage.clone())
        } else {
            continue;
        };
        let pair_id = match &pair {
            ExpectedPair::Reserve(binding, _) | ExpectedPair::Terminal(binding, _) => {
                binding.pair_id.clone()
            }
        };
        if expected.insert(pair_id, pair).is_some() {
            return Err(HookError::new(HookErrorKind::AggregateCorrupt));
        }
    }
    for event in control_events {
        if let Some(payload) = event.downcast_payload::<OperationReservedPayloadV1>()
            && let Some(binding) = &payload.hook_pair
        {
            let Some(ExpectedPair::Reserve(expected_binding, usage)) =
                expected.remove(&binding.pair_id)
            else {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            };
            if binding != &expected_binding
                || binding.pair_kind != HookPairKindV1::Reserve
                || event.envelope().event_id != binding.control_event_id
                || payload.operation_id != binding.operation_id
                || payload.reservation_id != binding.reservation_id
                || payload.trusted_reservation != usage
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
        } else if let Some(payload) = event.downcast_payload::<OperationSettledPayloadV1>()
            && let Some(binding) = &payload.hook_pair
        {
            let Some(ExpectedPair::Terminal(expected_binding, usage)) =
                expected.remove(&binding.pair_id)
            else {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            };
            if binding != &expected_binding
                || binding.pair_kind != HookPairKindV1::Terminal
                || event.envelope().event_id != binding.control_event_id
                || payload.operation_id != binding.operation_id
                || payload.reservation_id != binding.reservation_id
                || payload.accounted_usage != usage
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
        }
    }
    if expected.is_empty() {
        Ok(())
    } else {
        Err(HookError::new(HookErrorKind::PartialPair))
    }
}

fn fold_hook_events(
    schema_set: &SchemaSet,
    events: &[ValidatedEvent],
    registry: Option<&ResolvedHookRegistry>,
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
    let mut open_point: Option<OpenPointFold> = None;
    let mut pair_bindings = BTreeMap::new();
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
            let point = open_point
                .as_mut()
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let ordered =
                registry.map(|registry| registry.ordered_for_point(point.start.hook_point));
            let registration = ordered
                .as_ref()
                .and_then(|ordered| ordered.get(point.next_ordinal).copied());
            let expected_invocation = point
                .start
                .ordered_invocations
                .get(point.next_ordinal)
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            if point.active_invocation.is_some()
                || expected_invocation != &payload.invocation_id
                || payload.invocation_id != payload.pair.invocation_id
                || payload.pair.pair_kind != HookPairKindV1::Reserve
                || payload.pair.pair_fingerprint.as_str() == format!("sha256:{}", "0".repeat(64))
                || invocations.contains_key(&payload.invocation_id)
                || payload.key.scope != event.envelope().scope
                || payload.key.hook_point != point.start.hook_point
                || payload.key.subject_proposal_id != point.start.subject_proposal_id
                || payload.key.ordinal as usize != point.next_ordinal
                || payload.key.source_cursor != point.start.source_cursor
                || payload.key.input_digest != point.current_input_digest
                || payload.key.attempt == 0
                || point
                    .last_phase
                    .is_some_and(|last| payload.key.phase < last)
                || registration.is_some_and(|registration| {
                    payload.key.hook_id != registration.hook_id
                        || payload.key.hook_revision != registration.hook_revision
                        || phase_for(point.start.hook_point, registration.kind)
                            != Some(payload.key.phase)
                        || invocation_id_for(
                            &point.start.point_id,
                            registration,
                            point.next_ordinal as u32,
                        )
                        .as_ref()
                            != Ok(&payload.invocation_id)
                })
                || (payload.key.phase == HookPhaseV1::Transform
                    && payload.key.predecessor_output_digest != point.last_transform_output)
                || (payload.key.phase != HookPhaseV1::Transform
                    && payload.key.predecessor_output_digest.is_some())
                || pair_bindings
                    .insert(
                        payload.pair.pair_id.clone(),
                        (payload.pair.pair_kind, payload.pair.clone()),
                    )
                    .is_some()
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            point.last_phase = Some(payload.key.phase);
            point.active_invocation = Some(payload.invocation_id.clone());
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
            let point = open_point
                .as_mut()
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let entry = invocations
                .get_mut(&payload.invocation_id)
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let ordered =
                registry.map(|registry| registry.ordered_for_point(point.start.hook_point));
            let registration = ordered
                .as_ref()
                .and_then(|ordered| ordered.get(point.next_ordinal).copied());
            let terminal_shape_valid = match (entry.key.phase, payload.terminal_state) {
                (
                    HookPhaseV1::Transform,
                    pareto_protocol::HookInvocationTerminalStateV1::Succeeded,
                ) => {
                    payload.output_digest.is_some()
                        && payload.gate_decision.is_none()
                        && payload.observer_result.is_none()
                }
                (HookPhaseV1::Gate, pareto_protocol::HookInvocationTerminalStateV1::Succeeded) => {
                    payload.output_digest.is_some()
                        && payload.gate_decision.is_some()
                        && payload.observer_result.is_none()
                }
                (
                    HookPhaseV1::Observer,
                    pareto_protocol::HookInvocationTerminalStateV1::Succeeded,
                ) => {
                    payload.output_digest.is_some()
                        && payload.gate_decision.is_none()
                        && payload.observer_result.is_some()
                }
                (_, _) => payload.gate_decision.is_none() && payload.observer_result.is_none(),
            };
            if point.active_invocation.as_ref() != Some(&payload.invocation_id)
                || entry.terminal_state.is_some()
                || payload.pair.invocation_id != payload.invocation_id
                || payload.pair.pair_kind != HookPairKindV1::Terminal
                || entry.operation_id != payload.pair.operation_id
                || entry.reservation_id != payload.pair.reservation_id
                || !terminal_shape_valid
                || pair_bindings
                    .insert(
                        payload.pair.pair_id.clone(),
                        (payload.pair.pair_kind, payload.pair.clone()),
                    )
                    .is_some()
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            entry.terminal_state = Some(payload.terminal_state);
            entry.decision_id = Some(payload.decision_id.clone());
            point.decisions.push(payload.decision_id.clone());
            match entry.key.phase {
                HookPhaseV1::Transform => {
                    if payload.terminal_state
                        == pareto_protocol::HookInvocationTerminalStateV1::Succeeded
                    {
                        point.last_transform_output = payload.output_digest.clone();
                        point.current_input_digest = payload
                            .output_digest
                            .clone()
                            .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
                    } else {
                        point.terminal_outcome = Some((
                            HookBusinessDecisionV1::Deny,
                            HookExecutionStatusV1::TransformFailed,
                            payload.reason_code,
                        ));
                    }
                }
                HookPhaseV1::Gate => {
                    let required = registration.is_some_and(|value| value.required == Some(true));
                    if required && matches!(payload.gate_decision, Some(GateDecisionV1::Allow {})) {
                        point.required_gate_allows += 1;
                    }
                    if payload.terminal_state
                        != pareto_protocol::HookInvocationTerminalStateV1::Succeeded
                        || matches!(payload.gate_decision, Some(GateDecisionV1::Deny { .. }))
                        || (required
                            && matches!(payload.gate_decision, Some(GateDecisionV1::Abstain {})))
                    {
                        point.terminal_outcome = Some((
                            HookBusinessDecisionV1::Deny,
                            HookExecutionStatusV1::GateDenied,
                            payload.reason_code,
                        ));
                    }
                }
                HookPhaseV1::Observer => {
                    let fail_closed = registration.is_some_and(|value| {
                        value.observer_failure_policy
                            == Some(pareto_protocol::ObserverFailurePolicyV1::FailClosed)
                    });
                    if fail_closed
                        && (payload.terminal_state
                            != pareto_protocol::HookInvocationTerminalStateV1::Succeeded
                            || matches!(
                                payload.observer_result,
                                Some(ObserverResultV1::Failure { .. })
                            ))
                    {
                        point.terminal_outcome = Some((
                            if matches!(
                                point.start.hook_point,
                                HookPointV1::BeforeProposalAdmission
                                    | HookPointV1::BeforeAuthoritativeCommit
                            ) {
                                HookBusinessDecisionV1::Allow
                            } else {
                                HookBusinessDecisionV1::ObserveOnly
                            },
                            HookExecutionStatusV1::ObserverFailed,
                            HookReasonCodeV1::ObserverFailedClosed,
                        ));
                    }
                }
            }
            point.active_invocation = None;
            point.next_ordinal += 1;
        } else if let Some(payload) = event.downcast_payload::<HookPointStartedPayloadV1>() {
            if open_point.is_some()
                || finalized_points.contains(&payload.point_id)
                || payload.ordered_invocations.is_empty()
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            let ordered = registry.map(|registry| registry.ordered_for_point(payload.hook_point));
            if ordered.as_ref().is_some_and(|ordered| {
                ordered.len() != payload.ordered_invocations.len()
                    || ordered.iter().enumerate().any(|(ordinal, registration)| {
                        invocation_id_for(&payload.point_id, registration, ordinal as u32).as_ref()
                            != Ok(&payload.ordered_invocations[ordinal])
                    })
            }) {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            let required_gate_total = ordered.as_ref().map_or(0, |ordered| {
                ordered
                    .iter()
                    .filter(|registration| {
                        registration.kind == HookKindV1::Gate && registration.required == Some(true)
                    })
                    .count()
            });
            open_point = Some(OpenPointFold {
                start: payload.clone(),
                next_ordinal: 0,
                active_invocation: None,
                current_input_digest: payload.initial_input_digest.clone(),
                last_transform_output: None,
                last_phase: None,
                decisions: Vec::new(),
                skipped: Vec::new(),
                required_gate_total,
                required_gate_allows: 0,
                terminal_outcome: None,
            });
        } else if let Some(payload) = event.downcast_payload::<HookInvocationSkippedPayloadV1>() {
            let point = open_point
                .as_mut()
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let ordered =
                registry.map(|registry| registry.ordered_for_point(point.start.hook_point));
            let registration = ordered
                .as_ref()
                .and_then(|ordered| ordered.get(point.next_ordinal).copied());
            if point.active_invocation.is_some()
                || point.start.ordered_invocations.get(point.next_ordinal)
                    != Some(&payload.invocation_id)
                || payload.hook_point != point.start.hook_point
                || payload.input_digest != point.current_input_digest
                || registration.is_some_and(|registration| {
                    phase_for(point.start.hook_point, registration.kind) != Some(payload.phase)
                })
                || point.terminal_outcome.is_none()
                || !matches!(
                    payload.reason_code,
                    HookReasonCodeV1::SkippedAfterTransformFailure
                        | HookReasonCodeV1::SkippedAfterGateDenial
                        | HookReasonCodeV1::SkippedAfterObserverFailure
                )
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            point.skipped.push(payload.invocation_id.clone());
            point.next_ordinal += 1;
            skipped_count += 1;
        } else if let Some(payload) = event.downcast_payload::<HookPointFinalizedPayloadV1>() {
            let point = open_point
                .take()
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            let gate_bearing = matches!(
                point.start.hook_point,
                HookPointV1::BeforeProposalAdmission | HookPointV1::BeforeAuthoritativeCommit
            );
            let expected = point.terminal_outcome.unwrap_or({
                if gate_bearing
                    && (point.required_gate_total == 0
                        || point.required_gate_allows != point.required_gate_total)
                {
                    (
                        HookBusinessDecisionV1::Deny,
                        HookExecutionStatusV1::GateDenied,
                        if point.required_gate_total == 0 {
                            HookReasonCodeV1::RequiredGateEmpty
                        } else {
                            HookReasonCodeV1::RequiredGateMissing
                        },
                    )
                } else {
                    (
                        if gate_bearing {
                            HookBusinessDecisionV1::Allow
                        } else {
                            HookBusinessDecisionV1::ObserveOnly
                        },
                        HookExecutionStatusV1::Completed,
                        HookReasonCodeV1::Completed,
                    )
                }
            });
            if point.active_invocation.is_some()
                || point.next_ordinal != point.start.ordered_invocations.len()
                || payload.point_id != point.start.point_id
                || payload.hook_point != point.start.hook_point
                || payload.source_cursor != point.start.source_cursor
                || payload.initial_input_digest != point.start.initial_input_digest
                || payload.final_input_digest != point.current_input_digest
                || payload.ordered_invocations != point.start.ordered_invocations
                || payload.ordered_component_decisions != point.decisions
                || payload.skipped_invocations != point.skipped
                || (
                    payload.business_decision,
                    payload.execution_status,
                    payload.reason_code,
                ) != expected
                || finalized_points.contains(&payload.point_id)
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            finalized_points.push(payload.point_id.clone());
        } else if let Some(payload) = event.downcast_payload::<HookLateResultObservedPayloadV1>() {
            let entry = invocations
                .get(&payload.invocation_id)
                .ok_or_else(|| HookError::new(HookErrorKind::AggregateCorrupt))?;
            if entry.terminal_state.is_none()
                || entry.key.hook_id != payload.hook_id
                || entry.key.hook_revision != payload.hook_revision
                || entry.key.attempt != payload.attempt
                || payload.reason_code != HookReasonCodeV1::LateAfterTerminal
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
            late_result_count += 1;
        } else if let Some(payload) = event.downcast_payload::<HookMessageRejectedPayloadV1>() {
            if payload.hook_registry_revision != initialization.hook_registry_revision
                || payload.reason_code != HookReasonCodeV1::MessageRejected
            {
                return Err(HookError::new(HookErrorKind::AggregateCorrupt));
            }
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
async fn kernel_owned_execution() {
    tests::kernel_owned_execution_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn kernel_owned_gate_short_circuit() {
    tests::kernel_owned_gate_short_circuit_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn kernel_owned_timeout() {
    tests::kernel_owned_timeout_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn kernel_owned_rejection() {
    tests::kernel_owned_rejection_case().await;
}

#[cfg(test)]
#[tokio::test]
async fn resealed_history_rejection() {
    tests::resealed_history_rejection_case().await;
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
