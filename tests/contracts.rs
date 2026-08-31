use calcit_bindgen::{
    ChangeKind, Declaration, Definition, DefinitionStatus, Document, FunctionSignature, Lowering,
    Parameter, Type, compare, validate_document,
};

fn document() -> Document {
    Document {
        version: 2,
        package: "demo".to_owned(),
        package_version: "0.1.0".to_owned(),
        declarations: vec![Declaration::Struct {
            id: "demo/Person".to_owned(),
            namespace: "demo".to_owned(),
            name: "Person".to_owned(),
            type_parameters: vec![],
            fields: vec![],
        }],
        definitions: vec![Definition {
            id: "demo/read".to_owned(),
            namespace: "demo".to_owned(),
            name: "read".to_owned(),
            doc: String::new(),
            logical_schema: String::new(),
            signature: Some(FunctionSignature {
                parameters: vec![Parameter {
                    position: 0,
                    type_ir: Type::Struct {
                        id: "demo/Person".to_owned(),
                        arguments: vec![],
                    },
                }],
                result: Type::String,
            }),
            lowering: Lowering {
                backend: Some("native".to_owned()),
                target: None,
                kind: Some("dylib-method".to_owned()),
                symbol: Some("read".to_owned()),
                invoke: Some("sync".to_owned()),
                transport: Some("edn-buffer-v1".to_owned()),
                raw: String::new(),
            },
            status: DefinitionStatus::Supported,
            diagnostic_codes: vec![],
        }],
    }
}

#[test]
fn validates_a_monomorphic_composite_contract() {
    validate_document(&document()).expect("valid v2 document");
}

#[test]
fn rejects_unknown_versions_and_missing_declarations() {
    let mut old = document();
    old.version = 1;
    assert!(validate_document(&old).unwrap_err().contains("requires v2"));

    let mut missing = document();
    missing.declarations.clear();
    assert!(
        validate_document(&missing)
            .unwrap_err()
            .contains("missing declaration demo/Person")
    );
}

#[test]
fn compatibility_diff_classifies_additions_and_breaking_changes() {
    let old = document();
    let mut additive = old.clone();
    additive.definitions.push(Definition {
        id: "demo/extra".to_owned(),
        name: "extra".to_owned(),
        ..old.definitions[0].clone()
    });
    let additive_report = compare(&old, &additive);
    assert!(additive_report.compatible);
    assert_eq!(additive_report.changes[0].kind, ChangeKind::Additive);

    let mut breaking = old.clone();
    breaking.definitions[0]
        .signature
        .as_mut()
        .expect("signature")
        .result = Type::Number;
    let breaking_report = compare(&old, &breaking);
    assert!(!breaking_report.compatible);
    assert_eq!(breaking_report.changes[0].kind, ChangeKind::Breaking);
}
