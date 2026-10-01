use patch_core::{apply_manifest, plan_manifest, PatchManifest, Target};
use sha2::{Digest, Sha256};
use std::fs;

#[tokio::test]
async fn different_artifacts_with_identical_filenames_keep_their_verified_content() {
    let root = tempfile::tempdir().unwrap();
    let sources = tempfile::tempdir().unwrap();
    let a = sources.path().join("first.js");
    let b = sources.path().join("second.js");
    fs::write(&a, b"// first recipe").unwrap();
    fs::write(&b, b"// second recipe").unwrap();
    let manifest: PatchManifest = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "id": "cache-collision", "name": "Cache collision", "version": "1",
        "artifacts": [
            {"id": "first", "url": a.to_str().unwrap(), "fileName": "recipes.js",
                "sha256": hex::encode(Sha256::digest(b"// first recipe"))},
            {"id": "second", "url": b.to_str().unwrap(), "fileName": "recipes.js",
                "sha256": hex::encode(Sha256::digest(b"// second recipe"))}
        ],
        "operations": [
            {"type": "installFile", "artifact": "first", "destination": "kubejs/first/recipes.js"},
            {"type": "installFile", "artifact": "second", "destination": "kubejs/second/recipes.js"}
        ]
    }))
    .unwrap();
    apply_manifest(&manifest, "patch.json", root.path(), Target::Client, None)
        .await
        .unwrap();
    assert_eq!(
        fs::read(root.path().join("kubejs/first/recipes.js")).unwrap(),
        b"// first recipe"
    );
    assert_eq!(
        fs::read(root.path().join("kubejs/second/recipes.js")).unwrap(),
        b"// second recipe"
    );
    assert!(plan_manifest(&manifest, root.path(), Target::Client)
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        apply_manifest(&manifest, "patch.json", root.path(), Target::Client, None)
            .await
            .unwrap()
            .changed_paths,
        0
    );
}
