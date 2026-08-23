use sha2::{Digest as _, Sha256};

use crate::{
    ArtifactManifest, ErrorCode, RevisionHashView, RevisionId, RevisionMetadata, SchemaRef,
    ValidationError, canonical_json_bytes,
};

/// Lowercase SHA-256 digest in the wire form `sha256:<64 hex>`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, schemars::JsonSchema, serde::Serialize)]
#[serde(transparent)]
pub struct Digest(String);

impl Digest {
    /// Parses and validates a wire digest.
    pub fn parse(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        let hex = value.strip_prefix("sha256:").unwrap_or_default();
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ValidationError::new(
                ErrorCode::DigestMismatch,
                "",
                "digest",
                "expected lowercase sha256 wire digest",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the stable wire representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(|_| serde::de::Error::custom("invalid lowercase sha256 digest"))
    }
}

fn lp(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

/// Digests a validated JSON value in a type and complete-schema domain.
pub fn digest_json(
    domain: &str,
    schema: &SchemaRef,
    value: &serde_json::Value,
) -> Result<Digest, ValidationError> {
    let schema_bytes = canonical_json_bytes(&serde_json::to_value(schema).map_err(|_| {
        ValidationError::new(
            ErrorCode::InvariantViolation,
            "",
            "schema_ref",
            "schema reference serialization failed",
        )
    })?)?;
    let value_bytes = canonical_json_bytes(value)?;
    let mut hasher = Sha256::new();
    lp(&mut hasher, b"pareto-harness-digest-v1");
    lp(&mut hasher, domain.as_bytes());
    lp(&mut hasher, &schema_bytes);
    lp(&mut hasher, &value_bytes);
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
}

/// Builds an artifact manifest and returns its domain-separated identity.
pub fn digest_artifact(
    schema_ref: SchemaRef,
    kind: &str,
    media_type: &str,
    bytes: &[u8],
) -> Result<(ArtifactManifest, Digest), ValidationError> {
    let raw_digest = raw_sha256(bytes)?;
    let manifest = ArtifactManifest {
        schema_ref: schema_ref.clone(),
        artifact_kind: kind.to_owned(),
        media_type: media_type.to_owned(),
        byte_length: bytes.len().to_string(),
        raw_bytes_digest: raw_digest,
    };
    let value = serde_json::to_value(&manifest).map_err(|_| {
        ValidationError::new(
            ErrorCode::InvariantViolation,
            "",
            "artifact_manifest",
            "serialization failed",
        )
    })?;
    let identity = digest_json("artifact-manifest", &schema_ref, &value)?;
    Ok((manifest, identity))
}

/// Computes the behavior-only content digest of a revision hash view.
pub fn digest_revision_content(
    schema_ref: &SchemaRef,
    view: &RevisionHashView,
) -> Result<Digest, ValidationError> {
    let value = serde_json::to_value(view).map_err(|_| {
        ValidationError::new(
            ErrorCode::InvariantViolation,
            "",
            "revision_hash_view",
            "serialization failed",
        )
    })?;
    digest_json(
        &format!("revision:{}", view.revision_kind),
        schema_ref,
        &value,
    )
}

/// Derives the immutable revision ID from every metadata field except `revision_id`.
pub fn derive_revision_id(metadata: &RevisionMetadata) -> Result<RevisionId, ValidationError> {
    let mut value = serde_json::Map::new();
    value.insert(
        "logical_id".to_owned(),
        serde_json::json!(metadata.logical_id),
    );
    value.insert(
        "revision_kind".to_owned(),
        serde_json::json!(metadata.revision_kind),
    );
    if let Some(parent) = &metadata.parent_revision {
        value.insert("parent_revision".to_owned(), serde_json::json!(parent));
    }
    value.insert(
        "schema_ref".to_owned(),
        serde_json::json!(metadata.schema_ref),
    );
    value.insert(
        "content_digest".to_owned(),
        serde_json::json!(metadata.content_digest),
    );
    value.insert(
        "creator_actor".to_owned(),
        serde_json::json!(metadata.creator_actor),
    );
    value.insert("source".to_owned(), serde_json::json!(metadata.source));
    value.insert(
        "created_at".to_owned(),
        serde_json::json!(metadata.created_at),
    );
    let digest = digest_json(
        &format!("revision:{}", metadata.revision_kind),
        &metadata.schema_ref,
        &serde_json::Value::Object(value),
    )?;
    RevisionId::parse(format!("rev_{}", &digest.as_str()[7..]))
}

fn raw_sha256(bytes: &[u8]) -> Result<Digest, ValidationError> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
}

/// Digests a schema without self-referencing its own digest.
pub fn digest_schema(
    schema_id: &str,
    schema: &serde_json::Value,
) -> Result<Digest, ValidationError> {
    if schema.get("$id").and_then(serde_json::Value::as_str) != Some(schema_id) {
        return Err(ValidationError::new(
            ErrorCode::SchemaMismatch,
            "/$id",
            "schema_digest",
            "schema ID argument does not match the document",
        ));
    }
    let value_bytes = canonical_json_bytes(schema)?;
    let mut hasher = Sha256::new();
    lp(&mut hasher, b"pareto-harness-digest-v1");
    lp(&mut hasher, b"schema");
    lp(&mut hasher, schema_id.as_bytes());
    lp(&mut hasher, &value_bytes);
    Digest::parse(format!("sha256:{:x}", hasher.finalize()))
}
