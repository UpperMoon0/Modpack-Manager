use patch_core::{
    apply_manifest, load_manifest, load_patch_channel, plan_manifest, read_state, ApplyResult,
    PatchPlan, PatchProgress, PatchState, ProgressCallback, ResolvedPatchChannel, Target,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[tauri::command]
async fn resolve_patch_channel(source: String) -> Result<ResolvedPatchChannel, String> {
    load_patch_channel(&source).await.map_err(format_error)
}

#[tauri::command]
async fn plan_patch(manifest_source: String, root: String) -> Result<PatchPlan, String> {
    let manifest = load_manifest(&manifest_source)
        .await
        .map_err(format_error)?;
    plan_manifest(&manifest, &PathBuf::from(root), Target::Client).map_err(format_error)
}

#[tauri::command]
async fn apply_patch(
    app: AppHandle,
    manifest_source: String,
    root: String,
) -> Result<ApplyResult, String> {
    let manifest = load_manifest(&manifest_source)
        .await
        .map_err(format_error)?;

    let progress_app = app.clone();
    let progress: ProgressCallback = Arc::new(move |event: PatchProgress| {
        let _ = progress_app.emit("patch-progress", event);
    });

    apply_manifest(
        &manifest,
        &manifest_source,
        &PathBuf::from(root),
        Target::Client,
        Some(progress),
    )
    .await
    .map_err(format_error)
}

#[tauri::command]
fn read_patch_state(root: String) -> Result<Option<PatchState>, String> {
    read_state(&PathBuf::from(root), Target::Client).map_err(format_error)
}

fn format_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            resolve_patch_channel,
            plan_patch,
            apply_patch,
            read_patch_state
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Modpack Manager");
}
