# Integração Desktop ↔ Backend

> Mapa de status da integração entre o agente desktop (`voowork-desktop`) e a API Rails (`voowork-backend`).  
> Última revisão: julho/2026.

---

## Resumo executivo

| Área | Status |
|------|--------|
| Autenticação (login + validação) | ✅ Integrado |
| Projetos e tasks | ✅ Integrado |
| Tracking local (sessão, atividade, idle, screenshots) | ✅ Implementado (somente local) |
| Sync com nuvem | ⏸ Código pronto, **desligado** (`BACKEND_SYNC_ENABLED = false`) |
| Endpoints `/api/v1/agent/*` | ❌ Não existem no backend |

O desktop é **offline-first**: grava tudo no SQLite local e enfileira na `sync_queue`. O worker de sync só enviará dados quando o backend expuser os endpoints de agente e a flag `BACKEND_SYNC_ENABLED` for habilitada.

---

## Arquitetura de comunicação

```
React UI  ──invoke()──►  Rust Core  ──reqwest──►  API Rails (/api/v1)
                              │
                              ▼
                         SQLite local
                    (fonte de verdade offline)
```

- O **frontend nunca chama HTTP** — toda integração passa pelo Rust.
- Base URL: variável `VOOWORK_API_URL` (padrão dev: `http://localhost:3000`).
- Auth: JWT Bearer no header `Authorization`, TTL 24h no backend.

---

## Matriz de integração

### ✅ Integrado e em uso

| Feature desktop | Endpoint | Método | Arquivo Rust |
|-----------------|----------|--------|--------------|
| Login | `/api/v1/auth/login` | POST | `src-tauri/src/auth/api.rs` |
| Validar sessão / perfil | `/api/v1/auth/me` | GET | `src-tauri/src/auth/api.rs` |
| Listar projetos atribuídos | `/api/v1/auth/me` (campo `projects`) | GET | `src-tauri/src/projects/api.rs` |
| Listar tasks por projeto | `/api/v1/projects/{id}/tasks` | GET | `src-tauri/src/projects/api.rs` |

#### Contrato de login

**Request:**
```json
{ "auth": { "email": "user@empresa.com", "password": "..." } }
```

**Response (200):**
```json
{
  "token": "jwt...",
  "user": { "id": "...", "name": "...", "email": "...", "account_id": "...", "projects": [...] },
  "account": { "id": "...", "name": "..." }
}
```

O desktop mapeia `account` → `organization` internamente.

#### Contrato de projetos

Projetos vêm de `GET /auth/me` — apenas projetos **atribuídos ao usuário** via `project_members`, não todos da conta.

Tasks: `GET /api/v1/projects/{project_id}/tasks` — uma chamada por projeto após login.

Cache local: tabela `project_cache`, TTL 15 min (`PROJECT_CACHE_TTL_SECS = 900`).

---

### ⏸ Implementado no desktop, desligado

| Feature | Endpoint planejado | Método | Bloqueio |
|---------|-------------------|--------|----------|
| Sync de entidades | `/api/v1/agent/sync` | POST | Endpoint inexistente + `BACKEND_SYNC_ENABLED = false` |
| Upload de screenshot | `/api/v1/agent/screenshots/{id}/upload` | POST multipart | Endpoint inexistente + flag desligada |
| Registro de dispositivo | `/api/v1/agent/register` | POST | Não implementado em nenhum lado |

**Flag de controle** (`src-tauri/src/sync/constants.rs`):
```rust
pub const BACKEND_SYNC_ENABLED: bool = false;
```

**Payload de sync** (quando habilitado):
```json
{
  "entityType": "session|activity_tick|screenshot|idle_period",
  "entityId": "uuid",
  "payload": { ... },
  "signature": "ed25519..."
}
```

**Entity types enfileirados:**

| `entity_type` | Origem | Enfileirado |
|---------------|--------|-------------|
| `session` | Start/stop de sessão | ✅ |
| `activity_tick` | Buckets de 60s | ✅ |
| `screenshot` | Metadados + SHA-256 | ✅ |
| `idle_period` | Períodos de inatividade | ✅ |
| `app_focus` | Poll de janela ativa | ❌ Somente SQLite local |

---

### ❌ Não integrado

| Item | Desktop | Backend |
|------|---------|---------|
| Refresh token automático | Campo reservado, sempre `None` | Não existe |
| Logout com revogação JWT | Limpa SQLite local | Não existe |
| Validação server-side de hash chain | Preparado no sync | Sem endpoint |
| Registro de chave pública Ed25519 | Chave gerada localmente | Sem endpoint |
| App focus na nuvem | Gravado em `app_focus_events` | Sem modelo/endpoint |
| OAuth | Não implementado | Não usado pelo desktop |

---

## Features locais (sem dependência de API)

Tudo abaixo funciona **sem backend** — dados ficam em `~/.local/share/voowork-desktop/`:

| Feature | Tabela / diretório |
|---------|-------------------|
| Sessões de tracking | `sessions` |
| Activity ticks (60s) | `activity_ticks` |
| Screenshots PNG | `screenshots/` + tabela `screenshots` |
| Períodos idle | `idle_periods` |
| App focus | `app_focus_events` |
| Fila de sync (outbox) | `sync_queue` |
| Chaves Ed25519 | `device_metadata` |
| Preferências (tema, locale) | `settings` |

### Modo demo

Sem autenticação e sem `VOOWORK_API_URL`, o `seed.rs` popula 3 projetos mock e sessões históricas para desenvolvimento offline.

---

## Mapeamento de conceitos

| Backend (Rails) | Desktop (Rust) |
|-----------------|----------------|
| `Account` | `organization` |
| `User` | `user` |
| `ProjectMember` | Projetos no seletor |
| `Task` | Tasks no seletor |

Não há endpoint dedicado de organizações — a conta vem no login (`account`) e no perfil (`account_id`).

---

## Próximos passos (prioridade)

### P0 — Bloqueia sync em produção

1. **Backend:** criar namespace `api/v1/agent` com:
   - `POST /agent/sync` — receber lotes assinados, validar hash chain, idempotência por `entity_id`
   - `POST /agent/screenshots/:id/upload` — multipart PNG para Active Storage / S3
   - `POST /agent/register` — registrar `device_id` + `public_key` Ed25519

2. **Backend:** modelos e migrations para `sessions`, `activity_ticks`, `screenshots`, `idle_periods`, `devices`

3. **Desktop:** habilitar `BACKEND_SYNC_ENABLED = true` após endpoints estáveis

### P1 — Melhorias de auth

- Refresh token ou renovação silenciosa antes de expirar (24h)
- Logout com invalidação (`jwt_version` bump no backend)
- Registrar chave pública no primeiro login

### P2 — Complementos

- Enfileirar `app_focus` no sync (se o backend expuser endpoint)
- Blur real em screenshots (hoje é placeholder)
- SQLCipher para criptografia do banco em repouso

---

## Referências

| Documento | Conteúdo |
|-----------|----------|
| [PRODUCT.md](./PRODUCT.md) | Visão de produto |
| [features/README.md](./features/README.md) | Specs por feature |
| [features/01-authentication.md](./features/01-authentication.md) | Auth detalhada |
| [features/06-sync-and-offline.md](./features/06-sync-and-offline.md) | Outbox e worker |
| [db.mermaid](./db.mermaid) | Modelo ER local |
| Backend: `config/routes.rb` | Rotas Rails existentes |
| Backend: `docs/patterns/04-autenticacao-autorizacao.md` | JWT e multi-tenancy |
