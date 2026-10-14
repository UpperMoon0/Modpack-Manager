use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::Client;
use std::path::{Component, Path, PathBuf};
use url::Url;

fn http_client(source: &str) -> Result<Client> {
    let mut headers = HeaderMap::new();

    if source.starts_with("https://api.github.com/") {
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );

        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            if !token.trim().is_empty() {
                let value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("GITHUB_TOKEN contains invalid header characters")?;
                headers.insert(AUTHORIZATION, value);
            }
        }
    }

    Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300))
        .user_agent("Modpack-Manager/1.0 (+https://github.com/UpperMoon0/Modpack-Manager)")
        .default_headers(headers)
        .build()
        .context("failed to create HTTP client")
}

pub async fn load_text(source: &str) -> Result<String> {
    if is_http(source) {
        return String::from_utf8(http_bytes(source).await?).context("response is not UTF-8");
    }

    let path = local_source_path(source)?;
    tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))
}

pub async fn load_bytes(source: &str) -> Result<Vec<u8>> {
    if is_http(source) {
        return http_bytes(source).await;
    }

    let path = local_source_path(source)?;
    tokio::fs::read(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))
}

async fn http_bytes(source: &str) -> Result<Vec<u8>> {
    for attempt in 0..4 {
        // A fresh connection also recovers intermittent TLS handshake failures.
        let response = http_client(source)?.get(source).send().await;
        let result = match response {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() && status.as_u16() != 429 && !status.is_server_error() {
                    return Err(response.error_for_status().unwrap_err().into());
                }
                match response.error_for_status() {
                    Ok(response) => response.bytes().await.map(|b| b.to_vec()),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) if attempt == 3 => return Err(error).with_context(|| format!("failed to download {source} after bounded retries")),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(250 << attempt)).await,
        }
    }
    unreachable!()
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
