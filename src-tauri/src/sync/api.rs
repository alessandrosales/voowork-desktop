use std::path::Path;

use crate::auth::{error_message_from_body, is_auth_failure_status};
use crate::error::{AgentError, AgentResult};

use super::constants::ENTITY_SCREENSHOT;
use super::outbox::PendingSyncItem;

pub async fn send_sync_item(
    client: &reqwest::Client,
    api_base_url: &str,
    access_token: &str,
    item: &PendingSyncItem,
    screenshot_path: Option<String>,
) -> AgentResult<()> {
    let base_url = api_base_url.trim_end_matches('/');
    post_sync_payload(client, base_url, access_token, item).await?;

    if item.entity_type == ENTITY_SCREENSHOT {
        if let Some(file_path) = screenshot_path {
            upload_screenshot_file(client, base_url, access_token, &item.entity_id, &file_path)
                .await?;
        }
    }

    Ok(())
}

async fn post_sync_payload(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    item: &PendingSyncItem,
) -> AgentResult<()> {
    let url = format!("{base_url}/api/v1/agent/sync");
    let payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;

    let response = client
        .post(&url)
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "entityType": item.entity_type,
            "entityId": item.entity_id,
            "payload": payload,
            "signature": item.signature,
        }))
        .send()
        .await?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if is_auth_failure_status(status) {
        return Err(AgentError::Auth(error_message_from_body(&body)));
    }

    Err(AgentError::Other(format!("sync failed {status}: {body}")))
}

pub async fn upload_screenshot_file(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    screenshot_id: &str,
    file_path: &str,
) -> AgentResult<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AgentError::Other(format!(
            "screenshot file not found: {file_path}"
        )));
    }

    let bytes = std::fs::read(path)?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(format!("{screenshot_id}.png"))
        .mime_str("image/png")
        .map_err(|err| AgentError::Other(err.to_string()))?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let url = format!("{base_url}/api/v1/agent/screenshots/{screenshot_id}/upload");
    let response = client
        .post(url)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if is_auth_failure_status(status) {
        return Err(AgentError::Auth(error_message_from_body(&body)));
    }

    Err(AgentError::Other(format!(
        "screenshot upload failed {status}: {body}"
    )))
}
