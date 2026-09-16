use std::path::PathBuf;

use calcit_bindgen::{
    GenerationBackend, InterfaceContract, check_contract_directory, compare, compare_component,
    generate_contract_directory, load_contract,
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
    /// Validate a supported native or Component Interface IR contract.
    Validate { input: PathBuf },
    /// Compare two validated Interface IR v2/v3 contracts.
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
        /// Core WebAssembly module implementing a Component Interface IR contract.
        #[arg(long)]
        core_module: Option<PathBuf>,
        /// Select backends; native defaults to all and Component defaults to WIT.
        #[arg(long = "backend", value_enum)]
        backends: Vec<BackendArg>,
    },
    /// Check generated artifacts without modifying the output directory.
    Check {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Core WebAssembly module used when the Component artifacts were generated.
        #[arg(long)]
        core_module: Option<PathBuf>,
        /// Select the same backend set recorded by the generated manifest.
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
            let contract = load_contract(input)?;
            println!(
                "valid {} Interface IR v{}: {} {} ({} declarations, {} definitions)",
                contract.kind_name(),
                contract.version(),
                contract.package(),
                contract.package_version(),
                contract.declarations_len(),
                contract.definitions_len()
            );
        }
        Command::Diff { old, new, json } => {
            let old = load_contract(old)?;
            let new = load_contract(new)?;
            let report = match (old, new) {
                (InterfaceContract::Native(old), InterfaceContract::Native(new)) => {
                    compare(&old, &new)
                }
                (InterfaceContract::Component(old), InterfaceContract::Component(new)) => {
                    compare_component(&old, &new)
                }
                _ => {
                    return Err(
                        "diff requires both inputs to use the same Interface IR boundary"
                            .to_owned(),
                    );
                }
            };
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
            core_module,
            backends,
        } => {
            let contract = load_contract(input)?;
            let backends = backends
                .into_iter()
                .map(GenerationBackend::from)
                .collect::<Vec<_>>();
            let manifest =
                generate_contract_directory(&contract, core_module.as_deref(), &out, &backends)?;
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
            core_module,
            backends,
        } => {
            let contract = load_contract(input)?;
            let backends = backends
                .into_iter()
                .map(GenerationBackend::from)
                .collect::<Vec<_>>();
            let report =
                check_contract_directory(&contract, core_module.as_deref(), &out, &backends)?;
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
