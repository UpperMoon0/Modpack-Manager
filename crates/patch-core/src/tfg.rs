use crate::schema::{Artifact, Operation, PatchManifest, Target};
use crate::source::{load_bytes, load_text};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const GITHUB_RELEASE_LIMIT: usize = 30;
const EXTRA_GAUGES_PROJECT: &str = "extra-gauges";

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

#[derive(Clone)]
struct GithubModSpec {
    id: &'static str,
    name: &'static str,
    repository: &'static str,
    asset_prefix: &'static str,
    cleanup_patterns: &'static [&'static str],
    install_targets: Vec<Target>,
    cleanup_targets: Vec<Target>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthVersion {
    version_number: String,
    version_type: String,
    date_published: String,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

pub async fn resolve_tfg_patch() -> Result<TfgResolvedPatch> {
    let mut mods = Vec::new();

    for spec in github_specs() {
        mods.push(resolve_github_mod(&spec).await?);
    }
    mods.push(resolve_extra_gauges().await?);

    Ok(build_tfg_patch(mods))
}

fn github_specs() -> Vec<GithubModSpec> {
    let both = vec![Target::Client, Target::Server];

    vec![
        GithubModSpec {
            id: "economy",
            name: "Economy",
            repository: "UpperMoon0/Economy",
            asset_prefix: "economy-forge-1.20.1-",
            cleanup_patterns: &["mods/economy-*.jar"],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "openui",
            name: "OpenUI MC",
            repository: "UpperMoon0/OpenUI-MC",
            asset_prefix: "openui-mc-forge-1.20.1-",
            cleanup_patterns: &["mods/openui-mc-*.jar"],
            install_targets: vec![Target::Client],
            // Client-side only: remove stale copies from servers instead of installing it there.
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "simply-screens",
            name: "Simply Screens",
            repository: "UpperMoon0/Simply-Screens",
            asset_prefix: "simply_screens-forge-1.20.1-",
            cleanup_patterns: &[
                "mods/simply_screens-*.jar",
                "mods/simply-screens-*.jar",
            ],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "simply-speakers",
            name: "Simply Speakers",
            repository: "UpperMoon0/Simply-Speakers",
            asset_prefix: "simplyspeakers-forge-1.20.1-",
            cleanup_patterns: &[
                "mods/simplyspeakers-*.jar",
                "mods/simply_speakers-*.jar",
                "mods/simply-speakers-*.jar",
            ],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "create-horse-power-ce",
            name: "Create Horse Power - CE",
            repository: "UpperMoon0/CreateHorsePower-CE",
            asset_prefix: "createhorsepower-ce-1.20.1-",
            // Deliberately broad: removes the original mod and every previous CE build.
            cleanup_patterns: &["mods/createhorsepower-*.jar"],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "building-gadgets-extra",
            name: "Building Gadgets Extra",
            repository: "UpperMoon0/Building-Gadgets-Extra",
            asset_prefix: "building-gadgets-extra-forge-1.20.1-",
            cleanup_patterns: &[
                "mods/building-gadgets-extra-*.jar",
                "mods/buildinggadgetsextra-*.jar",
            ],
            install_targets: both.clone(),
            cleanup_targets: both,
        },
    ]
}

async fn resolve_github_mod(spec: &GithubModSpec) -> Result<TfgResolvedMod> {
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={}",
        spec.repository, GITHUB_RELEASE_LIMIT
    );
    let body = load_text(&url)
        .await
        .with_context(|| format!("failed to load releases for {}", spec.repository))?;
    let releases: Vec<GithubRelease> =
        serde_json::from_str(&body).context("GitHub releases response is invalid JSON")?;

    let (version, asset) = select_github_asset(&releases, spec.asset_prefix).with_context(|| {
        format!(
            "no stable Forge 1.20.1 release asset matching {:?} was found in {}",
            spec.asset_prefix, spec.repository
        )
    })?;

    let sha256 = match asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        Some(digest) => digest.to_ascii_lowercase(),
        None => sha256_remote(&asset.browser_download_url).await?,
    };

    Ok(TfgResolvedMod {
        id: spec.id.into(),
        name: spec.name.into(),
        version,
        file_name: asset.name,
        source: format!("GitHub · {}", spec.repository),
        targets: spec.install_targets.clone(),
        url: asset.browser_download_url,
        sha256,
    })
}

fn select_github_asset(
    releases: &[GithubRelease],
    asset_prefix: &str,
) -> Option<(String, GithubAsset)> {
    let mut stable: Vec<&GithubRelease> = releases
        .iter()
        .filter(|release| !release.draft && !release.prerelease)
        .collect();
    stable.sort_by(|left, right| right.published_at.cmp(&left.published_at));

    for release in stable {
        if let Some(asset) = release.assets.iter().find(|asset| {
            asset.name.starts_with(asset_prefix)
                && asset.name.ends_with(".jar")
                && !asset.name.ends_with("-sources.jar")
                && !asset.name.ends_with("-dev-shadow.jar")
                && !asset.name.ends_with("-javadoc.jar")
        }) {
            return Some((
                release.tag_name.trim_start_matches('v').to_owned(),
                asset.clone(),
            ));
        }
    }

    None
}

async fn resolve_extra_gauges() -> Result<TfgResolvedMod> {
    let query = "https://api.modrinth.com/v2/project/extra-gauges/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22forge%22%5D";
    let body = load_text(query)
        .await
        .context("failed to load Create: Extra Gauges versions from Modrinth")?;
    let versions: Vec<ModrinthVersion> =
        serde_json::from_str(&body).context("Modrinth versions response is invalid JSON")?;
    let (version, file) = select_modrinth_release(&versions)
        .context("Modrinth has no stable Forge 1.20.1 release for Create: Extra Gauges")?;

    let sha256 = match file.hashes.get("sha256") {
        Some(hash) => hash.to_ascii_lowercase(),
        None => sha256_remote(&file.url).await?,
    };

    Ok(TfgResolvedMod {
        id: "extra-gauges".into(),
        name: "Create: Extra Gauges".into(),
        version,
        file_name: file.filename,
        source: format!("Modrinth · {EXTRA_GAUGES_PROJECT}"),
        targets: vec![Target::Client, Target::Server],
        url: file.url,
        sha256,
    })
}

fn select_modrinth_release(versions: &[ModrinthVersion]) -> Option<(String, ModrinthFile)> {
    let mut stable: Vec<&ModrinthVersion> = versions
        .iter()
        .filter(|version| version.version_type == "release")
        .collect();
    stable.sort_by(|left, right| right.date_published.cmp(&left.date_published));

    for version in stable {
        let Some(file) = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
        else {
            continue;
        };

        if file.filename.ends_with(".jar") {
            return Some((version.version_number.clone(), file.clone()));
        }
    }

    None
}

async fn sha256_remote(url: &str) -> Result<String> {
    let bytes = load_bytes(url)
        .await
        .with_context(|| format!("failed to download {url} for checksum verification"))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn build_tfg_patch(mods: Vec<TfgResolvedMod>) -> TfgResolvedPatch {
    let both = vec![Target::Client, Target::Server];
    let mut artifacts = Vec::new();
    let mut operations = Vec::new();

    for managed in &mods {
        artifacts.push(Artifact {
            id: managed.id.clone(),
            url: managed.url.clone(),
            sha256: managed.sha256.clone(),
            file_name: Some(managed.file_name.clone()),
        });

        let (cleanup_patterns, cleanup_targets): (&[&str], Vec<Target>) =
            if managed.id == "extra-gauges" {
                (&["mods/extra_gauges-*.jar"], both.clone())
            } else {
                let spec = github_specs()
                    .into_iter()
                    .find(|spec| spec.id == managed.id)
                    .expect("managed GitHub mod must have a spec");
                (spec.cleanup_patterns, spec.cleanup_targets)
            };

        for pattern in cleanup_patterns {
            operations.push(Operation::RemoveMatching {
                pattern: (*pattern).into(),
                targets: cleanup_targets.clone(),
            });
        }

        operations.push(Operation::InstallFile {
            artifact: managed.id.clone(),
            destination: format!("mods/{}", managed.file_name),
            targets: managed.targets.clone(),
        });
    }

    let identity = mods
        .iter()
        .map(|managed| format!("{}={}:{}", managed.id, managed.version, managed.sha256))
        .collect::<Vec<_>>()
        .join("|");
    let fingerprint = hex::encode(Sha256::digest(identity.as_bytes()));

    TfgResolvedPatch {
        manifest: PatchManifest {
            schema_version: 1,
            id: "tfg-nstut-managed".into(),
            name: "TFG NsTut Managed Mods".into(),
            version: format!("1.20.1-{}", &fingerprint[..12]),
            description: "Latest compatible Forge 1.20.1 managed mod set for TFG.".into(),
            required_paths: vec!["mods".into(), "config".into(), "kubejs".into()],
            artifacts,
            operations,
        },
        mods,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GithubAsset {
        GithubAsset {
            name: name.into(),
            browser_download_url: format!("https://example.invalid/{name}"),
            digest: Some(format!("sha256:{}", "a".repeat(64))),
        }
    }

    #[test]
    fn github_selection_skips_newer_incompatible_and_prerelease_releases() {
        let releases = vec![
            GithubRelease {
                tag_name: "v2.0.0".into(),
                draft: false,
                prerelease: false,
                published_at: Some("2026-09-20T00:00:00Z".into()),
                assets: vec![asset("economy-neoforge-1.21.1-2.0.0.jar")],
            },
            GithubRelease {
                tag_name: "v1.5.0".into(),
                draft: false,
                prerelease: true,
                published_at: Some("2026-09-19T00:00:00Z".into()),
                assets: vec![asset("economy-forge-1.20.1-1.5.0.jar")],
            },
            GithubRelease {
                tag_name: "v1.4.0".into(),
                draft: false,
                prerelease: false,
                published_at: Some("2026-09-18T00:00:00Z".into()),
                assets: vec![
                    asset("economy-forge-1.20.1-1.4.0-sources.jar"),
                    asset("economy-forge-1.20.1-1.4.0.jar"),
                ],
            },
        ];

        let (version, selected) =
            select_github_asset(&releases, "economy-forge-1.20.1-").unwrap();
        assert_eq!(version, "1.4.0");
        assert_eq!(selected.name, "economy-forge-1.20.1-1.4.0.jar");
    }

    #[test]
    fn modrinth_selection_picks_latest_stable_primary_jar() {
        let versions = vec![
            ModrinthVersion {
                version_number: "2.0.8-rc1".into(),
                version_type: "beta".into(),
                date_published: "2026-09-20T00:00:00Z".into(),
                files: vec![ModrinthFile {
                    url: "https://example.invalid/rc.jar".into(),
                    filename: "extra_gauges-2.0.8-rc1.jar".into(),
                    primary: true,
                    hashes: HashMap::new(),
                }],
            },
            ModrinthVersion {
                version_number: "2.0.7".into(),
                version_type: "release".into(),
                date_published: "2025-10-02T00:00:00Z".into(),
                files: vec![ModrinthFile {
                    url: "https://example.invalid/release.jar".into(),
                    filename: "extra_gauges-2.0.7.jar".into(),
                    primary: true,
                    hashes: HashMap::new(),
                }],
            },
        ];

        let (version, file) = select_modrinth_release(&versions).unwrap();
        assert_eq!(version, "2.0.7");
        assert_eq!(file.filename, "extra_gauges-2.0.7.jar");
    }

    #[test]
    fn tfg_manifest_replaces_original_horse_power_and_keeps_openui_client_only() {
        let make = |id: &str, name: &str, targets: Vec<Target>| TfgResolvedMod {
            id: id.into(),
            name: name.into(),
            version: "1".into(),
            file_name: format!("{id}-1.jar"),
            source: "test".into(),
            targets,
            url: format!("https://example.invalid/{id}.jar"),
            sha256: "a".repeat(64),
        };

        let patch = build_tfg_patch(vec![
            make("openui", "OpenUI MC", vec![Target::Client]),
            make(
                "create-horse-power-ce",
                "Create Horse Power - CE",
                vec![Target::Client, Target::Server],
            ),
            make(
                "extra-gauges",
                "Create: Extra Gauges",
                vec![Target::Client, Target::Server],
            ),
        ]);

        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::RemoveMatching { pattern, targets }
                if pattern == "mods/createhorsepower-*.jar"
                    && targets.contains(&Target::Server)
        )));
        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::InstallFile { artifact, targets, .. }
                if artifact == "openui" && targets.as_slice() == [Target::Client]
        )));
        assert!(!patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::InstallFile { artifact, targets, .. }
                if artifact == "openui" && targets.contains(&Target::Server)
        )));
        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::RemoveMatching { pattern, targets }
                if pattern == "mods/openui-mc-*.jar"
                    && targets.contains(&Target::Server)
        )));
    }
}
