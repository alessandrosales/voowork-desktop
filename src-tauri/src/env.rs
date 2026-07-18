use std::path::{Path, PathBuf};

/// Carrega variáveis de ambiente compartilhadas com o voowork-backend.
///
/// IMPORTANTE: Em release builds, arquivos `.env` externos NÃO são carregados
/// porque o `.app` bundle é executado de um diretório diferente do projeto.
/// Para release, a API URL deve ser definida em tempo de compilação via:
///   API_URL=http://localhost:3000 npm run tauri build
/// ou em runtime via variável de ambiente do sistema.
pub fn load() {
    let root = project_root();

    // Carrega .env do backend (shared defaults)
    if let Some(backend_env) = backend_env_path(&root) {
        let _ = dotenvy::from_path(&backend_env);
    }

    // Carrega .env do desktop (dev overrides)
    let _ = dotenvy::from_path_override(root.join(".env"));

    if cfg!(debug_assertions) {
        // Dev: também carrega .env.local do backend e do desktop
        if let Some(backend_local) = backend_env_local_path(&root) {
            let _ = dotenvy::from_path_override(backend_local);
        }
        let _ = dotenvy::from_path_override(root.join(".env.local"));
    }
    // NOTA: Em release builds, NÃO tentamos carregar .env.production de arquivo.
    // O app bundle não inclui arquivos .env, e o CARGO_MANIFEST_DIR não existe.
    // A API URL em release é definida em tempo de compilação (option_env!) ou
    // em runtime via variável de ambiente do sistema.

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


