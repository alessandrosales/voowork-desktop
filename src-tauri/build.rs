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

    // Injeta TODAS as variáveis de ambiente em tempo de compilação.
    // Cada variável tem um fallback para garantir que o app funcione mesmo
    // sem .env (release build, CI, build limpo).
    //
    // Ordem de precedência:
    //   1. Valor do arquivo .env (carregado acima via dotenvy)
    //   2. Variável já definida no ambiente do shell
    //   3. Fallback hardcoded abaixo
    let vars: &[(&str, &str)] = &[
        ("API_URL",                 "http://localhost:3000"),
        ("S3_ENDPOINT",             ""),
        ("S3_REGION",               ""),
        ("S3_ACCESS_KEY",           ""),
        ("S3_SECRET_KEY",           ""),
        ("S3_BUCKET",               ""),
        ("VITE_WEB_URL",            "http://localhost:5173"),
        ("VITE_APP_VERSION",        "0.1.0"),
        ("SCREENSHOT_INTERVAL_SECS","300"),
        ("WEB_PANEL_URL",           ""),
    ];

    for (var, fallback) in vars {
        let val = std::env::var(var).unwrap_or_else(|_| fallback.to_string());
        println!("cargo:rustc-env={}={}", var, val);
        if std::env::var(var).is_ok() {
            println!("cargo:warning=build.rs — {} compilada", var);
        } else {
            println!("cargo:warning=build.rs — {} usando fallback", var);
        }
    }
}
