use calcit_bindgen::{
    ChangeKind, Declaration, Definition, DefinitionStatus, Document, FunctionSignature, Lowering,
    Parameter, StructField, Type, compare, load_document, validate_document,
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

fn export_envelope(document: &Document) -> serde_json::Value {
    let diagnostics: Vec<serde_json::Value> = vec![];
    let revision_payload =
        serde_json::to_vec(&(document, &diagnostics)).expect("encode revision input");
    serde_json::json!({
        "schema_version": 1,
        "interface_schema": "https://calcit-lang.org/schemas/ffi-interface-ir-v2.schema.json",
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
                "supported": document.definitions.len(),
                "unsupported": 0,
                "diagnostics": 0,
            },
        },
        "diagnostics": diagnostics,
    })
}

#[test]
fn validates_the_complete_ffi_export_envelope_contract() {
    let envelope = export_envelope(&document());
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
