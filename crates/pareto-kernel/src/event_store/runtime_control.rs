//! Trusted capability, budget, cancellation, deadline, and late-result control.
//!
//! This module deliberately has no public extension point. Providers, tools, and hooks may hand
//! opaque proposals to these entry points, but only the Kernel constructs authority, leases,
//! reservations, settlements, and authoritative clock observations.

use pareto_protocol::{
    AgentId, AuthorizationDecisionV1, AuthorizationOutcomeV1, BudgetAccountId, BudgetAccountV1,
    BudgetAllocationV1, BudgetAmountV1, BudgetDimensionV1, BudgetRefundedPayloadV1, BudgetScopeV1,
    BudgetVectorEntryV1, CallbackId, CancellationAcknowledgedPayloadV1, CancellationId,
    CancellationRequestedPayloadV1, CancellationTargetV1, CapabilityGrantV1, CapabilityId,
    CapabilityIssuedPayloadV1, CapabilityRevokedPayloadV1, ControlMessageRejectedPayloadV1, Digest,
    EventCursor, EventId, ExecutionMode, IsolationScope, LateResultObservedPayloadV1, OperationId,
    OperationInterruptibilityV1, OperationOutcomeV1, OperationReservedPayloadV1,
    OperationSettledPayloadV1, ProtectedOperationDeniedPayloadV1, ReservationId, RevisionId,
    RunState, RuntimeControlAccountProjectionV1, RuntimeControlInitializedPayloadV1,
    RuntimeControlOperationProjectionV1, RuntimeControlProjectionHashViewV1,
    RuntimeControlProjectionV1, StreamId, TaskId, TaskState, TimeoutKeyV1,
    TrustedOperationContractV1, UsageEvidenceClassV1, ValidatedEvent, canonical_json, digest_json,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::{Row, Sqlite, SqliteConnection, Transaction};
use std::collections::{BTreeMap, BTreeSet};

use super::lifecycle::{EstablishedAggregate, LifecycleTarget, load_established};
use super::{
    AdmittedRead, AppendResult, ErrorKind, EventStore, EventStoreError, PreparedEvent,
    SchemaRegistry, check_prepared_idempotency, insert_prepared, user_key, validate_row,
};

const ROW_COLUMNS: &str = "envelope_json,envelope_fingerprint,schema_set_json,schema_set_fingerprint,limits_json,limits_fingerprint,tenant_id,user_present,user_id,workspace_id,run_id,agent_id,stream_id,sequence_i64,event_id,causation_id,correlation_id";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeControlErrorKind {
    Unauthorized,
    AggregateNotFound,
    AggregateCorrupt,
    LifecycleStateDenied,
    CapabilityInactive,
    DelegationWidening,
    ResourceEnvelopeUnavailable,
    BudgetExhausted,
    OperationConflict,
    CancellationPending,
    DeadlineExceeded,
    TerminalConflict,
    IdempotencyConflict,
    ProducerUnauthorized,
    NotDue,
    RecordedReplay,
    Busy,
    Io,
}

#[derive(Debug)]
pub(super) struct RuntimeControlError {
    pub(super) kind: RuntimeControlErrorKind,
}

impl RuntimeControlError {
    fn new(kind: RuntimeControlErrorKind) -> Self {
        Self { kind }
    }
}

impl From<EventStoreError> for RuntimeControlError {
    fn from(error: EventStoreError) -> Self {
        let kind = match error.kind {
            ErrorKind::IdempotencyConflict => RuntimeControlErrorKind::IdempotencyConflict,
            ErrorKind::Busy => RuntimeControlErrorKind::Busy,
            ErrorKind::Io => RuntimeControlErrorKind::Io,
            _ => RuntimeControlErrorKind::AggregateCorrupt,
        };
        Self::new(kind)
    }
}

impl From<sqlx::Error> for RuntimeControlError {
    fn from(error: sqlx::Error) -> Self {
        EventStoreError::from(error).into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RuntimeControlTarget {
    pub(super) scope: IsolationScope,
    /// Authenticated principal. This is never accepted from an event payload.
    pub(super) principal: AgentId,
}

/// Kernel-injected time. Tests use a fake; Runtime Control never sleeps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ClockSample {
    pub(super) canonical_utc: String,
    pub(super) wall_millis: u64,
    pub(super) monotonic_millis: u64,
    pub(super) process_epoch: String,
}

pub(super) trait RuntimeClock {
    fn sample(&self) -> ClockSample;
}

#[derive(Clone)]
pub(super) struct InitializeRuntimeControlCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) payload: RuntimeControlInitializedPayloadV1,
}

#[derive(Clone, Serialize)]
pub(super) struct ProtectedOperationProposal {
    pub(super) event_id: EventId,
    pub(super) denied_event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) operation_id: OperationId,
    pub(super) reservation_id: ReservationId,
    pub(super) task_id: Option<TaskId>,
    pub(super) resource: pareto_protocol::ResourceSelectorV1,
    pub(super) operation: String,
    /// Audit-only and never used to lower the trusted envelope.
    pub(super) requested_usage: Vec<BudgetVectorEntryV1>,
    pub(super) callback_namespace: String,
    pub(super) interruptibility: OperationInterruptibilityV1,
    pub(super) absolute_deadline_utc: String,
    pub(super) timeout_policy_revision: RevisionId,
}

/// Process-local proof returned only after an authoritative reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OperationLease {
    scope: IsolationScope,
    operation_id: OperationId,
    reservation_id: ReservationId,
    producer_revision: RevisionId,
    process_epoch: String,
    deadline_monotonic_millis: u64,
    seal: Digest,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ReserveResult {
    Reserved {
        event_id: EventId,
        sequence: i64,
        lease: Box<OperationLease>,
        warnings: Vec<String>,
    },
    AlreadyReserved {
        event_id: EventId,
        sequence: i64,
    },
    Denied {
        event_id: EventId,
        sequence: i64,
        reason_code: String,
    },
}

#[derive(Clone)]
pub(super) struct SettlementCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) callback_id: CallbackId,
    pub(super) operation_id: OperationId,
    pub(super) reservation_id: ReservationId,
    pub(super) producer_revision: RevisionId,
    pub(super) outcome: OperationOutcomeV1,
    pub(super) evidence_class: UsageEvidenceClassV1,
    pub(super) observed_usage: Vec<BudgetVectorEntryV1>,
    /// Kernel-meter result. Ignored unless `evidence_class` is verified.
    pub(super) metered_usage: Vec<BudgetVectorEntryV1>,
    pub(super) reason_code: String,
}

#[derive(Clone)]
pub(super) struct RevokeCapabilityCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) grant_id: CapabilityId,
    pub(super) reason_code: String,
}

#[derive(Clone)]
pub(super) struct CancellationCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) cancellation_id: CancellationId,
    pub(super) target: CancellationTargetV1,
    pub(super) reason_code: String,
}

#[derive(Clone)]
pub(super) struct CancellationAckCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) cancellation_id: CancellationId,
    pub(super) operation_id: OperationId,
    pub(super) reservation_id: ReservationId,
    pub(super) producer_revision: RevisionId,
}

#[derive(Clone)]
pub(super) struct LateResultCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) callback_id: CallbackId,
    pub(super) operation_id: OperationId,
    pub(super) producer_revision: RevisionId,
    pub(super) redacted_payload_digest: Digest,
}

#[derive(Clone)]
pub(super) struct RefundCommand {
    pub(super) event_id: EventId,
    pub(super) occurred_at: String,
    pub(super) correlation_id: String,
    pub(super) settlement_event_id: EventId,
    pub(super) operation_id: OperationId,
    pub(super) refunded_usage: Vec<BudgetVectorEntryV1>,
    pub(super) reason_code: String,
}

#[derive(Clone)]
pub(super) struct TimeoutRecoveryCommand {
    pub(super) correlation_id: String,
    pub(super) operation_id: OperationId,
    pub(super) recovery_revision: RevisionId,
    pub(super) evidence_fingerprint: Digest,
}

#[derive(Clone, Debug, Default)]
struct AccountTotals {
    reserved: u64,
    gross_consumed: u64,
    refunded: u64,
}

#[derive(Clone, Debug)]
struct OperationRecord {
    reservation: OperationReservedPayloadV1,
    settlement: Option<(EventId, OperationSettledPayloadV1)>,
    callbacks: BTreeMap<CallbackId, EventId>,
    refunded: BTreeMap<BudgetDimensionV1, u64>,
}

#[derive(Clone, Debug)]
struct RuntimeControlState {
    initialized: RuntimeControlInitializedPayloadV1,
    grants: BTreeMap<CapabilityId, CapabilityGrantV1>,
    revoked: BTreeMap<CapabilityId, EventId>,
    accounts: BTreeMap<BudgetAccountId, (BudgetAccountV1, AccountTotals)>,
    operations: BTreeMap<OperationId, OperationRecord>,
    cancellations: BTreeMap<CancellationId, (EventId, CancellationRequestedPayloadV1)>,
    cancellation_acks: BTreeMap<(CancellationId, OperationId), EventId>,
    cancellation_count: u64,
    late_result_count: u64,
    rejected_message_count: u64,
    sequence: i64,
    last_event_id: EventId,
}

struct EstablishedControl {
    state: RuntimeControlState,
    lifecycle: EstablishedAggregate,
    stream_id: StreamId,
}

impl EventStore {
    pub(super) async fn initialize_runtime_control(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        command: &InitializeRuntimeControlCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let lifecycle = load_lifecycle(&mut tx, registry, target).await?;
        if target.principal != target.scope.agent_id
            || command.payload.source_contract.schema_set_ref
                != lifecycle.state.manifest.schema_set_ref
            || command.payload.source_contract.protocol_limits_ref
                != lifecycle.state.manifest.protocol_limits_ref
            || command.payload.budget_plan.budget_revision
                != lifecycle.state.manifest.budget_revision
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::LifecycleStateDenied,
            ));
        }
        validate_initialization(&lifecycle, &command.payload)?;
        let stream = runtime_control_stream_id(&target.scope)?;
        let event = control_event(
            &lifecycle,
            &stream,
            &command.event_id,
            1,
            &command.occurred_at,
            &command.correlation_id,
            "runtime-control-initialized",
            &command.payload,
        )?;
        let prepared = PreparedEvent::new(&event, &lifecycle.schema_set, &lifecycle.limits)?;
        if let Some(result) = check_prepared_idempotency(&mut tx, &prepared).await? {
            tx.commit().await?;
            return Ok(result);
        }
        if lifecycle.state.run_state != RunState::Created {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::LifecycleStateDenied,
            ));
        }
        if stream_event_count(&mut tx, &target.scope, &stream).await? != 0 {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::OperationConflict,
            ));
        }
        let result = insert_prepared(&mut tx, &prepared).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub(super) async fn issue_capability(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        event_id: &EventId,
        occurred_at: &str,
        correlation_id: &str,
        grant: CapabilityGrantV1,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        if grant.issuer_actor != target.principal {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let payload = CapabilityIssuedPayloadV1 {
            grant: grant.clone(),
        };
        if event_sequence(&mut tx, event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                event_id,
                occurred_at,
                correlation_id,
                "capability-issued",
                &payload,
            )
            .await;
        }
        ensure_management_state(&aggregate.lifecycle)?;
        if grant.scope.task_id.as_ref().is_some_and(|task_id| {
            aggregate
                .lifecycle
                .state
                .tasks
                .get(task_id)
                .is_none_or(|task| {
                    matches!(
                        task.state,
                        TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
                    )
                })
        }) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::LifecycleStateDenied,
            ));
        }
        validate_delegation(&aggregate.state, target, &grant, occurred_at)?;
        append_control(
            tx,
            &aggregate,
            event_id,
            occurred_at,
            correlation_id,
            "capability-issued",
            &payload,
        )
        .await
    }

    pub(super) async fn revoke_capability(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        command: &RevokeCapabilityCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let grant = aggregate
            .state
            .grants
            .get(&command.grant_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::CapabilityInactive))?;
        if target.principal != target.scope.agent_id && target.principal != grant.issuer_actor {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let payload = CapabilityRevokedPayloadV1 {
            grant_id: command.grant_id.clone(),
            revoked_by: target.principal.clone(),
            reason_code: command.reason_code.clone(),
            revoked_at_utc: command.occurred_at.clone(),
        };
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "capability-revoked",
                &payload,
            )
            .await;
        }
        if let Some(existing_event_id) = aggregate.state.revoked.get(&command.grant_id) {
            let sequence = event_sequence(&mut tx, existing_event_id)
                .await?
                .ok_or_else(corrupt_error)?;
            tx.commit().await?;
            return Ok(AppendResult::AlreadyCommitted {
                event_id: existing_event_id.clone(),
                sequence,
            });
        }
        ensure_management_state(&aggregate.lifecycle)?;
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "capability-revoked",
            &payload,
        )
        .await
    }

    pub(super) async fn reserve_protected_operation<C: RuntimeClock>(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        proposal: &ProtectedOperationProposal,
        clock: &C,
    ) -> Result<ReserveResult, RuntimeControlError> {
        let sample = clock.sample();
        validate_clock_sample(&sample)?;
        let request_digest = safe_digest("protected-operation-request", proposal)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        if let Some(existing) = aggregate.state.operations.get(&proposal.operation_id) {
            if existing.reservation.authorization_decision.request_digest == request_digest
                && existing.reservation.reservation_id == proposal.reservation_id
            {
                let (event_id, sequence) =
                    find_event_for_operation(&mut tx, target, &proposal.operation_id).await?;
                tx.commit().await?;
                return Ok(ReserveResult::AlreadyReserved { event_id, sequence });
            }
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::IdempotencyConflict,
            ));
        }
        if matches!(
            aggregate.lifecycle.state.manifest.execution_mode,
            ExecutionMode::RecordedReplay { .. }
        ) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::RecordedReplay,
            ));
        }
        let authorization = authorize(
            &aggregate.state,
            target,
            proposal,
            &sample,
            request_digest.clone(),
        );
        let grant = match authorization {
            Ok(grant) => grant,
            Err(reason) => {
                let decision = AuthorizationDecisionV1 {
                    outcome: AuthorizationOutcomeV1::Denied,
                    reason_code: reason.clone(),
                    grant_id: None,
                    request_digest,
                };
                let result = append_control(
                    tx,
                    &aggregate,
                    &proposal.denied_event_id,
                    &proposal.occurred_at,
                    &proposal.correlation_id,
                    "protected-operation-denied",
                    &ProtectedOperationDeniedPayloadV1 {
                        operation_id: proposal.operation_id.clone(),
                        subject_actor: target.principal.clone(),
                        decision,
                        decided_at_utc: sample.canonical_utc,
                    },
                )
                .await?;
                let (event_id, sequence) = append_identity(&result);
                return Ok(ReserveResult::Denied {
                    event_id,
                    sequence,
                    reason_code: reason,
                });
            }
        };
        ensure_reserve_lifecycle(&aggregate.lifecycle, proposal.task_id.as_ref())?;
        if cancellation_applies(
            &aggregate.state,
            proposal.task_id.as_ref(),
            &proposal.operation_id,
        ) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::CancellationPending,
            ));
        }
        if sample.canonical_utc >= proposal.absolute_deadline_utc {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::DeadlineExceeded,
            ));
        }
        let contract = find_contract(
            &aggregate.state,
            &proposal.resource.kind,
            &proposal.operation,
        )?;
        let (allocations, warnings) = match reserve_allocations(
            &aggregate.state,
            proposal.task_id.as_ref(),
            &target.principal,
            &contract.resource_envelope,
            &proposal.resource.kind,
            &proposal.operation,
        ) {
            Ok(value) => value,
            Err(error) if error.kind == RuntimeControlErrorKind::BudgetExhausted => {
                let reason = "budget_hard_limit".to_owned();
                let result = append_control(
                    tx,
                    &aggregate,
                    &proposal.denied_event_id,
                    &proposal.occurred_at,
                    &proposal.correlation_id,
                    "protected-operation-denied",
                    &ProtectedOperationDeniedPayloadV1 {
                        operation_id: proposal.operation_id.clone(),
                        subject_actor: target.principal.clone(),
                        decision: AuthorizationDecisionV1 {
                            outcome: AuthorizationOutcomeV1::Denied,
                            reason_code: reason.clone(),
                            grant_id: Some(grant.grant_id.clone()),
                            request_digest,
                        },
                        decided_at_utc: sample.canonical_utc,
                    },
                )
                .await?;
                let (event_id, sequence) = append_identity(&result);
                return Ok(ReserveResult::Denied {
                    event_id,
                    sequence,
                    reason_code: reason,
                });
            }
            Err(error) => return Err(error),
        };
        let timeout_key = TimeoutKeyV1 {
            recovery_revision: aggregate
                .state
                .initialized
                .clock_contract
                .recovery_revision
                .clone(),
            scope: target.scope.clone(),
            control_stream_id: aggregate.stream_id.clone(),
            operation_id: proposal.operation_id.clone(),
            reservation_id: proposal.reservation_id.clone(),
            absolute_deadline_utc: proposal.absolute_deadline_utc.clone(),
            timeout_policy_revision: proposal.timeout_policy_revision.clone(),
            clock_revision: aggregate
                .state
                .initialized
                .clock_contract
                .clock_revision
                .clone(),
            source_schema_set_ref: aggregate
                .state
                .initialized
                .source_contract
                .schema_set_ref
                .clone(),
            source_protocol_limits_ref: aggregate
                .state
                .initialized
                .source_contract
                .protocol_limits_ref
                .clone(),
            operation_contract_revision: contract.contract_revision.clone(),
            meter_revision: contract.meter_revision.clone(),
        };
        let decision = AuthorizationDecisionV1 {
            outcome: AuthorizationOutcomeV1::Allowed,
            reason_code: "capability_allowed".to_owned(),
            grant_id: Some(grant.grant_id.clone()),
            request_digest,
        };
        let payload = OperationReservedPayloadV1 {
            operation_id: proposal.operation_id.clone(),
            reservation_id: proposal.reservation_id.clone(),
            subject_actor: target.principal.clone(),
            task_id: proposal.task_id.clone(),
            resource: proposal.resource.clone(),
            operation: proposal.operation.clone(),
            grant_id: grant.grant_id,
            authorization_decision: decision,
            requested_usage: canonical_vector(&proposal.requested_usage)?,
            trusted_reservation: canonical_vector(&contract.resource_envelope)?,
            allocations,
            operation_contract_revision: contract.contract_revision.clone(),
            producer_revision: contract.producer_revision.clone(),
            callback_namespace: proposal.callback_namespace.clone(),
            interruptibility: proposal.interruptibility,
            absolute_deadline_utc: proposal.absolute_deadline_utc.clone(),
            timeout_key,
            warnings: warnings.clone(),
            reserved_at_utc: sample.canonical_utc.clone(),
        };
        let result = append_control(
            tx,
            &aggregate,
            &proposal.event_id,
            &proposal.occurred_at,
            &proposal.correlation_id,
            "operation-reserved",
            &payload,
        )
        .await?;
        let (event_id, sequence) = append_identity(&result);
        let lease = make_lease(target, &payload, &sample)?;
        Ok(ReserveResult::Reserved {
            event_id,
            sequence,
            lease: Box::new(lease),
            warnings,
        })
    }

    pub(super) async fn settle_operation<C: RuntimeClock>(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        lease: &OperationLease,
        command: &SettlementCommand,
        clock: &C,
    ) -> Result<AppendResult, RuntimeControlError> {
        let sample = clock.sample();
        validate_clock_sample(&sample)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let record = aggregate
            .state
            .operations
            .get(&command.operation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        verify_lease(
            target,
            lease,
            &record.reservation,
            &command.producer_revision,
        )?;
        if command.reservation_id != record.reservation.reservation_id
            || command.producer_revision != record.reservation.producer_revision
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::ProducerUnauthorized,
            ));
        }
        let payload = settlement_payload(&record.reservation, command)?;
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "operation-settled",
                &payload,
            )
            .await;
        }
        if record.settlement.is_some() || record.callbacks.contains_key(&command.callback_id) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::TerminalConflict,
            ));
        }
        let deadline_elapsed = if sample.process_epoch == lease.process_epoch {
            sample.monotonic_millis >= lease.deadline_monotonic_millis
        } else {
            sample.canonical_utc >= record.reservation.absolute_deadline_utc
        };
        if deadline_elapsed {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::DeadlineExceeded,
            ));
        }
        let cancelled = cancellation_applies(
            &aggregate.state,
            record.reservation.task_id.as_ref(),
            &record.reservation.operation_id,
        );
        if (cancelled && command.outcome != OperationOutcomeV1::Cancelled)
            || (!cancelled && command.outcome == OperationOutcomeV1::Cancelled)
            || command.outcome == OperationOutcomeV1::TimedOut
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::CancellationPending,
            ));
        }
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "operation-settled",
            &payload,
        )
        .await
    }

    pub(super) async fn request_cancellation(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        command: &CancellationCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        validate_cancel_authority(&aggregate.state, target, &command.target)?;
        let payload = CancellationRequestedPayloadV1 {
            cancellation_id: command.cancellation_id.clone(),
            requester: target.principal.clone(),
            target: command.target.clone(),
            reason_code: command.reason_code.clone(),
            requested_at_utc: command.occurred_at.clone(),
        };
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "cancellation-requested",
                &payload,
            )
            .await;
        }
        if let Some((existing_event_id, existing)) =
            aggregate.state.cancellations.get(&command.cancellation_id)
        {
            if existing != &payload {
                return Err(RuntimeControlError::new(
                    RuntimeControlErrorKind::IdempotencyConflict,
                ));
            }
            let sequence = event_sequence(&mut tx, existing_event_id)
                .await?
                .ok_or_else(corrupt_error)?;
            tx.commit().await?;
            return Ok(AppendResult::AlreadyCommitted {
                event_id: existing_event_id.clone(),
                sequence,
            });
        }
        ensure_management_state(&aggregate.lifecycle)?;
        if let CancellationTargetV1::Operation { operation_id } = &command.target {
            if aggregate
                .state
                .operations
                .get(operation_id)
                .is_none_or(|record| record.settlement.is_some())
            {
                return Err(RuntimeControlError::new(
                    RuntimeControlErrorKind::TerminalConflict,
                ));
            }
        }
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "cancellation-requested",
            &payload,
        )
        .await
    }

    pub(super) async fn acknowledge_cancellation(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        lease: &OperationLease,
        command: &CancellationAckCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let (_, request) = aggregate
            .state
            .cancellations
            .get(&command.cancellation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        let record = aggregate
            .state
            .operations
            .get(&command.operation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        verify_lease(
            target,
            lease,
            &record.reservation,
            &command.producer_revision,
        )?;
        if !cancel_target_matches(&request.target, &record.reservation)
            || command.reservation_id != record.reservation.reservation_id
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let payload = CancellationAcknowledgedPayloadV1 {
            cancellation_id: command.cancellation_id.clone(),
            operation_id: command.operation_id.clone(),
            reservation_id: command.reservation_id.clone(),
            producer_revision: command.producer_revision.clone(),
            acknowledged_at_utc: command.occurred_at.clone(),
        };
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "cancellation-acknowledged",
                &payload,
            )
            .await;
        }
        if let Some(existing_event_id) = aggregate.state.cancellation_acks.get(&(
            command.cancellation_id.clone(),
            command.operation_id.clone(),
        )) {
            let sequence = event_sequence(&mut tx, existing_event_id)
                .await?
                .ok_or_else(corrupt_error)?;
            tx.commit().await?;
            return Ok(AppendResult::AlreadyCommitted {
                event_id: existing_event_id.clone(),
                sequence,
            });
        }
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "cancellation-acknowledged",
            &payload,
        )
        .await
    }

    pub(super) async fn observe_late_result(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        lease: &OperationLease,
        command: &LateResultCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let record = aggregate
            .state
            .operations
            .get(&command.operation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        verify_lease(
            target,
            lease,
            &record.reservation,
            &command.producer_revision,
        )?;
        if record.settlement.is_none() {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::TerminalConflict,
            ));
        }
        let contract = aggregate
            .state
            .initialized
            .operation_contracts
            .iter()
            .find(|contract| {
                contract.contract_revision == record.reservation.operation_contract_revision
            })
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
        let payload = LateResultObservedPayloadV1 {
            operation_id: command.operation_id.clone(),
            callback_id: command.callback_id.clone(),
            classification: "late_after_terminal".to_owned(),
            payload_digest: command.redacted_payload_digest.clone(),
            redaction_policy_revision: contract.redaction_policy_revision.clone(),
            received_at_utc: command.occurred_at.clone(),
        };
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "late-result-observed",
                &payload,
            )
            .await;
        }
        if let Some(existing_event_id) = record.callbacks.get(&command.callback_id) {
            let sequence = event_sequence(&mut tx, existing_event_id)
                .await?
                .ok_or_else(corrupt_error)?;
            let result = AppendResult::AlreadyCommitted {
                event_id: existing_event_id.clone(),
                sequence,
            };
            tx.commit().await?;
            return Ok(result);
        }
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "late-result-observed",
            &payload,
        )
        .await
    }

    pub(super) async fn refund_budget(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        command: &RefundCommand,
    ) -> Result<AppendResult, RuntimeControlError> {
        if target.principal != target.scope.agent_id {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let refund = vector_map(&command.refunded_usage)?;
        let refund_payload = BudgetRefundedPayloadV1 {
            refund_id: command.event_id.clone(),
            settlement_event_id: command.settlement_event_id.clone(),
            operation_id: command.operation_id.clone(),
            refunded_usage: map_vector(&refund),
            authorized_by: target.principal.clone(),
            reason_code: command.reason_code.clone(),
            refunded_at_utc: command.occurred_at.clone(),
        };
        if event_sequence(&mut tx, &command.event_id).await?.is_some() {
            return append_control(
                tx,
                &aggregate,
                &command.event_id,
                &command.occurred_at,
                &command.correlation_id,
                "budget-refunded",
                &refund_payload,
            )
            .await;
        }
        let record = aggregate
            .state
            .operations
            .get(&command.operation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        let (settlement_event, settlement) = record
            .settlement
            .as_ref()
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::TerminalConflict))?;
        if settlement_event != &command.settlement_event_id {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::OperationConflict,
            ));
        }
        let accounted = vector_map(&settlement.accounted_usage)?;
        let cumulative = refund
            .iter()
            .map(|(dimension, amount)| {
                (
                    dimension.clone(),
                    amount.saturating_add(record.refunded.get(dimension).copied().unwrap_or(0)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if !vector_lte(&cumulative, &accounted) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::BudgetExhausted,
            ));
        }
        for allocation in &record.reservation.allocations {
            let (account, totals) = aggregate
                .state
                .accounts
                .get(&allocation.account_id)
                .ok_or_else(corrupt_error)?;
            let amount = refund.get(&account.dimension).copied().unwrap_or(0);
            if amount > totals.gross_consumed.saturating_sub(totals.refunded) {
                return Err(RuntimeControlError::new(
                    RuntimeControlErrorKind::BudgetExhausted,
                ));
            }
        }
        append_control(
            tx,
            &aggregate,
            &command.event_id,
            &command.occurred_at,
            &command.correlation_id,
            "budget-refunded",
            &refund_payload,
        )
        .await
    }

    pub(super) async fn recover_timeout<C: RuntimeClock>(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
        command: &TimeoutRecoveryCommand,
        clock: &C,
    ) -> Result<AppendResult, RuntimeControlError> {
        if target.principal != target.scope.agent_id {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let sample = clock.sample();
        validate_clock_sample(&sample)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let aggregate = load_control(&mut tx, registry, target).await?;
        let record = aggregate
            .state
            .operations
            .get(&command.operation_id)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::OperationConflict))?;
        if command.recovery_revision != record.reservation.timeout_key.recovery_revision {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::Unauthorized,
            ));
        }
        let identity = TimeoutCommandIdentity {
            timeout_key: &record.reservation.timeout_key,
            clock: &sample,
            evidence_fingerprint: &command.evidence_fingerprint,
        };
        let digest = safe_digest("timeout-recovery-command", &identity)?;
        let suffix = &digest.as_str()[7..];
        let event_id = EventId::parse(format!("event_timeout-{suffix}"))
            .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
        let reserved = vector_map(&record.reservation.trusted_reservation)?;
        let payload = OperationSettledPayloadV1 {
            operation_id: record.reservation.operation_id.clone(),
            reservation_id: record.reservation.reservation_id.clone(),
            callback_id: None,
            outcome: OperationOutcomeV1::TimedOut,
            evidence_class: UsageEvidenceClassV1::Unknown,
            observed_usage: Vec::new(),
            accounted_usage: map_vector(&reserved),
            released_usage: Vec::new(),
            reason_code: "deadline_elapsed".to_owned(),
            timeout_command_fingerprint: Some(digest),
            settled_at_utc: sample.canonical_utc.clone(),
        };
        if let Some((terminal_event_id, _)) = &record.settlement {
            if terminal_event_id == &event_id {
                return append_control(
                    tx,
                    &aggregate,
                    &event_id,
                    &sample.canonical_utc,
                    &command.correlation_id,
                    "operation-settled",
                    &payload,
                )
                .await;
            }
            let sequence: i64 =
                sqlx::query_scalar("SELECT sequence_i64 FROM events WHERE event_id=?")
                    .bind(terminal_event_id.as_str())
                    .fetch_one(&mut *tx)
                    .await?;
            let result = AppendResult::AlreadyCommitted {
                event_id: terminal_event_id.clone(),
                sequence,
            };
            tx.commit().await?;
            return Ok(result);
        }
        if sample.canonical_utc < record.reservation.absolute_deadline_utc {
            return Err(RuntimeControlError::new(RuntimeControlErrorKind::NotDue));
        }
        append_control(
            tx,
            &aggregate,
            &event_id,
            &sample.canonical_utc,
            &command.correlation_id,
            "operation-settled",
            &payload,
        )
        .await
    }

    pub(super) async fn runtime_control_projection(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
    ) -> Result<RuntimeControlProjectionV1, RuntimeControlError> {
        let mut connection = self.pool.acquire().await?;
        let aggregate = load_control(&mut connection, registry, target).await?;
        project_control(&self.store_id, target, &aggregate)
    }

    /// Recorded replay folds authoritative source events only. It has no executor or writer.
    pub(super) async fn replay_runtime_control(
        &self,
        registry: &SchemaRegistry,
        target: &RuntimeControlTarget,
    ) -> Result<RuntimeControlProjectionV1, RuntimeControlError> {
        self.runtime_control_projection(registry, target).await
    }
}

pub(super) async fn ensure_no_pending_for_run(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    scope: &IsolationScope,
) -> Result<(), RuntimeControlError> {
    let target = RuntimeControlTarget {
        scope: scope.clone(),
        principal: scope.agent_id.clone(),
    };
    let Some(aggregate) = load_control_optional(connection, registry, &target).await? else {
        return Ok(());
    };
    if aggregate
        .state
        .operations
        .values()
        .any(|record| record.settlement.is_none())
    {
        Err(RuntimeControlError::new(
            RuntimeControlErrorKind::OperationConflict,
        ))
    } else {
        Ok(())
    }
}

pub(super) async fn ensure_no_pending_for_task(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    scope: &IsolationScope,
    task_id: &TaskId,
) -> Result<(), RuntimeControlError> {
    let target = RuntimeControlTarget {
        scope: scope.clone(),
        principal: scope.agent_id.clone(),
    };
    let Some(aggregate) = load_control_optional(connection, registry, &target).await? else {
        return Ok(());
    };
    if aggregate.state.operations.values().any(|record| {
        record.settlement.is_none() && record.reservation.task_id.as_ref() == Some(task_id)
    }) {
        Err(RuntimeControlError::new(
            RuntimeControlErrorKind::OperationConflict,
        ))
    } else {
        Ok(())
    }
}

#[derive(Serialize)]
struct TimeoutCommandIdentity<'a> {
    timeout_key: &'a TimeoutKeyV1,
    clock: &'a ClockSample,
    evidence_fingerprint: &'a Digest,
}

fn runtime_control_stream_id(scope: &IsolationScope) -> Result<StreamId, RuntimeControlError> {
    let suffix = scope
        .run_id
        .as_str()
        .strip_prefix("run_")
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    StreamId::parse(format!("stream_runtime-control-{suffix}"))
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))
}

async fn load_lifecycle(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    target: &RuntimeControlTarget,
) -> Result<EstablishedAggregate, RuntimeControlError> {
    load_established(
        connection,
        registry,
        &LifecycleTarget {
            scope: target.scope.clone(),
            actor: target.scope.agent_id.clone(),
        },
    )
    .await
    .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::Unauthorized))
}

async fn load_control_optional(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    target: &RuntimeControlTarget,
) -> Result<Option<EstablishedControl>, RuntimeControlError> {
    let stream = runtime_control_stream_id(&target.scope)?;
    if stream_event_count(connection, &target.scope, &stream).await? == 0 {
        return Ok(None);
    }
    load_control(connection, registry, target).await.map(Some)
}

async fn load_control(
    connection: &mut SqliteConnection,
    registry: &SchemaRegistry,
    target: &RuntimeControlTarget,
) -> Result<EstablishedControl, RuntimeControlError> {
    let lifecycle = load_lifecycle(connection, registry, target).await?;
    let stream_id = runtime_control_stream_id(&target.scope)?;
    let (present, user) = user_key(&target.scope);
    let sql = format!(
        "SELECT {ROW_COLUMNS} FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? ORDER BY sequence_i64"
    );
    let rows = sqlx::query(&sql)
        .bind(target.scope.tenant_id.as_str())
        .bind(present)
        .bind(user)
        .bind(target.scope.workspace_id.as_str())
        .bind(target.scope.run_id.as_str())
        .bind(target.scope.agent_id.as_str())
        .bind(stream_id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    if rows.is_empty() {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::AggregateNotFound,
        ));
    }
    let read = AdmittedRead {
        scope: target.scope.clone(),
        stream_id: Some(stream_id.clone()),
        schema_set: lifecycle.schema_set.clone(),
        limits: lifecycle.limits.clone(),
    };
    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        events
            .push(validate_row(&row, &read).map_err(|_| {
                RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt)
            })?);
    }
    let state = fold_control(&lifecycle, &stream_id, &events)?;
    Ok(EstablishedControl {
        state,
        lifecycle,
        stream_id,
    })
}

fn fold_control(
    lifecycle: &EstablishedAggregate,
    stream: &StreamId,
    events: &[ValidatedEvent],
) -> Result<RuntimeControlState, RuntimeControlError> {
    let first = events
        .first()
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::AggregateNotFound))?;
    let initialized = first
        .downcast_payload::<RuntimeControlInitializedPayloadV1>()
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    if first.envelope().event_type != "runtime-control-initialized"
        || first.envelope().sequence != "1"
        || first.envelope().stream_id != *stream
        || first.envelope().scope != lifecycle.state.manifest.scope
        || first.envelope().actor != lifecycle.state.manifest.scope.agent_id
        || initialized.source_contract.schema_set_ref != lifecycle.state.manifest.schema_set_ref
        || initialized.source_contract.protocol_limits_ref
            != lifecycle.state.manifest.protocol_limits_ref
        || initialized.budget_plan.budget_revision != lifecycle.state.manifest.budget_revision
    {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::AggregateCorrupt,
        ));
    }
    validate_initialization(lifecycle, initialized)?;
    let mut state = RuntimeControlState {
        initialized: initialized.clone(),
        grants: initialized
            .initial_grants
            .iter()
            .cloned()
            .map(|g| (g.grant_id.clone(), g))
            .collect(),
        revoked: BTreeMap::new(),
        accounts: initialized
            .budget_plan
            .accounts
            .iter()
            .cloned()
            .map(|a| (a.account_id.clone(), (a, AccountTotals::default())))
            .collect(),
        operations: BTreeMap::new(),
        cancellations: BTreeMap::new(),
        cancellation_acks: BTreeMap::new(),
        cancellation_count: 0,
        late_result_count: 0,
        rejected_message_count: 0,
        sequence: 1,
        last_event_id: first.envelope().event_id.clone(),
    };
    for (index, event) in events.iter().enumerate().skip(1) {
        let sequence = i64::try_from(index + 1)
            .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
        let envelope = event.envelope();
        if envelope.sequence.parse::<i64>().ok() != Some(sequence)
            || envelope.scope != lifecycle.state.manifest.scope
            || envelope.actor != lifecycle.state.manifest.scope.agent_id
            || envelope.stream_id != *stream
            || event.schema_set_ref() != &lifecycle.state.manifest.schema_set_ref
            || event.protocol_limits_ref() != &lifecycle.state.manifest.protocol_limits_ref
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::AggregateCorrupt,
            ));
        }
        apply_control_event(&mut state, event)?;
        state.sequence = sequence;
        state.last_event_id = envelope.event_id.clone();
    }
    Ok(state)
}

fn apply_control_event(
    state: &mut RuntimeControlState,
    event: &ValidatedEvent,
) -> Result<(), RuntimeControlError> {
    match event.envelope().event_type.as_str() {
        "capability-issued" => {
            let payload = downcast::<CapabilityIssuedPayloadV1>(event)?;
            if state
                .grants
                .insert(payload.grant.grant_id.clone(), payload.grant.clone())
                .is_some()
            {
                return corrupt();
            }
        }
        "capability-revoked" => {
            if state
                .revoked
                .insert(
                    downcast::<CapabilityRevokedPayloadV1>(event)?
                        .grant_id
                        .clone(),
                    event.envelope().event_id.clone(),
                )
                .is_some()
            {
                return corrupt();
            }
        }
        "operation-reserved" => {
            let payload = downcast::<OperationReservedPayloadV1>(event)?.clone();
            if state.operations.contains_key(&payload.operation_id) {
                return corrupt();
            }
            for allocation in &payload.allocations {
                let (_, totals) = state
                    .accounts
                    .get_mut(&allocation.account_id)
                    .ok_or_else(corrupt_error)?;
                totals.reserved = totals
                    .reserved
                    .checked_add(allocation.amount.as_u64())
                    .ok_or_else(corrupt_error)?;
            }
            state.operations.insert(
                payload.operation_id.clone(),
                OperationRecord {
                    reservation: payload,
                    settlement: None,
                    callbacks: BTreeMap::new(),
                    refunded: BTreeMap::new(),
                },
            );
        }
        "operation-settled" => {
            let payload = downcast::<OperationSettledPayloadV1>(event)?.clone();
            let record = state
                .operations
                .get_mut(&payload.operation_id)
                .ok_or_else(corrupt_error)?;
            if record.settlement.is_some()
                || payload.reservation_id != record.reservation.reservation_id
            {
                return corrupt();
            }
            let accounted = vector_map(&payload.accounted_usage)?;
            for allocation in &record.reservation.allocations {
                let (account, totals) = state
                    .accounts
                    .get_mut(&allocation.account_id)
                    .ok_or_else(corrupt_error)?;
                let amount = accounted.get(&account.dimension).copied().unwrap_or(0);
                totals.reserved = totals
                    .reserved
                    .checked_sub(allocation.amount.as_u64())
                    .ok_or_else(corrupt_error)?;
                totals.gross_consumed = totals
                    .gross_consumed
                    .checked_add(amount)
                    .ok_or_else(corrupt_error)?;
            }
            if let Some(callback) = &payload.callback_id {
                record
                    .callbacks
                    .insert(callback.clone(), event.envelope().event_id.clone());
            }
            record.settlement = Some((event.envelope().event_id.clone(), payload));
        }
        "budget-refunded" => {
            let payload = downcast::<BudgetRefundedPayloadV1>(event)?;
            let refund = vector_map(&payload.refunded_usage)?;
            let record = state
                .operations
                .get(&payload.operation_id)
                .ok_or_else(corrupt_error)?;
            if record
                .settlement
                .as_ref()
                .is_none_or(|(id, _)| id != &payload.settlement_event_id)
            {
                return corrupt();
            }
            let allocation_ids = record
                .reservation
                .allocations
                .iter()
                .map(|a| a.account_id.clone())
                .collect::<Vec<_>>();
            let accounted = vector_map(
                &record
                    .settlement
                    .as_ref()
                    .expect("settlement checked")
                    .1
                    .accounted_usage,
            )?;
            let prior_refunded = record.refunded.clone();
            let mut next_refunded = prior_refunded;
            for (dimension, amount) in &refund {
                let next = next_refunded
                    .get(dimension)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(*amount)
                    .filter(|value| *value <= accounted.get(dimension).copied().unwrap_or(0))
                    .ok_or_else(corrupt_error)?;
                next_refunded.insert(dimension.clone(), next);
            }
            state
                .operations
                .get_mut(&payload.operation_id)
                .expect("operation checked")
                .refunded = next_refunded;
            for account_id in allocation_ids {
                let (account, totals) = state
                    .accounts
                    .get_mut(&account_id)
                    .ok_or_else(corrupt_error)?;
                let amount = refund.get(&account.dimension).copied().unwrap_or(0);
                totals.refunded = totals
                    .refunded
                    .checked_add(amount)
                    .filter(|v| *v <= totals.gross_consumed)
                    .ok_or_else(corrupt_error)?;
            }
        }
        "cancellation-requested" => {
            let payload = downcast::<CancellationRequestedPayloadV1>(event)?.clone();
            if state
                .cancellations
                .insert(
                    payload.cancellation_id.clone(),
                    (event.envelope().event_id.clone(), payload),
                )
                .is_some()
            {
                return corrupt();
            }
            state.cancellation_count += 1;
        }
        "cancellation-acknowledged" => {
            let payload = downcast::<CancellationAcknowledgedPayloadV1>(event)?;
            if state
                .cancellation_acks
                .insert(
                    (
                        payload.cancellation_id.clone(),
                        payload.operation_id.clone(),
                    ),
                    event.envelope().event_id.clone(),
                )
                .is_some()
            {
                return corrupt();
            }
        }
        "late-result-observed" => {
            let payload = downcast::<LateResultObservedPayloadV1>(event)?;
            let record = state
                .operations
                .get_mut(&payload.operation_id)
                .ok_or_else(corrupt_error)?;
            if record.settlement.is_none()
                || record
                    .callbacks
                    .insert(
                        payload.callback_id.clone(),
                        event.envelope().event_id.clone(),
                    )
                    .is_some()
            {
                return corrupt();
            }
            state.late_result_count += 1;
        }
        "control-message-rejected" => {
            let _ = downcast::<ControlMessageRejectedPayloadV1>(event)?;
            state.rejected_message_count += 1;
        }
        "protected-operation-denied" => {
            let _ = downcast::<ProtectedOperationDeniedPayloadV1>(event)?;
            state.rejected_message_count += 1;
        }
        _ => return corrupt(),
    }
    Ok(())
}

fn validate_initialization(
    lifecycle: &EstablishedAggregate,
    payload: &RuntimeControlInitializedPayloadV1,
) -> Result<(), RuntimeControlError> {
    if payload.initial_grants.is_empty()
        || payload.budget_plan.accounts.is_empty()
        || payload.operation_contracts.is_empty()
    {
        return corrupt();
    }
    if payload.source_contract.projection_schema_ref
        != *lifecycle
            .schema_set
            .schema_ref("runtime-control-projection")
            .ok_or_else(corrupt_error)?
    {
        return corrupt();
    }
    let mut grant_ids = BTreeSet::new();
    for grant in &payload.initial_grants {
        if !grant_ids.insert(grant.grant_id.clone())
            || grant.issuer_actor != lifecycle.state.manifest.scope.agent_id
            || grant.subject_actor != lifecycle.state.manifest.scope.agent_id
            || grant.scope.isolation != lifecycle.state.manifest.scope
            || grant.parent_grant_id.is_some()
            || grant.operations.is_empty()
            || grant.schema_ref
                != *lifecycle
                    .schema_set
                    .schema_ref("capability-grant")
                    .ok_or_else(corrupt_error)?
        {
            return corrupt();
        }
    }
    let mut accounts = BTreeSet::new();
    for account in &payload.budget_plan.accounts {
        if !accounts.insert(account.account_id.clone())
            || account
                .soft_limit
                .as_ref()
                .is_some_and(|soft| soft.as_u64() > account.hard_limit.as_u64())
        {
            return corrupt();
        }
    }
    validate_contracts(&payload.operation_contracts)
}

fn validate_contracts(contracts: &[TrustedOperationContractV1]) -> Result<(), RuntimeControlError> {
    let mut keys = BTreeSet::new();
    for contract in contracts {
        if contract.resource_envelope.is_empty()
            || !keys.insert((contract.resource_kind.clone(), contract.operation.clone()))
        {
            return corrupt();
        }
        canonical_vector(&contract.resource_envelope)?;
    }
    Ok(())
}

fn ensure_management_state(lifecycle: &EstablishedAggregate) -> Result<(), RuntimeControlError> {
    if matches!(
        lifecycle.state.run_state,
        RunState::Created | RunState::Running | RunState::Paused
    ) {
        Ok(())
    } else {
        Err(RuntimeControlError::new(
            RuntimeControlErrorKind::LifecycleStateDenied,
        ))
    }
}

fn ensure_reserve_lifecycle(
    lifecycle: &EstablishedAggregate,
    task_id: Option<&TaskId>,
) -> Result<(), RuntimeControlError> {
    if lifecycle.state.run_state != RunState::Running {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::LifecycleStateDenied,
        ));
    }
    if let Some(task_id) = task_id {
        if lifecycle
            .state
            .tasks
            .get(task_id)
            .is_none_or(|task| task.state != TaskState::Running)
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::LifecycleStateDenied,
            ));
        }
    }
    Ok(())
}

fn validate_delegation(
    state: &RuntimeControlState,
    target: &RuntimeControlTarget,
    child: &CapabilityGrantV1,
    now: &str,
) -> Result<(), RuntimeControlError> {
    if child.scope.isolation != target.scope
        || child.issuer_actor != target.principal
        || state.grants.contains_key(&child.grant_id)
        || child.issued_at_utc != now
        || child.constraints.not_before_utc >= child.constraints.expires_at_utc
        || child.constraints.max_operation_usage.is_empty()
    {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::Unauthorized,
        ));
    }
    let parent_id = child
        .parent_grant_id
        .as_ref()
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::DelegationWidening))?;
    let parent = active_grant(state, parent_id, now)?;
    if parent.subject_actor != target.principal
        || child.schema_ref != parent.schema_ref
        || !parent.constraints.allow_delegation
        || parent.constraints.remaining_delegation_depth == 0
        || child.constraints.remaining_delegation_depth
            >= parent.constraints.remaining_delegation_depth
        || child.scope.task_id.is_none() && parent.scope.task_id.is_some()
        || child
            .scope
            .task_id
            .as_ref()
            .is_some_and(|task| parent.scope.task_id.as_ref().is_some_and(|p| p != task))
        || child.resource != parent.resource
        || !child
            .operations
            .iter()
            .all(|op| parent.operations.contains(op))
        || child.constraints.not_before_utc < parent.constraints.not_before_utc
        || child.constraints.expires_at_utc > parent.constraints.expires_at_utc
        || !vector_lte(
            &vector_map(&child.constraints.max_operation_usage)?,
            &vector_map(&parent.constraints.max_operation_usage)?,
        )
    {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::DelegationWidening,
        ));
    }
    Ok(())
}

fn active_grant<'a>(
    state: &'a RuntimeControlState,
    id: &CapabilityId,
    now: &str,
) -> Result<&'a CapabilityGrantV1, RuntimeControlError> {
    let mut current = state
        .grants
        .get(id)
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::CapabilityInactive))?;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current.grant_id.clone())
            || state.revoked.contains_key(&current.grant_id)
            || now < current.constraints.not_before_utc.as_str()
            || now >= current.constraints.expires_at_utc.as_str()
        {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::CapabilityInactive,
            ));
        }
        let Some(parent) = &current.parent_grant_id else {
            return Ok(state.grants.get(id).expect("initial grant exists"));
        };
        current = state
            .grants
            .get(parent)
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::CapabilityInactive))?;
    }
}

fn authorize(
    state: &RuntimeControlState,
    target: &RuntimeControlTarget,
    proposal: &ProtectedOperationProposal,
    sample: &ClockSample,
    _digest: Digest,
) -> Result<CapabilityGrantV1, String> {
    for grant in state.grants.values() {
        if grant.subject_actor == target.principal
            && grant.scope.isolation == target.scope
            && grant
                .scope
                .task_id
                .as_ref()
                .is_none_or(|task| proposal.task_id.as_ref() == Some(task))
            && grant.resource == proposal.resource
            && grant.operations.contains(&proposal.operation)
            && active_grant(state, &grant.grant_id, &sample.canonical_utc).is_ok()
        {
            let contract = find_contract(state, &proposal.resource.kind, &proposal.operation)
                .map_err(|_| "resource_envelope_unavailable".to_owned())?;
            if vector_lte(
                &vector_map(&contract.resource_envelope)
                    .map_err(|_| "resource_envelope_invalid".to_owned())?,
                &vector_map(&grant.constraints.max_operation_usage)
                    .map_err(|_| "capability_limit_invalid".to_owned())?,
            ) {
                return Ok(grant.clone());
            }
        }
    }
    Err("default_deny".to_owned())
}

fn find_contract<'a>(
    state: &'a RuntimeControlState,
    kind: &str,
    operation: &str,
) -> Result<&'a TrustedOperationContractV1, RuntimeControlError> {
    state
        .initialized
        .operation_contracts
        .iter()
        .find(|c| c.resource_kind == kind && c.operation == operation)
        .ok_or_else(|| {
            RuntimeControlError::new(RuntimeControlErrorKind::ResourceEnvelopeUnavailable)
        })
}

fn reserve_allocations(
    state: &RuntimeControlState,
    task: Option<&TaskId>,
    actor: &AgentId,
    envelope: &[BudgetVectorEntryV1],
    kind: &str,
    operation: &str,
) -> Result<(Vec<BudgetAllocationV1>, Vec<String>), RuntimeControlError> {
    let vector = vector_map(envelope)?;
    let mut allocations = Vec::new();
    let mut warnings = BTreeSet::new();
    for (dimension, amount) in vector {
        let op_limit = state
            .initialized
            .budget_plan
            .operation_limits
            .iter()
            .find(|limit| {
                limit.resource_kind == kind
                    && limit.operation == operation
                    && limit.dimension == dimension
            })
            .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::BudgetExhausted))?;
        if amount > op_limit.hard_limit.as_u64() {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::BudgetExhausted,
            ));
        }
        if op_limit
            .soft_limit
            .as_ref()
            .is_some_and(|soft| amount > soft.as_u64())
        {
            warnings.insert(format!("operation:{dimension:?}"));
        }
        let mut matched = 0;
        for (id, (account, totals)) in &state.accounts {
            let scoped = matches!(&account.scope, BudgetScopeV1::Run)
                || matches!(&account.scope, BudgetScopeV1::Task { task_id } if Some(task_id) == task)
                || matches!(&account.scope, BudgetScopeV1::Actor { actor_id } if actor_id == actor);
            if scoped && account.dimension == dimension {
                matched += 1;
                let committed = totals.gross_consumed.saturating_sub(totals.refunded);
                let next = committed
                    .checked_add(totals.reserved)
                    .and_then(|v| v.checked_add(amount))
                    .ok_or_else(|| {
                        RuntimeControlError::new(RuntimeControlErrorKind::BudgetExhausted)
                    })?;
                if next > account.hard_limit.as_u64() {
                    return Err(RuntimeControlError::new(
                        RuntimeControlErrorKind::BudgetExhausted,
                    ));
                }
                if account
                    .soft_limit
                    .as_ref()
                    .is_some_and(|soft| next > soft.as_u64())
                {
                    warnings.insert(format!("account:{}", id.as_str()));
                }
                allocations.push(BudgetAllocationV1 {
                    account_id: id.clone(),
                    amount: BudgetAmountV1::new(amount),
                });
            }
        }
        let required = 1 + usize::from(task.is_some()) + 1;
        if matched != required {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::BudgetExhausted,
            ));
        }
    }
    allocations.sort_by(|a, b| a.account_id.cmp(&b.account_id));
    Ok((allocations, warnings.into_iter().collect()))
}

fn validate_cancel_authority(
    state: &RuntimeControlState,
    target: &RuntimeControlTarget,
    cancel: &CancellationTargetV1,
) -> Result<(), RuntimeControlError> {
    match cancel {
        CancellationTargetV1::Run | CancellationTargetV1::Task { .. }
            if target.principal == target.scope.agent_id =>
        {
            Ok(())
        }
        CancellationTargetV1::Operation { operation_id } => {
            let record = state
                .operations
                .get(operation_id)
                .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::Unauthorized))?;
            if target.principal == target.scope.agent_id
                || target.principal == record.reservation.subject_actor
            {
                Ok(())
            } else {
                Err(RuntimeControlError::new(
                    RuntimeControlErrorKind::Unauthorized,
                ))
            }
        }
        _ => Err(RuntimeControlError::new(
            RuntimeControlErrorKind::Unauthorized,
        )),
    }
}

fn cancellation_applies(
    state: &RuntimeControlState,
    task: Option<&TaskId>,
    operation: &OperationId,
) -> bool {
    state
        .cancellations
        .values()
        .any(|(_, request)| match &request.target {
            CancellationTargetV1::Run => true,
            CancellationTargetV1::Task { task_id } => Some(task_id) == task,
            CancellationTargetV1::Operation { operation_id } => operation_id == operation,
        })
}

fn cancel_target_matches(
    target: &CancellationTargetV1,
    reservation: &OperationReservedPayloadV1,
) -> bool {
    match target {
        CancellationTargetV1::Run => true,
        CancellationTargetV1::Task { task_id } => reservation.task_id.as_ref() == Some(task_id),
        CancellationTargetV1::Operation { operation_id } => {
            operation_id == &reservation.operation_id
        }
    }
}

fn settlement_payload(
    reservation: &OperationReservedPayloadV1,
    command: &SettlementCommand,
) -> Result<OperationSettledPayloadV1, RuntimeControlError> {
    let reserved = vector_map(&reservation.trusted_reservation)?;
    let accounted = if command.evidence_class == UsageEvidenceClassV1::KernelMeterVerified {
        let metered = vector_map(&command.metered_usage)?;
        if !vector_lte(&metered, &reserved) {
            return Err(RuntimeControlError::new(
                RuntimeControlErrorKind::BudgetExhausted,
            ));
        }
        metered
    } else {
        reserved.clone()
    };
    let released = vector_sub(&reserved, &accounted)?;
    Ok(OperationSettledPayloadV1 {
        operation_id: command.operation_id.clone(),
        reservation_id: command.reservation_id.clone(),
        callback_id: Some(command.callback_id.clone()),
        outcome: command.outcome,
        evidence_class: command.evidence_class,
        observed_usage: canonical_vector(&command.observed_usage)?,
        accounted_usage: map_vector(&accounted),
        released_usage: map_vector(&released),
        reason_code: command.reason_code.clone(),
        timeout_command_fingerprint: None,
        settled_at_utc: command.occurred_at.clone(),
    })
}

fn make_lease(
    target: &RuntimeControlTarget,
    reservation: &OperationReservedPayloadV1,
    sample: &ClockSample,
) -> Result<OperationLease, RuntimeControlError> {
    let deadline_wall = parse_utc_millis(&reservation.absolute_deadline_utc)?;
    let remaining = deadline_wall
        .checked_sub(sample.wall_millis)
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::DeadlineExceeded))?;
    let deadline_monotonic_millis = sample
        .monotonic_millis
        .checked_add(remaining)
        .ok_or_else(|| RuntimeControlError::new(RuntimeControlErrorKind::DeadlineExceeded))?;
    let mut lease = OperationLease {
        scope: target.scope.clone(),
        operation_id: reservation.operation_id.clone(),
        reservation_id: reservation.reservation_id.clone(),
        producer_revision: reservation.producer_revision.clone(),
        process_epoch: sample.process_epoch.clone(),
        deadline_monotonic_millis,
        seal: Digest::parse(format!("sha256:{}", "0".repeat(64))).expect("constant digest"),
    };
    lease.seal = lease_seal(&lease)?;
    Ok(lease)
}

fn verify_lease(
    target: &RuntimeControlTarget,
    lease: &OperationLease,
    reservation: &OperationReservedPayloadV1,
    producer: &RevisionId,
) -> Result<(), RuntimeControlError> {
    if lease.seal == lease_seal(lease)?
        && lease.scope == target.scope
        && lease.operation_id == reservation.operation_id
        && lease.reservation_id == reservation.reservation_id
        && &lease.producer_revision == producer
    {
        Ok(())
    } else {
        Err(RuntimeControlError::new(
            RuntimeControlErrorKind::ProducerUnauthorized,
        ))
    }
}

fn lease_seal(lease: &OperationLease) -> Result<Digest, RuntimeControlError> {
    #[derive(Serialize)]
    struct LeaseView<'a> {
        scope: &'a IsolationScope,
        operation: &'a OperationId,
        reservation: &'a ReservationId,
        producer: &'a RevisionId,
        epoch: &'a str,
        deadline_monotonic_millis: u64,
    }
    safe_digest(
        "operation-lease",
        &LeaseView {
            scope: &lease.scope,
            operation: &lease.operation_id,
            reservation: &lease.reservation_id,
            producer: &lease.producer_revision,
            epoch: &lease.process_epoch,
            deadline_monotonic_millis: lease.deadline_monotonic_millis,
        },
    )
}

async fn append_control<T: Serialize>(
    mut transaction: Transaction<'_, Sqlite>,
    aggregate: &EstablishedControl,
    event_id: &EventId,
    occurred_at: &str,
    correlation_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<AppendResult, RuntimeControlError> {
    let existing_sequence: Option<i64> =
        sqlx::query_scalar("SELECT sequence_i64 FROM events WHERE event_id=?")
            .bind(event_id.as_str())
            .fetch_optional(&mut *transaction)
            .await?;
    let sequence = existing_sequence.unwrap_or(aggregate.state.sequence + 1);
    let event = control_event(
        &aggregate.lifecycle,
        &aggregate.stream_id,
        event_id,
        sequence,
        occurred_at,
        correlation_id,
        event_type,
        payload,
    )?;
    let prepared = PreparedEvent::new(
        &event,
        &aggregate.lifecycle.schema_set,
        &aggregate.lifecycle.limits,
    )?;
    if let Some(result) = check_prepared_idempotency(&mut transaction, &prepared).await? {
        transaction.commit().await?;
        return Ok(result);
    }
    let result = insert_prepared(&mut transaction, &prepared).await?;
    transaction.commit().await?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn control_event<T: Serialize>(
    lifecycle: &EstablishedAggregate,
    stream: &StreamId,
    event_id: &EventId,
    sequence: i64,
    occurred_at: &str,
    correlation_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<ValidatedEvent, RuntimeControlError> {
    super::lifecycle::lifecycle_event(
        &lifecycle.schema_set,
        &lifecycle.limits,
        &lifecycle.state.manifest.scope,
        &lifecycle.state.manifest.scope.agent_id,
        stream,
        event_id,
        sequence,
        occurred_at,
        correlation_id,
        event_type,
        payload,
    )
    .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))
}

async fn stream_event_count(
    connection: &mut SqliteConnection,
    scope: &IsolationScope,
    stream: &StreamId,
) -> Result<i64, RuntimeControlError> {
    let (present, user) = user_key(scope);
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=?")
        .bind(scope.tenant_id.as_str()).bind(present).bind(user).bind(scope.workspace_id.as_str()).bind(scope.run_id.as_str()).bind(scope.agent_id.as_str()).bind(stream.as_str()).fetch_one(&mut *connection).await?)
}

async fn event_sequence(
    connection: &mut SqliteConnection,
    event_id: &EventId,
) -> Result<Option<i64>, RuntimeControlError> {
    Ok(
        sqlx::query_scalar("SELECT sequence_i64 FROM events WHERE event_id=?")
            .bind(event_id.as_str())
            .fetch_optional(&mut *connection)
            .await?,
    )
}

async fn find_event_for_operation(
    connection: &mut SqliteConnection,
    target: &RuntimeControlTarget,
    operation: &OperationId,
) -> Result<(EventId, i64), RuntimeControlError> {
    let stream = runtime_control_stream_id(&target.scope)?;
    let (present, user) = user_key(&target.scope);
    let pattern = format!("%\"operation_id\":\"{}\"%", operation.as_str());
    let row = sqlx::query("SELECT event_id,sequence_i64 FROM events WHERE tenant_id=? AND user_present=? AND user_id=? AND workspace_id=? AND run_id=? AND agent_id=? AND stream_id=? AND envelope_json LIKE ? AND json_extract(envelope_json,'$.event_type')='operation-reserved' LIMIT 1")
        .bind(target.scope.tenant_id.as_str()).bind(present).bind(user).bind(target.scope.workspace_id.as_str()).bind(target.scope.run_id.as_str()).bind(target.scope.agent_id.as_str()).bind(stream.as_str()).bind(pattern).fetch_one(&mut *connection).await?;
    let event_id = EventId::parse(row.get::<String, _>(0))
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    Ok((event_id, row.get(1)))
}

fn project_control(
    store_id: &str,
    target: &RuntimeControlTarget,
    aggregate: &EstablishedControl,
) -> Result<RuntimeControlProjectionV1, RuntimeControlError> {
    let schema_ref = aggregate
        .state
        .initialized
        .source_contract
        .projection_schema_ref
        .clone();
    let accounts = aggregate
        .state
        .accounts
        .values()
        .map(|(account, totals)| RuntimeControlAccountProjectionV1 {
            account: account.clone(),
            reserved: BudgetAmountV1::new(totals.reserved),
            gross_consumed: BudgetAmountV1::new(totals.gross_consumed),
            refunded: BudgetAmountV1::new(totals.refunded),
            net_consumed: BudgetAmountV1::new(totals.gross_consumed - totals.refunded),
        })
        .collect::<Vec<_>>();
    let operations = aggregate
        .state
        .operations
        .values()
        .map(|record| RuntimeControlOperationProjectionV1 {
            operation_id: record.reservation.operation_id.clone(),
            reservation_id: record.reservation.reservation_id.clone(),
            absolute_deadline_utc: record.reservation.absolute_deadline_utc.clone(),
            interruptibility: record.reservation.interruptibility,
            cancellation_requested: cancellation_applies(
                &aggregate.state,
                record.reservation.task_id.as_ref(),
                &record.reservation.operation_id,
            ),
            outcome: record.settlement.as_ref().map(|(_, p)| p.outcome),
            reserved_usage: record.reservation.trusted_reservation.clone(),
            accounted_usage: record
                .settlement
                .as_ref()
                .map_or_else(Vec::new, |(_, p)| p.accounted_usage.clone()),
        })
        .collect::<Vec<_>>();
    let active_grants = aggregate
        .state
        .grants
        .values()
        .filter(|grant| !aggregate.state.revoked.contains_key(&grant.grant_id))
        .cloned()
        .collect::<Vec<_>>();
    let revoked_grants = aggregate.state.revoked.keys().cloned().collect::<Vec<_>>();
    let cursor = EventCursor {
        sequence: aggregate.state.sequence.to_string(),
        event_id: aggregate.state.last_event_id.clone(),
    };
    let view = RuntimeControlProjectionHashViewV1 {
        projection_schema_ref: schema_ref.clone(),
        source_store_id: store_id.to_owned(),
        scope: target.scope.clone(),
        owner_actor: target.scope.agent_id.clone(),
        stream_id: aggregate.stream_id.clone(),
        cursor: cursor.clone(),
        source_contract: aggregate.state.initialized.source_contract.clone(),
        accounts: accounts.clone(),
        operations: operations.clone(),
        active_grants: active_grants.clone(),
        revoked_grants: revoked_grants.clone(),
        cancellation_count: aggregate.state.cancellation_count.to_string(),
        late_result_count: aggregate.state.late_result_count.to_string(),
        rejected_message_count: aggregate.state.rejected_message_count.to_string(),
    };
    let value = serde_json::to_value(&view)
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    let projection_digest = digest_json("runtime-control-projection", &schema_ref, &value)
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    Ok(RuntimeControlProjectionV1 {
        schema_ref,
        source_store_id: store_id.to_owned(),
        scope: target.scope.clone(),
        owner_actor: target.scope.agent_id.clone(),
        stream_id: aggregate.stream_id.clone(),
        cursor,
        source_contract: aggregate.state.initialized.source_contract.clone(),
        accounts,
        operations,
        active_grants,
        revoked_grants,
        cancellation_count: aggregate.state.cancellation_count.to_string(),
        late_result_count: aggregate.state.late_result_count.to_string(),
        rejected_message_count: aggregate.state.rejected_message_count.to_string(),
        projection_digest,
    })
}

fn safe_digest<T: Serialize>(domain: &str, value: &T) -> Result<Digest, RuntimeControlError> {
    let value = serde_json::to_value(value)
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    let bytes = canonical_json(&value)
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))?;
    let mut hasher = Sha256::new();
    hasher.update(b"pareto-runtime-control-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(bytes.as_bytes());
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .map_err(|_| RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt))
}

fn parse_utc_millis(value: &str) -> Result<u64, RuntimeControlError> {
    // Runtime Control accepts only its canonical millisecond UTC representation. Persisted wall
    // time is converted at the trusted boundary; live timeout comparisons use monotonic time.
    let bytes = value.as_bytes();
    if bytes.len() != 24
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return Err(RuntimeControlError::new(
            RuntimeControlErrorKind::AggregateCorrupt,
        ));
    }
    let number = |start: usize, end: usize| -> Result<i64, RuntimeControlError> {
        value[start..end]
            .parse::<i64>()
            .map_err(|_| corrupt_error())
    };
    let year = number(0, 4)?;
    let month = number(5, 7)?;
    let day = number(8, 10)?;
    let hour = number(11, 13)?;
    let minute = number(14, 16)?;
    let second = number(17, 19)?;
    let millis = number(20, 23)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return corrupt();
    }
    // Howard Hinnant's civil-date transform, yielding days since 1970-01-01.
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let total = days
        .checked_mul(86_400_000)
        .and_then(|v| v.checked_add(hour * 3_600_000 + minute * 60_000 + second * 1_000 + millis))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(corrupt_error)?;
    Ok(total)
}

fn validate_clock_sample(sample: &ClockSample) -> Result<(), RuntimeControlError> {
    if parse_utc_millis(&sample.canonical_utc)? == sample.wall_millis
        && !sample.process_epoch.is_empty()
    {
        Ok(())
    } else {
        Err(RuntimeControlError::new(
            RuntimeControlErrorKind::AggregateCorrupt,
        ))
    }
}

fn canonical_vector(
    vector: &[BudgetVectorEntryV1],
) -> Result<Vec<BudgetVectorEntryV1>, RuntimeControlError> {
    let map = vector_map(vector)?;
    Ok(map_vector(&map))
}

fn vector_map(
    vector: &[BudgetVectorEntryV1],
) -> Result<BTreeMap<BudgetDimensionV1, u64>, RuntimeControlError> {
    let mut map = BTreeMap::new();
    for entry in vector {
        if entry.amount.as_u64() == 0
            || map
                .insert(entry.dimension.clone(), entry.amount.as_u64())
                .is_some()
        {
            return corrupt();
        }
    }
    Ok(map)
}

fn map_vector(map: &BTreeMap<BudgetDimensionV1, u64>) -> Vec<BudgetVectorEntryV1> {
    map.iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(dimension, amount)| BudgetVectorEntryV1 {
            dimension: dimension.clone(),
            amount: BudgetAmountV1::new(*amount),
        })
        .collect()
}

fn vector_lte(
    left: &BTreeMap<BudgetDimensionV1, u64>,
    right: &BTreeMap<BudgetDimensionV1, u64>,
) -> bool {
    left.iter()
        .all(|(dimension, amount)| *amount <= right.get(dimension).copied().unwrap_or(0))
}

fn vector_sub(
    left: &BTreeMap<BudgetDimensionV1, u64>,
    right: &BTreeMap<BudgetDimensionV1, u64>,
) -> Result<BTreeMap<BudgetDimensionV1, u64>, RuntimeControlError> {
    let mut result = BTreeMap::new();
    for (dimension, amount) in left {
        result.insert(
            dimension.clone(),
            amount
                .checked_sub(right.get(dimension).copied().unwrap_or(0))
                .ok_or_else(corrupt_error)?,
        );
    }
    Ok(result)
}

fn downcast<T: 'static>(event: &ValidatedEvent) -> Result<&T, RuntimeControlError> {
    event.downcast_payload::<T>().ok_or_else(corrupt_error)
}
fn corrupt<T>() -> Result<T, RuntimeControlError> {
    Err(corrupt_error())
}
fn corrupt_error() -> RuntimeControlError {
    RuntimeControlError::new(RuntimeControlErrorKind::AggregateCorrupt)
}
fn append_identity(result: &AppendResult) -> (EventId, i64) {
    match result {
        AppendResult::Appended { event_id, sequence }
        | AppendResult::AlreadyCommitted { event_id, sequence } => (event_id.clone(), *sequence),
    }
}

#[cfg(test)]
include!("runtime_control/tests.rs");
