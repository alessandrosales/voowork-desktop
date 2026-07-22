# Sessão de tracking

Orquestração do timer, captura de atividade e screenshots.

## Ciclo de vida

1. Usuário seleciona projeto + tarefa e inicia (`start_tracking`).
2. **Antes de inserir:** `finalize_orphaned_trackings` fecha qualquer `active` órfão no SQLite (memória vazia após crash).
3. Rust gera `tracking_id` (UUID v4), grava no SQLite e enfileira na `sync_queue`.
4. `SyncWorker` envia `POST /api/v1/trackings` com o mesmo UUID.
5. Durante a sessão: apps, sites, peripheral events e screenshots são enfileirados.
6. Ao parar: `PATCH /api/v1/trackings/:id` com `status: inactive` e `ended_at`.

> **Alinhamento com API/webapp:** ver [alignment/tracking-data-alignment.md](../alignment/tracking-data-alignment.md).

## Commands Tauri

| Command | Descrição |
|---------|-----------|
| `start_tracking` | Inicia sessão com projeto + tarefa |
| `pause_tracking` / `resume_tracking` | Pausa manual |
| `stop_tracking` | Finaliza e enfileira PATCH |
| `get_tracking_status` | Estado atual para polling da UI |
| `dismiss_activity_buffer` | Descartar buffer de atividade |
| `confirm_still_working` | Responde ao prompt de inatividade |

## Três ciclos de captura

| Ciclo | Intervalo | O que captura |
|-------|-----------|---------------|
| Atividade | 200ms (thread dedicada) | Mouse + teclado → `ActivityBucket` |
| Foco | 15s (worker) | Janela ativa → `tracking_apps` / `tracking_sites` |
| Screenshot | ~300s (configurável) | Tela + peripheral events |

### Atividade (200ms)

O `ActivityTracker` roda em thread separada e acumula eventos no `ActivityBucket`:
- **Mouse:** `platform::poll_mouse_position()` — se posição mudou >1px, incrementa contagem
- **Teclado:** `platform::seconds_since_last_input()` — se idle < 400ms, incrementa contagem
- **Score:** `mouse_events + keyboard_events` mapeado para 0-100 (threshold: 500 eventos)

### Foco (15s)

- `capture_active_window()` → obtém app + título da janela ativa
- Se app mudou: fecha `tracking_app` anterior, abre novo
- Se site mudou (browser): fecha `tracking_site` anterior, abre novo
- Apps de comunicação (Zoom, Teams, Slack) marcam `meeting_exempt` (suspende inatividade)

### Screenshot (~300s)

A screenshot é o "coração" do sistema — ela dispara o pipeline completo:

1. `drain_bucket()` — esvazia o `ActivityBucket` atômico
2. `compute_activity_score()` — calcula score 0-100 com confiança anti-automação
3. `capture_pixels()` — captura a tela via `xcap` (todos os monitores, stitch)
4. `persist_capture()` — INSERT no SQLite + escreve WebP no disco
5. `flush_tracking_peripheral_events_for_period()` — cria `tracking_peripheral_events`
6. `SyncOutbox::enqueue()` — enfileira screenshot + peripheral events

### Inatividade (1s)

O `TrackingInactivityController` avalia o estado a cada 1s:

```
Active → Warning → Countdown (60s) → PausedInactivity
```

- Durante `PausedInactivity` (inatividade automática): screenshots continuam, mas marcadas como `time_category = 'inactivity'`
- Durante `ManualPaused` / `ManualWorkCheck` (pausa manual): screenshots são **puladas** (compatível com TimeDoctor)
- Durante `PausedInactivity`: screenshots continuam, mas marcadas como `time_category = 'inactivity'`
- Ao detectar input após inatividade: transiciona para `ResumePrompt` pedindo classificação ao usuário
- Se for app de comunicação: `meeting_exempt` suspende a inatividade

## Buffer de atividade

O `ActivityBuffer` acumula o primeiro minuto de atividade antes do timer começar:
- O buffer persiste no SQLite a cada segundo
- Ao iniciar o tracking (`start_tracking`), o buffer é "claimado" — os eventos do buffer viram o primeiro período de atividade
- Se o usuário não iniciar o tracking (apenas logou), o buffer é descartado após 1 minuto
- O buffer sobrevive a restart do app (se houver sessão auth válida)

## Finalização (stop e shutdown)

Ao parar o timer (`stop_tracking`):
1. Screenshot final é capturada com UUID real (fail-loud)
2. Peripheral events do período final são enfileirados com o mesmo UUID
3. Apps/sites abertos são fechados
4. Tracking é finalizado via PATCH na API (`status: inactive`, `ended_at`)

Ao fechar o app (quit/tray):
1. Screenshot final é capturada
2. Worker é parado com join síncrono (timeout configurável)
3. Apps/sites abertos são fechados
4. Tracking é finalizado via PATCH na API
5. Trackings órfãos (crash/kill anterior) são finalizados no próximo boot com `ended_at` estimado
6. `start_tracking` também finaliza órfãos no SQLite antes de abrir nova sessão (evita dois `active` com memória vazia)
7. Memória (`ActiveTracking`) só é limpa após finalize no SQLite/outbox ter sucesso

## Código

| Arquivo | Função |
|---------|--------|
| `tracking/mod.rs` | Start/stop, enfileiramento |
| `tracking/lifecycle.rs` | Abandono, finalização, shutdown |
| `tracking/worker.rs` | Loop principal de 1s |
| `tracking/capture.rs` | Screenshot, apps, sites, peripheral events |
| `activity/tracker.rs` | ActivityTracker (200ms) |
| `tracking_focus/mod.rs` | Captura de janela ativa |
| `tracking_inactivity/` | Máquina de estados de inatividade |
| `screenshot/mod.rs` | Captura e persistência de tela |
