use std::path::PathBuf;

use calcit_bindgen::{
    GenerationBackend, check_directory, check_directory_with_backends, compare, generate_directory,
    generate_directory_with_backends, load_document,
};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "calcit-bindgen", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate an Interface IR v2 envelope or document.
    Validate { input: PathBuf },
    /// Compare two validated Interface IR v2 documents.
    Diff {
        old: PathBuf,
        new: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Generate deterministic managed artifacts and a versioned manifest.
    Generate {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Generate only selected backends; omit to generate every backend.
        #[arg(long = "backend", value_enum)]
        backends: Vec<BackendArg>,
    },
    /// Check generated artifacts without modifying the output directory.
    Check {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Check only selected backends; must match the generated manifest.
        #[arg(long = "backend", value_enum)]
        backends: Vec<BackendArg>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendArg {
    Rust,
    Calcit,
    #[value(name = "typescript")]
    TypeScript,
    Wit,
}

impl From<BackendArg> for GenerationBackend {
    fn from(value: BackendArg) -> Self {
        match value {
            BackendArg::Rust => Self::Rust,
            BackendArg::Calcit => Self::Calcit,
            BackendArg::TypeScript => Self::TypeScript,
            BackendArg::Wit => Self::Wit,
        }
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("calcit-bindgen failed: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Validate { input } => {
            let document = load_document(input)?;
            println!(
                "valid Interface IR v{}: {} {} ({} declarations, {} definitions)",
                document.version,
                document.package,
                document.package_version,
                document.declarations.len(),
                document.definitions.len()
            );
        }
        Command::Diff { old, new, json } => {
            let report = compare(&load_document(old)?, &load_document(new)?);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|error| format!(
                        "failed to encode compatibility report: {error}"
                    ))?
                );
            } else if report.changes.is_empty() {
                println!("compatible: no interface changes");
            } else {
                println!("compatible: {}", report.compatible);
                for change in &report.changes {
                    println!("- {:?} {}: {}", change.kind, change.path, change.message);
                }
            }
            if !report.compatible {
                return Err("breaking Interface IR changes detected".to_owned());
            }
        }
        Command::Generate {
            input,
            out,
            backends,
        } => {
            let document = load_document(input)?;
            let manifest = if backends.is_empty() {
                generate_directory(&document, &out)?
            } else {
                let backends = backends
                    .into_iter()
                    .map(GenerationBackend::from)
                    .collect::<Vec<_>>();
                generate_directory_with_backends(&document, &out, &backends)?
            };
            println!(
                "generated {} artifact(s) for {} {} in {}",
                manifest.files.len(),
                manifest.package,
                manifest.package_version,
                out.display()
            );
        }
        Command::Check {
            input,
            out,
            backends,
        } => {
            let document = load_document(input)?;
            let report = if backends.is_empty() {
                check_directory(&document, &out)?
            } else {
                let backends = backends
                    .into_iter()
                    .map(GenerationBackend::from)
                    .collect::<Vec<_>>();
                check_directory_with_backends(&document, &out, &backends)?
            };
            if report.current {
                println!("generated artifacts are current: {}", out.display());
            } else {
                for issue in report.issues {
                    eprintln!("[{}] {}: {}", issue.kind, issue.path, issue.message);
                }
                return Err("generated artifacts are not current".to_owned());
            }
        }
    }
    Ok(())
}
