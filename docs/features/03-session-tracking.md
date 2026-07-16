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

## Tempo acumulado por task (`task_time_totals`)

O timer exibido ao **trocar de task** (sem sessão ativa na task selecionada) vem de `get_task_elapsed_seconds` → tabela local `task_time_totals`. A UI (`timer-app.tsx`) chama `refreshTaskElapsed` ao mudar o seletor e usa `taskElapsedSeconds` quando a seleção não coincide com a sessão ativa.

### Causa do bug

Ao pausar a task A (ex.: 00:02:20), trocar para a task B e voltar para A, o timer mostrava **00:00:00** em vez do tempo pausado.

**1. Rust — ordem errada em `finalize_active_tracking` (`lifecycle.rs`)**

Ao finalizar a sessão (ex.: `restart_tracking` → `stop_tracking`), o controller de inatividade era **zerado antes** de calcular o elapsed para gravar em `task_time_totals`.

Sem o controller, `snapshot_task_elapsed` / `compute_display_times` caía no fallback baseado em **screenshots** (intervalos esparsos, ~0s em sessões curtas). `set_task_active_seconds` **sobrescrevia** o valor correto (gravado no pause) com ~0. Ao voltar para a task A, `get_task_elapsed_seconds` lia 0 do SQLite.

**2. Rust — race no pause (`mod.rs`)**

A persistência em `pause_tracking` rodava em **thread separada**. Se o usuário trocava de task rápido (`restart_tracking`), o finalize podia executar antes da persistência do pause terminar, perdendo o snapshot.

**3. Frontend — não era a causa raiz**

O React já tratava troca de task: `refreshTaskElapsed` ao mudar `resolvedTaskId` e `displaySeconds = taskElapsedSeconds` quando a seleção ≠ sessão ativa. O sintoma (00:00:00) vinha do SQLite incorreto, não de lógica de exibição ausente.

### Solução

| Camada | Arquivo | Correção |
|--------|---------|----------|
| Rust | `tracking/lifecycle.rs` | Clonar `inactivity_controller` **antes** de limpar; calcular elapsed → persistir em `task_time_totals` → **só então** limpar o controller |
| Rust | `tracking/mod.rs` | `pause_tracking`: persistência **síncrona** via `persist_task_time_snapshot_state` (flush de screenshot continua async) |
| Rust | `tracking/mod.rs` | `restart_tracking`: chamar `persist_task_time_snapshot_state` **antes** de `stop_tracking`, garantindo snapshot mesmo em trocas rápidas |
| React | — | Nenhuma alteração necessária para este bug |

### Fluxo esperado após a correção

1. Pausar task A em ~00:02:20 → `persist_task_time_snapshot_state` grava no SQLite.
2. Trocar para task B e iniciar → `restart_tracking` persiste A de novo, finaliza A, inicia B.
3. Pausar B em ~00:00:47 → persistência síncrona grava B.
4. Voltar para A → `get_task_elapsed_seconds(A)` retorna **00:02:20**.
5. Voltar para B → retorna **00:00:47**.

### Verificação manual

1. Iniciar task A, pausar em ~00:02:20.
2. Trocar para task B, retomar, pausar em ~00:00:47.
3. Voltar para A → deve mostrar **00:02:20**.
4. Voltar para B → deve mostrar **00:00:47**.

Requer rebuild/restart do app Tauri após alterações em `src-tauri/`.
