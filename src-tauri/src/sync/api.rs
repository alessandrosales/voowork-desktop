use crate::auth::http_errors::{error_message_from_body, is_auth_failure_status};
use crate::error::{AgentError, AgentResult};
use crate::screenshot::upload_capture;
use reqwest::StatusCode;
use serde_json::{json, Value};

use super::constants::{
    ENTITY_TRACKING_INACTIVITY_PERIOD, ENTITY_TRACKING, ENTITY_TRACKING_APP, ENTITY_TRACKING_PERIPHERAL_EVENT,
    ENTITY_TRACKING_SCREENSHOT, ENTITY_TRACKING_SITE,
};
use super::outbox::PendingSyncItem;

pub async fn send_sync_item(
    client: &reqwest::Client,
    api_base_url: &str,
    access_token: &str,
    item: &PendingSyncItem,
    screenshot_path: Option<String>,
) -> AgentResult<Option<String>> {
    let base_url = api_base_url.trim_end_matches('/');
    let payload: Value = serde_json::from_str(&item.payload_json)?;

    match item.entity_type.as_str() {
        ENTITY_TRACKING => {
            send_tracking(client, base_url, access_token, &item.entity_id, &payload).await?;
            Ok(None)
        }
        ENTITY_TRACKING_SCREENSHOT | "screenshot" => {
            send_tracking_screenshot(
                client,
                base_url,
                access_token,
                &item.entity_id,
                &payload,
                screenshot_path,
            )
            .await
        }
        ENTITY_TRACKING_PERIPHERAL_EVENT | "peripheral_event" => {
            send_tracking_peripheral_event(client, base_url, access_token, &item.entity_id, &payload).await?;
            Ok(None)
        }
        ENTITY_TRACKING_APP => {
            send_tracking_app(client, base_url, access_token, &item.entity_id, &payload).await?;
            Ok(None)
        }
        ENTITY_TRACKING_SITE => {
            send_tracking_site(client, base_url, access_token, &item.entity_id, &payload).await?;
            Ok(None)
        }
        ENTITY_TRACKING_INACTIVITY_PERIOD | "idle_period" => {
            log::debug!("skipping tracking_inactivity_period sync (no backend endpoint): {}", item.entity_id);
            Ok(None)
        }
        other => Err(AgentError::Other(format!("unknown sync entity type: {other}"))),
    }
}

async fn send_tracking(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    entity_id: &str,
    payload: &Value,
) -> AgentResult<()> {
    let tracking_id = str_field(payload, "trackingId").unwrap_or(entity_id);

    if payload.get("endedAt").is_some() {
        let body = json!({
            "tracking": {
                "status": str_field(payload, "status").unwrap_or("inactive"),
                "ended_at": str_field(payload, "endedAt"),
            }
        });
        return patch_json(
            client,
            &format!("{base_url}/api/v1/trackings/{tracking_id}"),
            access_token,
            body,
        )
        .await;
    }

    let body = json!({
        "tracking": {
            "id": tracking_id,
            "project_id": str_field(payload, "projectId"),
            "task_id": str_field(payload, "taskId"),
            "user_id": str_field(payload, "userId"),
            "status": str_field(payload, "status").unwrap_or("active"),
            "device": str_field(payload, "device"),
            "started_at": str_field(payload, "startedAt"),
        }
    });
    post_json(
        client,
        &format!("{base_url}/api/v1/trackings"),
        access_token,
        body,
    )
    .await
}

async fn send_tracking_screenshot(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    entity_id: &str,
    payload: &Value,
    screenshot_path: Option<String>,
) -> AgentResult<Option<String>> {
    let tracking_id = str_field(payload, "trackingId")
        .ok_or_else(|| AgentError::Other("screenshot payload missing trackingId".into()))?;
    let screenshot_id = str_field(payload, "screenshotId").unwrap_or(entity_id);
    let original_id = str_field(payload, "originalId").unwrap_or(screenshot_id);
    let captured_at = str_field(payload, "capturedAt").unwrap_or("");

    let local_path = screenshot_path
        .ok_or_else(|| AgentError::Other("screenshot file path missing".into()))?;

    let storage_path = upload_capture(&local_path, screenshot_id).await?;

    let is_duplicate = payload.get("isDuplicate").and_then(|v| v.as_bool()).unwrap_or(false);
    let activity_level = str_field(payload, "activityLevel").unwrap_or("medium");

    let body = json!({
        "tracking_screenshot": {
            "id": screenshot_id,
            "original_id": original_id,
            "captured_at": captured_at,
            "path": storage_path,
            "is_duplicate": is_duplicate,
            "activity_level": activity_level,
        }
    });

    let response = client
        .post(format!(
            "{base_url}/api/v1/trackings/{tracking_id}/screenshots"
        ))
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    parse_screenshot_response(response).await
}

async fn send_tracking_peripheral_event(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    entity_id: &str,
    payload: &Value,
) -> AgentResult<()> {
    let tracking_id = str_field(payload, "trackingId")
        .ok_or_else(|| AgentError::Other("peripheral_event payload missing trackingId".into()))?;
    let event_id = str_field(payload, "eventId").unwrap_or(entity_id);

    let body = json!({
        "tracking_peripheral_event": {
            "id": event_id,
            "event": str_field(payload, "event"),
            "count": payload.get("count").cloned().unwrap_or(json!(0)),
            "screenshot_original_id": str_field(payload, "screenshotOriginalId"),
            "started_at": str_field(payload, "startedAt"),
            "ended_at": str_field(payload, "endedAt"),
        }
    });

    let response = client
        .post(format!(
            "{base_url}/api/v1/trackings/{tracking_id}/peripheral_events"
        ))
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    parse_peripheral_event_response(response).await
}

async fn send_tracking_app(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    entity_id: &str,
    payload: &Value,
) -> AgentResult<()> {
    let tracking_id = str_field(payload, "trackingId")
        .ok_or_else(|| AgentError::Other("tracking_app payload missing trackingId".into()))?;
    let app_id = str_field(payload, "appId").unwrap_or(entity_id);

    let body = json!({
        "tracking_app": {
            "id": app_id,
            "name": str_field(payload, "name"),
            "started_at": str_field(payload, "startedAt"),
            "ended_at": str_field(payload, "endedAt"),
        }
    });

    post_json(
        client,
        &format!("{base_url}/api/v1/trackings/{tracking_id}/apps"),
        access_token,
        body,
    )
    .await
}

async fn send_tracking_site(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    entity_id: &str,
    payload: &Value,
) -> AgentResult<()> {
    let tracking_id = str_field(payload, "trackingId")
        .ok_or_else(|| AgentError::Other("tracking_site payload missing trackingId".into()))?;
    let site_id = str_field(payload, "siteId").unwrap_or(entity_id);

    let body = json!({
        "tracking_site": {
            "id": site_id,
            "address": str_field(payload, "address"),
            "started_at": str_field(payload, "startedAt"),
            "ended_at": str_field(payload, "endedAt"),
        }
    });

    post_json(
        client,
        &format!("{base_url}/api/v1/trackings/{tracking_id}/sites"),
        access_token,
        body,
    )
    .await
}

async fn post_json(
    client: &reqwest::Client,
    url: &str,
    access_token: &str,
    body: Value,
) -> AgentResult<()> {
    let response = client
        .post(url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    parse_response(response).await
}

async fn patch_json(
    client: &reqwest::Client,
    url: &str,
    access_token: &str,
    body: Value,
) -> AgentResult<()> {
    let response = client
        .patch(url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    parse_response(response).await
}

async fn parse_screenshot_response(response: reqwest::Response) -> AgentResult<Option<String>> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status.is_success() {
        let remote_path = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|json| json.get("path").and_then(|value| value.as_str()).map(str::to_string));
        return Ok(remote_path);
    }

    if is_auth_failure_status(status) {
        return Err(AgentError::Auth(error_message_from_body(&body)));
    }

    if status == StatusCode::UNPROCESSABLE_ENTITY && is_duplicate_screenshot_error(&body) {
        log::info!("screenshot already exists on server, treating sync as confirmed");
        return Ok(None);
    }

    if status.is_client_error() {

        return Err(AgentError::SyncTerminal(format!(
            "screenshot rejected {status}: {body}"
        )));
    }

    Err(AgentError::Other(format!("screenshot sync failed {status}: {body}")))
}

fn is_duplicate_screenshot_error(body: &str) -> bool {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return body.contains("original_id") && body.contains("taken")
            || body.contains("original_id") && body.contains("em uso");
    };

    json.get("errors")
        .and_then(|errors| errors.get("original_id"))
        .and_then(|messages| messages.as_array())
        .is_some_and(|messages| {
            messages.iter().filter_map(|value| value.as_str()).any(|message| {
                message.contains("taken")
                    || message.contains("em uso")
                    || message.contains("already")
            })
        })
}

async fn parse_response(response: reqwest::Response) -> AgentResult<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    let body = response.text().await.unwrap_or_default();
    if is_auth_failure_status(status) {
        return Err(AgentError::Auth(error_message_from_body(&body)));
    }

    if status.is_client_error() {

        return Err(AgentError::SyncTerminal(format!("sync rejected {status}: {body}")));
    }

    Err(AgentError::Other(format!("sync failed {status}: {body}")))
}

async fn parse_peripheral_event_response(response: reqwest::Response) -> AgentResult<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    let body = response.text().await.unwrap_or_default();
    if is_auth_failure_status(status) {
        return Err(AgentError::Auth(error_message_from_body(&body)));
    }

    if status == StatusCode::UNPROCESSABLE_ENTITY && is_missing_screenshot_error(&body) {
        return Err(AgentError::Other(format!(
            "peripheral event aguardando screenshot sincronizar (retentável): {body}"
        )));
    }

    if status.is_client_error() {
        return Err(AgentError::SyncTerminal(format!("sync rejected {status}: {body}")));
    }

    Err(AgentError::Other(format!("sync failed {status}: {body}")))
}

fn is_missing_screenshot_error(body: &str) -> bool {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return body.contains("screenshot_original_id")
            && (body.contains("não encontrada")
                || body.contains("nao encontrada")
                || body.contains("not found"));
    };

    json.get("errors")
        .and_then(|errors| errors.get("screenshot_original_id"))
        .and_then(|messages| messages.as_array())
        .is_some_and(|messages| {
            messages.iter().filter_map(|value| value.as_str()).any(|message| {
                message.contains("não encontrada")
                    || message.contains("nao encontrada")
                    || message.contains("not found")
            })
        })
}

fn str_field<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(|value| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_screenshot_error_matches_rails_pt_br_message() {

        let body = r#"{"errors":{"screenshot_original_id":["captura de tela não encontrada neste rastreamento"]}}"#;
        assert!(is_missing_screenshot_error(body));
    }

    #[test]
    fn missing_screenshot_error_matches_english_message() {
        let body = r#"{"errors":{"screenshot_original_id":["screenshot not found in this tracking"]}}"#;
        assert!(is_missing_screenshot_error(body));
    }

    #[test]
    fn missing_screenshot_error_ignores_other_fields() {
        let body = r#"{"errors":{"tracking_id":["não encontrado"]}}"#;
        assert!(!is_missing_screenshot_error(body));
    }

    #[test]
    fn missing_screenshot_error_ignores_other_validation_on_same_field() {
        let body = r#"{"errors":{"screenshot_original_id":["é obrigatório"]}}"#;
        assert!(!is_missing_screenshot_error(body));
    }

    #[test]
    fn missing_screenshot_error_tolerates_invalid_json() {
        let body = "unprocessable entity: screenshot_original_id não encontrada";
        assert!(is_missing_screenshot_error(body));
    }

    #[test]
    fn missing_screenshot_error_rejects_invalid_json_without_match() {
        assert!(!is_missing_screenshot_error("not json at all"));
        assert!(!is_missing_screenshot_error(""));
        assert!(!is_missing_screenshot_error("resource not found"));
    }

    #[test]
    fn missing_screenshot_error_rejects_unrelated_json() {
        let body = r#"{"errors":{"count":["must be greater than zero"]}}"#;
        assert!(!is_missing_screenshot_error(body));
    }
}
