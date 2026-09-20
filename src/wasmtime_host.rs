//! Runnable Wasmtime host for generated buffered HTTP Components.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use cirru_edn::Edn;
use wasmtime::component::{Component, Linker, ResourceTable};
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
    wasi.inherit_stdout().inherit_stderr();
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
    if entry_type.params().len() != 0 || entry_type.results().len() != 0 {
        return Err(format!(
            "host entry {:?} must accept no parameters and return Unit",
            config.entry
        ));
    }
    store
        .run_concurrent(async |accessor| entry.call_concurrent(accessor, &[], &mut []).await)
        .await
        .map_err(|error| format!("failed to drive concurrent Component entry: {error:#}"))?
        .map_err(|error| format!("Component entry {:?} failed: {error:#}", config.entry))?;
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
            "allowed-origins",
            "preopens",
            "max-response-bytes",
        ],
        "host capability config",
    )?;
    let component = required_string(values.0.iter(), "component")?;
    let entry = required_string(values.0.iter(), "entry")?;
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
        allowed_origins,
        preopens,
        max_response_bytes,
    })
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
}
