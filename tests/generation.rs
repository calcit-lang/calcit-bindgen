use std::fs;
use std::process::Command;

use calcit_bindgen::{
    CheckIssueKind, Declaration, Definition, DefinitionStatus, Document, FunctionSignature,
    INTERFACE_FILE, Lowering, MANIFEST_FILE, Manifest, Parameter, StructField, Type,
    check_directory, generate_directory,
};

fn document() -> Document {
    let lowering = Lowering {
        backend: Some("native".to_owned()),
        target: None,
        kind: Some("dylib-method".to_owned()),
        symbol: Some("read".to_owned()),
        invoke: Some("sync".to_owned()),
        transport: Some("edn-buffer-v1".to_owned()),
        raw: String::new(),
    };
    let signature = FunctionSignature {
        parameters: vec![Parameter {
            position: 0,
            type_ir: Type::Struct {
                id: "demo/Person".to_owned(),
                arguments: vec![],
            },
        }],
        result: Type::String,
    };
    Document {
        version: 2,
        package: "demo".to_owned(),
        package_version: "0.1.0".to_owned(),
        declarations: vec![
            Declaration::Struct {
                id: "demo/Person".to_owned(),
                namespace: "demo".to_owned(),
                name: "Person".to_owned(),
                type_parameters: vec![],
                fields: vec![StructField {
                    name: "name".to_owned(),
                    type_ir: Type::String,
                }],
            },
            Declaration::Enum {
                id: "demo/Choice".to_owned(),
                namespace: "demo".to_owned(),
                name: "Choice".to_owned(),
                type_parameters: vec![],
                variants: vec![],
            },
        ],
        definitions: vec![
            Definition {
                id: "demo/write".to_owned(),
                namespace: "demo".to_owned(),
                name: "write".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                signature: Some(signature.clone()),
                lowering: Lowering {
                    symbol: Some("write".to_owned()),
                    ..lowering.clone()
                },
                status: DefinitionStatus::Supported,
                diagnostic_codes: vec![],
            },
            Definition {
                id: "demo/read".to_owned(),
                namespace: "demo".to_owned(),
                name: "read".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                signature: Some(signature),
                lowering,
                status: DefinitionStatus::Supported,
                diagnostic_codes: vec![],
            },
        ],
    }
}

#[test]
fn generate_is_deterministic_and_check_is_read_only() {
    let root = tempfile::tempdir().expect("temporary directory");
    let output = root.path().join("generated");
    let first_manifest = generate_directory(&document(), &output).expect("first generation");
    assert_eq!(first_manifest.files.len(), 1);
    assert_eq!(first_manifest.files[0].path, INTERFACE_FILE);
    assert_eq!(first_manifest.digest_algorithm, "fnv1a-128");
    let first_interface = fs::read(output.join(INTERFACE_FILE)).expect("first interface");
    let first_manifest_bytes = fs::read(output.join(MANIFEST_FILE)).expect("first manifest");

    let mut reordered = document();
    reordered.declarations.reverse();
    reordered.definitions.reverse();
    let second_manifest = generate_directory(&reordered, &output).expect("second generation");
    assert_eq!(first_manifest, second_manifest);
    assert_eq!(
        first_interface,
        fs::read(output.join(INTERFACE_FILE)).expect("second interface")
    );
    assert_eq!(
        first_manifest_bytes,
        fs::read(output.join(MANIFEST_FILE)).expect("second manifest")
    );

    let canonical: Document =
        serde_json::from_slice(&first_interface).expect("canonical interface");
    assert_eq!(canonical.declarations[0].id(), "demo/Choice");
    assert_eq!(canonical.definitions[0].id, "demo/read");

    let before = fs::read_dir(&output).expect("generated directory").count();
    let report = check_directory(&document(), &output).expect("check output");
    assert!(report.current);
    assert!(report.issues.is_empty());
    assert_eq!(
        before,
        fs::read_dir(&output)
            .expect("generated directory after check")
            .count()
    );
    assert_eq!(
        first_interface,
        fs::read(output.join(INTERFACE_FILE)).expect("interface after read-only check")
    );
    assert_eq!(
        first_manifest_bytes,
        fs::read(output.join(MANIFEST_FILE)).expect("manifest after read-only check")
    );
    assert_eq!(
        fs::read_dir(root.path())
            .expect("generation parent")
            .map(|entry| entry.expect("parent entry").file_name())
            .collect::<Vec<_>>(),
        vec!["generated"]
    );
}

#[test]
fn generation_rejects_unsupported_definitions_before_writing() {
    let root = tempfile::tempdir().expect("temporary directory");
    let output = root.path().join("generated");
    let mut unsupported = document();
    unsupported.definitions[0].status = DefinitionStatus::Unsupported;
    unsupported.definitions[0].signature = None;
    unsupported.definitions[0].diagnostic_codes = vec!["E_UNSUPPORTED".to_owned()];

    let error = generate_directory(&unsupported, &output).expect_err("unsupported generation");
    assert!(error.contains("unsupported: demo/write"));
    assert!(!output.exists());
}

#[test]
fn check_distinguishes_missing_modified_stale_and_unexpected_artifacts() {
    let root = tempfile::tempdir().expect("temporary directory");
    let output = root.path().join("generated");
    generate_directory(&document(), &output).expect("generate output");

    fs::remove_file(output.join(INTERFACE_FILE)).expect("remove interface");
    let missing = check_directory(&document(), &output).expect("missing check");
    assert!(
        missing
            .issues
            .iter()
            .any(|issue| { issue.kind == CheckIssueKind::Missing && issue.path == INTERFACE_FILE })
    );

    generate_directory(&document(), &output).expect("restore output");
    fs::write(output.join(INTERFACE_FILE), b"modified\n").expect("modify interface");
    let modified = check_directory(&document(), &output).expect("modified check");
    assert!(
        modified.issues.iter().any(|issue| {
            issue.kind == CheckIssueKind::Modified && issue.path == INTERFACE_FILE
        })
    );

    fs::remove_file(output.join(INTERFACE_FILE)).expect("remove modified interface");
    fs::create_dir(output.join(INTERFACE_FILE)).expect("replace interface with directory");
    let wrong_kind = check_directory(&document(), &output).expect("wrong-kind check");
    assert!(wrong_kind.issues.iter().any(|issue| {
        issue.kind == CheckIssueKind::Modified
            && issue.path == INTERFACE_FILE
            && issue.message.contains("regular file")
    }));
    fs::remove_dir(output.join(INTERFACE_FILE)).expect("remove wrong-kind interface");

    generate_directory(&document(), &output).expect("restore output again");
    let mut newer = document();
    newer.package_version = "0.2.0".to_owned();
    let stale = check_directory(&newer, &output).expect("stale check");
    assert!(stale.issues.iter().any(|issue| {
        issue.kind == CheckIssueKind::StaleManifest && issue.path == MANIFEST_FILE
    }));

    fs::write(output.join("manual.txt"), b"not generated").expect("write unexpected artifact");
    fs::create_dir(output.join("manual-dir")).expect("create unexpected directory");
    let unexpected = check_directory(&document(), &output).expect("unexpected check");
    assert!(
        unexpected.issues.iter().any(|issue| {
            issue.kind == CheckIssueKind::Unexpected && issue.path == "manual.txt"
        })
    );
    assert!(
        unexpected.issues.iter().any(|issue| {
            issue.kind == CheckIssueKind::Unexpected && issue.path == "manual-dir/"
        })
    );
    let generation_error = generate_directory(&document(), &output)
        .expect_err("generation must preserve unexpected user files");
    assert!(generation_error.contains("manual-dir/"));
    assert!(generation_error.contains("manual.txt"));
    assert_eq!(
        fs::read(output.join("manual.txt")).expect("preserved unexpected artifact"),
        b"not generated"
    );
}

#[test]
fn generate_refuses_to_replace_an_unowned_directory() {
    let root = tempfile::tempdir().expect("temporary directory");
    let output = root.path().join("generated");
    fs::create_dir(&output).expect("create output");
    fs::write(output.join("notes.txt"), b"user data").expect("write user data");

    let error = generate_directory(&document(), &output).expect_err("unowned output must fail");
    assert!(error.contains("refusing to replace unowned output directory"));
    assert_eq!(
        fs::read(output.join("notes.txt")).expect("preserved user data"),
        b"user data"
    );
}

#[test]
fn generate_and_check_cli_report_ci_friendly_status() {
    let root = tempfile::tempdir().expect("temporary directory");
    let input = root.path().join("interface-input.json");
    let output = root.path().join("generated");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&document()).expect("serialize input"),
    )
    .expect("write input");

    let generate = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "generate",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .output()
        .expect("run generate");
    assert!(generate.status.success());
    assert!(
        String::from_utf8(generate.stdout)
            .expect("generate stdout")
            .contains("generated 1 artifact(s)")
    );

    let check = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "check",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .output()
        .expect("run check");
    assert!(check.status.success());
    assert!(
        String::from_utf8(check.stdout)
            .expect("check stdout")
            .contains("generated artifacts are current")
    );

    fs::write(output.join(INTERFACE_FILE), b"modified\n").expect("modify generated artifact");
    let stale = Command::new(env!("CARGO_BIN_EXE_calcit-bindgen"))
        .args([
            "check",
            input.to_str().expect("input path"),
            "--out",
            output.to_str().expect("output path"),
        ])
        .output()
        .expect("run stale check");
    assert!(!stale.status.success());
    assert!(
        String::from_utf8(stale.stderr)
            .expect("stale stderr")
            .contains("[modified] interface.json")
    );

    let manifest: Manifest =
        serde_json::from_slice(&fs::read(output.join(MANIFEST_FILE)).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(manifest.schema_version, 1);
}
