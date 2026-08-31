use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{Declaration, Definition, DefinitionStatus, Document, Envelope, Ownership, Type};

pub fn load_document(path: impl AsRef<Path>) -> Result<Document, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let document = if value.get("command").is_some() {
        let envelope: Envelope = serde_json::from_value(value)
            .map_err(|error| format!("invalid ffi.export envelope: {error}"))?;
        if envelope.schema_version != 1 || envelope.command != "ffi.export" {
            return Err(format!(
                "expected ffi.export envelope schema v1, received command {:?} schema v{}",
                envelope.command, envelope.schema_version
            ));
        }
        envelope.data.interface
    } else {
        serde_json::from_value(value)
            .map_err(|error| format!("invalid Interface IR document: {error}"))?
    };
    validate_document(&document)?;
    Ok(document)
}

pub fn validate_document(document: &Document) -> Result<(), String> {
    if !matches!(document.version, 2 | 3) {
        return Err(format!(
            "unsupported Interface IR version {}; calcit-bindgen requires v2 or v3",
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
        validate_lifecycle_lowering(document.version, definition, &declarations)?;
    }
    Ok(())
}

fn validate_lifecycle_lowering(
    version: u32,
    definition: &Definition,
    declarations: &BTreeMap<&str, &Declaration>,
) -> Result<(), String> {
    let lowering = &definition.lowering;
    if version == 2 && (lowering.stream.is_some() || lowering.resource.is_some()) {
        return Err(format!(
            "{} uses lifecycle lowering, which requires Interface IR v3",
            definition.id
        ));
    }
    if lowering.stream.is_some() && lowering.resource.is_some() {
        return Err(format!(
            "{} cannot declare both stream and resource lifecycle lowering",
            definition.id
        ));
    }
    if (lowering.stream.is_some() || lowering.resource.is_some())
        && definition.status != DefinitionStatus::Unsupported
    {
        return Err(format!(
            "{} declares lifecycle lowering but must remain unsupported until a backend adapter and conformance vectors exist",
            definition.id
        ));
    }

    if let Some(stream) = &lowering.stream {
        require_lowering(
            definition,
            "stream",
            (
                Some("native"),
                Some("async"),
                Some("async-task-v1"),
                Some("async-stream"),
            ),
        )?;
        if stream.cancel != "cooperative" {
            return Err(format!(
                "{}.lowering.stream.cancel must be cooperative, got {:?}",
                definition.id, stream.cancel
            ));
        }
        if stream.task_result != Ownership::Own {
            return Err(format!(
                "{}.lowering.stream.task_result must be own",
                definition.id
            ));
        }
        let none = BTreeSet::new();
        validate_type(&stream.event_type, declarations, &none, false)?;
        if stream.callback_result != Type::Unit {
            return Err(format!(
                "{}.lowering.stream.callback_result must be Unit in lifecycle IR v3",
                definition.id
            ));
        }
        if let Some(signature) = definition.signature.as_ref()
            && !signature
                .parameters
                .iter()
                .any(|parameter| parameter.position == stream.callback_parameter)
        {
            return Err(format!(
                "{}.lowering.stream.callback_parameter {} does not reference a declared parameter position",
                definition.id, stream.callback_parameter
            ));
        }
    }

    if let Some(resource) = &lowering.resource {
        require_lowering(
            definition,
            "resource",
            (Some("native"), Some("sync"), Some("edn-buffer-v1"), None),
        )?;
        if resource.protocol != "opaque-resource-v1" {
            return Err(format!(
                "{}.lowering.resource.protocol must be opaque-resource-v1, got {:?}",
                definition.id, resource.protocol
            ));
        }
        let kind = lowering.kind.as_deref();
        match kind {
            Some("resource-constructor") => {
                if resource.result != Some(Ownership::Own) {
                    return Err(format!(
                        "{}.lowering.resource.result must be own for a resource constructor",
                        definition.id
                    ));
                }
                if !resource.parameters.is_empty() {
                    return Err(format!(
                        "{}.lowering.resource.parameters must be empty for a resource constructor",
                        definition.id
                    ));
                }
            }
            Some("resource-method") => {
                if resource.result.is_some() {
                    return Err(format!(
                        "{}.lowering.resource.result is only valid for a resource constructor",
                        definition.id
                    ));
                }
                if resource.parameters.is_empty() {
                    return Err(format!(
                        "{}.lowering.resource.parameters must declare at least one borrowed or cloned resource",
                        definition.id
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "{}.lowering.resource requires kind resource-constructor or resource-method",
                    definition.id
                ));
            }
        }

        let mut positions = BTreeSet::new();
        for parameter in &resource.parameters {
            if !positions.insert(parameter.position) {
                return Err(format!(
                    "{}.lowering.resource.parameters repeats position {}",
                    definition.id, parameter.position
                ));
            }
            if parameter.ownership == Ownership::Own {
                return Err(format!(
                    "{}.lowering.resource.parameters[{}].ownership cannot be own; input resources are borrow or clone until a consuming ABI is specified",
                    definition.id, parameter.position
                ));
            }
            if let Some(signature) = definition.signature.as_ref()
                && !signature
                    .parameters
                    .iter()
                    .any(|signature_parameter| signature_parameter.position == parameter.position)
            {
                return Err(format!(
                    "{}.lowering.resource.parameters[{}] does not reference a declared parameter position",
                    definition.id, parameter.position
                ));
            }
        }
    }
    Ok(())
}

fn require_lowering(
    definition: &Definition,
    lifecycle: &str,
    expected: (Option<&str>, Option<&str>, Option<&str>, Option<&str>),
) -> Result<(), String> {
    let lowering = &definition.lowering;
    let actual = (
        lowering.backend.as_deref(),
        lowering.invoke.as_deref(),
        lowering.transport.as_deref(),
        lowering.kind.as_deref(),
    );
    if expected.0.is_some_and(|value| actual.0 != Some(value))
        || expected.1.is_some_and(|value| actual.1 != Some(value))
        || expected.2.is_some_and(|value| actual.2 != Some(value))
        || expected.3.is_some_and(|value| actual.3 != Some(value))
    {
        return Err(format!(
            "{}.lowering.{lifecycle} conflicts with backend={:?}, invoke={:?}, transport={:?}, kind={:?}",
            definition.id, actual.0, actual.1, actual.2, actual.3
        ));
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
