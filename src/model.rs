use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub schema_version: u32,
    pub command: String,
    pub data: EnvelopeData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeData {
    pub interface: Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub package: String,
    pub package_version: String,
    pub declarations: Vec<Declaration>,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Declaration {
    Struct {
        id: String,
        namespace: String,
        name: String,
        type_parameters: Vec<String>,
        fields: Vec<StructField>,
    },
    Enum {
        id: String,
        namespace: String,
        name: String,
        type_parameters: Vec<String>,
        variants: Vec<EnumVariant>,
    },
}

impl Declaration {
    pub fn id(&self) -> &str {
        match self {
            Self::Struct { id, .. } | Self::Enum { id, .. } => id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Struct { .. } => "struct",
            Self::Enum { .. } => "enum",
        }
    }

    pub fn type_parameters(&self) -> &[String] {
        match self {
            Self::Struct {
                type_parameters, ..
            }
            | Self::Enum {
                type_parameters, ..
            } => type_parameters,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    #[serde(rename = "type")]
    pub type_ir: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub doc: String,
    pub logical_schema: String,
    pub signature: Option<FunctionSignature>,
    pub lowering: Lowering,
    pub status: DefinitionStatus,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionStatus {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub parameters: Vec<Parameter>,
    pub result: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub position: usize,
    #[serde(rename = "type")]
    pub type_ir: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Type {
    Unit,
    Bool,
    Number,
    String,
    Buffer,
    List { item: Box<Type> },
    Option { item: Box<Type> },
    Result { ok: Box<Type>, error: Box<Type> },
    Struct { id: String, arguments: Vec<Type> },
    Enum { id: String, arguments: Vec<Type> },
    TypeParameter { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lowering {
    pub backend: Option<String>,
    pub target: Option<String>,
    pub kind: Option<String>,
    pub symbol: Option<String>,
    pub invoke: Option<String>,
    pub transport: Option<String>,
    /// Structured stream/task lifecycle metadata introduced by Interface IR v3.
    /// It is absent from v2 documents and deliberately does not imply generated
    /// async adapters yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamLowering>,
    /// Structured opaque-resource ownership metadata introduced by Interface IR
    /// v3. It is absent from v2 documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceLowering>,
    pub raw: String,
}

/// Ownership at a native resource boundary.
///
/// `Own` transfers a newly-created resource lease to Calcit; `Borrow` passes an
/// existing lease only for the duration of a call; `Clone` denotes an explicit
/// additional lease when a future protocol supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ownership {
    Own,
    Borrow,
    Clone,
}

/// Lifecycle metadata for a native async stream/task boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamLowering {
    pub callback_parameter: usize,
    #[serde(rename = "event")]
    pub event_type: Type,
    pub callback_result: Type,
    pub cancel: String,
    pub task_result: Ownership,
}

/// Borrow/clone contract for one native resource parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceParameterOwnership {
    pub position: usize,
    pub ownership: Ownership,
}

/// Lifecycle metadata for an opaque native resource boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLowering {
    pub protocol: String,
    #[serde(default)]
    pub result: Option<Ownership>,
    #[serde(default)]
    pub parameters: Vec<ResourceParameterOwnership>,
}
