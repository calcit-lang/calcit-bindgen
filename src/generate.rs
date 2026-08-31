use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tempfile::Builder;

use crate::{Document, validate_document};

pub const INTERFACE_FILE: &str = "interface.json";
pub const RUST_BINDINGS_FILE: &str = "rust/bindings.rs";
pub const CALCIT_BINDINGS_FILE: &str = "calcit/bindings.cirru";
pub const TYPESCRIPT_BINDINGS_FILE: &str = "typescript/bindings.d.ts";
pub const WIT_BINDINGS_FILE: &str = "wit/interface.wit";
pub const MANIFEST_FILE: &str = "calcit-bindgen.manifest.json";
const MANIFEST_SCHEMA_VERSION: u32 = 2;
const GENERATOR_NAME: &str = "calcit-bindgen";
const DIGEST_ALGORITHM: &str = "fnv1a-128";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDigest {
    pub path: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub generator: String,
    pub generator_version: String,
    pub interface_version: u32,
    pub package: String,
    pub package_version: String,
    pub digest_algorithm: String,
    pub contract_digest: String,
    #[serde(default = "legacy_manifest_backends")]
    pub backends: Vec<GenerationBackend>,
    pub files: Vec<ArtifactDigest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationBackend {
    Rust,
    Calcit,
    #[serde(rename = "typescript")]
    TypeScript,
    Wit,
}

const ALL_BACKENDS: [GenerationBackend; 4] = [
    GenerationBackend::Rust,
    GenerationBackend::Calcit,
    GenerationBackend::TypeScript,
    GenerationBackend::Wit,
];

fn legacy_manifest_backends() -> Vec<GenerationBackend> {
    vec![GenerationBackend::Rust]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckIssueKind {
    Missing,
    Modified,
    StaleManifest,
    Unexpected,
}

impl fmt::Display for CheckIssueKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Missing => "missing",
            Self::Modified => "modified",
            Self::StaleManifest => "stale-manifest",
            Self::Unexpected => "unexpected",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckIssue {
    pub kind: CheckIssueKind,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub current: bool,
    pub issues: Vec<CheckIssue>,
}

struct RenderedOutput {
    manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
}

pub fn generate_directory(
    document: &Document,
    output: impl AsRef<Path>,
) -> Result<Manifest, String> {
    generate_directory_with_backends(document, output, &ALL_BACKENDS)
}

pub fn generate_directory_with_backends(
    document: &Document,
    output: impl AsRef<Path>,
    backends: &[GenerationBackend],
) -> Result<Manifest, String> {
    let output = output.as_ref();
    let rendered = render(document, backends)?;
    let parent = output_parent(output)?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create output parent {}: {error}",
            parent.display()
        )
    })?;

    let replacing = if output.exists() {
        ensure_owned_directory(output)?;
        true
    } else {
        false
    };

    let stage = Builder::new()
        .prefix(".calcit-bindgen-stage-")
        .tempdir_in(parent)
        .map_err(|error| {
            format!(
                "failed to create generation stage in {}: {error}",
                parent.display()
            )
        })?;
    write_rendered(stage.path(), &rendered)?;

    if replacing {
        let backup_holder = Builder::new()
            .prefix(".calcit-bindgen-backup-")
            .tempdir_in(parent)
            .map_err(|error| {
                format!(
                    "failed to reserve generation backup in {}: {error}",
                    parent.display()
                )
            })?;
        let backup_root = backup_holder.path().to_path_buf();
        let backup = backup_root.join("previous");
        fs::rename(output, &backup).map_err(|error| {
            format!(
                "failed to move existing generated directory {} to {}: {error}",
                output.display(),
                backup.display()
            )
        })?;
        if let Err(error) = fs::rename(stage.path(), output) {
            let restore = fs::rename(&backup, output);
            return match restore {
                Ok(()) => Err(format!(
                    "failed to install generated directory {}: {error}; previous output restored",
                    output.display()
                )),
                Err(restore_error) => {
                    let preserved = backup_holder.keep().join("previous");
                    Err(format!(
                        "failed to install generated directory {}: {error}; failed to restore {}; previous output remains at {}: {restore_error}",
                        output.display(),
                        output.display(),
                        preserved.display(),
                    ))
                }
            };
        }
        backup_holder.close().map_err(|error| {
            format!(
                "installed generated output but failed to remove backup {}: {error}",
                backup_root.display()
            )
        })?;
    } else {
        fs::rename(stage.path(), output).map_err(|error| {
            format!(
                "failed to install generated directory {}: {error}",
                output.display()
            )
        })?;
    }

    Ok(rendered.manifest)
}

pub fn check_directory(
    document: &Document,
    output: impl AsRef<Path>,
) -> Result<CheckReport, String> {
    check_directory_with_backends(document, output, &ALL_BACKENDS)
}

pub fn check_directory_with_backends(
    document: &Document,
    output: impl AsRef<Path>,
    backends: &[GenerationBackend],
) -> Result<CheckReport, String> {
    let output = output.as_ref();
    let rendered = render(document, backends)?;
    let mut issues = Vec::new();
    if !output.exists() {
        issues.push(issue(
            CheckIssueKind::Missing,
            display_path(output),
            "generated output directory does not exist",
        ));
        return Ok(CheckReport {
            current: false,
            issues,
        });
    }
    let metadata = fs::symlink_metadata(output)
        .map_err(|error| format!("failed to inspect {}: {error}", output.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        issues.push(issue(
            CheckIssueKind::Unexpected,
            display_path(output),
            "generated output path must be a real directory",
        ));
        return Ok(CheckReport {
            current: false,
            issues,
        });
    }

    let manifest_path = output.join(MANIFEST_FILE);
    match read_manifest(&manifest_path) {
        Ok(actual) if actual != rendered.manifest => issues.push(issue(
            CheckIssueKind::StaleManifest,
            MANIFEST_FILE,
            "manifest does not match the current Interface IR or generator version",
        )),
        Ok(_) => {}
        Err(error) if !manifest_path.exists() => {
            issues.push(issue(CheckIssueKind::Missing, MANIFEST_FILE, error))
        }
        Err(error) => issues.push(issue(CheckIssueKind::StaleManifest, MANIFEST_FILE, error)),
    }

    for artifact in &rendered.manifest.files {
        let path = output.join(&artifact.path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                issues.push(issue(
                    CheckIssueKind::Missing,
                    &artifact.path,
                    "generated artifact does not exist",
                ));
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "failed to inspect generated artifact {}: {error}",
                    path.display()
                ));
            }
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            issues.push(issue(
                CheckIssueKind::Modified,
                &artifact.path,
                "generated artifact must be a regular file",
            ));
            continue;
        }
        match fs::read(&path) {
            Ok(bytes) if digest(&bytes) != artifact.digest => issues.push(issue(
                CheckIssueKind::Modified,
                &artifact.path,
                "content digest differs from generated output",
            )),
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "failed to read generated artifact {}: {error}",
                    path.display()
                ));
            }
        }
    }

    let expected = managed_entries(&rendered.manifest);
    for path in collect_artifacts(output)? {
        if !expected.contains(&path) {
            issues.push(issue(
                CheckIssueKind::Unexpected,
                path,
                "artifact is not managed by the current manifest",
            ));
        }
    }
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.kind.to_string().cmp(&right.kind.to_string()))
    });
    Ok(CheckReport {
        current: issues.is_empty(),
        issues,
    })
}

fn render(document: &Document, backends: &[GenerationBackend]) -> Result<RenderedOutput, String> {
    validate_document(document)?;
    let unsupported = document
        .definitions
        .iter()
        .filter(|definition| definition.status == crate::DefinitionStatus::Unsupported)
        .map(|definition| definition.id.as_str())
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "generation requires supported definitions; unsupported: {}",
            unsupported.join(", ")
        ));
    }
    validate_generation_lowerings(document)?;
    let mut canonical = document.clone();
    canonical
        .declarations
        .sort_by(|left, right| left.id().cmp(right.id()));
    canonical
        .definitions
        .sort_by(|left, right| left.id.cmp(&right.id));
    for definition in &mut canonical.definitions {
        definition.diagnostic_codes.sort();
        definition.diagnostic_codes.dedup();
    }
    let mut interface = serde_json::to_vec_pretty(&canonical)
        .map_err(|error| format!("failed to encode canonical Interface IR: {error}"))?;
    interface.push(b'\n');
    let contract_digest = digest(&interface);
    let backends = backends.iter().copied().collect::<BTreeSet<_>>();
    if backends.is_empty() {
        return Err("generation requires at least one backend".to_owned());
    }
    let mut files = BTreeMap::from([(INTERFACE_FILE.to_owned(), interface)]);
    for backend in &backends {
        let (path, content) = match backend {
            GenerationBackend::Rust => (RUST_BINDINGS_FILE, crate::rust::render(&canonical)?),
            GenerationBackend::Calcit => (CALCIT_BINDINGS_FILE, crate::calcit::render(&canonical)?),
            GenerationBackend::TypeScript => (
                TYPESCRIPT_BINDINGS_FILE,
                crate::typescript::render(&canonical)?,
            ),
            GenerationBackend::Wit => (WIT_BINDINGS_FILE, crate::wit::render(&canonical)?),
        };
        files.insert(path.to_owned(), content.into_bytes());
    }
    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        generator: GENERATOR_NAME.to_owned(),
        generator_version: env!("CARGO_PKG_VERSION").to_owned(),
        interface_version: canonical.version,
        package: canonical.package,
        package_version: canonical.package_version,
        digest_algorithm: DIGEST_ALGORITHM.to_owned(),
        contract_digest,
        backends: backends.into_iter().collect(),
        files: files
            .iter()
            .map(|(path, bytes)| ArtifactDigest {
                path: path.clone(),
                digest: digest(bytes),
            })
            .collect(),
    };
    Ok(RenderedOutput { manifest, files })
}

fn validate_generation_lowerings(document: &Document) -> Result<(), String> {
    let unsupported = document
        .definitions
        .iter()
        .filter(|definition| {
            definition.lowering.backend.as_deref() != Some("native")
                || definition.lowering.invoke.as_deref() != Some("sync")
                || definition.lowering.transport.as_deref() != Some("edn-buffer-v1")
        })
        .map(|definition| {
            format!(
                "{} (backend={}, invoke={}, transport={})",
                definition.id,
                definition.lowering.backend.as_deref().unwrap_or("missing"),
                definition.lowering.invoke.as_deref().unwrap_or("missing"),
                definition
                    .lowering
                    .transport
                    .as_deref()
                    .unwrap_or("missing")
            )
        })
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generation supports only native + sync + edn-buffer-v1 definitions; unsupported: {}",
            unsupported.join(", ")
        ))
    }
}

fn write_rendered(directory: &Path, rendered: &RenderedOutput) -> Result<(), String> {
    for (relative, bytes) in &rendered.files {
        let path = directory.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&path, bytes).map_err(|error| {
            format!(
                "failed to write generated artifact {}: {error}",
                path.display()
            )
        })?;
    }
    let mut manifest = serde_json::to_vec_pretty(&rendered.manifest)
        .map_err(|error| format!("failed to encode generation manifest: {error}"))?;
    manifest.push(b'\n');
    let path = directory.join(MANIFEST_FILE);
    fs::write(&path, manifest).map_err(|error| {
        format!(
            "failed to write generation manifest {}: {error}",
            path.display()
        )
    })
}

fn ensure_owned_directory(output: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(output)
        .map_err(|error| format!("failed to inspect {}: {error}", output.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing to replace {}: output must be a real directory",
            output.display()
        ));
    }
    let manifest_path = output.join(MANIFEST_FILE);
    let manifest = read_manifest(&manifest_path).map_err(|error| {
        format!(
            "refusing to replace unowned output directory {}: {error}",
            output.display()
        )
    })?;
    if !(1..=MANIFEST_SCHEMA_VERSION).contains(&manifest.schema_version)
        || manifest.generator != GENERATOR_NAME
    {
        return Err(format!(
            "refusing to replace {}: unsupported ownership manifest",
            output.display()
        ));
    }
    let managed = managed_entries(&manifest);
    let unexpected = collect_artifacts(output)?
        .into_iter()
        .filter(|path| !managed.contains(path))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        return Err(format!(
            "refusing to replace {}: unexpected artifacts: {}",
            output.display(),
            unexpected.join(", ")
        ));
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn collect_artifacts(root: &Path) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    collect_artifacts_inner(root, root, &mut found)?;
    found.sort();
    Ok(found)
}

fn collect_artifacts_inner(
    root: &Path,
    directory: &Path,
    found: &mut Vec<String>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() && !file_type.is_symlink() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
            found.push(format!("{}/", relative_path(relative)?));
            collect_artifacts_inner(root, &path, found)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
            found.push(relative_path(relative)?);
        }
    }
    Ok(())
}

fn managed_entries(manifest: &Manifest) -> BTreeSet<String> {
    let mut managed = BTreeSet::from([MANIFEST_FILE.to_owned()]);
    for artifact in &manifest.files {
        managed.insert(artifact.path.clone());
        let components = artifact.path.split('/').collect::<Vec<_>>();
        for length in 1..components.len() {
            managed.insert(format!("{}/", components[..length].join("/")));
        }
    }
    managed
}

fn output_parent(output: &Path) -> Result<&Path, String> {
    if output.file_name().is_none() {
        return Err(format!(
            "generated output must name a dedicated directory, received {}",
            output.display()
        ));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    if parent.as_os_str().is_empty() {
        Ok(Path::new("."))
    } else {
        Ok(parent)
    }
}

fn relative_path(path: &Path) -> Result<String, String> {
    let components =
        path.components()
            .map(|component| {
                component.as_os_str().to_str().ok_or_else(|| {
                    format!("generated artifact path is not UTF-8: {}", path.display())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
    Ok(components.join("/"))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn issue(kind: CheckIssueKind, path: impl Into<String>, message: impl Into<String>) -> CheckIssue {
    CheckIssue {
        kind,
        path: path.into(),
        message: message.into(),
    }
}

fn digest(bytes: &[u8]) -> String {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:032x}")
}
