//! Runnable Wasmtime host for generated buffered HTTP Components.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub allowed_origins: Vec<String>,
    pub preopens: Vec<HostPreopen>,
    pub max_response_bytes: u64,
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
    let config = load_config(path)?;
    run(config).await
}

/// CLI entry used by the generated host crate.
pub async fn run_from_args() -> Result<(), String> {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let config = args.next().ok_or_else(|| {
        format!(
            "usage: {} <capabilities.cirru>",
            Path::new(&program)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("calcit-wasmtime-http-host")
        )
    })?;
    if args.next().is_some() {
        return Err("host accepts exactly one Cirru EDN capability file".to_owned());
    }
    run_config_file(config).await
}

async fn run(config: HostConfig) -> Result<(), String> {
    let mut engine_config = Config::new();
    engine_config
        .wasm_component_model_async(true)
        .wasm_component_model_async_stackful(true)
        .wasm_component_model_more_async_builtins(true)
        .concurrency_support(true);
    let engine = Engine::new(&engine_config)
        .map_err(|error| format!("failed to create Wasmtime engine: {error:#}"))?;
    let component = Component::from_file(&engine, &config.component).map_err(|error| {
        format!(
            "failed to load generated Component {}: {error:#}",
            config.component.display()
        )
    })?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)
        .map_err(|error| format!("failed to link WASI capabilities: {error:#}"))?;
    let mut http = WasiHttpConfig::default().max_response_bytes(config.max_response_bytes);
    for origin in &config.allowed_origins {
        http = http.allow_origin(origin)?;
    }
    wasmtime_http::add_to_linker(&mut linker, http)
        .map_err(|error| format!("failed to link buffered HTTP adapter: {error:#}"))?;

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
                format!(
                    "failed to grant preopen {} as {:?}: {error}",
                    preopen.host.display(),
                    preopen.guest
                )
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
        .map_err(|error| format!("failed to instantiate generated Component: {error:#}"))?;
    let entry = instance
        .get_func(&mut store, config.entry.as_str())
        .ok_or_else(|| {
            format!(
                "generated Component has no function export {:?}",
                config.entry
            )
        })?;
    let entry_type = entry.ty(&store);
    let parameters = entry_type.params().collect::<Vec<_>>();
    if parameters.len() != config.arguments.len() {
        return Err(format!(
            "host entry {:?} expects {} argument(s), but :arguments contains {}",
            config.entry,
            parameters.len(),
            config.arguments.len()
        ));
    }
    let arguments = parameters
        .iter()
        .zip(&config.arguments)
        .enumerate()
        .map(|(index, ((name, ty), value))| {
            decode_value(value, ty, &format!("arguments[{index}] ({name})"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_types = entry_type.results().collect::<Vec<_>>();
    let mut results = vec![Val::Bool(false); result_types.len()];
    store
        .run_concurrent(async |accessor| {
            entry
                .call_concurrent(accessor, &arguments, &mut results)
                .await
        })
        .await
        .map_err(|error| format!("failed to drive concurrent Component entry: {error:#}"))?
        .map_err(|error| format!("Component entry {:?} failed: {error:#}", config.entry))?;
    if !results.is_empty() {
        let output = encode_results(&results, &result_types)?;
        let formatted = cirru_edn::format(&output, false)
            .map_err(|error| format!("failed to format Component result as Cirru EDN: {error}"))?;
        println!("{}", formatted.trim());
    }
    Ok(())
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
            "allowed-origins",
            "preopens",
            "max-response-bytes",
        ],
        "host capability config",
    )?;
    let component = required_string(values.0.iter(), "component")?;
    let entry = required_string(values.0.iter(), "entry")?;
    let arguments = optional_list(values.0.iter(), "arguments")?.to_vec();
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
    Ok(HostConfig {
        component: resolve_path(base, component),
        entry,
        arguments,
        allowed_origins,
        preopens,
        max_response_bytes,
    })
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
    }

    #[test]
    fn preserves_full_width_integers_with_explicit_cirru_edn_cases() {
        let encoded = encode_u64(u64::MAX);
        assert_eq!(decode_u64(&encoded, "argument"), Ok(u64::MAX));

        let encoded = encode_i64(i64::MIN);
        assert_eq!(decode_i64(&encoded, "argument"), Ok(i64::MIN));
    }
}
