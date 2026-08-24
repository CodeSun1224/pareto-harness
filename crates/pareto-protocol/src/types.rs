use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Digest;

macro_rules! wire_id {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Parses a validated `", stringify!($name), "`.")]
            pub fn parse(value: impl Into<String>) -> Result<Self, crate::ValidationError> {
                let value = value.into();
                if !valid_wire_id(&value, $prefix) {
                    return Err(crate::ValidationError {
                        code: crate::ErrorCode::InvalidIdentifier,
                        path: String::new(),
                        contract: stringify!($name).to_owned(),
                        detail: "identifier prefix or format is invalid".to_owned(),
                    });
                }
                Ok(Self(value))
            }

            #[doc = concat!("Returns the `", stringify!($name), "` wire value.")]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::parse(value)
                    .map_err(|_| serde::de::Error::custom(concat!("invalid ", stringify!($name))))
            }
        }
    };
}

wire_id!(TenantId, "tenant_", "Tenant isolation identifier.");
wire_id!(UserId, "user_", "User isolation identifier.");
wire_id!(WorkspaceId, "workspace_", "Workspace isolation identifier.");
wire_id!(RunId, "run_", "Run identifier.");
wire_id!(TaskId, "task_", "Task identifier within one run lifecycle.");
wire_id!(AgentId, "agent_", "Agent or actor identifier.");
wire_id!(StreamId, "stream_", "Event stream identifier.");
wire_id!(EventId, "event_", "Event identifier.");
wire_id!(RequirementId, "req_", "Requirement identifier.");
wire_id!(RevisionId, "rev_", "Immutable revision identifier.");

fn valid_wire_id(value: &str, prefix: &str) -> bool {
    let suffix = value.strip_prefix(prefix).unwrap_or_default();
    value.len() > prefix.len()
        && value.len() <= 128
        && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !suffix.starts_with('-')
        && !value.ends_with('-')
}

fn deserialize_present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Complete, immutable schema identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(try_from = "SchemaRefWire")]
pub struct SchemaRef {
    /// Kebab-case schema type.
    pub r#type: String,
    /// Breaking contract version.
    pub major: u32,
    /// Compatible contract version.
    pub minor: u32,
    /// Digest of the exact schema bytes.
    pub schema_digest: Digest,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SchemaRefWire {
    r#type: String,
    major: u32,
    minor: u32,
    schema_digest: Digest,
}

impl TryFrom<SchemaRefWire> for SchemaRef {
    type Error = &'static str;

    fn try_from(wire: SchemaRefWire) -> Result<Self, Self::Error> {
        let valid_type = !wire.r#type.is_empty()
            && wire.r#type.len() <= 128
            && wire.r#type.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
            && wire.r#type.as_bytes()[0].is_ascii_lowercase();
        if !valid_type {
            return Err("invalid schema type");
        }
        Ok(Self {
            r#type: wire.r#type,
            major: wire.major,
            minor: wire.minor,
            schema_digest: wire.schema_digest,
        })
    }
}

/// Exact isolation keys carried by authoritative records.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IsolationScope {
    /// Required tenant boundary.
    pub tenant_id: TenantId,
    /// Exact optional authenticated user.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub user_id: Option<UserId>,
    /// Required workspace boundary.
    pub workspace_id: WorkspaceId,
    /// Required run boundary.
    pub run_id: RunId,
    /// Required agent/actor boundary.
    pub agent_id: AgentId,
}

/// Kernel-derived validation context; never constructed from payload fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedValidationContext {
    /// Expected exact isolation scope.
    pub(crate) scope: IsolationScope,
    /// Authenticated or delegated actor.
    pub(crate) actor: AgentId,
    /// Exact append target stream.
    pub(crate) target_stream: StreamId,
    /// Schema set pinned by the run manifest.
    pub(crate) schema_set_ref: SchemaSetRef,
    /// Limits profile pinned by the run manifest.
    pub(crate) protocol_limits_ref: ProtocolLimitsRef,
}

/// Immutable metadata shared by revisions.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionMetadata {
    /// Stable logical identifier.
    pub logical_id: String,
    /// Content and metadata-derived revision identifier.
    pub revision_id: RevisionId,
    /// Revision kind used for digest domain separation.
    pub revision_kind: String,
    /// Optional same-kind parent.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub parent_revision: Option<RevisionId>,
    /// Wire schema used by this revision.
    pub schema_ref: SchemaRef,
    /// Digest of the behavior content hash view.
    pub content_digest: Digest,
    /// Actor that created the revision.
    pub creator_actor: AgentId,
    /// Source classification or reference.
    pub source: String,
    /// Canonical UTC RFC 3339 millisecond time.
    pub created_at: String,
}

/// Behavior content used to compute a revision content digest.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionHashView {
    /// Revision kind used for domain separation.
    pub revision_kind: String,
    /// Versioned behavior content; revision metadata is excluded.
    pub content: Value,
}

impl RevisionMetadata {
    /// Proves that the supplied immutable revision ID matches its complete metadata preimage.
    pub fn validate_identity(&self) -> Result<(), crate::ValidationError> {
        if self.logical_id.is_empty()
            || self.revision_kind.is_empty()
            || self.source.is_empty()
            || !crate::validation::is_canonical_timestamp(&self.created_at)
        {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/metadata".to_owned(),
                contract: "revision_metadata".to_owned(),
                detail: "revision metadata semantic fields are invalid".to_owned(),
            });
        }
        if crate::derive_revision_id(self)? == self.revision_id {
            Ok(())
        } else {
            Err(crate::ValidationError {
                code: crate::ErrorCode::DigestMismatch,
                path: "/revision_id".to_owned(),
                contract: "revision_metadata".to_owned(),
                detail: "revision ID does not match metadata preimage".to_owned(),
            })
        }
    }
}

/// Manifest for arbitrary artifact bytes; raw bytes are never interpreted as JSON.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(try_from = "ArtifactManifestWire")]
pub struct ArtifactManifest {
    /// Manifest schema.
    pub schema_ref: SchemaRef,
    /// Artifact kind.
    pub artifact_kind: String,
    /// IANA media type.
    pub media_type: String,
    /// Canonical decimal byte length.
    pub byte_length: String,
    /// SHA-256 of exactly the raw artifact bytes.
    pub raw_bytes_digest: Digest,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArtifactManifestWire {
    schema_ref: SchemaRef,
    artifact_kind: String,
    media_type: String,
    byte_length: String,
    raw_bytes_digest: Digest,
}

impl TryFrom<ArtifactManifestWire> for ArtifactManifest {
    type Error = &'static str;

    fn try_from(wire: ArtifactManifestWire) -> Result<Self, Self::Error> {
        let decimal = wire.byte_length == "0"
            || (!wire.byte_length.starts_with('0')
                && wire.byte_length.bytes().all(|byte| byte.is_ascii_digit()));
        if wire.artifact_kind.is_empty()
            || wire.media_type.is_empty()
            || !decimal
            || wire.byte_length.len() > 128
        {
            return Err("invalid artifact manifest semantics");
        }
        Ok(Self {
            schema_ref: wire.schema_ref,
            artifact_kind: wire.artifact_kind,
            media_type: wire.media_type,
            byte_length: wire.byte_length,
            raw_bytes_digest: wire.raw_bytes_digest,
        })
    }
}

/// Event type and payload schema binding inside an admitted schema set.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTypeBinding {
    /// Stable event type.
    pub event_type: String,
    /// Event contract major version.
    pub major: u32,
    /// Event contract minor version.
    pub minor: u32,
    /// Required payload schema.
    pub payload_schema_ref: SchemaRef,
    /// Language-independent typed variant identifier.
    pub variant_id: String,
}

/// Immutable schema-set manifest content.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaSetManifest {
    /// Complete member schema references sorted by schema identity.
    pub schemas: Vec<SchemaRef>,
    /// Exact envelope schema accepted for events in this set.
    pub event_envelope_schema_ref: SchemaRef,
    /// Unique event bindings sorted by type and version.
    pub event_bindings: Vec<EventTypeBinding>,
}

/// Reference to an admitted schema-set manifest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaSetRef {
    /// Schema of the manifest itself.
    pub manifest_schema_ref: SchemaRef,
    /// Digest of manifest content in the schema-set domain.
    pub manifest_digest: Digest,
}

/// Versioned protocol resource limits fixed by a run.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolLimitsRef {
    /// Stable limits profile name.
    pub profile: String,
    /// Digest of the complete profile.
    pub digest: Digest,
}

/// Versioned policy describing which nondeterministic boundaries are recorded.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRecordingPolicyRef {
    /// Immutable policy revision.
    pub revision_id: RevisionId,
    /// Policy content digest.
    pub digest: Digest,
}

/// Final observed state of one nondeterministic boundary.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoundaryOutcome {
    /// A receipt was recorded.
    Received {
        /// Receipt artifact or record digest.
        receipt_digest: Digest,
    },
    /// The request failed before a receipt.
    Failed {
        /// Stable failure reason code.
        reason_code: String,
    },
    /// Cancellation completed without a receipt.
    Cancelled,
}

/// One boundary fact derived from the authoritative event range.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRecord {
    /// Boundary category.
    pub boundary_kind: String,
    /// Event or request identity.
    pub request_event_id: EventId,
    /// Recorded outcome.
    pub outcome: BoundaryOutcome,
}

/// Immutable post-run inventory used by replay and re-execution.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryInventoryRevision {
    /// Inventory revision metadata.
    pub metadata: RevisionMetadata,
    /// Exact Schema for the immutable inventory content hash preimage.
    pub hash_schema_ref: SchemaRef,
    /// Completed source run.
    pub source_run_id: RunId,
    /// Inclusive last event sequence used for finalization.
    pub final_event_sequence: String,
    /// Schema set of the source event range.
    pub schema_set_ref: SchemaSetRef,
    /// Recording policy used by the source run.
    pub recording_policy_ref: BoundaryRecordingPolicyRef,
    /// Ordered boundary facts; empty is an explicit deterministic recording.
    pub boundaries: Vec<BoundaryRecord>,
}

/// Reconciliation of audit facts that arrived after inventory finalization.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryReconciliationRevision {
    /// Reconciliation revision metadata.
    pub metadata: RevisionMetadata,
    /// Exact Schema for the immutable reconciliation content hash preimage.
    pub hash_schema_ref: SchemaRef,
    /// Inventory being reconciled without mutation.
    pub inventory_revision: RevisionId,
    /// Ordered late-result audit events.
    pub late_result_events: Vec<EventId>,
}

/// Frozen content-only preimage for a boundary inventory revision digest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryInventoryHashView {
    /// Completed source run.
    pub source_run_id: RunId,
    /// Inclusive final event sequence.
    pub final_event_sequence: String,
    /// Source schema set.
    pub schema_set_ref: SchemaSetRef,
    /// Boundary recording policy.
    pub recording_policy_ref: BoundaryRecordingPolicyRef,
    /// Ordered finalized boundary facts.
    pub boundaries: Vec<BoundaryRecord>,
}

/// Frozen content-only preimage for a boundary reconciliation revision digest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryReconciliationHashView {
    /// Inventory being reconciled.
    pub inventory_revision: RevisionId,
    /// Ordered late-result audit events.
    pub late_result_events: Vec<EventId>,
}

impl BoundaryInventoryRevision {
    /// Computes the behavior-content digest excluding revision metadata.
    pub fn content_digest(&self) -> Result<Digest, crate::ValidationError> {
        let value = serde_json::to_value(BoundaryInventoryHashView {
            source_run_id: self.source_run_id.clone(),
            final_event_sequence: self.final_event_sequence.clone(),
            schema_set_ref: self.schema_set_ref.clone(),
            recording_policy_ref: self.recording_policy_ref.clone(),
            boundaries: self.boundaries.clone(),
        })
        .map_err(|_| crate::ValidationError {
            code: crate::ErrorCode::InvariantViolation,
            path: String::new(),
            contract: "boundary_inventory_hash_view".to_owned(),
            detail: "hash view serialization failed".to_owned(),
        })?;
        crate::digest_json("revision:boundary_inventory", &self.hash_schema_ref, &value)
    }

    /// Validates finalization-only invariants; an empty boundary list is valid and explicit.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.metadata.revision_kind != "boundary_inventory"
            || self.hash_schema_ref.r#type != "boundary-inventory-hash-view"
            || self.content_digest()? != self.metadata.content_digest
            || self.metadata.validate_identity().is_err()
        {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::DigestMismatch,
                path: "/metadata".to_owned(),
                contract: "boundary_inventory_revision".to_owned(),
                detail: "inventory metadata kind or revision identity is invalid".to_owned(),
            });
        }
        if self.final_event_sequence.is_empty()
            || self.final_event_sequence == "0"
            || self.final_event_sequence.starts_with('0')
            || !self
                .final_event_sequence
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/final_event_sequence".to_owned(),
                contract: "boundary_inventory_revision".to_owned(),
                detail: "final event sequence must be a positive canonical decimal".to_owned(),
            });
        }
        let unique: std::collections::BTreeSet<_> = self
            .boundaries
            .iter()
            .map(|boundary| &boundary.request_event_id)
            .collect();
        if unique.len() != self.boundaries.len() {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/boundaries".to_owned(),
                contract: "boundary_inventory_revision".to_owned(),
                detail: "a request event may have only one finalized outcome".to_owned(),
            });
        }
        Ok(())
    }
}

impl BoundaryReconciliationRevision {
    /// Computes the immutable reconciliation delta digest excluding revision metadata.
    pub fn content_digest(&self) -> Result<Digest, crate::ValidationError> {
        let value = serde_json::to_value(BoundaryReconciliationHashView {
            inventory_revision: self.inventory_revision.clone(),
            late_result_events: self.late_result_events.clone(),
        })
        .map_err(|_| crate::ValidationError {
            code: crate::ErrorCode::InvariantViolation,
            path: String::new(),
            contract: "boundary_reconciliation_hash_view".to_owned(),
            detail: "hash view serialization failed".to_owned(),
        })?;
        crate::digest_json(
            "revision:boundary_reconciliation",
            &self.hash_schema_ref,
            &value,
        )
    }

    /// Validates that reconciliation is an explicit, non-duplicated late-result delta.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.metadata.revision_kind != "boundary_reconciliation"
            || self.hash_schema_ref.r#type != "boundary-reconciliation-hash-view"
            || self.content_digest()? != self.metadata.content_digest
            || self.metadata.validate_identity().is_err()
        {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::DigestMismatch,
                path: "/metadata".to_owned(),
                contract: "boundary_reconciliation_revision".to_owned(),
                detail: "reconciliation metadata kind or revision identity is invalid".to_owned(),
            });
        }
        let unique: std::collections::BTreeSet<_> = self.late_result_events.iter().collect();
        if self.late_result_events.is_empty() || unique.len() != self.late_result_events.len() {
            return Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/late_result_events".to_owned(),
                contract: "boundary_reconciliation_revision".to_owned(),
                detail: "reconciliation must contain unique late-result audit events".to_owned(),
            });
        }
        Ok(())
    }
}

/// Run execution and replay lineage contract.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionMode {
    /// First live execution; it cannot claim a source run.
    Live {},
    /// Replay using a finalized source recording.
    RecordedReplay {
        /// Completed source run.
        source_run_id: RunId,
        /// Finalized boundary inventory used by this replay.
        boundary_inventory_revision: RevisionId,
    },
    /// Re-execution compared against a finalized source recording.
    Reexecute {
        /// Completed source run.
        source_run_id: RunId,
        /// Finalized boundary inventory used for comparison.
        boundary_inventory_revision: RevisionId,
    },
    /// Deterministic fixture-driven execution.
    Simulated {
        /// Non-empty fixture revisions.
        fixture_revisions: Vec<RevisionId>,
        /// Whether the simulation is standalone or derived.
        simulation_origin: SimulationOrigin,
        /// Source required exactly for a derived simulation.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present_option"
        )]
        source_run_id: Option<RunId>,
    },
}

/// Explicit simulation lineage discriminator.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationOrigin {
    /// Fixture-only execution with no source run.
    Standalone,
    /// Simulation derived from a source run.
    Derived,
}

impl ExecutionMode {
    /// Validates lineage conditions that JSON shape alone cannot express.
    pub fn validate(&self, derived_run_id: &RunId) -> Result<(), crate::ValidationError> {
        let valid = match self {
            Self::Live {} => true,
            Self::RecordedReplay { source_run_id, .. } | Self::Reexecute { source_run_id, .. } => {
                source_run_id != derived_run_id
            }
            Self::Simulated {
                fixture_revisions,
                simulation_origin,
                source_run_id,
            } => {
                let lineage = matches!(
                    (simulation_origin, source_run_id),
                    (SimulationOrigin::Standalone, None) | (SimulationOrigin::Derived, Some(_))
                );
                !fixture_revisions.is_empty()
                    && lineage
                    && source_run_id
                        .as_ref()
                        .is_none_or(|source| source != derived_run_id)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/execution_mode".to_owned(),
                contract: "execution_lineage".to_owned(),
                detail: "source must not self-reference and simulated fixtures must be non-empty"
                    .to_owned(),
            })
        }
    }

    /// Binds replay/re-execution to the exact finalized source inventory supplied by the Kernel.
    pub fn validate_inventory(
        &self,
        inventory: &crate::Validated<BoundaryInventoryRevision>,
    ) -> Result<(), crate::ValidationError> {
        let inventory = inventory.get();
        let expected = &inventory.metadata.revision_id;
        let matches = match self {
            Self::RecordedReplay {
                source_run_id,
                boundary_inventory_revision,
            }
            | Self::Reexecute {
                source_run_id,
                boundary_inventory_revision,
            } => {
                source_run_id == &inventory.source_run_id && boundary_inventory_revision == expected
            }
            Self::Live {} | Self::Simulated { .. } => false,
        };
        if matches {
            Ok(())
        } else {
            Err(crate::ValidationError {
                code: crate::ErrorCode::InvariantViolation,
                path: "/execution_mode/boundary_inventory_revision".to_owned(),
                contract: "execution_lineage".to_owned(),
                detail: "mode does not pin the exact source inventory".to_owned(),
            })
        }
    }
}

/// Immutable event envelope validated before kernel admission.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    /// Envelope schema.
    pub schema_ref: SchemaRef,
    /// Exact isolation keys.
    pub scope: IsolationScope,
    /// Unique event identifier.
    pub event_id: EventId,
    /// Target stream.
    pub stream_id: StreamId,
    /// Owning run.
    pub run_id: RunId,
    /// Positive decimal sequence string.
    pub sequence: String,
    /// Optional causing event.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub causation_id: Option<EventId>,
    /// Correlation identifier.
    pub correlation_id: String,
    /// Registered event type.
    pub event_type: String,
    /// Event major version.
    pub event_major: u32,
    /// Event minor version.
    pub event_minor: u32,
    /// Canonical occurrence time.
    pub occurred_at: String,
    /// Authenticated/delegated actor.
    pub actor: AgentId,
    /// Exact payload schema.
    pub payload_schema_ref: SchemaRef,
    /// Digest of the payload.
    pub payload_digest: Digest,
    /// Payload validated through the event type registry.
    pub payload: Value,
}

/// Immutable run manifest pinning every behavior-affecting input.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    /// Manifest schema.
    pub schema_ref: SchemaRef,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Required revision pins by role.
    pub revisions: BTreeMap<String, RevisionId>,
    /// Optional plan revision.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub plan_revision: Option<RevisionId>,
    /// Exact admitted schema set.
    pub schema_set_ref: SchemaSetRef,
    /// Immutable budget snapshot revision.
    pub budget_revision: RevisionId,
    /// Versioned protocol limits.
    pub protocol_limits_ref: ProtocolLimitsRef,
    /// Boundary recording policy.
    pub boundary_recording_policy_ref: BoundaryRecordingPolicyRef,
    /// Execution/replay contract.
    pub execution_mode: ExecutionMode,
}

/// Authoritative state of a Run lifecycle aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Manifest is fixed but execution has not started.
    Created,
    /// The run is actively executing.
    Running,
    /// The run is explicitly paused.
    Paused,
    /// The run completed successfully.
    Succeeded,
    /// The run completed with failure.
    Failed,
    /// The run was cancelled.
    Cancelled,
}

/// Authoritative state of a Task owned by one Run lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// The task exists but is not ready to run.
    Created,
    /// The task is ready to be scheduled.
    Ready,
    /// The task is actively executing.
    Running,
    /// The task is explicitly paused.
    Paused,
    /// The task completed successfully.
    Succeeded,
    /// The task completed with failure.
    Failed,
    /// The task was cancelled.
    Cancelled,
}

/// Sequence-one payload that atomically fixes a complete Run Manifest.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCreatedPayload {
    /// Complete immutable manifest for the new Run.
    pub manifest: RunManifest,
}

/// Payload recording immutable Task ownership within a Run.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCreatedPayload {
    /// Task identifier unique within the Run.
    pub task_id: TaskId,
    /// Optional earlier-created parent Task in the same lifecycle stream.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_option"
    )]
    pub parent_task_id: Option<TaskId>,
    /// Fixed initial state; lifecycle validation requires `created`.
    pub initial_state: TaskState,
}

/// Payload recording one authoritative Run state transition.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStateTransitionedPayload {
    /// Folded state before the transition.
    pub from: RunState,
    /// Requested state after the transition.
    pub to: RunState,
    /// Stable non-empty reason code fixed by the command.
    pub reason_code: String,
}

/// Payload recording one authoritative Task state transition.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskStateTransitionedPayload {
    /// Target Task within the lifecycle aggregate.
    pub task_id: TaskId,
    /// Folded state before the transition.
    pub from: TaskState,
    /// Requested state after the transition.
    pub to: TaskState,
    /// Stable non-empty reason code fixed by the command.
    pub reason_code: String,
}

/// Structured evidence verdict; natural language cannot introduce a passing state.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerdict {
    /// Evidence proves the claim under its stated scope.
    Passed,
    /// Evidence disproves the claim.
    Failed,
    /// Evidence cannot decide the claim.
    Inconclusive,
    /// Evidence is no longer admissible.
    Invalidated,
}

/// Evidence linked to exact producer, verifier, subject, and artifact revisions.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    /// Evidence schema.
    pub schema_ref: SchemaRef,
    /// Exact isolation scope.
    pub scope: IsolationScope,
    /// Requirement being evidenced.
    pub requirement_id: RequirementId,
    /// Verifiable claim.
    pub claim: String,
    /// Evidence category.
    pub evidence_type: String,
    /// Producer revision.
    pub producer_revision: RevisionId,
    /// Verifier revision.
    pub verifier_revision: RevisionId,
    /// Subject revision.
    pub subject_revision: RevisionId,
    /// Artifact manifest digest.
    pub artifact_digest: Digest,
    /// Structured verdict.
    pub verdict: EvidenceVerdict,
    /// Evidence scope description.
    pub evidence_scope: String,
    /// Freshness contract.
    pub freshness: String,
    /// Explicit limitations.
    pub limitations: Vec<String>,
    /// Canonical observation time.
    pub observed_at: String,
}
