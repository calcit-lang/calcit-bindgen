use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    ComponentDefinition, ComponentDirection, ComponentDocument, Declaration, DefinitionStatus,
    Document, InterfaceContract, Type,
};

const FFI_INTERFACE_IR_V2_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/ffi-interface-ir-v2.schema.json";
const FFI_INTERFACE_IR_V3_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/ffi-interface-ir-v3.schema.json";
const COMPONENT_INTERFACE_IR_V2_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/component-interface-ir-v2.schema.json";
const COMPONENT_INTERFACE_IR_V3_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/component-interface-ir-v3.schema.json";
const COMPONENT_INTERFACE_IR_V4_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/component-interface-ir-v4.schema.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportEnvelope {
    schema_version: u32,
    interface_schema: String,
    command: String,
    revision: String,
    data: ExportEnvelopeData,
    diagnostics: Vec<ExportDiagnostic>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentExportEnvelope {
    schema_version: u32,
    interface_schema: String,
    command: String,
    revision: String,
    data: ComponentExportEnvelopeData,
    diagnostics: Vec<ExportDiagnostic>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentExportEnvelopeData {
    filters: ComponentExportFilters,
    interface: ComponentDocument,
    summary: ExportSummary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentExportFilters {
    boundary: String,
    #[allow(dead_code)]
    namespace: Option<String>,
    include_dependencies: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportEnvelopeData {
    filters: ExportFilters,
    interface: Document,
    summary: ExportSummary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportFilters {
    #[allow(dead_code)]
    namespace: Option<String>,
    include_dependencies: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportSummary {
    definitions: usize,
    supported: usize,
    unsupported: usize,
    diagnostics: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportDiagnostic {
    code: String,
    phase: String,
    severity: String,
    definition: String,
    path: String,
    message: String,
    suggestion: String,
}

pub fn load_document(path: impl AsRef<Path>) -> Result<Document, String> {
    match load_contract(path)? {
        InterfaceContract::Native(document) => Ok(document),
        InterfaceContract::Component(_) => {
            Err("expected native Interface IR, received Component Interface IR".to_owned())
        }
    }
}

pub fn load_contract(path: impl AsRef<Path>) -> Result<InterfaceContract, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value = parse_structured_source(&source, path)?;
    if value.get("command").is_some() {
        let schema = value
            .get("interface_schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "ffi.export envelope must declare interface_schema".to_owned())?;
        if matches!(
            schema,
            COMPONENT_INTERFACE_IR_V2_SCHEMA_ID
                | COMPONENT_INTERFACE_IR_V3_SCHEMA_ID
                | COMPONENT_INTERFACE_IR_V4_SCHEMA_ID
        ) {
            let envelope: ComponentExportEnvelope = serde_json::from_value(value)
                .map_err(|error| format!("invalid Component ffi.export envelope: {error}"))?;
            validate_component_export_envelope(&envelope)?;
            validate_component_document(&envelope.data.interface)?;
            return Ok(InterfaceContract::Component(envelope.data.interface));
        }
        let envelope: ExportEnvelope = serde_json::from_value(value)
            .map_err(|error| format!("invalid ffi.export envelope: {error}"))?;
        validate_export_envelope(&envelope)?;
        validate_document(&envelope.data.interface)?;
        Ok(InterfaceContract::Native(envelope.data.interface))
    } else {
        let document: Document = serde_json::from_value(value)
            .map_err(|error| format!("invalid Interface IR document: {error}"))?;
        validate_document(&document)?;
        Ok(InterfaceContract::Native(document))
    }
}

fn parse_structured_source(source: &str, path: &Path) -> Result<serde_json::Value, String> {
    if source.trim_start().starts_with('{') && !source.trim_start().starts_with("{}") {
        serde_json::from_str(source)
            .map_err(|error| format!("failed to parse JSON {}: {error}", path.display()))
    } else {
        let value = cirru_edn::parse(source)
            .map_err(|error| format!("failed to parse Cirru EDN {}: {error}", path.display()))?;
        edn_to_json(&value, "$")
    }
}

fn edn_to_json(value: &cirru_edn::Edn, path: &str) -> Result<serde_json::Value, String> {
    use cirru_edn::Edn;
    match value {
        Edn::Nil => Ok(serde_json::Value::Null),
        Edn::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        Edn::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= 0.0
                && *value <= 9_007_199_254_740_992.0 =>
        {
            let integer = *value as u64;
            Ok(serde_json::Value::Number(integer.into()))
        }
        Edn::Number(value) if value.is_finite() && value.fract() == 0.0 => Err(format!(
            "{path}: integer is outside the non-negative lossless JSON range"
        )),
        Edn::Number(value) if value.is_finite() => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| format!("{path}: number is not representable in JSON")),
        Edn::Str(value) => Ok(serde_json::Value::String(value.to_string())),
        Edn::Tag(value) => Ok(serde_json::Value::String(value.ref_str().to_owned())),
        Edn::List(values) => values
            .0
            .iter()
            .enumerate()
            .map(|(index, value)| edn_to_json(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        Edn::Map(values) => {
            let mut output = serde_json::Map::new();
            for (key, value) in &values.0 {
                let key = match key {
                    Edn::Tag(value) => value.ref_str().replace('-', "_"),
                    other => {
                        return Err(format!("{path}: map key {other} must be a tag"));
                    }
                };
                if output.contains_key(&key) {
                    return Err(format!("{path}: duplicate normalized map key {key:?}"));
                }
                output.insert(key.clone(), edn_to_json(value, &format!("{path}.{key}"))?);
            }
            Ok(serde_json::Value::Object(output))
        }
        other => Err(format!(
            "{path}: unsupported Cirru EDN value in an interface contract: {other}"
        )),
    }
}

fn validate_export_envelope(envelope: &ExportEnvelope) -> Result<(), String> {
    if envelope.schema_version != 1 || envelope.command != "ffi.export" {
        return Err(format!(
            "expected ffi.export envelope schema v1, received command {:?} schema v{}",
            envelope.command, envelope.schema_version
        ));
    }
    let expected_schema = match envelope.data.interface.version {
        2 => FFI_INTERFACE_IR_V2_SCHEMA_ID,
        3 => FFI_INTERFACE_IR_V3_SCHEMA_ID,
        version => {
            return Err(format!(
                "unsupported Interface IR version {version}; calcit-bindgen supports v2 and v3"
            ));
        }
    };
    if envelope.interface_schema != expected_schema {
        return Err(format!(
            "expected Interface IR schema {expected_schema:?}, received {:?}",
            envelope.interface_schema
        ));
    }
    if envelope.data.filters.include_dependencies {
        return Err("ffi.export v1 must not include dependency definitions".to_owned());
    }

    let supported = envelope
        .data
        .interface
        .definitions
        .iter()
        .filter(|definition| definition.status == DefinitionStatus::Supported)
        .count();
    let definitions = envelope.data.interface.definitions.len();
    let unsupported = definitions - supported;
    let summary = &envelope.data.summary;
    if summary.definitions != definitions
        || summary.supported != supported
        || summary.unsupported != unsupported
        || summary.diagnostics != envelope.diagnostics.len()
    {
        return Err(format!(
            "ffi.export summary does not match the embedded interface and diagnostics: expected definitions={definitions}, supported={supported}, unsupported={unsupported}, diagnostics={}",
            envelope.diagnostics.len()
        ));
    }

    let mut observed_diagnostic_codes = BTreeSet::new();
    for diagnostic in &envelope.diagnostics {
        let Some(definition) = envelope
            .data
            .interface
            .definitions
            .iter()
            .find(|definition| definition.id == diagnostic.definition)
        else {
            return Err(format!(
                "ffi.export diagnostic {} references unknown definition {}",
                diagnostic.code, diagnostic.definition
            ));
        };
        if !definition
            .diagnostic_codes
            .iter()
            .any(|code| code == &diagnostic.code)
        {
            return Err(format!(
                "ffi.export diagnostic {} is not listed by definition {}",
                diagnostic.code, diagnostic.definition
            ));
        }
        observed_diagnostic_codes
            .insert((diagnostic.definition.as_str(), diagnostic.code.as_str()));
    }
    for definition in &envelope.data.interface.definitions {
        for code in &definition.diagnostic_codes {
            if !observed_diagnostic_codes.contains(&(definition.id.as_str(), code.as_str())) {
                return Err(format!(
                    "ffi.export definition {} lists diagnostic code {} without a structured diagnostic",
                    definition.id, code
                ));
            }
        }
    }

    let revision_payload =
        serde_json::to_vec(&(&envelope.data.interface, &envelope.diagnostics))
            .map_err(|error| format!("failed to encode ffi.export revision input: {error}"))?;
    let expected_revision = format!("md5:{:x}", md5::compute(revision_payload));
    if envelope.revision != expected_revision {
        return Err(format!(
            "ffi.export revision mismatch: expected {expected_revision}, received {}",
            envelope.revision
        ));
    }

    Ok(())
}

fn validate_component_export_envelope(envelope: &ComponentExportEnvelope) -> Result<(), String> {
    if envelope.schema_version != 1 || envelope.command != "ffi.export" {
        return Err(format!(
            "expected Component ffi.export envelope schema v1, received command {:?} schema v{}",
            envelope.command, envelope.schema_version
        ));
    }
    let expected_schema = match envelope.data.interface.version {
        2 => COMPONENT_INTERFACE_IR_V2_SCHEMA_ID,
        3 => COMPONENT_INTERFACE_IR_V3_SCHEMA_ID,
        4 => COMPONENT_INTERFACE_IR_V4_SCHEMA_ID,
        version => {
            return Err(format!(
                "unsupported Component Interface IR version {version}; calcit-bindgen supports v2, v3, and v4"
            ));
        }
    };
    if envelope.interface_schema != expected_schema {
        return Err(format!(
            "expected Component Interface IR schema {expected_schema:?}, received {:?}",
            envelope.interface_schema
        ));
    }
    if envelope.data.filters.boundary != "component" {
        return Err(format!(
            "Component ffi.export boundary must be \"component\", received {:?}",
            envelope.data.filters.boundary
        ));
    }
    if envelope.data.filters.include_dependencies {
        return Err("Component ffi.export v1 must not include dependency definitions".to_owned());
    }

    validate_export_summary(
        &envelope.data.interface.definitions,
        &envelope.data.summary,
        &envelope.diagnostics,
    )?;

    let revision_payload = serde_json::to_vec(&(&envelope.data.interface, &envelope.diagnostics))
        .map_err(|error| {
        format!("failed to encode Component ffi.export revision input: {error}")
    })?;
    let expected_revision = format!("md5:{:x}", md5::compute(revision_payload));
    if envelope.revision != expected_revision {
        return Err(format!(
            "Component ffi.export revision mismatch: expected {expected_revision}, received {}",
            envelope.revision
        ));
    }
    Ok(())
}

fn validate_export_summary(
    definitions: &[ComponentDefinition],
    summary: &ExportSummary,
    diagnostics: &[ExportDiagnostic],
) -> Result<(), String> {
    let supported = definitions
        .iter()
        .filter(|definition| definition.status == DefinitionStatus::Supported)
        .count();
    let definition_count = definitions.len();
    let unsupported = definition_count - supported;
    if summary.definitions != definition_count
        || summary.supported != supported
        || summary.unsupported != unsupported
        || summary.diagnostics != diagnostics.len()
    {
        return Err(format!(
            "Component ffi.export summary does not match the embedded interface and diagnostics: expected definitions={definition_count}, supported={supported}, unsupported={unsupported}, diagnostics={}",
            diagnostics.len()
        ));
    }

    let mut observed_diagnostic_codes = BTreeSet::new();
    for diagnostic in diagnostics {
        let Some(definition) = definitions
            .iter()
            .find(|definition| definition.id == diagnostic.definition)
        else {
            return Err(format!(
                "Component ffi.export diagnostic {} references unknown definition {}",
                diagnostic.code, diagnostic.definition
            ));
        };
        if !definition
            .diagnostic_codes
            .iter()
            .any(|code| code == &diagnostic.code)
        {
            return Err(format!(
                "Component ffi.export diagnostic {} is not listed by definition {}",
                diagnostic.code, diagnostic.definition
            ));
        }
        observed_diagnostic_codes
            .insert((diagnostic.definition.as_str(), diagnostic.code.as_str()));
    }
    for definition in definitions {
        for code in &definition.diagnostic_codes {
            if !observed_diagnostic_codes.contains(&(definition.id.as_str(), code.as_str())) {
                return Err(format!(
                    "Component ffi.export definition {} lists diagnostic code {} without a structured diagnostic",
                    definition.id, code
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_document(document: &Document) -> Result<(), String> {
    if !matches!(document.version, 2 | 3) {
        return Err(format!(
            "unsupported Interface IR version {}; calcit-bindgen supports v2 and v3",
            document.version
        ));
    }
    if document.package.is_empty() || document.package_version.is_empty() {
        return Err("Interface IR package and package_version must not be empty".to_owned());
    }

    let declarations = document
        .declarations
        .iter()
        .map(|declaration| (declaration.id(), declaration))
        .collect::<BTreeMap<_, _>>();
    if declarations.len() != document.declarations.len() {
        return Err("Interface IR contains duplicate declaration IDs".to_owned());
    }
    let definition_ids = document
        .definitions
        .iter()
        .map(|definition| definition.id.as_str())
        .collect::<BTreeSet<_>>();
    if definition_ids.len() != document.definitions.len() {
        return Err("Interface IR contains duplicate definition IDs".to_owned());
    }

    let allow_numeric_refinements = document.version >= 3;
    for declaration in &document.declarations {
        let parameters = declaration
            .type_parameters()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if parameters.len() != declaration.type_parameters().len() {
            return Err(format!(
                "{} contains duplicate type parameters",
                declaration.id()
            ));
        }
        match declaration {
            Declaration::Struct { fields, .. } => {
                for field in fields {
                    validate_type(
                        &field.type_ir,
                        &declarations,
                        &parameters,
                        true,
                        allow_numeric_refinements,
                    )?;
                }
            }
            Declaration::Enum { variants, .. } => {
                for variant in variants {
                    for item in &variant.payload {
                        validate_type(
                            item,
                            &declarations,
                            &parameters,
                            true,
                            allow_numeric_refinements,
                        )?;
                    }
                }
            }
        }
    }

    for definition in &document.definitions {
        match (definition.status, definition.signature.as_ref()) {
            (DefinitionStatus::Supported, None) => {
                return Err(format!(
                    "supported definition {} has no signature",
                    definition.id
                ));
            }
            (DefinitionStatus::Supported, Some(signature)) => {
                if !definition.diagnostic_codes.is_empty() {
                    return Err(format!(
                        "supported definition {} still has diagnostics",
                        definition.id
                    ));
                }
                let none = BTreeSet::new();
                for (index, parameter) in signature.parameters.iter().enumerate() {
                    if parameter.position != index {
                        return Err(format!(
                            "{} has non-contiguous parameter positions",
                            definition.id
                        ));
                    }
                    validate_type(
                        &parameter.type_ir,
                        &declarations,
                        &none,
                        false,
                        allow_numeric_refinements,
                    )?;
                }
                validate_type(
                    &signature.result,
                    &declarations,
                    &none,
                    false,
                    allow_numeric_refinements,
                )?;
            }
            (DefinitionStatus::Unsupported, Some(_)) | (DefinitionStatus::Unsupported, None) => {}
        }
    }
    Ok(())
}

pub fn validate_component_document(document: &ComponentDocument) -> Result<(), String> {
    if !matches!(document.version, 2..=4) {
        return Err(format!(
            "unsupported Component Interface IR version {}; calcit-bindgen supports v2, v3, and v4",
            document.version
        ));
    }
    if document.package.is_empty() || document.package_version.is_empty() {
        return Err(
            "Component Interface IR package and package_version must not be empty".to_owned(),
        );
    }

    let declarations = document
        .declarations
        .iter()
        .map(|declaration| (declaration.id(), declaration))
        .collect::<BTreeMap<_, _>>();
    if declarations.len() != document.declarations.len() {
        return Err("Component Interface IR contains duplicate declaration IDs".to_owned());
    }
    for declaration in &document.declarations {
        let parameters = declaration
            .type_parameters()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if parameters.len() != declaration.type_parameters().len() {
            return Err(format!(
                "{} contains duplicate type parameters",
                declaration.id()
            ));
        }
        match declaration {
            Declaration::Struct { fields, .. } => {
                for field in fields {
                    validate_type(&field.type_ir, &declarations, &parameters, true, true)?;
                }
            }
            Declaration::Enum { variants, .. } => {
                for variant in variants {
                    for item in &variant.payload {
                        validate_type(item, &declarations, &parameters, true, true)?;
                    }
                }
            }
        }
    }

    let mut ids = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    for definition in &document.definitions {
        match (document.version, definition.invocation) {
            (2, None) | (3 | 4, Some(_)) => {}
            (2, Some(_)) => {
                return Err(format!(
                    "Component Interface IR v2 definition {} must not declare invocation",
                    definition.id
                ));
            }
            (3 | 4, None) => {
                return Err(format!(
                    "Component Interface IR v{} definition {} must declare invocation",
                    document.version, definition.id
                ));
            }
            _ => unreachable!(),
        }
        if !ids.insert(definition.id.as_str()) {
            return Err(format!(
                "Component Interface IR contains duplicate definition ID {}",
                definition.id
            ));
        }
        if definition.symbol.is_empty() {
            return Err(format!("{} has an empty Component symbol", definition.id));
        }
        match definition.direction {
            ComponentDirection::Import => {
                if definition.module.as_deref().is_none_or(str::is_empty) {
                    return Err(format!(
                        "Component import {} requires a non-empty module",
                        definition.id
                    ));
                }
            }
            ComponentDirection::Export if definition.module.is_some() => {
                return Err(format!(
                    "Component export {} must not declare a module",
                    definition.id
                ));
            }
            ComponentDirection::Export => {}
        }
        let identity = (
            definition.direction,
            definition.module.as_deref().unwrap_or(""),
            definition.symbol.as_str(),
        );
        if !bindings.insert(identity) {
            return Err(format!(
                "Component Interface IR contains duplicate {:?} binding {}/{}",
                definition.direction,
                definition.module.as_deref().unwrap_or("<world>"),
                definition.symbol
            ));
        }

        match (definition.status, definition.signature.as_ref()) {
            (DefinitionStatus::Supported, None) => {
                return Err(format!(
                    "supported Component definition {} has no signature",
                    definition.id
                ));
            }
            (DefinitionStatus::Supported, Some(signature)) => {
                if !definition.diagnostic_codes.is_empty() {
                    return Err(format!(
                        "supported Component definition {} still has diagnostics",
                        definition.id
                    ));
                }
                let none = BTreeSet::new();
                for (index, parameter) in signature.parameters.iter().enumerate() {
                    if parameter.position != index {
                        return Err(format!(
                            "{} has non-contiguous parameter positions",
                            definition.id
                        ));
                    }
                    validate_type(&parameter.type_ir, &declarations, &none, false, true)?;
                    if type_contains_readable_byte_stream(&parameter.type_ir) {
                        if !matches!(parameter.type_ir, Type::ReadableByteStream) {
                            return Err(format!(
                                "{}.signature.parameters[{}]: ReadableByteStream must be a direct parameter",
                                definition.id, parameter.position
                            ));
                        }
                        if document.version < 4
                            || definition.direction != ComponentDirection::Export
                            || definition.invocation != Some(crate::ComponentInvocation::Async)
                        {
                            return Err(format!(
                                "{}.signature.parameters[{}]: ReadableByteStream requires a Component Interface IR v4 async export",
                                definition.id, parameter.position
                            ));
                        }
                    }
                }
                validate_type(&signature.result, &declarations, &none, false, true)?;
                if type_contains_readable_byte_stream(&signature.result) {
                    return Err(format!(
                        "{}.signature.result: ReadableByteStream cannot escape the scoped consumer",
                        definition.id
                    ));
                }
            }
            (DefinitionStatus::Unsupported, Some(_)) | (DefinitionStatus::Unsupported, None) => {}
        }
    }
    Ok(())
}

fn type_contains_readable_byte_stream(type_ir: &Type) -> bool {
    match type_ir {
        Type::ReadableByteStream => true,
        Type::List { item } | Type::Option { item } => type_contains_readable_byte_stream(item),
        Type::Result { ok, error } => {
            type_contains_readable_byte_stream(ok) || type_contains_readable_byte_stream(error)
        }
        Type::Struct { arguments, .. } | Type::Enum { arguments, .. } => {
            arguments.iter().any(type_contains_readable_byte_stream)
        }
        Type::Unit
        | Type::Bool
        | Type::Number
        | Type::Int8
        | Type::Uint8
        | Type::Int16
        | Type::Uint16
        | Type::Int32
        | Type::Uint32
        | Type::Int64
        | Type::Uint64
        | Type::Float32
        | Type::Float64
        | Type::String
        | Type::Buffer
        | Type::TypeParameter { .. } => false,
    }
}

fn validate_type(
    type_ir: &Type,
    declarations: &BTreeMap<&str, &Declaration>,
    parameters: &BTreeSet<String>,
    allow_parameter: bool,
    allow_numeric_refinements: bool,
) -> Result<(), String> {
    match type_ir {
        Type::Unit
        | Type::Bool
        | Type::Number
        | Type::String
        | Type::Buffer
        | Type::ReadableByteStream => Ok(()),
        Type::Int8
        | Type::Uint8
        | Type::Int16
        | Type::Uint16
        | Type::Int32
        | Type::Uint32
        | Type::Int64
        | Type::Uint64
        | Type::Float32
        | Type::Float64
            if allow_numeric_refinements =>
        {
            Ok(())
        }
        Type::Int8
        | Type::Uint8
        | Type::Int16
        | Type::Uint16
        | Type::Int32
        | Type::Uint32
        | Type::Int64
        | Type::Uint64
        | Type::Float32
        | Type::Float64 => Err(
            "explicit numeric refinements require Interface IR v3 or Component Interface IR v2+"
                .to_owned(),
        ),
        Type::List { item } | Type::Option { item } => validate_type(
            item,
            declarations,
            parameters,
            allow_parameter,
            allow_numeric_refinements,
        ),
        Type::Result { ok, error } => {
            validate_type(
                ok,
                declarations,
                parameters,
                allow_parameter,
                allow_numeric_refinements,
            )?;
            validate_type(
                error,
                declarations,
                parameters,
                allow_parameter,
                allow_numeric_refinements,
            )
        }
        Type::Struct { id, arguments } | Type::Enum { id, arguments } => {
            let declaration = declarations
                .get(id.as_str())
                .ok_or_else(|| format!("type references missing declaration {id}"))?;
            let expected_kind = match type_ir {
                Type::Struct { .. } => "struct",
                Type::Enum { .. } => "enum",
                _ => unreachable!(),
            };
            if declaration.kind() != expected_kind {
                return Err(format!(
                    "type references {id} as {expected_kind}, but it is {}",
                    declaration.kind()
                ));
            }
            if arguments.len() != declaration.type_parameters().len() {
                return Err(format!(
                    "type applies {} argument(s) to {id}, which declares {}",
                    arguments.len(),
                    declaration.type_parameters().len()
                ));
            }
            for argument in arguments {
                validate_type(
                    argument,
                    declarations,
                    parameters,
                    allow_parameter,
                    allow_numeric_refinements,
                )?;
            }
            Ok(())
        }
        Type::TypeParameter { name } if allow_parameter && parameters.contains(name) => Ok(()),
        Type::TypeParameter { name } if allow_parameter => Err(format!(
            "declaration references undeclared type parameter {name}"
        )),
        Type::TypeParameter { name } => Err(format!(
            "callable signature is not monomorphic: exposed type parameter {name}"
        )),
    }
}
