use std::path::{Path, PathBuf};

macro_rules! restore_env {
    ($name:expr) => {
        if let Some(val) = option_env!($name) {
            let _ = std::env::set_var($name, val);
        }
    };
}

pub fn load() {
    if cfg!(debug_assertions) {
        load_from_files();
    } else {
        load_from_compile_time();
    }
}

fn load_from_files() {
    let root = project_root();

    if let Some(backend_env) = backend_env_path(&root) {
        let _ = dotenvy::from_path(&backend_env);
    }

    let _ = dotenvy::from_path_override(root.join(".env"));

    if let Some(backend_local) = backend_env_local_path(&root) {
        let _ = dotenvy::from_path_override(backend_local);
    }
    let _ = dotenvy::from_path_override(root.join(".env.local"));

    let _ = dotenvy::dotenv();
}

fn load_from_compile_time() {
    restore_env!("API_URL");
    restore_env!("FRONTEND_URL");
    restore_env!("S3_ENDPOINT");
    restore_env!("S3_REGION");
    restore_env!("S3_ACCESS_KEY");
    restore_env!("S3_SECRET_KEY");
    restore_env!("S3_BUCKET");
    restore_env!("VITE_APP_VERSION");

    restore_env!("SCREENSHOT_INTERVAL_SECS");

    let _ = dotenvy::dotenv();
}

fn project_root() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Some(root) = Path::new(&manifest_dir).parent() {
            return root.to_path_buf();
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn backend_root(desktop_root: &Path) -> Option<PathBuf> {
    desktop_root.parent().map(|parent| parent.join("voowork-backend"))
}

fn backend_env_path(desktop_root: &Path) -> Option<PathBuf> {
    backend_root(desktop_root).map(|root| root.join(".env"))
}

fn backend_env_local_path(desktop_root: &Path) -> Option<PathBuf> {
    let path = backend_root(desktop_root)?.join(".env.local");
    path.exists().then_some(path)
}
