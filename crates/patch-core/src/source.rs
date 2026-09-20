use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};
use url::Url;

pub async fn load_text(source: &str) -> Result<String> {
    if is_http(source) {
        let response = Client::new()
            .get(source)
            .send()
            .await
            .with_context(|| format!("failed to download {source}"))?
            .error_for_status()
            .with_context(|| format!("server rejected {source}"))?;
        return response.text().await.context("failed to read response body");
    }

    let path = local_source_path(source)?;
    tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))
}

pub async fn load_bytes(source: &str) -> Result<Vec<u8>> {
    if is_http(source) {
        let response = Client::new()
            .get(source)
            .send()
            .await
            .with_context(|| format!("failed to download {source}"))?
            .error_for_status()
            .with_context(|| format!("server rejected {source}"))?;
        return Ok(response
            .bytes()
            .await
            .context("failed to read response body")?
            .to_vec());
    }

    let path = local_source_path(source)?;
    tokio::fs::read(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))
}

pub fn resolve_artifact_source(manifest_source: &str, artifact_source: &str) -> Result<String> {
    if is_http(artifact_source) || artifact_source.starts_with("file://") {
        return Ok(artifact_source.to_owned());
    }

    if is_http(manifest_source) {
        let base = Url::parse(manifest_source).context("invalid manifest URL")?;
        return Ok(base
            .join(artifact_source)
            .context("invalid relative artifact URL")?
            .to_string());
    }

    let manifest_path = local_source_path(manifest_source)?;
    let parent = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(artifact_source).to_string_lossy().into_owned())
}

fn is_http(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn local_source_path(source: &str) -> Result<PathBuf> {
    if let Some(value) = source.strip_prefix("file://") {
        let url = Url::parse(source).context("invalid file URL")?;
        return url
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("invalid file URL path {value:?}"));
    }
    if source.contains("://") {
        bail!("unsupported source scheme in {source:?}");
    }
    Ok(PathBuf::from(source))
}
