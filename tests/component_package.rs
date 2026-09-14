use std::fs;
use std::process::Command;

use calcit_bindgen::{
    COMPONENT_FILE, ContractKind, GenerationBackend, InterfaceContract, WIT_BINDINGS_FILE,
    check_contract_directory, generate_contract_directory, load_contract,
};
use tempfile::TempDir;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

const CONTRACT: &str = "tests/fixtures/component-interface.cirru";

fn core_module() -> Vec<u8> {
    core_module_with_offset(1)
}

fn core_module_with_offset(offset: i32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"
        (module
          (import "host" "add-one" (func $host-add-one (param f64) (result f64)))
          (import "host" "echo" (func $host-echo (param i32 i32 i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "cabi_realloc")
            (param $old i32) (param $old-size i32) (param $align i32) (param $new-size i32)
            (result i32)
            (local $result i32)
            global.get $heap
            local.tee $result
            local.get $new-size
            i32.add
            global.set $heap
            local.get $result)
          (func (export "add-one") (param $value f64) (result f64)
            local.get $value
            f64.const {offset}
            f64.add)
          (func (export "call-host-add-one") (param $value f64) (result f64)
            local.get $value
            call $host-add-one)
          (func (export "echo-text") (param $pointer i32) (param $length i32) (result i32)
            i32.const 8
            local.get $pointer
            i32.store
            i32.const 12
            local.get $length
            i32.store
            i32.const 8)
          (func (export "call-host-echo") (param $pointer i32) (param $length i32) (result i32)
            local.get $pointer
            local.get $length
            i32.const 8
            call $host-echo
            i32.const 8))
        "#,
    ))
    .expect("compile the Canonical ABI fixture")
}

#[test]
fn check_reports_stale_when_core_module_changes() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let contract = component_contract();
    generate_contract_directory(&contract, Some(&core), &output, &[]).expect("package Component");

    let rebuilt = temporary.path().join("rebuilt.wasm");
    fs::write(&rebuilt, core_module_with_offset(2)).expect("write rebuilt core module");
    let report = check_contract_directory(&contract, Some(&rebuilt), &output, &[])
        .expect("check against rebuilt core module");
    assert!(!report.current);
}

fn component_contract() -> InterfaceContract {
    load_contract(CONTRACT).expect("load Component contract")
}

#[test]
fn packages_checks_and_runs_number_and_string_component() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let contract = component_contract();
    let manifest = generate_contract_directory(&contract, Some(&core), &output, &[])
        .expect("package Component");

    assert_eq!(manifest.contract_kind, ContractKind::Component);
    assert_eq!(manifest.backends, vec![GenerationBackend::Wit]);
    assert!(manifest.core_module_digest.is_some());
    assert!(output.join(WIT_BINDINGS_FILE).is_file());
    assert!(output.join(COMPONENT_FILE).is_file());
    assert!(
        check_contract_directory(&contract, Some(&core), &output, &[])
            .expect("check generated Component")
            .current
    );

    let engine = Engine::default();
    let component = Component::from_file(&engine, output.join(COMPONENT_FILE))
        .expect("load generated Component in Wasmtime");
    let mut linker = Linker::new(&engine);
    let mut root = linker.root();
    let mut host = root.instance("host").expect("define host instance");
    host.func_wrap("add-one", |_store, (value,): (f64,)| Ok((value + 2.0,)))
        .expect("define host add-one");
    host.func_wrap("echo", |_store, (value,): (String,)| Ok((value,)))
        .expect("define host echo");
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("instantiate generated Component");

    let add_one = instance
        .get_typed_func::<(f64,), (f64,)>(&mut store, "add-one")
        .expect("typed add-one export");
    assert_eq!(
        add_one.call(&mut store, (41.0,)).expect("call add-one"),
        (42.0,)
    );
    add_one
        .post_return(&mut store)
        .expect("finish add-one call");

    let call_host = instance
        .get_typed_func::<(f64,), (f64,)>(&mut store, "call-host-add-one")
        .expect("typed call-host-add-one export");
    assert_eq!(
        call_host.call(&mut store, (40.0,)).expect("call host"),
        (42.0,)
    );
    call_host.post_return(&mut store).expect("finish host call");

    let echo = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "echo-text")
        .expect("typed echo-text export");
    assert_eq!(
        echo.call(&mut store, ("Calcit".to_owned(),))
            .expect("call echo-text"),
        ("Calcit".to_owned(),)
    );
    echo.post_return(&mut store).expect("finish echo-text call");

    let call_host_echo = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "call-host-echo")
        .expect("typed call-host-echo export");
    assert_eq!(
        call_host_echo
            .call(&mut store, ("Agent".to_owned(),))
            .expect("call host echo"),
        ("Agent".to_owned(),)
    );
    call_host_echo
        .post_return(&mut store)
        .expect("finish host echo call");

    if let Ok(path) = std::env::var("CALCIT_BINDGEN_COMPONENT_OUTPUT") {
        fs::copy(output.join(COMPONENT_FILE), path).expect("copy Component for toolchain smoke");
    }
}

#[test]
fn rejects_abi_mismatch_before_creating_output() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, wat::parse_str("(module)").expect("empty module"))
        .expect("write mismatched core module");
    let output = temporary.path().join("generated");
    let error = generate_contract_directory(&component_contract(), Some(&core), &output, &[])
        .expect_err("ABI mismatch must fail");
    assert!(
        error.contains("does not implement the Component contract"),
        "unexpected error: {error}"
    );
    assert!(!output.exists());
}

#[test]
fn rejects_binding_name_rewrites_before_creating_output() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let mut contract = component_contract();
    let InterfaceContract::Component(document) = &mut contract else {
        panic!("expected Component contract");
    };
    document.definitions[0].symbol = "add_one".to_owned();

    let error = generate_contract_directory(&contract, Some(&core), &output, &[])
        .expect_err("binding identity rewrite must fail");
    assert!(error.contains("refusing to rewrite binding identity"));
    assert!(!output.exists());
}

#[test]
fn component_cli_reuses_validate_generate_and_check() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let binary = env!("CARGO_BIN_EXE_calcit-bindgen");

    let validate = Command::new(binary)
        .args(["validate", CONTRACT])
        .output()
        .expect("run validate");
    assert!(validate.status.success());
    assert!(String::from_utf8_lossy(&validate.stdout).contains("valid component Interface IR v1"));

    let generate = Command::new(binary)
        .arg("generate")
        .arg(CONTRACT)
        .arg("--core-module")
        .arg(&core)
        .arg("--out")
        .arg(&output)
        .output()
        .expect("run generate");
    assert!(
        generate.status.success(),
        "{}",
        String::from_utf8_lossy(&generate.stderr)
    );

    let check = Command::new(binary)
        .arg("check")
        .arg(CONTRACT)
        .arg("--core-module")
        .arg(&core)
        .arg("--out")
        .arg(&output)
        .output()
        .expect("run check");
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(String::from_utf8_lossy(&check.stdout).contains("generated artifacts are current"));
}
