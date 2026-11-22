use std::fs;
#[cfg(feature = "wasmtime-db")]
use std::time::Duration;

#[cfg(feature = "wasmtime-db")]
use calcit_bindgen::wasmtime_db::{
    DbConfig, DbError, DbParam, HttpDbServiceRunner, InMemoryKvBackend, KvBackend, SqlBackend,
    SqliteSqlBackend, WasmtimeDbState,
};
use calcit_bindgen::{
    CALCIT_DB_BINDINGS_FILE, ComponentDefinition, ComponentDirection, ComponentDocument,
    ComponentInvocation, Declaration, DefinitionStatus, EnumVariant, FunctionSignature,
    MANIFEST_FILE, Parameter, StructField, Type, WASMTIME_DB_ADAPTER_FILE, WIT_DB_FILE,
    check_contract_directory, generate_contract_directory, package_wasi_http_service,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wasm_encoder::{CustomSection, Section};
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

fn service_wat_full() -> String {
    r#"
    (module
      (import "wasi:http/types@0.2.12" "[resource-drop]incoming-request" (func $drop_req (param i32)))
      (import "wasi:http/types@0.2.12" "[method]incoming-request.path-with-query" (func $req_path (param i32 i32)))
      (import "wasi:http/types@0.2.12" "[constructor]fields" (func $new_fields (result i32)))
      (import "wasi:http/types@0.2.12" "[constructor]outgoing-response" (func $new_resp (param i32) (result i32)))
      (import "wasi:http/types@0.2.12" "[method]outgoing-response.set-status-code" (func $set_status (param i32 i32) (result i32)))
      (import "wasi:http/types@0.2.12" "[method]outgoing-response.body" (func $resp_body (param i32 i32)))
      (import "wasi:http/types@0.2.12" "[method]outgoing-body.write" (func $body_write (param i32 i32)))
      (import "wasi:http/types@0.2.12" "[static]outgoing-body.finish" (func $body_finish (param i32 i32 i32 i32)))
      (import "wasi:http/types@0.2.12" "[static]response-outparam.set" (func $outparam_set (param i32 i32 i32 i32 i64 i32 i32 i32 i32)))
      (import "wasi:io/streams@0.2.12" "[method]output-stream.blocking-write-and-flush" (func $stream_write (param i32 i32 i32 i32)))
      (import "wasi:io/streams@0.2.12" "[resource-drop]output-stream" (func $drop_stream (param i32)))

      (import "calcit:db/sql@0.1.0" "query" (func $sql_query (param i32 i32 i32 i32 i32)))
      (import "calcit:db/sql@0.1.0" "exec" (func $sql_exec (param i32 i32 i32 i32 i32)))
      (import "calcit:db/sql@0.1.0" "tx-begin" (func $sql_tx_begin (param i32)))
      (import "calcit:db/sql@0.1.0" "tx-exec" (func $sql_tx_exec (param i64 i32 i32 i32 i32 i32)))
      (import "calcit:db/kv@0.1.0" "get" (func $kv_get (param i32 i32 i32)))
      (import "calcit:db/kv@0.1.0" "set" (func $kv_set (param i32 i32 i32 i32 i32 i64 i32)))

      (memory (export "memory") 1)
      (global $heap (mut i32) (i32.const 1024))

      ;; 32..70: INSERT SQL (len 38)
      (data (i32.const 32) "INSERT INTO items (name) VALUES (?1)  ")
      ;; 80..85: "alpha" (len 5)
      (data (i32.const 80) "alpha")
      ;; 96..112: [DbParam::TextValue("alpha")] (size 16: disc=5 @0, ptr=80 @8, len=5 @12)
      (data (i32.const 96) "\05\00\00\00\00\00\00\00\50\00\00\00\05\00\00\00")
      ;; 128..136: "app:last" (len 8)
      (data (i32.const 128) "app:last")
      ;; 144..182: SELECT SQL (len 38)
      (data (i32.const 144) "SELECT id, name FROM items ORDER BY id")
      ;; 192..202: "secret:key" (len 10)
      (data (i32.const 192) "secret:key")
      ;; 208..214: "leaked" (len 6)
      (data (i32.const 208) "leaked")
      ;; 224..240: [DbParam::TextValue("leaked")] (disc=5 @0, ptr=208 @8, len=6 @12)
      (data (i32.const 224) "\05\00\00\00\00\00\00\00\d0\00\00\00\06\00\00\00")
      ;; 240..247: "exec ok" (len 7)
      (data (i32.const 240) "exec ok")

      (func (export "cabi_realloc") (param $old i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)
        (local $aligned i32)
        (local.set $aligned
          (i32.and
            (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
            (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
        (global.set $heap (i32.add (local.get $aligned) (local.get $new_size)))
        (local.get $aligned))

      (func $send_reply (param $out i32) (param $status i32) (param $ptr i32) (param $len i32)
        (local $fields i32)
        (local $resp i32)
        (local $body i32)
        (local $stream i32)
        (call $new_fields)
        (local.set $fields)
        (local.get $fields)
        (call $new_resp)
        (local.set $resp)
        (local.get $resp)
        (local.get $status)
        (call $set_status)
        (drop)
        (local.get $resp)
        (i32.const 768)
        (call $resp_body)
        (if (i32.load8_u (i32.const 768))
          (then (unreachable)))
        (i32.load (i32.const 772))
        (local.set $body)
        (local.get $body)
        (i32.const 768)
        (call $body_write)
        (if (i32.load8_u (i32.const 768))
          (then (unreachable)))
        (i32.load (i32.const 772))
        (local.set $stream)
        (local.get $out)
        (i32.const 0)
        (local.get $resp)
        (i32.const 0)
        (i64.const 0)
        (i32.const 0)
        (i32.const 0)
        (i32.const 0)
        (i32.const 0)
        (call $outparam_set)
        (local.get $stream)
        (local.get $ptr)
        (local.get $len)
        (i32.const 768)
        (call $stream_write)
        (if (i32.load8_u (i32.const 768))
          (then (unreachable)))
        (local.get $stream)
        (call $drop_stream)
        (local.get $body)
        (i32.const 0)
        (i32.const 0)
        (i32.const 768)
        (call $body_finish)
        (if (i32.load8_u (i32.const 768))
          (then (unreachable))))

      (func (export "wasi:http/incoming-handler@0.2.12#handle") (param $req i32) (param $out i32)
        (local $path_ptr i32)
        (local $path_len i32)
        (local $tag i32)
        (local $tx_id i64)

        (local.get $req)
        (i32.const 384)
        (call $req_path)
        (local.get $req)
        (call $drop_req)
        (if (i32.eqz (i32.load8_u (i32.const 384)))
          (then
            (call $send_reply (local.get $out) (i32.const 400) (i32.const 208) (i32.const 6))
            (return)))
        (local.set $path_ptr (i32.load (i32.const 388)))
        (local.set $path_len (i32.load (i32.const 392)))
        (if (i32.lt_u (local.get $path_len) (i32.const 2))
          (then
            (call $send_reply (local.get $out) (i32.const 404) (i32.const 208) (i32.const 6))
            (return)))
        (local.set $tag (i32.load8_u (i32.add (local.get $path_ptr) (i32.const 1))))

        ;; /init ('i' == 105)
        (if (i32.eq (local.get $tag) (i32.const 105))
          (then
            (call $sql_exec (i32.const 32) (i32.const 38) (i32.const 96) (i32.const 1) (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
                (return)))
            (call $kv_set (i32.const 128) (i32.const 8) (i32.const 80) (i32.const 5) (i32.const 0) (i64.const 0) (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
                (return)))
            (call $send_reply (local.get $out) (i32.const 200) (i32.const 240) (i32.const 7))
            (return)))

        ;; /query ('q' == 113)
        (if (i32.eq (local.get $tag) (i32.const 113))
          (then
            (call $sql_query (i32.const 144) (i32.const 38) (i32.const 0) (i32.const 0) (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.load (i32.const 520)) (i32.load (i32.const 524)))
                (return)))
            (call $send_reply (local.get $out) (i32.const 200) (i32.load (i32.const 524)) (i32.load (i32.const 528)))
            (return)))

        ;; /kv ('k' == 107)
        (if (i32.eq (local.get $tag) (i32.const 107))
          (then
            (call $kv_get (i32.const 128) (i32.const 8) (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
                (return)))
            (if (i32.eqz (i32.load8_u (i32.const 516)))
              (then
                (call $send_reply (local.get $out) (i32.const 404) (i32.const 240) (i32.const 7))
                (return)))
            (call $send_reply (local.get $out) (i32.const 200) (i32.load (i32.const 520)) (i32.load (i32.const 524)))
            (return)))

        ;; /denied-kv ('d' == 100)
        (if (i32.eq (local.get $tag) (i32.const 100))
          (then
            (call $kv_get (i32.const 192) (i32.const 10) (i32.const 512))
            (if (i32.and (i32.eq (i32.load8_u (i32.const 512)) (i32.const 1))
                         (i32.eq (i32.load8_u (i32.const 516)) (i32.const 1)))
              (then
                (call $send_reply (local.get $out) (i32.const 403) (i32.load (i32.const 520)) (i32.load (i32.const 524)))
                (return)))
            (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
            (return)))

        ;; /leak-tx ('l' == 108)
        (if (i32.eq (local.get $tag) (i32.const 108))
          (then
            (call $sql_tx_begin (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
                (return)))
            (local.set $tx_id (i64.load (i32.const 520)))
            (call $sql_tx_exec (local.get $tx_id) (i32.const 32) (i32.const 38) (i32.const 224) (i32.const 1) (i32.const 512))
            (if (i32.load8_u (i32.const 512))
              (then
                (call $send_reply (local.get $out) (i32.const 500) (i32.const 240) (i32.const 7))
                (return)))
            (call $send_reply (local.get $out) (i32.const 200) (i32.const 208) (i32.const 6))
            (return)))

        (call $send_reply (local.get $out) (i32.const 404) (i32.const 240) (i32.const 7)))
    )
    "#
    .to_owned()
}

fn minimal_sql_only_wat() -> String {
    r#"
    (module
      (import "wasi:http/types@0.2.12" "[resource-drop]incoming-request" (func $drop_req (param i32)))
      (import "wasi:http/types@0.2.12" "[constructor]fields" (func $new_fields (result i32)))
      (import "wasi:http/types@0.2.12" "[constructor]outgoing-response" (func $new_resp (param i32) (result i32)))
      (import "wasi:http/types@0.2.12" "[static]response-outparam.set" (func $outparam_set (param i32 i32 i32 i32 i64 i32 i32 i32 i32)))
      (import "calcit:db/sql@0.1.0" "tx-begin" (func $sql_tx_begin (param i32)))
      (memory (export "memory") 1)
      (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) (i32.const 512))
      (func (export "wasi:http/incoming-handler@0.2.12#handle") (param $req i32) (param $out i32)
        (call $sql_tx_begin (i32.const 256))
        (call $drop_req (local.get $req))
        (call $outparam_set (local.get $out) (i32.const 0) (call $new_resp (call $new_fields)) (i32.const 0) (i64.const 0) (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0)))
    )
    "#
    .to_owned()
}

#[test]
fn packages_wasi_http_service_deterministically_and_validates_guards() {
    let core_module = wat::parse_str(service_wat_full()).expect("assemble HTTP+DB WAT module");
    let first = package_wasi_http_service(&core_module).expect("package HTTP+DB Component A");
    let second = package_wasi_http_service(&core_module).expect("package HTTP+DB Component B");
    assert_eq!(Sha256::digest(&first), Sha256::digest(&second));

    let engine = Engine::new(&Config::new()).expect("Wasmtime engine");
    let component = Component::new(&engine, &first).expect("valid HTTP+DB Component");
    let component_type = component.component_type();
    let imports = component_type
        .imports(&engine)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert!(imports.contains(&"calcit:db/sql@0.1.0"), "{imports:?}");
    assert!(imports.contains(&"calcit:db/kv@0.1.0"), "{imports:?}");
    assert!(imports.contains(&"wasi:http/types@0.2.12"), "{imports:?}");
    let exports = component_type
        .exports(&engine)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert_eq!(exports, vec!["wasi:http/incoming-handler@0.2.12"]);

    // Modules importing only `calcit:db/sql@0.1.0` must omit unused `calcit:db/kv@0.1.0`.
    let sql_only_core =
        wat::parse_str(minimal_sql_only_wat()).expect("assemble SQL-only WAT module");
    let sql_only_component =
        package_wasi_http_service(&sql_only_core).expect("package SQL-only HTTP Component");
    let sql_only = Component::new(&engine, &sql_only_component).expect("valid SQL-only Component");
    let sql_only_type = sql_only.component_type();
    let sql_only_imports = sql_only_type
        .imports(&engine)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert!(
        sql_only_imports.contains(&"calcit:db/sql@0.1.0"),
        "{sql_only_imports:?}"
    );
    assert!(
        !sql_only_imports.contains(&"calcit:db/kv@0.1.0"),
        "{sql_only_imports:?}"
    );

    // Reject Preview 1 imports cleanly.
    let preview1 = wat::parse_str(
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) (i32.const 512))
          (func (export "wasi:http/incoming-handler@0.2.12#handle") (param i32 i32))
        )
        "#,
    )
    .expect("assemble Preview 1 module");
    let p1_err = package_wasi_http_service(&preview1).expect_err("reject Preview 1 import");
    assert!(p1_err.contains("E_WASI_HTTP_PREVIEW1_IMPORT"), "{p1_err}");

    // Reject embedded component-type custom sections.
    let mut embedded = core_module.clone();
    CustomSection {
        name: "component-type:fake".into(),
        data: std::borrow::Cow::Borrowed(b"dummy"),
    }
    .append_to(&mut embedded);
    let emb_err =
        package_wasi_http_service(&embedded).expect_err("reject embedded component metadata");
    assert!(
        emb_err.contains("E_WASI_EMBEDDED_COMPONENT_TYPE"),
        "{emb_err}"
    );
}

fn db_component_contract_document() -> ComponentDocument {
    let declaration_struct = |name: &str, fields: Vec<(&str, Type)>| Declaration::Struct {
        id: format!("demo.db/{name}"),
        namespace: "demo.db".to_owned(),
        name: name.to_owned(),
        type_parameters: vec![],
        fields: fields
            .into_iter()
            .map(|(field_name, type_ir)| StructField {
                name: field_name.to_owned(),
                type_ir,
            })
            .collect(),
    };
    let declaration_enum = |name: &str, variants: Vec<EnumVariant>| Declaration::Enum {
        id: format!("demo.db/{name}"),
        namespace: "demo.db".to_owned(),
        name: name.to_owned(),
        type_parameters: vec![],
        variants,
    };
    let struct_type = |id: &str| Type::Struct {
        id: id.to_owned(),
        arguments: vec![],
    };
    let enum_type = |id: &str| Type::Enum {
        id: id.to_owned(),
        arguments: vec![],
    };
    let variant = |name: &str, payload: Vec<Type>| EnumVariant {
        name: name.to_owned(),
        payload,
    };

    let param = "demo.db/DbParam";
    let error = "demo.db/DbError";
    let exec_result = "demo.db/DbExecResult";
    let query_result = "demo.db/DbQueryResult";

    ComponentDocument {
        version: 4,
        package: "demo".to_owned(),
        package_version: "0.1.0".to_owned(),
        declarations: vec![
            declaration_enum(
                "DbParam",
                vec![
                    variant("bool-value", vec![Type::Bool]),
                    variant("bytes-value", vec![Type::Buffer]),
                    variant("float-value", vec![Type::Float64]),
                    variant("int-value", vec![Type::Int64]),
                    variant("null-value", vec![]),
                    variant("text-value", vec![Type::String]),
                ],
            ),
            declaration_enum(
                "DbError",
                vec![
                    variant("backend", vec![Type::String]),
                    variant("capability-denied", vec![Type::String]),
                    variant("constraint-violation", vec![Type::String]),
                    variant("decode-error", vec![Type::String]),
                    variant("invalid-query", vec![Type::String]),
                    variant("timeout", vec![Type::String]),
                    variant("tx-error", vec![Type::String]),
                    variant("unsupported", vec![Type::String]),
                ],
            ),
            declaration_struct(
                "DbExecResult",
                vec![
                    (
                        "last-insert-id",
                        Type::Option {
                            item: Box::new(Type::Int64),
                        },
                    ),
                    ("rows-affected", Type::Uint64),
                ],
            ),
            declaration_struct(
                "DbQueryResult",
                vec![
                    (
                        "columns",
                        Type::List {
                            item: Box::new(Type::String),
                        },
                    ),
                    ("rows-json", Type::String),
                ],
            ),
        ],
        definitions: vec![
            ComponentDefinition {
                id: "demo.db/sql-query".to_owned(),
                namespace: "demo.db".to_owned(),
                name: "sql-query".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                direction: ComponentDirection::Import,
                invocation: Some(ComponentInvocation::Sync),
                module: Some("calcit:db/sql@0.1.0".to_owned()),
                symbol: "query".to_owned(),
                signature: Some(FunctionSignature {
                    parameters: vec![
                        Parameter {
                            position: 0,
                            type_ir: Type::String,
                        },
                        Parameter {
                            position: 1,
                            type_ir: Type::List {
                                item: Box::new(enum_type(param)),
                            },
                        },
                    ],
                    result: Type::Result {
                        ok: Box::new(struct_type(query_result)),
                        error: Box::new(enum_type(error)),
                    },
                }),
                status: DefinitionStatus::Supported,
                diagnostic_codes: vec![],
            },
            ComponentDefinition {
                id: "demo.db/sql-exec".to_owned(),
                namespace: "demo.db".to_owned(),
                name: "sql-exec".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                direction: ComponentDirection::Import,
                invocation: Some(ComponentInvocation::Sync),
                module: Some("calcit:db/sql@0.1.0".to_owned()),
                symbol: "exec".to_owned(),
                signature: Some(FunctionSignature {
                    parameters: vec![
                        Parameter {
                            position: 0,
                            type_ir: Type::String,
                        },
                        Parameter {
                            position: 1,
                            type_ir: Type::List {
                                item: Box::new(enum_type(param)),
                            },
                        },
                    ],
                    result: Type::Result {
                        ok: Box::new(struct_type(exec_result)),
                        error: Box::new(enum_type(error)),
                    },
                }),
                status: DefinitionStatus::Supported,
                diagnostic_codes: vec![],
            },
            ComponentDefinition {
                id: "demo.db/kv-get".to_owned(),
                namespace: "demo.db".to_owned(),
                name: "kv-get".to_owned(),
                doc: String::new(),
                logical_schema: String::new(),
                direction: ComponentDirection::Import,
                invocation: Some(ComponentInvocation::Sync),
                module: Some("calcit:db/kv@0.1.0".to_owned()),
                symbol: "get".to_owned(),
                signature: Some(FunctionSignature {
                    parameters: vec![Parameter {
                        position: 0,
                        type_ir: Type::String,
                    }],
                    result: Type::Result {
                        ok: Box::new(Type::Option {
                            item: Box::new(Type::Buffer),
                        }),
                        error: Box::new(enum_type(error)),
                    },
                }),
                status: DefinitionStatus::Supported,
                diagnostic_codes: vec![],
            },
        ],
    }
}

#[test]
fn generate_and_check_emit_managed_db_artifacts_and_detect_drift() {
    let dir = TempDir::new().expect("temp dir");
    let contract = calcit_bindgen::InterfaceContract::Component(db_component_contract_document());

    let core_wat = r#"
    (module
      (import "calcit:db/sql@0.1.0" "query" (func (param i32 i32 i32 i32 i32)))
      (import "calcit:db/sql@0.1.0" "exec" (func (param i32 i32 i32 i32 i32)))
      (import "calcit:db/kv@0.1.0" "get" (func (param i32 i32 i32)))
      (memory (export "memory") 1)
      (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) (i32.const 512))
    )
    "#;
    let core_bytes = wat::parse_str(core_wat).expect("assemble component core module");
    let core_path = dir.path().join("program.wasm");
    fs::write(&core_path, core_bytes).expect("write core wasm");

    let out = dir.path().join("generated");
    let manifest = generate_contract_directory(&contract, Some(&core_path), &out, &[])
        .expect("generate DB Component artifacts");
    assert!(out.join(WIT_DB_FILE).exists());
    assert!(out.join(CALCIT_DB_BINDINGS_FILE).exists());
    assert!(out.join(WASMTIME_DB_ADAPTER_FILE).exists());
    assert!(out.join(MANIFEST_FILE).exists());
    assert!(manifest.files.iter().any(|f| f.path == WIT_DB_FILE));
    assert!(
        manifest
            .files
            .iter()
            .any(|f| f.path == CALCIT_DB_BINDINGS_FILE)
    );
    assert!(
        manifest
            .files
            .iter()
            .any(|f| f.path == WASMTIME_DB_ADAPTER_FILE)
    );

    let report = check_contract_directory(&contract, Some(&core_path), &out, &[])
        .expect("check generated artifacts");
    assert!(report.current, "issues: {:?}", report.issues);

    fs::write(out.join(WIT_DB_FILE), b"// drifted\n").expect("modify wit/db.wit");
    let stale = check_contract_directory(&contract, Some(&core_path), &out, &[])
        .expect("check modified artifact");
    assert!(!stale.current);
    assert!(stale.issues.iter().any(|i| i.path == WIT_DB_FILE));
}

#[cfg(feature = "wasmtime-db")]
#[test]
fn sqlite_and_kv_backends_enforce_default_deny_bounds_and_transactions() {
    let mut sql = SqliteSqlBackend::in_memory().expect("open sqlite in memory");
    let mut kv = InMemoryKvBackend::new();

    // Default-deny policy rejects SQL and KV operations.
    let denied = DbConfig::default();
    assert!(matches!(
        sql.exec(&denied, "CREATE TABLE t (id INTEGER PRIMARY KEY)", &[]),
        Err(DbError::CapabilityDenied(_))
    ));
    assert!(matches!(
        sql.query(&denied, "SELECT 1", &[]),
        Err(DbError::CapabilityDenied(_))
    ));
    assert!(matches!(
        kv.set(&denied, "k", b"v", None),
        Err(DbError::CapabilityDenied(_))
    ));

    // DDL creates table, write inserts parameterized data (blocking SQL injection).
    let ddl_cfg = DbConfig::default().allow_sql_ddl();
    sql.exec(
        &ddl_cfg,
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL)",
        &[],
    )
    .expect("create table");

    let rw_cfg = DbConfig::default().allow_sql_read().allow_sql_write();
    let injected = "alice'); DROP TABLE users; --".to_owned();
    let inserted = sql
        .exec(
            &rw_cfg,
            "INSERT INTO users (name) VALUES (?1)",
            &[DbParam::TextValue(injected.clone())],
        )
        .expect("parameterized insert safely stores special characters");
    assert_eq!(inserted.rows_affected, 1);
    assert_eq!(inserted.last_insert_id, Some(1));

    let rows = sql
        .query(&rw_cfg, "SELECT id, name FROM users", &[])
        .expect("query users");
    let parsed: serde_json::Value = serde_json::from_str(&rows.rows_json).expect("valid JSON");
    assert_eq!(parsed[0]["name"], injected);

    // Duplicate UNIQUE insert maps to ConstraintViolation.
    assert!(matches!(
        sql.exec(
            &rw_cfg,
            "INSERT INTO users (name) VALUES (?1)",
            &[DbParam::TextValue(injected)],
        ),
        Err(DbError::ConstraintViolation(_))
    ));

    // Non-INSERT DML (`UPDATE`/`DELETE`) must report `last_insert_id: None`, and non-RETURNING
    // writes passed to `query` must fail with `InvalidQuery`.
    let temp_insert = sql
        .exec(
            &rw_cfg,
            "INSERT INTO users (name) VALUES (?1)",
            &[DbParam::TextValue("temp-user".to_owned())],
        )
        .expect("insert temp-user");
    let temp_id = temp_insert.last_insert_id.expect("temp row id");
    let updated = sql
        .exec(
            &rw_cfg,
            "UPDATE users SET name = ?1 WHERE id = ?2",
            &[
                DbParam::TextValue("temp-user-updated".to_owned()),
                DbParam::IntValue(temp_id),
            ],
        )
        .expect("update temp-user");
    assert_eq!(updated.rows_affected, 1);
    assert_eq!(updated.last_insert_id, None);
    let deleted = sql
        .exec(
            &rw_cfg,
            "DELETE FROM users WHERE id = ?1",
            &[DbParam::IntValue(temp_id)],
        )
        .expect("delete temp-user");
    assert_eq!(deleted.rows_affected, 1);
    assert_eq!(deleted.last_insert_id, None);
    assert!(matches!(
        sql.query(&rw_cfg, "DELETE FROM users WHERE id = -1", &[]),
        Err(DbError::InvalidQuery(_))
    ));

    // Multi-statement SQL, explicit BEGIN/COMMIT, and CTE writes without write cap must fail.
    assert!(matches!(
        sql.query(&rw_cfg, "SELECT 1; SELECT 2", &[]),
        Err(DbError::InvalidQuery(_))
    ));
    assert!(matches!(
        sql.exec(&rw_cfg, "BEGIN TRANSACTION", &[]),
        Err(DbError::InvalidQuery(_))
    ));
    let ro_cfg = DbConfig::default().allow_sql_read();
    assert!(matches!(
        sql.query(
            &ro_cfg,
            "WITH cte AS (SELECT 'x' AS v) INSERT INTO users (name) SELECT v FROM cte",
            &[],
        ),
        Err(DbError::CapabilityDenied(_))
    ));

    // Read-only cannot call exec on write statement even if disguised.
    assert!(matches!(
        sql.exec(&ro_cfg, "INSERT INTO users (name) VALUES ('bob')", &[]),
        Err(DbError::CapabilityDenied(_))
    ));

    // Transactions: commit persists, rollback discards, leaked tx rolls back automatically.
    let tx1 = sql.tx_begin(&rw_cfg).expect("begin tx1");
    sql.tx_exec(
        &rw_cfg,
        tx1,
        "INSERT INTO users (name) VALUES (?1)",
        &[DbParam::TextValue("committed".to_owned())],
    )
    .expect("tx1 insert");
    sql.tx_commit(tx1).expect("commit tx1");

    let tx2 = sql.tx_begin(&rw_cfg).expect("begin tx2");
    sql.tx_exec(
        &rw_cfg,
        tx2,
        "INSERT INTO users (name) VALUES (?1)",
        &[DbParam::TextValue("rolled-back".to_owned())],
    )
    .expect("tx2 insert");
    sql.tx_rollback(tx2).expect("rollback tx2");

    let tx3 = sql.tx_begin(&rw_cfg).expect("begin tx3");
    sql.tx_exec(
        &rw_cfg,
        tx3,
        "INSERT INTO users (name) VALUES (?1)",
        &[DbParam::TextValue("leaked".to_owned())],
    )
    .expect("tx3 insert");
    assert_eq!(sql.rollback_all_open_transactions().unwrap(), 1);

    let after = sql
        .query(&ro_cfg, "SELECT name FROM users ORDER BY id", &[])
        .expect("query after tx");
    assert!(after.rows_json.contains("committed"));
    assert!(!after.rows_json.contains("rolled-back"));
    assert!(!after.rows_json.contains("leaked"));

    // Bounded max_rows & max_result_bytes.
    let bounded_rows = DbConfig::default().allow_sql_read().max_rows(1);
    assert!(matches!(
        sql.query(&bounded_rows, "SELECT id FROM users", &[]),
        Err(DbError::InvalidQuery(_))
    ));
    let bounded_bytes = DbConfig::default().allow_sql_read().max_result_bytes(16);
    assert!(matches!(
        sql.query(&bounded_bytes, "SELECT id, name FROM users", &[]),
        Err(DbError::InvalidQuery(_))
    ));

    // KV prefix capabilities and TTL expiration.
    let kv_cfg = DbConfig::default().allow_kv_prefix("sess:");
    assert!(matches!(
        kv.set(&kv_cfg, "other:k", b"v", None),
        Err(DbError::CapabilityDenied(_))
    ));
    kv.set(&kv_cfg, "sess:1", b"val", Some(10))
        .expect("set sess:1");
    assert_eq!(kv.get(&kv_cfg, "sess:1").unwrap(), Some(b"val".to_vec()));
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(kv.get(&kv_cfg, "sess:1").unwrap(), None);
}

#[cfg(feature = "wasmtime-db")]
#[tokio::test]
async fn wasmtime_http_db_service_runs_end_to_end_under_8mib_cap() {
    let core_module = wat::parse_str(service_wat_full()).expect("assemble HTTP+DB WAT module");
    let component_bytes = package_wasi_http_service(&core_module).expect("package Component");

    let mut sql = SqliteSqlBackend::in_memory().expect("open sqlite in memory");
    let setup_cfg = DbConfig::default().allow_sql_ddl();
    sql.exec(
        &setup_cfg,
        "CREATE TABLE items (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        &[],
    )
    .expect("create schema before dropping DDL permission");

    // Host policy grants SQL read+write and `app:` KV prefix, while keeping DDL denied.
    let runtime_cfg = DbConfig::default()
        .allow_sql_read()
        .allow_sql_write()
        .allow_kv_prefix("app:");
    let db_state = WasmtimeDbState::new(
        runtime_cfg,
        Box::new(sql),
        Box::new(InMemoryKvBackend::new()),
    );

    let mut runner = HttpDbServiceRunner::new(&component_bytes, db_state, Some(8 * 1024 * 1024))
        .await
        .expect("create HttpDbServiceRunner");

    // 1. /init inserts SQL row and writes KV key.
    let init = runner
        .handle_request(http::Method::GET, "/init", vec![], vec![])
        .await
        .expect("handle /init");
    assert_eq!(init.status, 200);
    assert_eq!(init.body, b"exec ok");
    assert_eq!(init.leaked_tx_rolled_back, 0);

    // 2. /query reads typed JSON rows back via SQLite.
    let queried = runner
        .handle_request(http::Method::GET, "/query", vec![], vec![])
        .await
        .expect("handle /query");
    assert_eq!(queried.status, 200);
    let rows: serde_json::Value = serde_json::from_slice(&queried.body).expect("JSON rows");
    assert_eq!(rows, serde_json::json!([{ "id": 1, "name": "alpha" }]));

    // 3. /kv reads persisted KV key back.
    let kv_reply = runner
        .handle_request(http::Method::GET, "/kv", vec![], vec![])
        .await
        .expect("handle /kv");
    assert_eq!(kv_reply.status, 200);
    assert_eq!(kv_reply.body, b"alpha");

    // 4. /denied-kv triggers default-deny CapabilityDenied -> 403.
    let denied = runner
        .handle_request(http::Method::GET, "/denied-kv", vec![], vec![])
        .await
        .expect("handle /denied-kv");
    assert_eq!(denied.status, 403);
    assert!(
        String::from_utf8_lossy(&denied.body).contains("secret:key"),
        "denied body: {:?}",
        String::from_utf8_lossy(&denied.body)
    );

    // 5. /leak-tx starts a transaction, inserts without committing, and returns -> auto-rolled-back.
    let leaked = runner
        .handle_request(http::Method::GET, "/leak-tx", vec![], vec![])
        .await
        .expect("handle /leak-tx");
    assert_eq!(leaked.status, 200);
    assert_eq!(leaked.leaked_tx_rolled_back, 1);

    let queried_after = runner
        .handle_request(http::Method::GET, "/query", vec![], vec![])
        .await
        .expect("handle /query after leaked tx");
    assert_eq!(queried_after.status, 200);
    let rows_after: serde_json::Value =
        serde_json::from_slice(&queried_after.body).expect("JSON rows after leaked tx");
    assert_eq!(
        rows_after,
        serde_json::json!([{ "id": 1, "name": "alpha" }])
    );
    assert!(
        runner.peak_memory_bytes() <= 8 * 1024 * 1024,
        "peak memory {} exceeded 8 MiB limit",
        runner.peak_memory_bytes()
    );
}
