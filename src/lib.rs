//! Versioned Calcit FFI Interface IR validation and compatibility analysis.

mod calcit;
mod compatibility;
mod generate;
mod model;
mod names;
mod rust;
mod typescript;
mod validate;
mod wit;

pub use compatibility::{Change, ChangeKind, CompatibilityReport, compare};
pub use generate::{
    ArtifactDigest, CALCIT_BINDINGS_FILE, CheckIssue, CheckIssueKind, CheckReport,
    GenerationBackend, INTERFACE_FILE, MANIFEST_FILE, Manifest, RUST_BINDINGS_FILE,
    TYPESCRIPT_BINDINGS_FILE, WIT_BINDINGS_FILE, check_directory, check_directory_with_backends,
    generate_directory, generate_directory_with_backends,
};
pub use model::{
    Declaration, Definition, DefinitionStatus, Document, EnumVariant, Envelope, ExportFilters,
    ExportSummary, FunctionSignature, InterfaceDiagnostic, Lowering, Parameter, StructField, Type,
};
pub use validate::{load_document, validate_document};
