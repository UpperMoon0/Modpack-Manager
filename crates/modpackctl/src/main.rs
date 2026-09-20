use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use patch_core::{
    apply_manifest, load_manifest, plan_manifest, read_state, PatchProgress, ProgressCallback, Target,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "modpackctl", version, about = "Manifest-driven modpack patch CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Plan {
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value = "server")]
        target: CliTarget,
        #[arg(long)]
        json: bool,
    },
    Apply {
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value = "server")]
        target: CliTarget,
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value = "server")]
        target: CliTarget,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliTarget {
    Client,
    Server,
}

impl From<CliTarget> for Target {
    fn from(value: CliTarget) -> Self {
        match value {
            CliTarget::Client => Target::Client,
            CliTarget::Server => Target::Server,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Plan {
            manifest,
            root,
            target,
            json,
        } => {
            let patch = load_manifest(&manifest).await?;
            let plan = plan_manifest(&patch, &root, target.into())?;

            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!(
                    "{} {} -> {} ({})",
                    plan.manifest_name,
                    plan.manifest_version,
                    plan.root,
                    plan.target.as_str()
                );
                for item in &plan.items {
                    println!("  {:14} {:40} {}", item.kind, item.path, item.detail);
                }
                for warning in &plan.warnings {
                    eprintln!("warning: {warning}");
                }
            }
        }
        Command::Apply {
            manifest,
            root,
            target,
            json,
        } => {
            let patch = load_manifest(&manifest).await?;
            let progress: ProgressCallback = Arc::new(|event: PatchProgress| {
                eprintln!(
                    "[{:?}] {}/{} {}",
                    event.phase, event.current, event.total, event.message
                );
            });

            let result =
                apply_manifest(&patch, &manifest, &root, target.into(), Some(progress)).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "Applied {} {}. Changed {} paths.",
                    result.state.manifest_id, result.state.manifest_version, result.changed_paths
                );
                if let Some(path) = result.backup_path {
                    println!("Backup: {path}");
                }
            }
        }
        Command::Status { root, target, json } => {
            let state = read_state(&root, target.into())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else if let Some(state) = state {
                println!(
                    "{} {} applied at {}",
                    state.manifest_id, state.manifest_version, state.applied_at
                );
            } else {
                println!("No patch state recorded.");
            }
        }
    }

    Ok(())
}
