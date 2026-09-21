use patch_core::{
    apply_manifest, load_manifest, load_patch_channel, plan_manifest, read_state, resolve_tfg_patch,
    ApplyResult, PatchPlan, PatchProgress, PatchState, ProgressCallback, ResolvedPatchChannel,
    Target, TfgResolvedMod,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const TFG_SOURCE: &str = "tfg://managed/forge-1.20.1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TfgPlanResponse {
    patch_version: String,
    mods: Vec<TfgResolvedMod>,
    plan: PatchPlan,
}

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

    let progress = progress_callback(app);

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
async fn plan_tfg_patch(root: String) -> Result<TfgPlanResponse, String> {
    let resolved = resolve_tfg_patch().await.map_err(format_error)?;
    let plan =
        plan_manifest(&resolved.manifest, &PathBuf::from(root), Target::Client).map_err(format_error)?;

    Ok(TfgPlanResponse {
        patch_version: resolved.manifest.version,
        mods: resolved.mods,
        plan,
    })
}

#[tauri::command]
async fn apply_tfg_patch(
    app: AppHandle,
    root: String,
    expected_patch_version: String,
) -> Result<ApplyResult, String> {
    let resolved = resolve_tfg_patch().await.map_err(format_error)?;

    if resolved.manifest.version != expected_patch_version {
        return Err(format!(
            "TFG managed releases changed while this plan was open (expected {}, now {}). Check for updates again before applying.",
            expected_patch_version, resolved.manifest.version
        ));
    }

    apply_manifest(
        &resolved.manifest,
        TFG_SOURCE,
        &PathBuf::from(root),
        Target::Client,
        Some(progress_callback(app)),
    )
    .await
    .map_err(format_error)
}

#[tauri::command]
fn read_patch_state(root: String) -> Result<Option<PatchState>, String> {
    read_state(&PathBuf::from(root), Target::Client).map_err(format_error)
}

fn progress_callback(app: AppHandle) -> ProgressCallback {
    Arc::new(move |event: PatchProgress| {
        let _ = app.emit("patch-progress", event);
    })
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
            plan_tfg_patch,
            apply_tfg_patch,
            read_patch_state
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Modpack Manager");
}
