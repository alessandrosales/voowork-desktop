use crate::error::{AgentError, AgentResult};
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use std::path::Path;

pub const ENV_S3_ENDPOINT: &str = "S3_ENDPOINT";
pub const ENV_S3_REGION: &str = "S3_REGION";
pub const ENV_S3_ACCESS_KEY: &str = "S3_ACCESS_KEY";
pub const ENV_S3_SECRET_KEY: &str = "S3_SECRET_KEY";
pub const ENV_S3_BUCKET: &str = "S3_BUCKET";

const DEFAULT_S3_REGION: &str = "garage";

struct S3Config {
    endpoint: String,
    region: String,
    access_key: String,
    secret_key: String,
    bucket: String,
}

fn required_env(name: &str) -> AgentResult<String> {
    std::env::var(name).map_err(|_| {
        AgentError::Other(format!(
            "{name} não definida; configure no .env do voowork-backend"
        ))
    })
}

fn s3_config() -> AgentResult<S3Config> {
    Ok(S3Config {
        endpoint: required_env(ENV_S3_ENDPOINT)?,
        region: std::env::var(ENV_S3_REGION).unwrap_or_else(|_| DEFAULT_S3_REGION.to_string()),
        access_key: required_env(ENV_S3_ACCESS_KEY)?,
        secret_key: required_env(ENV_S3_SECRET_KEY)?,
        bucket: required_env(ENV_S3_BUCKET)?,
    })
}

fn content_type_for_extension(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/webp",
    }
}

fn object_key(screenshot_id: &str, extension: &str) -> String {
    format!("{screenshot_id}.{extension}")
}

fn storage_path(screenshot_id: &str, extension: &str) -> String {
    format!("screenshots/{}", object_key(screenshot_id, extension))
}

fn object_key_from_path(path: &str) -> AgentResult<String> {
    let key = path
        .strip_prefix("screenshots/")
        .ok_or_else(|| AgentError::Other(format!("invalid screenshot path: {path}")))?;
    if key.is_empty() {
        return Err(AgentError::Other(format!("invalid screenshot path: {path}")));
    }
    Ok(key.to_string())
}

fn bucket_for(config: &S3Config) -> AgentResult<Box<Bucket>> {
    let region = Region::Custom {
        region: config.region.clone(),
        endpoint: config.endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.access_key),
        Some(&config.secret_key),
        None,
        None,
        None,
    )
    .map_err(|err| AgentError::Other(format!("invalid S3 credentials: {err}")))?;

    Bucket::new(&config.bucket, region, credentials)
        .map(|bucket| bucket.with_path_style())
        .map_err(|err| AgentError::Other(format!("failed to configure S3 bucket: {err}")))
}

/// Uploads a local capture to S3/Garage and returns the metadata path (`screenshots/{id}.ext`).
pub async fn upload_capture(local_path: &str, screenshot_id: &str) -> AgentResult<String> {
    let path = Path::new(local_path);
    if !path.is_file() {
        return Err(AgentError::Other(format!(
            "screenshot file not found: {local_path}"
        )));
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("webp");
    let content_type = content_type_for_extension(extension);
    let key = object_key(screenshot_id, extension);
    let remote_path = storage_path(screenshot_id, extension);

    let file_bytes = std::fs::read(path).map_err(|err| {
        AgentError::Other(format!("failed to read screenshot {local_path}: {err}"))
    })?;

    let config = s3_config()?;
    let bucket = bucket_for(&config)?;

    bucket
        .put_object_with_content_type(&key, &file_bytes, content_type)
        .await
        .map_err(|err| AgentError::Other(format!("S3 upload failed for {key}: {err}")))?;

    log::info!(
        "uploaded screenshot {screenshot_id} to s3://{}/{}",
        config.bucket,
        key
    );

    Ok(remote_path)
}

/// Downloads a capture from S3/Garage using the metadata path (`screenshots/{id}.ext`).
pub async fn download_capture(path: &str) -> AgentResult<Vec<u8>> {
    let key = object_key_from_path(path)?;
    let config = s3_config()?;
    let bucket = bucket_for(&config)?;

    let response = bucket
        .get_object(&key)
        .await
        .map_err(|err| AgentError::Other(format!("S3 download failed for {key}: {err}")))?;

    Ok(response.bytes().to_vec())
}
