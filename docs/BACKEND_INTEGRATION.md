# Integração desktop ↔ API Rails

Mapa de como o `voowork-desktop` consome a API em `/api/v1`. Toda comunicação HTTP passa pelo Rust (`reqwest`); o frontend React usa apenas `invoke()`.

## Arquitetura

```
React UI ──invoke()──► Rust (Tauri) ──reqwest + JWT──► Rails API
                           │
                           ▼
                      SQLite local
                           │
                           ▼
                      sync_queue (outbox)
```

- **Offline-first:** dados gravados no SQLite; sync assíncrono via `SyncWorker`.
- **Auth:** JWT Bearer em todas as requisições autenticadas.
- **Env:** `VITE_API_URL` no `.env` do `voowork-backend` (padrão dev: `http://localhost:3000`).

## Autenticação

| Endpoint | Método | Uso no desktop |
|----------|--------|----------------|
| `/api/v1/auth/login` | POST | `auth/client.rs` — login com `{ auth: { email, password } }` |
| `/api/v1/auth/me` | GET | `validate_auth_session` — valida token no boot e refresh |

Sessão persistida em SQLite (`settings`: `auth_access_token`, `auth_user_json`, `auth_org_json`).

### Mensagens de erro na UI

Erros de login e auth são exibidos **somente com o texto vindo da API**, sem prefixo técnico.

| Camada | Comportamento |
|--------|---------------|
| API | Retorna corpo JSON (`errors`, `error`, etc.) ou 401 sem corpo |
| `auth/http_errors.rs` | Extrai mensagem legível do body (`error_message_from_body`) |
| `AgentError::Auth` | `#[error("{0}")]` — repassa a mensagem sem prefixo |
| React (`use-auth.tsx`) | Mostra `err.message` diretamente no formulário |

**Exemplo:** credenciais inválidas → UI mostra `E-mail ou senha inválidos`, **não** `auth error: E-mail ou senha inválidos`.

Outros variants de `AgentError` ainda usam prefixo interno (`database error:`, `session error:`, etc.) para logs; apenas `Auth` e `Other` expõem texto puro ao usuário.

## UUID gerado pelo cliente (offline-first)

O desktop gera UUIDs localmente antes de sincronizar. O backend **deve preservar** esses IDs no create — decisão de produto para idempotência e sync de recursos aninhados.

### Tracking

| Campo | Origem | Endpoint |
|-------|--------|----------|
| `id` | UUID gerado no desktop (`Uuid::new_v4`) | `POST /api/v1/trackings` |
| `project_id`, `task_id`, `user_id` | Sessão + seleção do usuário | mesmo |
| `device` | `HOSTNAME` ou `voowork-device` | mesmo |
| `started_at` | ISO 8601 UTC | mesmo |
| `ended_at`, `status: inactive` | Ao parar sessão | `PATCH /api/v1/trackings/:id` |

**Backend:** `tracking_params` permite `:id`. O concern `UuidPrimaryKey` faz `self.id ||= SecureRandom.uuid` — se o cliente envia `id`, ele é mantido.

**Desktop:** `sync/api.rs` envia `"id": tracking_id` no POST. Nenhum remapeamento de ID após resposta.

### Recursos aninhados

Todos usam o `tracking_id` local (igual ao remoto após o fix) e enviam `id` do cliente:

| Entidade | Endpoint | `:id` permitido no backend |
|----------|----------|----------------------------|
| `tracking_app` | `POST .../apps` | Sim |
| `tracking_site` | `POST .../sites` | Sim |
| `tracking_peripheral_event` | `POST .../peripheral_events` | Sim |
| `tracking_screenshot` | `POST .../screenshots` | Sim (metadado; ver gap abaixo) |

### Validação manual

```bash
# UUID local
sqlite3 ~/.local/share/voowork-desktop/voowork-desktop.db \
  "SELECT id FROM trackings ORDER BY started_at DESC LIMIT 1;"

# Mesmo UUID na API
curl -s -H "Authorization: Bearer TOKEN" \
  http://localhost:3000/api/v1/trackings/UUID | jq .id
```

Os dois valores devem coincidir.

## Projetos e tarefas

| Endpoint | Uso |
|----------|-----|
| `GET /api/v1/auth/me` | Lista de projetos atribuídos ao usuário (`user.projects`) |
| `GET /api/v1/projects/:id/tasks` | Cache de tarefas por projeto atribuído |

Sync automático no login e refresh por TTL (`projects/cache.rs`). Command Tauri: `sync_projects`.

O desktop usa **`/auth/me`** como fonte de projetos atribuídos (não `GET /projects`, que retorna todos da conta). Para cada projeto atribuído, busca tarefas via `GET /projects/:id/tasks`.

## Sync (outbox)

Worker: `sync/worker.rs` — lote de 10 itens, retry exponencial, evento `auth-session-expired` se token inválido.

| `entity_type` | Sync | Endpoint |
|---------------|------|----------|
| `tracking` | ✅ | POST / PATCH trackings |
| `tracking_app` | ✅ | POST .../apps |
| `tracking_site` | ✅ | POST .../sites |
| `tracking_peripheral_event` | ✅ | POST .../peripheral_events |
| `tracking_screenshot` | ✅ | POST .../screenshots (JSON metadata após upload S3) |
| `tracking_inactivity_period` | ❌ local | Ignorado pelo worker |

### Screenshots (S3 + JSON)

Fluxo em duas etapas:

1. **Upload direto para S3/Garage** (`screenshot/storage.rs`) — o desktop envia o arquivo local para o bucket configurado via `S3_*`. A chave do objeto é `{screenshot_id}.{ext}`; o path persistido é `screenshots/{screenshot_id}.{ext}`.
2. **Metadados na API** — `POST /api/v1/trackings/:tracking_id/screenshots` com JSON:

```json
{
  "tracking_screenshot": {
    "id": "uuid",
    "original_id": "uuid",
    "captured_at": "ISO8601",
    "path": "screenshots/{id}.jpg"
  }
}
```

O backend persiste apenas metadados e retorna `path`. **Não há multipart** nem upload no Rails — o download remoto no desktop usa o mesmo S3/Garage (`S3_*`) com o `path` retornado pela API.

Variáveis de ambiente (no `.env` do desktop): `S3_ENDPOINT`, `S3_REGION` (padrão `garage`), `S3_ACCESS_KEY`, `S3_SECRET_KEY`, `S3_BUCKET`. Em dev, use o Garage local (`bin/dev-infra` no backend).

### Re-sync

Se o JPEG ainda existir no desktop (`synced_at` nulo e arquivo local presente), o agente reenvia via outbox automaticamente.

## Referência de código

| Área | Arquivo |
|------|---------|
| HTTP sync | `src-tauri/src/sync/api.rs` |
| S3 upload/download | `src-tauri/src/screenshot/storage.rs` |
| Worker | `src-tauri/src/sync/worker.rs` |
| Auth client | `src-tauri/src/auth/client.rs` |
| Erros | `src-tauri/src/error.rs`, `src-tauri/src/auth/http_errors.rs` |
| Projetos | `src-tauri/src/projects/api.rs` |
| Enfileiramento | `src-tauri/src/tracking/mod.rs`, `tracking/capture.rs` |
| Apps/sites (janela ativa) | `src-tauri/src/tracking_focus/mod.rs` |
| Screenshots DB | `src-tauri/src/db/tracking_screenshots.rs` |
| Peripheral events DB | `src-tauri/src/db/tracking_peripheral_events.rs` |

## Backend (alterações mínimas)

No `voowork-backend`, o contrato já era JSON com `path` — as únicas mudanças necessárias:

- Controllers permitem `:id` no create (UUID do cliente, offline-first)
- Screenshots **não** entram mais como nested em `POST /trackings` (endpoint dedicado após upload S3)
- `TrackingScreenshot` só valida e persiste `path` — sem Active Storage, sem service de upload

Arquivos tocados:

- `app/controllers/api/v1/trackings_controller.rb`
- `app/controllers/api/v1/trackings/{apps,sites,peripheral_events,screenshots}_controller.rb`
- `app/models/tracking.rb` (remove nested screenshots)

## Escopo de responsabilidade

O desktop é um **agente de captura** (timer + atividade + screenshots). Nem todo campo/endpoint do domínio backend em `voowork-backend/docs/db.mermaid` é responsabilidade deste app.

### Responsabilidade do desktop (sync write)

| Recurso | Operação | Detalhe |
|---------|----------|---------|
| `trackings` | POST + PATCH stop | UUID cliente; `account_id` inferido pelo JWT no backend |
| `tracking_screenshots` | POST JSON (`path` após upload S3) | Desktop faz upload direto ao bucket |
| `tracking_peripheral_events` | POST | `mouse_activity` e `keyboard_activity` com contagens reais por período |
| `tracking_apps` / `tracking_sites` | POST ao fechar intervalo | Inclui recovery de crash/shutdown |
| `projects` / `tasks` | GET cache | Apenas `id` + `name` (suficiente para o timer) |

### Finalização remota (crash / shutdown)

- **Boot (`initialize_session`):** trackings órfãos recebem PATCH `inactive`, apps/sites abertos são fechados e enfileirados — a outbox **não** é mais purgada.
- **Shutdown (`shutdown_and_reset`):** mesmo comportamento para a sessão ativa + flush do período aberto antes de fechar.

Implementação: `src-tauri/src/sync/finalize.rs`.

### Fora do escopo do desktop (outros apps / backend)

| Item | Motivo |
|------|--------|
| `edition_reason` | Edição manual de tempo — responsabilidade do painel web |
| `GET/DELETE /trackings` | Dashboard e gestão ficam no frontend web |
| `project_members`, `project_customers`, `customers` | Gestão de acesso e clientes |
| Campos extras de `projects`/`tasks` (`featured`, `description`, `position`) | UI de gestão no web |
| `tracking_inactivity_period` | Entidade local-only (sem endpoint backend) |
| Nested create (filhos no POST do tracking) | Otimização opcional; desktop usa POSTs aninhados separados |
