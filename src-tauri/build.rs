use std::path::Path;

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-changed=icons");

    // build.rs roda em src-tauri/, o .env está em ../.env
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let project_root = Path::new(&manifest_dir).parent().map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new("..").to_path_buf());
    let env_path = project_root.join(".env");

    if env_path.exists() {
        println!("cargo:warning=build.rs — carregando env: {}", env_path.display());
        let _ = dotenvy::from_path(&env_path);
    } else {
        println!("cargo:warning=build.rs — .env não encontrado em {}, usando fallbacks", env_path.display());
        // Continua sem .env — as variáveis usarão os fallbacks definidos abaixo.
        // Isso permite build sem .env (CI, build limpo, etc.).
    }

    let is_release = std::env::var("PROFILE").as_deref() == Ok("release");

    // Cada variável abaixo é resolvida a partir de um ou mais nomes-fonte
    // (na ordem informada) e injetada sob um único nome canônico. Isso evita
    // o drift histórico entre build.rs, env.rs e os leitores em runtime.
    //
    // Precedência da fonte:
    //   1. Valor do arquivo .env (carregado acima via dotenvy)
    //   2. Variável já definida no ambiente do shell
    let read_first = |names: &[&str]| -> Option<String> {
        names
            .iter()
            .find_map(|name| std::env::var(name).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };

    // API_URL — base de toda comunicação com o backend (lida por auth::store).
    // Documentada como VITE_API_URL; aceita API_URL por compatibilidade.
    match read_first(&["VITE_API_URL", "API_URL"]) {
        Some(url) => inject("API_URL", &url),
        None if is_release => {
            // fail-loud: um release nunca deve sair apontando para localhost.
            panic!(
                "build.rs — VITE_API_URL (ou API_URL) é obrigatória em release. \
                 Defina no {}",
                env_path.display()
            );
        }
        None => {
            println!(
                "cargo:warning=build.rs — VITE_API_URL/API_URL ausente; usando http://localhost:3000 (dev)"
            );
            inject("API_URL", "http://localhost:3000");
        }
    }

    // FRONTEND_URL — painel web (lido por navigation::configured_web_panel_url).
    // Aceita VITE_WEB_URL por compatibilidade com o .env atual.
    if let Some(url) = read_first(&["FRONTEND_URL", "VITE_WEB_URL"]) {
        inject("FRONTEND_URL", &url);
    }

    inject_optional(&read_first, "VITE_APP_VERSION", &["VITE_APP_VERSION"], "0.1.0");

    // Credenciais S3 — fallback vazio (upload é opcional em dev).
    for var in ["S3_ENDPOINT", "S3_REGION", "S3_ACCESS_KEY", "S3_SECRET_KEY", "S3_BUCKET"] {
        inject(var, &read_first(&[var]).unwrap_or_default());
    }

    // SCREENSHOT_INTERVAL_SECS é um override APENAS de desenvolvimento.
    // Em release NÃO é injetada, para que a setting gravada pelo usuário no
    // SQLite tenha efeito (ver tracking::constants::load_screenshot_interval_secs).
    if !is_release {
        if let Some(secs) = read_first(&["SCREENSHOT_INTERVAL_SECS"]) {
            inject("SCREENSHOT_INTERVAL_SECS", &secs);
        }
    }
}

/// Injeta uma variável em tempo de compilação (`option_env!`/`env!`).
fn inject(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

fn inject_optional(
    read_first: &impl Fn(&[&str]) -> Option<String>,
    canonical: &str,
    names: &[&str],
    fallback: &str,
) {
    inject(canonical, &read_first(names).unwrap_or_else(|| fallback.to_string()));
}
