#![cfg(feature = "wasmtime-http")]

use std::fs;

use calcit_bindgen::wasmtime_host::{exit_code, run_config_file, run_config_file_with_exit_code};
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use wit_component::{ComponentEncoder, StringEncoding, dummy_module, embed_component_metadata};
use wit_parser::{ManglingAndAbi, Resolve};

const CODEC_WIT: &str = r#"
package test:codec;

world smoke {
  variant method { get, post }
  variant body { empty, text(string), bytes(list<u8>) }
  record request {
    body: body,
    headers: list<string>,
    max-response-bytes: u64,
    method: method,
    url: string,
  }
  record response { body: body, status: u16 }
  export run: func(request: request) -> result<response, string>;
  export optional: func(value: option<string>);
  export fallible: func(value: result<string, string>);
}
"#;

fn component_bytes() -> Vec<u8> {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("codec.wit", CODEC_WIT)
        .expect("parse codec WIT");
    let world = resolve
        .select_world(&[package], Some("smoke"))
        .expect("select codec world");
    let mut module = dummy_module(&resolve, world, ManglingAndAbi::Standard32);
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8)
        .expect("embed codec metadata");
    ComponentEncoder::default()
        .module(&module)
        .expect("accept codec dummy module")
        .validate(true)
        .encode()
        .expect("encode codec component")
}

fn config(component: &str, argument: &str) -> String {
    entry_config(component, "run", argument)
}

fn entry_config(component: &str, entry: &str, argument: &str) -> String {
    format!(
        r#"{{}}
  :component |{component}
  :entry |{entry}
  :arguments $ []
    {argument}
  :max-response-bytes 4096
  :allowed-origins $ []
  :preopens $ []
"#
    )
}

fn http_arguments(url: &str, limit: u64) -> String {
    format!(
        r#"[]
  {{}}
    :body $ :: :empty
    :headers $ []
    :max-response-bytes {limit}
    :method $ :: :get
    :url |{url}
"#
    )
}

fn file_http_config(
    component: &str,
    preopen: &str,
    limit: u64,
    allowed_origin: Option<&str>,
) -> String {
    let origins = allowed_origin
        .map(|origin| format!("[] |{origin}"))
        .unwrap_or_else(|| "[]".to_owned());
    format!(
        r#"{{}}
  :component |{component}
  :entry |call-host-http-request
  :arguments $ []
  :arguments-file |/data/request.cirru
  :result-file |/data/result.cirru
  :max-response-bytes {limit}
  :allowed-origins $ {origins}
  :preopens $ []
    {{}} (:host |{preopen}) (:guest |/data) (:access :read-write)
"#
    )
}

async fn serve_once(body: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind host codec HTTP server");
    let address = listener.local_addr().expect("read local HTTP address");
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept HTTP request");
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .await
            .expect("read HTTP request line");
        assert_eq!(request_line, "GET /items HTTP/1.1\r\n");
        let mut stream = reader.into_inner();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .await
            .expect("write response headers");
        stream.write_all(body).await.expect("write response body");
    });
    format!("http://{address}")
}

#[tokio::test]
async fn decodes_a_typed_entry_before_invocation() {
    let directory = tempdir().expect("create codec temp directory");
    let component_path = directory.path().join("codec.wasm");
    fs::write(&component_path, component_bytes()).expect("write codec component");
    let config_path = directory.path().join("capabilities.cirru");
    fs::write(
        &config_path,
        config(
            "codec.wasm",
            r#"{}
      :body $ :: :empty
      :headers $ []
      :max-response-bytes 64
      :method $ :: :get
      :url |https://example.test/items"#,
        ),
    )
    .expect("write codec config");

    let error = run_config_file(&config_path)
        .await
        .expect_err("dummy export traps after argument lowering");
    assert!(error.contains("failed to drive concurrent Component entry"));
    assert!(error.contains("unreachable"));
}

#[tokio::test]
async fn rejects_unknown_fields_before_invocation_with_a_path() {
    let directory = tempdir().expect("create codec temp directory");
    fs::write(directory.path().join("codec.wasm"), component_bytes())
        .expect("write codec component");
    let config_path = directory.path().join("capabilities.cirru");
    fs::write(
        &config_path,
        config(
            "codec.wasm",
            r#"{}
      :body $ :: :empty
      :headers $ []
      :max-response-bytes 64
      :method $ :: :get
      :url |https://example.test/items
      :surprise true"#,
        ),
    )
    .expect("write codec config");

    let error = run_config_file(&config_path)
        .await
        .expect_err("reject unknown request field");
    assert!(error.contains("arguments[0] (request) contains unknown field :surprise"));
}

#[tokio::test]
async fn rejects_out_of_range_values_with_a_field_path() {
    let directory = tempdir().expect("create codec temp directory");
    fs::write(directory.path().join("codec.wasm"), component_bytes())
        .expect("write codec component");
    let config_path = directory.path().join("capabilities.cirru");
    fs::write(
        &config_path,
        config(
            "codec.wasm",
            r#"{}
      :body $ :: :empty
      :headers $ []
      :max-response-bytes -1
      :method $ :: :get
      :url |https://example.test/items"#,
        ),
    )
    .expect("write codec config");

    let error = run_config_file(&config_path)
        .await
        .expect_err("reject negative u64");
    assert!(error.contains("arguments[0] (request).max-response-bytes"));
    assert!(error.contains(":: :u64 |decimal"));
}

#[tokio::test]
async fn rejects_nominal_option_and_result_cases() {
    let directory = tempdir().expect("create codec temp directory");
    fs::write(directory.path().join("codec.wasm"), component_bytes())
        .expect("write codec component");
    let config_path = directory.path().join("capabilities.cirru");

    for (entry, value) in [
        ("optional", "%:: 'Option 'some |value"),
        ("fallible", "%:: 'Result 'ok |value"),
    ] {
        fs::write(&config_path, entry_config("codec.wasm", entry, value))
            .expect("write nominal enum config");
        let error = run_config_file(&config_path)
            .await
            .expect_err("reject nominal Option or Result case");
        assert!(
            error.contains("arguments[0] (value) must use an unqualified :: case"),
            "unexpected {entry} error: {error}"
        );
    }
}

#[tokio::test]
async fn file_backed_inputs_fail_before_component_loading_with_stable_exits() {
    let directory = tempdir().expect("create file-backed host directory");
    let config_path = directory.path().join("capabilities.cirru");
    let arguments_path = directory.path().join("arguments.cirru");
    let root = directory.path().to_str().expect("UTF-8 temp path");

    fs::write(&arguments_path, "{} (:not |a-list)").expect("write invalid arguments");
    fs::write(
        &config_path,
        format!(
            r#"{{}}
  :component |missing.wasm
  :entry |run
  :arguments-file |/data/arguments.cirru
  :max-response-bytes 64
  :preopens $ []
    {{}} (:host |{root}) (:guest |/data) (:access :read)
"#
        ),
    )
    .expect("write invalid input config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::INVALID_INPUT
    );

    fs::write(&arguments_path, "[]").expect("write valid arguments");
    fs::write(
        &config_path,
        format!(
            r#"{{}}
  :component |missing.wasm
  :entry |run
  :arguments-file |/data/arguments.cirru
  :result-file |/data/result.cirru
  :max-response-bytes 64
  :preopens $ []
    {{}} (:host |{root}) (:guest |/data) (:access :read)
"#
        ),
    )
    .expect("write denied output config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::CAPABILITY_DENIED
    );
}

#[tokio::test]
#[ignore = "requires a real Calcit HTTP Component from CALCIT_BINDGEN_REAL_HTTP_COMPONENT"]
async fn drives_real_calcit_http_success_denial_and_limit_results() {
    let component = std::env::var("CALCIT_BINDGEN_REAL_HTTP_COMPONENT")
        .expect("CALCIT_BINDGEN_REAL_HTTP_COMPONENT must name a packaged Component");
    let directory = tempdir().expect("create real HTTP host temp directory");
    let config_path = directory.path().join("capabilities.cirru");

    let request_path = directory.path().join("request.cirru");
    let result_path = directory.path().join("result.cirru");
    let preopen = directory.path().to_str().expect("UTF-8 temp path");

    fs::write(
        &request_path,
        http_arguments("http://127.0.0.1:1/items", 64),
    )
    .expect("write denied request");
    fs::write(
        &config_path,
        file_http_config(&component, preopen, 64, None),
    )
    .expect("write denied config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::CAPABILITY_DENIED
    );
    assert!(
        fs::read_to_string(&result_path)
            .expect("read denied result")
            .contains("capability-denied")
    );

    let origin = serve_once(b"ok").await;
    fs::write(
        &request_path,
        http_arguments(&format!("{origin}/items"), 64),
    )
    .expect("write success request");
    fs::write(
        &config_path,
        file_http_config(&component, preopen, 64, Some(&origin)),
    )
    .expect("write success config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::SUCCESS
    );
    assert!(
        fs::read_to_string(&result_path)
            .expect("read success result")
            .starts_with(":: 'ok")
    );

    let origin = serve_once(b"123456789").await;
    fs::write(&request_path, http_arguments(&format!("{origin}/items"), 8))
        .expect("write response limit request");
    fs::write(
        &config_path,
        file_http_config(&component, preopen, 8, Some(&origin)),
    )
    .expect("write response limit config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::RESPONSE_TOO_LARGE
    );
    assert!(
        fs::read_to_string(&result_path)
            .expect("read response limit result")
            .contains("response-too-large")
    );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve closed transport address");
    let closed_origin = format!(
        "http://{}",
        listener.local_addr().expect("read closed address")
    );
    drop(listener);
    fs::write(
        &request_path,
        http_arguments(&format!("{closed_origin}/items"), 64),
    )
    .expect("write transport request");
    fs::write(
        &config_path,
        file_http_config(&component, preopen, 64, Some(&closed_origin)),
    )
    .expect("write transport config");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::TRANSPORT
    );

    fs::write(&request_path, "{} (:not |a-list)").expect("write invalid argument input");
    assert_eq!(
        run_config_file_with_exit_code(&config_path).await,
        exit_code::INVALID_INPUT
    );
}
