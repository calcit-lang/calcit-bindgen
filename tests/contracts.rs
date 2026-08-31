use calcit_bindgen::{
    ChangeKind, Declaration, Definition, DefinitionStatus, Document, FunctionSignature, Lowering,
    Ownership, Parameter, ResourceLowering, ResourceParameterOwnership, StreamLowering,
    StructField, Type, compare, validate_document,
};
use std::fs;
use std::process::Command;

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
                stream: None,
                resource: None,
                raw: String::new(),
            },
            status: DefinitionStatus::Supported,
            diagnostic_codes: vec![],
        }],
    }
}

fn lifecycle_document() -> Document {
    Document {
        version: 3,
        package: "demo.lifecycle".to_owned(),
        package_version: "0.1.0".to_owned(),
        declarations: vec![Declaration::Enum {
            id: "demo.lifecycle/Event".to_owned(),
            namespace: "demo.lifecycle".to_owned(),
            name: "Event".to_owned(),
            type_parameters: vec![],
            variants: vec![calcit_bindgen::EnumVariant {
                name: "message".to_owned(),
                payload: vec![Type::String],
            }],
        }],
        definitions: vec![
            Definition {
                id: "demo.lifecycle/serve".to_owned(),
                namespace: "demo.lifecycle".to_owned(),
                name: "serve".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                signature: None,
                lowering: Lowering {
                    backend: Some("native".to_owned()),
                    target: None,
                    kind: Some("async-stream".to_owned()),
                    symbol: Some("serve".to_owned()),
                    invoke: Some("async".to_owned()),
                    transport: Some("async-task-v1".to_owned()),
                    stream: Some(StreamLowering {
                        callback_parameter: 1,
                        event_type: Type::Enum {
                            id: "demo.lifecycle/Event".to_owned(),
                            arguments: vec![],
                        },
                        callback_result: Type::Unit,
                        cancel: "cooperative".to_owned(),
                        task_result: Ownership::Own,
                    }),
                    resource: None,
                    raw: String::new(),
                },
                status: DefinitionStatus::Unsupported,
                diagnostic_codes: vec!["E_FFI_IR_UNSUPPORTED_TYPE".to_owned()],
            },
            Definition {
                id: "demo.lifecycle/compile".to_owned(),
                namespace: "demo.lifecycle".to_owned(),
                name: "compile".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                signature: None,
                lowering: Lowering {
                    backend: Some("native".to_owned()),
                    target: None,
                    kind: Some("resource-constructor".to_owned()),
                    symbol: Some("compile".to_owned()),
                    invoke: Some("sync".to_owned()),
                    transport: Some("edn-buffer-v1".to_owned()),
                    stream: None,
                    resource: Some(ResourceLowering {
                        protocol: "opaque-resource-v1".to_owned(),
                        result: Some(Ownership::Own),
                        parameters: vec![],
                    }),
                    raw: String::new(),
                },
                status: DefinitionStatus::Unsupported,
                diagnostic_codes: vec!["E_FFI_IR_UNSUPPORTED_TYPE".to_owned()],
            },
            Definition {
                id: "demo.lifecycle/source".to_owned(),
                namespace: "demo.lifecycle".to_owned(),
                name: "source".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                signature: None,
                lowering: Lowering {
                    backend: Some("native".to_owned()),
                    target: None,
                    kind: Some("resource-method".to_owned()),
                    symbol: Some("source".to_owned()),
                    invoke: Some("sync".to_owned()),
                    transport: Some("edn-buffer-v1".to_owned()),
                    stream: None,
                    resource: Some(ResourceLowering {
                        protocol: "opaque-resource-v1".to_owned(),
                        result: None,
                        parameters: vec![ResourceParameterOwnership {
                            position: 0,
                            ownership: Ownership::Borrow,
                        }],
                    }),
                    raw: String::new(),
                },
                status: DefinitionStatus::Unsupported,
                diagnostic_codes: vec!["E_FFI_IR_UNSUPPORTED_TYPE".to_owned()],
            },
        ],
    }
}

#[test]
fn validates_a_monomorphic_composite_contract() {
    validate_document(&document()).expect("valid v2 document");
}

#[test]
fn v2_serialization_omits_absent_lifecycle_fields() {
    let encoded = serde_json::to_string(&document()).expect("serialize v2 document");
    assert!(!encoded.contains("\"stream\""));
    assert!(!encoded.contains("\"resource\""));
}

#[test]
fn validates_v3_lifecycle_contracts_without_enabling_generation() {
    let lifecycle = lifecycle_document();
    validate_document(&lifecycle).expect("valid lifecycle v3 document");
    let encoded = serde_json::to_string(&lifecycle).expect("serialize lifecycle v3 document");
    assert!(encoded.contains("\"event\""));
    let decoded: Document =
        serde_json::from_str(&encoded).expect("deserialize lifecycle v3 document");
    assert_eq!(decoded, lifecycle);

    let mut v2 = lifecycle_document();
    v2.version = 2;
    assert!(
        validate_document(&v2)
            .expect_err("v2 cannot contain lifecycle fields")
            .contains("requires Interface IR v3")
    );

    let mut invalid_stream = lifecycle_document();
    invalid_stream.definitions[0]
        .lowering
        .stream
        .as_mut()
        .expect("stream")
        .cancel = "best-effort".to_owned();
    assert!(
        validate_document(&invalid_stream)
            .expect_err("cancel mode must be precise")
            .contains("must be cooperative")
    );

    let mut invalid_resource = lifecycle_document();
    invalid_resource.definitions[2]
        .lowering
        .resource
        .as_mut()
        .expect("resource")
        .parameters[0]
        .ownership = Ownership::Own;
    assert!(
        validate_document(&invalid_resource)
            .expect_err("input ownership must not imply an unspecified consume")
            .contains("cannot be own")
    );

    let mut invalid_stream_parameter = lifecycle_document();
    invalid_stream_parameter.definitions[0].signature = Some(FunctionSignature {
        parameters: vec![Parameter {
            position: 3,
            type_ir: Type::String,
        }],
        result: Type::Unit,
    });
    assert!(
        validate_document(&invalid_stream_parameter)
            .expect_err("callback must name a declared parameter position")
            .contains("does not reference a declared parameter position")
    );

    let mut invalid_resource_parameter = lifecycle_document();
    invalid_resource_parameter.definitions[2].signature = Some(FunctionSignature {
        parameters: vec![Parameter {
            position: 3,
            type_ir: Type::String,
        }],
        result: Type::String,
    });
    assert!(
        validate_document(&invalid_resource_parameter)
            .expect_err("resource ownership must name a declared parameter position")
            .contains("does not reference a declared parameter position")
    );

    let mut prematurely_supported = lifecycle_document();
    prematurely_supported.definitions[0].status = DefinitionStatus::Supported;
    prematurely_supported.definitions[0].signature = Some(FunctionSignature {
        parameters: vec![],
        result: Type::Unit,
    });
    prematurely_supported.definitions[0]
        .diagnostic_codes
        .clear();
    assert!(
        validate_document(&prematurely_supported)
            .expect_err("lifecycle metadata must not claim generated support yet")
            .contains("must remain unsupported")
    );
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
    assert_eq!(
        breaking_report.changes[0].path,
        "definitions.demo/read.signature.result"
    );
}

#[test]
fn compatibility_diff_ignores_non_contract_metadata() {
    let old = document();
    let mut new = old.clone();
    new.package_version = "0.2.0".to_owned();
    new.definitions[0].doc = "updated documentation".to_owned();
    new.definitions[0].logical_schema = "display formatting changed".to_owned();
    new.definitions[0].lowering.raw = "debug rendering changed".to_owned();
    new.definitions[0].diagnostic_codes = vec!["W_DISPLAY_ONLY".to_owned()];

    let report = compare(&old, &new);
    assert!(report.compatible);
    assert!(report.changes.is_empty());
}

#[test]
fn compatibility_diff_treats_lifecycle_changes_as_breaking_before_generation_exists() {
    let old = lifecycle_document();
    let mut new = old.clone();
    new.definitions[0]
        .lowering
        .stream
        .as_mut()
        .expect("stream")
        .cancel = "manual".to_owned();
    let report = compare(&old, &new);
    assert!(!report.compatible);
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].kind, ChangeKind::Breaking);
    assert_eq!(
        report.changes[0].path,
        "definitions.demo.lifecycle/serve.lowering.stream"
    );
}

#[test]
fn compatibility_diff_reports_precise_signature_lowering_and_declaration_paths() {
    let mut old = document();
    if let Declaration::Struct { fields, .. } = &mut old.declarations[0] {
        fields.push(StructField {
            name: "name".to_owned(),
            type_ir: Type::String,
        });
    }
    let mut new = old.clone();
    if let Declaration::Struct { fields, .. } = &mut new.declarations[0] {
        fields[0].type_ir = Type::Number;
    }
    new.definitions[0]
        .signature
        .as_mut()
        .expect("signature")
        .result = Type::Number;
    new.definitions[0].lowering.symbol = Some("read_v2".to_owned());

    let report = compare(&old, &new);
    assert!(!report.compatible);
    assert_eq!(
        report
            .changes
            .iter()
            .map(|change| change.path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "declarations.demo/Person.fields[0].type",
            "definitions.demo/read.signature.result",
            "definitions.demo/read.lowering.symbol",
        ]
    );
}

#[test]
fn compatibility_diff_covers_every_structured_lowering_field() {
    let old = document();
    macro_rules! assert_lowering_change {
        ($field:ident, $value:expr) => {{
            let mut new = old.clone();
            new.definitions[0].lowering.$field = $value;
            let report = compare(&old, &new);
            assert!(
                !report.compatible,
                "{} must be breaking",
                stringify!($field)
            );
            assert_eq!(report.changes.len(), 1);
            assert_eq!(
                report.changes[0].path,
                format!("definitions.demo/read.lowering.{}", stringify!($field))
            );
        }};
    }

    assert_lowering_change!(backend, Some("wasm".to_owned()));
    assert_lowering_change!(target, Some("host-v2".to_owned()));
    assert_lowering_change!(kind, Some("static-method".to_owned()));
    assert_lowering_change!(symbol, Some("read_v2".to_owned()));
    assert_lowering_change!(invoke, Some("blocking".to_owned()));
    assert_lowering_change!(transport, Some("typed-buffer-v1".to_owned()));
}

#[test]
fn compatibility_diff_treats_new_support_as_additive_and_removed_support_as_breaking() {
    let mut unsupported = document();
    unsupported.definitions[0].status = DefinitionStatus::Unsupported;
    unsupported.definitions[0].signature = None;
    unsupported.definitions[0].diagnostic_codes = vec!["E_UNSUPPORTED".to_owned()];

    let enabled = compare(&unsupported, &document());
    assert!(enabled.compatible);
    assert_eq!(enabled.changes.len(), 1);
    assert_eq!(enabled.changes[0].kind, ChangeKind::Additive);
    assert_eq!(enabled.changes[0].path, "definitions.demo/read.status");

    let disabled = compare(&document(), &unsupported);
    assert!(!disabled.compatible);
    assert_eq!(disabled.changes.len(), 1);
    assert_eq!(disabled.changes[0].kind, ChangeKind::Breaking);
    assert_eq!(disabled.changes[0].path, "definitions.demo/read.status");
}

#[test]
fn compatibility_diff_reports_duplicate_ids_from_public_documents() {
    let baseline = document();

    let mut old_declarations = baseline.clone();
    old_declarations
        .declarations
        .push(old_declarations.declarations[0].clone());
    let report = compare(&old_declarations, &baseline);
    assert!(!report.compatible);
    assert!(report.changes.iter().any(|change| {
        change.path == "old.declarations.demo/Person" && change.message == "duplicate ID"
    }));

    let mut new_declarations = baseline.clone();
    new_declarations
        .declarations
        .push(new_declarations.declarations[0].clone());
    let report = compare(&baseline, &new_declarations);
    assert!(!report.compatible);
    assert!(report.changes.iter().any(|change| {
        change.path == "new.declarations.demo/Person" && change.message == "duplicate ID"
    }));

    let mut old_definitions = baseline.clone();
    old_definitions
        .definitions
        .push(old_definitions.definitions[0].clone());
    let report = compare(&old_definitions, &baseline);
    assert!(!report.compatible);
    assert!(report.changes.iter().any(|change| {
        change.path == "old.definitions.demo/read" && change.message == "duplicate ID"
    }));

    let mut new_definitions = baseline.clone();
    new_definitions
        .definitions
        .push(new_definitions.definitions[0].clone());
    let report = compare(&baseline, &new_definitions);
    assert!(!report.compatible);
    assert!(report.changes.iter().any(|change| {
        change.path == "new.definitions.demo/read" && change.message == "duplicate ID"
    }));
}

#[test]
fn diff_cli_uses_the_semantic_policy_for_json_and_text_output() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let old_path = directory.path().join("old.json");
    let metadata_path = directory.path().join("metadata.json");
    let breaking_path = directory.path().join("breaking.json");
    let old = document();
    fs::write(
        &old_path,
        serde_json::to_vec_pretty(&old).expect("serialize old document"),
    )
    .expect("write old document");

    let mut metadata = old.clone();
    metadata.package_version = "0.2.0".to_owned();
    metadata.definitions[0].doc = "new docs".to_owned();
    metadata.definitions[0].lowering.raw = "new debug text".to_owned();
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).expect("serialize metadata document"),
    )
    .expect("write metadata document");

    let compatible = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "diff",
            old_path.to_str().expect("old path"),
            metadata_path.to_str().expect("metadata path"),
            "--json",
        ])
        .output()
        .expect("run compatible diff");
    assert!(compatible.status.success());
    let compatible_json: serde_json::Value =
        serde_json::from_slice(&compatible.stdout).expect("compatible JSON report");
    assert_eq!(compatible_json["compatible"], true);
    assert_eq!(compatible_json["changes"], serde_json::json!([]));

    let mut breaking = old.clone();
    breaking.definitions[0].lowering.symbol = Some("read_v2".to_owned());
    fs::write(
        &breaking_path,
        serde_json::to_vec_pretty(&breaking).expect("serialize breaking document"),
    )
    .expect("write breaking document");

    let json = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "diff",
            old_path.to_str().expect("old path"),
            breaking_path.to_str().expect("breaking path"),
            "--json",
        ])
        .output()
        .expect("run breaking JSON diff");
    assert!(!json.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("breaking JSON report");
    assert_eq!(
        report["changes"][0]["path"],
        "definitions.demo/read.lowering.symbol"
    );

    let text = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "diff",
            old_path.to_str().expect("old path"),
            breaking_path.to_str().expect("breaking path"),
        ])
        .output()
        .expect("run breaking text diff");
    assert!(!text.status.success());
    assert!(
        String::from_utf8(text.stdout)
            .expect("text stdout")
            .contains("definitions.demo/read.lowering.symbol")
    );
}
