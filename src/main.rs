use std::path::PathBuf;

use calcit_bindgen::{check_directory, compare, generate_directory, load_document};
use clap::{Parser, Subcommand};

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
    },
    /// Check generated artifacts without modifying the output directory.
    Check {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
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
        Command::Generate { input, out } => {
            let manifest = generate_directory(&load_document(input)?, &out)?;
            println!(
                "generated {} artifact(s) for {} {} in {}",
                manifest.files.len(),
                manifest.package,
                manifest.package_version,
                out.display()
            );
        }
        Command::Check { input, out } => {
            let report = check_directory(&load_document(input)?, &out)?;
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
