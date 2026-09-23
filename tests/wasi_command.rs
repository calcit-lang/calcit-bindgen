use std::fs;
use std::process::{Command, Stdio};

use calcit_bindgen::package_wasi_command;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

#[test]
fn packages_wasi_031_command_run_export() {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    let engine = Engine::new(&config).expect("create Wasmtime engine");
    let temporary = TempDir::new().expect("temporary output directory");

    for (result_tag, expected_exit) in [(0, 0), (1, 1)] {
        let core = wat::parse_str(format!(
            r#"
        (module
          (memory (export "memory") 1)
          (func (export "wasi:cli/run@0.3.1#run") (result i32)
            i32.const {result_tag}))
        "#,
        ))
        .expect("compile command core fixture");
        let bytes = package_wasi_command(&core).expect("package WASI command Component");
        assert_eq!(
            bytes,
            package_wasi_command(&core).expect("repeat deterministic packaging")
        );
        let component = Component::new(&engine, &bytes).expect("validate WASI command Component");
        assert_eq!(component.component_type().imports(&engine).count(), 0);
        assert!(
            component
                .component_type()
                .exports(&engine)
                .any(|(name, _)| name == "wasi:cli/run@0.3.1")
        );

        if let Some(wasmtime_cli) = std::env::var_os("WASMTIME_49_CLI") {
            let path = temporary.path().join(format!("command-{result_tag}.wasm"));
            fs::write(&path, &bytes).expect("write command Component");
            let output = Command::new(wasmtime_cli)
                .args(["run", "-S", "p3"])
                .arg(path)
                .output()
                .expect("run WASI 0.3.1 command with Wasmtime 49");
            assert_eq!(
                output.status.code(),
                Some(expected_exit),
                "unexpected Wasmtime result: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn runs_wasi_031_host_import() {
    let Some(wasmtime_cli) = std::env::var_os("WASMTIME_49_CLI") else {
        return;
    };
    let core = wat::parse_str(
        r#"
        (module
          (import "wasi:cli/exit@0.3.1" "exit-with-code" (func $exit (param i32)))
          (memory (export "memory") 1)
          (func (export "wasi:cli/run@0.3.1#run") (result i32)
            i32.const 7
            call $exit
            i32.const 0))
        "#,
    )
    .expect("compile WASI host import fixture");
    let bytes = package_wasi_command(&core).expect("package WASI host import");
    let temporary = TempDir::new().expect("temporary output directory");
    let path = temporary.path().join("host-import.wasm");
    fs::write(&path, bytes).expect("write command Component");
    let output = Command::new(wasmtime_cli)
        .args(["run", "-S", "p3"])
        .arg(path)
        .output()
        .expect("run WASI 0.3.1 host import with Wasmtime 49");
    assert_eq!(
        output.status.code(),
        Some(7),
        "unexpected Wasmtime result: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runs_wasi_031_large_stdout_stream() {
    let core = wat::parse_str(
        r#"
        (module
          (import "wasi:cli/stdout@0.3.1" "[stream-new-0]write-via-stream" (func $new (result i64)))
          (import "wasi:cli/stdout@0.3.1" "[stream-write-0]write-via-stream" (func $write (param i32 i32 i32) (result i32)))
          (import "wasi:cli/stdout@0.3.1" "[stream-drop-writable-0]write-via-stream" (func $drop-writable (param i32)))
          (import "wasi:cli/stdout@0.3.1" "[future-read-1]write-via-stream" (func $read-future (param i32 i32) (result i32)))
          (import "wasi:cli/stdout@0.3.1" "[future-drop-readable-1]write-via-stream" (func $drop-future (param i32)))
          (import "wasi:cli/stdout@0.3.1" "write-via-stream" (func $stdout (param i32) (result i32)))
          (import "[export]wasi:cli/run@0.3.1" "[task-return]run" (func $return-run (param i32)))
          (memory (export "memory") 32)
          (data (i32.const 16) "hello from stream\0a")
          (func (export "[async-lift-stackful]wasi:cli/run@0.3.1#run")
            (local $pair i64)
            (local $written i32)
            (local $future i32)
            (local $writer i32)
            (local $offset i32)
            (local $remaining i32)
            call $new
            local.set $pair
            local.get $pair
            i32.wrap_i64
            call $stdout
            local.set $future
            local.get $pair
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            local.set $writer
            i32.const 16
            local.set $offset
            i32.const 1048576
            local.set $remaining
            block $finished
              loop $writing
                local.get $remaining
                i32.eqz
                br_if $finished
                local.get $writer
                local.get $offset
                local.get $remaining
                call $write
                local.tee $written
                i32.const 15
                i32.and
                if unreachable end
                local.get $written
                i32.const 4
                i32.shr_u
                local.tee $written
                i32.eqz
                if unreachable end
                local.get $offset
                local.get $written
                i32.add
                local.set $offset
                local.get $remaining
                local.get $written
                i32.sub
                local.set $remaining
                br $writing
              end
            end
            local.get $writer
            call $drop-writable
            local.get $future
            i32.const 64
            call $read-future
            i32.eqz
            if
            else
              unreachable
            end
            i32.const 64
            i32.load8_u
            i32.eqz
            if
            else
              unreachable
            end
            local.get $future
            call $drop-future
            i32.const 0
            call $return-run))
        "#,
    )
    .expect("compile stdout stream fixture");
    let bytes = package_wasi_command(&core).expect("package stdout stream command");
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    config.wasm_component_model_more_async_builtins(true);
    let engine = Engine::new(&config).expect("create Wasmtime engine");
    Component::new(&engine, &bytes).expect("validate stdout stream Component");
    let Some(wasmtime_cli) = std::env::var_os("WASMTIME_49_CLI") else {
        return;
    };
    let temporary = TempDir::new().expect("temporary output directory");
    let path = temporary.path().join("stdout.wasm");
    fs::write(&path, bytes).expect("write command Component");
    let output = Command::new(&wasmtime_cli)
        .args([
            "run",
            "-S",
            "p3",
            "-W",
            "component-model-more-async-builtins=y",
            "-W",
            "component-model-async-stackful=y",
        ])
        .arg(path)
        .output()
        .expect("run stdout command with Wasmtime 49");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), 1048576);
    assert!(output.stdout.starts_with(b"hello from stream\n"));

    let mut closed_output = Command::new(wasmtime_cli)
        .args([
            "run",
            "-S",
            "p3",
            "-W",
            "component-model-more-async-builtins=y",
            "-W",
            "component-model-async-stackful=y",
        ])
        .arg(temporary.path().join("stdout.wasm"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn command with closed stdout");
    drop(closed_output.stdout.take());
    let closed = closed_output
        .wait_with_output()
        .expect("wait for closed stdout command");
    assert!(
        !closed.status.success(),
        "closed stdout must not silently succeed"
    );
}

#[test]
fn rejects_wasi_030_run_export_for_031_world() {
    let core = wat::parse_str(
        r#"(module (memory (export "memory") 1) (func (export "wasi:cli/run@0.3.0#run") (result i32) i32.const 0))"#,
    )
    .expect("compile old command core fixture");
    let error = package_wasi_command(&core).expect_err("old command export must not package");
    assert!(error.contains("wasi:cli/run@0.3.1"), "{error}");
}

#[test]
fn rejects_preview1_imports_in_wasi_command() {
    let core = wat::parse_str(
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "wasi:cli/run@0.3.1#run") (result i32)
            i32.const 0))
        "#,
    )
    .expect("compile legacy core fixture");
    let error = package_wasi_command(&core).expect_err("Preview 1 import must be rejected");
    assert!(
        error.starts_with("E_WASI_COMMAND_PREVIEW1_IMPORT:"),
        "{error}"
    );
    assert!(error.contains("fd_write"), "{error}");
}

#[test]
fn rejects_component_input_without_cli_specific_error() {
    let core = wat::parse_str(
        r#"(module (memory (export "memory") 1) (func (export "wasi:cli/run@0.3.1#run") (result i32) i32.const 0))"#,
    )
    .expect("compile command core fixture");
    let component = package_wasi_command(&core).expect("package command");
    let error = package_wasi_command(&component).expect_err("Component input must be rejected");
    assert!(
        error.contains("expected a core WebAssembly module"),
        "{error}"
    );
    assert!(!error.contains("--core-module"), "{error}");
}

#[test]
fn pinned_wasi_wit_hashes_match_documentation() {
    let readme = include_str!("../src/wasi_wit/README.md");
    for (name, content) in [
        (
            "cli.wit",
            include_bytes!("../src/wasi_wit/cli.wit").as_slice(),
        ),
        (
            "clocks.wit",
            include_bytes!("../src/wasi_wit/clocks.wit").as_slice(),
        ),
        (
            "filesystem.wit",
            include_bytes!("../src/wasi_wit/filesystem.wit").as_slice(),
        ),
        (
            "random.wit",
            include_bytes!("../src/wasi_wit/random.wit").as_slice(),
        ),
        (
            "sockets.wit",
            include_bytes!("../src/wasi_wit/sockets.wit").as_slice(),
        ),
    ] {
        let row_prefix = format!("| `{name}` | `");
        let row = readme
            .lines()
            .find(|line| line.starts_with(&row_prefix))
            .unwrap_or_else(|| panic!("missing documented SHA-256 for {name}"));
        let documented = row
            .strip_prefix(&row_prefix)
            .and_then(|suffix| suffix.split('`').next())
            .expect("SHA-256 table cell");
        let actual = format!("{:x}", Sha256::digest(content));
        assert_eq!(actual, documented, "WIT hash drifted for {name}");
    }
}
