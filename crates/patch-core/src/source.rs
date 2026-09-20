use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::path::{Component, Path, PathBuf};
use url::Url;

fn http_client() -> Result<Client> {
    Client::builder()
        .user_agent("Modpack-Manager/1.0 (+https://github.com/UpperMoon0/Modpack-Manager)")
        .build()
        .context("failed to create HTTP client")
}

pub async fn load_text(source: &str) -> Result<String> {
    if is_http(source) {
        let response = http_client()?
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
        let response = http_client()?
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
    Ok(normalize_local_path(parent.join(artifact_source))
        .to_string_lossy()
        .into_owned())
}

fn normalize_local_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
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
