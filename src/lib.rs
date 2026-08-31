//! Versioned Calcit FFI Interface IR validation and compatibility analysis.

mod compatibility;
mod model;
mod validate;

pub use compatibility::{Change, ChangeKind, CompatibilityReport, compare};
pub use model::{
    Declaration, Definition, DefinitionStatus, Document, EnumVariant, Envelope, FunctionSignature,
    Lowering, Parameter, StructField, Type,
};
pub use validate::{load_document, validate_document};
