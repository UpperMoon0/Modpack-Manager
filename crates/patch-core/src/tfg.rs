use crate::schema::{validate_manifest, Operation, PatchManifest, Target};
use crate::source::load_text;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const TFG_FORK_RELEASE: &str = "https://raw.githubusercontent.com/UpperMoon0/Modpack-Modern/refs/heads/nstut/stable/nstut/release.json";
const TFG_FORK_RAW_ROOT: &str = "https://raw.githubusercontent.com/UpperMoon0/Modpack-Modern";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TfgResolvedMod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub file_name: String,
    pub source: String,
    pub targets: Vec<Target>,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct TfgResolvedPatch {
    pub manifest: PatchManifest,
    pub mods: Vec<TfgResolvedMod>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkRelease {
    schema_version: u32,
    overlay_version: String,
    source_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedModsFile {
    schema_version: u32,
    mods: Vec<ManagedMod>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedMod {
    id: String,
    name: String,
    version: String,
    repository: String,
    file_name: String,
    url: String,
    sha256: String,
    install_targets: Vec<Target>,
}

pub async fn resolve_tfg_patch() -> Result<TfgResolvedPatch> {
    let release_source = std::env::var("MODPACK_MANAGER_TFG_RELEASE")
        .unwrap_or_else(|_| TFG_FORK_RELEASE.to_owned());
    let release_body = load_text(&release_source)
        .await
        .context("failed to load the NsTut TFG stable release pointer")?;
    let release: ForkRelease = serde_json::from_str(&release_body)
        .context("NsTut TFG stable release pointer is invalid JSON")?;
    if release.schema_version != 1 {
        bail!(
            "unsupported NsTut TFG release-pointer schema {}; expected 1",
            release.schema_version
        );
    }
    let (default_manifest_source, default_managed_mods_source) =
        fork_tag_sources(&release.source_ref)?;
    let manifest_source = std::env::var("MODPACK_MANAGER_TFG_MANIFEST")
        .unwrap_or(default_manifest_source);
    let managed_mods_source = std::env::var("MODPACK_MANAGER_TFG_MANAGED_MODS")
        .unwrap_or(default_managed_mods_source);

    let manifest_body = load_text(&manifest_source)
        .await
        .context("failed to load the NsTut TFG fork manifest")?;
    let manifest: PatchManifest =
        serde_json::from_str(&manifest_body).context("NsTut TFG fork manifest is invalid JSON")?;
    validate_manifest(&manifest).context("NsTut TFG fork manifest failed validation")?;
    if manifest.version != release.overlay_version {
        bail!(
            "NsTut TFG stable pointer expects overlay {}, but manifest declares {}",
            release.overlay_version,
            manifest.version
        );
    }

    let managed_body = load_text(&managed_mods_source)
        .await
        .context("failed to load the NsTut TFG managed-mod index")?;
    let managed: ManagedModsFile = serde_json::from_str(&managed_body)
        .context("NsTut TFG managed-mod index is invalid JSON")?;
    if managed.schema_version != 1 {
        bail!(
            "unsupported NsTut managed-mod schema {}; expected 1",
            managed.schema_version
        );
    }

    let mut ids = HashSet::new();
    let mut mods = Vec::with_capacity(managed.mods.len());
    for managed_mod in managed.mods {
        if !ids.insert(managed_mod.id.clone()) {
            bail!("duplicate managed TFG mod id {:?}", managed_mod.id);
        }
        mods.push(resolve_managed_mod(&manifest, managed_mod)?);
    }

    Ok(TfgResolvedPatch { manifest, mods })
}

fn fork_tag_sources(source_ref: &str) -> Result<(String, String)> {
    if source_ref.is_empty()
        || !source_ref.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        bail!("NsTut TFG sourceRef contains unsafe characters");
    }

    let base = format!("{TFG_FORK_RAW_ROOT}/{source_ref}/nstut");
    Ok((
        format!("{base}/modpack-manager.patch.json"),
        format!("{base}/managed-mods.json"),
    ))
}

fn resolve_managed_mod(manifest: &PatchManifest, managed: ManagedMod) -> Result<TfgResolvedMod> {
    if managed.sha256.len() != 64
        || !managed
            .sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        bail!("managed release {} has an invalid sha256", managed.id);
    }
    if !managed.url.starts_with("https://") {
        bail!("managed release {} has a non-HTTPS URL", managed.id);
    }

    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.id == managed.id)
        .with_context(|| format!("fork manifest is missing artifact {}", managed.id))?;
    if artifact.url != managed.url
        || !artifact.sha256.eq_ignore_ascii_case(&managed.sha256)
        || artifact.file_name.as_deref() != Some(managed.file_name.as_str())
    {
        bail!(
            "fork manifest artifact {} does not match managed-mod metadata",
            managed.id
        );
    }

    let expected_destination = format!("mods/{}", managed.file_name);
    let installs = manifest
        .operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::InstallFile {
                artifact,
                destination,
                targets,
            } if artifact == &managed.id => Some((destination, targets)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if installs.len() != 1
        || installs[0].0 != &expected_destination
        || installs[0].1 != &managed.install_targets
    {
        bail!(
            "fork manifest install operation for {} does not match managed-mod metadata",
            managed.id
        );
    }

    let source = if let Some(project) = managed.repository.strip_prefix("modrinth:") {
        format!("Modrinth - {project}")
    } else {
        format!("GitHub - {}", managed.repository)
    };

    Ok(TfgResolvedMod {
        id: managed.id,
        name: managed.name,
        version: managed.version,
        file_name: managed.file_name,
        source,
        targets: managed.install_targets,
        url: managed.url,
        sha256: managed.sha256.to_ascii_lowercase(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Artifact;

    fn fixture_manifest() -> PatchManifest {
        PatchManifest {
            schema_version: 1,
            id: "tfg-nstut-managed".into(),
            name: "TFG NsTut".into(),
            version: "1".into(),
            description: String::new(),
            required_paths: vec![],
            artifacts: vec![Artifact {
                id: "economy".into(),
                url: "https://example.invalid/economy.jar".into(),
                sha256: "a".repeat(64),
                file_name: Some("economy-forge.jar".into()),
            }],
            operations: vec![Operation::InstallFile {
                artifact: "economy".into(),
                destination: "mods/economy-forge.jar".into(),
                targets: vec![Target::Client, Target::Server],
            }],
        }
    }

    fn fixture_mod() -> ManagedMod {
        ManagedMod {
            id: "economy".into(),
            name: "Economy".into(),
            version: "1.0.0".into(),
            repository: "UpperMoon0/Economy".into(),
            file_name: "economy-forge.jar".into(),
            url: "https://example.invalid/economy.jar".into(),
            sha256: "a".repeat(64),
            install_targets: vec![Target::Client, Target::Server],
        }
    }

    #[test]
    fn fork_tag_sources_reject_unsafe_refs_and_build_immutable_urls() {
        let (manifest, managed) = fork_tag_sources("nstut-0.13.10.2").unwrap();
        assert_eq!(
            manifest,
            "https://raw.githubusercontent.com/UpperMoon0/Modpack-Modern/nstut-0.13.10.2/nstut/modpack-manager.patch.json"
        );
        assert_eq!(
            managed,
            "https://raw.githubusercontent.com/UpperMoon0/Modpack-Modern/nstut-0.13.10.2/nstut/managed-mods.json"
        );
        assert!(fork_tag_sources("../main").is_err());
        assert!(fork_tag_sources("refs/heads/main").is_err());
    }

    #[test]
    fn managed_mod_must_match_fork_manifest_artifact_and_install() {
        let resolved = resolve_managed_mod(&fixture_manifest(), fixture_mod()).unwrap();
        assert_eq!(resolved.id, "economy");
        assert_eq!(resolved.source, "GitHub - UpperMoon0/Economy");
        assert_eq!(resolved.targets, vec![Target::Client, Target::Server]);
    }

    #[test]
    fn managed_mod_rejects_manifest_drift() {
        let mut managed = fixture_mod();
        managed.file_name = "economy-neoforge.jar".into();
        let error = resolve_managed_mod(&fixture_manifest(), managed).unwrap_err();
        assert!(error.to_string().contains("does not match"));
    }
}
