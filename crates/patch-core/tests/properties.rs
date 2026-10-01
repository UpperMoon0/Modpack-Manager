use patch_core::{apply_manifest, plan_manifest, PatchManifest, Target};
use std::fs;
use tempfile::TempDir;

fn manifest() -> PatchManifest {
    serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "id": "server-settings", "name": "Server settings", "version": "1",
        "operations": [{ "type": "patchProperties", "destination": "server.properties",
            "values": {"online-mode": false}, "targets": ["server"] }]
    }))
    .unwrap()
}

async fn apply(
    manifest: &PatchManifest,
    root: &TempDir,
) -> anyhow::Result<patch_core::ApplyResult> {
    apply_manifest(manifest, "patch.json", root.path(), Target::Server, None).await
}

#[tokio::test]
async fn offline_mode_preserves_settings_and_is_idempotent_and_server_only() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("server.properties");
    let original =
        "# server settings\r\nonline-mode=true\r\nlevel-name=custom-world\r\nserver-port=25570\r\n";
    fs::write(&path, original).unwrap();
    let manifest = manifest();
    assert!(plan_manifest(&manifest, root.path(), Target::Client)
        .unwrap()
        .items
        .is_empty());
    let plan = plan_manifest(&manifest, root.path(), Target::Server).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].kind, "patchProperties");
    assert_eq!(apply(&manifest, &root).await.unwrap().changed_paths, 1);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        original.replace("online-mode=true", "online-mode=false")
    );
    assert!(plan_manifest(&manifest, root.path(), Target::Server)
        .unwrap()
        .items
        .is_empty());
    assert_eq!(apply(&manifest, &root).await.unwrap().changed_paths, 0);
}

#[tokio::test]
async fn properties_creation_and_skip_if_missing_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let mut value = serde_json::to_value(manifest()).unwrap();
    value["operations"][0]["skipIfMissing"] = true.into();
    let skipped: PatchManifest = serde_json::from_value(value).unwrap();
    assert_eq!(apply(&skipped, &root).await.unwrap().changed_paths, 0);
    assert!(!root.path().join("server.properties").exists());
    assert_eq!(apply(&manifest(), &root).await.unwrap().changed_paths, 1);
    assert_eq!(
        fs::read_to_string(root.path().join("server.properties")).unwrap(),
        "online-mode=false\n"
    );
}

#[tokio::test]
async fn restores_properties_when_a_later_operation_fails() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("server.properties");
    let original = b"online-mode=true\nlevel-name=world-two\n";
    fs::write(&path, original).unwrap();
    // A malformed TOML document fails after the properties change has been backed up.
    fs::write(root.path().join("broken.toml"), "[broken").unwrap();
    let mut value = serde_json::to_value(manifest()).unwrap();
    value["operations"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "type": "patchToml", "destination": "broken.toml", "values": {"enabled": true}
        }));
    let manifest: PatchManifest = serde_json::from_value(value).unwrap();
    assert!(apply(&manifest, &root).await.is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn rejects_path_traversal_and_property_key_injection() {
    let root = tempfile::tempdir().unwrap();
    for (field, value) in [
        ("destination", serde_json::json!("../server.properties")),
        (
            "values",
            serde_json::json!({"online-mode\nserver-port": false}),
        ),
    ] {
        let mut input = serde_json::to_value(manifest()).unwrap();
        input["operations"][0][field] = value;
        let manifest: PatchManifest = serde_json::from_value(input).unwrap();
        assert!(plan_manifest(&manifest, root.path(), Target::Server).is_err());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_symlinked_server_properties() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = outside.path().join("server.properties");
    fs::write(&path, "online-mode=true\n").unwrap();
    std::os::unix::fs::symlink(&path, root.path().join("server.properties")).unwrap();
    assert!(plan_manifest(&manifest(), root.path(), Target::Server).is_err());
    assert!(apply(&manifest(), &root).await.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "online-mode=true\n");
}
