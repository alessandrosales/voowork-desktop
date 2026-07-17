# Voowork Desktop — Documentação do produto

Timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e sincroniza com a API Rails. Gestão e dashboard ficam no app web.

**Stack:** Tauri 2.x (Rust) + React 19 + TypeScript + Vite + SQLite local (WAL) + shadcn/ui + Tailwind CSS 4.

---

## Arquitetura

```
React UI  ──invoke()──►  Rust Core  ──reqwest──►  API Rails (/api/v1)
                              │
                              ▼
                         SQLite local (~/.local/share/voowork-desktop/)
                              │
                              ▼
                         sync_queue (outbox offline-first)
```

### Camadas

| Camada | Diretório | Responsabilidade |
|--------|-----------|------------------|
| UI | `src/` | Timer, login, overlay de inatividade, buffer alert, i18n |
| Core Rust | `src-tauri/src/` | Tracking, sync, captura, inatividade, auth, SQLite |
| Commands | `src-tauri/src/commands/` + `lib.rs` | IPC Tauri exposto à UI |

### Módulos Rust principais

| Módulo | Função |
|--------|--------|
| `activity/` | Polling de atividade mouse/teclado (200ms) |
| `tracking_focus/` | Janela ativa → `tracking_apps` / `tracking_sites` |
| `tracking/` | Orquestração do timer, worker, captura |
| `tracking_inactivity/` | Detecção de inatividade durante tracking |
| `screenshot/` | Captura via `xcap`, upload S3 |
| `sync/` | Outbox + worker REST (offline-first) |
| `db/` | SQLite schema e queries |
| `auth/` | Login JWT |

---

## Features

| Feature | Descrição | Doc |
|---------|-----------|-----|
| Autenticação | Login JWT, sessão local, validação no boot | [features/01-authentication.md](features/01-authentication.md) |
| Tracking | Timer, atividade, screenshots, foco, inatividade | [features/02-tracking.md](features/02-tracking.md) |
| Sync | Outbox offline-first, upload S3, retry | [features/03-sync.md](features/03-sync.md) |

---

## Integração com a API Rails

O desktop consome a API em `/api/v1`. Toda comunicação HTTP passa pelo Rust (`reqwest`); o frontend React usa apenas `invoke()`.

### Endpoints

| Endpoint | Método | Uso |
|----------|--------|-----|
| `/api/v1/auth/login` | POST | Login |
| `/api/v1/auth/me` | GET | Validar sessão + projetos |
| `/api/v1/trackings` | POST / PATCH | Criar / finalizar sessão |
| `/api/v1/trackings/:id/screenshots` | POST | Metadados da screenshot (após upload S3) |
| `/api/v1/trackings/:id/peripheral_events` | POST | Eventos de mouse/teclado |
| `/api/v1/trackings/:id/apps` | POST | Apps da janela ativa |
| `/api/v1/trackings/:id/sites` | POST | Sites navegados |
| `/api/v1/projects` | GET | Todos os projetos (admin) |
| `/api/v1/projects/:id/tasks` | GET | Tarefas de um projeto |

### UUID offline-first

O desktop gera UUIDs localmente **antes** de sincronizar. O backend preserva esses IDs no create, garantindo idempotência e que recursos aninhados usem o mesmo `tracking_id`.

### Screenshots (S3 + metadados)

1. Desktop captura a tela via `xcap` e salva como JPEG local
2. **Upload direto** para S3/Garage (config `S3_*` no `.env`)
3. **Metadados** enviados para API: `POST /api/v1/trackings/:id/screenshots`
4. API retorna o `path` (S3 object key) que o desktop armazena
5. Webapp consome a URL pelo campo `path` da API

### Variáveis de ambiente

Compartilhadas com `voowork-backend/.env`.

| Variável | Descrição |
|----------|-----------|
| `VITE_API_URL` | Base da API (padrão: `http://localhost:3000`) |
| `FRONTEND_URL` | Painel web (link no timer) |
| `S3_ENDPOINT` | Endpoint S3/Garage |
| `S3_REGION` | Região (padrão: `garage`) |
| `S3_ACCESS_KEY` | Access key S3 |
| `S3_SECRET_KEY` | Secret key S3 |
| `S3_BUCKET` | Bucket S3 |
| `SCREENSHOT_INTERVAL_SECS` | Override de intervalo em dev (mín. 10s) |

---

## Schema do banco

Ver [`docs/db.mermaid`](db.mermaid) — SQLite local com WAL, schema espelhado do backend + extensões locais (inatividade, task_time_totals, settings).

---

## Comandos

| Comando | Propósito |
|---------|-----------|
| `npm install` | Instalar dependências |
| `npm run tauri dev` | Dev (Vite + Tauri) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm run build` | Build frontend |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Verificar Rust |
| `cargo clippy --manifest-path src-tauri/Cargo.toml -D warnings` | Lint Rust |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Testes Rust |

### Pré-requisitos Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libxss-dev

# Permissão de captura de input (opcional, para polling real)
sudo usermod -aG input $USER
# logout/login
```

---

## Restrições de escopo

| Permitido | Proibido |
|-----------|----------|
| Tabelas/colunas **somente locais** no SQLite | Novas tabelas no PostgreSQL do backend |
| Usar entidades existentes de forma criativa | Novos endpoints ou campos na API |
| UI mínima (timer, inatividade, settings) | Dashboard de gestão no desktop |
| Dados que ficam só localmente (inatividade) | Mudanças em `voowork-backend/` |

### Arquivos de dados locais

```
~/.local/share/voowork-desktop/
├── voowork-desktop.db       # SQLite WAL
└── screenshots/             # JPEGs locais (limpos após sync S3)
```

Nunca commitar DB ou screenshots.
