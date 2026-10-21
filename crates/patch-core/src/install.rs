//! Build a complete client in isolated staging, then publish a Prism instance.
use crate::engine::{apply_fresh_manifest, ensure_no_symlink_components};
use crate::schema::validate_manifest;
use crate::{plan_manifest, Operation, PatchManifest, ProgressCallback, Target};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshInstallPlan {
    pub version: String,
    pub destination: String,
    pub artifact_count: usize,
    pub minecraft_version: String,
    pub forge_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshInstallResult {
    pub version: String,
    pub instance_path: String,
    pub minecraft_root: String,
    pub archive_path: String,
}

fn destination_path(destination: &Path) -> Result<PathBuf> {
    let absolute = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()?.join(destination)
    };
    // Plans and result paths cross a UTF-8 API boundary. Reject unrepresentable
    // paths rather than silently publishing into a replacement-character path.
    if absolute.to_str().is_none() {
        bail!("install destination must be valid UTF-8; choose a different folder");
    }
    let parent = absolute.parent().context("choose a new instance folder")?;
    ensure_no_symlink_components(parent, Path::new(""))?;
    // Check ancestors as well as the final destination, including broken links.
    for path in absolute.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                bail!("install destination cannot contain symlinks");
            }
        }
    }
    if !parent.is_dir() || absolute.file_name().is_none() {
        bail!("install parent must be an existing directory");
    }
    if absolute.exists() && (!absolute.is_dir() || fs::read_dir(&absolute)?.next().is_some()) {
        bail!("choose an empty or new folder; existing files will not be overwritten");
    }
    Ok(absolute)
}

pub fn plan_client_install(
    manifest: &PatchManifest,
    destination: &Path,
) -> Result<FreshInstallPlan> {
    validate_manifest(manifest)?;
    // Loader metadata is pinned to this supported upstream base. Reject future
    // bases until their launcher requirements have been reviewed explicitly.
    if !["0.13.12-nstut.", "0.13.15-nstut."]
        .iter()
        .any(|base| manifest.version.starts_with(base))
        || !manifest
            .version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        bail!("fresh install does not support this pack base yet");
    }
    let bases = manifest
        .operations
        .iter()
        .filter(|op| op.applies_to(Target::Client) && matches!(op, Operation::MigratePack { .. }))
        .count();
    if bases != 1 {
        bail!("fresh install requires exactly one verified complete client base");
    }
    let destination = destination_path(destination)?;
    let artifacts: std::collections::HashSet<_> = manifest
        .operations
        .iter()
        .filter(|op| op.applies_to(Target::Client))
        .filter_map(|op| op.artifact_id())
        .collect();
    Ok(FreshInstallPlan {
        version: manifest.version.clone(),
        destination: destination.to_string_lossy().into_owned(),
        artifact_count: artifacts.len(),
        minecraft_version: "1.20.1".into(),
        forge_version: "47.4.13".into(),
    })
}

pub async fn install_client(
    manifest: &PatchManifest,
    source: &str,
    destination: &Path,
    progress: Option<ProgressCallback>,
) -> Result<FreshInstallResult> {
    let plan = plan_client_install(manifest, destination)?;
    let destination = PathBuf::from(&plan.destination);
    let parent = destination.parent().context("missing install parent")?;
    let staging = tempfile::Builder::new()
        .prefix(".tfg-install-")
        .tempdir_in(parent)?;
    let instance = staging.path().join("instance");
    let root = instance.join(".minecraft");
    fs::create_dir_all(&root)?;
    apply_fresh_manifest(manifest, source, &root, progress.clone()).await?;
    if !plan_manifest(manifest, &root, Target::Client)?
        .items
        .is_empty()
    {
        bail!("fresh installation did not converge; destination left unchanged");
    }
    fs::write(instance.join("instance.cfg"), format!(
        "[General]\nInstanceType=OneSix\nname=TFG NsTut {}\nOverrideMemory=true\nMinMemAlloc=1024\nMaxMemAlloc=6144\n", plan.version
    ))?;
    let metadata = json!({"formatVersion":1,"components":[
        {"uid":"net.minecraft","version":plan.minecraft_version,"important":true},
        {"uid":"net.minecraftforge","version":plan.forge_version}
    ]});
    fs::write(
        instance.join("mmc-pack.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    let archive_name = "TFG-NsTut-Prism.zip";
    if let Some(callback) = &progress {
        callback(crate::PatchProgress {
            phase: crate::PatchProgressPhase::Applying,
            message: "Creating Prism Launcher import…".into(),
            current: 0,
            total: 1,
        });
    }
    write_prism_archive(&instance, &instance.join(archive_name))?;
    // Fresh installs have no pre-existing user state to retain in a backup.
    // Discard staging-only download copies after the import archive is complete.
    for directory in ["cache", "backups"] {
        let path = root.join(".modpack-manager").join(directory);
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        }
    }
    // Recheck after the long download to protect files added in the meantime.
    destination_path(&destination)?;
    if destination.exists() {
        fs::remove_dir(&destination).context("install destination is no longer empty")?;
    }
    fs::rename(&instance, &destination).context("failed to publish completed installation")?;
    if let Some(callback) = &progress {
        callback(crate::PatchProgress {
            phase: crate::PatchProgressPhase::Done,
            message: "TFG installed. Import the ZIP into Prism Launcher to play.".into(),
            current: 1,
            total: 1,
        });
    }
    Ok(FreshInstallResult {
        version: plan.version,
        instance_path: destination.to_string_lossy().into_owned(),
        minecraft_root: destination
            .join(".minecraft")
            .to_string_lossy()
            .into_owned(),
        archive_path: destination
            .join(archive_name)
            .to_string_lossy()
            .into_owned(),
    })
}

fn write_prism_archive(instance: &Path, archive: &Path) -> Result<()> {
    let mut zip = ZipWriter::new(File::create(archive)?);
    // Preserve the small update state but exclude download cache, locks/backups
    // and the ZIP itself. Prism's MultiMC importer reads instance.cfg at root.
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for entry in WalkDir::new(instance)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".modpack-manager")
    {
        let entry = entry?;
        if entry.path() == instance || entry.path() == archive {
            continue;
        }
        let relative = entry.path().strip_prefix(instance)?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if entry.file_type().is_dir() {
            zip.add_directory(name, options)?;
        } else if entry.file_type().is_file() {
            zip.start_file(name, options)?;
            std::io::copy(&mut File::open(entry.path())?, &mut zip)?;
        } else {
            bail!("unexpected non-file in fresh instance");
        }
    }
    let state = instance.join(".minecraft/.modpack-manager/state-client.json");
    zip.start_file(".minecraft/.modpack-manager/state-client.json", options)?;
    zip.write_all(&fs::read(state)?)?;
    zip.finish()?;
    Ok(())
}
