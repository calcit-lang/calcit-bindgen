//! Versioned Calcit FFI Interface IR validation and compatibility analysis.

mod calcit;
mod compatibility;
mod component;
mod generate;
mod model;
mod names;
mod rust;
mod typescript;
mod validate;
mod wit;

pub use compatibility::{Change, ChangeKind, CompatibilityReport, compare, compare_component};
pub use generate::{
    ArtifactDigest, CALCIT_BINDINGS_FILE, COMPONENT_FILE, CheckIssue, CheckIssueKind, CheckReport,
    ContractKind, GenerationBackend, INTERFACE_FILE, MANIFEST_FILE, Manifest, RUST_BINDINGS_FILE,
    TYPESCRIPT_BINDINGS_FILE, WIT_BINDINGS_FILE, check_contract_directory, check_directory,
    check_directory_with_backends, generate_contract_directory, generate_directory,
    generate_directory_with_backends,
};
pub use model::{
    ComponentDefinition, ComponentDirection, ComponentDocument, Declaration, Definition,
    DefinitionStatus, Document, EnumVariant, Envelope, FunctionSignature, InterfaceContract,
    Lowering, Parameter, StructField, Type,
};
pub use validate::{load_contract, load_document, validate_component_document, validate_document};
