use crate::auth::HTTP_TIMEOUT_SECS;
use crate::error::{AgentError, AgentResult};
use crate::models::TrackingScreenshotImage;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;

use super::storage::download_capture;
use super::{cache_dir_for_db, cache_file_path};

/// Número máximo de arquivos no cache de screenshots antes de fazer
/// evicção dos mais antigos.
const CACHE_MAX_FILES: usize = 200;

/// Verifica o diretório de cache e remove os arquivos mais antigos se
/// o número total exceder `CACHE_MAX_FILES`.
fn evict_cache_if_needed(db_path: &Path) {
    let cache_dir = cache_dir_for_db(db_path);
    if !cache_dir.is_dir() {
        return;
    }

    let mut entries: Vec<_> = match fs::read_dir(&cache_dir) {
        Ok(reader) => reader
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .collect(),
        Err(_) => return,
    };

    if entries.len() <= CACHE_MAX_FILES {
        return;
    }

    // Ordenar por data de modificação (mais antigos primeiro)
    entries.sort_by_key(|a| {
        a.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    // Remover o excesso (mais antigos)
    let to_remove = entries.len() - CACHE_MAX_FILES;
    for entry in entries.into_iter().take(to_remove) {
        if let Err(err) = fs::remove_file(entry.path()) {
            log::warn!("failed to evict cache file {:?}: {err}", entry.path());
        }
    }
}

pub async fn resolve_screenshot_image(
    api_base_url: &str,
    access_token: &str,
    db_path: &Path,
    screenshot_id: &str,
    tracking_id: &str,
    local_path: &str,
    synced_at: Option<&str>,
) -> AgentResult<TrackingScreenshotImage> {
    if Path::new(local_path).is_file() {
        return Ok(TrackingScreenshotImage {
            source: "local".into(),
            file_path: Some(local_path.to_string()),
            download_url: None,
        });
    }

    let cache_path = cache_file_path(db_path, screenshot_id);
    if cache_path.is_file() {
        return Ok(TrackingScreenshotImage {
            source: "cache".into(),
            file_path: Some(cache_path.to_string_lossy().into_owned()),
            download_url: None,
        });
    }

    if synced_at.is_none() {
        return Err(AgentError::Other(
            "screenshot ainda não sincronizado com a nuvem".into(),
        ));
    }

    let remote_path =
        fetch_remote_path(api_base_url, access_token, tracking_id, screenshot_id).await?;
    let bytes = download_capture(&remote_path).await?;

    evict_cache_if_needed(db_path);
    std::fs::create_dir_all(cache_dir_for_db(db_path))?;
    std::fs::write(&cache_path, bytes)?;

    Ok(TrackingScreenshotImage {
        source: "cache".into(),
        file_path: Some(cache_path.to_string_lossy().into_owned()),
        download_url: None,
    })
}

async fn fetch_remote_path(
    api_base_url: &str,
    access_token: &str,
    tracking_id: &str,
    screenshot_id: &str,
) -> AgentResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;

    let url = format!(
        "{}/api/v1/trackings/{tracking_id}/screenshots/{screenshot_id}",
        api_base_url.trim_end_matches('/')
    );

    let response = client.get(url).bearer_auth(access_token).send().await?;
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AgentError::Other(format!(
            "failed to load tracking screenshot metadata: {}",
            body
        )));
    }

    let payload: Value = response.json().await?;
    payload
        .get("path")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| AgentError::Other("tracking screenshot path missing".into()))
}
