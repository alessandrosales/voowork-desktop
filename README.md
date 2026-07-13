# Voowork

**Agente desktop** do [Voowork](https://voowork.com) — timer leve na máquina do colaborador. Captura tempo, atividade e screenshots em segundo plano e envia para a nuvem. **Gestão, dashboard e relatórios ficam no app web.**

📄 **Documentação do produto:** [docs/PRODUCT.md](docs/PRODUCT.md)  
📋 **Specs por feature:** [docs/features/README.md](docs/features/README.md)  
🔗 **Integração com backend:** [docs/BACKEND_INTEGRATION.md](docs/BACKEND_INTEGRATION.md)

O colaborador vê apenas um **timer compacto** (~480×700 px). Fechar a janela minimiza para a bandeja; a sessão continua ativa. O core Rust assume integridade e anti-fraude — invisível na UI.

## Stack

| Camada | Tecnologia |
|--------|------------|
| Shell desktop | Tauri 2.x (Rust + WebView) |
| UI | React 19 + TypeScript + Vite |
| Componentes | shadcn/ui + Tailwind CSS 4 |
| Banco local | SQLite via `rusqlite` (feature `bundled`) |
| Async / sync | `tokio` + `reqwest` |
| Input global | `rdev` (mouse/teclado — contagem, sem keylogging) |
| Screenshots | `xcap` |
| Assinatura | Ed25519 (`ed25519-dalek`) |

## Pré-requisitos

- [Bun](https://bun.sh) (gerenciador de pacotes do frontend)
- [Rust](https://rustup.rs) (toolchain estável)
- Dependências de sistema para Tauri 2 no Linux:

```bash
# Ubuntu / Debian
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Como rodar

### Desenvolvimento (recomendado)

```bash
# Instalar dependências do frontend
bun install

# Configurar ambiente (API local + painel web)
cp .env.example .env

# Subir app desktop com hot-reload
bun run tauri dev
```

### Apenas o frontend (sem Tauri)

```bash
bun run dev
```

Abre em `http://localhost:1420`, mas os comandos Rust (`start_session`, etc.) não funcionam fora do Tauri.

### Build de produção

```bash
# Defina a API de produção (ou crie .env.production a partir do .env.example)
cp .env.example .env.production
# edite VOOWORK_API_URL e VITE_VOOWORK_WEB_URL para produção

bun run tauri build
```

O binário gerado fica em `src-tauri/target/release/`.

### Scripts úteis

| Comando | Descrição |
|---------|-----------|
| `bun run tauri dev` | App desktop em modo dev |
| `bun run build` | Build do frontend |
| `bun run typecheck` | Verificação de tipos TypeScript |
| `bun run lint` | ESLint |
| `bun run format` | Prettier |

## Variáveis de ambiente

Copie `.env.example` para `.env` (dev) ou `.env.production` (build release).  
Arquivos `.env*` são gitignored; só o example fica no repositório.

| Variável | Dev (`.env`) | Prod (`.env.production`) | Descrição |
|----------|--------------|---------------------------|-----------|
| `VOOWORK_API_URL` | `http://localhost:3000` | `https://api.voowork.com` | Base URL da API (Rust: login, sync, projetos) |
| `VITE_VOOWORK_WEB_URL` | `http://localhost:5173` | `https://app.voowork.com` | Painel web aberto pelo link no timer |
| `VOOWORK_SCREENSHOT_INTERVAL_SECS` | opcional | — | Intervalo de screenshot em dev (mín. 10s) |

O sync worker envia payloads para `{VOOWORK_API_URL}/api/v1/agent/sync` quando `BACKEND_SYNC_ENABLED = true` (hoje **desligado** — ver [docs/BACKEND_INTEGRATION.md](docs/BACKEND_INTEGRATION.md)). Os itens permanecem na fila local até o backend expor os endpoints de agente.

## Estrutura do projeto

```
src/                          # Frontend React
├── App.tsx                   # Shell: login ou timer
├── main.tsx                  # ThemeProvider, DebugPanel (dev), App
├── components/
│   ├── timer-app.tsx         # Tela principal de start/stop + idle UI
│   ├── compact-login.tsx     # Login compacto
│   ├── idle-overlay.tsx      # Aviso/countdown/retomada de inatividade
│   ├── voowork-logo.tsx
│   ├── theme-toggle.tsx
│   ├── debug-panel.tsx       # Painel de debug (somente DEV)
│   └── ui/                   # Componentes shadcn/ui
├── hooks/
│   ├── use-tracking-session.ts  # Sessão, idle e invoke Tauri
│   └── use-auth.ts
└── lib/
    ├── tauri.ts              # trackedInvoke com debug
    └── debug-events.ts

src-tauri/src/                # Backend Rust
├── activity/                 # Tracker mouse/teclado + detecção de automação
├── app_focus/                # Janela ativa + filtros (self, file managers, calls)
├── clock/                    # Detecção de alteração manual do relógio
├── commands/                 # Comandos Tauri expostos ao frontend
├── crypto/                   # Chave Ed25519 por dispositivo
├── db/                       # SQLite + schema/migrations
├── idle/                     # Máquina de estados de inatividade
├── integrity/                # Hash chain (mini-blockchain local)
├── screenshot/               # Captura de tela + SHA-256
├── session/                  # Orquestração de sessões de tracking
└── sync/                     # Outbox pattern + worker com retry/backoff
```

### Dados locais

O banco SQLite e screenshots ficam em:

```
~/.local/share/voowork-agent/
├── voowork-agent.db
└── screenshots/
```

## Funcionalidades implementadas (v1)

### Tracking de sessão

- Start/stop de sessão vinculada a projeto e task
- Timer em tempo real com duração baseada em relógio monotônico (`Instant`)
- Seleção de projeto/task (cache local sincronizado com a API após login)
- Contagem agregada de eventos de mouse e teclado por intervalo (sem captura de conteúdo digitado)

### Captura de atividade

- Tracker via `rdev` em thread separada
- Fallback automático para modo simulado se `rdev` não tiver permissão (comum no Linux)
- Agregação em buckets de 60 segundos
- Detecção de padrões de automação (mouse jigglers / auto-clickers) com `activity_score_confidence` por tick

### Screenshots

- Captura periódica com intervalo semi-aleatório (~5 min + jitter)
- Hash SHA-256 calculado no momento da captura
- Metadados gravados para correlação com ticks de atividade

### Armazenamento e sincronização

- SQLite como fonte de verdade offline
- Padrão outbox: grava local → enfileira sync → confirma no servidor
- Fila `sync_queue` append-only (sem UPDATE destrutivo em registros enviados)
- Worker assíncrono com retry e backoff exponencial (até 1 hora)
- Assinatura Ed25519 em cada payload de sincronização

### Proteções anti-fraude

| Proteção | Status |
|----------|--------|
| Hash chain em `sessions` e `activity_ticks` | ✅ |
| Detecção de alteração manual do relógio do sistema | ✅ |
| Score de confiança para atividade automatizada | ✅ (sinaliza, não bloqueia) |
| SHA-256 em screenshots | ✅ |
| Assinatura do agente por dispositivo (Ed25519) | ✅ |
| Validação da hash chain antes do sync | ✅ (marca sessão como `suspicious`) |
| Criptografia do banco em repouso (SQLCipher) | ⏳ Planejado |
| Login/autenticação com backend Voowork | ✅ |

### Interface (frontend)

- **Login** — tela compacta de e-mail/senha (`CompactLogin`)
- **Timer** — start/stop/pause, seletor de projeto/task, overlay de idle
- **Perfil** — menu do usuário com logout e link para o painel web
- Tema dark/light persistido no SQLite (não em `localStorage`)
- i18n (pt-BR, en, es)
- Hook `useTrackingSession()` desacoplando UI dos comandos Tauri
- Painel de debug (somente `import.meta.env.DEV`)

### Sistema

- Ícone na bandeja do sistema (tray)
- Minimizar para bandeja ao fechar a janela (sessão continua ativa)
- Links externos abertos no navegador do sistema

## Comandos Tauri disponíveis

| Comando | Descrição |
|---------|-----------|
| `start_session` | Inicia sessão de tracking |
| `stop_session` | Para sessão ativa |
| `get_session_status` | Status da sessão (timer, eventos, flags) |
| `get_app_status` | Status geral (sync, dispositivo, tracker) |
| `get_setting` / `set_setting` | Leitura/escrita de configurações locais |
| `list_projects` | Lista projetos/tasks do cache local |
| `login` / `logout` / `get_auth_state` | Autenticação com a API |
| `validate_auth_session` | Valida JWT no boot via `GET /auth/me` |
| `sync_projects` | Força refresh do cache de projetos |

## O que ainda não está implementado

- Sync remoto com o backend (`BACKEND_SYNC_ENABLED = false`; endpoints `/api/v1/agent/*` inexistentes)
- Upload de screenshots para storage na nuvem
- Registro de dispositivo (chave pública Ed25519) no backend
- Refresh token automático
- Criptografia do SQLite em repouso (`bundled-sqlcipher`)
- Blur real em screenshots (placeholder atual)
- Validação server-side da hash chain

## Permissões Linux (tracker de hardware)

Para o `rdev` capturar eventos globais de mouse/teclado (em vez do modo simulado):

```bash
# Adicionar usuário ao grupo input
sudo usermod -aG input $USER
# Reiniciar sessão após o comando
```

Sem essa permissão, o agente funciona em **modo simulado** — útil para desenvolvimento, mas não para produção.

## Licença

Projeto privado — Voowork.
