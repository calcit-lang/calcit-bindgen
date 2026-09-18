use std::fs;
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use calcit_bindgen::{
    COMPONENT_FILE, ComponentDefinition, ComponentDirection, ComponentDocument,
    ComponentInvocation, DefinitionStatus, FunctionSignature, InterfaceContract, Parameter, Type,
    generate_contract_directory,
};
use tempfile::TempDir;
use wasmtime::component::{Component, Linker};
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
    wat::parse_str(
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
        "#,
    )
    .expect("compile async Canonical ABI core fixture")
}

#[test]
fn packages_and_executes_async_export_and_import_with_wasmtime() {
    let temporary = TempDir::new().expect("temporary workspace");
    let core = temporary.path().join("program.wasm");
    fs::write(&core, core_module()).expect("write core module");
    let output = temporary.path().join("generated");
    generate_contract_directory(&contract(), Some(&core), &output, &[])
        .expect("package async Component");

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
