# Sessão de tracking

Orquestração de timer, captura de atividade e sync com a API.

## Ciclo de vida

1. Usuário seleciona projeto + tarefa e inicia (`start_tracking`).
2. Rust gera `tracking_id` (UUID v4) e grava em SQLite.
3. Item enfileirado na `sync_queue` (`entity_type: tracking`).
4. `SyncWorker` envia `POST /api/v1/trackings` com o **mesmo UUID**.
5. Durante a sessão: apps, sites, peripheral events e screenshots enfileirados.
6. Ao parar: `PATCH /api/v1/trackings/:id` com `status: inactive` e `ended_at`.

## UUID do cliente

O ID é gerado no desktop **antes** de qualquer chamada à API. O backend preserva esse UUID no create (`:id` permitido em `tracking_params`).

Isso garante que recursos aninhados usem o mesmo `tracking_id` local e remoto:

```
POST /api/v1/trackings/{id}/apps
POST /api/v1/trackings/{id}/sites
POST /api/v1/trackings/{id}/peripheral_events
```

Sem esse contrato, o backend geraria outro UUID e os nested syncs retornariam 404.

## Commands Tauri

| Command | Descrição |
|---------|-----------|
| `start_tracking` | Inicia sessão |
| `pause_tracking` / `resume_tracking` | Pausa manual |
| `stop_tracking` | Finaliza e enfileira PATCH |
| `get_tracking_status` | Estado para polling da UI |

## Código

| Módulo | Responsabilidade |
|--------|------------------|
| `tracking/mod.rs` | Start/stop, enqueue tracking |
| `tracking/capture.rs` | Apps, sites, events, screenshots |
| `sync/api.rs` | Payloads HTTP para a API |

Ver também: [BACKEND_INTEGRATION.md](../BACKEND_INTEGRATION.md).
