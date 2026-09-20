//! Runnable Wasmtime host for generated buffered HTTP Components.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component as PathComponent, Path, PathBuf};

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};
use cirru_edn::Edn;
use wasmtime::component::{Component, Linker, ResourceTable, Type, Val};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::wasmtime_http::{self, WasiHttpConfig};

/// One explicitly granted host directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPreopen {
    pub host: PathBuf,
    pub guest: String,
    pub writable: bool,
}

/// Closed capability configuration consumed by the generated host crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostConfig {
    pub component: PathBuf,
    pub entry: String,
    pub arguments: Vec<Edn>,
    pub arguments_file: Option<String>,
    pub result_file: Option<String>,
    pub allowed_origins: Vec<String>,
    pub preopens: Vec<HostPreopen>,
    pub max_response_bytes: u64,
}

/// Stable process exits used by the generated buffered HTTP host.
pub mod exit_code {
    pub const SUCCESS: i32 = 0;
    pub const INTERNAL: i32 = 1;
    pub const INVALID_INPUT: i32 = 2;
    pub const CAPABILITY_DENIED: i32 = 3;
    pub const TRANSPORT: i32 = 4;
    pub const RESPONSE_TOO_LARGE: i32 = 5;
    pub const UNSUPPORTED: i32 = 6;
}

const MAX_ARGUMENTS_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
enum HostFailureKind {
    Internal,
    InvalidInput,
    CapabilityDenied,
}

#[derive(Debug)]
struct HostFailure {
    kind: HostFailureKind,
    message: String,
}

impl HostFailure {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: HostFailureKind::Internal,
            message: message.into(),
        }
    }

    fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            kind: HostFailureKind::InvalidInput,
            message: message.into(),
        }
    }

    fn capability_denied(message: impl Into<String>) -> Self {
        Self {
            kind: HostFailureKind::CapabilityDenied,
            message: message.into(),
        }
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            HostFailureKind::Internal => exit_code::INTERNAL,
            HostFailureKind::InvalidInput => exit_code::INVALID_INPUT,
            HostFailureKind::CapabilityDenied => exit_code::CAPABILITY_DENIED,
        }
    }
}

struct HostState {
    table: ResourceTable,
    wasi: WasiCtx,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Parse a Cirru EDN capability file. Relative paths are resolved beside the file.
pub fn load_config(path: impl AsRef<Path>) -> Result<HostConfig, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read host capability config {}: {error}",
            path.display()
        )
    })?;
    let value = cirru_edn::parse(&source).map_err(|error| {
        format!(
            "failed to parse Cirru EDN host capability config {}: {error}",
            path.display()
        )
    })?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    parse_config(&value, base)
}

/// Run the generated Component with default-deny network and filesystem capabilities.
pub async fn run_config_file(path: impl AsRef<Path>) -> Result<(), String> {
    run_config_file_inner(path)
        .await
        .map(|_| ())
        .map_err(|failure| failure.message)
}

/// Run one capability file with generated-host stderr and stable process semantics.
pub async fn run_config_file_with_exit_code(path: impl AsRef<Path>) -> i32 {
    match run_config_file_inner(path).await {
        Ok(code) => code,
        Err(failure) => {
            eprintln!("calcit Wasmtime host failed: {}", failure.message);
            failure.exit_code()
        }
    }
}

/// CLI entry used by the generated host crate.
pub async fn run_from_args() -> Result<(), String> {
    run_from_args_inner()
        .await
        .map(|_| ())
        .map_err(|failure| failure.message)
}

/// CLI entry used by newly generated hosts that preserve typed failure exits.
pub async fn run_from_args_with_exit_code() -> i32 {
    match run_from_args_inner().await {
        Ok(code) => code,
        Err(failure) => {
            eprintln!("calcit Wasmtime host failed: {}", failure.message);
            failure.exit_code()
        }
    }
}

async fn run_from_args_inner() -> Result<i32, HostFailure> {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let config = args.next().ok_or_else(|| {
        HostFailure::invalid_input(format!(
            "usage: {} <capabilities.cirru>",
            Path::new(&program)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("calcit-wasmtime-http-host")
        ))
    })?;
    if args.next().is_some() {
        return Err(HostFailure::invalid_input(
            "host accepts exactly one Cirru EDN capability file",
        ));
    }
    run_config_file_inner(config).await
}

async fn run_config_file_inner(path: impl AsRef<Path>) -> Result<i32, HostFailure> {
    let config = load_config(path).map_err(HostFailure::invalid_input)?;
    run(config).await
}

async fn run(mut config: HostConfig) -> Result<i32, HostFailure> {
    if let Some(arguments_file) = &config.arguments_file {
        let file = open_preopen_file(&config.preopens, arguments_file, false)?;
        config.arguments = read_arguments_file(file, arguments_file)?;
    }
    let mut result_file = config
        .result_file
        .as_deref()
        .map(|path| open_preopen_file(&config.preopens, path, true).map(|file| (file, path)))
        .transpose()?;

    let mut engine_config = Config::new();
    engine_config
        .wasm_component_model_async(true)
        .wasm_component_model_async_stackful(true)
        .wasm_component_model_more_async_builtins(true)
        .concurrency_support(true);
    let engine = Engine::new(&engine_config).map_err(|error| {
        HostFailure::internal(format!("failed to create Wasmtime engine: {error:#}"))
    })?;
    let component = Component::from_file(&engine, &config.component).map_err(|error| {
        HostFailure::internal(format!(
            "failed to load generated Component {}: {error:#}",
            config.component.display()
        ))
    })?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|error| {
        HostFailure::internal(format!("failed to link WASI capabilities: {error:#}"))
    })?;
    let mut http = WasiHttpConfig::default().max_response_bytes(config.max_response_bytes);
    for origin in &config.allowed_origins {
        http = http
            .allow_origin(origin)
            .map_err(HostFailure::invalid_input)?;
    }
    wasmtime_http::add_to_linker(&mut linker, http).map_err(|error| {
        HostFailure::internal(format!("failed to link buffered HTTP adapter: {error:#}"))
    })?;

    let mut wasi = WasiCtxBuilder::new();
    // Reserve stdout for the host's machine-readable Cirru EDN result.
    wasi.inherit_stderr();
    for preopen in &config.preopens {
        let (dir_perms, file_perms) = if preopen.writable {
            (DirPerms::all(), FilePerms::all())
        } else {
            (DirPerms::READ, FilePerms::READ)
        };
        wasi.preopened_dir(&preopen.host, &preopen.guest, dir_perms, file_perms)
            .map_err(|error| {
                HostFailure::capability_denied(format!(
                    "failed to grant preopen {} as {:?}: {error}",
                    preopen.host.display(),
                    preopen.guest
                ))
            })?;
    }
    let mut store = Store::new(
        &engine,
        HostState {
            table: ResourceTable::new(),
            wasi: wasi.build(),
        },
    );
    let instance = linker
        .instantiate_async(&mut store, &component)
        .await
        .map_err(|error| {
            HostFailure::internal(format!(
                "failed to instantiate generated Component: {error:#}"
            ))
        })?;
    let entry = instance
        .get_func(&mut store, config.entry.as_str())
        .ok_or_else(|| {
            HostFailure::invalid_input(format!(
                "generated Component has no function export {:?}",
                config.entry
            ))
        })?;
    let entry_type = entry.ty(&store);
    let parameters = entry_type.params().collect::<Vec<_>>();
    if parameters.len() != config.arguments.len() {
        return Err(HostFailure::invalid_input(format!(
            "host entry {:?} expects {} argument(s), but :arguments contains {}",
            config.entry,
            parameters.len(),
            config.arguments.len()
        )));
    }
    let arguments = parameters
        .iter()
        .zip(&config.arguments)
        .enumerate()
        .map(|(index, ((name, ty), value))| {
            decode_value(value, ty, &format!("arguments[{index}] ({name})"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(HostFailure::invalid_input)?;
    let result_types = entry_type.results().collect::<Vec<_>>();
    let mut results = vec![Val::Bool(false); result_types.len()];
    store
        .run_concurrent(async |accessor| {
            entry
                .call_concurrent(accessor, &arguments, &mut results)
                .await
        })
        .await
        .map_err(|error| {
            HostFailure::internal(format!(
                "failed to drive concurrent Component entry: {error:#}"
            ))
        })?
        .map_err(|error| {
            HostFailure::internal(format!(
                "Component entry {:?} failed: {error:#}",
                config.entry
            ))
        })?;
    let mut exit = exit_code::SUCCESS;
    if !results.is_empty() {
        let output = encode_results(&results, &result_types).map_err(HostFailure::internal)?;
        exit = classify_result_exit(&output, &result_types);
        let formatted = cirru_edn::format(&output, false).map_err(|error| {
            HostFailure::internal(format!(
                "failed to format Component result as Cirru EDN: {error}"
            ))
        })?;
        let formatted = formatted.trim();
        println!("{formatted}");
        if let Some((file, guest_path)) = result_file.as_mut() {
            file.set_len(0).map_err(|error| {
                HostFailure::capability_denied(format!(
                    "failed to truncate Cirru EDN Component result file {guest_path:?}: {error}"
                ))
            })?;
            file.write_all(format!("{formatted}\n").as_bytes())
                .map_err(|error| {
                    HostFailure::capability_denied(format!(
                        "failed to write Cirru EDN Component result to {guest_path:?}: {error}"
                    ))
                })?;
        }
    }
    Ok(exit)
}

fn parse_config(value: &Edn, base: &Path) -> Result<HostConfig, String> {
    let Edn::Map(values) = value else {
        return Err("host capability config must be a Cirru EDN map".to_owned());
    };
    validate_fields(
        values.0.iter(),
        &[
            "component",
            "entry",
            "arguments",
            "arguments-file",
            "result-file",
            "allowed-origins",
            "preopens",
            "max-response-bytes",
        ],
        "host capability config",
    )?;
    let component = required_string(values.0.iter(), "component")?;
    let entry = required_string(values.0.iter(), "entry")?;
    let arguments = optional_list(values.0.iter(), "arguments")?.to_vec();
    let arguments_file = optional_string(values.0.iter(), "arguments-file")?;
    if arguments_file.is_some() && !arguments.is_empty() {
        return Err(
            ":arguments-file cannot be combined with non-empty inline :arguments".to_owned(),
        );
    }
    let result_file = optional_string(values.0.iter(), "result-file")?;
    let max_response_bytes = required_u64(values.0.iter(), "max-response-bytes")?;
    let allowed_origins = optional_list(values.0.iter(), "allowed-origins")?
        .iter()
        .enumerate()
        .map(|(index, value)| edn_string(value, &format!("allowed-origins[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let preopens = optional_list(values.0.iter(), "preopens")?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_preopen(value, base, index))
        .collect::<Result<Vec<_>, _>>()?;
    validate_preopen_guest_paths(&preopens)?;
    Ok(HostConfig {
        component: resolve_path(base, component),
        entry,
        arguments,
        arguments_file,
        result_file,
        allowed_origins,
        preopens,
        max_response_bytes,
    })
}

fn validate_preopen_guest_paths(preopens: &[HostPreopen]) -> Result<(), String> {
    let mut seen = Vec::new();
    for (index, preopen) in preopens.iter().enumerate() {
        let normalized = guest_path_components(&preopen.guest, "preopen guest path")
            .map_err(|failure| format!("preopens[{index}].guest: {}", failure.message))?;
        if seen.contains(&normalized) {
            return Err(format!(
                "preopens[{index}].guest duplicates another normalized guest path"
            ));
        }
        seen.push(normalized);
    }
    Ok(())
}

fn read_arguments_file(mut file: File, guest_path: &str) -> Result<Vec<Edn>, HostFailure> {
    let metadata = file.metadata().map_err(|error| {
        input_file_error(
            format!("failed to inspect Cirru EDN arguments file {guest_path:?}"),
            error,
        )
    })?;
    if metadata.len() > MAX_ARGUMENTS_FILE_BYTES {
        return Err(HostFailure::invalid_input(format!(
            "Cirru EDN arguments file {guest_path:?} exceeds the {MAX_ARGUMENTS_FILE_BYTES} byte limit"
        )));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_ARGUMENTS_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            input_file_error(
                format!("failed to read Cirru EDN arguments file {guest_path:?}"),
                error,
            )
        })?;
    if bytes.len() as u64 > MAX_ARGUMENTS_FILE_BYTES {
        return Err(HostFailure::invalid_input(format!(
            "Cirru EDN arguments file {guest_path:?} exceeds the {MAX_ARGUMENTS_FILE_BYTES} byte limit"
        )));
    }
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        HostFailure::invalid_input(format!(
            "Cirru EDN arguments file {guest_path:?} is not UTF-8: {error}"
        ))
    })?;
    let value = cirru_edn::parse(source).map_err(|error| {
        HostFailure::invalid_input(format!(
            "failed to parse Cirru EDN arguments file {guest_path:?}: {error}"
        ))
    })?;
    match value {
        Edn::List(values) => Ok(values.0.to_vec()),
        _ => Err(HostFailure::invalid_input(format!(
            "Cirru EDN arguments file {guest_path:?} must contain one top-level list"
        ))),
    }
}

fn input_file_error(context: String, error: std::io::Error) -> HostFailure {
    let message = format!("{context}: {error}");
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        HostFailure::capability_denied(message)
    } else {
        HostFailure::invalid_input(message)
    }
}

fn open_preopen_file(
    preopens: &[HostPreopen],
    guest_path: &str,
    write: bool,
) -> Result<File, HostFailure> {
    let requested = guest_path_components(guest_path, "file path")?;
    let mut matches = Vec::new();
    for preopen in preopens {
        let prefix = guest_path_components(&preopen.guest, "preopen guest path")?;
        if requested.starts_with(&prefix) {
            matches.push((prefix.len(), preopen));
        }
    }
    matches.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
    let Some((prefix_length, preopen)) = matches.first().copied() else {
        return Err(HostFailure::capability_denied(format!(
            "guest path {guest_path:?} is not granted by any :preopens entry"
        )));
    };
    if write && !preopen.writable {
        return Err(HostFailure::capability_denied(format!(
            "guest path {guest_path:?} requires a :read-write preopen"
        )));
    }

    let root = Dir::open_ambient_dir(&preopen.host, ambient_authority()).map_err(|error| {
        HostFailure::capability_denied(format!(
            "failed to open preopen host directory {}: {error}",
            preopen.host.display()
        ))
    })?;
    let relative = requested[prefix_length..]
        .iter()
        .fold(PathBuf::new(), |path, segment| path.join(segment));
    if relative.as_os_str().is_empty() {
        return Err(HostFailure::capability_denied(format!(
            "guest path {guest_path:?} must name a file below its preopen"
        )));
    }
    let mut options = OpenOptions::new();
    options.follow(FollowSymlinks::No);
    if write {
        options.write(true).create(true);
    } else {
        options.read(true);
    }
    root.open_with(&relative, &options).map_err(|error| {
        if write {
            HostFailure::capability_denied(format!(
                "failed to open Cirru EDN result file {guest_path:?}: {error}"
            ))
        } else {
            input_file_error(
                format!("failed to open Cirru EDN arguments file {guest_path:?}"),
                error,
            )
        }
    })
}

fn guest_path_components(path: &str, context: &str) -> Result<Vec<String>, HostFailure> {
    let mut output = Vec::new();
    for component in Path::new(path).components() {
        match component {
            PathComponent::RootDir | PathComponent::CurDir => {}
            PathComponent::Normal(segment) => {
                let segment = segment.to_str().ok_or_else(|| {
                    HostFailure::capability_denied(format!("{context} must be valid UTF-8"))
                })?;
                output.push(segment.to_owned());
            }
            PathComponent::ParentDir => {
                return Err(HostFailure::capability_denied(format!(
                    "{context} cannot contain .. traversal"
                )));
            }
            PathComponent::Prefix(_) => {
                return Err(HostFailure::capability_denied(format!(
                    "{context} cannot use a host path prefix"
                )));
            }
        }
    }
    Ok(output)
}

fn classify_result_exit(output: &Edn, result_types: &[Type]) -> i32 {
    let Edn::Enum(result) = output else {
        return exit_code::SUCCESS;
    };
    if result.type_name.is_some() || result.variant.as_ref() != "err" {
        return exit_code::SUCCESS;
    }
    if !is_closed_http_result(result_types) {
        return exit_code::INTERNAL;
    }
    let [Edn::Enum(error)] = result.extra.as_slice() else {
        return exit_code::INTERNAL;
    };
    http_error_exit(error.variant.as_ref())
}

fn is_closed_http_result(result_types: &[Type]) -> bool {
    let [Type::Result(result)] = result_types else {
        return false;
    };
    let Some(Type::Variant(error)) = result.err() else {
        return false;
    };
    let mut cases = error
        .cases()
        .map(|case| case.name.to_owned())
        .collect::<Vec<_>>();
    cases.sort();
    cases
        == [
            "capability-denied",
            "invalid-request",
            "response-too-large",
            "transport",
            "unsupported",
        ]
}

fn http_error_exit(case: &str) -> i32 {
    match case {
        "invalid-request" => exit_code::INVALID_INPUT,
        "capability-denied" => exit_code::CAPABILITY_DENIED,
        "transport" => exit_code::TRANSPORT,
        "response-too-large" => exit_code::RESPONSE_TOO_LARGE,
        "unsupported" => exit_code::UNSUPPORTED,
        _ => exit_code::INTERNAL,
    }
}

const MAX_LOSSLESS_INTEGER: f64 = 9_007_199_254_740_991.0;

fn decode_value(value: &Edn, ty: &Type, path: &str) -> Result<Val, String> {
    match ty {
        Type::Bool => match value {
            Edn::Bool(value) => Ok(Val::Bool(*value)),
            _ => Err(format!("{path} must be a Bool")),
        },
        Type::S8 => decode_signed(value, i8::MIN as i64, i8::MAX as i64, path)
            .map(|value| Val::S8(value as i8)),
        Type::U8 => decode_unsigned(value, u8::MAX as u64, path).map(|value| Val::U8(value as u8)),
        Type::S16 => decode_signed(value, i16::MIN as i64, i16::MAX as i64, path)
            .map(|value| Val::S16(value as i16)),
        Type::U16 => {
            decode_unsigned(value, u16::MAX as u64, path).map(|value| Val::U16(value as u16))
        }
        Type::S32 => decode_signed(value, i32::MIN as i64, i32::MAX as i64, path)
            .map(|value| Val::S32(value as i32)),
        Type::U32 => {
            decode_unsigned(value, u32::MAX as u64, path).map(|value| Val::U32(value as u32))
        }
        Type::S64 => decode_i64(value, path).map(Val::S64),
        Type::U64 => decode_u64(value, path).map(Val::U64),
        Type::Float32 => decode_float32(value, path).map(Val::Float32),
        Type::Float64 => decode_float(value, path).map(Val::Float64),
        Type::String => edn_string(value, path).map(Val::String),
        Type::List(list) if list.ty() == Type::U8 => match value {
            Edn::Buffer(value) => Ok(Val::List(value.iter().copied().map(Val::U8).collect())),
            _ => Err(format!("{path} must be a Buffer for list<u8>")),
        },
        Type::List(list) => {
            let Edn::List(values) = value else {
                return Err(format!("{path} must be a List"));
            };
            values
                .0
                .iter()
                .enumerate()
                .map(|(index, value)| decode_value(value, &list.ty(), &format!("{path}[{index}]")))
                .collect::<Result<Vec<_>, _>>()
                .map(Val::List)
        }
        Type::Record(record) => decode_record(value, record, path),
        Type::Variant(variant) => decode_variant(value, variant, path),
        Type::Enum(enum_type) => decode_enum(value, enum_type, path),
        Type::Option(option) => decode_option(value, &option.ty(), path),
        Type::Result(result) => decode_result(value, result, path),
        _ => Err(format!("{path} uses unsupported Component type {ty:?}")),
    }
}

fn decode_signed(value: &Edn, min: i64, max: i64, path: &str) -> Result<i64, String> {
    let Edn::Number(value) = value else {
        return Err(format!("{path} must be an integer"));
    };
    if !value.is_finite() || value.fract() != 0.0 || *value < min as f64 || *value > max as f64 {
        return Err(format!("{path} must be an integer in {min}..={max}"));
    }
    Ok(*value as i64)
}

fn decode_unsigned(value: &Edn, max: u64, path: &str) -> Result<u64, String> {
    let Edn::Number(value) = value else {
        return Err(format!("{path} must be an unsigned integer"));
    };
    if !value.is_finite() || value.fract() != 0.0 || *value < 0.0 || *value > max as f64 {
        return Err(format!("{path} must be an unsigned integer in 0..={max}"));
    }
    Ok(*value as u64)
}

fn decode_i64(value: &Edn, path: &str) -> Result<i64, String> {
    if let Edn::Number(value) = value
        && value.is_finite()
        && value.fract() == 0.0
        && value.abs() <= MAX_LOSSLESS_INTEGER
    {
        return Ok(*value as i64);
    }
    decode_explicit_integer(value, "s64", path)?
        .parse::<i64>()
        .map_err(|_| format!("{path} must contain an exact s64 decimal string"))
}

fn decode_u64(value: &Edn, path: &str) -> Result<u64, String> {
    if let Edn::Number(value) = value
        && value.is_finite()
        && value.fract() == 0.0
        && *value >= 0.0
        && *value <= MAX_LOSSLESS_INTEGER
    {
        return Ok(*value as u64);
    }
    decode_explicit_integer(value, "u64", path)?
        .parse::<u64>()
        .map_err(|_| format!("{path} must contain an exact u64 decimal string"))
}

fn decode_explicit_integer<'a>(value: &'a Edn, width: &str, path: &str) -> Result<&'a str, String> {
    let Edn::Enum(value) = value else {
        return Err(format!(
            "{path} must be a lossless Number or :: :{width} |decimal"
        ));
    };
    match (
        value.type_name.as_ref(),
        value.variant.as_ref(),
        value.extra.as_slice(),
    ) {
        (None, variant, [Edn::Str(decimal)]) if variant == width => Ok(decimal),
        _ => Err(format!(
            "{path} must be a lossless Number or :: :{width} |decimal"
        )),
    }
}

fn decode_float(value: &Edn, path: &str) -> Result<f64, String> {
    match value {
        Edn::Number(value) if value.is_finite() => Ok(*value),
        _ => Err(format!("{path} must be a finite Number")),
    }
}

fn decode_float32(value: &Edn, path: &str) -> Result<f32, String> {
    let value = decode_float(value, path)? as f32;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{path} must fit a finite float32"))
    }
}

fn decode_record(
    value: &Edn,
    record: &wasmtime::component::types::Record,
    path: &str,
) -> Result<Val, String> {
    let Edn::Map(values) = value else {
        return Err(format!("{path} must be a map with tag keys"));
    };
    let fields = record.fields().collect::<Vec<_>>();
    for key in values.0.keys() {
        let Edn::Tag(key) = key else {
            return Err(format!("{path} keys must be tags"));
        };
        if !fields.iter().any(|field| field.name == key.ref_str()) {
            return Err(format!("{path} contains unknown field :{}", key.ref_str()));
        }
    }
    fields
        .into_iter()
        .map(|field| {
            let value = values
                .0
                .get(&Edn::tag(field.name))
                .ok_or_else(|| format!("{path} requires field :{}", field.name))?;
            decode_value(value, &field.ty, &format!("{path}.{}", field.name))
                .map(|value| (field.name.to_owned(), value))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Val::Record)
}

fn decode_variant(
    value: &Edn,
    variant: &wasmtime::component::types::Variant,
    path: &str,
) -> Result<Val, String> {
    let Edn::Enum(value) = value else {
        return Err(format!("{path} must be a Cirru EDN enum"));
    };
    if value.type_name.is_some() {
        return Err(format!("{path} must use an unqualified :: variant"));
    }
    let case = variant
        .cases()
        .find(|case| case.name == value.variant.as_ref())
        .ok_or_else(|| format!("{path} contains unknown case :{}", value.variant))?;
    let payload = match case.ty {
        Some(ty) if value.extra.len() == 1 => Some(Box::new(decode_value(
            &value.extra[0],
            &ty,
            &format!("{path}.{}", case.name),
        )?)),
        Some(_) => return Err(format!("{path}.{} requires exactly one payload", case.name)),
        None if value.extra.is_empty() => None,
        None => return Err(format!("{path}.{} accepts no payload", case.name)),
    };
    Ok(Val::Variant(case.name.to_owned(), payload))
}

fn decode_enum(
    value: &Edn,
    enum_type: &wasmtime::component::types::Enum,
    path: &str,
) -> Result<Val, String> {
    let Edn::Enum(value) = value else {
        return Err(format!("{path} must be a Cirru EDN enum"));
    };
    if value.type_name.is_some() || !value.extra.is_empty() {
        return Err(format!(
            "{path} must be an unqualified payload-free :: case"
        ));
    }
    let case = enum_type
        .names()
        .find(|case| *case == value.variant.as_ref())
        .ok_or_else(|| format!("{path} contains unknown case :{}", value.variant))?;
    Ok(Val::Enum(case.to_owned()))
}

fn decode_option(value: &Edn, ty: &Type, path: &str) -> Result<Val, String> {
    let Edn::Enum(value) = value else {
        return Err(format!("{path} must be :: :none or :: :some value"));
    };
    if value.type_name.is_some() {
        return Err(format!("{path} must use an unqualified :: case"));
    }
    match (value.variant.as_ref(), value.extra.as_slice()) {
        ("none", []) => Ok(Val::Option(None)),
        ("some", [payload]) => decode_value(payload, ty, &format!("{path}.some"))
            .map(|value| Val::Option(Some(Box::new(value)))),
        _ => Err(format!("{path} must be :: :none or :: :some value")),
    }
}

fn decode_result(
    value: &Edn,
    result: &wasmtime::component::types::ResultType,
    path: &str,
) -> Result<Val, String> {
    let Edn::Enum(value) = value else {
        return Err(format!("{path} must be a :: :ok or :: :err value"));
    };
    if value.type_name.is_some() {
        return Err(format!("{path} must use an unqualified :: case"));
    }
    let decoded = match value.variant.as_ref() {
        "ok" => Ok(decode_optional_payload(
            &value.extra,
            result.ok(),
            &format!("{path}.ok"),
        )?),
        "err" => Err(decode_optional_payload(
            &value.extra,
            result.err(),
            &format!("{path}.err"),
        )?),
        _ => return Err(format!("{path} must be a :: :ok or :: :err value")),
    };
    Ok(Val::Result(decoded))
}

fn decode_optional_payload(
    values: &[Edn],
    ty: Option<Type>,
    path: &str,
) -> Result<Option<Box<Val>>, String> {
    match (ty, values) {
        (None, []) => Ok(None),
        (Some(ty), [value]) => decode_value(value, &ty, path).map(|value| Some(Box::new(value))),
        (None, _) => Err(format!("{path} accepts no payload")),
        (Some(_), _) => Err(format!("{path} requires exactly one payload")),
    }
}

fn encode_results(results: &[Val], types: &[Type]) -> Result<Edn, String> {
    if results.len() != types.len() {
        return Err("Component result count changed after invocation".to_owned());
    }
    match (results, types) {
        ([value], [ty]) => encode_value(value, ty, "result"),
        (values, types) => values
            .iter()
            .zip(types)
            .enumerate()
            .map(|(index, (value, ty))| encode_value(value, ty, &format!("results[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Edn::List(values.into())),
    }
}

fn encode_value(value: &Val, ty: &Type, path: &str) -> Result<Edn, String> {
    match (value, ty) {
        (Val::Bool(value), Type::Bool) => Ok(Edn::Bool(*value)),
        (Val::S8(value), Type::S8) => Ok(Edn::Number(*value as f64)),
        (Val::U8(value), Type::U8) => Ok(Edn::Number(*value as f64)),
        (Val::S16(value), Type::S16) => Ok(Edn::Number(*value as f64)),
        (Val::U16(value), Type::U16) => Ok(Edn::Number(*value as f64)),
        (Val::S32(value), Type::S32) => Ok(Edn::Number(*value as f64)),
        (Val::U32(value), Type::U32) => Ok(Edn::Number(*value as f64)),
        (Val::S64(value), Type::S64) => Ok(encode_i64(*value)),
        (Val::U64(value), Type::U64) => Ok(encode_u64(*value)),
        (Val::Float32(value), Type::Float32) if value.is_finite() => Ok(Edn::Number(*value as f64)),
        (Val::Float64(value), Type::Float64) if value.is_finite() => Ok(Edn::Number(*value)),
        (Val::String(value), Type::String) => Ok(Edn::str(value.as_str())),
        (Val::List(values), Type::List(list)) if list.ty() == Type::U8 => values
            .iter()
            .map(|value| match value {
                Val::U8(value) => Ok(*value),
                _ => Err(format!("{path} contains a non-u8 value")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Edn::Buffer),
        (Val::List(values), Type::List(list)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| encode_value(value, &list.ty(), &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Edn::List(values.into())),
        (Val::Record(fields), Type::Record(record)) => record
            .fields()
            .map(|field| {
                let value = fields
                    .iter()
                    .find_map(|(name, value)| (name == field.name).then_some(value))
                    .ok_or_else(|| format!("{path} is missing result field :{}", field.name))?;
                encode_value(value, &field.ty, &format!("{path}.{}", field.name))
                    .map(|value| (Edn::tag(field.name), value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Edn::map_from_iter),
        (Val::Variant(case, payload), Type::Variant(variant)) => {
            let case_type = variant
                .cases()
                .find(|candidate| candidate.name == case)
                .ok_or_else(|| format!("{path} contains unknown result case :{case}"))?
                .ty;
            encode_case(case, payload.as_deref(), case_type.as_ref(), path)
        }
        (Val::Enum(case), Type::Enum(enum_type)) if enum_type.names().any(|name| name == case) => {
            Ok(Edn::enum_value(case.as_str(), vec![]))
        }
        (Val::Option(None), Type::Option(_)) => Ok(Edn::enum_value("none", vec![])),
        (Val::Option(Some(value)), Type::Option(option)) => Ok(Edn::enum_value(
            "some",
            vec![encode_value(value, &option.ty(), &format!("{path}.some"))?],
        )),
        (Val::Result(Ok(value)), Type::Result(result)) => {
            encode_case("ok", value.as_deref(), result.ok().as_ref(), path)
        }
        (Val::Result(Err(value)), Type::Result(result)) => {
            encode_case("err", value.as_deref(), result.err().as_ref(), path)
        }
        _ => Err(format!(
            "{path} cannot encode Component value {value:?} as {ty:?}"
        )),
    }
}

fn encode_i64(value: i64) -> Edn {
    if value.unsigned_abs() <= MAX_LOSSLESS_INTEGER as u64 {
        Edn::Number(value as f64)
    } else {
        Edn::enum_value("s64", vec![Edn::str(value.to_string())])
    }
}

fn encode_u64(value: u64) -> Edn {
    if value <= MAX_LOSSLESS_INTEGER as u64 {
        Edn::Number(value as f64)
    } else {
        Edn::enum_value("u64", vec![Edn::str(value.to_string())])
    }
}

fn encode_case(
    case: &str,
    payload: Option<&Val>,
    payload_type: Option<&Type>,
    path: &str,
) -> Result<Edn, String> {
    let extra = match (payload, payload_type) {
        (None, None) => vec![],
        (Some(value), Some(ty)) => vec![encode_value(value, ty, &format!("{path}.{case}"))?],
        _ => {
            return Err(format!(
                "{path}.{case} result payload does not match its type"
            ));
        }
    };
    Ok(Edn::enum_value(case, extra))
}

fn parse_preopen(value: &Edn, base: &Path, index: usize) -> Result<HostPreopen, String> {
    let Edn::Map(values) = value else {
        return Err(format!("preopens[{index}] must be a map"));
    };
    validate_fields(
        values.0.iter(),
        &["host", "guest", "access"],
        &format!("preopens[{index}]"),
    )?;
    let host = required_string(values.0.iter(), "host")?;
    let guest = required_string(values.0.iter(), "guest")?;
    let access = required_tag(values.0.iter(), "access")?;
    let writable = match access.as_str() {
        "read" => false,
        "read-write" => true,
        _ => {
            return Err(format!(
                "preopens[{index}].access must be :read or :read-write"
            ));
        }
    };
    Ok(HostPreopen {
        host: resolve_path(base, host),
        guest,
        writable,
    })
}

fn validate_fields<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    let mut seen = Vec::new();
    for (key, _) in values {
        let Edn::Tag(tag) = key else {
            return Err(format!("{context} keys must be tags"));
        };
        let name = tag.ref_str();
        if !allowed.contains(&name) {
            return Err(format!("{context} contains unknown field :{name}"));
        }
        if seen.contains(&name) {
            return Err(format!("{context} contains duplicate field :{name}"));
        }
        seen.push(name);
    }
    Ok(())
}

fn find_field<'a>(
    mut values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Option<&'a Edn> {
    values.find_map(|(key, value)| match key {
        Edn::Tag(tag) if tag.ref_str() == name => Some(value),
        _ => None,
    })
}

fn required_string<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Result<String, String> {
    let value = find_field(values, name)
        .ok_or_else(|| format!("host capability config requires :{name}"))?;
    edn_string(value, name)
}

fn required_tag<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Result<String, String> {
    match find_field(values, name) {
        Some(Edn::Tag(value)) => Ok(value.ref_str().to_owned()),
        Some(_) => Err(format!(":{name} must be a tag")),
        None => Err(format!("host capability config requires :{name}")),
    }
}

fn required_u64<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Result<u64, String> {
    match find_field(values, name) {
        Some(Edn::Number(value))
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= 0.0
                && *value <= 9_007_199_254_740_991.0 =>
        {
            Ok(*value as u64)
        }
        Some(_) => Err(format!(":{name} must be a non-negative lossless integer")),
        None => Err(format!("host capability config requires :{name}")),
    }
}

fn optional_list<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Result<&'a [Edn], String> {
    match find_field(values, name) {
        Some(Edn::List(values)) => Ok(&values.0),
        Some(_) => Err(format!(":{name} must be a list")),
        None => Ok(&[]),
    }
}

fn optional_string<'a>(
    values: impl Iterator<Item = (&'a Edn, &'a Edn)>,
    name: &str,
) -> Result<Option<String>, String> {
    match find_field(values, name) {
        Some(value) => edn_string(value, name).map(Some),
        None => Ok(None),
    }
}

fn edn_string(value: &Edn, path: &str) -> Result<String, String> {
    match value {
        Edn::Str(value) => Ok(value.to_string()),
        _ => Err(format!("{path} must be a string")),
    }
}

fn resolve_path(base: &Path, value: String) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_default_deny_capabilities_and_explicit_preopens() {
        let value = cirru_edn::parse(
            r#"{}
  :component |component/app.wasm
  :entry |run
  :max-response-bytes 4096
  :allowed-origins $ [] |https://api.example.com
  :preopens $ []
    {} (:host |workspace) (:guest |/workspace) (:access :read-write)
"#,
        )
        .expect("parse host config");
        let config = parse_config(&value, Path::new("/project")).expect("decode host config");
        assert_eq!(config.component, Path::new("/project/component/app.wasm"));
        assert_eq!(config.entry, "run");
        assert!(config.arguments.is_empty());
        assert_eq!(config.arguments_file, None);
        assert_eq!(config.result_file, None);
        assert_eq!(config.allowed_origins, vec!["https://api.example.com"]);
        assert_eq!(config.max_response_bytes, 4096);
        assert_eq!(
            config.preopens,
            vec![HostPreopen {
                host: PathBuf::from("/project/workspace"),
                guest: "/workspace".to_owned(),
                writable: true,
            }]
        );
    }

    #[test]
    fn rejects_implicit_or_open_capabilities() {
        let value = cirru_edn::parse("{} (:component |app.wasm) (:entry |run)")
            .expect("parse incomplete config");
        assert!(
            parse_config(&value, Path::new("."))
                .unwrap_err()
                .contains(":max-response-bytes")
        );

        let value = cirru_edn::parse(
            "{} (:component |app.wasm) (:entry |run) (:max-response-bytes 64) (:preopens ([] ({} (:host |.) (:guest |/) (:access :all))))",
        )
        .expect("parse invalid access config");
        assert!(
            parse_config(&value, Path::new("."))
                .unwrap_err()
                .contains(":read or :read-write")
        );

        let value = cirru_edn::parse(
            "{} (:component |app.wasm) (:entry |run) (:max-response-bytes 64) (:allow-all-network true)",
        )
        .expect("parse unknown capability config");
        assert!(
            parse_config(&value, Path::new("."))
                .unwrap_err()
                .contains("unknown field :allow-all-network")
        );

        let value = cirru_edn::parse(
            r#"{}
  :component |app.wasm
  :entry |run
  :max-response-bytes 64
  :preopens $ []
    {} (:host |one) (:guest |/data) (:access :read)
    {} (:host |two) (:guest |data/.) (:access :read-write)
"#,
        )
        .expect("parse duplicate guest paths");
        assert!(
            parse_config(&value, Path::new("."))
                .unwrap_err()
                .contains("duplicates another normalized guest path")
        );
    }

    #[test]
    fn preserves_full_width_integers_with_explicit_cirru_edn_cases() {
        let encoded = encode_u64(u64::MAX);
        assert_eq!(decode_u64(&encoded, "argument"), Ok(u64::MAX));

        let encoded = encode_i64(i64::MIN);
        assert_eq!(decode_i64(&encoded, "argument"), Ok(i64::MIN));
    }

    #[test]
    fn parses_file_backed_arguments_and_results_without_inline_ambiguity() {
        let value = cirru_edn::parse(
            r#"{}
  :component |component.wasm
  :entry |run
  :arguments $ []
  :arguments-file |/input/request.cirru
  :result-file |/output/result.cirru
  :max-response-bytes 4096
  :preopens $ []
"#,
        )
        .expect("parse file-backed config");
        let config = parse_config(&value, Path::new("/project")).expect("decode file config");
        assert_eq!(
            config.arguments_file.as_deref(),
            Some("/input/request.cirru")
        );
        assert_eq!(config.result_file.as_deref(), Some("/output/result.cirru"));

        let ambiguous = cirru_edn::parse(
            r#"{}
  :component |component.wasm
  :entry |run
  :arguments $ [] |inline
  :arguments-file |/input/request.cirru
  :max-response-bytes 4096
"#,
        )
        .expect("parse ambiguous config");
        assert!(
            parse_config(&ambiguous, Path::new("/project"))
                .unwrap_err()
                .contains("cannot be combined")
        );
    }

    #[test]
    fn resolves_only_authorized_guest_paths_with_required_access() {
        let directory = tempdir().expect("create preopen root");
        let input = directory.path().join("input.cirru");
        fs::write(&input, "[]").expect("write input");
        let read_only = HostPreopen {
            host: directory.path().to_path_buf(),
            guest: "/data".to_owned(),
            writable: false,
        };
        open_preopen_file(std::slice::from_ref(&read_only), "/data/input.cirru", false)
            .expect("open readable input");
        assert_eq!(
            open_preopen_file(std::slice::from_ref(&read_only), "/data/result.cirru", true)
                .unwrap_err()
                .exit_code(),
            exit_code::CAPABILITY_DENIED
        );
        assert_eq!(
            open_preopen_file(
                std::slice::from_ref(&read_only),
                "/other/input.cirru",
                false
            )
            .unwrap_err()
            .exit_code(),
            exit_code::CAPABILITY_DENIED
        );
        assert_eq!(
            open_preopen_file(std::slice::from_ref(&read_only), "/data/../secret", false)
                .unwrap_err()
                .exit_code(),
            exit_code::CAPABILITY_DENIED
        );

        let writable = HostPreopen {
            writable: true,
            ..read_only
        };
        open_preopen_file(&[writable], "/data/result.cirru", true).expect("open writable output");
        assert!(directory.path().join("result.cirru").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_preopen_symlink_escape_for_input_and_output() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("create symlink test root");
        let granted = directory.path().join("granted");
        let outside = directory.path().join("outside");
        fs::create_dir(&granted).expect("create granted directory");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("input.cirru"), "[]").expect("write outside input");
        symlink(&outside, granted.join("escape")).expect("create escape symlink");
        let preopen = HostPreopen {
            host: granted,
            guest: "/data".to_owned(),
            writable: true,
        };
        for (path, write) in [
            ("/data/escape/input.cirru", false),
            ("/data/escape/result.cirru", true),
        ] {
            let error = open_preopen_file(std::slice::from_ref(&preopen), path, write)
                .expect_err("reject symlink escape");
            assert_eq!(error.exit_code(), exit_code::CAPABILITY_DENIED);
            assert!(error.message.contains("failed to open"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn retained_result_handle_cannot_be_redirected_after_validation() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("create retained handle root");
        let outside = directory.path().join("outside.cirru");
        let result = directory.path().join("result.cirru");
        fs::write(&outside, "outside").expect("write outside sentinel");
        let preopen = HostPreopen {
            host: directory.path().to_path_buf(),
            guest: "/data".to_owned(),
            writable: true,
        };
        let mut file = open_preopen_file(&[preopen], "/data/result.cirru", true)
            .expect("retain validated result handle");
        fs::remove_file(&result).expect("unlink validated result path");
        symlink(&outside, &result).expect("replace result path with symlink");

        file.write_all(b"safe").expect("write retained file handle");

        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
        assert_eq!(fs::read_to_string(&result).unwrap(), "outside");
    }

    #[test]
    fn maps_input_permission_failures_to_capability_denied() {
        let error = input_file_error(
            "read input".to_owned(),
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert_eq!(error.exit_code(), exit_code::CAPABILITY_DENIED);
    }

    #[test]
    fn maps_closed_http_failures_to_stable_exit_codes() {
        for (case, expected) in [
            ("invalid-request", exit_code::INVALID_INPUT),
            ("capability-denied", exit_code::CAPABILITY_DENIED),
            ("transport", exit_code::TRANSPORT),
            ("response-too-large", exit_code::RESPONSE_TOO_LARGE),
            ("unsupported", exit_code::UNSUPPORTED),
        ] {
            assert_eq!(http_error_exit(case), expected, "case {case}");
        }
        let success =
            cirru_edn::parse(":: :ok $ {} (:status 200)").expect("parse typed HTTP success");
        assert_eq!(classify_result_exit(&success, &[]), exit_code::SUCCESS);
        let untyped_error = cirru_edn::parse(":: :err $ :: :transport |detail")
            .expect("parse untyped transport error");
        assert_eq!(
            classify_result_exit(&untyped_error, &[]),
            exit_code::INTERNAL
        );
    }
}
