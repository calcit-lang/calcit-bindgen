use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use calcit_bindgen::{
    COMPONENT_FILE, ContractKind, GenerationBackend, InterfaceContract, WIT_BINDINGS_FILE,
    check_contract_directory, generate_contract_directory, load_contract,
};
use tempfile::TempDir;
use wasmtime::component::{Component, ComponentType, Lift, Linker, Lower};
use wasmtime::{Engine, Store};

const CONTRACT: &str = "tests/fixtures/component-interface.cirru";

#[derive(Clone, Debug, PartialEq, ComponentType, Lift, Lower)]
#[component(record)]
struct ProfileStats {
    score: f64,
}

#[derive(Clone, Debug, PartialEq, ComponentType, Lift, Lower)]
#[component(record)]
struct Profile {
    active: bool,
    #[component(name = "maybe-name")]
    maybe_name: Option<String>,
    name: String,
    outcome: Result<Vec<f64>, String>,
    scores: Vec<f64>,
    stats: ProfileStats,
}

#[derive(Clone, Debug, PartialEq, ComponentType, Lift, Lower)]
#[component(record)]
struct NumericScalars {
    f32: f32,
    f64: f64,
    i16: i16,
    i32: i32,
    i64: i64,
    i8: i8,
    u16: u16,
    u32: u32,
    u64: u64,
    u8: u8,
}

#[derive(Clone, Debug, PartialEq, ComponentType, Lift, Lower)]
#[component(variant)]
enum Event {
    #[component(name = "idle")]
    Idle,
    #[component(name = "moved")]
    Moved((f64, f64)),
    #[component(name = "named")]
    Named(String),
    #[component(name = "profile")]
    Profile(Profile),
}

fn core_module() -> Vec<u8> {
    core_module_with_offset(1)
}

fn core_module_with_offset(offset: i32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"
        (module
          (import "host" "add-one" (func $host-add-one (param f64) (result f64)))
          (import "host" "bool-not" (func $host-bool-not (param i32) (result i32)))
          (import "host" "buffer" (func $host-buffer (param i32 i32 i32)))
          (import "host" "echo" (func $host-echo (param i32 i32 i32)))
          (import "host" "numbers" (func $host-numbers (param i32 i32 i32)))
          (import "host" "numeric-scalars" (func $host-numeric-scalars
            (param f32 f64 i32 i32 i64 i32 i32 i32 i64 i32 i32)))
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
          (func (export "add-one") (param $value f64) (result f64)
            local.get $value
            f64.const {offset}
            f64.add)
          (func (export "bool-not") (param $flag i32) (result i32)
            local.get $flag
            i32.eqz)
          (func (export "call-host-add-one") (param $value f64) (result f64)
            local.get $value
            call $host-add-one)
          (func (export "call-host-bool-not") (param $flag i32) (result i32)
            local.get $flag
            call $host-bool-not)
          (func (export "echo-buffer") (param $pointer i32) (param $length i32) (result i32)
            i32.const 8
            local.get $pointer
            i32.store
            i32.const 12
            local.get $length
            i32.store
            i32.const 8)
          (func (export "call-host-buffer") (param $pointer i32) (param $length i32) (result i32)
            local.get $pointer
            local.get $length
            i32.const 8
            call $host-buffer
            i32.const 8)
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
            i32.const 8)
          (func (export "call-host-numbers") (param $pointer i32) (param $length i32) (result i32)
            local.get $pointer
            local.get $length
            i32.const 8
            call $host-numbers
            i32.const 8)
          (func $echo-list (param $pointer i32) (param $length i32) (result i32)
            i32.const 8
            local.get $pointer
            i32.store
            i32.const 12
            local.get $length
            i32.store
            i32.const 8)
          (export "echo-bools" (func $echo-list))
          (export "echo-buffers" (func $echo-list))
          (export "echo-number-lists" (func $echo-list))
          (export "echo-numbers" (func $echo-list))
          (export "echo-texts" (func $echo-list))
          (func (export "choose-number")
            (param $flag i32) (param $yes f64) (param $no f64) (result f64)
            local.get $yes
            local.get $no
            local.get $flag
            select)
          (func (export "choose-buffer")
            (param $flag i32)
            (param $yes-pointer i32) (param $yes-length i32)
            (param $no-pointer i32) (param $no-length i32)
            (result i32)
            i32.const 8
            local.get $yes-pointer
            local.get $no-pointer
            local.get $flag
            select
            i32.store
            i32.const 12
            local.get $yes-length
            local.get $no-length
            local.get $flag
            select
            i32.store
            i32.const 8)
          (func (export "echo-numeric-scalars")
            (param $f32 f32) (param $f64 f64) (param $i16 i32) (param $i32 i32)
            (param $i64 i64) (param $i8 i32) (param $u16 i32) (param $u32 i32)
            (param $u64 i64) (param $u8 i32) (result i32)
            i32.const 64
            local.get $f32
            f32.store
            i32.const 72
            local.get $f64
            f64.store
            i32.const 80
            local.get $i16
            i32.store16
            i32.const 84
            local.get $i32
            i32.store
            i32.const 88
            local.get $i64
            i64.store
            i32.const 96
            local.get $i8
            i32.store8
            i32.const 98
            local.get $u16
            i32.store16
            i32.const 100
            local.get $u32
            i32.store
            i32.const 104
            local.get $u64
            i64.store
            i32.const 112
            local.get $u8
            i32.store8
            i32.const 64)
          (func (export "call-host-numeric-scalars")
            (param $f32 f32) (param $f64 f64) (param $i16 i32) (param $i32 i32)
            (param $i64 i64) (param $i8 i32) (param $u16 i32) (param $u32 i32)
            (param $u64 i64) (param $u8 i32) (result i32)
            local.get $f32
            local.get $f64
            local.get $i16
            local.get $i32
            local.get $i64
            local.get $i8
            local.get $u16
            local.get $u32
            local.get $u64
            local.get $u8
            i32.const 128
            call $host-numeric-scalars
            i32.const 128)
          (func (export "is-buffer") (param i32 i32) (result i32)
            i32.const 1))
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
fn packages_checks_and_runs_recursive_list_and_scalar_component() {
    let temporary = TempDir::new().expect("temporary workspace");
    let output = package_check_and_run(&component_contract(), &core_module(), temporary.path());

    if let Ok(path) = std::env::var("CALCIT_BINDGEN_COMPONENT_OUTPUT") {
        fs::copy(output.join(COMPONENT_FILE), path).expect("copy Component for toolchain smoke");
    }
}

#[test]
#[ignore = "requires a matching Calcit core checkout"]
fn packages_and_runs_real_calcit_enum_component() {
    let contract_path = std::env::var("CALCIT_BINDGEN_REAL_CONTRACT")
        .expect("CALCIT_BINDGEN_REAL_CONTRACT must name the generated contract");
    let core_path = std::env::var("CALCIT_BINDGEN_REAL_CORE")
        .expect("CALCIT_BINDGEN_REAL_CORE must name the generated core module");
    let contract = load_contract(contract_path).expect("load real Calcit Component contract");
    let core_module = fs::read(core_path).expect("read real Calcit core module");
    let temporary = TempDir::new().expect("temporary workspace");
    let output = package_check_and_run(&contract, &core_module, temporary.path());
    if let Ok(path) = std::env::var("CALCIT_BINDGEN_COMPONENT_OUTPUT") {
        fs::copy(output.join(COMPONENT_FILE), path).expect("copy real Calcit Component");
    }
}

fn package_check_and_run(
    contract: &InterfaceContract,
    core_module: &[u8],
    temporary: &Path,
) -> PathBuf {
    let has_variants = matches!(contract, InterfaceContract::Component(document) if document
        .definitions
        .iter()
        .any(|definition| definition.symbol == "echo-option-number"));
    let has_structs = matches!(contract, InterfaceContract::Component(document) if document
        .definitions
        .iter()
        .any(|definition| definition.symbol == "echo-profile"));
    let has_enums = matches!(contract, InterfaceContract::Component(document) if document
        .definitions
        .iter()
        .any(|definition| definition.symbol == "echo-event"));
    let has_numerics = matches!(contract, InterfaceContract::Component(document) if document
        .definitions
        .iter()
        .any(|definition| definition.symbol == "echo-numeric-scalars"));
    let core = temporary.join("program.wasm");
    fs::write(&core, core_module).expect("write core module");
    let output = temporary.join("generated");
    let manifest = generate_contract_directory(contract, Some(&core), &output, &[])
        .expect("package Component");

    assert_eq!(manifest.contract_kind, ContractKind::Component);
    assert_eq!(manifest.backends, vec![GenerationBackend::Wit]);
    assert!(manifest.core_module_digest.is_some());
    assert!(output.join(WIT_BINDINGS_FILE).is_file());
    assert!(output.join(COMPONENT_FILE).is_file());
    let wit =
        fs::read_to_string(output.join(WIT_BINDINGS_FILE)).expect("read generated Component WIT");
    assert!(wit.contains("echo-buffers: func(arg0: list<list<u8>>) -> list<list<u8>>;"));
    assert!(wit.contains("echo-number-lists: func(arg0: list<list<f64>>) -> list<list<f64>>;"));
    if has_variants {
        assert!(wit.contains("echo-option-number: func(arg0: option<f64>) -> option<f64>;"));
        assert!(wit.contains(
            "echo-result-number: func(arg0: result<f64, string>) -> result<f64, string>;"
        ));
        assert!(
            wit.contains("echo-result-unit: func(arg0: result<_, string>) -> result<_, string>;")
        );
        assert!(wit.contains("ping: func();"));
    }
    if has_structs {
        assert!(wit.contains("record component-wasm-main-profile {"));
        assert!(wit.contains("record component-wasm-main-profile-stats {"));
        assert!(wit.contains("maybe-name: option<string>,"));
        assert!(wit.contains("outcome: result<list<f64>, string>,"));
        assert!(wit.contains(
            "echo-profile: func(arg0: component-wasm-main-profile) -> component-wasm-main-profile;"
        ));
    }
    if has_enums {
        assert!(wit.contains("variant component-wasm-main-event {"));
        assert!(wit.contains("    idle,"));
        assert!(wit.contains("    moved(tuple<f64, f64>),"));
        assert!(wit.contains("    named(string),"));
        assert!(wit.contains("    profile(component-wasm-main-profile),"));
        assert!(wit.contains(
            "echo-event: func(arg0: component-wasm-main-event) -> component-wasm-main-event;"
        ));
    }
    if has_numerics {
        assert!(wit.contains("record component-wasm-main-numeric-scalars {"));
        for field in [
            "%f32: f32,",
            "%f64: f64,",
            "i16: s16,",
            "i32: s32,",
            "i64: s64,",
            "i8: s8,",
            "u16: u16,",
            "u32: u32,",
            "u64: u64,",
            "u8: u8,",
        ] {
            assert!(wit.contains(field), "missing numeric WIT field {field}");
        }
        assert!(wit.contains(
            "echo-numeric-scalars: func(arg0: component-wasm-main-numeric-scalars) -> component-wasm-main-numeric-scalars;"
        ));
    }
    assert!(
        check_contract_directory(contract, Some(&core), &output, &[])
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
    host.func_wrap("bool-not", |_store, (value,): (bool,)| Ok((!value,)))
        .expect("define host bool-not");
    host.func_wrap("buffer", |_store, (mut value,): (Vec<u8>,)| {
        value.reverse();
        Ok((value,))
    })
    .expect("define host buffer");
    host.func_wrap("echo", |_store, (value,): (String,)| Ok((value,)))
        .expect("define host echo");
    host.func_wrap("numbers", |_store, (mut value,): (Vec<f64>,)| {
        value.reverse();
        Ok((value,))
    })
    .expect("define host numbers");
    if has_numerics {
        host.func_wrap("numeric-scalars", |_store, (value,): (NumericScalars,)| {
            Ok((value,))
        })
        .expect("define host numeric scalars");
    }
    if has_variants {
        host.func_wrap("option-number", |_store, (value,): (Option<f64>,)| {
            Ok((value,))
        })
        .expect("define host option-number");
        host.func_wrap(
            "result-number",
            |_store, (value,): (Result<f64, String>,)| Ok((value,)),
        )
        .expect("define host result-number");
        host.func_wrap("ping", |_store, (): ()| Ok(()))
            .expect("define host ping");
    }
    if has_structs {
        host.func_wrap("profile", |_store, (mut value,): (Profile,)| {
            value.active = !value.active;
            value.stats.score += 1.0;
            Ok((value,))
        })
        .expect("define host profile");
    }
    if has_enums {
        host.func_wrap("event", |_store, (value,): (Event,)| Ok((value,)))
            .expect("define host event");
    }
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

    let call_host = instance
        .get_typed_func::<(f64,), (f64,)>(&mut store, "call-host-add-one")
        .expect("typed call-host-add-one export");
    assert_eq!(
        call_host.call(&mut store, (40.0,)).expect("call host"),
        (42.0,)
    );

    let bool_not = instance
        .get_typed_func::<(bool,), (bool,)>(&mut store, "bool-not")
        .expect("typed bool-not export");
    assert_eq!(
        bool_not.call(&mut store, (true,)).expect("negate true"),
        (false,)
    );
    assert_eq!(
        bool_not.call(&mut store, (false,)).expect("negate false"),
        (true,)
    );

    let call_host_bool = instance
        .get_typed_func::<(bool,), (bool,)>(&mut store, "call-host-bool-not")
        .expect("typed call-host-bool-not export");
    assert_eq!(
        call_host_bool
            .call(&mut store, (true,))
            .expect("call host bool-not"),
        (false,)
    );
    assert_eq!(
        call_host_bool
            .call(&mut store, (false,))
            .expect("call host bool-not with false"),
        (true,)
    );

    if has_numerics {
        let value = NumericScalars {
            f32: 1.5,
            f64: 1.25,
            i16: i16::MIN,
            i32: i32::MIN,
            i64: -9_007_199_254_740_991,
            i8: i8::MIN,
            u16: u16::MAX,
            u32: u32::MAX,
            u64: 9_007_199_254_740_991,
            u8: u8::MAX,
        };
        for name in ["echo-numeric-scalars", "call-host-numeric-scalars"] {
            let function = instance
                .get_typed_func::<(NumericScalars,), (NumericScalars,)>(&mut store, name)
                .expect("typed numeric Struct export");
            assert_eq!(
                function
                    .call(&mut store, (value.clone(),))
                    .expect("round-trip numeric Struct"),
                (value.clone(),)
            );
        }
    }

    if has_variants {
        let echo_option_number = instance
            .get_typed_func::<(Option<f64>,), (Option<f64>,)>(&mut store, "echo-option-number")
            .expect("typed echo-option-number export");
        for value in [None, Some(7.5)] {
            assert_eq!(
                echo_option_number
                    .call(&mut store, (value,))
                    .expect("echo Option<Number>"),
                (value,)
            );
        }

        let echo_option_text = instance
            .get_typed_func::<(Option<String>,), (Option<String>,)>(&mut store, "echo-option-text")
            .expect("typed echo-option-text export");
        for value in [None, Some("你好".to_owned())] {
            assert_eq!(
                echo_option_text
                    .call(&mut store, (value.clone(),))
                    .expect("echo Option<String>"),
                (value,)
            );
        }

        let echo_result_number = instance
            .get_typed_func::<(Result<f64, String>,), (Result<f64, String>,)>(
                &mut store,
                "echo-result-number",
            )
            .expect("typed echo-result-number export");
        for value in [Ok(9.25), Err("bad".to_owned())] {
            assert_eq!(
                echo_result_number
                    .call(&mut store, (value.clone(),))
                    .expect("echo Result<Number,String>"),
                (value,)
            );
        }

        let echo_result_unit = instance
            .get_typed_func::<(Result<(), String>,), (Result<(), String>,)>(
                &mut store,
                "echo-result-unit",
            )
            .expect("typed echo-result-unit export");
        for value in [Ok(()), Err("bad".to_owned())] {
            assert_eq!(
                echo_result_unit
                    .call(&mut store, (value.clone(),))
                    .expect("echo Result<Unit,String>"),
                (value,)
            );
        }

        let echo_result_numbers = instance
            .get_typed_func::<(Result<Vec<f64>, String>,), (Result<Vec<f64>, String>,)>(
                &mut store,
                "echo-result-numbers",
            )
            .expect("typed echo-result-numbers export");
        for value in [Ok(vec![]), Ok(vec![2.0, 4.0, 8.0]), Err("bad".to_owned())] {
            assert_eq!(
                echo_result_numbers
                    .call(&mut store, (value.clone(),))
                    .expect("echo Result<List<Number>,String>"),
                (value,)
            );
        }

        let call_host_option = instance
            .get_typed_func::<(Option<f64>,), (Option<f64>,)>(&mut store, "call-host-option-number")
            .expect("typed call-host-option-number export");
        assert_eq!(
            call_host_option
                .call(&mut store, (Some(12.5),))
                .expect("call host Option<Number>"),
            (Some(12.5),)
        );

        let call_host_result = instance
            .get_typed_func::<(Result<f64, String>,), (Result<f64, String>,)>(
                &mut store,
                "call-host-result-number",
            )
            .expect("typed call-host-result-number export");
        for value in [Ok(6.5), Err("bad".to_owned())] {
            assert_eq!(
                call_host_result
                    .call(&mut store, (value.clone(),))
                    .expect("call host Result<Number,String>"),
                (value,)
            );
        }

        let ping = instance
            .get_typed_func::<(), ()>(&mut store, "ping")
            .expect("typed ping export");
        ping.call(&mut store, ()).expect("call Unit export");

        let call_host_ping = instance
            .get_typed_func::<(), ()>(&mut store, "call-host-ping")
            .expect("typed call-host-ping export");
        call_host_ping.call(&mut store, ()).expect("call host Unit");
    }

    if has_structs {
        let profile = Profile {
            active: true,
            maybe_name: Some("Ada".to_owned()),
            name: "Ada".to_owned(),
            outcome: Ok(vec![4.0, 5.0]),
            scores: vec![1.0, 2.0, 3.0],
            stats: ProfileStats { score: 7.5 },
        };
        let echo_profile = instance
            .get_typed_func::<(Profile,), (Profile,)>(&mut store, "echo-profile")
            .expect("typed echo-profile export");
        assert_eq!(
            echo_profile
                .call(&mut store, (profile.clone(),))
                .expect("echo Struct record"),
            (profile.clone(),)
        );

        let call_host_profile = instance
            .get_typed_func::<(Profile,), (Profile,)>(&mut store, "call-host-profile")
            .expect("typed call-host-profile export");
        let mut expected = profile.clone();
        expected.active = false;
        expected.stats.score = 8.5;
        assert_eq!(
            call_host_profile
                .call(&mut store, (profile,))
                .expect("call host Struct record"),
            (expected,)
        );

        let alternate_profile = Profile {
            active: true,
            maybe_name: None,
            name: "Ada".to_owned(),
            outcome: Err("bad".to_owned()),
            scores: vec![1.0, 2.0, 3.0],
            stats: ProfileStats { score: 7.5 },
        };
        assert_eq!(
            echo_profile
                .call(&mut store, (alternate_profile.clone(),))
                .expect("echo Struct record with None and Err"),
            (alternate_profile.clone(),)
        );
        let mut alternate_expected = alternate_profile.clone();
        alternate_expected.active = false;
        alternate_expected.stats.score = 8.5;
        assert_eq!(
            call_host_profile
                .call(&mut store, (alternate_profile,))
                .expect("call host Struct record with None and Err"),
            (alternate_expected,)
        );
    }

    if has_enums {
        let profile = Profile {
            active: true,
            maybe_name: Some("Ada".to_owned()),
            name: "Ada".to_owned(),
            outcome: Ok(vec![4.0, 5.0]),
            scores: vec![1.0, 2.0, 3.0],
            stats: ProfileStats { score: 7.5 },
        };
        let events = [
            Event::Idle,
            Event::Moved((3.0, 4.0)),
            Event::Named("Ada".to_owned()),
            Event::Profile(profile),
        ];
        let echo_event = instance
            .get_typed_func::<(Event,), (Event,)>(&mut store, "echo-event")
            .expect("typed echo-event export");
        let call_host_event = instance
            .get_typed_func::<(Event,), (Event,)>(&mut store, "call-host-event")
            .expect("typed call-host-event export");
        for event in events {
            assert_eq!(
                echo_event
                    .call(&mut store, (event.clone(),))
                    .expect("echo Enum variant"),
                (event.clone(),)
            );
            assert_eq!(
                call_host_event
                    .call(&mut store, (event.clone(),))
                    .expect("call host Enum variant"),
                (event,)
            );
        }
    }

    let echo_buffer = instance
        .get_typed_func::<(Vec<u8>,), (Vec<u8>,)>(&mut store, "echo-buffer")
        .expect("typed echo-buffer export");
    for bytes in [vec![], vec![0, 255, 17], vec![128, 0, 254, 1]] {
        assert_eq!(
            echo_buffer
                .call(&mut store, (bytes.clone(),))
                .expect("echo Buffer"),
            (bytes,)
        );
    }

    let choose_buffer = instance
        .get_typed_func::<(bool, Vec<u8>, Vec<u8>), (Vec<u8>,)>(&mut store, "choose-buffer")
        .expect("typed choose-buffer export");
    assert_eq!(
        choose_buffer
            .call(&mut store, (true, vec![0, 255], vec![17, 0, 128]))
            .expect("choose Buffer true branch"),
        (vec![0, 255],)
    );
    assert_eq!(
        choose_buffer
            .call(&mut store, (false, vec![0, 255], vec![17, 0, 128]))
            .expect("choose Buffer false branch"),
        (vec![17, 0, 128],)
    );

    let is_buffer = instance
        .get_typed_func::<(Vec<u8>,), (bool,)>(&mut store, "is-buffer")
        .expect("typed is-buffer export");
    assert_eq!(
        is_buffer
            .call(&mut store, (vec![0, 255, 17],))
            .expect("check Buffer identity"),
        (true,)
    );

    let call_host_buffer = instance
        .get_typed_func::<(Vec<u8>,), (Vec<u8>,)>(&mut store, "call-host-buffer")
        .expect("typed call-host-buffer export");
    assert_eq!(
        call_host_buffer
            .call(&mut store, (vec![0, 255, 17],))
            .expect("call host Buffer"),
        (vec![17, 255, 0],)
    );

    let echo_bools = instance
        .get_typed_func::<(Vec<bool>,), (Vec<bool>,)>(&mut store, "echo-bools")
        .expect("typed echo-bools export");
    assert_eq!(
        echo_bools
            .call(&mut store, (vec![true, false, true],))
            .expect("echo Bool List"),
        (vec![true, false, true],)
    );

    let echo_numbers = instance
        .get_typed_func::<(Vec<f64>,), (Vec<f64>,)>(&mut store, "echo-numbers")
        .expect("typed echo-numbers export");
    for numbers in [vec![], vec![1.0, -2.5, 7.0]] {
        assert_eq!(
            echo_numbers
                .call(&mut store, (numbers.clone(),))
                .expect("echo Number List"),
            (numbers,)
        );
    }

    let call_host_numbers = instance
        .get_typed_func::<(Vec<f64>,), (Vec<f64>,)>(&mut store, "call-host-numbers")
        .expect("typed call-host-numbers export");
    assert_eq!(
        call_host_numbers
            .call(&mut store, (vec![1.0, -2.5, 7.0],))
            .expect("call host Number List"),
        (vec![7.0, -2.5, 1.0],)
    );

    let echo_texts = instance
        .get_typed_func::<(Vec<String>,), (Vec<String>,)>(&mut store, "echo-texts")
        .expect("typed echo-texts export");
    let texts = vec!["alpha".to_owned(), String::new(), "世界".to_owned()];
    assert_eq!(
        echo_texts
            .call(&mut store, (texts.clone(),))
            .expect("echo String List"),
        (texts,)
    );

    let echo_buffers = instance
        .get_typed_func::<(Vec<Vec<u8>>,), (Vec<Vec<u8>>,)>(&mut store, "echo-buffers")
        .expect("typed echo-buffers export");
    let buffers = vec![vec![0, 255], vec![], vec![17, 0, 128]];
    assert_eq!(
        echo_buffers
            .call(&mut store, (buffers.clone(),))
            .expect("echo Buffer List"),
        (buffers,)
    );

    let echo_number_lists = instance
        .get_typed_func::<(Vec<Vec<f64>>,), (Vec<Vec<f64>>,)>(&mut store, "echo-number-lists")
        .expect("typed nested Number List export");
    let nested = vec![vec![1.0, 2.0], vec![], vec![-3.0, 4.5, 7.0]];
    assert_eq!(
        echo_number_lists
            .call(&mut store, (nested.clone(),))
            .expect("echo nested Number List"),
        (nested,)
    );

    let choose_number = instance
        .get_typed_func::<(bool, f64, f64), (f64,)>(&mut store, "choose-number")
        .expect("typed choose-number export");
    assert_eq!(
        choose_number
            .call(&mut store, (true, 3.0, 4.0))
            .expect("choose true branch"),
        (3.0,)
    );
    assert_eq!(
        choose_number
            .call(&mut store, (false, 3.0, 4.0))
            .expect("choose false branch"),
        (4.0,)
    );

    let echo = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "echo-text")
        .expect("typed echo-text export");
    assert_eq!(
        echo.call(&mut store, ("Calcit".to_owned(),))
            .expect("call echo-text"),
        ("Calcit".to_owned(),)
    );

    let call_host_echo = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "call-host-echo")
        .expect("typed call-host-echo export");
    assert_eq!(
        call_host_echo
            .call(&mut store, ("Agent".to_owned(),))
            .expect("call host echo"),
        ("Agent".to_owned(),)
    );

    output
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
    assert!(String::from_utf8_lossy(&validate.stdout).contains("valid component Interface IR v2"));

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
