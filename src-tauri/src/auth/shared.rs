use crate::error::{AgentError, AgentResult};

pub fn resolve_base_url(configured_url: &str) -> AgentResult<&str> {
    let base_url = configured_url.trim();
    if base_url.is_empty() {
        return Err(AgentError::Auth(
            "API não configurada (defina VOOWORK_API_URL)".into(),
        ));
    }

    Ok(base_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_base_url_rejects_empty() {
        let err = resolve_base_url("  ").unwrap_err();
        assert!(matches!(err, AgentError::Auth(_)));
    }
}
