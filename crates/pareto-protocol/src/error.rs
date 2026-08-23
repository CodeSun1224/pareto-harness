use serde::{Deserialize, Serialize};

/// Stable categories returned at the untrusted protocol boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Input exceeded a versioned resource limit.
    LimitExceeded,
    /// JSON syntax or duplicate object keys are invalid.
    InvalidJson,
    /// A typed identifier has the wrong format or prefix.
    InvalidIdentifier,
    /// A digest is malformed or does not match the content.
    DigestMismatch,
    /// A schema is unknown, duplicated, or outside the pinned set.
    SchemaMismatch,
    /// A trusted scope field differs from the wire value.
    ScopeMismatch,
    /// The event type is not bound to the supplied payload schema.
    EventTypeMismatch,
    /// A timestamp is not canonical UTC RFC 3339 milliseconds.
    InvalidTimestamp,
    /// A semantic cross-field invariant failed.
    InvariantViolation,
}

/// Sanitized validation error that never embeds the rejected payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationError {
    /// Stable machine-readable category.
    pub code: ErrorCode,
    /// JSON Pointer locating the failure.
    pub path: String,
    /// Expected contract identity.
    pub contract: String,
    /// Safe detail without raw untrusted data.
    pub detail: String,
}

impl ValidationError {
    pub(crate) fn new(code: ErrorCode, path: &str, contract: &str, detail: &str) -> Self {
        Self {
            code,
            path: path.to_owned(),
            contract: contract.to_owned(),
            detail: detail.to_owned(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:?} at {} ({})",
            self.code, self.path, self.contract
        )
    }
}

impl std::error::Error for ValidationError {}
