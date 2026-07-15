# Voowork Desktop

**Agente desktop** do [Voowork](https://voowork.com) — timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e envia para a nuvem. **Gestão, dashboard e relatórios ficam no app web.**

| Documento | Conteúdo |
|-----------|----------|
| [docs/PRODUCT.md](docs/PRODUCT.md) | Visão de produto e escopo do agente |
| [docs/features/README.md](docs/features/README.md) | Specs por feature |
| [docs/BACKEND_INTEGRATION.md](docs/BACKEND_INTEGRATION.md) | Integração desktop ↔ API |
| [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) | **Plano de gargalos** (fases + checkboxes) |
| [docs/UPGRADE.md](docs/UPGRADE.md) | Migração SQLite e breaking changes |
| [docs/SMOKE_TEST.md](docs/SMOKE_TEST.md) | Checklist manual pré-release |

O colaborador vê apenas um **timer compacto** (~480×700 px). Fechar a janela minimiza para a bandeja; a sessão continua ativa.

## Stack

| Camada | Tecnologia |
|--------|------------|
| Shell desktop | Tauri 2.x (Rust + WebView) |
| UI | React 19 + TypeScript + Vite |
| Componentes | shadcn/ui + Tailwind CSS 4 |
| Banco local | SQLite via `rusqlite` (WAL) |
| Async / sync | Tokio + `reqwest` |
| Input global | `rdev` (contagem, sem keylogging) |
| Screenshots | `xcap` |
| Assinatura | Ed25519 (`ed25519-dalek`) |

## Pré-requisitos

- [Node.js](https://nodejs.org) 20+
- [Rust](https://rustup.rs) (stable)
- Linux (Tauri 2):

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Como rodar

```bash
npm install
cp .env.example .env
npm run tauri dev
```

API local padrão: `http://localhost:3000` (`VOOWORK_API_URL`).

## Variáveis de ambiente

| Variável | Descrição |
|----------|-----------|
| `VOOWORK_API_URL` | Base da API (login, sync, projetos) |
| `VITE_VOOWORK_WEB_URL` | Painel web (link no timer) |
| `VOOWORK_SCREENSHOT_INTERVAL_SECS` | Override de intervalo em dev (mín. 10s) |

## Estrutura do projeto

```
src/                          # Frontend React
├── components/
│   ├── timer-app.tsx         # Timer + idle + buffer alert
│   ├── idle-overlay.tsx      # Máquina de estados idle
│   ├── buffer-alert.tsx      # "Você ainda está trabalhando?"
│   └── compact-login.tsx
├── hooks/
│   ├── use-tracking-session.ts
│   └── use-auth.ts
└── lib/

src-tauri/src/                # Core Rust
├── activity/                 # rdev + anti-automação
├── tracking_focus/           # Janela ativa → tracking_apps / tracking_sites
├── idle/                     # Máquina de estados idle
├── tracking/                 # Orquestração, buffer, worker
├── screenshot/               # Captura xcap
├── sync/                     # Outbox + worker REST
├── db/                       # SQLite (espelha backend + extras locais)
└── auth/                     # Login JWT
```

### Dados locais

```
~/.local/share/voowork-desktop/
├── voowork-desktop.db
└── screenshots/
```

## Funcionalidades (estado atual)

### Tracking

- Start/pause/stop vinculado a projeto + **task obrigatória**
- Timer monotônico + acumulado por task (`task_time_totals`)
- Activity buffer: alerta após ~2 min sem timer (só quando autenticado)
- Recuperação de trackings órfãos após crash

### Captura

- Mouse/teclado agregado (`tracking_peripheral_events`)
- App em foco → `tracking_apps` (poll 15s)
- URLs em browser → `tracking_sites`
- Screenshots ~5 min, upload multipart
- `activity_confidence` anti-automação (memória — não persiste)

### Idle

- Warning → countdown 60s → pausa automática
- Meeting exempt (Zoom, Teams, Meet…)
- Classificação ao retornar (`meeting_call`, `offline_work`, discard)
- `time_category` local em screenshots (`active` / `idle`)

### Sync

- Offline-first com outbox + retry exponencial
- Entidades: `tracking`, `screenshot`, `peripheral_event`, `tracking_app`, `tracking_site`
- `idle_period` ignorado no sync (sem backend)

### UI

- Login, timer, overlay idle, buffer alert, tray, i18n (pt-BR/en/es)

## Comandos Tauri principais

| Comando | Descrição |
|---------|-----------|
| `start_tracking` | Inicia sessão |
| `pause_tracking` / `stop_tracking` | Pausa ou encerra |
| `get_tracking_status` | Estado completo para UI |
| `dismiss_activity_buffer` | Ignora alerta de buffer |
| `confirm_still_working` | Confirma presença no idle |
| `classify_idle_period` / `skip_idle_classification` | Retorno do idle |
| `login` / `logout` / `validate_auth_session` | Auth |
| `list_projects` / `sync_projects` | Cache de projetos |

Lista completa em `src-tauri/src/lib.rs` (`generate_handler`).

## Próximas entregas

Ver **[docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)** — fases 1–3 com checkboxes:

1. **Comportamento:** activity score, pausa manual, buffer persistente, idle local
2. **Captura:** blur, JPEG, multi-monitor, título de janela
3. **UX:** settings mínimas, limpeza de código morto

## Permissões Linux

```bash
sudo usermod -aG input $USER
# logout/login
```

Sem permissão → modo **simulado** (dev only).

## Licença

Projeto privado — Voowork.
