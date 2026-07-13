use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("session error: {0}")]
    Session(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("{0}")]
    Other(String),
}

pub type AgentResult<T> = Result<T, AgentError>;

pub fn guard_native<F, T>(operation: &str, f: F) -> AgentResult<T>
where
    F: FnOnce() -> AgentResult<T>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => {
            log::error!("native capture panicked: {operation}");
            Err(AgentError::Other(format!(
                "native capture failed: {operation}"
            )))
        }
    }
}

impl serde::Serialize for AgentError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
