use patch_core::{
    apply_manifest, install_client, plan_client_install, plan_manifest, Operation, PatchManifest,
    PatchProgressPhase, Target,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, sync::Arc};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn fixture(bad_payload: bool) -> (tempfile::TempDir, PatchManifest) {
    let source = tempfile::tempdir().unwrap();
    let bytes = b"new pack defaults";
    let mut zip = ZipWriter::new(fs::File::create(source.path().join("base.zip")).unwrap());
    let index = json!({"schemaVersion":1,"files":{"config/default.txt":if bad_payload {"0".repeat(64)}else{hash(bytes)},"kubejs/script.js":hash(b"script")},"previousFiles":{"mods/old-core.jar":hash(b"old")}});
    for (name, data) in [
        ("migration-index.json", serde_json::to_vec(&index).unwrap()),
        ("files/config/default.txt", bytes.to_vec()),
        ("files/kubejs/script.js", b"script".to_vec()),
    ] {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
    fs::write(source.path().join("core.jar"), b"core").unwrap();
    let m=serde_json::from_value(json!({"schemaVersion":1,"id":"tfg","name":"TFG","version":"0.13.12-nstut.1","requiredPaths":["mods","config","kubejs","defaultconfigs",".nstut-tfg-base-0.13.12"],"artifacts":[{"id":"base","url":source.path().join("base.zip"),"sha256":hash(&fs::read(source.path().join("base.zip")).unwrap())},{"id":"core","url":source.path().join("core.jar"),"sha256":hash(b"core")}],"operations":[{"type":"migratePack","artifact":"base","sourceFile":"mods/old-core.jar","sourceSha256":hash(b"old"),"targetFile":"mods/core.jar","targetSha256":hash(b"core"),"marker":".nstut-tfg-base-0.13.12","markerContent":"complete\n","targets":["client"]},{"type":"installFile","artifact":"core","destination":"mods/core.jar","targets":["client"]}]})).unwrap();
    (source, m)
}

#[tokio::test]
async fn installs_empty_client_and_exports_a_complete_updateable_prism_instance() {
    let (_source, m) = fixture(false);
    let parent = tempfile::tempdir().unwrap();
    let dest = parent.path().join("fresh");
    let result = install_client(&m, "test.json", &dest, None).await.unwrap();
    let root = std::path::Path::new(&result.minecraft_root);
    assert_eq!(fs::read(root.join("mods/core.jar")).unwrap(), b"core");
    assert!(!root.join("mods/old-core.jar").exists());
    assert_eq!(
        fs::read(root.join("config/default.txt")).unwrap(),
        b"new pack defaults"
    );
    assert!(plan_manifest(&m, root, Target::Client)
        .unwrap()
        .items
        .is_empty());
    assert!(!root.join(".modpack-manager/cache").exists());
    assert!(!root.join(".modpack-manager/backups").exists());
    let meta: serde_json::Value =
        serde_json::from_slice(&fs::read(dest.join("mmc-pack.json")).unwrap()).unwrap();
    assert_eq!(meta["components"][0]["version"], "1.20.1");
    assert_eq!(meta["components"][1]["version"], "47.4.13");
    let mut zip = ZipArchive::new(fs::File::open(result.archive_path).unwrap()).unwrap();
    assert!(zip.by_name("instance.cfg").is_ok());
    assert!(zip.by_name("mmc-pack.json").is_ok());
    let imported = parent.path().join("imported");
    fs::create_dir(&imported).unwrap();
    zip.extract(&imported).unwrap();
    assert!(
        plan_manifest(&m, &imported.join(".minecraft"), Target::Client)
            .unwrap()
            .items
            .is_empty()
    );
    let names: Vec<_> = zip.file_names().collect();
    assert!(!names
        .iter()
        .any(|n| n.contains("cache/") || n.contains("backups/") || n.ends_with(".zip")));
    assert_eq!(
        apply_manifest(
            &m,
            "test.json",
            &imported.join(".minecraft"),
            Target::Client,
            None
        )
        .await
        .unwrap()
        .changed_paths,
        0
    );
}

#[tokio::test]
async fn rejects_existing_install_without_touching_its_world() {
    let (_source, m) = fixture(false);
    let dest = tempfile::tempdir().unwrap();
    fs::write(dest.path().join("world.dat"), b"private world").unwrap();
    assert!(install_client(&m, "test.json", dest.path(), None)
        .await
        .is_err());
    assert_eq!(
        fs::read(dest.path().join("world.dat")).unwrap(),
        b"private world"
    );
    assert_eq!(fs::read_dir(dest.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn failed_payload_or_overlay_leaves_destination_empty_and_cleans_staging() {
    for overlay in [false, true] {
        let (_source, mut m) = fixture(!overlay);
        let parent = tempfile::tempdir().unwrap();
        let dest = parent.path().join("empty");
        fs::create_dir(&dest).unwrap();
        if overlay {
            m.operations.push(
                serde_json::from_value(
                    json!({"type":"patchToml","destination":"config/default.txt","values":{"x":1}}),
                )
                .unwrap(),
            );
        }
        assert!(install_client(&m, "test.json", &dest, None).await.is_err());
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    }
}

#[tokio::test]
async fn rechecks_destination_after_downloads_without_overwriting_new_files() {
    let (_source, m) = fixture(false);
    let parent = tempfile::tempdir().unwrap();
    let dest = parent.path().join("fresh");
    let moved = dest.clone();
    let callback = Arc::new(move |event: patch_core::PatchProgress| {
        if matches!(event.phase, PatchProgressPhase::Done) && !moved.exists() {
            fs::create_dir(&moved).unwrap();
            fs::write(moved.join("new-user-file"), b"keep").unwrap();
        }
    });
    assert!(install_client(&m, "test.json", &dest, Some(callback))
        .await
        .is_err());
    assert_eq!(fs::read(dest.join("new-user-file")).unwrap(), b"keep");
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn rejects_overlay_only_unknown_bases_and_missing_parent() {
    let (_source, mut m) = fixture(false);
    let parent = tempfile::tempdir().unwrap();
    assert!(plan_client_install(&m, &parent.path().join("missing/child")).is_err());
    m.version = "0.13.10-nstut.34".into();
    assert!(plan_client_install(&m, &parent.path().join("new")).is_err());
    m.version = "0.13.12-nstut.1\nInjected=true".into();
    assert!(plan_client_install(&m, &parent.path().join("new")).is_err());
    m.version = "0.13.12-nstut.1".into();
    m.operations
        .retain(|o| !matches!(o, Operation::MigratePack { .. }));
    assert!(plan_client_install(&m, &parent.path().join("new")).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_destination_or_ancestor() {
    let (_source, m) = fixture(false);
    let parent = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(other.path(), &link).unwrap();
    assert!(plan_client_install(&m, &link).is_err());
    assert!(plan_client_install(&m, &link.join("child")).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_non_utf8_destination_without_publishing_to_a_different_path() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let (_source, m) = fixture(false);
    let parent = tempfile::tempdir().unwrap();
    let dest = parent.path().join(OsString::from_vec(b"fresh-\xff".to_vec()));
    fs::create_dir(&dest).unwrap();
    let replacement = parent.path().join("fresh-\u{fffd}");
    let error = install_client(&m, "test.json", &dest, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("UTF-8"));
    assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
    assert!(!replacement.exists());
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    assert!(plan_client_install(&m, &dest.join("child")).is_err());
}
