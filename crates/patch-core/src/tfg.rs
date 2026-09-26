use crate::schema::{Artifact, Operation, PatchManifest, Target};
use crate::source::load_text;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const TFG_RELEASE_INDEX: &str = "https://raw.githubusercontent.com/UpperMoon0/Modpack-Manager/main/data/tfg-releases.json";

const TFG_PROFILE_POLICY_VERSION: &str = "2026-09-26-gtceu-weather-safety-v5";
const TFG_HORSE_POWER_RECIPE: &str = r#"// priority: 0
"use strict";

function registerCreateHorsePowerBlockRecipes(event) {

	event.remove({id: 'createhorsepower:horse_crank' })

	event.shaped('createhorsepower:horse_crank', [
		' A ',
		'EBD',
		'CCC'
	], {
		A: '#forge:fences/wooden',
		B: '#forge:small_gears/bronze',
		C: '#tfc:rock/raw',
		D: '#forge:tools/hammers',
		E: '#forge:tools/saws'
	}).id('tfg:shaped/horse_crank_bronze')

	event.shaped('createhorsepower:horse_crank', [
		' A ',
		'EBD',
		'CCC'
	], {
		A: '#forge:fences/wooden',
		B: '#forge:small_gears/bismuth_bronze',
		C: '#tfc:rock/raw',
		D: '#forge:tools/hammers',
		E: '#forge:tools/saws'
	}).id('tfg:shaped/horse_crank_bismuth_bronze')

	event.shaped('createhorsepower:horse_crank', [
		' A ',
		'EBD',
		'CCC'
	], {
		A: '#forge:fences/wooden',
		B: '#forge:small_gears/black_bronze',
		C: '#tfc:rock/raw',
		D: '#forge:tools/hammers',
		E: '#forge:tools/saws'
	}).id('tfg:shaped/horse_crank_black_bronze')
}
"#;

fn horse_power_config_values() -> BTreeMap<String, serde_json::Value> {
    let mut values = BTreeMap::new();
    values.insert("creatureRPMRange".into(), 16.into());
    values.insert("smallCreatureStressRange".into(), 16.into());
    values.insert("mediumCreatureStressRange".into(), 24.into());
    values.insert("largeCreatureStressRange".into(), 32.into());
    values.insert("poorMultiplier".into(), serde_json::Value::from(0.5));
    values.insert("normalMultiplier".into(), serde_json::Value::from(1.0));
    values.insert("greatMultiplier".into(), serde_json::Value::from(2.0));
    values.insert(
        "balance.globalRpmMultiplier".into(),
        serde_json::Value::from(1.0),
    );
    values.insert(
        "balance.globalStressMultiplier".into(),
        serde_json::Value::from(1.0),
    );
    values.insert(
        "balance.enableIndividualAnimalStats".into(),
        serde_json::Value::from(false),
    );
    values.insert(
        "path.evaluationMode".into(),
        serde_json::Value::from("LEGACY"),
    );
    values.insert(
        "path.enableStressScaling".into(),
        serde_json::Value::from(false),
    );
    values.insert(
        "path.minimumCoverage".into(),
        serde_json::Value::from(1.0),
    );
    values
}


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
    cleanup_patterns: &'static [&'static str],
    install_targets: Vec<Target>,
    cleanup_targets: Vec<Target>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedReleaseIndex {
    schema_version: u32,
    mods: Vec<PublishedRelease>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedRelease {
    id: String,
    name: String,
    version: String,
    file_name: String,
    source: String,
    url: String,
    sha256: String,
}

pub async fn resolve_tfg_patch() -> Result<TfgResolvedPatch> {
    let release_index_source = std::env::var("MODPACK_MANAGER_TFG_RELEASE_INDEX")
        .unwrap_or_else(|_| TFG_RELEASE_INDEX.to_owned());
    let body = load_text(&release_index_source)
        .await
        .context("failed to load the managed TFG release index")?;
    let index: PublishedReleaseIndex =
        serde_json::from_str(&body).context("managed TFG release index is invalid JSON")?;
    if index.schema_version != 1 {
        anyhow::bail!(
            "unsupported managed TFG release index schema {}",
            index.schema_version
        );
    }

    let mut published = index
        .mods
        .into_iter()
        .map(|release| (release.id.clone(), release))
        .collect::<std::collections::HashMap<_, _>>();
    let mut mods = Vec::new();

    for spec in github_specs() {
        let release = published
            .remove(spec.id)
            .with_context(|| format!("managed TFG release index is missing {}", spec.id))?;
        mods.push(resolve_published_release(release, spec.install_targets)?);
    }

    let extra = published
        .remove("extra-gauges")
        .context("managed TFG release index is missing extra-gauges")?;
    mods.push(resolve_published_release(
        extra,
        vec![Target::Client, Target::Server],
    )?);

    Ok(build_tfg_patch(mods))
}

fn resolve_published_release(
    release: PublishedRelease,
    targets: Vec<Target>,
) -> Result<TfgResolvedMod> {
    if release.sha256.len() != 64 || !release.sha256.chars().all(|ch| ch.is_ascii_hexdigit()) {
        anyhow::bail!("managed release {} has an invalid sha256", release.id);
    }
    if !release.url.starts_with("https://") {
        anyhow::bail!("managed release {} has a non-HTTPS URL", release.id);
    }

    Ok(TfgResolvedMod {
        id: release.id,
        name: release.name,
        version: release.version,
        file_name: release.file_name,
        source: release.source,
        targets,
        url: release.url,
        sha256: release.sha256.to_ascii_lowercase(),
    })
}

fn github_specs() -> Vec<GithubModSpec> {
    let both = vec![Target::Client, Target::Server];

    vec![
        GithubModSpec {
            id: "economy",
            cleanup_patterns: &["mods/economy-*.jar"],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "openui",
            cleanup_patterns: &["mods/openui-mc-*.jar"],
            install_targets: vec![Target::Client],
            // Client-side only: remove stale copies from servers instead of installing it there.
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "create-precise-controls",
            cleanup_patterns: &["mods/create-precise-controls-*.jar"],
            install_targets: vec![Target::Client],
            // Client-side only: keep dedicated servers clean if a client JAR was copied there.
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "simply-screens",
            cleanup_patterns: &[
                "mods/simply_screens-*.jar",
                "mods/simply-screens-*.jar",
            ],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "simply-speakers",
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
            // Deliberately broad: removes the original mod and every previous CE build.
            cleanup_patterns: &["mods/createhorsepower-*.jar"],
            install_targets: both.clone(),
            cleanup_targets: both.clone(),
        },
        GithubModSpec {
            id: "building-gadgets-extra",
            cleanup_patterns: &[
                "mods/building-gadgets-extra-*.jar",
                "mods/buildinggadgetsextra-*.jar",
            ],
            install_targets: both.clone(),
            cleanup_targets: both,
        },
    ]
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

    operations.push(Operation::WriteText {
        destination: "kubejs/server_scripts/create_horse_power/recipes.js".into(),
        content: TFG_HORSE_POWER_RECIPE.into(),
        targets: both.clone(),
    });

    // TFG Core intentionally redirects Forge SERVER configs to the game-level
    // defaultconfigs directory, so this is the live config for dedicated and
    // integrated TFG servers rather than merely a new-world template.
    let horse_power_config = horse_power_config_values();
    operations.push(Operation::PatchToml {
        destination: "defaultconfigs/createhorsepower-server.toml".into(),
        values: horse_power_config,
        skip_if_missing: false,
        targets: both.clone(),
    });

    let mut gtceu_config = BTreeMap::new();
    gtceu_config.insert(
        "machines.shouldWeatherOrTerrainExplosion".into(),
        serde_json::Value::from(false),
    );
    operations.push(Operation::PatchYaml {
        destination: "config/gtceu.yaml".into(),
        values: gtceu_config,
        skip_if_missing: false,
        targets: vec![Target::Server],
    });

    let identity = format!(
        "{}|{}",
        TFG_PROFILE_POLICY_VERSION,
        mods.iter()
            .map(|managed| format!("{}={}:{}", managed.id, managed.version, managed.sha256))
            .collect::<Vec<_>>()
            .join("|")
    );
    let fingerprint = hex::encode(Sha256::digest(identity.as_bytes()));

    TfgResolvedPatch {
        manifest: PatchManifest {
            schema_version: 1,
            id: "tfg-nstut-managed".into(),
            name: "TFG NsTut Managed Mods".into(),
            version: format!("1.20.1-{}", &fingerprint[..12]),
            description: "Latest compatible Forge 1.20.1 managed mod set for TFG.".into(),
            required_paths: vec![
                "mods".into(),
                "config".into(),
                "kubejs".into(),
                "defaultconfigs/createhorsepower-server.toml".into(),
            ],
            artifacts,
            operations,
        },
        mods,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_release_validation_accepts_verified_https_release() {
        let resolved = resolve_published_release(
            PublishedRelease {
                id: "economy".into(),
                name: "Economy".into(),
                version: "1.0.0".into(),
                file_name: "economy-forge-1.20.1-1.0.0.jar".into(),
                source: "GitHub · UpperMoon0/Economy".into(),
                url: "https://example.invalid/economy.jar".into(),
                sha256: "a".repeat(64),
            },
            vec![Target::Client, Target::Server],
        )
        .unwrap();

        assert_eq!(resolved.id, "economy");
        assert_eq!(resolved.targets, vec![Target::Client, Target::Server]);
        assert_eq!(resolved.sha256, "a".repeat(64));
    }

    #[test]
    fn tfg_manifest_replaces_original_horse_power_and_keeps_client_only_mods_off_server() {
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
                "create-precise-controls",
                "Create: Precise Controls",
                vec![Target::Client],
            ),
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

        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::InstallFile { artifact, targets, .. }
                if artifact == "create-precise-controls"
                    && targets.as_slice() == [Target::Client]
        )));
        assert!(!patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::InstallFile { artifact, targets, .. }
                if artifact == "create-precise-controls" && targets.contains(&Target::Server)
        )));
        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::RemoveMatching { pattern, targets }
                if pattern == "mods/create-precise-controls-*.jar"
                    && targets.contains(&Target::Server)
        )));

        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::WriteText { destination, content, targets }
                if destination == "kubejs/server_scripts/create_horse_power/recipes.js"
                    && content.contains("tfg:shaped/horse_crank_bronze")
                    && targets.contains(&Target::Server)
                    && targets.contains(&Target::Client)
        )));
        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::PatchToml { destination, values, skip_if_missing, targets }
                if destination == "defaultconfigs/createhorsepower-server.toml"
                    && !skip_if_missing
                    && values.get("balance.enableIndividualAnimalStats") == Some(&serde_json::Value::Bool(false))
                    && values.get("path.evaluationMode") == Some(&serde_json::Value::String("LEGACY".into()))
                    && values.get("path.enableStressScaling") == Some(&serde_json::Value::Bool(false))
                    && targets.contains(&Target::Server)
                    && targets.contains(&Target::Client)
        )));
        assert!(!patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::PatchToml { destination, .. }
                if destination == "world/serverconfig/createhorsepower-server.toml"
                    || destination == "serverconfig/createhorsepower-server.toml"
        )));
        assert!(patch.manifest.operations.iter().any(|operation| matches!(
            operation,
            Operation::PatchYaml { destination, values, skip_if_missing, targets }
                if destination == "config/gtceu.yaml"
                    && !skip_if_missing
                    && values.get("machines.shouldWeatherOrTerrainExplosion")
                        == Some(&serde_json::Value::Bool(false))
                    && targets.as_slice() == [Target::Server]
        )));

        assert!(
            !patch
                .manifest
                .required_paths
                .iter()
                .any(|path| path == "config/gtceu.yaml"),
            "server-only GTCEu config must not be a global client requirement"
        );
    }
}
