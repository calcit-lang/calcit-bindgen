use calcit_bindgen::{
    ChangeKind, ComponentDefinition, ComponentDirection, ComponentDocument, ComponentInvocation,
    Declaration, Definition, DefinitionStatus, Document, EnumVariant, FunctionSignature,
    InterfaceContract, Lowering, Parameter, StructField, Type, compare, compare_component,
    load_contract, load_document, validate_component_document, validate_document,
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

fn load_json(value: &serde_json::Value) -> Result<Document, String> {
    let file = tempfile::NamedTempFile::new().expect("temporary Interface IR");
    fs::write(
        file.path(),
        serde_json::to_vec_pretty(value).expect("encode Interface IR fixture"),
    )
    .expect("write Interface IR fixture");
    load_document(file.path())
}

#[test]
fn loads_default_cirru_edn_component_contract() {
    let contract = load_contract("tests/fixtures/component-interface.cirru")
        .expect("load the public Component Interface IR fixture");
    let InterfaceContract::Component(document) = contract else {
        panic!("expected a Component contract");
    };
    assert_eq!(document.version, 2);
    assert_eq!(document.package, "component-wasm");
    assert_eq!(document.definitions.len(), 25);
    assert!(
        document
            .definitions
            .iter()
            .any(|definition| definition.symbol == "add-one")
    );
    assert!(document.definitions.iter().any(|definition| {
        definition.symbol == "echo-buffer"
            && definition.signature.as_ref().is_some_and(|signature| {
                signature.parameters[0].type_ir == Type::Buffer && signature.result == Type::Buffer
            })
    }));
    assert!(document.definitions.iter().any(|definition| {
        definition.symbol == "echo-number-lists"
            && definition.signature.as_ref().is_some_and(|signature| {
                signature.parameters[0].type_ir
                    == Type::List {
                        item: Box::new(Type::List {
                            item: Box::new(Type::Number),
                        }),
                    }
                    && signature.result
                        == Type::List {
                            item: Box::new(Type::List {
                                item: Box::new(Type::Number),
                            }),
                        }
            })
    }));
}

#[test]
fn component_compatibility_tracks_numeric_widths() {
    let InterfaceContract::Component(old) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load Component contract")
    else {
        panic!("expected a Component contract");
    };
    let mut new = old.clone();
    let declaration = new
        .declarations
        .iter_mut()
        .find(|declaration| declaration.id() == "component-wasm.main/NumericScalars")
        .expect("numeric Struct declaration");
    let Declaration::Struct { fields, .. } = declaration else {
        panic!("expected numeric Struct");
    };
    fields
        .iter_mut()
        .find(|field| field.name == "u16")
        .expect("u16 field")
        .type_ir = Type::Uint32;

    let report = compare_component(&old, &new);
    assert!(!report.compatible);
    assert!(report.changes.iter().any(|change| {
        change.path.contains("NumericScalars.fields")
            && change.message.contains("Uint16")
            && change.message.contains("Uint32")
    }));
}

fn write_component_envelope(path: &std::path::Path, document: &ComponentDocument) {
    let diagnostics = Vec::<serde_json::Value>::new();
    let revision = format!(
        "md5:{:x}",
        md5::compute(serde_json::to_vec(&(document, &diagnostics)).expect("revision input"))
    );
    let definitions = document.definitions.len();
    let envelope = serde_json::json!({
        "schema_version": 1,
        "interface_schema": "https://calcit-lang.org/schemas/component-interface-ir-v2.schema.json",
        "command": "ffi.export",
        "revision": revision,
        "data": {
            "filters": {
                "boundary": "component",
                "namespace": null,
                "include_dependencies": false
            },
            "interface": document,
            "summary": {
                "definitions": definitions,
                "supported": definitions,
                "unsupported": 0,
                "diagnostics": 0
            }
        },
        "diagnostics": diagnostics
    });
    fs::write(
        path,
        serde_json::to_vec_pretty(&envelope).expect("encode Component envelope"),
    )
    .expect("write Component envelope");
}

#[test]
fn diff_cli_reports_component_numeric_width_changes() {
    let InterfaceContract::Component(old) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load Component contract")
    else {
        panic!("expected a Component contract");
    };
    let mut new = old.clone();
    let definition = new
        .definitions
        .iter_mut()
        .find(|definition| definition.symbol == "echo-numeric-scalars")
        .expect("numeric export");
    definition
        .signature
        .as_mut()
        .expect("numeric signature")
        .result = Type::Float64;

    let directory = tempfile::tempdir().expect("temporary directory");
    let old_path = directory.path().join("old.json");
    let new_path = directory.path().join("new.json");
    write_component_envelope(&old_path, &old);
    write_component_envelope(&new_path, &new);
    let output = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "diff",
            old_path.to_str().expect("old path"),
            new_path.to_str().expect("new path"),
            "--json",
        ])
        .output()
        .expect("run Component diff");
    assert!(!output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Component diff report");
    assert_eq!(report["compatible"], false);
    assert_eq!(
        report["changes"][0]["path"],
        "definitions.component-wasm.main/echo-numeric-scalars.signature.result"
    );
}

#[test]
fn rejects_old_component_contract_versions() {
    let InterfaceContract::Component(mut document) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load Component contract")
    else {
        panic!("expected a Component contract");
    };
    document.version = 1;
    assert!(
        validate_component_document(&document)
            .unwrap_err()
            .contains("supports v2, v3, and v4")
    );
}

#[test]
fn validates_component_v3_invocation_contracts() {
    let InterfaceContract::Component(mut document) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load Component contract")
    else {
        panic!("expected a Component contract");
    };
    document.version = 3;
    for definition in &mut document.definitions {
        definition.invocation = Some(ComponentInvocation::Sync);
    }
    validate_component_document(&document).expect("valid Component Interface IR v3 document");

    document.definitions[0].invocation = None;
    assert!(
        validate_component_document(&document)
            .unwrap_err()
            .contains("must declare invocation")
    );
}

fn component_v4_with_declaration(
    declaration: Declaration,
    boundary_type: Type,
    use_as_result: bool,
) -> ComponentDocument {
    ComponentDocument {
        version: 4,
        package: "stream-declaration".to_owned(),
        package_version: "0.0.0".to_owned(),
        declarations: vec![declaration],
        definitions: vec![ComponentDefinition {
            id: "stream-declaration/consume".to_owned(),
            namespace: "stream-declaration".to_owned(),
            name: "consume".to_owned(),
            doc: String::new(),
            logical_schema: String::new(),
            direction: ComponentDirection::Export,
            invocation: Some(ComponentInvocation::Async),
            module: None,
            symbol: "consume".to_owned(),
            signature: Some(FunctionSignature {
                parameters: if use_as_result {
                    Vec::new()
                } else {
                    vec![Parameter {
                        position: 0,
                        type_ir: boundary_type.clone(),
                    }]
                },
                result: if use_as_result {
                    boundary_type
                } else {
                    Type::Unit
                },
            }),
            status: DefinitionStatus::Supported,
            diagnostic_codes: Vec::new(),
        }],
    }
}

#[test]
fn component_v4_rejects_streams_hidden_in_struct_and_enum_declarations() {
    let structure = Declaration::Struct {
        id: "stream-declaration/Envelope".to_owned(),
        namespace: "stream-declaration".to_owned(),
        name: "Envelope".to_owned(),
        type_parameters: Vec::new(),
        fields: vec![StructField {
            name: "body".to_owned(),
            type_ir: Type::ReadableByteStream,
        }],
    };
    let structure_ref = Type::Struct {
        id: "stream-declaration/Envelope".to_owned(),
        arguments: Vec::new(),
    };
    for use_as_result in [false, true] {
        let error = validate_component_document(&component_v4_with_declaration(
            structure.clone(),
            structure_ref.clone(),
            use_as_result,
        ))
        .expect_err("Struct declarations must not hide readable byte streams");
        assert!(error.contains("Envelope.fields.body"), "{error}");
    }

    let enumeration = Declaration::Enum {
        id: "stream-declaration/Payload".to_owned(),
        namespace: "stream-declaration".to_owned(),
        name: "Payload".to_owned(),
        type_parameters: Vec::new(),
        variants: vec![EnumVariant {
            name: "body".to_owned(),
            payload: vec![Type::Option {
                item: Box::new(Type::ReadableByteStream),
            }],
        }],
    };
    let enumeration_ref = Type::Enum {
        id: "stream-declaration/Payload".to_owned(),
        arguments: Vec::new(),
    };
    for use_as_result in [false, true] {
        let error = validate_component_document(&component_v4_with_declaration(
            enumeration.clone(),
            enumeration_ref.clone(),
            use_as_result,
        ))
        .expect_err("Enum declarations must not hide readable byte streams");
        assert!(
            error.contains("Payload.variants.body.payload[0]"),
            "{error}"
        );
    }
}

#[test]
fn loads_explicit_json_component_projection() {
    let InterfaceContract::Component(document) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load EDN contract")
    else {
        panic!("expected a Component contract");
    };
    let diagnostics = Vec::<serde_json::Value>::new();
    let revision = format!(
        "md5:{:x}",
        md5::compute(serde_json::to_vec(&(&document, &diagnostics)).expect("revision input"))
    );
    let definitions = document.definitions.len();
    let envelope = serde_json::json!({
        "schema_version": 1,
        "interface_schema": "https://calcit-lang.org/schemas/component-interface-ir-v2.schema.json",
        "command": "ffi.export",
        "revision": revision,
        "data": {
            "filters": {
                "boundary": "component",
                "namespace": null,
                "include_dependencies": false
            },
            "interface": document,
            "summary": {
                "definitions": definitions,
                "supported": definitions,
                "unsupported": 0,
                "diagnostics": 0
            }
        },
        "diagnostics": diagnostics
    });
    let file = tempfile::NamedTempFile::new().expect("temporary Component JSON");
    fs::write(
        file.path(),
        serde_json::to_vec_pretty(&envelope).expect("encode Component JSON"),
    )
    .expect("write Component JSON");
    assert!(matches!(
        load_contract(file.path()).expect("load explicit JSON projection"),
        InterfaceContract::Component(_)
    ));
}

#[test]
fn loads_explicit_json_component_v3_projection() {
    let InterfaceContract::Component(mut document) =
        load_contract("tests/fixtures/component-interface.cirru").expect("load EDN contract")
    else {
        panic!("expected a Component contract");
    };
    document.version = 3;
    for definition in &mut document.definitions {
        definition.invocation = Some(ComponentInvocation::Sync);
    }
    let diagnostics = Vec::<serde_json::Value>::new();
    let revision = format!(
        "md5:{:x}",
        md5::compute(serde_json::to_vec(&(&document, &diagnostics)).expect("revision input"))
    );
    let definitions = document.definitions.len();
    let envelope = serde_json::json!({
        "schema_version": 1,
        "interface_schema": "https://calcit-lang.org/schemas/component-interface-ir-v3.schema.json",
        "command": "ffi.export",
        "revision": revision,
        "data": {
            "filters": {
                "boundary": "component",
                "namespace": null,
                "include_dependencies": false
            },
            "interface": document,
            "summary": {
                "definitions": definitions,
                "supported": definitions,
                "unsupported": 0,
                "diagnostics": 0
            }
        },
        "diagnostics": diagnostics
    });
    let file = tempfile::NamedTempFile::new().expect("temporary Component v3 JSON");
    fs::write(
        file.path(),
        serde_json::to_vec_pretty(&envelope).expect("encode Component v3 JSON"),
    )
    .expect("write Component v3 JSON");
    assert!(matches!(
        load_contract(file.path()).expect("load explicit Component v3 JSON projection"),
        InterfaceContract::Component(_)
    ));
}

#[derive(Clone, serde::Serialize)]
struct TestDiagnostic {
    code: &'static str,
    phase: &'static str,
    severity: &'static str,
    definition: &'static str,
    path: &'static str,
    message: &'static str,
    suggestion: &'static str,
}

fn export_envelope(document: &Document, diagnostics: Vec<TestDiagnostic>) -> serde_json::Value {
    let revision_payload =
        serde_json::to_vec(&(document, &diagnostics)).expect("encode revision input");
    let supported = document
        .definitions
        .iter()
        .filter(|definition| definition.status == DefinitionStatus::Supported)
        .count();
    let interface_schema = format!(
        "https://calcit-lang.org/schemas/ffi-interface-ir-v{}.schema.json",
        document.version
    );
    serde_json::json!({
        "schema_version": 1,
        "interface_schema": interface_schema,
        "command": "ffi.export",
        "revision": format!("md5:{:x}", md5::compute(revision_payload)),
        "data": {
            "filters": {
                "namespace": null,
                "include_dependencies": false,
            },
            "interface": document,
            "summary": {
                "definitions": document.definitions.len(),
                "supported": supported,
                "unsupported": document.definitions.len() - supported,
                "diagnostics": diagnostics.len(),
            },
        },
        "diagnostics": diagnostics,
    })
}

#[test]
fn validates_the_complete_ffi_export_envelope_contract() {
    let envelope = export_envelope(&document(), vec![]);
    assert_eq!(
        load_json(&envelope).expect("valid export envelope"),
        document()
    );

    let mut wrong_schema = envelope.clone();
    wrong_schema["interface_schema"] = serde_json::json!("https://example.test/other.json");
    assert!(
        load_json(&wrong_schema)
            .unwrap_err()
            .contains("expected Interface IR schema")
    );

    let mut wrong_summary = envelope.clone();
    wrong_summary["data"]["summary"]["definitions"] = serde_json::json!(2);
    assert!(
        load_json(&wrong_summary)
            .unwrap_err()
            .contains("summary does not match")
    );

    let mut wrong_revision = envelope.clone();
    wrong_revision["revision"] = serde_json::json!("md5:00000000000000000000000000000000");
    assert!(
        load_json(&wrong_revision)
            .unwrap_err()
            .contains("revision mismatch")
    );

    let mut dependencies = envelope.clone();
    dependencies["data"]["filters"]["include_dependencies"] = serde_json::json!(true);
    assert!(
        load_json(&dependencies)
            .unwrap_err()
            .contains("must not include dependency")
    );

    let mut unknown_field = envelope;
    unknown_field["future_contract"] = serde_json::json!(true);
    assert!(
        load_json(&unknown_field)
            .unwrap_err()
            .contains("unknown field")
    );

    let mut raw_unknown_field = serde_json::to_value(document()).expect("encode raw document");
    raw_unknown_field["definitions"][0]["signature"]["future_contract"] = serde_json::json!(true);
    assert!(
        load_json(&raw_unknown_field)
            .unwrap_err()
            .contains("unknown field")
    );

    let mut unsupported = document();
    unsupported.definitions[0].status = DefinitionStatus::Unsupported;
    unsupported.definitions[0].signature = None;
    unsupported.definitions[0].diagnostic_codes = vec!["E_TEST_BOUNDARY".to_owned()];
    let diagnostic = TestDiagnostic {
        code: "E_TEST_BOUNDARY",
        phase: "ffi-interface-ir",
        severity: "error",
        definition: "demo/read",
        path: "signature.result",
        message: "test boundary is unsupported",
        suggestion: "keep it behind a handwritten adapter",
    };
    let diagnostic_envelope = export_envelope(&unsupported, vec![diagnostic.clone()]);
    assert_eq!(
        load_json(&diagnostic_envelope).expect("valid envelope with a diagnostic"),
        unsupported
    );

    let missing_structured_diagnostic = export_envelope(&unsupported, vec![]);
    assert!(
        load_json(&missing_structured_diagnostic)
            .unwrap_err()
            .contains("without a structured diagnostic")
    );

    let unknown_definition = TestDiagnostic {
        definition: "demo/missing",
        ..diagnostic.clone()
    };
    assert!(
        load_json(&export_envelope(&unsupported, vec![unknown_definition]))
            .unwrap_err()
            .contains("references unknown definition")
    );

    let unknown_code = TestDiagnostic {
        code: "E_OTHER_BOUNDARY",
        ..diagnostic.clone()
    };
    assert!(
        load_json(&export_envelope(&unsupported, vec![unknown_code]))
            .unwrap_err()
            .contains("is not listed by definition")
    );

    let mut incomplete_diagnostic = diagnostic_envelope;
    incomplete_diagnostic["diagnostics"][0]
        .as_object_mut()
        .expect("diagnostic object")
        .remove("suggestion");
    assert!(
        load_json(&incomplete_diagnostic)
            .unwrap_err()
            .contains("missing field `suggestion`")
    );
}

#[test]
fn loads_native_v3_export_envelope_with_numeric_widths() {
    let mut document = document();
    document.version = 3;
    document.definitions[0]
        .signature
        .as_mut()
        .expect("signature")
        .result = Type::Int32;
    let envelope = export_envelope(&document, vec![]);
    assert_eq!(
        load_json(&envelope).expect("valid native Interface IR v3 envelope"),
        document
    );
}

#[test]
fn rejects_unknown_versions_and_missing_declarations() {
    let mut old = document();
    old.version = 1;
    assert!(
        validate_document(&old)
            .unwrap_err()
            .contains("supports v2 and v3")
    );

    let mut missing = document();
    missing.declarations.clear();
    assert!(
        validate_document(&missing)
            .unwrap_err()
            .contains("missing declaration demo/Person")
    );

    let mut component_only_numeric = document();
    component_only_numeric.definitions[0]
        .signature
        .as_mut()
        .expect("signature")
        .result = Type::Int32;
    assert!(
        validate_document(&component_only_numeric)
            .unwrap_err()
            .contains("require Interface IR v3")
    );

    component_only_numeric.version = 3;
    validate_document(&component_only_numeric).expect("native Interface IR v3 numeric width");
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
