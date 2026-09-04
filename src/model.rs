use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub schema_version: u32,
    pub interface_schema: String,
    pub command: String,
    pub revision: String,
    pub data: EnvelopeData,
    pub diagnostics: Vec<InterfaceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeData {
    pub filters: ExportFilters,
    pub interface: Document,
    pub summary: ExportSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportFilters {
    pub namespace: Option<String>,
    pub include_dependencies: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSummary {
    pub definitions: usize,
    pub supported: usize,
    pub unsupported: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceDiagnostic {
    pub code: String,
    pub phase: String,
    pub severity: String,
    pub definition: String,
    pub path: String,
    pub message: String,
    pub suggestion: String,
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
    pub raw: String,
}
