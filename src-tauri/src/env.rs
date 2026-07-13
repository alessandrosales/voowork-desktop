use std::path::{Path, PathBuf};

/// Carrega variáveis de ambiente de `.env` na raiz do projeto desktop.
pub fn load() {
    let root = project_root();

    let _ = dotenvy::from_path(root.join(".env"));

    if cfg!(debug_assertions) {
        let _ = dotenvy::from_path_override(root.join(".env.local"));
    } else {
        let _ = dotenvy::from_path_override(root.join(".env.production"));
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
