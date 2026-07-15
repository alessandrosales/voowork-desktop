use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use keyring::Entry;

use crate::error::{AgentError, AgentResult};

const SERVICE: &str = "com.voowork.desktop";
const ACCOUNT_ACCESS_TOKEN: &str = "auth_access_token";
const KEYRING_OP_TIMEOUT: Duration = Duration::from_secs(2);

fn entry() -> AgentResult<Entry> {
    Entry::new(SERVICE, ACCOUNT_ACCESS_TOKEN).map_err(|err| {
        AgentError::Other(format!("failed to open credential store: {err}"))
    })
}

fn recv_keyring_result<T>(
    operation: &str,
    rx: mpsc::Receiver<Result<T, keyring::Error>>,
) -> AgentResult<T> {
    match rx.recv_timeout(KEYRING_OP_TIMEOUT) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(keyring::Error::NoEntry)) => Err(AgentError::Other(format!(
            "credential store entry missing during {operation}"
        ))),
        Ok(Err(err)) => Err(AgentError::Other(format!(
            "credential store {operation} failed: {err}"
        ))),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            log::warn!(
                "credential store {operation} timed out after {}s",
                KEYRING_OP_TIMEOUT.as_secs()
            );
            Err(AgentError::Other(format!(
                "credential store {operation} timed out"
            )))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AgentError::Other(format!(
            "credential store {operation} worker disconnected"
        ))),
    }
}

pub fn store_access_token(token: &str) -> AgentResult<()> {
    let entry = entry()?;
    let token = token.to_string();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(entry.set_password(&token));
    });

    recv_keyring_result("store access token", rx)?;
    Ok(())
}

pub fn read_access_token() -> AgentResult<Option<String>> {
    let entry = entry()?;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(entry.get_password());
    });

    match rx.recv_timeout(KEYRING_OP_TIMEOUT) {
        Ok(Ok(token)) if token.is_empty() => Ok(None),
        Ok(Ok(token)) => Ok(Some(token)),
        Ok(Err(keyring::Error::NoEntry)) => Ok(None),
        Ok(Err(err)) => Err(AgentError::Other(format!(
            "credential store read access token failed: {err}"
        ))),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            log::warn!(
                "credential store read access token timed out after {}s",
                KEYRING_OP_TIMEOUT.as_secs()
            );
            Ok(None)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AgentError::Other(
            "credential store read access token worker disconnected".into(),
        )),
    }
}

pub fn clear_access_token() -> AgentResult<()> {
    let entry = entry()?;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(entry.delete_credential());
    });

    match rx.recv_timeout(KEYRING_OP_TIMEOUT) {
        Ok(Ok(())) | Ok(Err(keyring::Error::NoEntry)) => Ok(()),
        Ok(Err(err)) => Err(AgentError::Other(format!(
            "credential store clear access token failed: {err}"
        ))),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            log::warn!(
                "credential store clear access token timed out after {}s",
                KEYRING_OP_TIMEOUT.as_secs()
            );
            Ok(())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AgentError::Other(
            "credential store clear access token worker disconnected".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::KEYRING_OP_TIMEOUT;

    #[test]
    fn keyring_timeout_is_reasonable() {
        assert!(KEYRING_OP_TIMEOUT.as_secs() >= 1);
        assert!(KEYRING_OP_TIMEOUT.as_secs() <= 5);
    }
}
