//! Bounded Wasmtime adapter for Calcit's closed HTTP Component contract.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderName, HeaderValue, Method, Uri};
use http_body_util::{BodyExt, Full};
use wasmtime::component::{ComponentType, Lift, Linker, Lower};
use wasmtime_wasi_http::DEFAULT_FORBIDDEN_HEADERS;
use wasmtime_wasi_http::p2::bindings::http::types::ErrorCode;
use wasmtime_wasi_http::p2::types::OutgoingRequestConfig;

const CLIENT_INSTANCE: &str = "calcit:wasi-http/client";
const REQUEST_FUNCTION: &str = "request";

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum HttpMethod {
    #[component(name = "delete")]
    Delete,
    #[component(name = "get")]
    Get,
    #[component(name = "head")]
    Head,
    #[component(name = "options")]
    Options,
    #[component(name = "patch")]
    Patch,
    #[component(name = "post")]
    Post,
    #[component(name = "put")]
    Put,
}

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(record)]
pub struct HttpHeader {
    pub name: String,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum HttpBody {
    #[component(name = "bytes")]
    Bytes(Vec<u8>),
    #[component(name = "empty")]
    Empty,
    #[component(name = "text")]
    Text(String),
}

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(record)]
pub struct HttpRequest {
    pub body: HttpBody,
    pub headers: Vec<HttpHeader>,
    #[component(name = "max-response-bytes")]
    pub max_response_bytes: u64,
    pub method: HttpMethod,
    pub url: String,
}

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(record)]
pub struct HttpResponse {
    pub body: HttpBody,
    pub headers: Vec<HttpHeader>,
    pub status: u16,
}

#[derive(Clone, Debug, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum HttpError {
    #[component(name = "capability-denied")]
    CapabilityDenied(String),
    #[component(name = "invalid-request")]
    InvalidRequest(String),
    #[component(name = "response-too-large")]
    ResponseTooLarge(u64),
    #[component(name = "transport")]
    Transport(String),
    #[component(name = "unsupported")]
    Unsupported(String),
}

/// Explicit network capabilities and transport timeouts for the adapter.
#[derive(Clone, Debug)]
pub struct WasiHttpConfig {
    allowed_origins: Arc<BTreeSet<String>>,
    unsupported_reason: Option<Arc<str>>,
    max_response_bytes: u64,
    pub connect_timeout: Duration,
    pub first_byte_timeout: Duration,
    pub between_bytes_timeout: Duration,
}

impl Default for WasiHttpConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Arc::default(),
            unsupported_reason: None,
            max_response_bytes: u64::MAX,
            connect_timeout: Duration::from_secs(30),
            first_byte_timeout: Duration::from_secs(30),
            between_bytes_timeout: Duration::from_secs(30),
        }
    }
}

impl WasiHttpConfig {
    /// Link the contract while reporting that this host has no HTTP runtime.
    pub fn unsupported(reason: impl Into<Arc<str>>) -> Self {
        Self {
            unsupported_reason: Some(reason.into()),
            ..Self::default()
        }
    }

    /// Allow one exact `scheme://authority` origin.
    pub fn allow_origin(mut self, origin: &str) -> Result<Self, String> {
        let uri = origin
            .parse::<Uri>()
            .map_err(|error| format!("invalid HTTP origin {origin:?}: {error}"))?;
        let normalized = request_origin(&uri)
            .ok_or_else(|| format!("HTTP origin must contain scheme and authority: {origin:?}"))?;
        if uri
            .path_and_query()
            .is_some_and(|value| value.as_str() != "/")
        {
            return Err(format!(
                "HTTP origin must not contain a path or query: {origin:?}"
            ));
        }
        Arc::make_mut(&mut self.allowed_origins).insert(normalized);
        Ok(self)
    }

    /// Apply a host-owned response ceiling in addition to the guest request limit.
    pub fn max_response_bytes(mut self, limit: u64) -> Self {
        self.max_response_bytes = limit;
        self
    }

    fn permits(&self, uri: &Uri) -> bool {
        request_origin(uri).is_some_and(|origin| self.allowed_origins.contains(&origin))
    }
}

/// Link the closed Calcit HTTP import to Wasmtime's production WASI 0.2 HTTP transport.
pub fn add_to_linker<T: 'static>(
    linker: &mut Linker<T>,
    config: WasiHttpConfig,
) -> wasmtime::Result<()> {
    linker.instance(CLIENT_INSTANCE)?.func_wrap_concurrent(
        REQUEST_FUNCTION,
        move |_accessor, (request,): (HttpRequest,)| {
            let config = config.clone();
            Box::pin(async move { Ok((send(&config, request).await,)) })
        },
    )?;
    Ok(())
}

/// Execute one bounded request through the transport behind `wasi:http/outgoing-handler`.
pub async fn send(
    config: &WasiHttpConfig,
    request: HttpRequest,
) -> Result<HttpResponse, HttpError> {
    if let Some(reason) = &config.unsupported_reason {
        return Err(HttpError::Unsupported(reason.to_string()));
    }
    let max_response_bytes = request.max_response_bytes.min(config.max_response_bytes);
    let uri = request
        .url
        .parse::<Uri>()
        .map_err(|error| HttpError::InvalidRequest(format!("invalid URL: {error}")))?;
    let Some(scheme) = uri.scheme_str() else {
        return Err(HttpError::InvalidRequest(
            "URL must contain an http or https scheme".to_owned(),
        ));
    };
    if scheme != "http" && scheme != "https" {
        return Err(HttpError::InvalidRequest(format!(
            "unsupported URL scheme {scheme:?}"
        )));
    }
    if uri.authority().is_none() {
        return Err(HttpError::InvalidRequest(
            "URL must contain an authority".to_owned(),
        ));
    }
    if !config.permits(&uri) {
        return Err(HttpError::CapabilityDenied(format!(
            "HTTP origin {} is not allowed",
            request_origin(&uri).expect("validated absolute URI")
        )));
    }
    let use_tls = scheme == "https";

    let mut builder = http::Request::builder()
        .method(method(request.method))
        .uri(uri);
    for header in request.headers {
        let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|error| {
            HttpError::InvalidRequest(format!("invalid header name {:?}: {error}", header.name))
        })?;
        if DEFAULT_FORBIDDEN_HEADERS.contains(&name) {
            return Err(HttpError::InvalidRequest(format!(
                "forbidden HTTP header {:?}",
                header.name
            )));
        }
        let value = HeaderValue::from_bytes(&header.value).map_err(|error| {
            HttpError::InvalidRequest(format!(
                "invalid value for header {:?}: {error}",
                header.name
            ))
        })?;
        builder = builder.header(name, value);
    }
    let body = match request.body {
        HttpBody::Empty => Bytes::new(),
        HttpBody::Text(value) => Bytes::from(value),
        HttpBody::Bytes(value) => Bytes::from(value),
    };
    let body = Full::new(body)
        .map_err(|never: Infallible| match never {})
        .boxed_unsync();
    let request = builder
        .body(body)
        .map_err(|error| HttpError::InvalidRequest(format!("invalid HTTP request: {error}")))?;
    let transport = OutgoingRequestConfig {
        use_tls,
        connect_timeout: config.connect_timeout,
        first_byte_timeout: config.first_byte_timeout,
        between_bytes_timeout: config.between_bytes_timeout,
    };
    let response = wasmtime_wasi_http::p2::default_send_request_handler(request, transport)
        .await
        .map_err(map_transport_error)?;
    let wasmtime_wasi_http::p2::types::IncomingResponse {
        resp,
        worker: _worker,
        ..
    } = response;
    let (parts, mut body) = resp.into_parts();
    let mut bytes = Vec::new();
    while let Some(frame) = tokio::time::timeout(config.between_bytes_timeout, body.frame())
        .await
        .map_err(|_| HttpError::Transport("response body timed out".to_owned()))?
    {
        let frame = frame.map_err(map_transport_error)?;
        if let Ok(data) = frame.into_data() {
            let observed = bytes.len() as u64 + data.len() as u64;
            if observed > max_response_bytes {
                return Err(HttpError::ResponseTooLarge(observed));
            }
            bytes.extend_from_slice(&data);
        }
    }
    let headers = parts
        .headers
        .iter()
        .map(|(name, value)| HttpHeader {
            name: name.as_str().to_owned(),
            value: value.as_bytes().to_vec(),
        })
        .collect();
    let body = if bytes.is_empty() {
        HttpBody::Empty
    } else if is_textual(&parts.headers) {
        match String::from_utf8(bytes) {
            Ok(value) => HttpBody::Text(value),
            Err(error) => HttpBody::Bytes(error.into_bytes()),
        }
    } else {
        HttpBody::Bytes(bytes)
    };
    Ok(HttpResponse {
        body,
        headers,
        status: parts.status.as_u16(),
    })
}

fn request_origin(uri: &Uri) -> Option<String> {
    Some(format!("{}://{}", uri.scheme_str()?, uri.authority()?))
}

fn method(value: HttpMethod) -> Method {
    match value {
        HttpMethod::Delete => Method::DELETE,
        HttpMethod::Get => Method::GET,
        HttpMethod::Head => Method::HEAD,
        HttpMethod::Options => Method::OPTIONS,
        HttpMethod::Patch => Method::PATCH,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
    }
}

fn map_transport_error(error: ErrorCode) -> HttpError {
    HttpError::Transport(format!("{error:?}"))
}

fn is_textual(headers: &http::HeaderMap) -> bool {
    let Some(value) = headers.get(http::header::CONTENT_TYPE) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let media_type = value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase();
    media_type.starts_with("text/")
        || media_type == "application/json"
        || media_type.ends_with("+json")
}
