use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;

use wasm_encoder::reencode::{Error as ReencodeError, Reencode};
use wasm_encoder::{ImportSection, Module};
use wasmparser::{Encoding, Parser, Payload, Validator};
use wit_component::{ComponentEncoder, StringEncoding, embed_component_metadata};
use wit_parser::Resolve;

use crate::ComponentDocument;

pub(crate) struct ComponentArtifacts {
    pub wit: String,
    pub component: Vec<u8>,
}

pub(crate) fn package(
    document: &ComponentDocument,
    core_module: &[u8],
) -> Result<ComponentArtifacts, String> {
    validate_core_module(core_module)?;
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
    Ok(ComponentArtifacts { wit, component })
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
            return Err(
                "--core-module expects a core WebAssembly module, not a Component".to_owned(),
            );
        }
        _ => return Err("core WebAssembly input has no module header".to_owned()),
    }
    Validator::new()
        .validate_all(bytes)
        .map_err(|error| format!("invalid core WebAssembly module: {error}"))?;
    Ok(())
}
