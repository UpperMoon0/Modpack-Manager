use crate::schema::{validate_manifest, Artifact, Operation, PatchManifest, Target};
use crate::source::{load_bytes, load_text, resolve_artifact_source};
use anyhow::{bail, Context, Result};
use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use globset::Glob;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use toml_edit::{value as toml_value, DocumentMut, Item, Table};
use walkdir::WalkDir;
use zip::ZipArchive;

pub type ProgressCallback = Arc<dyn Fn(PatchProgress) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub kind: String,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPlan {
    pub manifest_id: String,
    pub manifest_name: String,
    pub manifest_version: String,
    pub target: Target,
    pub root: String,
    pub already_applied: bool,
    pub items: Vec<PlanItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchState {
    pub manifest_id: String,
    pub manifest_version: String,
    pub target: Target,
    pub manifest_source: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub state: PatchState,
    pub backup_path: Option<String>,
    pub changed_paths: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProgress {
    pub phase: PatchProgressPhase,
    pub message: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchProgressPhase {
    Loading,
    Downloading,
    BackingUp,
    Applying,
    Rollback,
    Done,
}

pub async fn load_manifest(source: &str) -> Result<PatchManifest> {
    let text = load_text(source).await?;
    let manifest: PatchManifest =
        serde_json::from_str(&text).context("patch manifest is not valid JSON")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn read_state(root: &Path, target: Target) -> Result<Option<PatchState>> {
    let path = state_path(root, target);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let state = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid patch state {}", path.display()))?;
    Ok(Some(state))
}

pub fn plan_manifest(manifest: &PatchManifest, root: &Path, target: Target) -> Result<PatchPlan> {
    validate_manifest(manifest)?;
    validate_root(root, manifest)?;

    let state = read_state(root, target)?;
    let already_applied = state.as_ref().is_some_and(|state| {
        state.manifest_id == manifest.id && state.manifest_version == manifest.version
    });

    let selected: Vec<&Operation> = manifest
        .operations
        .iter()
        .filter(|operation| operation.applies_to(target))
        .collect();
    let desired_files = desired_install_files(manifest, target)?;

    let mut items = Vec::new();
    let mut warnings = Vec::new();

    for operation in &selected {
        match operation {
            Operation::RemoveMatching { pattern, .. } => {
                for path in resolve_matches(root, pattern)? {
                    // A managed destination is represented by its install/replace diff below.
                    // Do not report the cleanup glob as a second change for the same file.
                    if desired_files.contains_key(&path) {
                        continue;
                    }
                    items.push(PlanItem {
                        kind: "remove".into(),
                        path: slash_path(&path),
                        detail: format!("Remove stale path matched by {pattern}"),
                    });
                }
            }
            Operation::InstallFile {
                artifact,
                destination,
                ..
            } => {
                let expected = desired_files
                    .get(Path::new(destination))
                    .with_context(|| format!("missing desired checksum for {destination:?}"))?;
                let path = root.join(destination);
                ensure_no_symlink_components(root, Path::new(destination))?;

                if path.is_file() && verify_sha256(&path, expected)? {
                    continue;
                }

                let exists = path.exists();
                items.push(PlanItem {
                    kind: if exists { "replace" } else { "install" }.into(),
                    path: destination.clone(),
                    detail: if exists {
                        format!("Installed file differs from verified artifact {artifact}")
                    } else {
                        format!("Install verified artifact {artifact}")
                    },
                });
            }
            Operation::ExtractZip {
                artifact,
                destination,
                clean_destination,
                ..
            } => items.push(PlanItem {
                kind: "extractZip".into(),
                path: destination.clone(),
                detail: if *clean_destination {
                    format!("Replace directory from verified archive {artifact}")
                } else {
                    format!("Merge verified archive {artifact} into directory")
                },
            }),
            Operation::WriteText {
                destination,
                content,
                ..
            } => {
                let path = root.join(destination);
                ensure_no_symlink_components(root, Path::new(destination))?;
                if path.is_file() && fs::read(&path)? == content.as_bytes() {
                    continue;
                }

                items.push(PlanItem {
                    kind: if path.exists() { "replaceText" } else { "writeText" }.into(),
                    path: destination.clone(),
                    detail: if path.exists() {
                        "Managed text differs from desired content".into()
                    } else {
                        "Create managed text file".into()
                    },
                });
            }
            Operation::PatchToml {
                destination,
                values,
                skip_if_missing,
                ..
            } => {
                let path = root.join(destination);
                ensure_no_symlink_components(root, Path::new(destination))?;
                if *skip_if_missing && !path.exists() {
                    continue;
                }
                if !toml_patch_needed(&path, values)? {
                    continue;
                }

                items.push(PlanItem {
                    kind: "patchToml".into(),
                    path: destination.clone(),
                    detail: format!("Update {} managed TOML values", values.len()),
                });
            }
        }
    }

    if already_applied && !items.is_empty() {
        warnings.push(format!(
            "Patch {} is recorded as applied, but the live installation has {} managed change(s).",
            manifest.version,
            items.len()
        ));
    }
    if selected.is_empty() {
        warnings.push(format!(
            "Manifest has no operations for the {} target.",
            target.as_str()
        ));
    }

    Ok(PatchPlan {
        manifest_id: manifest.id.clone(),
        manifest_name: manifest.name.clone(),
        manifest_version: manifest.version.clone(),
        target,
        root: root.display().to_string(),
        already_applied,
        items,
        warnings,
    })
}

pub async fn apply_manifest(
    manifest: &PatchManifest,
    manifest_source: &str,
    root: &Path,
    target: Target,
    progress: Option<ProgressCallback>,
) -> Result<ApplyResult> {
    validate_manifest(manifest)?;
    validate_root(root, manifest)?;

    let manager_dir = root.join(".modpack-manager");
    fs::create_dir_all(&manager_dir)
        .with_context(|| format!("failed to create {}", manager_dir.display()))?;
    ensure_no_symlink_components(root, Path::new(".modpack-manager"))?;

    let lock_path = manager_dir.join("patch.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    lock.try_lock_exclusive()
        .context("another patch operation is already running for this installation")?;

    let selected: Vec<&Operation> = manifest
        .operations
        .iter()
        .filter(|operation| operation.applies_to(target))
        .collect();
    let desired_files = desired_install_files(manifest, target)?;

    let artifact_ids: HashSet<&str> = selected
        .iter()
        .filter_map(|operation| operation.artifact_id())
        .collect();

    let cache_dir = manager_dir
        .join("cache")
        .join(safe_segment(&manifest.id))
        .join(safe_segment(&manifest.version));
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;

    let mut cached_artifacts = HashMap::new();
    let artifact_total = artifact_ids.len();

    for (index, artifact_id) in artifact_ids.iter().enumerate() {
        let artifact = manifest
            .artifacts
            .iter()
            .find(|candidate| candidate.id == *artifact_id)
            .with_context(|| format!("missing artifact {artifact_id:?}"))?;
        emit(
            &progress,
            PatchProgressPhase::Downloading,
            format!("Preparing {}", artifact.id),
            index,
            artifact_total,
        );
        let path = prepare_artifact(artifact, manifest_source, &cache_dir).await?;
        cached_artifacts.insert(artifact.id.clone(), path);
        emit(
            &progress,
            PatchProgressPhase::Downloading,
            format!("Verified {}", artifact.id),
            index + 1,
            artifact_total,
        );
    }

    let backup_dir = manager_dir.join("backups").join(format!(
        "{}-{}",
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
        safe_segment(&manifest.version)
    ));
    let mut transaction = Transaction::new(root.to_path_buf(), backup_dir.clone());

    let apply_result: Result<()> = (|| {
        let total = selected.len();

        for (index, operation) in selected.iter().enumerate() {
            emit(
                &progress,
                PatchProgressPhase::Applying,
                operation_message(operation),
                index,
                total,
            );

            match operation {
                Operation::RemoveMatching { pattern, .. } => {
                    for relative in resolve_matches(root, pattern)? {
                        if desired_files.contains_key(&relative) {
                            continue;
                        }
                        transaction.backup_once(&relative, &progress)?;
                        remove_path(&root.join(&relative))?;
                    }
                }
                Operation::InstallFile {
                    artifact,
                    destination,
                    ..
                } => {
                    let relative = PathBuf::from(destination);
                    ensure_no_symlink_components(root, &relative)?;
                    let destination_path = root.join(&relative);
                    let expected = desired_files
                        .get(&relative)
                        .with_context(|| format!("missing desired checksum for {destination:?}"))?;

                    if destination_path.is_file() && verify_sha256(&destination_path, expected)? {
                        continue;
                    }

                    transaction.backup_once(&relative, &progress)?;
                    let source = cached_artifacts
                        .get(artifact)
                        .with_context(|| format!("artifact {artifact:?} was not prepared"))?;
                    if let Some(parent) = destination_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(source, &destination_path).with_context(|| {
                        format!(
                            "failed to install {} to {}",
                            source.display(),
                            destination_path.display()
                        )
                    })?;
                }
                Operation::ExtractZip {
                    artifact,
                    destination,
                    clean_destination,
                    ..
                } => {
                    let relative = PathBuf::from(destination);
                    ensure_no_symlink_components(root, &relative)?;
                    transaction.backup_once(&relative, &progress)?;
                    let source = cached_artifacts
                        .get(artifact)
                        .with_context(|| format!("artifact {artifact:?} was not prepared"))?;
                    let destination = root.join(&relative);
                    extract_zip(source, root, &relative, &destination, *clean_destination)?;
                }
                Operation::WriteText {
                    destination,
                    content,
                    ..
                } => {
                    let relative = PathBuf::from(destination);
                    ensure_no_symlink_components(root, &relative)?;
                    let destination = root.join(&relative);
                    if destination.is_file() && fs::read(&destination)? == content.as_bytes() {
                        continue;
                    }

                    transaction.backup_once(&relative, &progress)?;
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&destination, content.as_bytes())
                        .with_context(|| format!("failed to write {}", destination.display()))?;
                }
                Operation::PatchToml {
                    destination,
                    values,
                    skip_if_missing,
                    ..
                } => {
                    let relative = PathBuf::from(destination);
                    ensure_no_symlink_components(root, &relative)?;
                    let destination = root.join(&relative);
                    if *skip_if_missing && !destination.exists() {
                        continue;
                    }
                    if !toml_patch_needed(&destination, values)? {
                        continue;
                    }

                    transaction.backup_once(&relative, &progress)?;
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    patch_toml_file(&destination, values)?;
                }
            }

            emit(
                &progress,
                PatchProgressPhase::Applying,
                operation_message(operation),
                index + 1,
                total,
            );
        }

        let state = PatchState {
            manifest_id: manifest.id.clone(),
            manifest_version: manifest.version.clone(),
            target,
            manifest_source: manifest_source.to_owned(),
            applied_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        };
        write_state_atomic(root, &state)?;
        Ok(())
    })();

    if let Err(error) = apply_result {
        emit(
            &progress,
            PatchProgressPhase::Rollback,
            "Patch failed; restoring pre-patch files".into(),
            0,
            transaction.touched.len(),
        );
        if let Err(rollback_error) = transaction.rollback(&progress) {
            return Err(error.context(format!(
                "rollback also failed: {rollback_error:#}. Backup remains at {}",
                backup_dir.display()
            )));
        }
        return Err(error.context("patch failed; original files were restored"));
    }

    let state =
        read_state(root, target)?.context("patch state disappeared after a successful apply")?;

    emit(
        &progress,
        PatchProgressPhase::Done,
        "Patch applied successfully".into(),
        1,
        1,
    );

    Ok(ApplyResult {
        state,
        backup_path: if transaction.touched.is_empty() {
            None
        } else {
            Some(backup_dir.display().to_string())
        },
        changed_paths: transaction.touched.len(),
    })
}

async fn prepare_artifact(
    artifact: &Artifact,
    manifest_source: &str,
    cache_dir: &Path,
) -> Result<PathBuf> {
    let file_name = artifact
        .file_name
        .clone()
        .unwrap_or_else(|| safe_segment(&artifact.id));
    let destination = cache_dir.join(file_name);

    if destination.exists() && verify_sha256(&destination, &artifact.sha256)? {
        return Ok(destination);
    }

    let source = resolve_artifact_source(manifest_source, &artifact.url)?;
    let bytes = load_bytes(&source).await?;
    verify_bytes_sha256(&bytes, &artifact.sha256)
        .with_context(|| format!("checksum verification failed for artifact {:?}", artifact.id))?;

    let temporary = destination.with_extension("part");
    fs::write(&temporary, bytes).with_context(|| {
        format!("failed to write temporary artifact {}", temporary.display())
    })?;
    if destination.exists() {
        remove_path(&destination)?;
    }
    fs::rename(&temporary, &destination).with_context(|| {
        format!(
            "failed to move verified artifact into {}",
            destination.display()
        )
    })?;
    Ok(destination)
}

fn verify_sha256(path: &Path, expected: &str) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected))
}

fn verify_bytes_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = hex::encode(Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("expected SHA-256 {expected}, got {actual}");
    }
    Ok(())
}

fn validate_root(root: &Path, manifest: &PatchManifest) -> Result<()> {
    if !root.exists() {
        bail!("modpack root {} does not exist", root.display());
    }

    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() {
        bail!("modpack root {} cannot be a symlink", root.display());
    }
    if !metadata.is_dir() {
        bail!("modpack root {} is not a directory", root.display());
    }

    for required in &manifest.required_paths {
        let relative = Path::new(required);
        ensure_no_symlink_components(root, relative)?;
        if !root.join(relative).exists() {
            bail!(
                "selected directory is not compatible with this patch: required path {:?} is missing",
                required
            );
        }
    }
    Ok(())
}

fn ensure_no_symlink_components(root: &Path, relative: &Path) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if current.exists() {
            let metadata = fs::symlink_metadata(&current)?;
            if metadata.file_type().is_symlink() {
                bail!("refusing to patch through symlink {}", current.display());
            }
        }
    }
    Ok(())
}

fn resolve_matches(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let matcher = Glob::new(pattern)
        .with_context(|| format!("invalid glob {pattern:?}"))?
        .compile_matcher();

    let mut matches = Vec::new();

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if path == root {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .context("walked path escaped modpack root")?;

        if relative.starts_with(".modpack-manager") {
            continue;
        }

        if entry.file_type().is_symlink() {
            if matcher.is_match(slash_path(relative)) {
                bail!("refusing to patch symlink {}", path.display());
            }
            continue;
        }

        if matcher.is_match(slash_path(relative)) {
            matches.push(relative.to_path_buf());
        }
    }

    matches.sort_by_key(|path| path.components().count());
    let mut minimal = Vec::<PathBuf>::new();

    for path in matches {
        if !minimal.iter().any(|parent| path.starts_with(parent)) {
            minimal.push(path);
        }
    }

    Ok(minimal)
}

fn desired_install_files(manifest: &PatchManifest, target: Target) -> Result<HashMap<PathBuf, String>> {
    let mut desired = HashMap::new();

    for operation in manifest
        .operations
        .iter()
        .filter(|operation| operation.applies_to(target))
    {
        let Operation::InstallFile {
            artifact,
            destination,
            ..
        } = operation
        else {
            continue;
        };

        let artifact = manifest
            .artifacts
            .iter()
            .find(|candidate| candidate.id == *artifact)
            .with_context(|| format!("missing artifact {artifact:?}"))?;
        let relative = PathBuf::from(destination);

        if let Some(existing) = desired.insert(relative.clone(), artifact.sha256.clone()) {
            if !existing.eq_ignore_ascii_case(&artifact.sha256) {
                bail!(
                    "conflicting managed artifacts target the same destination {}",
                    slash_path(&relative)
                );
            }
        }
    }

    Ok(desired)
}

fn toml_patch_needed(path: &Path, values: &BTreeMap<String, serde_json::Value>) -> Result<bool> {
    let current = if path.exists() {
        fs::read(path).with_context(|| format!("failed to read TOML {}", path.display()))?
    } else {
        Vec::new()
    };
    Ok(current != render_patched_toml(path, values)?)
}

fn render_patched_toml(
    path: &Path,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<Vec<u8>> {
    let source = if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read TOML {}", path.display()))?
    } else {
        String::new()
    };

    let mut document = if source.trim().is_empty() {
        DocumentMut::new()
    } else {
        source
            .parse::<DocumentMut>()
            .with_context(|| format!("invalid TOML {}", path.display()))?
    };

    for (key, value) in values {
        set_toml_value(&mut document, key, value)
            .with_context(|| format!("failed to patch TOML key {key:?}"))?;
    }

    Ok(document.to_string().into_bytes())
}

fn extract_zip(
    archive_path: &Path,
    root: &Path,
    destination_relative: &Path,
    destination: &Path,
    clean_destination: bool,
) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open archive {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("invalid zip archive {}", archive_path.display()))?;

    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if is_zip_symlink(entry.unix_mode()) {
            bail!("archive contains forbidden symlink entry {:?}", entry.name());
        }
        let enclosed = entry
            .enclosed_name()
            .with_context(|| format!("archive contains unsafe path {:?}", entry.name()))?
            .to_path_buf();
        entries.push(enclosed);
    }

    if clean_destination && destination.exists() {
        remove_path(destination)?;
    }
    fs::create_dir_all(destination)?;

    for (index, enclosed) in entries.iter().enumerate() {
        let output_relative = destination_relative.join(enclosed);
        ensure_no_symlink_components(root, &output_relative)?;
        let output = root.join(&output_relative);

        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut target =
            File::create(&output).with_context(|| format!("failed to create {}", output.display()))?;
        std::io::copy(&mut entry, &mut target)
            .with_context(|| format!("failed to extract {}", output.display()))?;
        target.flush()?;
    }

    Ok(())
}

fn is_zip_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn write_state_atomic(root: &Path, state: &PatchState) -> Result<()> {
    let path = state_path(root, state.target);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(state)?)?;

    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(&temporary, &path)?;
    Ok(())
}

fn state_path(root: &Path, target: Target) -> PathBuf {
    root.join(".modpack-manager")
        .join(format!("state-{}.json", target.as_str()))
}

fn operation_message(operation: &Operation) -> String {
    match operation {
        Operation::RemoveMatching { pattern, .. } => format!("Removing paths matching {pattern}"),
        Operation::InstallFile { destination, .. } => format!("Installing {destination}"),
        Operation::ExtractZip { destination, .. } => format!("Updating {destination}"),
        Operation::WriteText { destination, .. } => format!("Writing {destination}"),
        Operation::PatchToml { destination, .. } => format!("Patching TOML {destination}"),
    }
}

fn patch_toml_file(path: &Path, values: &BTreeMap<String, serde_json::Value>) -> Result<()> {
    fs::write(path, render_patched_toml(path, values)?)
        .with_context(|| format!("failed to write TOML {}", path.display()))
}

fn set_toml_value(
    document: &mut DocumentMut,
    dotted_key: &str,
    value: &serde_json::Value,
) -> Result<()> {
    let parts: Vec<&str> = dotted_key.split('.').collect();
    let (leaf, parents) = parts
        .split_last()
        .context("TOML key cannot be empty")?;

    let mut table = document.as_table_mut();
    for part in parents {
        if !table.contains_key(part) {
            table.insert(part, Item::Table(Table::new()));
        }
        table = table
            .get_mut(part)
            .and_then(Item::as_table_mut)
            .with_context(|| format!("TOML path component {part:?} is not a table"))?;
    }

    let item = match value {
        serde_json::Value::Bool(value) => toml_value(*value),
        serde_json::Value::String(value) => toml_value(value.clone()),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                toml_value(value)
            } else if let Some(value) = value.as_u64() {
                let value = i64::try_from(value).context("TOML integer exceeds i64 range")?;
                toml_value(value)
            } else if let Some(value) = value.as_f64() {
                toml_value(value)
            } else {
                bail!("unsupported numeric TOML value");
            }
        }
        _ => bail!("patchToml supports only scalar boolean, numeric, and string values"),
    };

    table.insert(leaf, item);
    Ok(())
}

fn emit(
    callback: &Option<ProgressCallback>,
    phase: PatchProgressPhase,
    message: String,
    current: usize,
    total: usize,
) {
    if let Some(callback) = callback {
        callback(PatchProgress {
            phase,
            message,
            current,
            total,
        });
    }
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn safe_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn remove_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to remove symlink {}", path.display());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove file {}", path.display()))?;
    }

    Ok(())
}

fn copy_path(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to back up symlink {}", source.display());
    }

    if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in WalkDir::new(source).follow_links(false) {
            let entry = entry?;
            if entry.file_type().is_symlink() {
                bail!("refusing to back up symlink {}", entry.path().display());
            }

            let relative = entry.path().strip_prefix(source)?;
            if relative.as_os_str().is_empty() {
                continue;
            }

            let target = destination.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry.path(), &target)?;
            }
        }
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
    }

    Ok(())
}

#[derive(Debug)]
struct TouchedPath {
    relative: PathBuf,
    existed: bool,
}

struct Transaction {
    root: PathBuf,
    backup_root: PathBuf,
    touched: Vec<TouchedPath>,
}

impl Transaction {
    fn new(root: PathBuf, backup_root: PathBuf) -> Self {
        Self {
            root,
            backup_root,
            touched: Vec::new(),
        }
    }

    fn backup_once(
        &mut self,
        relative: &Path,
        progress: &Option<ProgressCallback>,
    ) -> Result<()> {
        if self
            .touched
            .iter()
            .any(|existing| relative.starts_with(&existing.relative))
        {
            return Ok(());
        }

        ensure_no_symlink_components(&self.root, relative)?;
        let source = self.root.join(relative);
        let existed = source.exists();

        emit(
            progress,
            PatchProgressPhase::BackingUp,
            format!("Backing up {}", slash_path(relative)),
            self.touched.len(),
            self.touched.len() + 1,
        );

        if existed {
            let destination = self.backup_root.join("files").join(relative);
            copy_path(&source, &destination)?;
        }

        self.touched.push(TouchedPath {
            relative: relative.to_path_buf(),
            existed,
        });

        Ok(())
    }

    fn rollback(&self, progress: &Option<ProgressCallback>) -> Result<()> {
        for (index, touched) in self.touched.iter().rev().enumerate() {
            emit(
                progress,
                PatchProgressPhase::Rollback,
                format!("Restoring {}", slash_path(&touched.relative)),
                index,
                self.touched.len(),
            );

            let destination = self.root.join(&touched.relative);
            remove_path(&destination)?;

            if touched.existed {
                let source = self.backup_root.join("files").join(&touched.relative);
                copy_path(&source, &destination)?;
            }
        }

        Ok(())
    }
}
