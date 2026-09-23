use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use wasm_encoder::reencode::{Error as ReencodeError, Reencode};
use wasm_encoder::{ImportSection, Module};
use wasmparser::{Encoding, ExternalKind, FuncType, Parser, Payload, TypeRef, Validator};
use wit_component::{ComponentEncoder, StringEncoding, embed_component_metadata};
use wit_parser::Resolve;

use crate::ComponentDocument;

pub(crate) struct ComponentArtifacts {
    pub wit: String,
    pub component: Vec<u8>,
    pub lifecycle_surface: LifecycleSurface,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleSurface {
    pub imports: Vec<LifecycleImport>,
    pub exports: Vec<LifecycleExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleImport {
    pub module: String,
    pub name: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleExport {
    pub name: String,
    pub signature: String,
}

pub(crate) fn package(
    document: &ComponentDocument,
    core_module: &[u8],
) -> Result<ComponentArtifacts, String> {
    validate_core_module(core_module)?;
    let lifecycle_surface = inspect_lifecycle_surface(core_module)?;
    let wit = crate::wit::render_component(document)?;
    let import_aliases = crate::wit::component_import_aliases(document)?;
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("calcit-component.wit", &wit)
        .map_err(|error| format!("generated Component WIT is invalid: {error:#}"))?;
    let world = resolve
        .select_world(&[package], None)
        .map_err(|error| format!("failed to select generated Component world: {error:#}"))?;

    let rewritten_modules = import_aliases
        .iter()
        .filter_map(|(original, alias)| {
            let alias = alias.trim_start_matches('%');
            (original != alias).then(|| (original.clone(), alias.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut module = rewrite_import_modules(core_module, &rewritten_modules)?;
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8)
        .map_err(|error| format!("failed to embed Component type metadata: {error:#}"))?;
    let mut encoder = ComponentEncoder::default();
    encoder.module(&module).map_err(|error| {
        format!("core module does not implement the Component contract's Canonical ABI: {error:#}")
    })?;
    encoder.validate(true).import_name_map(
        rewritten_modules
            .iter()
            .map(|(original, alias)| (alias.clone(), original.clone()))
            .collect::<HashMap<_, _>>(),
    );
    let component = encoder.encode().map_err(|error| {
        format!("core module does not implement the Component contract's Canonical ABI: {error:#}")
    })?;
    Ok(ComponentArtifacts {
        wit,
        component,
        lifecycle_surface,
    })
}

/// Package a core module against the WASI v0.3.0 `wasi:cli/command` world.
pub fn package_wasi_command(core_module: &[u8]) -> Result<Vec<u8>, String> {
    validate_core_module(core_module)?;
    for payload in Parser::new(0).parse_all(core_module) {
        if let Payload::ImportSection(reader) =
            payload.map_err(|error| format!("invalid core WebAssembly module: {error}"))?
        {
            for import in reader.into_imports() {
                let import =
                    import.map_err(|error| format!("invalid core WebAssembly import: {error}"))?;
                if import.module == "wasi_snapshot_preview1" {
                    return Err(format!(
                        "E_WASI_COMMAND_PREVIEW1_IMPORT: `{}` still imports WASI Preview 1; emit WASI 0.3 Canonical ABI imports instead",
                        import.name
                    ));
                }
            }
        }
    }
    let mut resolve = Resolve::default();
    let mut cli_package = None;
    for (name, wit) in [
        ("clocks.wit", include_str!("wasi_wit/clocks.wit")),
        ("filesystem.wit", include_str!("wasi_wit/filesystem.wit")),
        ("sockets.wit", include_str!("wasi_wit/sockets.wit")),
        ("random.wit", include_str!("wasi_wit/random.wit")),
        ("cli.wit", include_str!("wasi_wit/cli.wit")),
    ] {
        let package = resolve
            .push_str(name, wit)
            .map_err(|error| format!("invalid pinned WASI 0.3 WIT package {name}: {error:#}"))?;
        if name == "cli.wit" {
            cli_package = Some(package);
        }
    }
    let world = resolve
        .select_world(
            &[cli_package.expect("pinned CLI WIT package is included")],
            Some("command"),
        )
        .map_err(|error| format!("failed to select WASI 0.3 command world: {error:#}"))?;
    let mut module = core_module.to_vec();
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8)
        .map_err(|error| format!("failed to embed WASI command WIT metadata: {error:#}"))?;
    let mut encoder = ComponentEncoder::default();
    encoder.module(&module).map_err(|error| {
        format!("core module does not implement the WASI 0.3 command Canonical ABI: {error:#}")
    })?;
    encoder.validate(true);
    encoder.encode().map_err(|error| {
        format!("core module does not implement the WASI 0.3 command Canonical ABI: {error:#}")
    })
}

fn inspect_lifecycle_surface(bytes: &[u8]) -> Result<LifecycleSurface, String> {
    let mut types = Vec::<FuncType>::new();
    let mut function_types = Vec::<u32>::new();
    let mut imports = Vec::<LifecycleImport>::new();
    let mut pending_exports = Vec::<(String, u32)>::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|error| format!("invalid core WebAssembly module: {error}"))? {
            Payload::TypeSection(reader) => {
                types = reader
                    .into_iter_err_on_gc_types()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("invalid core WebAssembly function type: {error}"))?;
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import
                        .map_err(|error| format!("invalid core WebAssembly import: {error}"))?;
                    let type_index = match import.ty {
                        TypeRef::Func(index) | TypeRef::FuncExact(index) => index,
                        _ => continue,
                    };
                    function_types.push(type_index);
                    if is_lifecycle_import(import.module, import.name) {
                        imports.push(LifecycleImport {
                            module: import.module.to_owned(),
                            name: import.name.to_owned(),
                            signature: function_signature(&types, type_index)?,
                        });
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for type_index in reader {
                    function_types.push(type_index.map_err(|error| {
                        format!("invalid core WebAssembly function declaration: {error}")
                    })?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export
                        .map_err(|error| format!("invalid core WebAssembly export: {error}"))?;
                    if export.kind == ExternalKind::Func && is_lifecycle_export(export.name) {
                        pending_exports.push((export.name.to_owned(), export.index));
                    }
                }
            }
            _ => {}
        }
    }

    let mut exports = pending_exports
        .into_iter()
        .map(|(name, function_index)| {
            let type_index = *function_types
                .get(function_index as usize)
                .ok_or_else(|| format!("lifecycle export {name:?} has no function type"))?;
            Ok(LifecycleExport {
                name,
                signature: function_signature(&types, type_index)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    imports.sort();
    exports.sort();
    Ok(LifecycleSurface { imports, exports })
}

fn function_signature(types: &[FuncType], type_index: u32) -> Result<String, String> {
    types
        .get(type_index as usize)
        .map(ToString::to_string)
        .ok_or_else(|| format!("function references missing type index {type_index}"))
}

fn is_lifecycle_import(module: &str, name: &str) -> bool {
    module == "$root"
        || module == "[export]$root"
        || name.contains("[async-lower]")
        || name.contains("[task-return]")
        || name.contains("[task-cancel]")
        || name.contains("[stream-")
}

fn is_lifecycle_export(name: &str) -> bool {
    name.starts_with("[async-lift")
        || name.starts_with("[callback][async-lift")
        || name.starts_with("cabi_post_")
}

struct ImportModuleReencoder<'a> {
    aliases: &'a BTreeMap<String, String>,
}

impl ImportModuleReencoder<'_> {
    fn module<'a>(&'a self, module: &'a str) -> &'a str {
        self.aliases.get(module).map_or(module, String::as_str)
    }
}

impl Reencode for ImportModuleReencoder<'_> {
    type Error = Infallible;

    fn parse_imports(
        &mut self,
        section: &mut ImportSection,
        imports: wasmparser::Imports<'_>,
    ) -> Result<(), ReencodeError<Self::Error>> {
        match imports {
            wasmparser::Imports::Single(_, import) => {
                let ty = self.entity_type(import.ty)?;
                section.import(self.module(import.module), import.name, ty);
            }
            wasmparser::Imports::Compact1 { module, items } => {
                for item in items {
                    let item = item?;
                    let ty = self.entity_type(item.ty)?;
                    section.import(self.module(module), item.name, ty);
                }
            }
            wasmparser::Imports::Compact2 { module, ty, names } => {
                let ty = self.entity_type(ty)?;
                for name in names {
                    section.import(self.module(module), name?, ty);
                }
            }
        }
        Ok(())
    }
}

fn rewrite_import_modules(
    bytes: &[u8],
    aliases: &BTreeMap<String, String>,
) -> Result<Vec<u8>, String> {
    if aliases.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut module = Module::new();
    ImportModuleReencoder { aliases }
        .parse_core_module(&mut module, Parser::new(0), bytes)
        .map_err(|error| format!("failed to preserve the core module while mapping qualified Component imports: {error}"))?;
    Ok(module.finish())
}

fn validate_core_module(bytes: &[u8]) -> Result<(), String> {
    let first = Parser::new(0)
        .parse_all(bytes)
        .next()
        .ok_or_else(|| "core module is empty".to_owned())?
        .map_err(|error| format!("invalid core WebAssembly module: {error}"))?;
    match first {
        Payload::Version {
            encoding: Encoding::Module,
            ..
        } => {}
        Payload::Version {
            encoding: Encoding::Component,
            ..
        } => {
            return Err("expected a core WebAssembly module, not a Component".to_owned());
        }
        _ => return Err("core WebAssembly input has no module header".to_owned()),
    }
    Validator::new()
        .validate_all(bytes)
        .map_err(|error| format!("invalid core WebAssembly module: {error}"))?;
    Ok(())
}
