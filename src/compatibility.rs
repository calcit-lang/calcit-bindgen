use std::collections::BTreeMap;

use serde::Serialize;

use crate::Document;

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

pub fn compare(old: &Document, new: &Document) -> CompatibilityReport {
    let mut changes = Vec::new();
    if old.package != new.package {
        changes.push(breaking(
            "package",
            format!("package changed from {} to {}", old.package, new.package),
        ));
    }
    compare_items(
        "declarations",
        old.declarations.iter().map(|item| (item.id(), item)),
        new.declarations.iter().map(|item| (item.id(), item)),
        &mut changes,
    );
    compare_items(
        "definitions",
        old.definitions.iter().map(|item| (item.id.as_str(), item)),
        new.definitions.iter().map(|item| (item.id.as_str(), item)),
        &mut changes,
    );
    CompatibilityReport {
        compatible: !changes
            .iter()
            .any(|change| change.kind == ChangeKind::Breaking),
        changes,
    }
}

fn compare_items<'a, T: PartialEq + ?Sized + 'a>(
    section: &str,
    old: impl Iterator<Item = (&'a str, &'a T)>,
    new: impl Iterator<Item = (&'a str, &'a T)>,
    changes: &mut Vec<Change>,
) {
    let old = old.collect::<BTreeMap<_, _>>();
    let new = new.collect::<BTreeMap<_, _>>();
    for (id, old_value) in &old {
        match new.get(id) {
            None => changes.push(breaking(format!("{section}.{id}"), "removed")),
            Some(new_value) if *old_value != *new_value => {
                changes.push(breaking(format!("{section}.{id}"), "contract changed"));
            }
            Some(_) => {}
        }
    }
    for id in new.keys() {
        if !old.contains_key(id) {
            changes.push(Change {
                kind: ChangeKind::Additive,
                path: format!("{section}.{id}"),
                message: "added".to_owned(),
            });
        }
    }
}

fn breaking(path: impl Into<String>, message: impl Into<String>) -> Change {
    Change {
        kind: ChangeKind::Breaking,
        path: path.into(),
        message: message.into(),
    }
}
