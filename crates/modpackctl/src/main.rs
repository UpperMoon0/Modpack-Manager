use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use patch_core::{
    apply_manifest, load_manifest, load_patch_channel, plan_manifest, read_state, resolve_tfg_patch,
    PatchManifest, PatchProgress, ProgressCallback, Target,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "modpackctl", version, about = "Manifest-driven modpack patch CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct PatchSource {
    #[arg(long, conflicts_with_all = ["channel", "tfg"])]
    manifest: Option<String>,

    #[arg(long, conflicts_with_all = ["manifest", "tfg"])]
    channel: Option<String>,

    /// Resolve the built-in TFG Forge 1.20.1 managed-mod profile.
    #[arg(long, conflicts_with_all = ["manifest", "channel"])]
    tfg: bool,
}

struct ResolvedSource {
    manifest: PatchManifest,
    source: String,
}

impl PatchSource {
    async fn resolve(self) -> Result<ResolvedSource> {
        match (self.manifest, self.channel, self.tfg) {
            (Some(source), None, false) => Ok(ResolvedSource {
                manifest: load_manifest(&source).await?,
                source,
            }),
            (None, Some(channel), false) => {
                let resolved = load_patch_channel(&channel).await?;
                eprintln!(
                    "Resolved channel {} -> patch {}",
                    resolved.name, resolved.version
                );
                Ok(ResolvedSource {
                    manifest: load_manifest(&resolved.manifest_source).await?,
                    source: resolved.manifest_source,
                })
            }
            (None, None, true) => {
                let resolved = resolve_tfg_patch().await?;
                eprintln!(
                    "Resolved TFG Forge 1.20.1 profile -> {} managed mods",
                    resolved.mods.len()
                );
                for managed in &resolved.mods {
                    eprintln!("  {} {} ({})", managed.name, managed.version, managed.source);
                }
                Ok(ResolvedSource {
                    manifest: resolved.manifest,
                    source: "tfg://managed/forge-1.20.1".into(),
                })
            }
            _ => bail!("provide exactly one of --manifest, --channel, or --tfg"),
        }
    }
}

#[derive(Subcommand)]
enum Command {
    Plan {
        #[command(flatten)]
        source: PatchSource,
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value = "server")]
        target: CliTarget,
        #[arg(long)]
        json: bool,
    },
    Apply {
        #[command(flatten)]
        source: PatchSource,
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
            source,
            root,
            target,
            json,
        } => {
            let resolved = source.resolve().await?;
            let plan = plan_manifest(&resolved.manifest, &root, target.into())?;

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
            source,
            root,
            target,
            json,
        } => {
            let resolved = source.resolve().await?;
            let progress: ProgressCallback = Arc::new(|event: PatchProgress| {
                eprintln!(
                    "[{:?}] {}/{} {}",
                    event.phase, event.current, event.total, event.message
                );
            });

            let result = apply_manifest(
                &resolved.manifest,
                &resolved.source,
                &root,
                target.into(),
                Some(progress),
            )
            .await?;

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
