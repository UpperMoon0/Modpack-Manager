use crate::source::{load_text, resolve_artifact_source};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

fn default_check_interval_minutes() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchChannel {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default = "default_check_interval_minutes")]
    pub check_interval_minutes: u64,
    pub latest: PatchChannelRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchChannelRelease {
    pub version: String,
    pub manifest: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPatchChannel {
    pub id: String,
    pub name: String,
    pub check_interval_minutes: u64,
    pub version: String,
    pub manifest_source: String,
    pub notes: String,
    pub published_at: Option<String>,
}

pub async fn load_patch_channel(source: &str) -> Result<ResolvedPatchChannel> {
    let text = load_text(source).await?;
    let channel: PatchChannel =
        serde_json::from_str(&text).context("patch channel is not valid JSON")?;

    if channel.schema_version != 1 {
        bail!(
            "unsupported patch channel schema version {}; expected 1",
            channel.schema_version
        );
    }
    if channel.id.trim().is_empty() {
        bail!("patch channel id cannot be empty");
    }
    if channel.name.trim().is_empty() {
        bail!("patch channel name cannot be empty");
    }
    if channel.latest.version.trim().is_empty() {
        bail!("patch channel latest.version cannot be empty");
    }
    if channel.latest.manifest.trim().is_empty() {
        bail!("patch channel latest.manifest cannot be empty");
    }

    let check_interval_minutes = channel.check_interval_minutes.clamp(5, 24 * 60);
    let manifest_source = resolve_artifact_source(source, &channel.latest.manifest)
        .context("failed to resolve patch channel manifest source")?;

    Ok(ResolvedPatchChannel {
        id: channel.id,
        name: channel.name,
        check_interval_minutes,
        version: channel.latest.version,
        manifest_source,
        notes: channel.latest.notes,
        published_at: channel.latest.published_at,
    })
}
