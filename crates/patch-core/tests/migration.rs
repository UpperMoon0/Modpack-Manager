use patch_core::{apply_manifest, plan_manifest, PatchManifest, Target};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Write};
use zip::{write::SimpleFileOptions, ZipWriter};

fn hash(data: &[u8]) -> String { hex::encode(Sha256::digest(data)) }

fn fixture(bad_hash: bool) -> (tempfile::TempDir, tempfile::TempDir, PatchManifest) {
    let root = tempfile::tempdir().unwrap(); let source = tempfile::tempdir().unwrap();
    for d in ["mods","config","defaultconfigs","kubejs","world"] { fs::create_dir(root.path().join(d)).unwrap(); }
    fs::write(root.path().join("mods/old-core.jar"),b"old-core").unwrap();
    fs::write(root.path().join("kubejs/obsolete.js"),b"old script").unwrap();
    fs::write(root.path().join("kubejs/custom.js"),b"custom script").unwrap();
    fs::write(root.path().join("config/settings.toml"),b"[settings]\ncustom=9\ndefault=2\nremoved=3\n").unwrap();
    fs::write(root.path().join("config/trueuuid-registry.json"),b"private identity").unwrap();
    fs::write(root.path().join("world/level.dat"),b"world data").unwrap();
    let old=b"[settings]\ncustom=1\ndefault=2\nremoved=3\n";
    let new=b"[settings]\ncustom=4\ndefault=5\nadded=6\n";
    let archive=source.path().join("base.zip");
    let mut zip=ZipWriter::new(fs::File::create(&archive).unwrap());
    let index=json!({"schemaVersion":1,"files":{"config/settings.toml":if bad_hash {"0".repeat(64)} else {hash(new)},"kubejs/new.js":hash(b"new script")},"previousFiles":{"config/settings.toml":hash(old),"kubejs/obsolete.js":hash(b"old script"),"mods/old-core.jar":hash(b"old-core")}});
    for (name,data) in [("migration-index.json",serde_json::to_vec(&index).unwrap()),("files/config/settings.toml",new.to_vec()),("files/kubejs/new.js",b"new script".to_vec()),("previous/config/settings.toml",old.to_vec())] {
        zip.start_file(name,SimpleFileOptions::default()).unwrap();zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
    let core=source.path().join("core.jar");fs::write(&core,b"new-core").unwrap();
    let m=serde_json::from_value(json!({"schemaVersion":1,"id":"migration","name":"Migration","version":"12","requiredPaths":["mods","config","kubejs","defaultconfigs",".nstut-tfg-base-12"],"artifacts":[{"id":"base","url":archive.to_str().unwrap(),"sha256":hash(&fs::read(&archive).unwrap())},{"id":"core","url":core.to_str().unwrap(),"sha256":hash(b"new-core")}],"operations":[{"type":"migratePack","artifact":"base","sourceFile":"mods/old-core.jar","sourceSha256":hash(b"old-core"),"targetFile":"mods/new-core.jar","targetSha256":hash(b"new-core"),"marker":".nstut-tfg-base-12","markerContent":"base12\n","targets":["server","client"]},{"type":"installFile","artifact":"core","destination":"mods/new-core.jar"}]})).unwrap();
    (root,source,m)
}

#[tokio::test]
async fn upgrades_installed_base_preserving_world_identity_custom_files_and_settings() {
    let (root,_source,m)=fixture(false);
    assert!(plan_manifest(&m,root.path(),Target::Server).unwrap().items.iter().any(|i| i.kind=="migratePack"));
    apply_manifest(&m,"migration.json",root.path(),Target::Server,None).await.unwrap();
    assert!(!root.path().join("mods/old-core.jar").exists());
    assert!(!root.path().join("kubejs/obsolete.js").exists());
    assert_eq!(fs::read(root.path().join("world/level.dat")).unwrap(),b"world data");
    assert_eq!(fs::read(root.path().join("config/trueuuid-registry.json")).unwrap(),b"private identity");
    assert_eq!(fs::read(root.path().join("kubejs/custom.js")).unwrap(),b"custom script");
    let config=fs::read_to_string(root.path().join("config/settings.toml")).unwrap();
    assert!(config.contains("custom = 9") && config.contains("default = 5") && config.contains("added = 6") && !config.contains("removed"));
    assert!(plan_manifest(&m,root.path(),Target::Server).unwrap().items.is_empty());
    assert_eq!(apply_manifest(&m,"migration.json",root.path(),Target::Server,None).await.unwrap().changed_paths,0);
}

#[tokio::test]
async fn rejects_bad_archive_payload_before_any_mutation() {
    let (root,_source,m)=fixture(true);
    assert!(apply_manifest(&m,"migration.json",root.path(),Target::Client,None).await.is_err());
    assert_eq!(fs::read(root.path().join("mods/old-core.jar")).unwrap(),b"old-core");
    assert!(!root.path().join("kubejs/new.js").exists());
}

#[tokio::test]
async fn rolls_back_entire_base_when_later_overlay_fails() {
    let (root,_source,mut m)=fixture(false);
    fs::write(root.path().join("config/broken.toml"),b"[broken").unwrap();
    m.operations.push(serde_json::from_value(json!({"type":"patchToml","destination":"config/broken.toml","values":{"x":1}})).unwrap());
    assert!(apply_manifest(&m,"migration.json",root.path(),Target::Server,None).await.is_err());
    assert_eq!(fs::read(root.path().join("mods/old-core.jar")).unwrap(),b"old-core");
    assert!(!root.path().join("mods/new-core.jar").exists());
    assert!(!root.path().join(".nstut-tfg-base-12").exists());
    assert!(root.path().join("kubejs/obsolete.js").exists());
}

#[tokio::test]
async fn resumes_exact_interrupted_migration_after_old_core_removal() {
    let (root,_source,m)=fixture(false);
    fs::create_dir(root.path().join(".modpack-manager")).unwrap();
    fs::write(root.path().join(".modpack-manager/.nstut-tfg-base-12.pending"),hash(&serde_json::to_vec(&m.operations[0]).unwrap())).unwrap();
    fs::remove_file(root.path().join("mods/old-core.jar")).unwrap();
    apply_manifest(&m,"migration.json",root.path(),Target::Client,None).await.unwrap();
    assert!(plan_manifest(&m,root.path(),Target::Client).unwrap().items.is_empty());
}

#[test]
fn rejects_unknown_base_and_forged_completion_marker() {
    let (root,_source,m)=fixture(false);
    fs::write(root.path().join("mods/old-core.jar"),b"wrong core").unwrap();
    assert!(plan_manifest(&m,root.path(),Target::Server).is_err());
    fs::write(root.path().join(".nstut-tfg-base-12"),b"base12\n").unwrap();
    assert!(plan_manifest(&m,root.path(),Target::Server).is_err());
}

#[tokio::test]
async fn prepares_matching_overlay_before_base_replaces_it() {
    let (root,source,mut m)=fixture(false);
    let data=b"already correct custom overlay";
    fs::write(root.path().join("kubejs/new.js"),data).unwrap();
    let overlay=source.path().join("overlay.js");fs::write(&overlay,data).unwrap();
    m.artifacts.push(serde_json::from_value(json!({"id":"overlay","url":overlay.to_str().unwrap(),"sha256":hash(data)})).unwrap());
    m.operations.push(serde_json::from_value(json!({"type":"installFile","artifact":"overlay","destination":"kubejs/new.js"})).unwrap());
    apply_manifest(&m,"migration.json",root.path(),Target::Server,None).await.unwrap();
    assert_eq!(fs::read(root.path().join("kubejs/new.js")).unwrap(),data);
    assert!(plan_manifest(&m,root.path(),Target::Server).unwrap().items.is_empty());
}

#[tokio::test]
async fn repairs_missing_or_modified_core_after_committed_migration() {
    for target in [Target::Client, Target::Server] {
        for missing in [true, false] {
            let (root, source, m) = fixture(false);
            apply_manifest(&m, "migration.json", root.path(), target, None).await.unwrap();
            let core = root.path().join("mods/new-core.jar");
            if missing { fs::remove_file(&core).unwrap(); }
            else { fs::write(&core, b"modified core").unwrap(); }
            // Repair must not download or reapply the completed base migration.
            fs::remove_file(source.path().join("base.zip")).unwrap();
            fs::write(root.path().join("kubejs/new.js"), b"user edit after migration").unwrap();
            let plan = plan_manifest(&m, root.path(), target).unwrap();
            assert!(plan.already_applied);
            assert_eq!(plan.items.len(), 1);
            assert_eq!(plan.items[0].path, "mods/new-core.jar");
            apply_manifest(&m, "migration.json", root.path(), target, None).await.unwrap();
            assert_eq!(fs::read(core).unwrap(), b"new-core");
            assert_eq!(fs::read(root.path().join("kubejs/new.js")).unwrap(), b"user edit after migration");
            assert_eq!(fs::read(root.path().join("world/level.dat")).unwrap(), b"world data");
            assert!(plan_manifest(&m, root.path(), target).unwrap().items.is_empty());
        }
    }
}

#[tokio::test]
async fn damaged_core_still_requires_matching_installation_state_and_marker() {
    let (root, _source, m) = fixture(false);
    apply_manifest(&m, "migration.json", root.path(), Target::Client, None).await.unwrap();
    fs::remove_file(root.path().join("mods/new-core.jar")).unwrap();
    let state_path = root.path().join(".modpack-manager/state-client.json");
    let original = fs::read(&state_path).unwrap();
    for (field, value) in [("manifestId", "other-pack"), ("target", "server")] {
        let mut state: serde_json::Value = serde_json::from_slice(&original).unwrap();
        state[field] = json!(value);
        fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(plan_manifest(&m, root.path(), Target::Client).is_err());
    }
    fs::write(&state_path, original).unwrap();
    fs::write(root.path().join(".nstut-tfg-base-12"), b"wrong marker").unwrap();
    assert!(plan_manifest(&m, root.path(), Target::Client).is_err());
}
