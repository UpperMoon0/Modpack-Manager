use patch_core::load_patch_channel;
use std::fs;

#[tokio::test]
async fn resolves_relative_manifest_from_a_stable_channel_source() {
    let temp = tempfile::tempdir().unwrap();
    let channel = temp.path().join("channel.json");
    let manifest = temp.path().join("patch.json");

    fs::write(&manifest, "{}").unwrap();
    fs::write(
        &channel,
        r#"{
          "schemaVersion": 1,
          "id": "tfg-stable",
          "name": "TFG Stable",
          "checkIntervalMinutes": 30,
          "latest": {
            "version": "2026.09.20.2",
            "manifest": "./patch.json",
            "notes": "test"
          }
        }"#,
    )
    .unwrap();

    let resolved = load_patch_channel(channel.to_str().unwrap()).await.unwrap();

    assert_eq!(resolved.id, "tfg-stable");
    assert_eq!(resolved.version, "2026.09.20.2");
    assert_eq!(resolved.check_interval_minutes, 30);
    assert_eq!(resolved.manifest_source, manifest.to_string_lossy());
}

#[tokio::test]
async fn clamps_remote_polling_interval() {
    let temp = tempfile::tempdir().unwrap();
    let channel = temp.path().join("channel.json");
    fs::write(
        &channel,
        r#"{
          "schemaVersion": 1,
          "id": "fast",
          "name": "Fast",
          "checkIntervalMinutes": 1,
          "latest": {
            "version": "1",
            "manifest": "./patch.json"
          }
        }"#,
    )
    .unwrap();

    let resolved = load_patch_channel(channel.to_str().unwrap()).await.unwrap();
    assert_eq!(resolved.check_interval_minutes, 5);
}
