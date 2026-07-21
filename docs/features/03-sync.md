# Sync (offline-first)

Outbox queue + worker assíncrono. Dados gravados primeiro no SQLite local, depois sincronizados com a API.

## Outbox (`sync_queue`)

Tabela SQLite que acumula itens pendentes de sync. Cada item tem:

| Campo | Descrição |
|-------|-----------|
| `entity_type` | Tipo do recurso (tracking, screenshot, etc.) |
| `entity_id` | UUID do recurso (gerado pelo desktop) |
| `payload_json` | JSON com os dados a enviar |
| `status` | `pending` → `sending` → `confirmed` / `failed` |
| `attempts` | Número de tentativas |
| `next_retry_at` | Backoff exponencial |

## SyncWorker

Worker assíncrono (Tokio) que processa a fila:

1. A cada 5s (fila vazia) ou 2s (após lote), busca até 10 itens pendentes
2. Marca como `sending`
3. Envia para o endpoint correspondente
4. Se sucesso: marca `confirmed`
5. Se erro de auth: emite `auth-session-expired` e para
6. Se erro transitório: marca `failed` com retry exponencial

## Entidades sincronizadas

| Tipo | Endpoint | Método | Observação |
|------|----------|--------|------------|
| `tracking` | `/api/v1/trackings` | POST / PATCH | POST no start, PATCH no stop |
| `tracking_screenshot` | `/api/v1/trackings/:id/screenshots` | POST | Após upload do WebP para S3 |
| `tracking_peripheral_event` | `/api/v1/trackings/:id/peripheral_events` | POST | Mouse/keyboard counts |
| `tracking_app` | `/api/v1/trackings/:id/apps` | POST | Quando o app é fechado |
| `tracking_site` | `/api/v1/trackings/:id/sites` | POST | Quando o site é fechado |
| `tracking_inactivity_period` | — | — | Local only (sem endpoint) |

## Screenshots (S3 + metadados)

Fluxo em duas etapas:

1. **Upload direto para S3/Garage** (`screenshot/storage.rs`)
   - Lê o WebP local
   - Envia para o bucket configurado via `S3_*`
   - Chave do objeto: `{screenshot_id}.{ext}`
   - Path remoto: `screenshots/{screenshot_id}.{ext}`

2. **Metadados na API**
   - `POST /api/v1/trackings/:tracking_id/screenshots`
   - Payload: `{ id, original_id, captured_at, path }`
   - API retorna o `path` que o desktop armazena em `path` e `remote_path`

3. **Limpeza**: após sync bem-sucedido, o WebP local é apagado

## Cache de projetos

Após login, o desktop busca projetos e tarefas da API:
- `GET /api/v1/auth/me` → projetos atribuídos (member) ou `GET /api/v1/projects` (admin)
- `GET /api/v1/projects/:id/tasks` → tarefas de cada projeto
- Cache com TTL de 15 minutos em `settings`
- Reset automático se o `organization_id` mudar

## Retry e recuperação

- Retry exponencial: `2^n` segundos (2s, 4s, 8s, 16s, 32s, 64s, 128s, 256s) cap 3600s, máx. 8 tentativas (dead-letter após)
- Erros 4xx (exceto 401/403) são terminais — vão para dead-letter imediatamente
- Trackings órfãos (crash) são finalizados no próximo boot

## Código

| Arquivo | Função |
|---------|--------|
| `sync/outbox.rs` | Enfileiramento, mark confirmed/failed |
| `sync/worker.rs` | Worker assíncrono Tokio |
| `sync/api.rs` | Payloads HTTP para cada endpoint |
| `sync/finalize.rs` | Finalização de trackings órfãos |
| `screenshot/storage.rs` | Upload/download S3/Garage |
| `projects/cache.rs` | Cache de projetos com TTL |
