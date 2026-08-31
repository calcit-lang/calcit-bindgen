use std::fs;
use std::process::Command;
use std::slice;

use calcit_bindgen::{RUST_BINDINGS_FILE, generate_directory, load_document};
use calcit_native_ffi::{BUFFER_PROTOCOL_VERSION, CalcitFfiBuffer, buffer_status, encode_edn};
use cirru_edn::{Edn, EdnListView};
use libloading::{Library, Symbol};

const MD5_INTERFACE: &str = "tests/fixtures/md5-interface.json";

#[test]
fn generated_md5_adapter_compiles_and_runs_through_a_real_dylib() {
    let document = load_document(MD5_INTERFACE).expect("load md5 interface");
    let root = tempfile::tempdir().expect("temporary directory");
    let generated = root.path().join("generated");
    generate_directory(&document, &generated).expect("generate Rust adapter");
    let bindings = fs::read_to_string(generated.join(RUST_BINDINGS_FILE))
        .expect("read generated Rust bindings");
    assert!(bindings.contains("calcit_native_ffi::export_edn_buffer_method_v1!"));
    assert!(!bindings.contains("todo!"));
    assert!(!bindings.contains("Dynamic"));

    let crate_root = root.path().join("md5-native");
    fs::create_dir_all(crate_root.join("src")).expect("create fixture crate");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "calcit-bindgen-md5-native"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
calcit_native_ffi = "0.1.3"
cirru_edn = "0.8.0"
md5 = "0.7.0"
"#,
    )
    .expect("write fixture manifest");
    fs::write(crate_root.join("src/bindings.rs"), bindings).expect("write bindings");
    fs::write(
        crate_root.join("src/lib.rs"),
        r#"include!("bindings.rs");

struct Service;

impl CalcitStdFfi for Service {
    fn calcit_std_hash_md5(&self, arg0: String) -> Result<String, String> {
        Ok(format!("{:x}", md5::compute(arg0)))
    }
}

static SERVICE: Service = Service;
export_calcit_std_ffi!(SERVICE);
"#,
    )
    .expect("write fixture source");

    let build = Command::new(env!("CARGO"))
        .args(["build", "--offline", "--quiet"])
        .current_dir(&crate_root)
        .output()
        .expect("build generated dylib fixture");
    assert!(
        build.status.success(),
        "generated dylib failed to compile:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let library_path = crate_root
        .join("target/debug")
        .join(dynamic_library_name("calcit_bindgen_md5_native"));
    // SAFETY: the test loads the just-built fixture and resolves signatures
    // defined by calcit-native-ffi buffer ABI v1.
    unsafe {
        let library = Library::new(&library_path).expect("load generated dylib");
        let version: Symbol<'_, unsafe extern "C" fn() -> u32> = library
            .get(b"calcit_ffi_buffer_version\0")
            .expect("version symbol");
        assert_eq!(version(), BUFFER_PROTOCOL_VERSION);

        let md5: Symbol<'_, unsafe extern "C" fn(*const u8, usize, *mut CalcitFfiBuffer) -> i32> =
            library.get(b"md5_calcit_ffi_v1\0").expect("md5 symbol");
        let free: Symbol<'_, unsafe extern "C" fn(CalcitFfiBuffer)> = library
            .get(b"calcit_ffi_buffer_free\0")
            .expect("buffer free symbol");
        let request =
            encode_edn(&Edn::List(EdnListView(vec![Edn::str("hello")]))).expect("encode request");
        let mut output = CalcitFfiBuffer::empty();
        assert_eq!(
            md5(request.as_ptr(), request.len(), &mut output),
            buffer_status::OK
        );
        let response = slice::from_raw_parts(output.ptr, output.len).to_vec();
        free(output);
        let value = cirru_edn::parse(std::str::from_utf8(&response).expect("UTF-8 response"))
            .expect("parse response");
        assert_eq!(value, Edn::str("5d41402abc4b2a76b9719d911017c592"));
    }
}

fn dynamic_library_name(stem: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}
