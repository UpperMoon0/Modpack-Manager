use patch_core::{
    apply_manifest, plan_manifest, read_state, Artifact, Operation, PatchManifest, Target,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn basic_root() -> TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("mods")).unwrap();
    fs::create_dir_all(temp.path().join("config")).unwrap();
    temp
}

fn artifact(id: &str, path: &Path) -> Artifact {
    let bytes = fs::read(path).unwrap();
    Artifact {
        id: id.to_owned(),
        url: path.file_name().unwrap().to_string_lossy().into_owned(),
        sha256: sha256(&bytes),
        file_name: None,
    }
}

fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);

    for (name, content) in files {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(content).unwrap();
    }

    zip.finish().unwrap();
}

fn manifest_source(temp: &TempDir) -> PathBuf {
    let path = temp.path().join("patch.json");
    fs::write(&path, "{}").unwrap();
    path
}

#[tokio::test]
async fn applies_custom_mod_and_script_patch_idempotently() {
    let root = basic_root();
    fs::write(root.path().join("mods/Economy-old.jar"), b"old").unwrap();
    fs::create_dir_all(root.path().join("kubejs/server_scripts")).unwrap();
    fs::write(root.path().join("kubejs/server_scripts/stale.js"), b"stale").unwrap();

    let payloads = tempfile::tempdir().unwrap();
    let jar = payloads.path().join("Economy-new.jar");
    fs::write(&jar, b"new-jar").unwrap();

    let scripts = payloads.path().join("kubejs.zip");
    write_zip(
        &scripts,
        &[
            ("server_scripts/main.js", b"ServerEvents.loaded(() => {});"),
            ("startup_scripts/init.js", b"console.info('patched');"),
        ],
    );

    let source = manifest_source(&payloads);
    let manifest = PatchManifest {
        schema_version: 1,
        id: "tfg-test".into(),
        name: "TFG test patch".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["mods".into(), "config".into()],
        artifacts: vec![artifact("economy", &jar), artifact("scripts", &scripts)],
        operations: vec![
            Operation::RemoveMatching {
                pattern: "mods/Economy-*.jar".into(),
                targets: vec![Target::Client, Target::Server],
            },
            Operation::InstallFile {
                artifact: "economy".into(),
                destination: "mods/Economy-new.jar".into(),
                targets: vec![Target::Client, Target::Server],
            },
            Operation::ExtractZip {
                artifact: "scripts".into(),
                destination: "kubejs".into(),
                clean_destination: true,
                targets: vec![Target::Client, Target::Server],
            },
            Operation::WriteText {
                destination: "config/patch-version.txt".into(),
                content: "1\n".into(),
                targets: vec![Target::Server],
            },
        ],
    };

    let plan = plan_manifest(&manifest, root.path(), Target::Server).unwrap();
    assert!(!plan.already_applied);
    assert!(plan
        .items
        .iter()
        .any(|item| item.path == "mods/Economy-old.jar"));

    let result = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();

    assert!(result.changed_paths >= 4);
    assert!(!root.path().join("mods/Economy-old.jar").exists());
    assert_eq!(
        fs::read(root.path().join("mods/Economy-new.jar")).unwrap(),
        b"new-jar"
    );
    assert!(!root.path().join("kubejs/server_scripts/stale.js").exists());
    assert_eq!(
        fs::read(root.path().join("kubejs/server_scripts/main.js")).unwrap(),
        b"ServerEvents.loaded(() => {});"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("config/patch-version.txt")).unwrap(),
        "1\n"
    );

    let state = read_state(root.path(), Target::Server).unwrap().unwrap();
    assert_eq!(state.manifest_id, "tfg-test");
    assert_eq!(state.manifest_version, "1");

    let second_plan = plan_manifest(&manifest, root.path(), Target::Server).unwrap();
    assert!(second_plan.already_applied);

    apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        fs::read(root.path().join("mods/Economy-new.jar")).unwrap(),
        b"new-jar"
    );
}

#[tokio::test]
async fn rolls_back_when_a_later_operation_fails() {
    let root = basic_root();
    fs::write(root.path().join("config/value.txt"), b"original").unwrap();

    let payloads = tempfile::tempdir().unwrap();
    let broken = payloads.path().join("broken.zip");
    fs::write(&broken, b"not a zip").unwrap();
    let source = manifest_source(&payloads);

    let manifest = PatchManifest {
        schema_version: 1,
        id: "rollback-test".into(),
        name: "rollback".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["config".into()],
        artifacts: vec![artifact("broken", &broken)],
        operations: vec![
            Operation::WriteText {
                destination: "config/value.txt".into(),
                content: "changed".into(),
                targets: vec![Target::Server],
            },
            Operation::ExtractZip {
                artifact: "broken".into(),
                destination: "kubejs".into(),
                clean_destination: true,
                targets: vec![Target::Server],
            },
        ],
    };

    let error = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap_err();

    assert!(format!("{error:#}").contains("original files were restored"));
    assert_eq!(
        fs::read(root.path().join("config/value.txt")).unwrap(),
        b"original"
    );
    assert!(!root.path().join("kubejs").exists());
    assert!(read_state(root.path(), Target::Server).unwrap().is_none());
}

#[tokio::test]
async fn rejects_zip_path_traversal() {
    let root = basic_root();
    let payloads = tempfile::tempdir().unwrap();

    let archive = payloads.path().join("unsafe.zip");
    write_zip(&archive, &[("../escape.txt", b"nope")]);
    let source = manifest_source(&payloads);

    let manifest = PatchManifest {
        schema_version: 1,
        id: "zip-safety".into(),
        name: "zip safety".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["config".into()],
        artifacts: vec![artifact("unsafe", &archive)],
        operations: vec![Operation::ExtractZip {
            artifact: "unsafe".into(),
            destination: "kubejs".into(),
            clean_destination: true,
            targets: vec![Target::Server],
        }],
    };

    let error = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap_err();

    assert!(format!("{error:#}").contains("unsafe path"));
    assert!(!root.path().parent().unwrap().join("escape.txt").exists());
}

#[test]
fn rejects_manager_metadata_and_parent_traversal_targets() {
    let root = basic_root();

    for destination in [".modpack-manager/hack.txt", "../escape.txt"] {
        let manifest = PatchManifest {
            schema_version: 1,
            id: "safety".into(),
            name: "safety".into(),
            version: "1".into(),
            description: String::new(),
            required_paths: vec![],
            artifacts: vec![],
            operations: vec![Operation::WriteText {
                destination: destination.into(),
                content: "bad".into(),
                targets: vec![Target::Client],
            }],
        };

        assert!(plan_manifest(&manifest, root.path(), Target::Client).is_err());
    }
}


#[tokio::test]
async fn patch_toml_preserves_tfg_lists_and_unrelated_settings() {
    use std::collections::BTreeMap;

    let root = basic_root();
    fs::create_dir_all(root.path().join("defaultconfigs")).unwrap();
    let config_path = root.path().join("defaultconfigs/createhorsepower-server.toml");
    fs::write(
        &config_path,
        r#"creatureRPMRange = 4
largeCreatures = ["tfc:horse", "tfg:sniffer", "minecraft:zombie_horse"]
greatPathBlock = ["rnr:brick_road", "greate:steel_shaft"]

[balance]
globalRpmMultiplier = 1.0
enableIndividualAnimalStats = true

[custom]
keepMe = "yes"
"#,
    )
    .unwrap();

    let mut values = BTreeMap::new();
    values.insert("creatureRPMRange".into(), serde_json::Value::from(16));
    values.insert(
        "balance.enableIndividualAnimalStats".into(),
        serde_json::Value::from(false),
    );
    values.insert(
        "path.evaluationMode".into(),
        serde_json::Value::from("LEGACY"),
    );

    let manifest = PatchManifest {
        schema_version: 1,
        id: "toml-overlay".into(),
        name: "TOML overlay".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["defaultconfigs/createhorsepower-server.toml".into()],
        artifacts: vec![],
        operations: vec![Operation::PatchToml {
            destination: "defaultconfigs/createhorsepower-server.toml".into(),
            values,
            skip_if_missing: false,
            targets: vec![Target::Server],
        }],
    };

    let source = manifest_source(&root);
    apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();

    let patched = fs::read_to_string(&config_path).unwrap();
    let document = patched.parse::<toml_edit::DocumentMut>().unwrap();

    assert_eq!(document["creatureRPMRange"].as_integer(), Some(16));
    assert_eq!(
        document["largeCreatures"].as_array().unwrap().len(),
        3,
        "TFG worker list must be preserved"
    );
    assert_eq!(
        document["greatPathBlock"].as_array().unwrap().len(),
        2,
        "TFG path list must be preserved"
    );
    assert_eq!(
        document["balance"]["enableIndividualAnimalStats"].as_bool(),
        Some(false)
    );
    assert_eq!(
        document["path"]["evaluationMode"].as_str(),
        Some("LEGACY")
    );
    assert_eq!(document["custom"]["keepMe"].as_str(), Some("yes"));
}

#[tokio::test]
async fn optional_patch_toml_does_not_create_missing_active_world_config() {
    use std::collections::BTreeMap;

    let root = basic_root();
    let mut values = BTreeMap::new();
    values.insert("creatureRPMRange".into(), serde_json::Value::from(16));

    let manifest = PatchManifest {
        schema_version: 1,
        id: "toml-optional".into(),
        name: "optional TOML".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["config".into()],
        artifacts: vec![],
        operations: vec![Operation::PatchToml {
            destination: "world/serverconfig/createhorsepower-server.toml".into(),
            values,
            skip_if_missing: true,
            targets: vec![Target::Server],
        }],
    };

    let source = manifest_source(&root);
    apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();

    assert!(
        !root
            .path()
            .join("world/serverconfig/createhorsepower-server.toml")
            .exists()
    );
}


#[tokio::test]
async fn plan_reports_only_live_drift_and_apply_preserves_matching_managed_files() {
    use std::collections::BTreeMap;

    let root = basic_root();
    fs::create_dir_all(root.path().join("defaultconfigs")).unwrap();

    let payloads = tempfile::tempdir().unwrap();
    let jar = payloads.path().join("managed-new.jar");
    fs::write(&jar, b"desired-jar").unwrap();
    let source = manifest_source(&payloads);

    fs::write(root.path().join("mods/managed-new.jar"), b"desired-jar").unwrap();
    fs::write(root.path().join("mods/managed-old.jar"), b"stale").unwrap();
    fs::write(root.path().join("config/managed.txt"), b"desired\n").unwrap();
    fs::write(
        root.path().join("defaultconfigs/managed.toml"),
        "enabled = true\n",
    )
    .unwrap();

    let mut values = BTreeMap::new();
    values.insert("enabled".into(), serde_json::Value::from(true));

    let manifest = PatchManifest {
        schema_version: 1,
        id: "live-diff".into(),
        name: "Live diff".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["mods".into(), "config".into(), "defaultconfigs".into()],
        artifacts: vec![artifact("managed", &jar)],
        operations: vec![
            Operation::RemoveMatching {
                pattern: "mods/managed-*.jar".into(),
                targets: vec![Target::Client],
            },
            Operation::InstallFile {
                artifact: "managed".into(),
                destination: "mods/managed-new.jar".into(),
                targets: vec![Target::Client],
            },
            Operation::WriteText {
                destination: "config/managed.txt".into(),
                content: "desired\n".into(),
                targets: vec![Target::Client],
            },
            Operation::PatchToml {
                destination: "defaultconfigs/managed.toml".into(),
                values,
                skip_if_missing: false,
                targets: vec![Target::Client],
            },
        ],
    };

    let plan = plan_manifest(&manifest, root.path(), Target::Client).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].kind, "remove");
    assert_eq!(plan.items[0].path, "mods/managed-old.jar");

    let result = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Client,
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.changed_paths, 1);
    assert_eq!(
        fs::read(root.path().join("mods/managed-new.jar")).unwrap(),
        b"desired-jar"
    );

    let clean_plan = plan_manifest(&manifest, root.path(), Target::Client).unwrap();
    assert!(clean_plan.already_applied);
    assert!(clean_plan.items.is_empty());

    fs::write(root.path().join("mods/managed-new.jar"), b"tampered").unwrap();
    let drift = plan_manifest(&manifest, root.path(), Target::Client).unwrap();
    assert!(drift.already_applied);
    assert_eq!(drift.items.len(), 1);
    assert_eq!(drift.items[0].kind, "replace");
    assert_eq!(drift.items[0].path, "mods/managed-new.jar");
    assert!(drift.warnings.iter().any(|warning| warning.contains("live installation")));
}


#[tokio::test]
async fn patch_yaml_updates_nested_scalar_and_preserves_unrelated_gtceu_config() {
    use std::collections::BTreeMap;

    let root = basic_root();
    let config_path = root.path().join("config/gtceu.yaml");
    fs::write(
        &config_path,
        r#"machines:
  requireGTToolsForBlocks: true
  shouldWeatherOrTerrainExplosion: true
  energyUsageMultiplier: 100
  doesExplosionDamagesTerrain: true

recipes:
  harderBrickRecipes: true
"#,
    )
    .unwrap();

    let mut values = BTreeMap::new();
    values.insert(
        "machines.shouldWeatherOrTerrainExplosion".into(),
        serde_json::Value::from(false),
    );

    let manifest = PatchManifest {
        schema_version: 1,
        id: "yaml-overlay".into(),
        name: "YAML overlay".into(),
        version: "1".into(),
        description: String::new(),
        required_paths: vec!["config/gtceu.yaml".into()],
        artifacts: vec![],
        operations: vec![Operation::PatchYaml {
            destination: "config/gtceu.yaml".into(),
            values,
            skip_if_missing: false,
            targets: vec![Target::Server],
        }],
    };

    let plan = plan_manifest(&manifest, root.path(), Target::Server).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].kind, "patchYaml");

    let source = manifest_source(&root);
    let first = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();
    assert_eq!(first.changed_paths, 1);

    let patched = fs::read_to_string(&config_path).unwrap();
    assert!(patched.contains("  shouldWeatherOrTerrainExplosion: false"));
    assert!(patched.contains("  requireGTToolsForBlocks: true"));
    assert!(patched.contains("  energyUsageMultiplier: 100"));
    assert!(patched.contains("  doesExplosionDamagesTerrain: true"));
    assert!(patched.contains("recipes:\n  harderBrickRecipes: true"));

    let clean_plan = plan_manifest(&manifest, root.path(), Target::Server).unwrap();
    assert!(clean_plan.items.is_empty());

    let second = apply_manifest(
        &manifest,
        source.to_str().unwrap(),
        root.path(),
        Target::Server,
        None,
    )
    .await
    .unwrap();
    assert_eq!(second.changed_paths, 0);
}
