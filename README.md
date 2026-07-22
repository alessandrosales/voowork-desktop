# Voowork Desktop

**Agente desktop** do [Voowork](https://voowork.com) — timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e envia para a nuvem. **Gestão, dashboard e relatórios ficam no app web.**

| Documento | Conteúdo |
|-----------|----------|
| [docs/README.md](docs/README.md) | Visão geral do produto, stack, integração com API |
| [docs/db.mermaid](docs/db.mermaid) | Schema SQLite local |
| [docs/features/README.md](docs/features/README.md) | Features: auth, tracking, sync |

O colaborador vê apenas um **timer compacto** (~480×700 px). Fechar a janela minimiza para a bandeja; a sessão continua ativa.

## Stack

| Camada | Tecnologia |
|--------|------------|
| Shell desktop | Tauri 2.x (Rust + WebView) |
| UI | React 19 + TypeScript + Vite |
| Componentes | shadcn/ui + Tailwind CSS 4 |
| Banco local | SQLite via `rusqlite` (WAL) |
| Async / sync | Tokio + `reqwest` |
| Atividade (mouse/teclado) | Polling nativo por OS (`activity/` — ver `docs/features/02-tracking.md`) |
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
cp ../voowork-backend/.env.example ../voowork-backend/.env
npm run tauri dev
```

API local padrão: `http://localhost:3000` (`VITE_API_URL` no `.env` do backend).

## Variáveis de ambiente

| Variável | Lida por | Descrição |
|----------|----------|-----------|
| `API_URL` | Rust | Base da API Rails (login, sync, projetos) |
| `VITE_WEB_URL` | Vite | Painel web (link no timer) |
| `S3_*` | Rust | Upload direto de screenshots |

**IMPORTANTE:** O valor de `API_URL` é **compilado no binário** em todos os builds
(`npm run tauri dev` e `npm run tauri build`). O `build.rs` lê o `.env` da raiz do
projeto e injeta o valor via `cargo:rustc-env`. Portanto:

1. Edite `API_URL` no `.env` da raiz do projeto
2. Rode `npm run tauri build` (ou `npm run tauri dev`)
3. O app buildado usa a URL que você definiu

Não precisa de comandos especiais — apenas `npm run tauri build`.

Para mudar a URL, edite o `.env` e recompile.

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
├── activity/                 # Polling de atividade + anti-automação
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

Ver **[docs/README.md](docs/README.md)** para documentação completa do produto:

1. **Comportamento:** activity score, pausa manual, buffer persistente, idle local
2. **Captura:** WebP, multi-monitor, título de janela
3. **UX:** settings mínimas, limpeza de código morto

## Permissões Linux

```bash
sudo usermod -aG input $USER
# logout/login
```

Sem permissão → modo **simulado** (dev only).

## Licença

Projeto privado — Voowork.
