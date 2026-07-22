use crate::error::AgentError;
use reqwest::StatusCode;
use serde_json::Value;

pub fn is_auth_failure_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    )
}

pub fn error_message_from_body(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return raw_body_message(body);
    };

    if let Some(message) = value.get("error").and_then(Value::as_str) {
        return message.to_string();
    }

    if let Some(message) = value.get("message").and_then(Value::as_str) {
        return message.to_string();
    }

    let Some(errors) = value.get("errors") else {
        return json_value_message(&value, body);
    };

    if let Some(messages) = errors.as_array() {
        let parsed = messages
            .iter()
            .filter_map(message_from_error_item)
            .collect::<Vec<_>>();

        if !parsed.is_empty() {
            return parsed.join(". ");
        }
    }

    if let Some(map) = errors.as_object() {
        let parsed = map
            .iter()
            .flat_map(|(field, entry)| messages_from_field(field, entry))
            .collect::<Vec<_>>();

        if !parsed.is_empty() {
            return parsed.join(". ");
        }
    }

    if let Some(message) = errors.as_str() {
        return message.to_string();
    }

    json_value_message(&value, body)
}

pub fn auth_error_from_response(status: StatusCode, body: &str) -> AgentError {
    let message = error_message_from_body(body);
    if message.is_empty() {
        AgentError::Auth(format!("requisição falhou ({status})"))
    } else {
        AgentError::Auth(message)
    }
}

fn message_from_error_item(entry: &Value) -> Option<String> {
    entry
        .as_str()
        .map(str::to_string)
        .or_else(|| entry.get("detail").and_then(Value::as_str).map(str::to_string))
        .or_else(|| entry.get("title").and_then(Value::as_str).map(str::to_string))
}

fn messages_from_field(field: &str, entry: &Value) -> Vec<String> {
    if let Some(messages) = entry.as_array() {
        return messages
            .iter()
            .filter_map(Value::as_str)
            .map(|message| format_attribute_message(field, message))
            .collect();
    }

    if let Some(message) = entry.as_str() {
        return vec![format_attribute_message(field, message)];
    }

    Vec::new()
}

fn format_attribute_message(attribute: &str, message: &str) -> String {
    if attribute == "base" {
        message.to_string()
    } else {
        format!("{attribute} {message}")
    }
}

fn json_value_message(value: &Value, body: &str) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| raw_body_message(body))
}

fn raw_body_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "E-mail ou senha inválidos".into()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_from_body_reads_string_array() {
        let body = r#"{"errors":["E-mail ou senha inválidos"]}"#;
        assert_eq!(
            error_message_from_body(body),
            "E-mail ou senha inválidos"
        );
    }

    #[test]
    fn error_message_from_body_reads_attribute_hash() {
        let body = r#"{"errors":{"email":["can't be blank"],"password":["is too short"]}}"#;
        assert_eq!(
            error_message_from_body(body),
            "email can't be blank. password is too short"
        );
    }

    #[test]
    fn error_message_from_body_reads_detail_objects() {
        let body = r#"{"errors":[{"detail":"Credenciais inválidas"}]}"#;
        assert_eq!(error_message_from_body(body), "Credenciais inválidas");
    }

    #[test]
    fn error_message_from_body_reads_error_key() {
        let body = r#"{"error":"Unauthorized"}"#;
        assert_eq!(error_message_from_body(body), "Unauthorized");
    }
}
