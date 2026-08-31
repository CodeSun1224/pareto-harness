use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    AgentId, BudgetVectorEntryV1, Digest, EffectAttemptId, EffectId, EffectPairId, EventCursor,
    EventId, IsolationScope, OperationId, ReservationId, RevisionId, RevisionMetadata, RunId,
    SchemaRef, SchemaSetRef, StreamId, TaskId,
};

/// Closed external idempotency behavior declared by an Effect registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectIdempotencyPolicyV1 {
    /// The external boundary exposes no idempotency key contract.
    Unsupported,
    /// The external boundary accepts a stable idempotency key.
    Keyed,
}

/// Closed policy for an outcome that cannot be proven.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectUnknownOutcomePolicyV1 {
    /// Preserve unknown and require reconciliation; never redispatch automatically.
    ReconcileOnly,
}

/// Closed resource limits for one Effect registration.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectLimitsV1 {
    /// Maximum canonical request bytes.
    pub max_request_bytes: u64,
    /// Maximum canonical Receipt observation bytes.
    pub max_receipt_bytes: u64,
    /// Maximum bounded result-summary bytes.
    pub max_result_summary_bytes: u64,
    /// Maximum number of limitation codes.
    pub max_limitations: u32,
}

/// One exact Effect registration pinned by a Run Manifest registry revision.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRegistrationV1 {
    /// Stable closed Effect kind.
    pub effect_kind: String,
    /// Immutable behavior revision.
    pub effect_revision: RevisionId,
    /// Immutable executor descriptor revision.
    pub executor_revision: RevisionId,
    /// Canonical executor descriptor content digest.
    pub executor_descriptor_digest: Digest,
    /// Exact executor configuration digest.
    pub executor_config_digest: Digest,
    /// Manifest-pinned Receipt admission adapter revision.
    pub adapter_revision: RevisionId,
    /// Manifest-pinned Receipt producer revision.
    pub producer_revision: RevisionId,
    /// Retained trusted operation contract revision.
    pub operation_contract_revision: RevisionId,
    /// Exact request schema.
    pub request_schema_ref: SchemaRef,
    /// Exact Receipt observation schema.
    pub receipt_schema_ref: SchemaRef,
    /// External idempotency contract.
    pub idempotency_policy: EffectIdempotencyPolicyV1,
    /// Unknown-outcome handling contract.
    pub unknown_outcome_policy: EffectUnknownOutcomePolicyV1,
    /// Exact reconciliation policy revision.
    pub reconciliation_policy_revision: RevisionId,
    /// Exact redaction policy revision.
    pub redaction_policy_revision: RevisionId,
    /// Closed resource limits.
    pub limits: EffectLimitsV1,
}

impl EffectRegistrationV1 {
    /// Validates the stable kind and non-zero closed limits.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        let kind_valid = !self.effect_kind.is_empty()
            && self.effect_kind.len() <= 128
            && self.effect_kind.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            });
        let limits_valid = self.limits.max_request_bytes > 0
            && self.limits.max_receipt_bytes > 0
            && self.limits.max_result_summary_bytes > 0
            && self.limits.max_limitations > 0;
        if kind_valid && limits_valid {
            Ok(())
        } else {
            Err(effect_contract_error(
                "/effect_registration",
                "Effect kind or limits are invalid",
            ))
        }
    }
}

/// Immutable content-addressed Effect registry revision.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRegistryRevisionV1 {
    /// Immutable revision metadata.
    pub metadata: RevisionMetadata,
    /// Digest of the complete ordered registry configuration.
    pub config_digest: Digest,
    /// Canonically ordered registrations.
    pub registrations: Vec<EffectRegistrationV1>,
}

impl EffectRegistryRevisionV1 {
    /// Validates metadata, registration semantics, uniqueness, and canonical order.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        let valid = self.metadata.revision_kind == "effect_registry"
            && self.metadata.validate_identity().is_ok()
            && !self.registrations.is_empty()
            && self
                .registrations
                .iter()
                .all(|registration| registration.validate().is_ok())
            && self
                .registrations
                .windows(2)
                .all(|pair| pair[0].effect_kind < pair[1].effect_kind);
        if valid {
            Ok(())
        } else {
            Err(effect_contract_error(
                "/effect_registry",
                "Effect registry metadata, entries, uniqueness, or order is invalid",
            ))
        }
    }
}

/// Frozen content preimage for an executor descriptor revision.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectExecutorDescriptorHashViewV1 {
    /// Exact Receipt adapter revision.
    pub adapter_revision: RevisionId,
    /// Exact Receipt producer revision.
    pub producer_revision: RevisionId,
    /// Exact request schema.
    pub request_schema_ref: SchemaRef,
    /// Exact Receipt schema.
    pub receipt_schema_ref: SchemaRef,
    /// Exact executor configuration digest.
    pub config_digest: Digest,
    /// Exact resource contract revision.
    pub resource_contract_revision: RevisionId,
    /// Exact trusted meter contract revision.
    pub meter_contract_revision: RevisionId,
    /// Exact recovery contract revision.
    pub recovery_contract_revision: RevisionId,
    /// In-process reference implementation compatibility digest.
    pub implementation_compatibility_digest: Digest,
}

/// Immutable content-addressed executor descriptor.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectExecutorDescriptorV1 {
    /// Immutable revision metadata with `effect_executor` kind.
    pub metadata: RevisionMetadata,
    /// Exact hash-view schema.
    pub hash_schema_ref: SchemaRef,
    /// Frozen descriptor content.
    pub content: EffectExecutorDescriptorHashViewV1,
}

impl EffectExecutorDescriptorV1 {
    /// Computes the descriptor content digest.
    pub fn content_digest(&self) -> Result<Digest, crate::ValidationError> {
        let value = serde_json::to_value(&self.content).map_err(|_| {
            effect_contract_error("/content", "executor descriptor serialization failed")
        })?;
        crate::digest_json("revision:effect_executor", &self.hash_schema_ref, &value)
    }

    /// Validates immutable descriptor identity and content.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.metadata.revision_kind == "effect_executor"
            && self.hash_schema_ref.r#type == "effect-executor-descriptor-hash-view"
            && self.content_digest()? == self.metadata.content_digest
            && self.metadata.validate_identity().is_ok()
        {
            Ok(())
        } else {
            Err(effect_contract_error(
                "/effect_executor_descriptor",
                "executor descriptor revision or content identity is invalid",
            ))
        }
    }
}

/// Public, non-authoritative request for one protected Effect.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRequestV1 {
    /// Requested registered Effect kind.
    pub effect_kind: String,
    /// Authenticated or delegated Effect subject.
    pub subject_actor: AgentId,
    /// Exact Task when Task-scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    /// Exact schema for the normalized request value.
    pub request_schema_ref: SchemaRef,
    /// Normalized, non-authoritative request value.
    pub request: Value,
    /// Kernel-admitted digest of the client idempotency key.
    pub client_idempotency_key_digest: Digest,
    /// Canonical absolute deadline.
    pub deadline_at: String,
    /// Safe caller correlation value.
    pub correlation_id: String,
}

/// Closed dispatch position in the Effect state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectDispatchStateV1 {
    /// Intent exists and no dispatch lease has been delivered.
    Intended,
    /// The dispatch boundary has been claimed.
    Claimed,
    /// The attempt has a unique authoritative conclusion.
    Concluded,
}

/// Closed external conclusion preserved independently from operation status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectExternalConclusionV1 {
    /// No authoritative external conclusion exists yet.
    Pending,
    /// The external effect was applied.
    Applied,
    /// The external effect was proven not applied.
    NotApplied,
    /// Only a bounded subset was proven applied.
    Partial,
    /// Whether the external effect applied cannot be proven.
    Unknown,
}

/// Closed reconciliation axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectReconciliationStateV1 {
    /// The authoritative conclusion needs no reconciliation.
    NotRequired,
    /// Reconciliation is open.
    Required,
    /// Reconciliation resolved the external world as applied.
    ResolvedApplied,
    /// Reconciliation resolved the external world as not applied.
    ResolvedNotApplied,
    /// Reconciliation resolved a partial application.
    ResolvedPartial,
}

/// Closed class carried by an untrusted Receipt observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectReceiptOutcomeClassV1 {
    /// The external effect reports application.
    Applied,
    /// The external system reports rejection before application.
    RejectedBeforeApply,
    /// The external system reports partial application.
    Partial,
    /// The external outcome is unknown.
    Unknown,
}

/// Closed cause for explicit Effect recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectRecoveryCauseV1 {
    /// The Intent or claim process epoch is no longer live.
    ProcessEpochLost,
    /// The absolute deadline is due.
    DeadlineDue,
    /// Cancellation is authoritative and effective.
    CancellationEffective,
}

/// Closed Effect atomic-pair kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectPairKindV1 {
    /// Runtime Control reservation paired with an Effect Intent.
    ReserveIntent,
    /// Runtime Control settlement paired with an authoritative Effect conclusion.
    TerminalConclusion,
}

/// Stable identity shared by one atomic control and Effect Event pair.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPairBindingV1 {
    /// Stable pair identity.
    pub pair_id: EffectPairId,
    /// Closed pair kind.
    pub pair_kind: EffectPairKindV1,
    /// Canonical full-command fingerprint.
    pub pair_fingerprint: Digest,
    /// Counterpart Runtime Control Event.
    pub control_event_id: EventId,
    /// Effect Event in this pair.
    pub effect_event_id: EventId,
    /// Protected operation identity.
    pub operation_id: OperationId,
    /// Budget reservation identity.
    pub reservation_id: ReservationId,
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Digest of the canonical prepared Runtime Control Event with recursive pair seals cleared.
    pub control_prepared_digest: Digest,
    /// Digest of the canonical prepared Effect Event with recursive pair seals cleared.
    pub effect_prepared_digest: Digest,
}

/// Recovery identity available before a dispatch claim exists.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRecoveryBaseKeyV1 {
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Protected operation identity.
    pub operation_id: OperationId,
    /// Budget reservation identity.
    pub reservation_id: ReservationId,
    /// Exact executor revision.
    pub executor_revision: RevisionId,
    /// Exact executor descriptor digest.
    pub executor_descriptor_digest: Digest,
    /// Exact executor configuration digest.
    pub executor_config_digest: Digest,
    /// Exact source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Exact trusted meter contract revision.
    pub meter_contract_revision: RevisionId,
    /// Exact recovery contract revision.
    pub recovery_contract_revision: RevisionId,
    /// Initial process epoch digest.
    pub initial_process_epoch_digest: Digest,
    /// Canonical absolute deadline.
    pub deadline_at: String,
}

/// Recovery identity extended at the durable dispatch claim boundary.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectRecoveryKeyV1 {
    /// Complete pre-claim recovery identity.
    pub base: EffectRecoveryBaseKeyV1,
    /// Durable claim Event.
    pub claim_event_id: EventId,
    /// Digest of the exact claim Event bytes.
    pub claim_event_digest: Digest,
    /// Claim process epoch digest.
    pub claim_process_epoch_digest: Digest,
    /// Claim Clock sample digest.
    pub claim_clock_digest: Digest,
    /// External idempotency identity digest.
    pub external_key_digest: Digest,
    /// Exact claim policy revision.
    pub claim_policy_revision: RevisionId,
}

/// Sequence-one source contract for one derived Effect stream.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectStreamInitializedPayloadV1 {
    /// Source Run fixed by the sequence-one contract.
    pub source_run_id: RunId,
    /// Lifecycle cursor used to admit initialization.
    pub lifecycle_cursor: EventCursor,
    /// Manifest-pinned Effect registry revision.
    pub effect_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub effect_registry_config_digest: Digest,
    /// Exact boundary recording policy revision.
    pub boundary_recording_policy_revision: RevisionId,
    /// Exact source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Exact protocol-limits profile digest.
    pub protocol_limits_digest: Digest,
    /// Exact Effect reducer revision.
    pub reducer_revision: RevisionId,
    /// Exact Effect output reader revision.
    pub output_reader_revision: RevisionId,
    /// Exact history digest revision.
    pub history_digest_revision: RevisionId,
}

/// Atomic reservation-side Effect Intent fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectIntendedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable first attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Safe requested Effect kind.
    pub effect_kind: String,
    /// Exact subject.
    pub subject_actor: AgentId,
    /// Exact Task when Task-scoped.
    pub task_id: Option<TaskId>,
    /// Canonical request digest.
    pub request_digest: Digest,
    /// Kernel-admitted client key digest.
    pub idempotency_key_digest: Digest,
    /// Exact registry revision.
    pub effect_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub effect_registry_config_digest: Digest,
    /// Exact Effect behavior revision.
    pub effect_revision: RevisionId,
    /// Exact executor revision.
    pub executor_revision: RevisionId,
    /// Exact executor descriptor digest.
    pub executor_descriptor_digest: Digest,
    /// Exact executor configuration digest.
    pub executor_config_digest: Digest,
    /// Atomic reserve/Intent pair.
    pub pair: EffectPairBindingV1,
    /// Full reserved resource vector.
    pub reserved_usage: Vec<BudgetVectorEntryV1>,
    /// Complete pre-claim recovery identity.
    pub recovery_base_key: EffectRecoveryBaseKeyV1,
}

/// Durable claim fact immediately before the external dispatch boundary.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDispatchClaimedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Canonical request digest.
    pub request_digest: Digest,
    /// External idempotency identity digest.
    pub external_key_digest: Digest,
    /// Exact executor revision.
    pub executor_revision: RevisionId,
    /// Exact descriptor digest.
    pub executor_descriptor_digest: Digest,
    /// Exact executor configuration digest.
    pub executor_config_digest: Digest,
    /// Full post-claim recovery identity.
    pub recovery_key: EffectRecoveryKeyV1,
}

/// Untrusted external result observation before Kernel admission.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectReceiptObservationV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// External idempotency identity digest.
    pub external_key_digest: Digest,
    /// Manifest-pinned producer revision.
    pub producer_revision: RevisionId,
    /// Manifest-pinned adapter revision.
    pub adapter_revision: RevisionId,
    /// Closed observed outcome.
    pub outcome_class: EffectReceiptOutcomeClassV1,
    /// Canonical observation time.
    pub observed_at: String,
    /// Digest of the bounded safe Receipt representation.
    pub receipt_digest: Digest,
    /// Digest of the bounded safe result summary.
    pub result_digest: Digest,
    /// Externally observed, non-authoritative usage.
    pub observed_usage: Vec<BudgetVectorEntryV1>,
    /// Sorted stable limitation codes.
    pub limitations: Vec<String>,
}

/// Atomic settlement-side admitted Receipt fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectReceiptAdmittedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Exact producer revision.
    pub producer_revision: RevisionId,
    /// Exact adapter revision.
    pub adapter_revision: RevisionId,
    /// Closed admitted external conclusion.
    pub external_conclusion: EffectExternalConclusionV1,
    /// Safe Receipt digest.
    pub receipt_digest: Digest,
    /// Safe result digest.
    pub result_digest: Digest,
    /// Kernel-accounted usage.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
    /// Sorted stable limitations.
    pub limitations: Vec<String>,
    /// Atomic settlement/conclusion pair.
    pub pair: EffectPairBindingV1,
}

/// Atomic no-Receipt conclusion for one Effect attempt.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectAttemptConcludedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Closed authoritative external conclusion.
    pub external_conclusion: EffectExternalConclusionV1,
    /// Stable safe reason code.
    pub reason_code: String,
    /// Kernel-accounted usage.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
    /// Atomic settlement/conclusion pair.
    pub pair: EffectPairBindingV1,
}

/// Atomic conclusion opening reconciliation for partial or unknown application.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectReconciliationRequiredPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Partial or unknown authoritative conclusion.
    pub external_conclusion: EffectExternalConclusionV1,
    /// Stable safe reason code.
    pub reason_code: String,
    /// Kernel-accounted conservative usage.
    pub accounted_usage: Vec<BudgetVectorEntryV1>,
    /// Safe admitted Receipt digest when the conclusion came from an observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<Digest>,
    /// Sorted stable limitation codes retained for inventory and reconciliation.
    pub limitations: Vec<String>,
    /// Digest of confirmed external components when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_components_digest: Option<Digest>,
    /// Digest of unknown external components.
    pub unknown_components_digest: Digest,
    /// Atomic settlement/conclusion pair.
    pub pair: EffectPairBindingV1,
}

/// Safe admitted observation used by an explicit reconciliation command.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectReconciliationObservedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Exact reconciliation producer revision.
    pub producer_revision: RevisionId,
    /// Source observation Events.
    pub source_observation_event_ids: Vec<EventId>,
    /// Canonical evidence fingerprint.
    pub evidence_fingerprint: Digest,
}

/// Append-only resolution of an open reconciliation axis.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectReconciledPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Closed resolved reconciliation state.
    pub reconciliation_state: EffectReconciliationStateV1,
    /// Source admitted observation Event.
    pub source_observation_event_id: EventId,
    /// Canonical evidence fingerprint.
    pub evidence_fingerprint: Digest,
}

/// Safe audit fact for a Receipt arriving after an authoritative conclusion.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectLateReceiptObservedPayloadV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Safe Receipt digest.
    pub receipt_digest: Digest,
    /// Exact producer revision when safely known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_revision: Option<RevisionId>,
    /// Stable safe reason code.
    pub reason_code: String,
    /// Exact redaction policy revision.
    pub redaction_policy_revision: RevisionId,
}

/// Safe default-deny rejection fact.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectMessageRejectedPayloadV1 {
    /// Safe Effect identity when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_id: Option<EffectId>,
    /// Safe attempt identity when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<EffectAttemptId>,
    /// Safe requested kind when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_kind: Option<String>,
    /// Stable safe reason code.
    pub reason_code: String,
    /// Safe input digest.
    pub input_digest: Digest,
    /// Exact registry revision.
    pub effect_registry_revision: RevisionId,
    /// Exact redaction policy revision.
    pub redaction_policy_revision: RevisionId,
}

/// One folded Effect attempt projection entry.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectProjectionEntryV1 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// Safe Effect kind.
    pub effect_kind: String,
    /// Canonical request digest.
    pub request_digest: Digest,
    /// Kernel-admitted client key digest.
    pub idempotency_key_digest: Digest,
    /// Protected operation identity.
    pub operation_id: OperationId,
    /// Budget reservation identity.
    pub reservation_id: ReservationId,
    /// Exact executor revision.
    pub executor_revision: RevisionId,
    /// Exact executor descriptor digest.
    pub executor_descriptor_digest: Digest,
    /// Dispatch axis.
    pub dispatch_state: EffectDispatchStateV1,
    /// External conclusion axis.
    pub external_conclusion: EffectExternalConclusionV1,
    /// Reconciliation axis.
    pub reconciliation_state: EffectReconciliationStateV1,
    /// Full claim recovery identity when claimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_key: Option<EffectRecoveryKeyV1>,
    /// Admitted Receipt digest when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<Digest>,
}

/// Pure-folded Effect stream projection.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectProjectionV1 {
    /// Source Event Store identity.
    pub source_store_id: String,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Manifest owner and Kernel signer.
    pub owner_actor: AgentId,
    /// Derived Effect stream.
    pub effect_stream_id: StreamId,
    /// Inclusive folded cursor.
    pub inclusive_cursor: EventCursor,
    /// Manifest-pinned source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Manifest-pinned Effect registry revision.
    pub effect_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub effect_registry_config_digest: Digest,
    /// Exact reducer revision.
    pub reducer_revision: RevisionId,
    /// Continuous history digest.
    pub history_digest: Digest,
    /// Effect entries sorted by stable identity.
    pub effects: Vec<EffectProjectionEntryV1>,
    /// Folded late Receipt count.
    pub late_receipt_count: u64,
    /// Folded rejected-message count.
    pub rejected_count: u64,
    /// Digest of the complete projection hash view.
    pub projection_digest: Digest,
}

/// Projection fields covered by the projection digest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectProjectionHashViewV1 {
    /// Source Event Store identity.
    pub source_store_id: String,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Manifest owner and Kernel signer.
    pub owner_actor: AgentId,
    /// Derived Effect stream.
    pub effect_stream_id: StreamId,
    /// Inclusive folded cursor.
    pub inclusive_cursor: EventCursor,
    /// Manifest-pinned source SchemaSet.
    pub source_schema_set_ref: SchemaSetRef,
    /// Manifest-pinned Effect registry revision.
    pub effect_registry_revision: RevisionId,
    /// Exact registry configuration digest.
    pub effect_registry_config_digest: Digest,
    /// Exact reducer revision.
    pub reducer_revision: RevisionId,
    /// Continuous history digest.
    pub history_digest: Digest,
    /// Effect entries sorted by stable identity.
    pub effects: Vec<EffectProjectionEntryV1>,
    /// Folded late Receipt count.
    pub late_receipt_count: u64,
    /// Folded rejected-message count.
    pub rejected_count: u64,
}

/// Inventory-time reconciliation binding for partial or unknown outcomes.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectReconciliationBindingV2 {
    /// Reconciliation was open at the fixed inventory horizon.
    Open {
        /// Evidence digest available at finalization.
        evidence_digest: Digest,
    },
    /// Reconciliation was resolved at the fixed inventory horizon.
    Resolved {
        /// Source reconciliation Event.
        source_reconciliation_event_id: EventId,
        /// Evidence digest supporting the resolution.
        evidence_digest: Digest,
    },
}

/// Lossless Effect conclusion encoded by Boundary Inventory V2.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectBoundaryOutcomeV2 {
    /// The external effect was applied.
    Applied {
        /// Admitted Receipt digest.
        receipt_digest: Digest,
        /// Safe result digest.
        result_digest: Digest,
        /// Sorted stable limitation codes.
        limitations: Vec<String>,
    },
    /// The external effect was proven not applied.
    NotApplied {
        /// Stable reason code.
        reason_code: String,
        /// Sorted stable limitation codes.
        limitations: Vec<String>,
    },
    /// A bounded subset was proven applied.
    Partial {
        /// Optional admitted Receipt digest.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        receipt_digest: Option<Digest>,
        /// Digest of confirmed components.
        confirmed_components_digest: Digest,
        /// Digest of unknown components.
        unknown_components_digest: Digest,
        /// Sorted stable limitation codes.
        limitations: Vec<String>,
        /// Reconciliation state at the fixed horizon.
        reconciliation_binding: EffectReconciliationBindingV2,
    },
    /// Whether the external effect applied cannot be proven.
    Unknown {
        /// Sorted stable limitation codes.
        limitations: Vec<String>,
        /// Reconciliation state at the fixed horizon.
        reconciliation_binding: EffectReconciliationBindingV2,
    },
    /// Intent existed but no dispatch claim was delivered.
    CancelledBeforeClaim {
        /// Stable cancellation or deadline reason.
        reason_code: String,
    },
}

/// One lossless Effect fact in Boundary Inventory V2.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectBoundaryRecordV2 {
    /// Stable Effect identity.
    pub effect_id: EffectId,
    /// Canonical request digest.
    pub request_digest: Digest,
    /// Stable attempt identity.
    pub attempt_id: EffectAttemptId,
    /// External idempotency identity digest when claimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_digest: Option<Digest>,
    /// Exact executor revision.
    pub executor_revision: RevisionId,
    /// Exact executor descriptor digest.
    pub executor_descriptor_digest: Digest,
    /// Protected operation identity.
    pub operation_id: OperationId,
    /// Budget reservation identity.
    pub reservation_id: ReservationId,
    /// Lossless conclusion at the fixed inventory horizon.
    pub outcome: EffectBoundaryOutcomeV2,
}

/// Frozen content preimage for Boundary Inventory V2.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryInventoryHashViewV2 {
    /// Completed source Run.
    pub source_run_id: RunId,
    /// Source SchemaSet.
    pub schema_set_ref: SchemaSetRef,
    /// Exact Effect stream.
    pub effect_stream_id: StreamId,
    /// Inclusive Effect cursor fixed by finalization.
    pub effect_inclusive_cursor: EventCursor,
    /// Continuous history digest at that cursor.
    pub effect_history_digest: Digest,
    /// Boundary recording policy revision.
    pub recording_policy_revision: RevisionId,
    /// Canonically ordered lossless Effect records.
    pub effects: Vec<EffectBoundaryRecordV2>,
}

/// Immutable Boundary Inventory V2 used by Effect-capable replay.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryInventoryRevisionV2 {
    /// Immutable revision metadata.
    pub metadata: RevisionMetadata,
    /// Exact V2 hash-view schema.
    pub hash_schema_ref: SchemaRef,
    /// Completed source Run.
    pub source_run_id: RunId,
    /// Source SchemaSet.
    pub schema_set_ref: SchemaSetRef,
    /// Exact Effect stream.
    pub effect_stream_id: StreamId,
    /// Inclusive Effect cursor fixed by finalization.
    pub effect_inclusive_cursor: EventCursor,
    /// Continuous history digest at that cursor.
    pub effect_history_digest: Digest,
    /// Boundary recording policy revision.
    pub recording_policy_revision: RevisionId,
    /// Canonically ordered lossless Effect records.
    pub effects: Vec<EffectBoundaryRecordV2>,
}

impl BoundaryInventoryRevisionV2 {
    /// Computes the V2 inventory content digest.
    pub fn content_digest(&self) -> Result<Digest, crate::ValidationError> {
        let value = serde_json::to_value(BoundaryInventoryHashViewV2 {
            source_run_id: self.source_run_id.clone(),
            schema_set_ref: self.schema_set_ref.clone(),
            effect_stream_id: self.effect_stream_id.clone(),
            effect_inclusive_cursor: self.effect_inclusive_cursor.clone(),
            effect_history_digest: self.effect_history_digest.clone(),
            recording_policy_revision: self.recording_policy_revision.clone(),
            effects: self.effects.clone(),
        })
        .map_err(|_| effect_contract_error("/inventory", "V2 inventory serialization failed"))?;
        crate::digest_json(
            "revision:boundary_inventory_v2",
            &self.hash_schema_ref,
            &value,
        )
    }

    /// Validates identity and canonical Effect record order.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        let valid = self.metadata.revision_kind == "boundary_inventory_v2"
            && self.hash_schema_ref.r#type == "boundary-inventory-hash-view"
            && self.hash_schema_ref.major == 2
            && self.content_digest()? == self.metadata.content_digest
            && self.metadata.validate_identity().is_ok()
            && self
                .effects
                .windows(2)
                .all(|pair| pair[0].effect_id < pair[1].effect_id);
        if valid {
            Ok(())
        } else {
            Err(effect_contract_error(
                "/boundary_inventory_v2",
                "V2 inventory identity or Effect order is invalid",
            ))
        }
    }
}

fn effect_contract_error(path: &str, detail: &str) -> crate::ValidationError {
    crate::ValidationError {
        code: crate::ErrorCode::InvariantViolation,
        path: path.to_owned(),
        contract: "effect_contract_v1".to_owned(),
        detail: detail.to_owned(),
    }
}
