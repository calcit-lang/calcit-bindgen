use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use calcit_bindgen::package_wasi_http_proxy;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn minimal_http_proxy_wat(body: &str) -> String {
    format!(
        r#"
        (module
          (import "wasi:http/types@0.2.12" "[resource-drop]incoming-request" (func $drop_req (param i32)))
          (import "wasi:http/types@0.2.12" "[constructor]fields" (func $new_fields (result i32)))
          (import "wasi:http/types@0.2.12" "[constructor]outgoing-response" (func $new_resp (param i32) (result i32)))
          (import "wasi:http/types@0.2.12" "[method]outgoing-response.body" (func $resp_body (param i32 i32)))
          (import "wasi:http/types@0.2.12" "[method]outgoing-body.write" (func $body_write (param i32 i32)))
          (import "wasi:http/types@0.2.12" "[static]outgoing-body.finish" (func $body_finish (param i32 i32 i32 i32)))
          (import "wasi:http/types@0.2.12" "[static]response-outparam.set" (func $outparam_set (param i32 i32 i32 i32 i64 i32 i32 i32 i32)))
          (import "wasi:io/streams@0.2.12" "[method]output-stream.blocking-write-and-flush" (func $stream_write (param i32 i32 i32 i32)))
          (import "wasi:io/streams@0.2.12" "[resource-drop]output-stream" (func $drop_stream (param i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (data (i32.const 16) "{body}")
          (func (export "cabi_realloc") (param $old i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)
            (local $aligned i32)
            (local.set $aligned
              (i32.and
                (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
                (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
            (global.set $heap (i32.add (local.get $aligned) (local.get $new_size)))
            (local.get $aligned))
          (func (export "wasi:http/incoming-handler@0.2.12#handle") (param $req i32) (param $out i32)
            (local $fields i32)
            (local $resp i32)
            (local $body i32)
            (local $stream i32)
            (local.get $req)
            (call $drop_req)
            (call $new_fields)
            (local.set $fields)
            (local.get $fields)
            (call $new_resp)
            (local.set $resp)
            (local.get $resp)
            (i32.const 256)
            (call $resp_body)
            (if (i32.load8_u (i32.const 256))
              (then (unreachable)))
            (i32.load (i32.const 260))
            (local.set $body)
            (local.get $body)
            (i32.const 256)
            (call $body_write)
            (if (i32.load8_u (i32.const 256))
              (then (unreachable)))
            (i32.load (i32.const 260))
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
            (i32.const 16)
            (i32.const {body_len})
            (i32.const 256)
            (call $stream_write)
            (if (i32.load8_u (i32.const 256))
              (then (unreachable)))
            (local.get $stream)
            (call $drop_stream)
            (local.get $body)
            (i32.const 0)
            (i32.const 0)
            (i32.const 256)
            (call $body_finish)
            (if (i32.load8_u (i32.const 256))
              (then (unreachable)))))
        "#,
        body_len = body.len()
    )
}

fn free_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    listener.local_addr().expect("ephemeral local addr")
}

fn connect_with_retry(child: &mut Child, addr: SocketAddr) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("poll wasmtime serve") {
            let mut stderr = String::new();
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            panic!("wasmtime serve exited early ({status}): {stderr}");
        }
        match TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
            Ok(stream) => return stream,
            Err(err) => {
                if Instant::now() >= deadline {
                    panic!("timed out connecting to wasmtime serve at {addr}: {err}");
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn decode_chunked_body(raw: &str) -> String {
    let mut rest = raw;
    let mut out = String::new();
    loop {
        let (size_line, after_size) = rest
            .split_once("\r\n")
            .unwrap_or_else(|| panic!("invalid chunked encoding: {raw:?}"));
        let size_hex = size_line.split(';').next().expect("chunk size").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .unwrap_or_else(|err| panic!("invalid chunk size {size_hex:?}: {err}"));
        if size == 0 {
            break;
        }
        let chunk = after_size
            .get(..size)
            .unwrap_or_else(|| panic!("truncated chunk of {size} bytes in {raw:?}"));
        out.push_str(chunk);
        rest = after_size
            .get(size..)
            .and_then(|s| s.strip_prefix("\r\n"))
            .unwrap_or_else(|| panic!("missing CRLF after chunk in {raw:?}"));
    }
    out
}

fn send_http_get(child: &mut Child, addr: SocketAddr, path: &str) -> (String, String) {
    let mut stream = connect_with_retry(child, addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .expect("write HTTP request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read HTTP response");
    let (head, raw_body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("invalid HTTP response for {path}: {response:?}"));
    let is_chunked = head.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
        })
    });
    let body = if is_chunked {
        decode_chunked_body(raw_body)
    } else {
        raw_body.to_owned()
    };
    (head.to_owned(), body)
}

#[test]
fn packages_and_serves_wasi_0212_http_proxy_component() {
    let expected_body = "hello from wasi-http proxy";
    let core = wat::parse_str(minimal_http_proxy_wat(expected_body))
        .expect("compile minimal WASI HTTP proxy WAT");
    let bytes = package_wasi_http_proxy(&core).expect("package WASI 0.2.12 HTTP proxy Component");
    assert_eq!(
        bytes,
        package_wasi_http_proxy(&core).expect("repeat deterministic HTTP proxy packaging")
    );

    let config = Config::new();
    let engine = Engine::new(&config).expect("create Wasmtime engine");
    let component = Component::new(&engine, &bytes).expect("validate WASI HTTP proxy Component");
    let component_type = component.component_type();
    let imports = component_type
        .imports(&engine)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert_eq!(
        imports,
        vec![
            "wasi:io/error@0.2.12",
            "wasi:io/streams@0.2.12",
            "wasi:http/types@0.2.12",
        ]
    );
    assert!(
        component_type
            .exports(&engine)
            .any(|(name, _)| name == "wasi:http/incoming-handler@0.2.12")
    );

    let Some(wasmtime_cli) = std::env::var_os("WASMTIME_49_CLI") else {
        return;
    };
    let temporary = TempDir::new().expect("temporary output directory");
    let component_path = temporary.path().join("proxy.wasm");
    fs::write(&component_path, &bytes).expect("write HTTP proxy Component");

    let addr = free_loopback_addr();
    let mut guard = ChildGuard(
        Command::new(wasmtime_cli)
            .args([
                "serve",
                "--addr",
                &addr.to_string(),
                "--max-instance-reuse-count",
                "8",
            ])
            .arg(&component_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn wasmtime serve"),
    );

    for attempt in ["/first", "/second"] {
        let (head, body) = send_http_get(&mut guard.0, addr, attempt);
        assert!(
            head.starts_with("HTTP/1.1 200 OK"),
            "unexpected status head for {attempt}: {head:?}"
        );
        assert_eq!(body, expected_body, "unexpected body for {attempt}");
    }
}

#[test]
fn rejects_wasi_020_incoming_handler_export_for_0212_world() {
    let core = wat::parse_str(
        r#"(module
          (memory (export "memory") 1)
          (func (export "wasi:http/incoming-handler@0.2.0#handle") (param i32 i32)))"#,
    )
    .expect("compile old 0.2.0 handler fixture");
    let error = package_wasi_http_proxy(&core).expect_err("old 0.2.0 export must not package");
    assert!(
        error.contains("wasi:http/incoming-handler@0.2.12"),
        "{error}"
    );
}

#[test]
fn rejects_preview1_imports_in_wasi_http_proxy() {
    let core = wat::parse_str(
        r#"(module
          (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "wasi:http/incoming-handler@0.2.12#handle") (param i32 i32)))"#,
    )
    .expect("compile Preview 1 HTTP fixture");
    let error = package_wasi_http_proxy(&core).expect_err("Preview 1 import must be rejected");
    assert!(error.starts_with("E_WASI_HTTP_PREVIEW1_IMPORT:"), "{error}");
    assert!(error.contains("fd_write"), "{error}");
}

#[test]
fn rejects_component_input_in_wasi_http_proxy() {
    let core = wat::parse_str(minimal_http_proxy_wat("ok")).expect("compile proxy core fixture");
    let component = package_wasi_http_proxy(&core).expect("package HTTP proxy");
    let error = package_wasi_http_proxy(&component).expect_err("Component input must be rejected");
    assert!(
        error.contains("expected a core WebAssembly module"),
        "{error}"
    );
    assert!(!error.contains("--core-module"), "{error}");
}

#[test]
fn pinned_wasi_http_wit_hashes_match_documentation() {
    let readme = include_str!("../src/wasi_http_wit/README.md");
    for (name, content) in [
        (
            "cli.wit",
            include_bytes!("../src/wasi_http_wit/cli.wit").as_slice(),
        ),
        (
            "clocks.wit",
            include_bytes!("../src/wasi_http_wit/clocks.wit").as_slice(),
        ),
        (
            "filesystem.wit",
            include_bytes!("../src/wasi_http_wit/filesystem.wit").as_slice(),
        ),
        (
            "http.wit",
            include_bytes!("../src/wasi_http_wit/http.wit").as_slice(),
        ),
        (
            "io.wit",
            include_bytes!("../src/wasi_http_wit/io.wit").as_slice(),
        ),
        (
            "random.wit",
            include_bytes!("../src/wasi_http_wit/random.wit").as_slice(),
        ),
        (
            "sockets.wit",
            include_bytes!("../src/wasi_http_wit/sockets.wit").as_slice(),
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
