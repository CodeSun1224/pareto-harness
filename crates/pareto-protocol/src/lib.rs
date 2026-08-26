#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Versioned, closed-world protocol contracts for the Pareto Harness kernel.

mod canonical;
mod compatibility;
mod digest;
mod error;
mod runtime_control;
mod schema;
mod types;
mod validation;

pub use canonical::{canonical_json, canonical_json_bytes};
pub use compatibility::prove_old_writer_new_reader;
pub use digest::{
    Digest, derive_revision_id, digest_artifact, digest_json, digest_revision_content,
    digest_schema,
};
pub use error::{ErrorCode, ValidationError};
pub use runtime_control::*;
pub use schema::{
    GeneratedSchemaBundle, SchemaDocument, generate_schema_bundle, generate_schema_set,
};
pub use types::*;
pub use validation::{
    EventVariantDecoder, ProtocolLimitsProfileV1, ProtocolLimitsV1, ProtocolRecord,
    SchemaAdmissionAuthorizer, SchemaSet, Validated, ValidatedEvent, parse_bounded,
};
