use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Client,
    Server,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required_paths: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Operation {
    RemoveMatching {
        pattern: String,
        #[serde(default)]
        targets: Vec<Target>,
    },
    InstallFile {
        artifact: String,
        destination: String,
        #[serde(default)]
        targets: Vec<Target>,
    },
    ExtractZip {
        artifact: String,
        destination: String,
        #[serde(default = "default_true")]
        clean_destination: bool,
        #[serde(default)]
        targets: Vec<Target>,
    },
    WriteText {
        destination: String,
        content: String,
        #[serde(default)]
        targets: Vec<Target>,
    },
    PatchToml {
        destination: String,
        values: BTreeMap<String, JsonValue>,
        #[serde(default)]
        skip_if_missing: bool,
        #[serde(default)]
        targets: Vec<Target>,
    },
}

fn default_true() -> bool {
    true
}

impl Operation {
    pub fn targets(&self) -> &[Target] {
        match self {
            Self::RemoveMatching { targets, .. }
            | Self::InstallFile { targets, .. }
            | Self::ExtractZip { targets, .. }
            | Self::WriteText { targets, .. }
            | Self::PatchToml { targets, .. } => targets,
        }
    }

    pub fn applies_to(&self, target: Target) -> bool {
        self.targets().is_empty() || self.targets().contains(&target)
    }

    pub fn artifact_id(&self) -> Option<&str> {
        match self {
            Self::InstallFile { artifact, .. } | Self::ExtractZip { artifact, .. } => {
                Some(artifact)
            }
            _ => None,
        }
    }
}

pub fn validate_manifest(manifest: &PatchManifest) -> Result<()> {
    if manifest.schema_version != 1 {
        bail!(
            "unsupported manifest schema version {}; expected 1",
            manifest.schema_version
        );
    }
    validate_identifier(&manifest.id).context("invalid manifest id")?;
    if manifest.version.trim().is_empty() {
        bail!("manifest version cannot be empty");
    }

    for path in &manifest.required_paths {
        validate_relative_path(path).with_context(|| format!("invalid required path {path:?}"))?;
    }

    let mut ids = HashSet::new();
    for artifact in &manifest.artifacts {
        validate_identifier(&artifact.id)
            .with_context(|| format!("invalid artifact id {:?}", artifact.id))?;
        if !ids.insert(artifact.id.as_str()) {
            bail!("duplicate artifact id {:?}", artifact.id);
        }
        if artifact.url.trim().is_empty() {
            bail!("artifact {:?} has an empty url", artifact.id);
        }
        if artifact.sha256.len() != 64
            || !artifact.sha256.chars().all(|character| character.is_ascii_hexdigit())
        {
            bail!(
                "artifact {:?} must declare a 64-character SHA-256 checksum",
                artifact.id
            );
        }
        if let Some(file_name) = &artifact.file_name {
            validate_file_name(file_name)
                .with_context(|| format!("invalid fileName for artifact {:?}", artifact.id))?;
        }
    }

    for operation in &manifest.operations {
        match operation {
            Operation::RemoveMatching { pattern, .. } => validate_pattern(pattern)?,
            Operation::InstallFile {
                artifact,
                destination,
                ..
            } => {
                ensure_artifact_exists(artifact, &ids)?;
                validate_file_destination(destination, "installFile")?;
            }
            Operation::ExtractZip {
                artifact,
                destination,
                ..
            } => {
                ensure_artifact_exists(artifact, &ids)?;
                validate_destination(destination)?;
                if destination.trim().is_empty() || destination == "." {
                    bail!("extractZip destination cannot be the modpack root");
                }
            }
            Operation::WriteText { destination, .. } => {
                validate_file_destination(destination, "writeText")?;
            }
            Operation::PatchToml {
                destination,
                values,
                ..
            } => {
                validate_file_destination(destination, "patchToml")?;
                if values.is_empty() {
                    bail!("patchToml values cannot be empty");
                }
                for (key, value) in values {
                    validate_toml_key(key)?;
                    validate_toml_value(value)
                        .with_context(|| format!("invalid patchToml value for {key:?}"))?;
                }
            }
        }
    }

    Ok(())
}

fn validate_identifier(value: &str) -> Result<()> {
    if value.trim().is_empty() || matches!(value, "." | "..") {
        bail!("identifier cannot be empty, '.' or '..'");
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    {
        bail!("identifier contains unsafe characters");
    }
    Ok(())
}

fn ensure_artifact_exists(artifact: &str, ids: &HashSet<&str>) -> Result<()> {
    if !ids.contains(artifact) {
        bail!("operation references unknown artifact {artifact:?}");
    }
    Ok(())
}

pub fn validate_relative_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    if path.is_absolute() {
        bail!("absolute paths are forbidden");
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("path traversal is forbidden")
            }
        }
    }
    Ok(())
}

fn validate_destination(value: &str) -> Result<()> {
    validate_relative_path(value)?;
    if manager_path(value) {
        bail!("patch operations cannot target manager metadata");
    }
    Ok(())
}

fn validate_file_destination(value: &str, operation: &str) -> Result<()> {
    validate_destination(value)?;
    if value.trim().is_empty() || value == "." {
        bail!("{operation} destination must name a file");
    }
    Ok(())
}

fn validate_pattern(value: &str) -> Result<()> {
    validate_relative_path(value)?;
    if value.trim().is_empty() || value == "." {
        bail!("removeMatching pattern cannot target the modpack root");
    }
    if manager_path(value) {
        bail!("patch operations cannot target manager metadata");
    }
    globset::Glob::new(value).context("invalid removeMatching glob")?;
    Ok(())
}

fn validate_toml_key(value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("TOML key cannot be empty");
    }
    for segment in value.split('.') {
        if segment.is_empty()
            || !segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        {
            bail!("TOML dotted key contains unsafe characters");
        }
    }
    Ok(())
}

fn validate_toml_value(value: &JsonValue) -> Result<()> {
    match value {
        JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => Ok(()),
        _ => bail!("patchToml supports only boolean, numeric, and string scalar values"),
    }
}

fn manager_path(value: &str) -> bool {
    Path::new(value)
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .is_some_and(|part| part.eq_ignore_ascii_case(".modpack-manager"))
}

fn validate_file_name(value: &str) -> Result<()> {
    if matches!(value, "." | "..") {
        bail!("fileName cannot be '.' or '..'");
    }
    let path = Path::new(value);
    if path.components().count() != 1 {
        bail!("fileName must be a single file name");
    }
    validate_relative_path(value)
}
