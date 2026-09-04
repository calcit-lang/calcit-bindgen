use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{Declaration, DefinitionStatus, Document, Envelope, Type};

const FFI_INTERFACE_IR_V2_SCHEMA_ID: &str =
    "https://calcit-lang.org/schemas/ffi-interface-ir-v2.schema.json";

pub fn load_document(path: impl AsRef<Path>) -> Result<Document, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let document = if value.get("command").is_some() {
        let envelope: Envelope = serde_json::from_value(value)
            .map_err(|error| format!("invalid ffi.export envelope: {error}"))?;
        validate_export_envelope(&envelope)?;
        envelope.data.interface
    } else {
        serde_json::from_value(value)
            .map_err(|error| format!("invalid Interface IR document: {error}"))?
    };
    validate_document(&document)?;
    Ok(document)
}

fn validate_export_envelope(envelope: &Envelope) -> Result<(), String> {
    if envelope.schema_version != 1 || envelope.command != "ffi.export" {
        return Err(format!(
            "expected ffi.export envelope schema v1, received command {:?} schema v{}",
            envelope.command, envelope.schema_version
        ));
    }
    if envelope.interface_schema != FFI_INTERFACE_IR_V2_SCHEMA_ID {
        return Err(format!(
            "expected Interface IR schema {FFI_INTERFACE_IR_V2_SCHEMA_ID:?}, received {:?}",
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

pub fn validate_document(document: &Document) -> Result<(), String> {
    if document.version != 2 {
        return Err(format!(
            "unsupported Interface IR version {}; calcit-bindgen requires v2",
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
                    validate_type(&field.type_ir, &declarations, &parameters, true)?;
                }
            }
            Declaration::Enum { variants, .. } => {
                for variant in variants {
                    for item in &variant.payload {
                        validate_type(item, &declarations, &parameters, true)?;
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
                    validate_type(&parameter.type_ir, &declarations, &none, false)?;
                }
                validate_type(&signature.result, &declarations, &none, false)?;
            }
            (DefinitionStatus::Unsupported, Some(_)) | (DefinitionStatus::Unsupported, None) => {}
        }
    }
    Ok(())
}

fn validate_type(
    type_ir: &Type,
    declarations: &BTreeMap<&str, &Declaration>,
    parameters: &BTreeSet<String>,
    allow_parameter: bool,
) -> Result<(), String> {
    match type_ir {
        Type::Unit | Type::Bool | Type::Number | Type::String | Type::Buffer => Ok(()),
        Type::List { item } | Type::Option { item } => {
            validate_type(item, declarations, parameters, allow_parameter)
        }
        Type::Result { ok, error } => {
            validate_type(ok, declarations, parameters, allow_parameter)?;
            validate_type(error, declarations, parameters, allow_parameter)
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
                validate_type(argument, declarations, parameters, allow_parameter)?;
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
