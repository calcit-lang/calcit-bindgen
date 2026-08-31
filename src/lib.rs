//! Versioned Calcit FFI Interface IR validation and compatibility analysis.

mod compatibility;
mod generate;
mod model;
mod validate;

pub use compatibility::{Change, ChangeKind, CompatibilityReport, compare};
pub use generate::{
    ArtifactDigest, CheckIssue, CheckIssueKind, CheckReport, INTERFACE_FILE, MANIFEST_FILE,
    Manifest, check_directory, generate_directory,
};
pub use model::{
    Declaration, Definition, DefinitionStatus, Document, EnumVariant, Envelope, FunctionSignature,
    Lowering, Parameter, StructField, Type,
};
pub use validate::{load_document, validate_document};
