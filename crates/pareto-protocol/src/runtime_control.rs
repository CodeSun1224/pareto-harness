//! Closed versioned contracts for trusted Runtime Control state and events.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AgentId, BudgetAccountId, CallbackId, CancellationId, CapabilityId, Digest, EventCursor,
    EventId, EventTypeBinding, IsolationScope, OperationId, ProtocolLimitsRef, ReservationId,
    RevisionId, SchemaRef, SchemaSetRef, StreamId, TaskId,
};

fn deserialize_present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Canonical unsigned decimal budget amount.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct BudgetAmountV1(String);

impl BudgetAmountV1 {
    /// Creates a checked canonical amount.
    pub fn new(value: u64) -> Self {
        Self(value.to_string())
    }

    /// Parses a canonical unsigned decimal amount.
    pub fn parse(value: impl Into<String>) -> Result<Self, crate::ValidationError> {
        let value = value.into();
        let canonical = value == "0"
            || (!value.starts_with('0')
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && value.parse::<u64>().is_ok());
        if !canonical {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: String::new(),
                contract: "budget_amount_v1".to_owned(),
                detail: "budget amount must be a canonical unsigned u64 decimal".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the canonical wire amount.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the checked numeric amount.
    pub fn as_u64(&self) -> u64 {
        self.0
            .parse()
            .expect("BudgetAmountV1 construction validates u64")
    }
}

impl<'de> Deserialize<'de> for BudgetAmountV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid BudgetAmountV1"))
    }
}

/// Closed resource-accounting dimension set.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum BudgetDimensionV1 {
    /// Model or synthetic token units.
    Tokens,
    /// Smallest fixed cost unit.
    CostMicrounits,
    /// Elapsed milliseconds.
    ElapsedMillis,
    /// Protected tool-call count.
    ToolCalls,
    /// Versioned extension dimension.
    Other {
        /// Stable dimension name.
        name: String,
        /// Immutable unit identity.
        unit_revision: RevisionId,
    },
}

/// One dimension and amount in a deterministic resource vector.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetVectorEntryV1 {
    /// Resource dimension.
    pub dimension: BudgetDimensionV1,
    /// Canonical amount.
    pub amount: BudgetAmountV1,
}

/// Exact protected-resource selector.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSelectorV1 {
    /// Closed resource kind.
    pub kind: String,
    /// Optional exact logical identity.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub id: Option<String>,
}

/// Complete Capability target scope.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityScopeV1 {
    /// Full Run isolation scope.
    pub isolation: IsolationScope,
    /// Optional exact Task narrowing.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub task_id: Option<TaskId>,
}

/// Immutable Capability constraints.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityConstraintsV1 {
    /// Inclusive canonical UTC activation time.
    pub not_before_utc: String,
    /// Exclusive canonical UTC expiry time.
    pub expires_at_utc: String,
    /// Per-operation maximum resource vector.
    pub max_operation_usage: Vec<BudgetVectorEntryV1>,
    /// Whether direct delegation is allowed.
    pub allow_delegation: bool,
    /// Maximum remaining child depth.
    pub remaining_delegation_depth: u32,
}

/// Immutable signed Capability fact; bytes alone do not grant authority.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantV1 {
    /// Grant Schema identity.
    pub schema_ref: SchemaRef,
    /// Aggregate-unique grant identity.
    pub grant_id: CapabilityId,
    /// Persisted issuing actor.
    pub issuer_actor: AgentId,
    /// Exact subject actor.
    pub subject_actor: AgentId,
    /// Full scope and optional Task narrowing.
    pub scope: CapabilityScopeV1,
    /// Exact resource selector.
    pub resource: ResourceSelectorV1,
    /// Sorted unique non-empty operations.
    pub operations: Vec<String>,
    /// Closed constraints.
    pub constraints: CapabilityConstraintsV1,
    /// Optional direct parent.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub parent_grant_id: Option<CapabilityId>,
    /// Canonical UTC issue time.
    pub issued_at_utc: String,
}

/// Scope of one budget account.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum BudgetScopeV1 {
    /// Entire Run.
    Run,
    /// Exact Task.
    Task {
        /// Task identity.
        task_id: TaskId,
    },
    /// Exact Actor within the Run.
    Actor {
        /// Actor identity.
        actor_id: AgentId,
    },
}

/// Immutable budget account definition.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetAccountV1 {
    /// Account identity.
    pub account_id: BudgetAccountId,
    /// Account scope.
    pub scope: BudgetScopeV1,
    /// Account dimension.
    pub dimension: BudgetDimensionV1,
    /// Hard limit.
    pub hard_limit: BudgetAmountV1,
    /// Optional soft warning threshold.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub soft_limit: Option<BudgetAmountV1>,
}

/// Per-operation upper limit definition.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationBudgetLimitV1 {
    /// Resource kind.
    pub resource_kind: String,
    /// Operation name.
    pub operation: String,
    /// Dimension.
    pub dimension: BudgetDimensionV1,
    /// Hard per-operation limit.
    pub hard_limit: BudgetAmountV1,
    /// Optional soft threshold.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub soft_limit: Option<BudgetAmountV1>,
}

/// Immutable multi-scope budget plan.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetPlanV1 {
    /// Manifest-pinned budget revision.
    pub budget_revision: RevisionId,
    /// Sorted unique accounts.
    pub accounts: Vec<BudgetAccountV1>,
    /// Sorted unique per-operation limits.
    pub operation_limits: Vec<OperationBudgetLimitV1>,
}

/// Retained trusted resource envelope and producer contract.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedOperationContractV1 {
    /// Exact contract revision.
    pub contract_revision: RevisionId,
    /// Exact retained source SchemaSet for this contract.
    pub source_schema_set_ref: SchemaSetRef,
    /// Exact trusted adapter revision.
    pub adapter_revision: RevisionId,
    /// Resource kind.
    pub resource_kind: String,
    /// Operation name.
    pub operation: String,
    /// Sorted unique dimensions that the trusted envelope must cover.
    pub required_dimensions: Vec<BudgetDimensionV1>,
    /// Authoritative maximum resource vector.
    pub resource_envelope: Vec<BudgetVectorEntryV1>,
    /// Kernel meter revision.
    pub meter_revision: RevisionId,
    /// Exact Kernel enforcement policy revision.
    pub meter_policy_revision: RevisionId,
    /// Approved producer revision.
    pub producer_revision: RevisionId,
    /// Exact callback namespace bound at dispatch and callback admission.
    pub callback_namespace: String,
    /// Late-result redaction policy revision.
    pub redaction_policy_revision: RevisionId,
}

/// Manifest-pinned Runtime Control source identity.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlSourceContractV1 {
    /// Exact source SchemaSet.
    pub schema_set_ref: SchemaSetRef,
    /// Exact source protocol limits.
    pub protocol_limits_ref: ProtocolLimitsRef,
    /// Exact lifecycle cursor observed by sequence-one initialization.
    pub lifecycle_cursor: EventCursor,
    /// Exact reducer revision.
    pub reducer_revision: RevisionId,
    /// Sorted exact control event payload bindings accepted by the reducer.
    pub accepted_event_bindings: Vec<EventTypeBinding>,
    /// Exact history-chain algorithm revision.
    pub history_digest_revision: RevisionId,
    /// Exact projection output Schema.
    pub projection_schema_ref: SchemaRef,
    /// Exact retained output reader revision.
    pub projection_reader_revision: RevisionId,
}

/// Runtime clock policy fixed at initialization.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeClockContractV1 {
    /// Clock policy revision.
    pub clock_revision: RevisionId,
    /// Timeout recovery contract revision.
    pub recovery_revision: RevisionId,
}

/// Sequence-one Runtime Control payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlInitializedPayloadV1 {
    /// Manifest-pinned source contract.
    pub source_contract: RuntimeControlSourceContractV1,
    /// Initial minimum grants.
    pub initial_grants: Vec<CapabilityGrantV1>,
    /// Immutable budget plan.
    pub budget_plan: BudgetPlanV1,
    /// Clock and recovery contract.
    pub clock_contract: RuntimeClockContractV1,
    /// Sorted exact references resolved only through the retained Kernel registry.
    pub operation_contract_refs: Vec<RevisionId>,
}

/// Capability issue/delegation payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityIssuedPayloadV1 {
    /// Issued grant.
    pub grant: CapabilityGrantV1,
}

/// Capability revocation payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRevokedPayloadV1 {
    /// Revoked grant.
    pub grant_id: CapabilityId,
    /// Authorized revoker.
    pub revoked_by: AgentId,
    /// Stable reason.
    pub reason_code: String,
    /// Canonical UTC revocation time.
    pub revoked_at_utc: String,
}

/// Structured authorization outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationOutcomeV1 {
    /// Request was admitted.
    Allowed,
    /// Request was denied.
    Denied,
}

/// Safe authorization decision record.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationDecisionV1 {
    /// Decision outcome.
    pub outcome: AuthorizationOutcomeV1,
    /// Stable reason code.
    pub reason_code: String,
    /// Matching grant when allowed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub grant_id: Option<CapabilityId>,
    /// Safe request digest.
    pub request_digest: Digest,
}

/// Protected-operation denial audit payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedOperationDeniedPayloadV1 {
    /// Operation identity.
    pub operation_id: OperationId,
    /// Subject actor.
    pub subject_actor: AgentId,
    /// Exact decision.
    pub decision: AuthorizationDecisionV1,
    /// Canonical UTC decision time.
    pub decided_at_utc: String,
}

/// Allocation of one reserved dimension to one account.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetAllocationV1 {
    /// Account identity.
    pub account_id: BudgetAccountId,
    /// Reserved amount.
    pub amount: BudgetAmountV1,
}

/// Operation interruptibility class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationInterruptibilityV1 {
    /// Operation polls cancellation cooperatively.
    Cooperative,
    /// Operation cannot be interrupted inside its boundary.
    Uninterruptible,
}

/// Stable timeout identity fixed at reserve.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeoutKeyV1 {
    /// Recovery contract revision.
    pub recovery_revision: RevisionId,
    /// Full isolation scope.
    pub scope: IsolationScope,
    /// Control stream.
    pub control_stream_id: StreamId,
    /// Operation identity.
    pub operation_id: OperationId,
    /// Reservation identity.
    pub reservation_id: ReservationId,
    /// Absolute canonical UTC deadline.
    pub absolute_deadline_utc: String,
    /// Timeout policy revision.
    pub timeout_policy_revision: RevisionId,
    /// Clock policy revision.
    pub clock_revision: RevisionId,
    /// Exact source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Exact source limits.
    pub source_protocol_limits_ref: ProtocolLimitsRef,
    /// Trusted operation contract.
    pub operation_contract_revision: RevisionId,
    /// Kernel meter contract.
    pub meter_revision: RevisionId,
}

/// Authoritative reservation payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationReservedPayloadV1 {
    /// Operation identity.
    pub operation_id: OperationId,
    /// Reservation identity.
    pub reservation_id: ReservationId,
    /// Subject actor.
    pub subject_actor: AgentId,
    /// Exact Task.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub task_id: Option<TaskId>,
    /// Resource selector.
    pub resource: ResourceSelectorV1,
    /// Operation name.
    pub operation: String,
    /// Matching capability.
    pub grant_id: CapabilityId,
    /// Complete allow decision and request digest.
    pub authorization_decision: AuthorizationDecisionV1,
    /// Untrusted observation retained for audit.
    pub requested_usage: Vec<BudgetVectorEntryV1>,
    /// Trusted reserved vector.
    pub trusted_reservation: Vec<BudgetVectorEntryV1>,
    /// Per-account allocations.
    pub allocations: Vec<BudgetAllocationV1>,
    /// Trusted operation contract.
    pub operation_contract_revision: RevisionId,
    /// Approved producer.
    pub producer_revision: RevisionId,
    /// Callback namespace.
    pub callback_namespace: String,
    /// Interruptibility class.
    pub interruptibility: OperationInterruptibilityV1,
    /// Absolute deadline.
    pub absolute_deadline_utc: String,
    /// Stable timeout key.
    pub timeout_key: TimeoutKeyV1,
    /// Soft-limit warnings.
    pub warnings: Vec<String>,
    /// Canonical UTC reserve time.
    pub reserved_at_utc: String,
}

/// Terminal protected-operation outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationOutcomeV1 {
    /// Operation succeeded.
    Succeeded,
    /// Operation failed.
    Failed,
    /// Operation confirmed cancellation.
    Cancelled,
    /// Kernel deadline won.
    TimedOut,
}

/// Authority class for accounted usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageEvidenceClassV1 {
    /// Deterministic Kernel meter evidence.
    KernelMeterVerified,
    /// Missing or unverified usage, accounted conservatively.
    Unknown,
}

/// Durable, independently verifiable Kernel meter evidence.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelMeterEvidenceV1 {
    /// Exact retained meter revision.
    pub meter_revision: RevisionId,
    /// Live process epoch in which metering occurred.
    pub process_epoch: String,
    /// Canonical metered vector.
    pub usage: Vec<BudgetVectorEntryV1>,
    /// Whether the Kernel stopped an attempted unit before envelope overflow.
    pub contract_violation: bool,
    /// Domain-separated canonical evidence fingerprint.
    pub snapshot_fingerprint: Digest,
}

/// Authoritative terminal settlement payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSettledPayloadV1 {
    /// Operation identity.
    pub operation_id: OperationId,
    /// Reservation identity.
    pub reservation_id: ReservationId,
    /// Optional callback identity.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub callback_id: Option<CallbackId>,
    /// Canonical callback command fingerprint when callback-driven.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub callback_fingerprint: Option<Digest>,
    /// Terminal outcome.
    pub outcome: OperationOutcomeV1,
    /// Usage evidence authority.
    pub evidence_class: UsageEvidenceClassV1,
    /// Durable Kernel meter evidence when present.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub kernel_meter_evidence: Option<KernelMeterEvidenceV1>,
    /// Non-authoritative observation.
    pub observed_usage: Vec<BudgetVectorEntryV1>,
    /// Authoritative accounted vector.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
    /// Released vector.
    pub released_usage: Vec<BudgetVectorEntryV1>,
    /// Stable reason code.
    pub reason_code: String,
    /// Optional timeout command fingerprint.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub timeout_command_fingerprint: Option<Digest>,
    /// Canonical UTC settlement time.
    pub settled_at_utc: String,
}

/// Owner-authorized budget correction payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetRefundedPayloadV1 {
    /// Refund identity.
    pub refund_id: EventId,
    /// Referenced settlement event.
    pub settlement_event_id: EventId,
    /// Operation identity.
    pub operation_id: OperationId,
    /// Refund vector.
    pub refunded_usage: Vec<BudgetVectorEntryV1>,
    /// Authorized owner.
    pub authorized_by: AgentId,
    /// Stable reason.
    pub reason_code: String,
    /// Canonical UTC correction time.
    pub refunded_at_utc: String,
}

/// Cancellation target.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum CancellationTargetV1 {
    /// Entire Run.
    Run,
    /// Exact Task and descendants' operations.
    Task {
        /// Task identity.
        task_id: TaskId,
    },
    /// Exact operation only.
    Operation {
        /// Operation identity.
        operation_id: OperationId,
    },
}

/// Cancellation request payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationRequestedPayloadV1 {
    /// Cancellation identity.
    pub cancellation_id: CancellationId,
    /// Authorized requester.
    pub requester: AgentId,
    /// Exact target.
    pub target: CancellationTargetV1,
    /// Stable reason.
    pub reason_code: String,
    /// Canonical UTC request time.
    pub requested_at_utc: String,
}

/// Cooperative cancellation acknowledgement payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancellationAcknowledgedPayloadV1 {
    /// Referenced cancellation.
    pub cancellation_id: CancellationId,
    /// Exact operation.
    pub operation_id: OperationId,
    /// Exact reservation.
    pub reservation_id: ReservationId,
    /// Approved producer.
    pub producer_revision: RevisionId,
    /// Closed acknowledgement authority (`producer_lease` or `kernel_recovery`).
    pub authority_kind: String,
    /// Canonical UTC acknowledgement time.
    pub acknowledged_at_utc: String,
}

/// Safe late-result audit payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LateResultObservedPayloadV1 {
    /// Terminal operation.
    pub operation_id: OperationId,
    /// Late callback identity.
    pub callback_id: CallbackId,
    /// Canonical callback command fingerprint.
    pub callback_fingerprint: Digest,
    /// Safe classification.
    pub classification: String,
    /// Digest of redacted source bytes.
    pub payload_digest: Digest,
    /// Redaction policy.
    pub redaction_policy_revision: RevisionId,
    /// Canonical UTC receive time.
    pub received_at_utc: String,
}

/// Safe rejected-control-message audit payload.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlMessageRejectedPayloadV1 {
    /// Safe message class.
    pub message_kind: String,
    /// Stable reason.
    pub reason_code: String,
    /// Digest of canonical safe identity fields.
    pub message_digest: Digest,
    /// Canonical UTC rejection time.
    pub rejected_at_utc: String,
}

/// Derived account totals.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlAccountProjectionV1 {
    /// Account definition.
    pub account: BudgetAccountV1,
    /// Live reserved amount.
    pub reserved: BudgetAmountV1,
    /// Gross consumed amount.
    pub gross_consumed: BudgetAmountV1,
    /// Refunded amount.
    pub refunded: BudgetAmountV1,
    /// Net consumed amount.
    pub net_consumed: BudgetAmountV1,
}

/// Derived operation status.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlOperationProjectionV1 {
    /// Complete immutable reservation and authority identity.
    pub reservation: OperationReservedPayloadV1,
    /// Operation identity.
    pub operation_id: OperationId,
    /// Reservation identity.
    pub reservation_id: ReservationId,
    /// Persisted absolute UTC deadline.
    pub absolute_deadline_utc: String,
    /// Interruptibility fixed at reserve.
    pub interruptibility: OperationInterruptibilityV1,
    /// Whether an effective cancellation request covers this operation.
    pub cancellation_requested: bool,
    /// Optional terminal outcome.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub outcome: Option<OperationOutcomeV1>,
    /// Complete terminal settlement when present.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub settlement: Option<OperationSettledPayloadV1>,
    /// Reserved vector.
    pub reserved_usage: Vec<BudgetVectorEntryV1>,
    /// Accounted terminal usage.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
}

/// Complete recoverable cancellation request and acknowledgement state.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlCancellationProjectionV1 {
    /// Durable request.
    pub request: CancellationRequestedPayloadV1,
    /// Sorted operation acknowledgements for this request.
    pub acknowledgements: Vec<CancellationAcknowledgedPayloadV1>,
}

/// Digest preimage for Runtime Control Projection.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlProjectionHashViewV1 {
    /// Projection Schema.
    pub projection_schema_ref: SchemaRef,
    /// Source store identity.
    pub source_store_id: String,
    /// Complete scope.
    pub scope: IsolationScope,
    /// Owner actor.
    pub owner_actor: AgentId,
    /// Control stream.
    pub stream_id: StreamId,
    /// Inclusive cursor.
    pub cursor: EventCursor,
    /// Exact source contract.
    pub source_contract: RuntimeControlSourceContractV1,
    /// Digest of the exact admitted authoritative history.
    pub history_digest: Digest,
    /// Immutable initialization budget identity.
    pub budget_revision: RevisionId,
    /// Immutable initialization clock contract.
    pub clock_contract: RuntimeClockContractV1,
    /// Sorted retained trusted operation contracts.
    pub operation_contracts: Vec<TrustedOperationContractV1>,
    /// Sorted account totals.
    pub accounts: Vec<RuntimeControlAccountProjectionV1>,
    /// Sorted operations.
    pub operations: Vec<RuntimeControlOperationProjectionV1>,
    /// Sorted active grants.
    pub active_grants: Vec<CapabilityGrantV1>,
    /// Sorted revoked grant IDs.
    pub revoked_grants: Vec<CapabilityId>,
    /// Sorted complete cancellation state.
    pub cancellations: Vec<RuntimeControlCancellationProjectionV1>,
    /// Cancellation request count.
    pub cancellation_count: String,
    /// Late-result audit count.
    pub late_result_count: String,
    /// Rejected-message audit count.
    pub rejected_message_count: String,
}

/// Deterministic full-history Runtime Control Projection.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeControlProjectionV1 {
    /// Projection Schema.
    pub schema_ref: SchemaRef,
    /// Source store identity.
    pub source_store_id: String,
    /// Complete scope.
    pub scope: IsolationScope,
    /// Owner actor.
    pub owner_actor: AgentId,
    /// Control stream.
    pub stream_id: StreamId,
    /// Inclusive cursor.
    pub cursor: EventCursor,
    /// Exact source contract.
    pub source_contract: RuntimeControlSourceContractV1,
    /// Digest of the exact admitted authoritative history.
    pub history_digest: Digest,
    /// Immutable initialization budget identity.
    pub budget_revision: RevisionId,
    /// Immutable initialization clock contract.
    pub clock_contract: RuntimeClockContractV1,
    /// Sorted retained trusted operation contracts.
    pub operation_contracts: Vec<TrustedOperationContractV1>,
    /// Sorted account totals.
    pub accounts: Vec<RuntimeControlAccountProjectionV1>,
    /// Sorted operations.
    pub operations: Vec<RuntimeControlOperationProjectionV1>,
    /// Sorted active grants.
    pub active_grants: Vec<CapabilityGrantV1>,
    /// Sorted revoked grant IDs.
    pub revoked_grants: Vec<CapabilityId>,
    /// Sorted complete cancellation state.
    pub cancellations: Vec<RuntimeControlCancellationProjectionV1>,
    /// Cancellation request count.
    pub cancellation_count: String,
    /// Late-result audit count.
    pub late_result_count: String,
    /// Rejected-message audit count.
    pub rejected_message_count: String,
    /// Projection digest.
    pub projection_digest: Digest,
}
