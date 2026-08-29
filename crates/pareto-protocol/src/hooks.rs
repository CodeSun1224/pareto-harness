use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    AgentId, BudgetVectorEntryV1, Digest, EventCursor, EventId, HookDecisionId, HookId,
    HookInvocationId, HookPairId, IsolationScope, OperationId, ProposalId, ReservationId,
    RevisionId, RevisionMetadata, RunId, SchemaRef, SchemaSetRef, StreamId, TaskId,
};

fn deserialize_present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Closed Hook behavior kind.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HookKindV1 {
    /// Read-only observation with no business-decision authority.
    Observer,
    /// Closed allow, deny, or abstain decision component.
    Gate,
    /// Bounded transformation of a non-authoritative proposal.
    Transform,
}

/// Closed lifecycle points admitted by the first Hook contract.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HookPointV1 {
    /// Before a proposal is admitted.
    BeforeProposalAdmission,
    /// After proposal admission is fixed.
    AfterProposalAdmission,
    /// Before an authoritative Kernel commit.
    BeforeAuthoritativeCommit,
    /// After an authoritative Kernel commit.
    AfterAuthoritativeCommit,
}

/// Closed, Kernel-derived phase ordering.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HookPhaseV1 {
    /// Transform phase.
    Transform,
    /// Gate phase.
    Gate,
    /// Observer phase.
    Observer,
}

/// Observer-only failure behavior fixed at registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverFailurePolicyV1 {
    /// Record a safe warning and continue.
    WarnAndContinue,
    /// Fail execution progress without rewriting the fixed business decision.
    FailClosed,
}

/// Closed reason vocabulary for persisted Hook decisions and audit facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookReasonCodeV1 {
    /// Successful point or invocation completion.
    Completed,
    /// A transform completed successfully.
    Transformed,
    /// A gate explicitly allowed the input.
    Allowed,
    /// An observer completed successfully.
    Observed,
    /// A registered policy denied the input.
    PolicyDenied,
    /// A transform handler was unavailable.
    TransformMissing,
    /// A transform output failed validation.
    TransformOutputInvalid,
    /// A transform attempted to change a protected field.
    TransformProtectedField,
    /// A required gate handler was unavailable.
    RequiredGateMissing,
    /// A gate output failed validation.
    GateOutputInvalid,
    /// A gate explicitly denied the input.
    GateDenied,
    /// A required gate abstained.
    RequiredGateAbstained,
    /// A gate-bearing point had no required gate.
    RequiredGateEmpty,
    /// An observer failed with fail-closed policy.
    ObserverFailedClosed,
    /// A handler returned an output for the wrong Hook kind.
    HookKindMismatch,
    /// The handler failed before producing a valid output.
    HandlerFailed,
    /// Cancellation won terminal serialization.
    Cancelled,
    /// Timeout recovery won terminal serialization.
    TimedOut,
    /// Invocation was skipped after transform rejection.
    SkippedAfterTransformFailure,
    /// Invocation was skipped after gate denial.
    SkippedAfterGateDenial,
    /// Invocation was skipped after observer fail-closed.
    SkippedAfterObserverFailure,
    /// A result arrived after a terminal result.
    LateAfterTerminal,
    /// A bounded message failed trusted admission.
    MessageRejected,
}

/// Stable kind discriminator for atomic Runtime Control/Hook pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPairKindV1 {
    /// Budget reservation plus Hook invocation reservation.
    Reserve,
    /// Budget settlement plus Hook invocation terminal.
    Terminal,
}

/// Closed resource limits applied before semantic output validation.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookLimitsV1 {
    /// Maximum encoded input bytes.
    pub max_input_bytes: u64,
    /// Maximum encoded output bytes.
    pub max_output_bytes: u64,
    /// Maximum JSON nesting depth.
    pub max_depth: u32,
    /// Maximum collection entries.
    pub max_collection_items: u32,
}

/// Versioned transform mask over non-authoritative proposal fields.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformContractV1 {
    /// Stable, explicit field identifiers; wildcards are forbidden by semantic validation.
    pub allowed_fields: Vec<String>,
    /// Closed schema for transformed proposal fields.
    pub field_schema_ref: SchemaRef,
    /// Schema for the protected-field hash view.
    pub protected_hash_view_schema_ref: SchemaRef,
}

/// One exact Hook registration pinned by a Run Manifest registry revision.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookRegistrationV1 {
    /// Stable logical Hook identity.
    pub hook_id: HookId,
    /// Immutable Hook implementation/configuration revision.
    pub hook_revision: RevisionId,
    /// Exact configuration digest.
    pub config_digest: Digest,
    /// Closed behavior kind.
    pub kind: HookKindV1,
    /// Non-empty, unique allowed Hook points.
    pub hook_points: Vec<HookPointV1>,
    /// Stable ascending phase-local priority.
    pub priority: i32,
    /// Gate-only required marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Observer-only failure policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observer_failure_policy: Option<ObserverFailurePolicyV1>,
    /// Transform-only field contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_contract: Option<TransformContractV1>,
    /// Retained trusted operation contract revision.
    pub resource_contract_revision: RevisionId,
    /// Exact request schema.
    pub input_schema_ref: SchemaRef,
    /// Exact result schema.
    pub output_schema_ref: SchemaRef,
    /// Closed invocation limits.
    pub limits: HookLimitsV1,
    /// Versioned redaction policy revision.
    pub redaction_policy_revision: RevisionId,
    /// Exact in-process reference handler compatibility identity.
    pub handler_compatibility_digest: Digest,
}

impl HookRegistrationV1 {
    /// Validates kind-specific fields and the first Hook point matrix.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        let unique: std::collections::BTreeSet<_> = self.hook_points.iter().collect();
        let point_valid = !self.hook_points.is_empty()
            && unique.len() == self.hook_points.len()
            && self.hook_points.windows(2).all(|pair| pair[0] < pair[1])
            && self.hook_points.iter().all(|point| {
                matches!(
                    (self.kind, point),
                    (HookKindV1::Observer, _)
                        | (HookKindV1::Gate, HookPointV1::BeforeProposalAdmission)
                        | (HookKindV1::Gate, HookPointV1::BeforeAuthoritativeCommit)
                        | (HookKindV1::Transform, HookPointV1::BeforeProposalAdmission)
                )
            });
        let fields_valid = match self.kind {
            HookKindV1::Observer => {
                self.required.is_none()
                    && self.observer_failure_policy.is_some()
                    && self.transform_contract.is_none()
            }
            HookKindV1::Gate => {
                self.required.is_some()
                    && self.observer_failure_policy.is_none()
                    && self.transform_contract.is_none()
            }
            HookKindV1::Transform => {
                self.required.is_none()
                    && self.observer_failure_policy.is_none()
                    && self.transform_contract.as_ref().is_some_and(|contract| {
                        !contract.allowed_fields.is_empty()
                            && contract.allowed_fields.iter().all(|field| {
                                !field.is_empty() && !field.contains('*') && field.starts_with('/')
                            })
                            && contract
                                .allowed_fields
                                .windows(2)
                                .all(|pair| pair[0] < pair[1])
                    })
            }
        };
        if point_valid && fields_valid {
            Ok(())
        } else {
            Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/hook_registration".to_owned(),
                contract: "hook_registration_v1".to_owned(),
                detail: "Hook kind, point, or kind-specific fields are invalid".to_owned(),
            })
        }
    }
}

/// Immutable content-addressed Hook registry revision.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookRegistryRevisionV1 {
    /// Immutable revision metadata.
    pub metadata: RevisionMetadata,
    /// Digest of the complete ordered registry configuration.
    pub config_digest: Digest,
    /// Canonically ordered exact registrations.
    pub registrations: Vec<HookRegistrationV1>,
}

impl HookRegistryRevisionV1 {
    /// Validates immutable metadata, registrations, uniqueness, and canonical registry order.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.metadata.revision_kind != "hook_registry"
            || self.metadata.validate_identity().is_err()
            || self.registrations.is_empty()
            || self
                .registrations
                .iter()
                .any(|registration| registration.validate().is_err())
        {
            return Err(hook_registry_error());
        }
        let unique: std::collections::BTreeSet<_> = self
            .registrations
            .iter()
            .map(|registration| (&registration.hook_id, &registration.hook_revision))
            .collect();
        if unique.len() != self.registrations.len()
            || self
                .registrations
                .windows(2)
                .any(|pair| registration_sort_key(&pair[0]) >= registration_sort_key(&pair[1]))
        {
            return Err(hook_registry_error());
        }
        Ok(())
    }
}

fn registration_sort_key(
    registration: &HookRegistrationV1,
) -> (&[HookPointV1], HookPhaseV1, i32, &HookId, &RevisionId) {
    let phase = match registration.kind {
        HookKindV1::Transform => HookPhaseV1::Transform,
        HookKindV1::Gate => HookPhaseV1::Gate,
        HookKindV1::Observer => HookPhaseV1::Observer,
    };
    (
        &registration.hook_points,
        phase,
        registration.priority,
        &registration.hook_id,
        &registration.hook_revision,
    )
}

fn hook_registry_error() -> crate::ValidationError {
    crate::ValidationError {
        code: crate::ErrorCode::InvariantViolation,
        path: "/hook_registry".to_owned(),
        contract: "hook_registry_revision_v1".to_owned(),
        detail: "Hook registry metadata, uniqueness, or canonical order is invalid".to_owned(),
    }
}

/// Scope and lineage key for one deterministic Hook invocation.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookInvocationKeyV1 {
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Exact owning Task when the point is Task-scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    /// Hook point.
    pub hook_point: HookPointV1,
    /// Kernel-derived phase.
    pub phase: HookPhaseV1,
    /// Registered Hook identity.
    pub hook_id: HookId,
    /// Registered immutable revision.
    pub hook_revision: RevisionId,
    /// Non-authoritative subject proposal.
    pub subject_proposal_id: ProposalId,
    /// Stable phase-local ordinal.
    pub ordinal: u32,
    /// Exact source Event cursor.
    pub source_cursor: EventCursor,
    /// Digest of bytes delivered to this invocation.
    pub input_digest: Digest,
    /// Previous verified Transform output, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_output_digest: Option<Digest>,
    /// Stable retry attempt.
    pub attempt: u32,
}

/// Non-authoritative proposal accepted by Transform handlers.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformProposalV1 {
    /// Stable proposal identity.
    pub proposal_id: ProposalId,
    /// Exact proposal schema.
    pub schema_ref: SchemaRef,
    /// Proposal fields; Kernel validation treats all output as untrusted.
    pub fields: Value,
}

/// Closed primitive value admitted by the first Transform field contract.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransformFieldValueV1 {
    /// UTF-8 text value.
    Text(String),
    /// Signed integer value.
    Integer(i64),
    /// Boolean value.
    Boolean(bool),
}

/// Versioned, bounded request view delivered to a Hook handler.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookRequestViewV1 {
    /// Hook point being executed.
    pub hook_point: HookPointV1,
    /// Kernel-derived phase.
    pub phase: HookPhaseV1,
    /// Exact digest of the delivered proposal bytes.
    pub input_digest: Digest,
    /// Non-authoritative proposal view.
    pub proposal: TransformProposalV1,
    /// Business decision fixed before Observer dispatch, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_business_decision: Option<HookBusinessDecisionV1>,
}

/// Protected proposal identity compared before and after every Transform.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedProposalHashViewV1 {
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Proposal identity.
    pub proposal_id: ProposalId,
    /// Manifest-pinned schema set.
    pub schema_set_ref: SchemaSetRef,
    /// Manifest-pinned Hook registry revision.
    pub hook_registry_revision: RevisionId,
    /// Digest binding all authority, budget, deadline, cancellation, and terminal fields.
    pub authority_digest: Digest,
    /// Digest binding every unknown field.
    pub unknown_fields_digest: Digest,
}

/// Closed Gate component result.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum GateDecisionV1 {
    /// Explicit allow.
    Allow {},
    /// Explicit deny with a stable safe reason.
    Deny {
        /// Stable reason code.
        reason_code: HookReasonCodeV1,
    },
    /// No opinion; never equivalent to required allow.
    Abstain {},
}

/// Closed Observer result without business-decision authority.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObserverResultV1 {
    /// Observation succeeded.
    Observed {
        /// Digest of the bounded, redacted annotation.
        annotation_digest: Digest,
    },
    /// Safe warning.
    Warning {
        /// Stable warning reason.
        reason_code: HookReasonCodeV1,
        /// Digest of the bounded, redacted annotation.
        annotation_digest: Digest,
    },
    /// Observer execution failed.
    Failure {
        /// Stable failure reason.
        reason_code: HookReasonCodeV1,
    },
}

/// Immutable point business decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookBusinessDecisionV1 {
    /// Proposal or commit is allowed.
    Allow,
    /// Proposal or commit is denied.
    Deny,
    /// Observer-only point has no admission decision.
    ObserveOnly,
}

/// Execution state kept separate from the immutable business decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookExecutionStatusV1 {
    /// All required phases completed.
    Completed,
    /// Observer fail-closed prevented downstream progress.
    ObserverFailed,
    /// Transform failure rejected the whole proposal.
    TransformFailed,
    /// Gate composition denied progress.
    GateDenied,
}

/// Stable identity shared by one atomic control and Hook Event pair.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookPairBindingV1 {
    /// Stable pair identity.
    pub pair_id: HookPairId,
    /// Closed pair kind, preventing reserve/terminal cross-kind reuse.
    pub pair_kind: HookPairKindV1,
    /// Canonical full-command fingerprint.
    pub pair_fingerprint: Digest,
    /// Counterpart Runtime Control Event.
    pub control_event_id: EventId,
    /// Hook Event in this pair.
    pub hook_event_id: EventId,
    /// Protected operation identity.
    pub operation_id: OperationId,
    /// Budget reservation identity.
    pub reservation_id: ReservationId,
    /// Hook invocation identity.
    pub invocation_id: HookInvocationId,
}

/// Sequence-one Hook stream source contract.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookStreamInitializedPayloadV1 {
    /// Source Run fixed by the sequence-one contract.
    pub source_run_id: RunId,
    /// Exact Manifest-pinned registry revision.
    pub hook_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub hook_registry_config_digest: Digest,
    /// Exact source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
}

/// Hook point start and initial lineage.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookPointStartedPayloadV1 {
    /// Stable point execution identity.
    pub point_id: HookDecisionId,
    /// Closed Hook point.
    pub hook_point: HookPointV1,
    /// Subject proposal.
    pub subject_proposal_id: ProposalId,
    /// Source Event cursor.
    pub source_cursor: EventCursor,
    /// Initial input digest.
    pub initial_input_digest: Digest,
    /// Deterministic complete invocation order.
    pub ordered_invocations: Vec<HookInvocationId>,
}

/// Atomic reservation-side Hook fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookInvocationReservedPayloadV1 {
    /// Stable invocation identity.
    pub invocation_id: HookInvocationId,
    /// Complete deterministic invocation key.
    pub key: HookInvocationKeyV1,
    /// Atomic control/Hook pair binding.
    pub pair: HookPairBindingV1,
    /// Full reserved resource vector.
    pub reserved_usage: Vec<BudgetVectorEntryV1>,
}

/// Closed terminal state of one invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookInvocationTerminalStateV1 {
    /// Handler output succeeded and was validated.
    Succeeded,
    /// Handler or validation failed.
    Failed,
    /// Cancellation won terminal serialization.
    Cancelled,
    /// Deadline recovery won terminal serialization.
    TimedOut,
}

/// Atomic settlement-side Hook result and component decision.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookInvocationTerminalPayloadV1 {
    /// Stable invocation identity.
    pub invocation_id: HookInvocationId,
    /// Kernel-derived component decision identity.
    pub decision_id: HookDecisionId,
    /// Unique terminal state.
    pub terminal_state: HookInvocationTerminalStateV1,
    /// Atomic settlement/Hook pair binding.
    pub pair: HookPairBindingV1,
    /// Safe digest of validated output, when present.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub output_digest: Option<Digest>,
    /// Gate-only closed result.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub gate_decision: Option<GateDecisionV1>,
    /// Observer-only non-authoritative result.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub observer_result: Option<ObserverResultV1>,
    /// Kernel-accounted usage.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
    /// Stable safe terminal reason.
    pub reason_code: HookReasonCodeV1,
}

/// Canonical skipped invocation audit fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookInvocationSkippedPayloadV1 {
    /// Invocation not dispatched.
    pub invocation_id: HookInvocationId,
    /// Closed Hook point.
    pub hook_point: HookPointV1,
    /// Kernel-derived phase.
    pub phase: HookPhaseV1,
    /// Stable skip reason.
    pub reason_code: HookReasonCodeV1,
    /// Input digest that would have been delivered.
    pub input_digest: Digest,
}

/// Final immutable point decision and separate execution status.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookPointFinalizedPayloadV1 {
    /// Stable point execution identity.
    pub point_id: HookDecisionId,
    /// Closed Hook point.
    pub hook_point: HookPointV1,
    /// Exact source cursor.
    pub source_cursor: EventCursor,
    /// Point initial input digest.
    pub initial_input_digest: Digest,
    /// Final validated Transform input digest.
    pub final_input_digest: Digest,
    /// Complete deterministic invocation order.
    pub ordered_invocations: Vec<HookInvocationId>,
    /// Ordered component decisions.
    pub ordered_component_decisions: Vec<HookDecisionId>,
    /// Canonically skipped invocations.
    pub skipped_invocations: Vec<HookInvocationId>,
    /// Immutable business decision.
    pub business_decision: HookBusinessDecisionV1,
    /// Separate execution status.
    pub execution_status: HookExecutionStatusV1,
    /// Stable safe final reason.
    pub reason_code: HookReasonCodeV1,
}

/// Safe late-result audit fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookLateResultObservedPayloadV1 {
    /// Original invocation identity.
    pub invocation_id: HookInvocationId,
    /// Exact registered Hook.
    pub hook_id: HookId,
    /// Exact registered revision.
    pub hook_revision: RevisionId,
    /// Exact attempt.
    pub attempt: u32,
    /// Safe output digest only.
    pub output_digest: Digest,
    /// Stable late-result reason.
    pub reason_code: HookReasonCodeV1,
    /// Applied redaction policy.
    pub redaction_policy_revision: RevisionId,
}

/// Safe default-deny rejection fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookMessageRejectedPayloadV1 {
    /// Kernel-derived rejection decision.
    pub decision_id: HookDecisionId,
    /// Closed Hook point.
    pub hook_point: HookPointV1,
    /// Hook identity when safely known.
    pub hook_id: Option<HookId>,
    /// Hook revision when safely known.
    pub hook_revision: Option<RevisionId>,
    /// Stable rejection reason.
    pub reason_code: HookReasonCodeV1,
    /// Safe proposal identity.
    pub safe_subject_id: ProposalId,
    /// Safe input digest.
    pub input_digest: Digest,
    /// Exact registry revision.
    pub hook_registry_revision: RevisionId,
    /// Exact source cursor.
    pub source_cursor: EventCursor,
    /// Applied redaction policy.
    pub redaction_policy_revision: RevisionId,
}

/// One folded Hook invocation projection entry.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookInvocationProjectionEntryV1 {
    /// Stable invocation identity.
    pub invocation_id: HookInvocationId,
    /// Complete invocation lineage key.
    pub key: HookInvocationKeyV1,
    /// Bound protected operation.
    pub operation_id: OperationId,
    /// Bound budget reservation.
    pub reservation_id: ReservationId,
    /// Unique terminal when settled.
    pub terminal_state: Option<HookInvocationTerminalStateV1>,
    /// Component decision when terminal.
    pub decision_id: Option<HookDecisionId>,
}

/// Pure-folded Hook stream projection.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookProjectionV1 {
    /// Source Event Store identity.
    pub source_store_id: String,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Manifest owner and Kernel signer.
    pub owner_actor: AgentId,
    /// Derived Hook stream.
    pub hook_stream_id: StreamId,
    /// Inclusive folded cursor.
    pub inclusive_cursor: EventCursor,
    /// Manifest-pinned source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Manifest-pinned Hook registry.
    pub hook_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub hook_registry_config_digest: Digest,
    /// Exact reducer revision.
    pub reducer_revision: RevisionId,
    /// Continuous history digest.
    pub history_digest: Digest,
    /// Sorted invocation state.
    pub invocations: Vec<HookInvocationProjectionEntryV1>,
    /// Sorted finalized point decisions.
    pub finalized_points: Vec<HookDecisionId>,
    /// Folded skip count.
    pub skipped_count: u64,
    /// Folded late-result count.
    pub late_result_count: u64,
    /// Folded rejection count.
    pub rejected_count: u64,
    /// Digest of the complete projection hash view.
    pub projection_digest: Digest,
}

/// Projection fields covered by the projection digest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookProjectionHashViewV1 {
    /// Source Event Store identity.
    pub source_store_id: String,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Manifest owner and Kernel signer.
    pub owner_actor: AgentId,
    /// Derived Hook stream.
    pub hook_stream_id: StreamId,
    /// Inclusive folded cursor.
    pub inclusive_cursor: EventCursor,
    /// Manifest-pinned source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Manifest-pinned Hook registry.
    pub hook_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub hook_registry_config_digest: Digest,
    /// Exact reducer revision.
    pub reducer_revision: RevisionId,
    /// Continuous history digest.
    pub history_digest: Digest,
    /// Sorted invocation state.
    pub invocations: Vec<HookInvocationProjectionEntryV1>,
    /// Sorted finalized point decisions.
    pub finalized_points: Vec<HookDecisionId>,
    /// Folded skip count.
    pub skipped_count: u64,
    /// Folded late-result count.
    pub late_result_count: u64,
    /// Folded rejection count.
    pub rejected_count: u64,
}
