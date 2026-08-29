use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserializer as _;
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest as _;

use crate::{
    AgentId, Digest, ErrorCode, EventEnvelope, EventTypeBinding, EvidenceRecord, IsolationScope,
    ProtocolLimitsRef, RunManifest, SchemaDocument, SchemaRef, SchemaSetManifest, SchemaSetRef,
    StreamId, TrustedValidationContext, ValidationError, canonical_json_bytes, digest_json,
    digest_schema,
};

type AdmittedSchemas = (
    BTreeMap<SchemaRef, Value>,
    BTreeMap<SchemaRef, Arc<jsonschema::Validator>>,
);

/// Fixed V1 limits. A kernel may select a stricter, separately versioned profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolLimitsV1;

/// Published immutable preimage for the V1 resource-limits identity.
#[derive(Clone, Debug, Eq, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolLimitsProfileV1 {
    /// Raw transport bytes.
    pub raw_record_bytes: usize,
    /// Canonical record bytes.
    pub record_bytes: usize,
    /// Canonical payload bytes.
    pub payload_bytes: usize,
    /// Decoded UTF-8 string bytes.
    pub string_bytes: usize,
    /// Maximum value depth.
    pub depth: usize,
    /// Maximum members in one array or object.
    pub collection: usize,
    /// Maximum returned errors.
    pub errors: usize,
}

impl ProtocolLimitsV1 {
    /// Digest of the canonical V1 limits profile published by RFC-0002.
    pub const DIGEST: &'static str =
        "sha256:503a7fd1d3d1d93412ce8f3a0f5bdfd1298afa1c2289a60b97bd00dea73fadc0";
    /// Raw JSON transport ceiling before parsing.
    pub const RAW_RECORD_BYTES: usize = 1_048_576;
    /// Canonical semantic record ceiling.
    pub const RECORD_BYTES: usize = 1_048_576;
    /// Canonical payload ceiling.
    pub const PAYLOAD_BYTES: usize = 786_432;
    /// Decoded UTF-8 string ceiling.
    pub const STRING_BYTES: usize = 262_144;
    /// Root is depth one.
    pub const DEPTH: usize = 64;
    /// Per-array or per-object member ceiling.
    pub const COLLECTION: usize = 16_384;
    /// Maximum deterministic errors returned by a batch API.
    pub const ERRORS: usize = 32;

    /// Returns the complete published profile preimage.
    pub const fn profile() -> ProtocolLimitsProfileV1 {
        ProtocolLimitsProfileV1 {
            raw_record_bytes: Self::RAW_RECORD_BYTES,
            record_bytes: Self::RECORD_BYTES,
            payload_bytes: Self::PAYLOAD_BYTES,
            string_bytes: Self::STRING_BYTES,
            depth: Self::DEPTH,
            collection: Self::COLLECTION,
            errors: Self::ERRORS,
        }
    }

    /// Recomputes the profile identity from the canonical published preimage.
    pub fn computed_digest() -> Result<String, ValidationError> {
        let value = serde_json::to_value(Self::profile())
            .map_err(|_| invariant("limits profile serialization failed"))?;
        Ok(format!(
            "sha256:{:x}",
            sha2::Sha256::digest(canonical_json_bytes(&value)?)
        ))
    }
}

/// A value that passed protocol validation. Its field is private to prevent extension forgery.
#[derive(Clone, Debug, PartialEq)]
pub struct Validated<T>(T);

impl<T> Validated<T> {
    /// Borrows the validated value without granting kernel authorization.
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Consumes the wrapper and returns the validated value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Event admitted through one exact registry variant and its payload schema.
pub struct ValidatedEvent {
    envelope: EventEnvelope,
    schema_set_ref: SchemaSetRef,
    protocol_limits_ref: ProtocolLimitsRef,
    variant_id: String,
    decoded: Box<dyn Any + Send + Sync>,
}

impl fmt::Debug for ValidatedEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedEvent")
            .field("envelope", &self.envelope)
            .field("schema_set_ref", &self.schema_set_ref)
            .field("protocol_limits_ref", &self.protocol_limits_ref)
            .field("variant_id", &self.variant_id)
            .finish_non_exhaustive()
    }
}

impl ValidatedEvent {
    /// Returns the validated immutable envelope.
    pub fn envelope(&self) -> &EventEnvelope {
        &self.envelope
    }

    /// Returns the exact immutable SchemaSet used to admit this event.
    pub fn schema_set_ref(&self) -> &SchemaSetRef {
        &self.schema_set_ref
    }

    /// Returns the exact protocol-limits identity used to admit this event.
    pub fn protocol_limits_ref(&self) -> &ProtocolLimitsRef {
        &self.protocol_limits_ref
    }

    /// Returns the exact language-independent typed variant selected by the registry.
    pub fn variant_id(&self) -> &str {
        &self.variant_id
    }

    /// Borrows the payload as the exact Rust type produced by the admitted decoder.
    pub fn downcast_payload<T: Any>(&self) -> Option<&T> {
        self.decoded.downcast_ref()
    }
}

/// Kernel-owned policy boundary that authorizes a structurally valid schema-set transition.
pub trait SchemaAdmissionAuthorizer {
    /// Authorizes the candidate after all bytes, identities, digests, and compatibility checks pass.
    fn authorize(
        &self,
        parent: Option<&SchemaSetRef>,
        candidate: &SchemaSetRef,
    ) -> Result<(), ValidationError>;
}

/// Typed decoder bound to exactly one event variant and payload schema.
pub trait EventVariantDecoder: Send + Sync {
    /// Stable language-independent variant identifier.
    fn variant_id(&self) -> &str;
    /// Exact payload schema accepted by this decoder.
    fn payload_schema_ref(&self) -> &SchemaRef;
    /// Decodes a schema-valid payload into its closed typed representation.
    fn decode(&self, payload: &Value) -> Result<Box<dyn Any + Send + Sync>, ValidationError>;
}

struct BuiltinEventDecoder<T> {
    variant_id: String,
    payload_schema_ref: SchemaRef,
    marker: PhantomData<T>,
}

impl<T> EventVariantDecoder for BuiltinEventDecoder<T>
where
    T: Any + DeserializeOwned + Send + Sync,
{
    fn variant_id(&self) -> &str {
        &self.variant_id
    }

    fn payload_schema_ref(&self) -> &SchemaRef {
        &self.payload_schema_ref
    }

    fn decode(&self, payload: &Value) -> Result<Box<dyn Any + Send + Sync>, ValidationError> {
        serde_json::from_value::<T>(payload.clone())
            .map(|value| Box::new(value) as Box<dyn Any + Send + Sync>)
            .map_err(|_| schema_error("/payload", "typed lifecycle payload decoding failed"))
    }
}

fn builtin_event_decoder(binding: &EventTypeBinding) -> Option<Arc<dyn EventVariantDecoder>> {
    macro_rules! decoder {
        ($type:ty) => {
            Arc::new(BuiltinEventDecoder::<$type> {
                variant_id: binding.variant_id.clone(),
                payload_schema_ref: binding.payload_schema_ref.clone(),
                marker: PhantomData,
            })
        };
    }
    match (
        binding.variant_id.as_str(),
        binding.payload_schema_ref.r#type.as_str(),
    ) {
        ("runtime-control-initialized-v1", "runtime-control-initialized-payload") => {
            Some(decoder!(crate::RuntimeControlInitializedPayloadV1))
        }
        ("capability-issued-v1", "capability-issued-payload") => {
            Some(decoder!(crate::CapabilityIssuedPayloadV1))
        }
        ("capability-revoked-v1", "capability-revoked-payload") => {
            Some(decoder!(crate::CapabilityRevokedPayloadV1))
        }
        ("protected-operation-denied-v1", "protected-operation-denied-payload") => {
            Some(decoder!(crate::ProtectedOperationDeniedPayloadV1))
        }
        ("operation-reserved-v1", "operation-reserved-payload") => {
            Some(decoder!(crate::OperationReservedPayloadV1))
        }
        ("operation-settled-v1", "operation-settled-payload") => {
            Some(decoder!(crate::OperationSettledPayloadV1))
        }
        ("budget-refunded-v1", "budget-refunded-payload") => {
            Some(decoder!(crate::BudgetRefundedPayloadV1))
        }
        ("cancellation-requested-v1", "cancellation-requested-payload") => {
            Some(decoder!(crate::CancellationRequestedPayloadV1))
        }
        ("cancellation-acknowledged-v1", "cancellation-acknowledged-payload") => {
            Some(decoder!(crate::CancellationAcknowledgedPayloadV1))
        }
        ("late-result-observed-v1", "late-result-observed-payload") => {
            Some(decoder!(crate::LateResultObservedPayloadV1))
        }
        ("control-message-rejected-v1", "control-message-rejected-payload") => {
            Some(decoder!(crate::ControlMessageRejectedPayloadV1))
        }
        ("hook-stream-initialized-v1", "hook-stream-initialized-payload") => {
            Some(decoder!(crate::HookStreamInitializedPayloadV1))
        }
        ("hook-point-started-v1", "hook-point-started-payload") => {
            Some(decoder!(crate::HookPointStartedPayloadV1))
        }
        ("hook-invocation-reserved-v1", "hook-invocation-reserved-payload") => {
            Some(decoder!(crate::HookInvocationReservedPayloadV1))
        }
        ("hook-invocation-terminal-v1", "hook-invocation-terminal-payload") => {
            Some(decoder!(crate::HookInvocationTerminalPayloadV1))
        }
        ("hook-invocation-skipped-v1", "hook-invocation-skipped-payload") => {
            Some(decoder!(crate::HookInvocationSkippedPayloadV1))
        }
        ("hook-point-finalized-v1", "hook-point-finalized-payload") => {
            Some(decoder!(crate::HookPointFinalizedPayloadV1))
        }
        ("hook-late-result-observed-v1", "hook-late-result-observed-payload") => {
            Some(decoder!(crate::HookLateResultObservedPayloadV1))
        }
        ("hook-message-rejected-v1", "hook-message-rejected-payload") => {
            Some(decoder!(crate::HookMessageRejectedPayloadV1))
        }
        ("run-created-v1", "run-created-payload") | ("run-created-v2", "run-created-payload") => {
            Some(decoder!(crate::RunCreatedPayload))
        }
        ("task-created-v1", "task-created-payload") => Some(decoder!(crate::TaskCreatedPayload)),
        ("run-state-transitioned-v1", "run-state-transitioned-payload") => {
            Some(decoder!(crate::RunStateTransitionedPayload))
        }
        ("task-state-transitioned-v1", "task-state-transitioned-payload") => {
            Some(decoder!(crate::TaskStateTransitionedPayload))
        }
        _ => None,
    }
}

mod sealed {
    pub trait ProtocolRecord {}
}

/// Closed, context-free top-level protocol record that can cross an untrusted JSON boundary.
///
/// This trait is sealed. Records whose admission requires trusted run context (events, run
/// manifests, and evidence) deliberately do not implement it and must use their dedicated
/// `SchemaSet` boundary.
///
/// ```compile_fail
/// use pareto_protocol::{EventEnvelope, ProtocolRecord};
/// fn bypass<T: ProtocolRecord>() {}
/// bypass::<EventEnvelope>();
/// ```
pub trait ProtocolRecord: sealed::ProtocolRecord + DeserializeOwned + Serialize {
    /// Exact public Schema type name.
    const SCHEMA_TYPE: &'static str;
    /// Applies cross-field semantics after limits, Schema, and Serde validation.
    fn validate_semantics(&self, _set: &SchemaSet) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Immutable admitted schema set with event type bindings.
#[derive(Clone)]
pub struct SchemaSet {
    reference: SchemaSetRef,
    manifest: SchemaSetManifest,
    _documents: BTreeMap<SchemaRef, Value>,
    validators: BTreeMap<SchemaRef, Arc<jsonschema::Validator>>,
    decoders: BTreeMap<String, Arc<dyn EventVariantDecoder>>,
}

impl fmt::Debug for SchemaSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaSet")
            .field("reference", &self.reference)
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

struct BootstrapAuthorizer;
impl SchemaAdmissionAuthorizer for BootstrapAuthorizer {
    fn authorize(
        &self,
        _parent: Option<&SchemaSetRef>,
        _candidate: &SchemaSetRef,
    ) -> Result<(), ValidationError> {
        Ok(())
    }
}

impl SchemaSet {
    /// Bootstraps only the exact schema set generated by this protocol release.
    pub fn bootstrap_initial(
        manifest: SchemaSetManifest,
        documents: Vec<SchemaDocument>,
        expected_reference: &SchemaSetRef,
    ) -> Result<Self, ValidationError> {
        let embedded = crate::generate_schema_bundle()?.reference;
        if &embedded != expected_reference {
            return Err(schema_error(
                "/manifest_digest",
                "initial schema set does not match the embedded trust root",
            ));
        }
        Self::admit_with(
            &BootstrapAuthorizer,
            None,
            manifest,
            documents,
            expected_reference,
            Vec::new(),
        )
    }

    /// Admits a complete set after structural proof and an explicit Kernel policy decision.
    pub fn admit_with(
        authorizer: &dyn SchemaAdmissionAuthorizer,
        parent: Option<&SchemaSet>,
        manifest: SchemaSetManifest,
        documents: Vec<SchemaDocument>,
        expected_reference: &SchemaSetRef,
        decoders: Vec<Arc<dyn EventVariantDecoder>>,
    ) -> Result<Self, ValidationError> {
        validate_manifest_order(&manifest)?;
        let (verified_documents, validators) = validate_schema_documents(&manifest, documents)?;
        if !manifest
            .schemas
            .contains(&expected_reference.manifest_schema_ref)
        {
            return Err(schema_error(
                "/manifest_schema_ref",
                "manifest schema is not a member",
            ));
        }
        let value = serde_json::to_value(&manifest)
            .map_err(|_| invariant("schema set serialization failed"))?;
        let actual = digest_json(
            "schema-set",
            &expected_reference.manifest_schema_ref,
            &value,
        )?;
        if actual != expected_reference.manifest_digest {
            return Err(ValidationError::new(
                ErrorCode::DigestMismatch,
                "/manifest_digest",
                "schema_set",
                "manifest digest mismatch",
            ));
        }
        if let Some(parent) = parent {
            validate_schema_evolution(parent, &verified_documents)?;
        }
        let mut decoder_map = BTreeMap::new();
        let builtin_decoders = manifest
            .event_bindings
            .iter()
            .filter_map(builtin_event_decoder);
        for decoder in builtin_decoders.chain(decoders) {
            let id = decoder.variant_id().to_owned();
            if id.is_empty() || decoder_map.insert(id.clone(), decoder).is_some() {
                return Err(schema_error(
                    "/event_bindings",
                    "decoder IDs must be non-empty and unique",
                ));
            }
        }
        for binding in &manifest.event_bindings {
            let decoder = decoder_map.get(&binding.variant_id).ok_or_else(|| {
                schema_error("/event_bindings", "event binding has no typed decoder")
            })?;
            if decoder.payload_schema_ref() != &binding.payload_schema_ref {
                return Err(schema_error(
                    "/event_bindings",
                    "typed decoder schema does not match binding",
                ));
            }
        }
        if decoder_map.len() != manifest.event_bindings.len() {
            return Err(schema_error("/event_bindings", "unbound typed decoder"));
        }
        authorizer.authorize(parent.map(SchemaSet::reference), expected_reference)?;
        Ok(Self {
            reference: expected_reference.clone(),
            manifest,
            _documents: verified_documents,
            validators,
            decoders: decoder_map,
        })
    }

    /// Returns the exact admitted schema-set reference.
    pub fn reference(&self) -> &SchemaSetRef {
        &self.reference
    }

    /// Returns true only for a complete member SchemaRef.
    pub fn contains(&self, schema: &SchemaRef) -> bool {
        self.manifest.schemas.binary_search(schema).is_ok()
    }

    /// Returns a schema only when this immutable set has exactly one member of the given type.
    pub fn schema_ref(&self, schema_type: &str) -> Option<&SchemaRef> {
        self.exact_schema(schema_type)
    }

    /// Returns the exact immutable event binding selected by type and version.
    pub fn event_type_binding(
        &self,
        event_type: &str,
        major: u32,
        minor: u32,
    ) -> Option<&EventTypeBinding> {
        self.manifest.event_bindings.iter().find(|binding| {
            binding.event_type == event_type && binding.major == major && binding.minor == minor
        })
    }

    /// Validates an already bounded JSON value against one exact admitted member Schema.
    pub fn validate_value_against(
        &self,
        schema_ref: &SchemaRef,
        value: &Value,
    ) -> Result<(), Vec<ValidationError>> {
        if !self.contains(schema_ref) {
            return Err(vec![schema_error(
                "/schema_ref",
                "value Schema is not an exact admitted member",
            )]);
        }
        let validator = self
            .validators
            .get(schema_ref)
            .expect("admitted member validator");
        let mut errors = Vec::new();
        validate_json_schema(validator, value, "", &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            sort_and_truncate(&mut errors);
            Err(errors)
        }
    }

    /// Parses one untrusted top-level record in limits → Schema → Serde → semantic order.
    pub fn parse_record<T: ProtocolRecord>(
        &self,
        bytes: &[u8],
    ) -> Result<Validated<T>, Vec<ValidationError>> {
        let value = parse_bounded_value(bytes).map_err(|error| vec![error])?;
        let schema_ref = self.exact_schema(T::SCHEMA_TYPE).ok_or_else(|| {
            vec![schema_error(
                "/schema_ref",
                "record Schema is not uniquely admitted",
            )]
        })?;
        let declared_schema = value.get("schema_ref").or_else(|| {
            value
                .get("metadata")
                .and_then(|item| item.get("schema_ref"))
        });
        if let Some(declared) = declared_schema {
            if !matches!(
                serde_json::from_value::<SchemaRef>(declared.clone()),
                Ok(ref declared) if declared == schema_ref
            ) {
                return Err(vec![schema_error(
                    "/schema_ref",
                    "record does not declare its exact admitted Schema",
                )]);
            }
        }
        let validator = self
            .validators
            .get(schema_ref)
            .expect("admitted member validator");
        let mut errors = Vec::new();
        validate_json_schema(validator, &value, "", &mut errors);
        if !errors.is_empty() {
            sort_and_truncate(&mut errors);
            return Err(errors);
        }
        let record: T = serde_json::from_value(value).map_err(|_| {
            vec![ValidationError::new(
                ErrorCode::InvalidJson,
                "",
                "typed_record",
                "record does not match closed typed contract",
            )]
        })?;
        record
            .validate_semantics(self)
            .map_err(|error| vec![error])?;
        Ok(Validated(record))
    }

    fn event_binding(&self, envelope: &EventEnvelope) -> Option<&EventTypeBinding> {
        self.manifest.event_bindings.iter().find(|binding| {
            binding.event_type == envelope.event_type
                && binding.major == envelope.event_major
                && binding.minor == envelope.event_minor
        })
    }

    fn exact_schema(&self, schema_type: &str) -> Option<&SchemaRef> {
        let mut matches = self
            .manifest
            .schemas
            .iter()
            .filter(|schema| schema.r#type == schema_type);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    #[allow(dead_code)] // Consumed by the future in-crate kernel module; unit tests exercise it now.
    pub(crate) fn trusted_context(
        &self,
        scope: IsolationScope,
        actor: AgentId,
        target_stream: StreamId,
        protocol_limits_ref: ProtocolLimitsRef,
    ) -> TrustedValidationContext {
        TrustedValidationContext {
            scope,
            actor,
            target_stream,
            schema_set_ref: self.reference.clone(),
            protocol_limits_ref,
        }
    }

    /// Kernel-facing validation entry point; validation does not grant authorization/capability.
    pub fn validate_event_at_boundary(
        &self,
        envelope: EventEnvelope,
        expected_scope: IsolationScope,
        authenticated_actor: AgentId,
        target_stream: StreamId,
        protocol_limits_ref: ProtocolLimitsRef,
    ) -> Result<ValidatedEvent, Vec<ValidationError>> {
        let context = self.trusted_context(
            expected_scope,
            authenticated_actor,
            target_stream,
            protocol_limits_ref,
        );
        self.validate_event(envelope, &context)
    }

    /// Validates an event against the exact run context, registry binding, limits, and payload digest.
    pub fn validate_event(
        &self,
        envelope: EventEnvelope,
        context: &TrustedValidationContext,
    ) -> Result<ValidatedEvent, Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Err(error) = validate_limits_ref(&context.protocol_limits_ref) {
            errors.push(error);
        }
        if let Ok(value) = serde_json::to_value(&envelope) {
            if let Err(error) = validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES) {
                errors.push(error);
            }
        }
        if let Err(error) =
            validate_value_limits(&envelope.payload, ProtocolLimitsV1::PAYLOAD_BYTES)
        {
            errors.push(error);
        }
        if !errors.is_empty() {
            sort_and_truncate(&mut errors);
            return Err(errors);
        }
        if context.schema_set_ref != self.reference {
            errors.push(ValidationError::new(
                ErrorCode::SchemaMismatch,
                "/schema_set_ref",
                "trusted_context",
                "wrong schema set for run",
            ));
        }
        validate_scope(&envelope.scope, &context.scope, &mut errors);
        if envelope.actor != context.actor {
            errors.push(scope_error("/actor", "actor mismatch"));
        }
        if envelope.stream_id != context.target_stream {
            errors.push(scope_error("/stream_id", "stream mismatch"));
        }
        if envelope.run_id != context.scope.run_id {
            errors.push(scope_error("/run_id", "run mismatch"));
        }
        if envelope.schema_ref != self.manifest.event_envelope_schema_ref {
            errors.push(schema_error(
                "/schema_ref",
                "envelope schema is not the exact schema bound by this set",
            ));
        } else if let (Some(validator), Ok(value)) = (
            self.validators.get(&envelope.schema_ref),
            serde_json::to_value(&envelope),
        ) {
            validate_json_schema(validator, &value, "", &mut errors);
        }
        if !self.contains(&envelope.payload_schema_ref) {
            errors.push(schema_error(
                "/payload_schema_ref",
                "payload schema is not a set member",
            ));
        }
        match self.event_binding(&envelope) {
            Some(binding) if binding.payload_schema_ref == envelope.payload_schema_ref => {
                if binding.variant_id.is_empty() {
                    errors.push(ValidationError::new(
                        ErrorCode::EventTypeMismatch,
                        "/event_type",
                        "event_type_registry",
                        "typed variant identifier is empty",
                    ));
                }
                if let Some(validator) = self.validators.get(&binding.payload_schema_ref) {
                    validate_json_schema(validator, &envelope.payload, "/payload", &mut errors);
                } else {
                    errors.push(schema_error(
                        "/payload_schema_ref",
                        "payload schema document is unavailable",
                    ));
                }
            }
            _ => errors.push(ValidationError::new(
                ErrorCode::EventTypeMismatch,
                "/payload_schema_ref",
                "event_type_registry",
                "event type/version is not bound to payload schema",
            )),
        }
        if envelope.sequence.is_empty()
            || envelope.sequence == "0"
            || !is_decimal(&envelope.sequence)
        {
            errors.push(ValidationError::new(
                ErrorCode::InvariantViolation,
                "/sequence",
                "positive_decimal",
                "sequence must be positive canonical decimal",
            ));
        }
        if !is_canonical_timestamp(&envelope.occurred_at) {
            errors.push(ValidationError::new(
                ErrorCode::InvalidTimestamp,
                "/occurred_at",
                "utc_rfc3339_millis",
                "timestamp must use canonical UTC milliseconds",
            ));
        }
        match digest_json(
            "event-payload",
            &envelope.payload_schema_ref,
            &envelope.payload,
        ) {
            Ok(actual) if actual == envelope.payload_digest => {}
            _ => errors.push(ValidationError::new(
                ErrorCode::DigestMismatch,
                "/payload_digest",
                "event_payload",
                "payload digest mismatch",
            )),
        }
        sort_and_truncate(&mut errors);
        if errors.is_empty() {
            let variant_id = self
                .event_binding(&envelope)
                .expect("successful binding")
                .variant_id
                .clone();
            let decoded = self
                .decoders
                .get(&variant_id)
                .expect("admission requires decoder")
                .decode(&envelope.payload)
                .map_err(|error| vec![error])?;
            Ok(ValidatedEvent {
                envelope,
                schema_set_ref: context.schema_set_ref.clone(),
                protocol_limits_ref: context.protocol_limits_ref.clone(),
                variant_id,
                decoded,
            })
        } else {
            Err(errors)
        }
    }

    /// Validates a run manifest at the create-run boundary.
    pub fn validate_run_manifest(
        &self,
        manifest: RunManifest,
        expected_scope: &IsolationScope,
    ) -> Result<Validated<RunManifest>, Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Ok(value) = serde_json::to_value(&manifest) {
            if let Err(error) = validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES) {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        validate_scope(&manifest.scope, expected_scope, &mut errors);
        if manifest.schema_set_ref != self.reference {
            errors.push(schema_error(
                "/schema_set_ref",
                "run manifest does not pin the admitted schema set",
            ));
        }
        if let Err(error) = validate_limits_ref(&manifest.protocol_limits_ref) {
            errors.push(error);
        }
        if self.exact_schema("run-manifest") != Some(&manifest.schema_ref) {
            errors.push(schema_error(
                "/schema_ref",
                "run manifest does not use the exact schema bound by this set",
            ));
        } else if let (Some(validator), Ok(value)) = (
            self.validators.get(&manifest.schema_ref),
            serde_json::to_value(&manifest),
        ) {
            validate_json_schema(validator, &value, "", &mut errors);
        }
        let mut required_roles = vec![
            "task",
            "behavior",
            "workspace",
            "environment",
            "context_graph",
            "model_snapshot",
            "tool_set",
            "kernel",
        ];
        let is_hook_manifest = manifest.schema_ref.major == 2;
        if is_hook_manifest {
            required_roles.push("hook_registry");
        }
        for role in &required_roles {
            if !manifest.revisions.contains_key(*role) {
                errors.push(ValidationError::new(
                    ErrorCode::InvariantViolation,
                    &format!("/revisions/{role}"),
                    "run_manifest",
                    "required revision pin is missing",
                ));
            }
        }
        if manifest.revisions.len() != required_roles.len() {
            errors.push(ValidationError::new(
                ErrorCode::InvariantViolation,
                "/revisions",
                "run_manifest",
                "revision roles must equal the closed required set",
            ));
        }
        if is_hook_manifest != manifest.hook_registry_config_digest.is_some() {
            errors.push(ValidationError::new(
                ErrorCode::InvariantViolation,
                "/hook_registry_config_digest",
                "run_manifest",
                "V2 requires and V1 forbids a Hook registry config digest",
            ));
        }
        if let Err(error) = manifest.execution_mode.validate(&manifest.scope.run_id) {
            errors.push(error);
        }
        sort_and_truncate(&mut errors);
        if errors.is_empty() {
            Ok(Validated(manifest))
        } else {
            Err(errors)
        }
    }

    /// Validates evidence scope, schema membership, time, and admission fields.
    pub fn validate_evidence(
        &self,
        evidence: EvidenceRecord,
        expected_scope: &IsolationScope,
        protocol_limits_ref: &ProtocolLimitsRef,
    ) -> Result<Validated<EvidenceRecord>, Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Err(error) = validate_limits_ref(protocol_limits_ref) {
            errors.push(error);
        }
        if let Ok(value) = serde_json::to_value(&evidence) {
            if let Err(error) = validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES) {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            sort_and_truncate(&mut errors);
            return Err(errors);
        }
        validate_scope(&evidence.scope, expected_scope, &mut errors);
        if self.exact_schema("evidence-record") != Some(&evidence.schema_ref) {
            errors.push(schema_error(
                "/schema_ref",
                "evidence does not use the exact schema bound by this set",
            ));
        } else if let (Some(validator), Ok(value)) = (
            self.validators.get(&evidence.schema_ref),
            serde_json::to_value(&evidence),
        ) {
            validate_json_schema(validator, &value, "", &mut errors);
        }
        if !is_canonical_timestamp(&evidence.observed_at) {
            errors.push(ValidationError::new(
                ErrorCode::InvalidTimestamp,
                "/observed_at",
                "utc_rfc3339_millis",
                "timestamp must use canonical UTC milliseconds",
            ));
        }
        for (path, value) in [
            ("/claim", &evidence.claim),
            ("/evidence_type", &evidence.evidence_type),
            ("/evidence_scope", &evidence.evidence_scope),
            ("/freshness", &evidence.freshness),
        ] {
            if value.is_empty() {
                errors.push(ValidationError::new(
                    ErrorCode::InvariantViolation,
                    path,
                    "evidence_record",
                    "field must not be empty",
                ));
            }
        }
        sort_and_truncate(&mut errors);
        if errors.is_empty() {
            Ok(Validated(evidence))
        } else {
            Err(errors)
        }
    }

    /// Admits a finalized boundary inventory only when it matches the validated source run.
    pub fn validate_boundary_inventory(
        &self,
        inventory: crate::BoundaryInventoryRevision,
        source_manifest: crate::RunManifest,
        expected_source_scope: &IsolationScope,
    ) -> Result<Validated<crate::BoundaryInventoryRevision>, Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Ok(value) = serde_json::to_value(&inventory) {
            if let Err(error) = validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES) {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let validated_source =
            self.validate_run_manifest(source_manifest, expected_source_scope)?;
        if inventory.source_run_id != validated_source.get().scope.run_id {
            errors.push(scope_error(
                "/source_run_id",
                "inventory source run does not match the validated source manifest",
            ));
        }
        if inventory.recording_policy_ref != validated_source.get().boundary_recording_policy_ref {
            errors.push(ValidationError::new(
                ErrorCode::InvariantViolation,
                "/recording_policy_ref",
                "boundary_inventory_revision",
                "inventory recording policy does not match the validated source manifest",
            ));
        }
        if inventory.schema_set_ref != self.reference {
            errors.push(schema_error(
                "/schema_set_ref",
                "inventory does not pin the exact admitted SchemaSet",
            ));
        }
        if self.exact_schema("boundary-inventory-revision") != Some(&inventory.metadata.schema_ref)
        {
            errors.push(schema_error(
                "/metadata/schema_ref",
                "inventory does not use the exact admitted top-level Schema",
            ));
        } else if let (Some(validator), Ok(value)) = (
            self.validators.get(&inventory.metadata.schema_ref),
            serde_json::to_value(&inventory),
        ) {
            validate_json_schema(validator, &value, "", &mut errors);
        }
        if self.exact_schema("boundary-inventory-hash-view") != Some(&inventory.hash_schema_ref) {
            errors.push(schema_error(
                "/hash_schema_ref",
                "inventory hash view does not use the exact admitted Schema",
            ));
        }
        if let Err(error) = inventory.validate() {
            errors.push(error);
        }
        sort_and_truncate(&mut errors);
        if errors.is_empty() {
            Ok(Validated(inventory))
        } else {
            Err(errors)
        }
    }

    /// Admits a late-result reconciliation against one already validated finalized inventory.
    pub fn validate_boundary_reconciliation(
        &self,
        reconciliation: crate::BoundaryReconciliationRevision,
        inventory: &Validated<crate::BoundaryInventoryRevision>,
    ) -> Result<Validated<crate::BoundaryReconciliationRevision>, Vec<ValidationError>> {
        let mut errors = Vec::new();
        if let Ok(value) = serde_json::to_value(&reconciliation) {
            if let Err(error) = validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES) {
                errors.push(error);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        if reconciliation.inventory_revision != inventory.get().metadata.revision_id {
            errors.push(ValidationError::new(
                ErrorCode::InvariantViolation,
                "/inventory_revision",
                "boundary_reconciliation_revision",
                "reconciliation does not reference the validated finalized inventory",
            ));
        }
        if self.exact_schema("boundary-reconciliation-revision")
            != Some(&reconciliation.metadata.schema_ref)
        {
            errors.push(schema_error(
                "/metadata/schema_ref",
                "reconciliation does not use the exact admitted top-level Schema",
            ));
        } else if let (Some(validator), Ok(value)) = (
            self.validators.get(&reconciliation.metadata.schema_ref),
            serde_json::to_value(&reconciliation),
        ) {
            validate_json_schema(validator, &value, "", &mut errors);
        }
        if self.exact_schema("boundary-reconciliation-hash-view")
            != Some(&reconciliation.hash_schema_ref)
        {
            errors.push(schema_error(
                "/hash_schema_ref",
                "reconciliation hash view does not use the exact admitted Schema",
            ));
        }
        if let Err(error) = reconciliation.validate() {
            errors.push(error);
        }
        sort_and_truncate(&mut errors);
        if errors.is_empty() {
            Ok(Validated(reconciliation))
        } else {
            Err(errors)
        }
    }
}

/// Parses a closed typed record and applies raw and canonical semantic limits.
pub fn parse_bounded<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ValidationError> {
    let value = parse_bounded_value(bytes)?;
    serde_json::from_value(value).map_err(|_| {
        ValidationError::new(
            ErrorCode::InvalidJson,
            "",
            "typed_record",
            "record does not match closed typed contract",
        )
    })
}

fn parse_bounded_value(bytes: &[u8]) -> Result<Value, ValidationError> {
    if bytes.len() > ProtocolLimitsV1::RAW_RECORD_BYTES {
        return Err(limit("raw record byte ceiling exceeded"));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = deserializer
        .deserialize_any(UniqueValueVisitor)
        .map_err(|_| {
            ValidationError::new(
                ErrorCode::InvalidJson,
                "",
                "json",
                "invalid JSON or duplicate object key",
            )
        })?;
    deserializer.end().map_err(|_| {
        ValidationError::new(ErrorCode::InvalidJson, "", "json", "trailing JSON data")
    })?;
    validate_value_limits(&value, ProtocolLimitsV1::RECORD_BYTES)?;
    Ok(value)
}

fn validate_schema_documents(
    manifest: &SchemaSetManifest,
    documents: Vec<SchemaDocument>,
) -> Result<AdmittedSchemas, ValidationError> {
    if documents.len() != manifest.schemas.len() {
        return Err(schema_error(
            "/schemas",
            "member document count differs from manifest",
        ));
    }
    let mut verified = BTreeMap::new();
    let mut validators = BTreeMap::new();
    for document in documents {
        let schema_id = document
            .document
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("/$id", "schema document lacks ID"))?;
        if document.document.get("$schema").and_then(Value::as_str)
            != Some("https://json-schema.org/draft/2020-12/schema")
        {
            return Err(schema_error(
                "/$schema",
                "schema document does not use the fixed metaschema",
            ));
        }
        // SHA-256 of the Draft 2020-12 root meta-schema embedded by jsonschema 0.50.0.
        let _pinned_metaschema_digest =
            "sha256:483c0526fdeb85e072d9cca4eee4ba7f1179d1ce89cb21c42b3c01296442f9e6";
        if !jsonschema::draft202012::meta::is_valid(&document.document) {
            return Err(schema_error(
                "",
                "schema document fails pinned Draft 2020-12 meta-validation",
            ));
        }
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&document.document)
            .map_err(|_| schema_error("", "schema document fails Draft 2020-12 compilation"))?;
        let suffix = schema_id
            .strip_prefix("urn:pareto-harness:schema:")
            .ok_or_else(|| schema_error("/$id", "schema ID prefix mismatch"))?;
        let (name, version) = suffix
            .rsplit_once(':')
            .ok_or_else(|| schema_error("/$id", "schema ID lacks version"))?;
        let (major, minor) = version
            .split_once('.')
            .ok_or_else(|| schema_error("/$id", "schema ID lacks major/minor"))?;
        for component in [major, minor] {
            if component.is_empty()
                || (component.len() > 1 && component.starts_with('0'))
                || !component.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(schema_error(
                    "/$id",
                    "schema version is not canonical decimal",
                ));
            }
        }
        let digest = digest_schema(schema_id, &document.document)?;
        let reference = SchemaRef {
            r#type: name.to_owned(),
            major: major
                .parse()
                .map_err(|_| schema_error("/$id", "invalid major"))?,
            minor: minor
                .parse()
                .map_err(|_| schema_error("/$id", "invalid minor"))?,
            schema_digest: digest,
        };
        if manifest.schemas.binary_search(&reference).is_err() {
            return Err(schema_error(
                "/$id",
                "schema bytes do not match a manifest member",
            ));
        }
        if verified
            .insert(reference.clone(), document.document)
            .is_some()
        {
            return Err(schema_error("/$id", "duplicate schema document"));
        }
        validators.insert(reference, Arc::new(validator));
    }
    Ok((verified, validators))
}

fn validate_schema_evolution(
    parent: &SchemaSet,
    candidate: &BTreeMap<SchemaRef, Value>,
) -> Result<(), ValidationError> {
    for (new_ref, new_schema) in candidate {
        if let Some((_, old_schema)) = parent
            ._documents
            .iter()
            .find(|(old_ref, _)| old_ref.r#type == new_ref.r#type && old_ref.major == new_ref.major)
        {
            crate::prove_old_writer_new_reader(old_schema, new_schema)?;
        }
    }
    Ok(())
}

fn validate_json_schema(
    validator: &jsonschema::Validator,
    instance: &Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    for error in validator.iter_errors(instance) {
        let suffix = error.instance_path().as_str();
        errors.push(schema_error(
            &format!("{path}{suffix}"),
            "payload does not match admitted Draft 2020-12 schema",
        ));
    }
}

fn validate_manifest_order(manifest: &SchemaSetManifest) -> Result<(), ValidationError> {
    if !manifest.schemas.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(schema_error(
            "/schemas",
            "schemas must be strictly sorted and unique",
        ));
    }
    let mut ids = BTreeSet::new();
    for schema in &manifest.schemas {
        let id = format!("{}:{}.{}", schema.r#type, schema.major, schema.minor);
        if !ids.insert(id) {
            return Err(schema_error("/schemas", "duplicate schema identity"));
        }
    }
    if !manifest.event_bindings.windows(2).all(|pair| {
        (&pair[0].event_type, pair[0].major, pair[0].minor)
            < (&pair[1].event_type, pair[1].major, pair[1].minor)
    }) {
        return Err(schema_error(
            "/event_bindings",
            "event bindings must be strictly sorted and unique",
        ));
    }
    if manifest.event_bindings.iter().any(|binding| {
        manifest
            .schemas
            .binary_search(&binding.payload_schema_ref)
            .is_err()
    }) {
        return Err(schema_error(
            "/event_bindings",
            "event binding references a non-member schema",
        ));
    }
    if manifest
        .schemas
        .binary_search(&manifest.event_envelope_schema_ref)
        .is_err()
        || manifest.event_envelope_schema_ref.r#type != "event-envelope"
    {
        return Err(schema_error(
            "/event_envelope_schema_ref",
            "exact event envelope schema must be a set member",
        ));
    }
    Ok(())
}

fn validate_scope(
    actual: &IsolationScope,
    expected: &IsolationScope,
    errors: &mut Vec<ValidationError>,
) {
    if actual.tenant_id != expected.tenant_id {
        errors.push(scope_error("/scope/tenant_id", "tenant mismatch"));
    }
    if actual.user_id != expected.user_id {
        errors.push(scope_error(
            "/scope/user_id",
            "user presence or value mismatch",
        ));
    }
    if actual.workspace_id != expected.workspace_id {
        errors.push(scope_error("/scope/workspace_id", "workspace mismatch"));
    }
    if actual.run_id != expected.run_id {
        errors.push(scope_error("/scope/run_id", "run mismatch"));
    }
    if actual.agent_id != expected.agent_id {
        errors.push(scope_error("/scope/agent_id", "agent mismatch"));
    }
}

fn validate_value_limits(value: &Value, byte_limit: usize) -> Result<(), ValidationError> {
    walk_limits(value, 1)?;
    if canonical_json_bytes(value)?.len() > byte_limit {
        return Err(limit("canonical byte ceiling exceeded"));
    }
    Ok(())
}

fn validate_limits_ref(reference: &ProtocolLimitsRef) -> Result<(), ValidationError> {
    if reference.profile == "protocol-limits-v1"
        && reference.digest.as_str() == ProtocolLimitsV1::DIGEST
    {
        Ok(())
    } else {
        Err(ValidationError::new(
            ErrorCode::InvariantViolation,
            "/protocol_limits_ref",
            "protocol_limits_v1",
            "limits profile identity is not the exact supported V1 reference",
        ))
    }
}

fn walk_limits(value: &Value, depth: usize) -> Result<(), ValidationError> {
    if depth > ProtocolLimitsV1::DEPTH {
        return Err(limit("nesting depth exceeded"));
    }
    match value {
        Value::String(value) if value.len() > ProtocolLimitsV1::STRING_BYTES => {
            Err(limit("decoded string byte ceiling exceeded"))
        }
        Value::Array(values) => {
            if values.len() > ProtocolLimitsV1::COLLECTION {
                return Err(limit("array element ceiling exceeded"));
            }
            for value in values {
                walk_limits(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > ProtocolLimitsV1::COLLECTION {
                return Err(limit("object member ceiling exceeded"));
            }
            for (name, value) in values {
                if name.len() > ProtocolLimitsV1::STRING_BYTES {
                    return Err(limit("object name byte ceiling exceeded"));
                }
                walk_limits(value, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_decimal(value: &str) -> bool {
    value == "0" || (!value.starts_with('0') && value.bytes().all(|byte| byte.is_ascii_digit()))
}

pub(crate) fn is_canonical_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    let shape = bytes.len() == 24
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'.'
        && bytes[23] == b'Z'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19 | 23) || byte.is_ascii_digit()
        });
    if !shape {
        return false;
    }
    let number = |start: usize, end: usize| value[start..end].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second), Some(_millis)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
        number(20, 23),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day >= 1 && day <= max_day && hour <= 23 && minute <= 59 && second <= 59
}

fn sort_and_truncate(errors: &mut Vec<ValidationError>) {
    errors.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| format!("{:?}", left.code).cmp(&format!("{:?}", right.code)))
    });
    errors.truncate(ProtocolLimitsV1::ERRORS);
}

fn scope_error(path: &str, detail: &str) -> ValidationError {
    ValidationError::new(
        ErrorCode::ScopeMismatch,
        path,
        "trusted_validation_context",
        detail,
    )
}
fn schema_error(path: &str, detail: &str) -> ValidationError {
    ValidationError::new(ErrorCode::SchemaMismatch, path, "schema_set", detail)
}
fn limit(detail: &str) -> ValidationError {
    ValidationError::new(ErrorCode::LimitExceeded, "", "protocol_limits_v1", detail)
}
fn invariant(detail: &str) -> ValidationError {
    ValidationError::new(ErrorCode::InvariantViolation, "", "protocol", detail)
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, _value: f64) -> Result<Value, E> {
        Err(E::custom("floating point values are forbidden"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_some<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueValueSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(name) = map.next_key::<String>()? {
            if values.contains_key(&name) {
                return Err(serde::de::Error::custom("duplicate object key"));
            }
            values.insert(name, map.next_value_seed(UniqueValueSeed)?);
        }
        Ok(Value::Object(values))
    }
}

struct UniqueValueSeed;

impl<'de> serde::de::DeserializeSeed<'de> for UniqueValueSeed {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

macro_rules! protocol_record {
    ($type:ty, $schema:literal) => {
        impl sealed::ProtocolRecord for $type {}
        impl ProtocolRecord for $type {
            const SCHEMA_TYPE: &'static str = $schema;
        }
    };
}

protocol_record!(crate::ArtifactManifest, "artifact-manifest");
protocol_record!(
    crate::BoundaryInventoryHashView,
    "boundary-inventory-hash-view"
);
protocol_record!(
    crate::BoundaryReconciliationHashView,
    "boundary-reconciliation-hash-view"
);
protocol_record!(crate::RevisionHashView, "revision-hash-view");
protocol_record!(crate::ProjectionHistorySeedV1, "projection-history-seed");
protocol_record!(crate::ProjectionHistoryStepV1, "projection-history-step");
protocol_record!(
    crate::ProjectionReducerDescriptorV1,
    "projection-reducer-descriptor"
);
protocol_record!(crate::ProjectionReducerRef, "projection-reducer-ref");
protocol_record!(crate::HookProjectionV1, "hook-projection");
protocol_record!(crate::HookProjectionHashViewV1, "hook-projection-hash-view");
protocol_record!(crate::RunTaskProjection, "run-task-projection");
protocol_record!(
    crate::RunTaskProjectionHashViewV1,
    "run-task-projection-hash-view"
);
protocol_record!(
    crate::RunTaskProjectionSnapshot,
    "run-task-projection-snapshot"
);
protocol_record!(
    crate::RunTaskProjectionSnapshotHashViewV1,
    "run-task-projection-snapshot-hash-view"
);
protocol_record!(crate::SourceReducerKeyV1, "source-reducer-key");
protocol_record!(crate::SchemaSetManifest, "schema-set-manifest");

impl sealed::ProtocolRecord for ProtocolLimitsProfileV1 {}
impl ProtocolRecord for ProtocolLimitsProfileV1 {
    const SCHEMA_TYPE: &'static str = "protocol-limits-profile";
    fn validate_semantics(&self, _set: &SchemaSet) -> Result<(), ValidationError> {
        if self == &ProtocolLimitsV1::profile() {
            Ok(())
        } else {
            Err(schema_error(
                "",
                "limits profile is not the exact V1 profile",
            ))
        }
    }
}

impl sealed::ProtocolRecord for crate::RevisionMetadata {}
impl ProtocolRecord for crate::RevisionMetadata {
    const SCHEMA_TYPE: &'static str = "revision-metadata";
    fn validate_semantics(&self, _set: &SchemaSet) -> Result<(), ValidationError> {
        self.validate_identity()
    }
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<SchemaSet>();
    check::<ValidatedEvent>();
    check::<AgentId>();
    check::<StreamId>();
    check::<Digest>();
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::{EventId, ProtocolLimitsRef, RunId, SchemaDocument, TenantId, UserId};

    fn digest(hex: char) -> Digest {
        Digest::parse(format!("sha256:{}", hex.to_string().repeat(64))).unwrap()
    }
    fn document(name: &str, body: Value) -> (SchemaDocument, SchemaRef) {
        let mut object = body.as_object().cloned().unwrap();
        let id = format!("urn:pareto-harness:schema:{name}:1.0");
        object.insert("$id".to_owned(), Value::String(id.clone()));
        object.insert(
            "$schema".to_owned(),
            Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
        );
        let value = Value::Object(object);
        let reference = SchemaRef {
            r#type: name.to_owned(),
            major: 1,
            minor: 0,
            schema_digest: digest_schema(&id, &value).unwrap(),
        };
        (
            SchemaDocument {
                filename: format!("{name}.json"),
                document: value,
            },
            reference,
        )
    }

    fn scope() -> IsolationScope {
        IsolationScope {
            tenant_id: TenantId::parse("tenant_local").unwrap(),
            user_id: Some(UserId::parse("user_alice").unwrap()),
            workspace_id: crate::WorkspaceId::parse("workspace_repo").unwrap(),
            run_id: RunId::parse("run_one").unwrap(),
            agent_id: AgentId::parse("agent_primary").unwrap(),
        }
    }

    struct AllowAdmission;
    impl SchemaAdmissionAuthorizer for AllowAdmission {
        fn authorize(
            &self,
            _parent: Option<&SchemaSetRef>,
            _candidate: &SchemaSetRef,
        ) -> Result<(), ValidationError> {
            Ok(())
        }
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct RunStartedPayload {
        message: String,
    }

    struct RunStartedDecoder(SchemaRef);
    impl EventVariantDecoder for RunStartedDecoder {
        fn variant_id(&self) -> &str {
            "run-started-v1"
        }
        fn payload_schema_ref(&self) -> &SchemaRef {
            &self.0
        }
        fn decode(&self, payload: &Value) -> Result<Box<dyn Any + Send + Sync>, ValidationError> {
            serde_json::from_value::<RunStartedPayload>(payload.clone())
                .map(|value| Box::new(value) as Box<dyn Any + Send + Sync>)
                .map_err(|_| schema_error("/payload", "typed payload decoding failed"))
        }
    }

    fn admitted() -> (SchemaSet, SchemaRef, SchemaRef) {
        let (payload_doc, payload_ref) = document(
            "run-started-payload",
            json!({"type":"object","properties":{"message":{"type":"string"},"part_a":{"type":"string"},"part_b":{"type":"string"}},"required":["message"],"unevaluatedProperties":false}),
        );
        let (envelope_doc, envelope_ref) = document("event-envelope", json!({"type":"object"}));
        let (manifest_doc, manifest_ref) =
            document("schema-set-manifest", json!({"type":"object"}));
        let mut schemas = vec![
            payload_ref.clone(),
            envelope_ref.clone(),
            manifest_ref.clone(),
        ];
        schemas.sort();
        let manifest = SchemaSetManifest {
            schemas,
            event_envelope_schema_ref: envelope_ref.clone(),
            event_bindings: vec![EventTypeBinding {
                event_type: "run-started".to_owned(),
                major: 1,
                minor: 0,
                payload_schema_ref: payload_ref.clone(),
                variant_id: "run-started-v1".to_owned(),
            }],
        };
        let reference = SchemaSetRef {
            manifest_digest: digest_json(
                "schema-set",
                &manifest_ref,
                &serde_json::to_value(&manifest).unwrap(),
            )
            .unwrap(),
            manifest_schema_ref: manifest_ref,
        };
        let set = SchemaSet::admit_with(
            &AllowAdmission,
            None,
            manifest,
            vec![payload_doc, envelope_doc, manifest_doc],
            &reference,
            vec![Arc::new(RunStartedDecoder(payload_ref.clone()))],
        )
        .unwrap();
        (set, payload_ref, envelope_ref)
    }

    fn valid_event(
        set: &SchemaSet,
        payload_ref: SchemaRef,
        envelope_ref: SchemaRef,
    ) -> (EventEnvelope, TrustedValidationContext) {
        let payload = json!({"message":"started"});
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };
        let context = set.trusted_context(
            scope(),
            AgentId::parse("agent_primary").unwrap(),
            StreamId::parse("stream_run-one").unwrap(),
            limits,
        );
        let event = EventEnvelope {
            schema_ref: envelope_ref,
            scope: scope(),
            event_id: EventId::parse("event_one").unwrap(),
            stream_id: StreamId::parse("stream_run-one").unwrap(),
            run_id: RunId::parse("run_one").unwrap(),
            sequence: "1".to_owned(),
            causation_id: None,
            correlation_id: "corr-one".to_owned(),
            event_type: "run-started".to_owned(),
            event_major: 1,
            event_minor: 0,
            occurred_at: "2026-08-22T10:11:12.123Z".to_owned(),
            actor: AgentId::parse("agent_primary").unwrap(),
            payload_digest: digest_json("event-payload", &payload_ref, &payload).unwrap(),
            payload_schema_ref: payload_ref,
            payload,
        };
        (event, context)
    }

    #[test]
    fn bootstrap_rejects_self_signed_or_missing_member_bytes() {
        let bundle = crate::generate_schema_bundle().unwrap();
        assert!(
            SchemaSet::bootstrap_initial(
                bundle.manifest.clone(),
                bundle.schemas.clone(),
                &bundle.reference,
            )
            .is_ok()
        );
        assert!(
            SchemaSet::admit_with(
                &AllowAdmission,
                None,
                bundle.manifest.clone(),
                Vec::new(),
                &bundle.reference,
                Vec::new(),
            )
            .is_err()
        );
        let mut wrong = bundle.reference.clone();
        wrong.manifest_digest = digest('f');
        assert!(
            SchemaSet::admit_with(
                &AllowAdmission,
                None,
                bundle.manifest,
                bundle.schemas,
                &wrong,
                Vec::new()
            )
            .is_err()
        );
    }

    #[test]
    fn isolation_boundaries_and_payload_schema_fail_closed() {
        let (set, payload_ref, envelope_ref) = admitted();
        let (event, context) = valid_event(&set, payload_ref, envelope_ref);
        let validated = set.validate_event(event.clone(), &context).unwrap();
        assert_eq!(
            validated
                .downcast_payload::<RunStartedPayload>()
                .unwrap()
                .message,
            "started"
        );
        for mutate in 0..5 {
            let mut candidate = event.clone();
            match mutate {
                0 => candidate.sequence.clear(),
                1 => candidate.payload = json!({"wrong":true}),
                2 => {
                    candidate.scope.workspace_id =
                        crate::WorkspaceId::parse("workspace_other").unwrap()
                }
                3 => candidate.actor = AgentId::parse("agent_other").unwrap(),
                _ => candidate.schema_ref.r#type = "evidence-record".to_owned(),
            }
            if mutate == 1 {
                candidate.payload_digest = digest_json(
                    "event-payload",
                    &candidate.payload_schema_ref,
                    &candidate.payload,
                )
                .unwrap();
                let errors = set.validate_event(candidate, &context).unwrap_err();
                assert!(
                    errors
                        .iter()
                        .any(|error| error.path.starts_with("/payload"))
                );
            } else {
                assert!(set.validate_event(candidate, &context).is_err());
            }
        }
        let mut wrong_limits = context;
        wrong_limits.protocol_limits_ref.digest = digest('d');
        assert!(set.validate_event(event, &wrong_limits).is_err());
    }

    fn payload_with_canonical_bytes(target: usize) -> Value {
        let empty = json!({"message":"","part_a":"","part_b":""});
        let overhead = canonical_json_bytes(&empty).unwrap().len();
        let content = target - overhead;
        let first = content.min(ProtocolLimitsV1::STRING_BYTES);
        let second = (content - first).min(ProtocolLimitsV1::STRING_BYTES);
        let third = content - first - second;
        assert!(third <= ProtocolLimitsV1::STRING_BYTES);
        json!({
            "message":"x".repeat(first),
            "part_a":"x".repeat(second),
            "part_b":"x".repeat(third)
        })
    }

    #[test]
    fn typed_event_payload_and_record_bytes_are_exact() {
        let (set, payload_ref, envelope_ref) = admitted();
        let (mut event, context) = valid_event(&set, payload_ref.clone(), envelope_ref);

        event.payload = payload_with_canonical_bytes(ProtocolLimitsV1::PAYLOAD_BYTES);
        event.payload_digest = digest_json("event-payload", &payload_ref, &event.payload).unwrap();
        assert_eq!(
            canonical_json_bytes(&event.payload).unwrap().len(),
            ProtocolLimitsV1::PAYLOAD_BYTES
        );
        assert!(set.validate_event(event.clone(), &context).is_ok());

        let mut payload_over = event.clone();
        payload_over.payload = payload_with_canonical_bytes(ProtocolLimitsV1::PAYLOAD_BYTES + 1);
        payload_over.payload_digest =
            digest_json("event-payload", &payload_ref, &payload_over.payload).unwrap();
        assert!(
            set.validate_event(payload_over, &context)
                .unwrap_err()
                .iter()
                .any(|error| error.code == ErrorCode::LimitExceeded)
        );

        let current = canonical_json_bytes(&serde_json::to_value(&event).unwrap())
            .unwrap()
            .len();
        event.correlation_id =
            "c".repeat(event.correlation_id.len() + ProtocolLimitsV1::RECORD_BYTES - current);
        assert_eq!(
            canonical_json_bytes(&serde_json::to_value(&event).unwrap())
                .unwrap()
                .len(),
            ProtocolLimitsV1::RECORD_BYTES
        );
        assert!(set.validate_event(event.clone(), &context).is_ok());
        event.correlation_id.push('c');
        assert!(
            set.validate_event(event, &context)
                .unwrap_err()
                .iter()
                .any(|error| error.code == ErrorCode::LimitExceeded)
        );

        let minified = serde_json::to_vec(&payload_with_canonical_bytes(1024)).unwrap();
        let pretty = serde_json::to_vec_pretty(&payload_with_canonical_bytes(1024)).unwrap();
        assert_eq!(
            canonical_json_bytes(&parse_bounded_value(&minified).unwrap()).unwrap(),
            canonical_json_bytes(&parse_bounded_value(&pretty).unwrap()).unwrap()
        );
        let escaped = br#"{"message":"\u0078"}"#;
        assert_eq!(
            canonical_json_bytes(&parse_bounded_value(escaped).unwrap()).unwrap(),
            br#"{"message":"x"}"#
        );
    }

    #[test]
    fn typed_run_and_evidence_record_bytes_are_exact() {
        let bundle = crate::generate_schema_bundle().unwrap();
        let schema = |name: &str| {
            bundle
                .manifest
                .schemas
                .iter()
                .find(|item| item.r#type == name)
                .unwrap()
                .clone()
        };
        let set = SchemaSet::bootstrap_initial(
            bundle.manifest.clone(),
            bundle.schemas.clone(),
            &bundle.reference,
        )
        .unwrap();
        let limits = ProtocolLimitsRef {
            profile: "protocol-limits-v1".to_owned(),
            digest: Digest::parse(ProtocolLimitsV1::DIGEST).unwrap(),
        };

        let mut revisions = std::collections::BTreeMap::new();
        for index in 0..12_000 {
            revisions.insert(
                format!("{}-{index:05}", "r".repeat(50)),
                crate::RevisionId::parse("rev_x").unwrap(),
            );
        }
        revisions.insert(
            "padding".to_owned(),
            crate::RevisionId::parse("rev_x").unwrap(),
        );
        let mut run = RunManifest {
            schema_ref: schema("run-manifest"),
            scope: scope(),
            revisions,
            hook_registry_config_digest: Some(digest('e')),
            plan_revision: None,
            schema_set_ref: bundle.reference.clone(),
            budget_revision: crate::RevisionId::parse("rev_budget").unwrap(),
            protocol_limits_ref: limits.clone(),
            boundary_recording_policy_ref: crate::BoundaryRecordingPolicyRef {
                revision_id: crate::RevisionId::parse("rev_policy").unwrap(),
                digest: digest('f'),
            },
            execution_mode: crate::ExecutionMode::Live {},
        };
        let base = canonical_json_bytes(&serde_json::to_value(&run).unwrap())
            .unwrap()
            .len();
        assert!(base < ProtocolLimitsV1::RECORD_BYTES);
        let padding = "p".repeat(ProtocolLimitsV1::RECORD_BYTES - base);
        let value = run.revisions.remove("padding").unwrap();
        run.revisions.insert(format!("padding{padding}"), value);
        assert_eq!(
            canonical_json_bytes(&serde_json::to_value(&run).unwrap())
                .unwrap()
                .len(),
            ProtocolLimitsV1::RECORD_BYTES
        );
        assert!(
            set.validate_run_manifest(run.clone(), &scope())
                .unwrap_err()
                .iter()
                .all(|error| error.code != ErrorCode::LimitExceeded)
        );
        let (pad_key, value) = run.revisions.pop_last().unwrap();
        run.revisions.insert(format!("{pad_key}p"), value);
        assert!(
            set.validate_run_manifest(run, &scope())
                .unwrap_err()
                .iter()
                .any(|error| error.code == ErrorCode::LimitExceeded)
        );

        let mut evidence = crate::EvidenceRecord {
            schema_ref: schema("evidence-record"),
            scope: scope(),
            requirement_id: crate::RequirementId::parse("req_0003").unwrap(),
            claim: "claim".to_owned(),
            evidence_type: "contract".to_owned(),
            producer_revision: crate::RevisionId::parse("rev_producer").unwrap(),
            verifier_revision: crate::RevisionId::parse("rev_verifier").unwrap(),
            subject_revision: crate::RevisionId::parse("rev_subject").unwrap(),
            artifact_digest: digest('a'),
            verdict: crate::EvidenceVerdict::Passed,
            evidence_scope: "scope".to_owned(),
            freshness: "exact-commit".to_owned(),
            limitations: vec![String::new(); 4],
            observed_at: "2026-08-22T10:11:12.123Z".to_owned(),
        };
        let base = canonical_json_bytes(&serde_json::to_value(&evidence).unwrap())
            .unwrap()
            .len();
        let mut remaining = ProtocolLimitsV1::RECORD_BYTES - base;
        for item in &mut evidence.limitations {
            let length = remaining.min(ProtocolLimitsV1::STRING_BYTES);
            *item = "l".repeat(length);
            remaining -= length;
        }
        assert_eq!(remaining, 0);
        assert_eq!(
            canonical_json_bytes(&serde_json::to_value(&evidence).unwrap())
                .unwrap()
                .len(),
            ProtocolLimitsV1::RECORD_BYTES
        );
        assert!(
            set.validate_evidence(evidence.clone(), &scope(), &limits)
                .is_ok()
        );
        evidence.limitations.last_mut().unwrap().push('l');
        assert!(
            set.validate_evidence(evidence, &scope(), &limits)
                .unwrap_err()
                .iter()
                .any(|error| error.code == ErrorCode::LimitExceeded)
        );
    }

    #[test]
    fn errors_are_sorted_and_truncated_without_payload_echo() {
        let mut errors: Vec<_> = (0..40)
            .rev()
            .map(|index| {
                ValidationError::new(
                    ErrorCode::SchemaMismatch,
                    &format!("/payload/{index:02}"),
                    "schema_set",
                    "safe summary",
                )
            })
            .collect();
        sort_and_truncate(&mut errors);
        assert_eq!(errors.len(), ProtocolLimitsV1::ERRORS);
        assert!(errors.windows(2).all(|pair| pair[0].path <= pair[1].path));
        assert!(errors.iter().all(|error| error.detail == "safe summary"));
    }
}
