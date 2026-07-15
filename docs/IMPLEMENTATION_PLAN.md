# Plano de Implementação — voowork-desktop

**Agente desktop** do [Voowork](https://voowork.com) — timer, captura de atividade e sync offline-first com API Rails.

**Stack:** Tauri 2.x (Rust) + React 19 + TypeScript + Vite + SQLite (WAL) + shadcn/ui + Tailwind CSS 4.

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

## Restrições de escopo

| Permitido | Proibido |
|-----------|----------|
| Tabelas/colunas **somente locais** no SQLite | Novas tabelas no PostgreSQL do backend |
| Usar entidades existentes de forma criativa | Novos endpoints ou campos na API |
| UI mínima (timer, inatividade, settings) | Dashboard de gestão no desktop |
| Docs e testes no repositório desktop | Mudanças em `voowork-backend/docs/db.mermaid` |

## Domain schema reference

Entidades de domínio espelhadas do schema PostgreSQL em `voowork-backend/docs/db.mermaid`:

**Entidades com sync (mirrored):**
- `projects` / `tasks` — cache local via `GET /auth/me` e `GET /projects/:id/tasks`
- `trackings` — POST/PATCH via `sync_queue`
- `tracking_screenshots` — POST metadados após upload S3
- `tracking_peripheral_events` — POST agregados
- `tracking_apps` / `tracking_sites` — POST ao fechar intervalo

**Entidades local-only (SQLite desktop):**
- `device_metadata` — identidade do dispositivo (chave Ed25519)
- `settings` — chave/valor para sessão, preferências
- `sync_queue` — outbox offline-first com retry
- `tracking_inactivity_periods` — períodos de inatividade (sem sync)
- `task_time_totals` — tempo acumulado por task (cache local)

Schema completo em [`docs/db.mermaid`](db.mermaid).

---

## Fases do projeto

### Fase 1: Core Infrastructure ✅ (concluído)

| Item | Status | Entrega |
|------|--------|---------|
| 1.1 | ✅ | Scaffold Vite + React + TypeScript + Tauri 2.x |
| 1.2 | ✅ | Configuração `tauri.conf.json` (window, permissions, icons) |
| 1.3 | ✅ | SQLite schema inicial (`rusqlite`, WAL mode) |
| 1.4 | ✅ | Estrutura de módulos Rust (`src-tauri/src/`) |
| 1.5 | ✅ | Sistema de erros (`AgentError` com variantes) |
| 1.6 | ✅ | App state gerenciado (`AppState` em `app_state.rs`) |
| 1.7 | ✅ | Credential store (keyring para token JWT) |
| 1.8 | ✅ | Variáveis de ambiente (`VOOWORK_API_URL`, `VITE_VOOWORK_WEB_URL`) |
| 1.9 | ✅ | `.env.example` e documentação de setup |
| 1.10 | ✅ | `package.json` com scripts (`tauri dev`, `typecheck`, `build`) |

**Módulos entregues:** `db/schema.rs`, `db/mod.rs`, `error.rs`, `app_state.rs`, `env.rs`, `lib.rs`, `main.rs`, `crypto/mod.rs`

---

### Fase 2: Tracking Engine ✅ (concluído)

| Item | Status | Entrega |
|------|--------|---------|
| 2.1 | ✅ | Hooks globais de mouse/teclado via `rdev` (contagem agregada) |
| 2.2 | ✅ | Anti-automação (`activity_confidence` heurístico em memória) |
| 2.3 | ✅ | Captura de janela ativa (`tracking_focus/` → `tracking_apps` / `tracking_sites`) |
| 2.4 | ✅ | Polling de URL em browsers para `tracking_sites` |
| 2.5 | ✅ | Detecção de inatividade do usuário (`tracking_inactivity/`) |
| 2.6 | ✅ | Máquina de estados idle: warning → countdown → pause |
| 2.7 | ✅ | Classificação ao retornar do idle (meeting_call, offline_work, discard) |
| 2.8 | ✅ | Screenshot capture via `xcap` |
| 2.9 | ✅ | Upload direto para S3/Garage (screenshot async) |
| 2.10 | ✅ | Worker de orquestração (`tracking/` — buffer, capture, enqueue) |
| 2.11 | ✅ | Task time accumulator (`task_time_totals`) |

**Módulos entregues:** `activity/`, `tracking_focus/`, `tracking_inactivity/`, `tracking/`, `screenshot/`, `db/task_time.rs`, `db/tracking_screenshots.rs`, `db/tracking_peripheral_events.rs`

**Documentos:** `docs/features/03-session-tracking.md`, `docs/features/04-activity-monitoring.md`, `docs/features/05-screenshots.md`

---

### Fase 3: Sync Layer ✅ (concluído)

| Item | Status | Entrega |
|------|--------|---------|
| 3.1 | ✅ | Login JWT (`POST /api/v1/auth/login`) com `auth/client.rs` |
| 3.2 | ✅ | Token store (keyring + SQLite fallback) em `auth/token_store.rs` |
| 3.3 | ✅ | Validação de sessão no boot (`GET /api/v1/auth/me`) |
| 3.4 | ✅ | Cache de projetos/tarefas (`projects/`) com TTL |
| 3.5 | ✅ | Outbox pattern (`sync_queue` em SQLite) |
| 3.6 | ✅ | SyncWorker com batch de 10, retry exponencial |
| 3.7 | ✅ | Evento `auth-session-expired` emitido pelo worker |
| 3.8 | ✅ | POST de trackings, screenshots, peripheral_events, apps, sites |
| 3.9 | ✅ | PATCH de tracking ao finalizar (status: inactive) |
| 3.10 | ✅ | Finalização remota em crash/shutdown (`sync/finalize.rs`) |
| 3.11 | ✅ | Mensagens de erro sem prefixo técnico na UI (`http_errors.rs`) |

**Módulos entregues:** `auth/`, `sync/`, `projects/`, `db/sync_queue.rs`, `db/projects.rs`

**Documentos:** `docs/features/01-authentication.md`, `docs/features/02-projects-and-tasks.md`, `docs/features/06-sync-and-offline.md`, `docs/BACKEND_INTEGRATION.md`

---

### Fase 4: Desktop UI ✅ (concluído)

| Item | Status | Entrega |
|------|--------|---------|
| 4.1 | ✅ | Tela de login (`compact-login.tsx`) |
| 4.2 | ✅ | Timer principal (`timer-app.tsx`) com seletor de projeto/tarefa |
| 4.3 | ✅ | Estado do timer (running, paused, stopped) |
| 4.4 | ✅ | Overlay de inatividade (`tracking-inactivity-overlay.tsx`) |
| 4.5 | ✅ | Alerta de buffer (`buffer-alert.tsx` — "Você ainda está trabalhando?") |
| 4.6 | ✅ | Mini-timer widget (`mini-timer-widget.tsx`) |
| 4.7 | ✅ | i18n (pt-BR, en, es) |
| 4.8 | ✅ | Seletor de idioma |
| 4.9 | ✅ | External link guard para abrir painel web |
| 4.10 | ✅ | Hooks React (`use-tracking-session`, `use-auth`, `use-display-elapsed`, `use-mini-timer`) |
| 4.11 | ✅ | Estilo global com Tailwind CSS 4 + shadcn/ui |

**Módulos entregues:** `src/App.tsx`, `src/components/`, `src/hooks/`, `src/i18n/`, `src/lib/tauri.ts`, `src/lib/navigation.ts`

**Documentos:** `docs/features/README.md`

---

### Fase 5: Tray & System Integration ⬜ (em andamento)

| Item | Status | Entrega |
|------|--------|---------|
| 5.1 | ✅ | System tray com ícone e menu |
| 5.2 | ✅ | Tray menu: mostrar/esconder janela, status do timer |
| 5.3 | ✅ | Atualização dinâmica do tray (timer running/paused) |
| 5.4 | ⬜ | Minimizar para tray ao fechar (Fechar → tray, não quit) |
| 5.5 | ⬜ | Notificações do sistema (buffer alert, idle) |
| 5.6 | ⬜ | Tray tooltip com estado do timer |
| 5.7 | ⬜ | Auto-start no login do SO (opcional) |

**Módulos entregues:** `tray/`, `windows/`, `commands/navigation.rs`

**Documentos:** `docs/features/09-tray-and-system.md`

---

### Fase 6: Polish & Hardening ⬜ (pendente)

| Item | Status | Entrega |
|------|--------|---------|
| 6.1 | ⬜ | Tratamento de edge cases (crash recovery, race conditions) |
| 6.2 | ⬜ | Performance: batch inserts, lazy loading, worker backpressure |
| 6.3 | ⬜ | Testes unitários e de integração (Rust `#[cfg(test)]`) |
| 6.4 | ⬜ | Testes end-to-end (WebDriver/WebdriverIO) |
| 6.5 | ⬜ | Logging estruturado (tracing) |
| 6.6 | ⬜ | Migrations aditivas e rollback |
| 6.7 | ⬜ | Limpeza de dados locais antigos (screenshots órfãos, sync_queue obsoleta) |
| 6.8 | ⬜ | Validação de instância única (evitar múltiplos processos) |
| 6.9 | ⬜ | Permissões Linux (input group) documentadas e validadas |
| 6.10 | ⬜ | Documentação de API completa (docs/features/) |

---

## Gargalos (bottlenecks) atuais

### 1. Modular agent orchestration

`docs/IMPLEMENTATION_PLAN.md` estava ausente (este arquivo). O orquestrador multi-agente do OpenCode depende deste documento para decidir fluxos, fases e gargalos. Sem ele, agents operam sem contexto de roadmap.

**Impacto:** planejamento assíncrono entre agents Rust, React, verification e auditor.

**Resolução:** este documento preenche esse gap. Agentes devem consultá-lo via `opencode.jsonc` → `instructions`.

---

### 2. Schema migration consolidation

A refatoração recente removeu tabelas legadas (`sessions`, `screenshots`, `app_focus_events`, `activity_ticks`, `project_cache`) e consolidou o schema em `db/schema.rs`. A migration correspondente (drop + recreate para entidades novas) está em andamento.

**Detalhe:** `docs/UPGRADE.md` documenta o plano — migração aditiva com `DROP TABLE` seguro para tabelas legadas. O schema final está em `docs/db.mermaid`.

**Risco:** usuários com DB legado podem perder trackings antigos (descartados, não migrados). Ver `docs/UPGRADE.md` para recomendações pré-upgrade.

---

### 3. Parameter naming convention (snake_case vs camelCase)

3 call sites conhecidos com mismatch entre `taskId` (camelCase, frontend) e `task_id` (snake_case, Rust). A Tauri IPC serializa como camelCase por padrão; o Rust espera snake_case nos structs de comando.

**Exemplo:**
```rust
// Rust command espera { task_id: "..." }
#[tauri::command]
async fn start_tracking(app: tauri::AppHandle, task_id: String) -> Result<...>
```

```typescript
// Frontend invoca com taskId (camelCase)
await invoke("start_tracking", { taskId: selectedTaskId });
```

**Resolução:** padronizar todos os comandos Tauri para usar `#[serde(rename_all = "camelCase")]` nos argumentos Rust, ou usar `serde` rename manual. A fix está em andamento.

---

### 4. Linux input permissions

A captura real de atividade (mouse/teclado via `rdev`) exige o usuário no grupo `input`. Sem essa permissão, o tracker opera em modo **simulado** (útil apenas para dev).

```bash
sudo usermod -aG input $USER
# logout/login para efetivar
```

**Impacto:** produção depende de setup correto. O modo simulado não gera dados reais de periférico.

**Resolução:** documentado em `docs/SMOKE_TEST.md` e `tauri-dev-cli.md`. Validar no smoke test pré-release.

---

### 5. Offline-first sync consistency

O outbox pattern é robusto, mas há cenários de borda:
- Sync de screenshots com payload S3 após crash (arquivo local perdido, metadado órfão)
- Conflito de PATCH concorrente (usuário finaliza no web enquanto desktop tenta PATCH)
- Race condition entre `finalize.rs` no boot e worker concorrente

**Mitigação:** `sync_queue` com `status` e `attempts`; item com `synced_at` nulo é reenviado; screenshots sem arquivo local são ignorados. Evento `auth-session-expired` interrompe worker.

---

## Estrutura do projeto (mapeamento)

```
src/                              # React UI
├── App.tsx                       # Root com providers (Auth, i18n)
├── components/
│   ├── timer-app.tsx             # Timer principal
│   ├── timer-app-sections.tsx    # Seções do timer (running, paused)
│   ├── compact-login.tsx         # Formulário de login
│   ├── tracking-inactivity-overlay.tsx  # Overlay de idle
│   ├── buffer-alert.tsx          # Alerta de buffer de atividade
│   ├── mini-timer-widget.tsx     # Widget flutuante mini-timer
│   └── external-link-guard.tsx   # Confirmação de link externo
├── hooks/
│   ├── use-auth.tsx              # Hook de autenticação
│   ├── use-tracking-session.ts   # Hook de estado do tracking
│   ├── use-display-elapsed.ts    # Tempo decorrido formatado
│   ├── use-mini-timer.ts         # Estado do mini-timer
│   └── use-mini-widget-drag.ts   # Arrastar mini-widget
├── i18n/
│   ├── locales/
│   │   ├── en.json               # Inglês
│   │   ├── pt-BR.json            # Português Brasil
│   │   └── es.json               # Espanhol
├── lib/
│   ├── tauri.ts                  # Helpers invoke()
│   └── navigation.ts             # Navegação entre estados

src-tauri/src/                    # Core Rust
├── activity/                     # rdev hooks + anti-automação
│   ├── mod.rs
│   ├── tracker.rs
│   ├── constants.rs
│   └── automation.rs
├── auth/                         # Login JWT, token store
│   ├── mod.rs
│   ├── client.rs                 # HTTP client para API auth
│   ├── token_store.rs            # Keyring + SQLite
│   ├── store.rs                  # Persistência settings
│   ├── commands.rs               # login / logout commands
│   └── http_errors.rs            # Extração de mensagens de erro
├── commands/                     # Tauri commands expostos
│   ├── mod.rs
│   ├── tracking.rs               # start/pause/stop/resume
│   ├── projects.rs               # list_projects, sync_projects
│   ├── settings.rs               # Configurações
│   ├── dashboard.rs              # Resumo do dashboard
│   └── navigation.rs             # Tray navigation
├── crypto/                       # Ed25519 assinatura
│   └── mod.rs
├── db/                           # SQLite schema e queries
│   ├── mod.rs
│   ├── schema.rs                 # Migrations
│   ├── projects.rs               # Queries projects/tasks
│   ├── trackings.rs              # Queries trackings
│   ├── tracking_screenshots.rs   # Queries screenshots
│   ├── tracking_peripheral_events.rs
│   ├── tracking_inactivity_periods.rs
│   ├── task_time.rs              # task_time_totals
│   ├── frontend_settings.rs      # Preferências UI
│   └── dashboard.rs              # Aggregates
├── projects/                     # Cache remoto de projetos
│   ├── mod.rs
│   ├── api.rs                    # HTTP para API
│   └── cache.rs                  # TTL cache
├── screenshot/                   # Captura xcap + S3
│   ├── mod.rs
│   ├── constants.rs
│   ├── process.rs                # Capture + compress
│   ├── storage.rs                # Upload/download S3
│   └── remote.rs                 # Sync remoto
├── sync/                         # Outbox + worker REST
│   ├── mod.rs
│   ├── api.rs                    # Payloads HTTP
│   ├── outbox.rs                 # Enqueue itens
│   ├── worker.rs                 # SyncWorker loop
│   ├── constants.rs              # Limites, timeouts
│   └── finalize.rs               # Crash/shutdown recovery
├── tracking/                     # Orquestração do timer
│   ├── mod.rs                    # Start/stop, capture loop
│   └── capture.rs                # Enqueue apps, sites, events
├── tracking_focus/               # Janela ativa
│   └── mod.rs
├── tracking_inactivity/          # Detecção de idle
│   └── mod.rs
├── tray/                         # System tray
│   └── mod.rs
├── windows/                      # Gerenciamento de janelas
│   └── mod.rs
├── app_state.rs                  # AppState compartilhado
├── error.rs                      # AgentError enum
├── env.rs                        # Leitura de env vars
├── icons.rs                      # Ícones embutidos
├── locale.rs                     # Suporte a locale
├── models.rs                     # Structs de domínio
├── navigation.rs                 # Navegação entre views
└── lib.rs                        # generate_handler! + setup
```

---

## Fluxo de trabalho para agentes OpenCode

### Orquestração

Ver `.opencode/agents/desktop-agent-orchestrator.md` para fluxos:

| Fluxo | Uso |
|-------|-----|
| **Full** | planner → rust? → react? → verification? → auditor |
| **Implementation** | rust\|react (1-2) → verification? → auditor |
| **Minimal** | rust\|react (1) → auditor |
| **Audit-only** | auditor |

### Decisões rápidas

| Sinal | Ação |
|-------|------|
| Bug no worker de sync | rust → verification → auditor |
| Atualizar overlay de idle | react → verification → auditor |
| Novo comando Tauri + UI | planner → rust → react → verification → auditor |
| Implementar item deste plano | planner → layers → verification → auditor |
| Revisar implementação | auditor only |

---

## Referências

| Documento | Conteúdo |
|-----------|----------|
| [README.md](../README.md) | Stack, setup, comandos, visão geral |
| [docs/BACKEND_INTEGRATION.md](BACKEND_INTEGRATION.md) | Matriz desktop ↔ API Rails |
| [docs/db.mermaid](db.mermaid) | Schema SQLite local |
| [docs/features/README.md](features/README.md) | Especificações por feature |
| [docs/SMOKE_TEST.md](SMOKE_TEST.md) | Checklist de smoke test pré-release |
| [docs/UPGRADE.md](UPGRADE.md) | Migração SQLite e breaking changes |
| [AGENTS.md](../AGENTS.md) | Instruções para agentes OpenCode |
| [.opencode/rules/backend-boundary.md](../.opencode/rules/backend-boundary.md) | Regra crítica: sem alterações no backend |
| [.opencode/rules/tauri-dev-cli.md](../.opencode/rules/tauri-dev-cli.md) | Comandos Tauri e CLI |
| [.opencode/rules/sqlite-local-schema.md](../.opencode/rules/sqlite-local-schema.md) | Regras de schema SQLite |
