# Tauri Dev CLI

Use os comandos oficiais do projeto para desenvolvimento e verificação. Não invente entrypoints alternativos.

## Desenvolvimento

```bash
npm install
cp ../voowork-backend/.env.example ../voowork-backend/.env   # se ainda não existir
npm run tauri dev
```

Isso sobe Vite (frontend) + Tauri (Rust) com hot reload.

## Build

```bash
npm run build          # tsc -b && vite build (frontend)
npm run tauri build    # bundle completo do app
```

## Verificação rápida

```bash
npm run typecheck      # TypeScript — obrigatório após mudanças em src/
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -D warnings  # quando relevante
```

## Descoberta de commands Tauri

Lista completa em `src-tauri/src/lib.rs` (`generate_handler`).

Principais:
- `start_tracking`, `pause_tracking`, `stop_tracking`
- `get_tracking_status`, `dismiss_activity_buffer`, `confirm_still_working`
- `login`, `logout`, `validate_auth_session`
- `list_projects`, `sync_projects`

## Variáveis de ambiente

| Variável | Uso |
|----------|-----|
| `VITE_API_URL` | API Rails (Rust lê em runtime) |
| `FRONTEND_URL` | Link para painel web |
| `S3_*` | Upload direto de screenshots |
| `SCREENSHOT_INTERVAL_SECS` | Override dev do intervalo de screenshot |

## Pré-requisitos Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Permissão de input (captura real):
```bash
sudo usermod -aG input $USER
# logout/login
```

Sem permissão → modo **simulado** (apenas dev).

## Dados locais

```
~/.local/share/voowork-desktop/
├── voowork-desktop.db
└── screenshots/
```

Nunca commitar DB ou screenshots.

## Regra de integração HTTP

O **frontend React nunca chama HTTP** — toda comunicação com a API passa pelo Rust via `invoke()`.
