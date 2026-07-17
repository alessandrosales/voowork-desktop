# AGENTS.md — voowork-desktop

Agente desktop do Voowork — timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e sincroniza com a API Rails. Gestão e dashboard ficam no app web.

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
| Docs | `docs/` | Produto, schema, features |

### Módulos Rust

| Módulo | Função |
|--------|--------|
| `activity/` | Polling de atividade mouse/teclado |
| `tracking_focus/` | Janela ativa → `tracking_apps` / `tracking_sites` |
| `tracking/` | Orquestração, buffer, worker |
| `tracking_inactivity/` | Detecção de inatividade |
| `screenshot/` | Captura `xcap`, upload S3 |
| `sync/` | Outbox + worker REST |
| `db/` | SQLite schema, queries |
| `auth/` | Login JWT |

---

## Documentação

| Arquivo | Conteúdo |
|---------|----------|
| `docs/README.md` | Visão geral do produto, stack, integração com API |
| `docs/db.mermaid` | Schema SQLite local |
| `docs/features/01-authentication.md` | Login e sessão |
| `docs/features/02-tracking.md` | Timer, atividade, screenshots, foco, inatividade |
| `docs/features/03-sync.md` | Outbox offline-first, S3, retry |

---

## Regras críticas

1. **Backend boundary** — Nunca alterar `voowork-backend/` (schema PostgreSQL, endpoints). Escopo local apenas.
2. **SQLite local** — Novas tabelas/colunas só no SQLite do desktop; documentar em `docs/db.mermaid`.
3. **Tauri dev CLI** — `npm run tauri dev` para desenvolvimento; frontend nunca chama HTTP direto.

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

---

## Variáveis de ambiente

Compartilhadas com `voowork-backend/.env`.

| Variável | Descrição |
|----------|-----------|
| `VITE_API_URL` | Base da API (padrão: `http://localhost:3000`) |
| `FRONTEND_URL` | Painel web (link no timer) |
| `S3_*` | S3/Garage para screenshots |
| `SCREENSHOT_INTERVAL_SECS` | Override de intervalo em dev (mín. 10s) |
