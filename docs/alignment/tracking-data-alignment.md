# Alinhamento de dados de tracking — Desktop, API e Webapp

Documento de referência para manter **SQLite local (desktop)**, **PostgreSQL (API Rails)** e **relatórios do webapp** consistentes.

**Última atualização:** 2026-07-22  
**Incidente que motivou:** tracking `7fe60583-…` permaneceu `active` no PG/SQLite enquanto `64d8c3bd-…` foi criado e finalizado, inflando relatórios (~1120s com sobreposição).

---

## 1. Fonte de verdade por camada

| Camada | Papel | Escopo |
|--------|-------|--------|
| **Desktop SQLite** | Offline-first, outbox de sync | Timer, apps/sites, screenshots, peripheral events |
| **API `/api/v1/trackings`** | Persistência central | Mesmos UUIDs do desktop após sync |
| **Webapp `/api/v1/reports/*`** | Agregação para dashboards | Lê apenas PostgreSQL |

Regra: o webapp **nunca** lê SQLite. Qualquer divergência visível no painel é divergência **API ↔ desktop**, não “bug de frontend” isolado.

---

## 2. Contrato de sincronização (desktop → API)

### Tracking

| Evento desktop | `sync_queue` | API |
|----------------|--------------|-----|
| Start | `POST` create (`status: active`) | `POST /api/v1/trackings` |
| Stop | `PATCH` (`status: inactive`, `endedAt`) | `PATCH /api/v1/trackings/:id` |

### Filhos (apps, sites, screenshots, peripheral_events)

- Enfileirados ao **fechar** o segmento (ou no stop do tracking).
- App com `ended_at` vazio **não** sobe — comportamento esperado.

### Invariantes

1. **No máximo 1 tracking `active` por `user_id` por conta** (garantido no backend desde 2026-07-22).
2. Desktop finaliza órfãos no SQLite antes de cada `start_tracking` e no `initialize_session`.
3. Memória (`ActiveTracking`) e SQLite devem convergir no stop; memória **não** é limpa se finalize no DB falhar.

---

## 3. Correções implementadas

### 3.1 Desktop (`voowork-desktop`)

| Arquivo | Mudança |
|---------|---------|
| `src-tauri/src/tracking/mod.rs` | `start_tracking` chama `finalize_orphaned_trackings` antes de inserir |
| `src-tauri/src/tracking/mod.rs` | `capture_final_screenshot_and_finalize` só limpa memória após finalize OK |
| `src-tauri/src/lib.rs` | Log de erro se `initialize_session` falhar |
| `src-tauri/src/db/trackings.rs` | `estimate_tracking_ended_at` considera apps/sites abertos |

### 3.2 Backend (`voowork-backend`)

| Arquivo | Mudança |
|---------|---------|
| `app/models/tracking/close_stale_actives.rb` | Ao criar tracking `active`, fecha outros `active` do mesmo usuário |
| `app/services/reports/tracking_duration.rb` | Deduplica intervalos sobrepostos **por usuário** nos relatórios |
| `app/controllers/api/v1/reports/*` | `project_time`, `task_time`, `user_time`, `timeline`, `counters` usam deduplicação |
| `app/controllers/api/v1/reports/timelines_controller.rb` | Blocos expõem `status`, `is_live`, `user_id` |

### 3.3 Frontend (`voowork-frontend`)

| Arquivo | Mudança |
|---------|---------|
| `app/lib/api/types.ts` | `TimelineBlock` com `status`, `is_live`, `user_id` |
| `app/components/dashboard/activity-timeline.tsx` | Deduplica totais por dia; destaque visual para blocos `live` |

---

## 4. Semântica dos relatórios (webapp)

### Duração de tracking `active`

- `ended_at = null` → relatórios usam `Time.now.utc` como fim **provisório**.
- O tempo **cresce** enquanto o tracking permanecer `active` no PG.
- Timeline expõe `is_live: true` para esses blocos.

### Deduplicação de sobreposição

Dois trackings do **mesmo usuário** com intervalos sobrepostos contam **uma vez** no total agregado.

Dois usuários diferentes em paralelo continuam somando normalmente.

Endpoints afetados:

- `GET /api/v1/reports/project_time`
- `GET /api/v1/reports/task_time`
- `GET /api/v1/reports/user_time`
- `GET /api/v1/reports/timeline` (campo `total_seconds` por dia)
- `GET /api/v1/reports/counters` (`total_hours`)

### O que **não** muda

- `GET /api/v1/trackings` lista cada registro individualmente (sem merge).
- `trackings_count` nos relatórios continua sendo contagem de **registros**, não de horas únicas.

---

## 5. Checklist de verificação manual

### Após sessão de tracking no desktop

```bash
# SQLite local
sqlite3 ~/.local/share/voowork-desktop/voowork-desktop.db \
  "SELECT id, status, started_at, ended_at FROM trackings ORDER BY started_at DESC LIMIT 5;"

sqlite3 ~/.local/share/voowork-desktop/voowork-desktop.db \
  "SELECT status, COUNT(*) FROM sync_queue WHERE entity_type='tracking' GROUP BY status;"
```

### API (login de dev)

```bash
TOKEN=$(curl -s -X POST http://localhost:3000/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"auth":{"email":"dev1@acme.com","password":"12345678"}}' \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["token"])')

curl -s -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/v1/trackings | python3 -m json.tool
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/v1/reports/project_time | python3 -m json.tool
```

### Critérios de aceite

- [ ] Mesmos UUIDs em SQLite e `GET /api/v1/trackings`
- [ ] Nenhum item `pending`/`failed` na `sync_queue` após sync
- [ ] No máximo 1 tracking `active` por usuário no PG
- [ ] `project_time.total_seconds` ≈ soma deduplicada dos blocos da timeline (mesmo filtro de data)
- [ ] Bloco `is_live` no webapp corresponde ao único `active` no PG

---

## 6. Remediação de dados legados (produção/dev)

Para o incidente `7fe60583` (órfão `active`):

### PostgreSQL

```sql
-- Verificar
SELECT id, status, started_at, ended_at FROM trackings
WHERE user_id = '<user-uuid>' AND status = 'active';

-- Fechar órfão (ajustar ended_at para último evento conhecido ou started_at do tracking seguinte)
UPDATE trackings
SET status = 'inactive', ended_at = '2026-07-22T13:27:33.173Z', updated_at = NOW()
WHERE id = '7fe60583-9977-4bc0-9110-bf677095deff' AND ended_at IS NULL;
```

### SQLite local

Reiniciar o desktop com as correções: `initialize_session` + próximo `start` finalizam órfãos automaticamente.

Ou manualmente:

```sql
UPDATE trackings SET status = 'inactive', ended_at = started_at, updated_at = started_at
WHERE id = '7fe60583-9977-4bc0-9110-bf677095deff' AND ended_at IS NULL;
```

Enfileirar stop na outbox se necessário (o sync worker enviará o PATCH).

---

## 7. Trabalho futuro (opcional)

| Item | Repo | Prioridade | Status |
|------|------|------------|--------|
| Expor `active_trackings_count` em `/reports/counters` | backend | Baixa | Pendente |
| Alerta no webapp se `trackings_count > 1` com overlap histórico | frontend | Baixa | Pendente |
| Teste E2E desktop → API → relatório | desktop | Média | Pendente |
| Índice parcial PG: `UNIQUE (account_id, user_id) WHERE status = 'active'` | backend | Média | Pendente (após limpar legado) |

---

## 9. Status da entrega (2026-07-22)

### Concluído no código

| Repo | Entrega |
|------|---------|
| **desktop** | Guard de órfãos em `start_tracking` + boot; finalize seguro; `estimate_tracking_ended_at` tolera tracking sem filhos; teste `start_finalizes_sqlite_orphan_when_memory_is_empty` |
| **backend** | `CloseStaleActives` no create; `Reports::TrackingDuration` nos relatórios; timeline com `status` / `is_live` / `user_id` |
| **frontend** | Tipos e timeline alinhados; deduplicação visual de totais por dia; destaque de blocos `live` |
| **docs** | Este arquivo + links em `README.md`, `02-tracking.md`, `regression-test-checklist.md` |

### Ação manual pendente (dados do incidente)

1. Fechar tracking órfão `7fe60583-…` no PostgreSQL (SQL na §6).
2. Reiniciar desktop ou rodar SQL local na §6 para alinhar SQLite.
3. Confirmar `sync_queue` sem `pending`/`failed` para entity `tracking`.

### Verificação automatizada (rodar localmente)

```bash
# Desktop
cargo test --manifest-path src-tauri/Cargo.toml start_finalizes_sqlite_orphan_when_memory_is_empty
cargo check --manifest-path src-tauri/Cargo.toml

# Backend (DB de teste isolado)
cd ../voowork-backend
DATABASE_TEST_NAME=voowork_test bin/rails test \
  test/services/reports/tracking_duration_test.rb \
  test/controllers/api/v1/trackings_controller_test.rb \
  test/controllers/api/v1/reports/

# Frontend
cd ../voowork-frontend && npm run typecheck
```

---

## 10. Regras de negócio estilo Time Doctor (desktop)

Mapeamento das RNs implementadas no agente desktop (sem alteração de API).

| RN | Comportamento | Onde |
|----|---------------|------|
| **1 dispositivo ativo** | Antes de `start`/`restart`, consulta `GET /api/v1/trackings?status=active&user_id=…&unpaged=true`. Se houver `active` em **outro** `device`, bloqueia com `AgentError::Session`. | `tracking/reconcile.rs` → `prepare_before_start` |
| **Handoff mesmo device** | Se remoto `active` no **mesmo** `device` (órfão), `PATCH` fecha antes do novo `POST`. | `trackings/api.rs` + `prepare_before_start` |
| **Flush antes do POST** | Com token e sync habilitado, `sync_worker.flush_blocking` drena a outbox antes do start (first-sync-wins online). | `commands/tracking.rs` |
| **Reconcile no login** | Após login/`validate_auth_session`, se local `active` e id remoto ≠ local → `stop_tracking` local (conflito perdido). | `tracking/reconcile.rs` → `auth/commands.rs` |
| **UI (P1)** | `TrackingStatus` expõe `remoteActiveDevice`, `remoteActiveTrackingId`, `syncPending`; badges no `timer-app`. | `models.rs`, `status_report.rs`, `timer-app.tsx` |

### Endpoints usados (existentes)

```
GET  /api/v1/trackings?status=active&user_id={id}&unpaged=true
PATCH /api/v1/trackings/{id}  { tracking: { status: inactive, ended_at } }
```

### Offline

- Sem rede: reconcile e checagem remota são ignorados (log `warn`); start local continua offline-first.
- Com rede: regras acima aplicam antes de criar novo tracking.

---

## 8. Referências no código

- Desktop sync: `docs/features/03-sync.md`
- Tracking lifecycle: `docs/features/02-tracking.md`
- Regression: `docs/regression-test-checklist.md` (seções Quit, Sync, Reports)
- Backend service: `app/services/reports/tracking_duration.rb`
- Backend concern: `app/models/tracking/close_stale_actives.rb`
