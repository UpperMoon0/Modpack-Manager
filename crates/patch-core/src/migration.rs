use crate::engine::{ensure_no_symlink_components, ProgressCallback, Transaction};
use crate::schema::Operation;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    schema_version: u32,
    files: BTreeMap<String, String>,
    previous_files: BTreeMap<String, String>,
}

pub(crate) fn validate_pack_path(path: &str) -> Result<()> {
    crate::schema::validate_relative_path(path)?;
    if path.contains('\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        bail!("invalid pack migration path {path:?}");
    }
    let roots = [
        "mods/",
        "config/",
        "defaultconfigs/",
        "kubejs/",
        "resourcepacks/",
        "shaderpacks/",
        "tacz/",
    ];
    if !roots.iter().any(|prefix| path.starts_with(prefix)) {
        bail!("migration cannot change server-owned or launcher files: {path}");
    }
    if matches!(path, "config/trueuuid-registry.json") || path.starts_with("kubejs/export/") {
        bail!("migration cannot replace private runtime data: {path}");
    }
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn matches_hash(root: &Path, relative: &str, expected: &str) -> Result<bool> {
    ensure_no_symlink_components(root, Path::new(relative))?;
    let path = root.join(relative);
    Ok(path.is_file() && hash(&fs::read(path)?) == expected.to_ascii_lowercase())
}

fn pending_path(root: &Path, marker: &str) -> std::path::PathBuf {
    root.join(".modpack-manager")
        .join(format!("{marker}.pending"))
}

pub(crate) fn needed(operation: &Operation, root: &Path) -> Result<bool> {
    let Operation::MigratePack {
        source_file,
        source_sha256,
        target_file,
        target_sha256,
        marker,
        marker_content,
        ..
    } = operation
    else {
        bail!("not a migration operation")
    };
    ensure_no_symlink_components(root, Path::new(marker))?;
    if root.join(marker).is_file() && fs::read(root.join(marker))? == marker_content.as_bytes() {
        if !matches_hash(root, target_file, target_sha256)? {
            bail!("base marker exists but target core artifact does not match");
        }
        return Ok(false);
    }
    if matches_hash(root, source_file, source_sha256)? {
        return Ok(true);
    }
    // A process interruption can occur between removing the old core and installing
    // the new core. Resume only the exact operation whose verified migration began.
    let pending = pending_path(root, marker);
    ensure_no_symlink_components(root, pending.strip_prefix(root)?)?;
    if pending.is_file() && fs::read_to_string(pending)? == hash(&serde_json::to_vec(operation)?) {
        return Ok(true);
    }
    bail!("installation does not match the supported source base; no files changed")
}

fn read_entry(zip: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>> {
    let mut file = zip
        .by_name(name)
        .with_context(|| format!("missing migration entry {name}"))?;
    if file.size() > 512 * 1024 * 1024 {
        bail!("migration entry is too large: {name}");
    }
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

// Preserve values changed by the user relative to the old shipped defaults,
// while taking new defaults and deleting obsolete unmodified settings.
fn merge_values(current: &Value, previous: &Value, next: &Value) -> Value {
    if current == previous {
        return next.clone();
    }
    match (current, previous, next) {
        (Value::Object(cur), Value::Object(old), Value::Object(new)) => {
            let mut result = new.clone();
            for (key, value) in cur {
                match old.get(key) {
                    Some(before) if before == value => {}
                    Some(before) => {
                        let after = new.get(key).unwrap_or(value);
                        result.insert(key.clone(), merge_values(value, before, after));
                    }
                    None => {
                        result.insert(key.clone(), value.clone());
                    }
                }
            }
            for key in old.keys() {
                if !cur.contains_key(key) {
                    result.remove(key);
                }
            }
            Value::Object(result)
        }
        _ => current.clone(),
    }
}

fn merge_config(path: &str, current: &[u8], previous: &[u8], next: &[u8]) -> Result<Vec<u8>> {
    if current == previous {
        return Ok(next.to_vec());
    }
    if next == previous {
        return Ok(current.to_vec());
    }
    if path.ends_with(".json") {
        if let (Ok(cur), Ok(old), Ok(new)) = (
            serde_json::from_slice(current),
            serde_json::from_slice(previous),
            serde_json::from_slice(next),
        ) {
            let mut data = serde_json::to_vec_pretty(&merge_values(&cur, &old, &new))?;
            data.push(b'\n');
            return Ok(data);
        }
    }
    if path.ends_with(".toml") {
        if let (Ok(cur), Ok(old), Ok(new)) = (
            std::str::from_utf8(current),
            std::str::from_utf8(previous),
            std::str::from_utf8(next),
        ) {
            if let (Ok(cur), Ok(old), Ok(new)) = (
                toml_edit::de::from_str::<Value>(cur),
                toml_edit::de::from_str::<Value>(old),
                toml_edit::de::from_str::<Value>(new),
            ) {
                return Ok(
                    toml_edit::ser::to_string_pretty(&merge_values(&cur, &old, &new))?.into_bytes(),
                );
            }
        }
    }
    // Unrecognised config formats keep the user's edited file. Mods can generate
    // missing defaults; the original is also retained in the transaction backup.
    Ok(current.to_vec())
}

pub(crate) fn apply(
    archive: &Path,
    root: &Path,
    operation: &Operation,
    transaction: &mut Transaction,
    progress: &Option<ProgressCallback>,
    desired_files: &std::collections::HashMap<std::path::PathBuf, String>,
) -> Result<()> {
    let Operation::MigratePack {
        target_file,
        target_sha256,
        marker,
        ..
    } = operation
    else {
        bail!("not a migration")
    };
    let mut zip = ZipArchive::new(File::open(archive)?)?;
    let index: Index = serde_json::from_slice(&read_entry(&mut zip, "migration-index.json")?)?;
    if index.schema_version != 1 || desired_files.get(Path::new(target_file)) != Some(target_sha256)
    {
        bail!("migration target does not match a verified install operation");
    }
    let mut names = HashSet::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        if !names.insert(file.name().to_owned())
            || file.is_dir()
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            bail!("invalid or duplicate migration archive entry");
        }
        if file.name() != "migration-index.json"
            && !file.name().starts_with("files/")
            && !file.name().starts_with("previous/config/")
        {
            bail!("unexpected migration archive entry {}", file.name());
        }
        if let Some(path) = file
            .name()
            .strip_prefix("files/")
            .or_else(|| file.name().strip_prefix("previous/"))
        {
            validate_pack_path(path)?;
        }
    }
    for (path, checksum) in index.files.iter().chain(index.previous_files.iter()) {
        validate_pack_path(path)?;
        ensure_no_symlink_components(root, Path::new(path))?;
        if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("invalid inventory checksum");
        }
    }
    // Validate every payload and old-default entry before the first mutation.
    for (path, checksum) in &index.files {
        let data = read_entry(&mut zip, &format!("files/{path}"))?;
        if hash(&data) != checksum.to_ascii_lowercase() {
            bail!("migration payload checksum mismatch: {path}");
        }
    }
    for name in &names {
        if let Some(path) = name.strip_prefix("previous/") {
            let expected = index
                .previous_files
                .get(path)
                .context("unowned previous default")?;
            if hash(&read_entry(&mut zip, name)?) != expected.to_ascii_lowercase() {
                bail!("previous config checksum mismatch");
            }
        } else if let Some(path) = name.strip_prefix("files/") {
            if !index.files.contains_key(path) {
                bail!("unindexed migration payload");
            }
        }
    }
    let pending = pending_path(root, marker);
    fs::write(&pending, hash(&serde_json::to_vec(operation)?))?;
    for (path, old_hash) in &index.previous_files {
        if index.files.contains_key(path)
            || desired_files.contains_key(Path::new(path))
            || !root.join(path).exists()
        {
            continue;
        }
        // Keep edited obsolete configs, but remove all known obsolete code/assets
        // and old mod versions. Unknown user files are never part of this inventory.
        if path.starts_with("config/") && !matches_hash(root, path, old_hash)? {
            continue;
        }
        transaction.backup_once(Path::new(path), progress)?;
        fs::remove_file(root.join(path))?;
    }
    for path in index.files.keys() {
        let mut next = read_entry(&mut zip, &format!("files/{path}"))?;
        let destination = root.join(path);
        if path.starts_with("config/")
            && !path.starts_with("config/ftbquests/")
            && !path.starts_with("config/fancymenu/")
            && destination.is_file()
        {
            let current = fs::read(&destination)?;
            let previous_name = format!("previous/{path}");
            if names.contains(&previous_name) {
                next = merge_config(
                    path,
                    &current,
                    &read_entry(&mut zip, &previous_name)?,
                    &next,
                )?;
            } else if !index.previous_files.contains_key(path) {
                // A new upstream config name colliding with an existing custom file.
                next = current;
            }
        }
        if destination.is_file() && fs::read(&destination)? == next {
            continue;
        }
        transaction.backup_once(Path::new(path), progress)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(destination, next)?;
    }
    // The engine commits the base marker after all mod and overlay operations.
    // Keep the resume record until then, including after process interruption.
    Ok(())
}

pub(crate) fn finish(
    root: &Path,
    operation: &Operation,
    transaction: &mut Transaction,
    progress: &Option<ProgressCallback>,
) -> Result<()> {
    let Operation::MigratePack {
        target_file,
        target_sha256,
        marker,
        marker_content,
        ..
    } = operation
    else {
        bail!("not a migration")
    };
    if !matches_hash(root, target_file, target_sha256)? {
        bail!("target base core was not installed");
    }
    if root.join(marker).is_file() && fs::read(root.join(marker))? == marker_content.as_bytes() {
        return Ok(());
    }
    transaction.backup_once(Path::new(marker), progress)?;
    fs::write(root.join(marker), marker_content)?;
    let pending = pending_path(root, marker);
    if pending.exists() {
        fs::remove_file(pending)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_custom_values_with_new_defaults() {
        let old = serde_json::json!({"settings":{"custom":1,"default":2,"removed":3}});
        let cur =
            serde_json::json!({"settings":{"custom":9,"default":2,"removed":3},"private":"keep"});
        let new = serde_json::json!({"settings":{"custom":4,"default":5,"added":6}});
        assert_eq!(
            merge_values(&cur, &old, &new),
            serde_json::json!({"settings":{"custom":9,"default":5,"added":6},"private":"keep"})
        );
    }
    #[test]
    fn forbids_world_auth_and_launcher_paths() {
        for path in [
            "world/level.dat",
            "config/trueuuid-registry.json",
            "../mods/a.jar",
            "mods/../world/level.dat",
            "mods\\a.jar",
            "kubejs/export/private.json",
            "start_server.sh",
        ] {
            assert!(validate_pack_path(path).is_err(), "{path}");
        }
    }
}
