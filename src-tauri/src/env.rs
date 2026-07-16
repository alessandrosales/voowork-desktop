use std::path::{Path, PathBuf};

/// Carrega variáveis de ambiente compartilhadas com o voowork-backend.
pub fn load() {
    let root = project_root();

    if let Some(backend_env) = backend_env_path(&root) {
        let _ = dotenvy::from_path(&backend_env);
    }

    // Overrides opcionais do desktop (após o .env do backend).
    let _ = dotenvy::from_path_override(root.join(".env"));

    if cfg!(debug_assertions) {
        if let Some(backend_local) = backend_env_local_path(&root) {
            let _ = dotenvy::from_path_override(backend_local);
        }
        let _ = dotenvy::from_path_override(root.join(".env.local"));
    } else if let Some(production_env) = backend_env_production_path(&root) {
        let _ = dotenvy::from_path_override(production_env);
    }

    // Fallback quando o processo é iniciado a partir de outro cwd.
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

fn backend_env_production_path(desktop_root: &Path) -> Option<PathBuf> {
    let path = backend_root(desktop_root)?.join(".env.production");
    path.exists().then_some(path)
}
