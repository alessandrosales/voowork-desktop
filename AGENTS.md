# AGENTS.md — voowork-desktop

Agente desktop do Voowork — timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e sincroniza com a API Rails. **Gestão e dashboard ficam no app web.**

Stack: Tauri 2.x (Rust) + React 19 + TypeScript + Vite + SQLite local (WAL) + shadcn/ui + Tailwind CSS 4.

## First-reads

| Arquivo | Por quê |
|---------|---------|
| `README.md` | Stack, comandos, estrutura |
| `opencode.jsonc` | Registry de agentes e instructions |
| `docs/IMPLEMENTATION_PLAN.md` | **Plano de gargalos** — fases com checkboxes |
| `docs/BACKEND_INTEGRATION.md` | Mapa desktop ↔ API Rails |
| `docs/db.mermaid` | Schema SQLite local |
| `.opencode/rules/backend-boundary.md` | **Crítico** — não alterar backend |
| `.opencode/agents/desktop-agent-orchestrator.md` | Fluxo de orquestração |

## Regras críticas (`.opencode/rules/`)

1. **Backend boundary** — Nunca alterar `voowork-backend` (schema PostgreSQL, endpoints, `db.mermaid` do backend). Escopo local apenas.
2. **SQLite local** — Novas tabelas/colunas só no SQLite do desktop; documentar em `docs/db.mermaid`.
3. **Tauri dev CLI** — `npm run tauri dev` para desenvolvimento; frontend nunca chama HTTP direto.

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
| UI | `src/` | Timer, overlay de inatividade, buffer alert, login, i18n |
| Core Rust | `src-tauri/src/` | Tracking, sync, captura, inatividade, auth, SQLite |
| Commands | `src-tauri/src/commands/` + `lib.rs` | IPC Tauri exposto à UI |
| Docs | `docs/` | Specs, plano, integração |

### Módulos Rust principais

| Módulo | Função |
|--------|--------|
| `activity/` | Hooks globais mouse/teclado (`rdev`) |
| `tracking_focus/` | Janela ativa OS → `tracking_apps` / `tracking_sites`
| `tracking/` | Orquestração, buffer, worker |
| `tracking_inactivity/` | Detecção de inatividade do usuário durante tracking |
| `screenshot/` | Captura `xcap`, upload |
| `sync/` | Outbox + worker REST |
| `db/` | SQLite schema, queries |
| `auth/` | Login JWT |

## Comandos

| Comando | Propósito |
|---------|-----------|
| `npm install` | Instalar dependências |
| `cp .env.example .env` | Configurar env |
| `npm run tauri dev` | Dev (Vite + Tauri) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm run build` | Build frontend |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Verificar Rust |
| `cargo clippy --manifest-path src-tauri/Cargo.toml` | Lint Rust |

## Variáveis de ambiente

| Variável | Descrição |
|----------|-----------|
| `VOOWORK_API_URL` | Base da API (padrão: `http://localhost:3000`) |
| `VITE_VOOWORK_WEB_URL` | Painel web (link no timer) |
| `VOOWORK_SCREENSHOT_INTERVAL_SECS` | Override de intervalo em dev (mín. 10s) |

## Restrições de escopo (IMPLEMENTATION_PLAN)

| Permitido | Proibido |
|-----------|----------|
| Tabelas/colunas **somente locais** no SQLite | Novas tabelas no PostgreSQL do backend |
| Usar entidades existentes de forma criativa | Novos endpoints ou campos na API |
| UI mínima (timer, inatividade, settings) | Dashboard de gestão no desktop |
| Docs e testes no repositório desktop | Mudanças em `voowork-backend/docs/db.mermaid` |

## OpenCode system

Config em `opencode.jsonc` — 7 agentes (orchestrator + 6 subagentes), 3 rules.

### Orquestração (`.opencode/agents/desktop-agent-orchestrator.md`)

Fluxos:
- **Full**: planner → rust? → react? → verification? → auditor
- **Implementation**: rust|react (1-2) → verification? → auditor
- **Minimal**: um specialist → auditor
- **Audit-only**: auditor only

Nunca pule o auditor. Nunca invoque todos os specialists.

### Instructions carregadas automaticamente

- `AGENTS.md`
- `.opencode/rules/backend-boundary.md`
- `.opencode/rules/tauri-dev-cli.md`
- `.opencode/rules/sqlite-local-schema.md`

### Skills (`.opencode/skills/` + `.cursor/skills/`)

Instaladas via [skills.sh](https://skills.sh). Fonte canônica do CLI: `.agents/skills/` (espelhada para OpenCode e Cursor).

| Skill | Fonte | Uso |
|-------|-------|-----|
| `calling-rust-from-tauri-frontend` | dchuk/claude-code-tauri-skills | `invoke()` / commands Tauri |
| `integrating-tauri-js-frontends` | dchuk/claude-code-tauri-skills | React ↔ Rust |
| `understanding-tauri-ipc` | dchuk/claude-code-tauri-skills | IPC core |
| `configuring-tauri-permissions` | dchuk/claude-code-tauri-skills | Permissões Linux/input |
| `adding-tauri-system-tray` | dchuk/claude-code-tauri-skills | System tray |
| `listening-to-tauri-events` | dchuk/claude-code-tauri-skills | Eventos Rust → UI |
| `testing-tauri-apps` | dchuk/claude-code-tauri-skills | Testes Tauri |
| `debugging-tauri-apps` | dchuk/claude-code-tauri-skills | Debug |
| `packaging-tauri-for-linux` | dchuk/claude-code-tauri-skills | Build Linux |
| `shadcn` | shadcn/ui | Componentes shadcn/ui |
| `tailwind-v4-shadcn` | secondsky/claude-skills | Tailwind v4 + shadcn |
| `vercel-react-best-practices` | vercel-labs/agent-skills | Performance React |
| `rust-async-patterns` | wshobson/agents | Tokio, workers, async |

**Atualizar skills:**

```bash
npx skills update -y
rsync -a --delete .agents/skills/ .opencode/skills/
rsync -a --delete .agents/skills/ .cursor/skills/
```

**Adicionar nova skill:**

```bash
npx skills add <owner/repo> --skill <nome> --agent cursor opencode --copy -y
rsync -a --delete .agents/skills/ .opencode/skills/
rsync -a --delete .agents/skills/ .cursor/skills/
```
