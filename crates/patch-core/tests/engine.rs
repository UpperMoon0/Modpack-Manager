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
