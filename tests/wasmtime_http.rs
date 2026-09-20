#![cfg(feature = "wasmtime-http")]

use calcit_bindgen::wasmtime_http::{
    HttpBody, HttpError, HttpMethod, HttpRequest, WasiHttpConfig, send,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wit_component::{ComponentEncoder, StringEncoding, dummy_module, embed_component_metadata};
use wit_parser::{LiftLowerAbi, ManglingAndAbi, Resolve};

const HTTP_CONTRACT_WIT: &str = r#"
package calcit:wasi-http;

interface types {
  variant http-method { delete, get, head, options, patch, post, put }
  record http-header { name: string, value: list<u8> }
  variant http-body { bytes(list<u8>), empty, text(string) }
  record http-request {
    body: http-body,
    headers: list<http-header>,
    max-response-bytes: u64,
    method: http-method,
    url: string,
  }
  record http-response { body: http-body, headers: list<http-header>, status: u16 }
  variant http-error {
    capability-denied(string),
    invalid-request(string),
    response-too-large(u64),
    transport(string),
    unsupported(string),
  }
}

interface client {
  use types.{http-error, http-request, http-response};
  request: async func(request: http-request) -> result<http-response, http-error>;
}

world smoke {
  import client;
}
"#;

async fn serve_once(body: &'static [u8], content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local HTTP server");
    let address = listener.local_addr().expect("local HTTP address");
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
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
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

fn request(url: String, max_response_bytes: u64) -> HttpRequest {
    HttpRequest {
        body: HttpBody::Empty,
        headers: vec![],
        max_response_bytes,
        method: HttpMethod::Get,
        url,
    }
}

#[tokio::test]
async fn adapter_links_the_closed_component_contract() {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("http-contract.wit", HTTP_CONTRACT_WIT)
        .expect("parse HTTP contract WIT");
    let world = resolve
        .select_world(&[package], Some("smoke"))
        .expect("select HTTP smoke world");
    let mut module = dummy_module(
        &resolve,
        world,
        ManglingAndAbi::Legacy(LiftLowerAbi::AsyncStackful),
    );
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8)
        .expect("embed HTTP contract metadata");
    let component = ComponentEncoder::default()
        .module(&module)
        .expect("accept HTTP dummy module")
        .validate(true)
        .encode()
        .expect("encode HTTP smoke component");

    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    let engine = Engine::new(&config).expect("create Component engine");
    let component = Component::new(&engine, component).expect("compile HTTP smoke component");
    let mut linker = Linker::new(&engine);
    calcit_bindgen::wasmtime_http::add_to_linker(&mut linker, WasiHttpConfig::default())
        .expect("link buffered HTTP adapter");
    linker
        .instantiate_async(&mut Store::new(&engine, ()), &component)
        .await
        .expect("instantiate HTTP smoke component");
}

#[tokio::test]
async fn performs_real_bounded_http_request() {
    let origin = serve_once(br#"{"ok":true}"#, "Application/JSON; Charset=UTF-8").await;
    let config = WasiHttpConfig::default()
        .allow_origin(&origin)
        .expect("allow local HTTP origin");
    let response = send(&config, request(format!("{origin}/items"), 64))
        .await
        .expect("perform bounded HTTP request");
    assert_eq!(response.status, 200);
    assert!(matches!(response.body, HttpBody::Text(ref value) if value == r#"{"ok":true}"#));
}

#[tokio::test]
async fn denies_ungranted_origins_without_network_access() {
    let error = send(
        &WasiHttpConfig::default(),
        request("http://127.0.0.1:1/items".to_owned(), 64),
    )
    .await
    .expect_err("deny origin without a capability");
    assert!(matches!(error, HttpError::CapabilityDenied(_)));
}

#[tokio::test]
async fn reports_an_unsupported_host_as_a_typed_error() {
    let error = send(
        &WasiHttpConfig::unsupported("HTTP disabled by this embedder"),
        request("http://example.test/items".to_owned(), 64),
    )
    .await
    .expect_err("report unsupported host");
    assert!(
        matches!(error, HttpError::Unsupported(ref reason) if reason == "HTTP disabled by this embedder")
    );
}

#[tokio::test]
async fn rejects_responses_over_the_declared_limit() {
    let origin = serve_once(b"123456789", "application/octet-stream").await;
    let config = WasiHttpConfig::default()
        .allow_origin(&origin)
        .expect("allow local HTTP origin");
    let error = send(&config, request(format!("{origin}/items"), 8))
        .await
        .expect_err("reject oversized response");
    assert!(matches!(error, HttpError::ResponseTooLarge(9)));
}

#[tokio::test]
async fn host_response_limit_caps_the_guest_request_limit() {
    let origin = serve_once(b"123456789", "application/octet-stream").await;
    let config = WasiHttpConfig::default()
        .allow_origin(&origin)
        .expect("allow local HTTP origin")
        .max_response_bytes(4);
    let error = send(&config, request(format!("{origin}/items"), 64))
        .await
        .expect_err("host response limit must cap the guest request limit");
    assert!(matches!(error, HttpError::ResponseTooLarge(9)));
}
