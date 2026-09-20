use std::fs;
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use calcit_bindgen::{
    COMPONENT_FILE, ComponentDefinition, ComponentDirection, ComponentDocument,
    ComponentInvocation, DefinitionStatus, FunctionSignature, InterfaceContract, MANIFEST_FILE,
    Parameter, Type, WIT_BINDINGS_FILE, check_contract_directory, generate_contract_directory,
    load_contract,
};
use tempfile::TempDir;
use wasmtime::component::{Component, Linker, StreamReader};
use wasmtime::{Config, Engine, Store};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

fn definition(
    name: &str,
    direction: ComponentDirection,
    module: Option<&str>,
    parameters: Vec<Type>,
    result: Type,
) -> ComponentDefinition {
    ComponentDefinition {
        id: format!("async.main/{name}"),
        namespace: "async.main".into(),
        name: name.into(),
        doc: String::new(),
        logical_schema: String::new(),
        direction,
        invocation: Some(ComponentInvocation::Async),
        module: module.map(str::to_owned),
        symbol: name.into(),
        signature: Some(FunctionSignature {
            parameters: parameters
                .into_iter()
                .enumerate()
                .map(|(position, type_ir)| Parameter { position, type_ir })
                .collect(),
            result,
        }),
        status: DefinitionStatus::Supported,
        diagnostic_codes: Vec::new(),
    }
}

fn contract() -> InterfaceContract {
    InterfaceContract::Component(ComponentDocument {
        version: 3,
        package: "async-smoke".into(),
        package_version: "0.0.0".into(),
        declarations: Vec::new(),
        definitions: vec![
            definition(
                "double",
                ComponentDirection::Import,
                Some("host"),
                vec![Type::Number],
                Type::Number,
            ),
            definition(
                "fetch",
                ComponentDirection::Export,
                None,
                Vec::new(),
                Type::Number,
            ),
            definition(
                "choose-result",
                ComponentDirection::Export,
                None,
                vec![Type::Bool],
                Type::Result {
                    ok: Box::new(Type::Number),
                    error: Box::new(Type::Number),
                },
            ),
            definition(
                "call-host-double",
                ComponentDirection::Export,
                None,
                vec![Type::Number],
                Type::Number,
            ),
        ],
    })
}

fn core_module() -> Vec<u8> {
    wat::parse_str(core_module_wat()).expect("compile async Canonical ABI core fixture")
}

fn core_module_wat() -> &'static str {
    r#"
        (module
          (import "host" "[async-lower]double"
            (func $double (param f64 i32) (result i32)))
          (import "[export]$root" "[task-return]fetch"
            (func $return-fetch (param f64)))
          (import "[export]$root" "[task-return]call-host-double"
            (func $return-call-host-double (param f64)))
          (import "[export]$root" "[task-return]choose-result"
            (func $return-choose-result (param i32 f64)))
          (import "$root" "[waitable-set-new]" (func $waitable-set-new (result i32)))
          (import "$root" "[waitable-set-wait]"
            (func $waitable-set-wait (param i32 i32) (result i32)))
          (import "$root" "[waitable-set-drop]" (func $waitable-set-drop (param i32)))
          (import "$root" "[waitable-join]" (func $waitable-join (param i32 i32)))
          (import "$root" "[subtask-drop]" (func $subtask-drop (param i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "cabi_realloc")
            (param $old i32) (param $old-size i32) (param $align i32) (param $new-size i32)
            (result i32)
            (local $result i32)
            global.get $heap
            local.get $align
            i32.const 1
            i32.sub
            i32.add
            local.get $align
            i32.const 1
            i32.sub
            i32.const -1
            i32.xor
            i32.and
            local.tee $result
            local.get $new-size
            i32.add
            global.set $heap
            local.get $result)
          (func (export "[async-lift-stackful]fetch")
            f64.const 42
            call $return-fetch)
          (func (export "[async-lift-stackful]choose-result") (param $ok i32)
            local.get $ok
            if
              i32.const 0
              f64.const 7
              call $return-choose-result
            else
              i32.const 1
              f64.const 9
              call $return-choose-result
            end)
          (func (export "[async-lift-stackful]call-host-double") (param $value f64)
            (local $status i32)
            (local $subtask i32)
            (local $waitable-set i32)
            local.get $value
            i32.const 0
            call $double
            local.tee $status
            i32.const 15
            i32.and
            i32.const 2
            i32.ne
            if
              local.get $status
              i32.const 4
              i32.shr_u
              local.set $subtask
              call $waitable-set-new
              local.set $waitable-set
              local.get $subtask
              local.get $waitable-set
              call $waitable-join
              block $completed
                loop $waiting
                  local.get $waitable-set
                  i32.const 16
                  call $waitable-set-wait
                  drop
                  i32.const 16
                  i32.load
                  local.get $subtask
                  i32.ne
                  if unreachable end
                  i32.const 20
                  i32.load
                  i32.const 2
                  i32.eq
                  br_if $completed
                  br $waiting
                end
              end
              local.get $subtask
              i32.const 0
              call $waitable-join
              local.get $subtask
              call $subtask-drop
              local.get $waitable-set
              call $waitable-set-drop
            end
            i32.const 0
            f64.load
            call $return-call-host-double))
        "#
}

fn core_module_with_broken_task_return_signature() -> Vec<u8> {
    wat::parse_str(
        core_module_wat()
            .replace(
                "(func $return-fetch (param f64))",
                "(func $return-fetch (param i32))",
            )
            .replace("f64.const 42", "i32.const 42"),
    )
    .expect("compile mismatched async Canonical ABI core fixture")
}

fn core_module_with_missing_task_return_symbol() -> Vec<u8> {
    wat::parse_str(core_module_wat().replace("[task-return]fetch", "[task-return]fetch-missing"))
        .expect("compile async Canonical ABI core fixture with a missing lifecycle symbol")
}

fn readable_byte_stream_contract() -> InterfaceContract {
    InterfaceContract::Component(ComponentDocument {
        version: 4,
        package: "stream-smoke".into(),
        package_version: "0.0.0".into(),
        declarations: Vec::new(),
        definitions: vec![definition(
            "consume",
            ComponentDirection::Export,
            None,
            vec![Type::ReadableByteStream],
            Type::Unit,
        )],
    })
}

fn readable_byte_stream_core_module() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
          (import "[export]$root" "[task-return]consume"
            (func $return-consume))
          (import "[export]$root" "[stream-read-0]consume"
            (func $stream-read (param i32 i32 i32) (result i32)))
          (import "[export]$root" "[stream-cancel-read-0]consume"
            (func $stream-cancel-read (param i32) (result i32)))
          (import "[export]$root" "[stream-drop-readable-0]consume"
            (func $stream-drop-readable (param i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "cabi_realloc")
            (param $old i32) (param $old-size i32) (param $align i32) (param $new-size i32)
            (result i32)
            (local $result i32)
            global.get $heap
            local.get $align
            i32.const 1
            i32.sub
            i32.add
            local.get $align
            i32.const 1
            i32.sub
            i32.const -1
            i32.xor
            i32.and
            local.tee $result
            local.get $new-size
            i32.add
            global.set $heap
            local.get $result)
          (func (export "[async-lift-stackful]consume") (param $stream i32)
            local.get $stream
            call $stream-drop-readable
            call $return-consume))
        "#,
    )
    .expect("compile readable byte-stream Canonical ABI core fixture")
}

#[test]
fn packages_and_executes_async_export_and_import_with_wasmtime() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let manifest = generate_contract_directory(&contract(), Some(&core), &output, &[])
        .expect("package async Component");
    let lifecycle = manifest
        .lifecycle_surface
        .expect("async Component manifest should record the core lifecycle surface");
    assert!(lifecycle.imports.iter().any(|import| {
        import.module == "host"
            && import.name == "[async-lower]double"
            && import.signature == "(func (param f64 i32) (result i32))"
    }));
    assert!(
        lifecycle.imports.iter().any(|import| {
            import.module == "[export]$root" && import.name == "[task-return]fetch"
        })
    );
    assert!(lifecycle.exports.iter().any(|export| {
        export.name == "[async-lift-stackful]call-host-double"
            && export.signature == "(func (param f64))"
    }));

    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    let engine = Engine::new(&config).expect("create async Component engine");
    let component = Component::from_file(&engine, output.join(COMPONENT_FILE))
        .expect("load generated async Component");
    let mut linker = Linker::new(&engine);
    let host_poll_count = Arc::new(AtomicUsize::new(0));
    let host_poll_count_for_call = Arc::clone(&host_poll_count);
    linker
        .instance("host")
        .expect("define host instance")
        .func_wrap_concurrent("double", move |_accessor, (value,): (f64,)| {
            let host_poll_count = Arc::clone(&host_poll_count_for_call);
            Box::pin(async move {
                let mut first_poll = true;
                poll_fn(|context| {
                    host_poll_count.fetch_add(1, Ordering::SeqCst);
                    if first_poll {
                        first_poll = false;
                        context.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        Poll::Ready(())
                    }
                })
                .await;
                Ok((value * 2.0,))
            })
        })
        .expect("define concurrent host double");
    let mut store = Store::new(&engine, ());

    block_on(async {
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("instantiate async Component");
        let fetch = instance
            .get_typed_func::<(), (f64,)>(&mut store, "fetch")
            .expect("typed async fetch export");
        assert_eq!(
            fetch
                .call_async(&mut store, ())
                .await
                .expect("call async fetch"),
            (42.0,)
        );

        let choose_result = instance
            .get_typed_func::<(bool,), (Result<f64, f64>,)>(&mut store, "choose-result")
            .expect("typed async Result export");
        assert_eq!(
            choose_result
                .call_async(&mut store, (true,))
                .await
                .expect("call async Result success"),
            (Ok(7.0),)
        );
        assert_eq!(
            choose_result
                .call_async(&mut store, (false,))
                .await
                .expect("call async Result error"),
            (Err(9.0),)
        );

        let call_host_double = instance
            .get_typed_func::<(f64,), (f64,)>(&mut store, "call-host-double")
            .expect("typed async host-call export");
        assert_eq!(
            call_host_double
                .call_async(&mut store, (21.0,))
                .await
                .expect("call async host import"),
            (42.0,)
        );
    });
    assert!(host_poll_count.load(Ordering::SeqCst) >= 2);
}

#[test]
fn lifecycle_symbol_or_signature_drift_fails_before_replacing_managed_output() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write valid core module");
    let output = temporary.path().join("generated");
    generate_contract_directory(&contract(), Some(&core), &output, &[])
        .expect("package valid async Component");
    let manifest_before = fs::read(output.join(MANIFEST_FILE)).expect("read valid manifest");

    for (kind, broken_core) in [
        ("signature", core_module_with_broken_task_return_signature()),
        ("symbol", core_module_with_missing_task_return_symbol()),
    ] {
        fs::write(&core, broken_core).unwrap_or_else(|error| {
            panic!("write core module with lifecycle {kind} drift: {error}")
        });
        let generate_error = generate_contract_directory(&contract(), Some(&core), &output, &[])
            .err()
            .unwrap_or_else(|| panic!("generation must reject lifecycle {kind} drift"));
        assert!(
            generate_error.contains("Canonical ABI"),
            "unexpected generation error for {kind} drift: {generate_error}"
        );
        let check_error = check_contract_directory(&contract(), Some(&core), &output, &[])
            .err()
            .unwrap_or_else(|| panic!("check must reject lifecycle {kind} drift"));
        assert!(
            check_error.contains("Canonical ABI"),
            "unexpected check error for {kind} drift: {check_error}"
        );
        assert_eq!(
            fs::read(output.join(MANIFEST_FILE)).expect("read preserved manifest"),
            manifest_before,
            "failed generation or check replaced the managed output after {kind} drift"
        );
    }
}

#[test]
fn packages_readable_byte_stream_with_canonical_abi_builtins() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, readable_byte_stream_core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    let manifest =
        generate_contract_directory(&readable_byte_stream_contract(), Some(&core), &output, &[])
            .expect("package readable byte-stream Component");
    let lifecycle = manifest
        .lifecycle_surface
        .expect("stream Component manifest should record the core lifecycle surface");
    assert!(lifecycle.imports.iter().any(|import| {
        import.module == "[export]$root" && import.name == "[stream-drop-readable-0]consume"
    }));

    let wit = fs::read_to_string(output.join(WIT_BINDINGS_FILE)).expect("read generated WIT");
    assert!(wit.contains("export consume: async func(arg0: stream<u8>);"));

    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    config.wasm_component_model_more_async_builtins(true);
    let engine = Engine::new(&config).expect("create async Component engine");
    let component = Component::from_file(&engine, output.join(COMPONENT_FILE))
        .expect("load readable byte-stream Component");
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    block_on(async {
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("instantiate readable byte-stream Component");
        let consume = instance
            .get_typed_func::<(StreamReader<u8>,), ()>(&mut store, "consume")
            .expect("typed readable byte-stream export");
        let reader =
            StreamReader::new(&mut store, vec![1_u8, 2, 3]).expect("create host byte stream");
        store
            .run_concurrent(async |accessor| consume.call_concurrent(accessor, (reader,)).await)
            .await
            .expect("run concurrent guest call")
            .expect("consume readable byte stream");
    });
    store.assert_concurrent_state_empty();
}

#[test]
#[ignore = "requires the Calcit 0.18.0 lifecycle fixtures"]
fn packages_real_calcit_stackless_lifecycle_surface() {
    let fixtures = [
        (
            "CALCIT_BINDGEN_ASYNC_EXPORT_CONTRACT",
            "CALCIT_BINDGEN_ASYNC_EXPORT_CORE",
            &["[async-lift-stackful]load-text", "cabi_post_load-wide-sync"][..],
            &["[task-return]load-text"][..],
        ),
        (
            "CALCIT_BINDGEN_ASYNC_IMPORT_CONTRACT",
            "CALCIT_BINDGEN_ASYNC_IMPORT_CORE",
            &[
                "[async-lift]call-host-load",
                "[callback][async-lift]call-host-load",
            ][..],
            &["[async-lower][subtask-cancel]", "[task-cancel]"][..],
        ),
        (
            "CALCIT_BINDGEN_STREAM_CONTRACT",
            "CALCIT_BINDGEN_STREAM_CORE",
            &["[async-lift]consume", "[callback][async-lift]consume"][..],
            &[
                "[async-lower][stream-cancel-read-0]consume",
                "[stream-drop-readable-0]consume",
            ][..],
        ),
    ];

    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    config.wasm_component_model_more_async_builtins(true);
    let engine = Engine::new(&config).expect("create real lifecycle Component engine");

    for (contract_variable, core_variable, expected_exports, expected_imports) in fixtures {
        let contract_path = std::env::var(contract_variable)
            .unwrap_or_else(|_| panic!("{contract_variable} must name a generated contract"));
        let core_path = std::env::var(core_variable)
            .unwrap_or_else(|_| panic!("{core_variable} must name a generated core module"));
        let contract = load_contract(contract_path).expect("load real lifecycle contract");
        let temporary = TempDir::new().expect("temporary lifecycle package directory");
        let output = temporary.path().join("generated");
        let manifest = generate_contract_directory(
            &contract,
            Some(std::path::Path::new(&core_path)),
            &output,
            &[],
        )
        .expect("package real Calcit lifecycle core");
        let lifecycle = manifest
            .lifecycle_surface
            .expect("real lifecycle package should record its core surface");

        for name in expected_exports {
            assert!(
                lifecycle.exports.iter().any(|export| export.name == *name),
                "missing lifecycle export {name:?} in {core_variable}: {lifecycle:?}"
            );
        }
        for name in expected_imports {
            assert!(
                lifecycle.imports.iter().any(|import| import.name == *name),
                "missing lifecycle import {name:?} in {core_variable}: {lifecycle:?}"
            );
        }
        Component::from_file(&engine, output.join(COMPONENT_FILE))
            .expect("Wasmtime should accept the packaged real lifecycle Component");
    }
}
