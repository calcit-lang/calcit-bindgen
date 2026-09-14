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
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("calcit-component.wit", &wit)
        .map_err(|error| format!("generated Component WIT is invalid: {error:#}"))?;
    let world = resolve
        .select_world(&[package], None)
        .map_err(|error| format!("failed to select generated Component world: {error:#}"))?;

    let mut module = core_module.to_vec();
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8)
        .map_err(|error| format!("failed to embed Component type metadata: {error:#}"))?;
    let component = ComponentEncoder::default()
        .module(&module)
        .map_err(|error| {
            format!(
                "core module does not implement the Component contract's Canonical ABI: {error:#}"
            )
        })?
        .validate(true)
        .encode()
        .map_err(|error| {
            format!(
                "core module does not implement the Component contract's Canonical ABI: {error:#}"
            )
        })?;
    Ok(ComponentArtifacts { wit, component })
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
