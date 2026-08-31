use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt::Debug;

use serde::Serialize;

use crate::{
    Declaration, Definition, DefinitionStatus, Document, EnumVariant, FunctionSignature, Lowering,
    StructField,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Additive,
    Breaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub changes: Vec<Change>,
}

/// Compare the public, generated Interface IR contract.
///
/// Package versions, documentation, display-only schemas, raw lowering text,
/// and diagnostics are intentionally excluded. They do not change generated
/// declarations, callable signatures, or host lowering behavior.
pub fn compare(old: &Document, new: &Document) -> CompatibilityReport {
    let mut changes = Vec::new();
    if old.package != new.package {
        changes.push(breaking(
            "package",
            format!("changed from {:?} to {:?}", old.package, new.package),
        ));
    }

    let old_declarations = index_unique(
        "old",
        "declarations",
        &old.declarations,
        |item| item.id(),
        &mut changes,
    );
    let new_declarations = index_unique(
        "new",
        "declarations",
        &new.declarations,
        |item| item.id(),
        &mut changes,
    );
    for (id, old_declaration) in &old_declarations {
        let path = format!("declarations.{id}");
        match new_declarations.get(id) {
            None => changes.push(breaking(path, "removed")),
            Some(new_declaration) => {
                compare_declaration(&path, old_declaration, new_declaration, &mut changes)
            }
        }
    }
    for id in new_declarations.keys() {
        if !old_declarations.contains_key(id) {
            changes.push(additive(format!("declarations.{id}"), "added"));
        }
    }

    let old_definitions = index_unique(
        "old",
        "definitions",
        &old.definitions,
        |item| item.id.as_str(),
        &mut changes,
    );
    let new_definitions = index_unique(
        "new",
        "definitions",
        &new.definitions,
        |item| item.id.as_str(),
        &mut changes,
    );
    for (id, old_definition) in &old_definitions {
        let path = format!("definitions.{id}");
        match new_definitions.get(id) {
            None => changes.push(breaking(path, "removed")),
            Some(new_definition) => {
                compare_definition(&path, old_definition, new_definition, &mut changes)
            }
        }
    }
    for id in new_definitions.keys() {
        if !old_definitions.contains_key(id) {
            changes.push(additive(format!("definitions.{id}"), "added"));
        }
    }

    CompatibilityReport {
        compatible: !changes
            .iter()
            .any(|change| change.kind == ChangeKind::Breaking),
        changes,
    }
}

/// Index values by ID while reporting invalid public `Document` vectors.
///
/// File loading validates uniqueness before comparison, but library callers may
/// construct `Document` values directly. Keeping the first item makes the
/// remaining field comparison deterministic while the duplicate itself makes
/// the report incompatible.
fn index_unique<'a, T>(
    side: &str,
    collection: &str,
    items: &'a [T],
    id: impl Fn(&'a T) -> &'a str,
    changes: &mut Vec<Change>,
) -> BTreeMap<&'a str, &'a T> {
    let mut indexed = BTreeMap::new();
    for item in items {
        let item_id = id(item);
        match indexed.entry(item_id) {
            Entry::Vacant(entry) => {
                entry.insert(item);
            }
            Entry::Occupied(_) => changes.push(breaking(
                format!("{side}.{collection}.{item_id}"),
                "duplicate ID",
            )),
        }
    }
    indexed
}

/// Compare one nominal declaration after both documents are indexed.
fn compare_declaration(
    path: &str,
    old: &Declaration,
    new: &Declaration,
    changes: &mut Vec<Change>,
) {
    match (old, new) {
        (
            Declaration::Struct {
                namespace: old_namespace,
                name: old_name,
                type_parameters: old_parameters,
                fields: old_fields,
                ..
            },
            Declaration::Struct {
                namespace: new_namespace,
                name: new_name,
                type_parameters: new_parameters,
                fields: new_fields,
                ..
            },
        ) => {
            compare_value(
                format!("{path}.namespace"),
                old_namespace,
                new_namespace,
                changes,
            );
            compare_value(format!("{path}.name"), old_name, new_name, changes);
            compare_value(
                format!("{path}.type-parameters"),
                old_parameters,
                new_parameters,
                changes,
            );
            compare_struct_fields(path, old_fields, new_fields, changes);
        }
        (
            Declaration::Enum {
                namespace: old_namespace,
                name: old_name,
                type_parameters: old_parameters,
                variants: old_variants,
                ..
            },
            Declaration::Enum {
                namespace: new_namespace,
                name: new_name,
                type_parameters: new_parameters,
                variants: new_variants,
                ..
            },
        ) => {
            compare_value(
                format!("{path}.namespace"),
                old_namespace,
                new_namespace,
                changes,
            );
            compare_value(format!("{path}.name"), old_name, new_name, changes);
            compare_value(
                format!("{path}.type-parameters"),
                old_parameters,
                new_parameters,
                changes,
            );
            compare_enum_variants(path, old_variants, new_variants, changes);
        }
        _ => changes.push(breaking(
            format!("{path}.kind"),
            format!("changed from {} to {}", old.kind(), new.kind()),
        )),
    }
}

/// Compare ordered struct fields because field order participates in layout.
fn compare_struct_fields(
    path: &str,
    old: &[StructField],
    new: &[StructField],
    changes: &mut Vec<Change>,
) {
    compare_length(
        format!("{path}.fields.length"),
        old.len(),
        new.len(),
        changes,
    );
    for (index, (old_field, new_field)) in old.iter().zip(new).enumerate() {
        compare_value(
            format!("{path}.fields[{index}].name"),
            &old_field.name,
            &new_field.name,
            changes,
        );
        compare_value(
            format!("{path}.fields[{index}].type"),
            &old_field.type_ir,
            &new_field.type_ir,
            changes,
        );
    }
}

/// Compare ordered enum variants and each variant payload.
fn compare_enum_variants(
    path: &str,
    old: &[EnumVariant],
    new: &[EnumVariant],
    changes: &mut Vec<Change>,
) {
    compare_length(
        format!("{path}.variants.length"),
        old.len(),
        new.len(),
        changes,
    );
    for (index, (old_variant, new_variant)) in old.iter().zip(new).enumerate() {
        compare_value(
            format!("{path}.variants[{index}].name"),
            &old_variant.name,
            &new_variant.name,
            changes,
        );
        compare_value(
            format!("{path}.variants[{index}].payload"),
            &old_variant.payload,
            &new_variant.payload,
            changes,
        );
    }
}

/// Compare one callable definition according to its support status.
fn compare_definition(path: &str, old: &Definition, new: &Definition, changes: &mut Vec<Change>) {
    match (old.status, new.status) {
        (DefinitionStatus::Unsupported, DefinitionStatus::Unsupported) => {
            // Lifecycle metadata is already a public contract even before a
            // backend can generate an adapter for it. Keep ordinary unsupported
            // diagnostics out of compatibility, but surface task/resource
            // ownership changes deterministically.
            compare_lifecycle_lowering(path, &old.lowering, &new.lowering, changes);
        }
        (DefinitionStatus::Unsupported, DefinitionStatus::Supported) => {
            changes.push(additive(
                format!("{path}.status"),
                "changed from unsupported to supported",
            ));
        }
        (DefinitionStatus::Supported, DefinitionStatus::Unsupported) => {
            changes.push(breaking(
                format!("{path}.status"),
                "changed from supported to unsupported",
            ));
        }
        (DefinitionStatus::Supported, DefinitionStatus::Supported) => {
            compare_value(
                format!("{path}.namespace"),
                &old.namespace,
                &new.namespace,
                changes,
            );
            compare_value(format!("{path}.name"), &old.name, &new.name, changes);
            compare_signature(
                path,
                old.signature.as_ref(),
                new.signature.as_ref(),
                changes,
            );
            compare_lowering(path, &old.lowering, &new.lowering, changes);
        }
    }
}

/// Compare the monomorphic callable signature consumed by generated adapters.
fn compare_signature(
    path: &str,
    old: Option<&FunctionSignature>,
    new: Option<&FunctionSignature>,
    changes: &mut Vec<Change>,
) {
    match (old, new) {
        (Some(old), Some(new)) => {
            compare_length(
                format!("{path}.signature.parameters.length"),
                old.parameters.len(),
                new.parameters.len(),
                changes,
            );
            for (index, (old_parameter, new_parameter)) in
                old.parameters.iter().zip(&new.parameters).enumerate()
            {
                compare_value(
                    format!("{path}.signature.parameters[{index}].position"),
                    &old_parameter.position,
                    &new_parameter.position,
                    changes,
                );
                compare_value(
                    format!("{path}.signature.parameters[{index}].type"),
                    &old_parameter.type_ir,
                    &new_parameter.type_ir,
                    changes,
                );
            }
            compare_value(
                format!("{path}.signature.result"),
                &old.result,
                &new.result,
                changes,
            );
        }
        (None, None) => {}
        _ => changes.push(breaking(
            format!("{path}.signature"),
            "signature presence changed",
        )),
    }
}

/// Compare the structured lowering fields used at the host call boundary.
fn compare_lowering(path: &str, old: &Lowering, new: &Lowering, changes: &mut Vec<Change>) {
    compare_value(
        format!("{path}.lowering.backend"),
        &old.backend,
        &new.backend,
        changes,
    );
    compare_value(
        format!("{path}.lowering.target"),
        &old.target,
        &new.target,
        changes,
    );
    compare_value(
        format!("{path}.lowering.kind"),
        &old.kind,
        &new.kind,
        changes,
    );
    compare_value(
        format!("{path}.lowering.symbol"),
        &old.symbol,
        &new.symbol,
        changes,
    );
    compare_value(
        format!("{path}.lowering.invoke"),
        &old.invoke,
        &new.invoke,
        changes,
    );
    compare_value(
        format!("{path}.lowering.transport"),
        &old.transport,
        &new.transport,
        changes,
    );
    compare_lifecycle_lowering(path, old, new, changes);
}

fn compare_lifecycle_lowering(
    path: &str,
    old: &Lowering,
    new: &Lowering,
    changes: &mut Vec<Change>,
) {
    compare_value(
        format!("{path}.lowering.stream"),
        &old.stream,
        &new.stream,
        changes,
    );
    compare_value(
        format!("{path}.lowering.resource"),
        &old.resource,
        &new.resource,
        changes,
    );
}

/// Report an ordered collection length change as breaking.
fn compare_length(path: String, old: usize, new: usize, changes: &mut Vec<Change>) {
    if old != new {
        changes.push(breaking(path, format!("changed from {old} to {new}")));
    }
}

/// Report one changed contract value with stable debug rendering.
fn compare_value<T: Debug + PartialEq>(path: String, old: &T, new: &T, changes: &mut Vec<Change>) {
    if old != new {
        changes.push(breaking(path, format!("changed from {old:?} to {new:?}")));
    }
}

/// Construct an additive compatibility change.
fn additive(path: impl Into<String>, message: impl Into<String>) -> Change {
    Change {
        kind: ChangeKind::Additive,
        path: path.into(),
        message: message.into(),
    }
}

/// Construct a breaking compatibility change.
fn breaking(path: impl Into<String>, message: impl Into<String>) -> Change {
    Change {
        kind: ChangeKind::Breaking,
        path: path.into(),
        message: message.into(),
    }
}
