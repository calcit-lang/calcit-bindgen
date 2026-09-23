use std::fs;
use std::process::Command;

use calcit_bindgen::package_wasi_command;
use tempfile::TempDir;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

#[test]
fn packages_wasi_03_command_run_export() {
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
          (func (export "wasi:cli/run@0.3.0#run") (result i32)
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
                .any(|(name, _)| name == "wasi:cli/run@0.3.0")
        );

        if let Some(wasmtime_cli) = std::env::var_os("WASMTIME_47_CLI") {
            let path = temporary.path().join(format!("command-{result_tag}.wasm"));
            fs::write(&path, &bytes).expect("write command Component");
            let output = Command::new(wasmtime_cli)
                .args(["run", "-S", "p3"])
                .arg(path)
                .output()
                .expect("run WASI 0.3 command with Wasmtime 47");
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
fn rejects_preview1_imports_in_wasi_command() {
    let core = wat::parse_str(
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "wasi:cli/run@0.3.0#run") (result i32)
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
