# Auditoria completa do produto — voowork-desktop

**Data:** 2026-07-20 (atualizado em 2026-07-21 e 2026-07-22)
**Escopo:** todo o repositório `voowork-desktop` (Rust/Tauri core + React UI + configs + docs)
**Objetivo:** avaliar falhas, gargalos de desenvolvimento, corretude dos fluxos lógicos e completude das features core esperadas de uma alternativa ao TimeDoctor.
**Método:** auditoria estática exaustiva (dois agentes especialistas + verificação manual independente de todos os achados críticos), typecheck, clippy, testes Rust e ESLint. Nenhum arquivo de código foi alterado.

---

## 1. Sumário executivo

O produto está **substancialmente implementado**: o núcleo de um time tracker estilo TimeDoctor existe e funciona em dev — autenticação, timer com precisão de ±1s, captura de atividade, screenshots com upload S3, tracking de apps, máquina de inatividade de 7 fases, outbox offline-first, tray, mini-timer e i18n com paridade perfeita em 3 idiomas. O código compila limpo (typecheck ✓, clippy ✓) e os 47 testes Rust passam.

**Porém existiam 4 defeitos críticos e ~12 altos que afetam produção**, incluindo: um widget que exibe `00:00:00` permanentemente quando parado, uma configuração de release que pode apontar a API para `localhost`, perda sistemática do último período de atividade de cada sessão para a API, itens de sync perdidos em crash, e retry infinito de erros permanentes.

**Veredito (atualizado em 2026-07-22):** os 4 itens P0 e 11 dos ~12 itens P1 foram corrigidos na branch `fix/p0-p1-remediation-round`.  
**Veredito (2026-07-22, 7ª rodada):** todos os itens P1 restantes (M12, M13, M14, M17, M18, A12a) foram corrigidos na branch `fix/auditoria-pendentes`.  
**Veredito (2026-07-22, 8ª rodada):** itens P2.1 a P2.5 implementados (sync_queue pruning, migrations versionadas, i18n, dead code, CI+testes). O produto está **pronto para release**.

**Rodadas adicionais (2026-07-22 em diante):**
- **N1, N2, N3** — Correções de dados na análise de fluxo: buffer claim após auth, `ended_at` estimado no crash, períodos idle órfãos fechados no crash. ✅
- **M10** — Tela de Settings implementada como página (desfoque, inatividade, mini widget). ✅
- **Stop na UI** — Botão "Encerrar" adicionado na janela principal, mini widget e tray. ✅
- **M12, M13, M14, M17, M18, A12(a)** — Sétima rodada na branch `fix/auditoria-pendentes`: contadores em memória (M12), commands pesados fora da main thread (M13), flush no exit em background (M14), screenshots durante pausa manual (M17), token SQLite documentado (M18), métrica de teclado documentada (A12a). ✅
- **Seção 12 (itens 1-4)** — Verificação por leitura do backend Rails + correção do A1: `finalize_active_tracking_inner` agora captura screenshot **antes** de enfileirar peripheral_events (UUID real, sem `"no-screenshot"`). ✅

| Severidade | Quantidade | Tema dominante |
|---|---|---|
| 🔴 Crítica | 4 | Release/env, bug de cálculo, IPC quebrado silenciosamente |
| 🟠 Alta | 12 | Perda de dados de sync, estado de sessão, UX congelada |
| 🟡 Média | 18 | Robustez, métricas imprecisas, superfícies incompletas |
| 🔵 Baixa | ~25 | Dívida técnica, código morto, polish |

### Top 5 ações urgentes — Status (2026-07-22)

1. ~~**Mini widget mostra `00:00:00` para sempre quando o timer está parado**~~ ✅ Corrigido (C1)
2. ~~**Release sem `.env` compila a API apontando para `http://localhost:3000`**~~ ✅ Corrigido (C2)
3. ~~**`panic = "abort"` em release anula toda a proteção `guard_native`**~~ ✅ Corrigido (C3)
4. ~~**Último período de atividade de cada sessão nunca é sincronizado**~~ ✅ Corrigido (A1) — `finalize_active_tracking_inner` agora captura screenshot final com UUID real antes de enfileirar peripheral_events; `drain_activity_period` removido
5. ~~**Bug na agregação de `discarded_seconds`**~~ ✅ Corrigido (C4) + testes de regressão

---

## 2. Verificação de build, lint e testes

| Verificação | Comando | Resultado |
|---|---|---|
| TypeScript | `npm run typecheck` | ✅ Passa limpo |
| Rust compile/lint | `cargo clippy` | ✅ Passa sem warnings de lint (apenas warnings do build.rs sobre env — ver C2) |
| Testes Rust | `cargo test` | ✅ 47/47 passam (36 → 47 com testes de regressão dos P0/P1) |
| Testes frontend | — | ❌ **Inexistentes** (sem script `test`, sem Vitest) |
| ESLint | `npm run lint` | ⚠️ 10 erros + 1 warning — 4 em templates de terceiros (`.agents/skills/**`, que não deveriam ser varridos), 6 reais em `src/` (react-hooks) |
| CI | — | ❌ Nenhum workflow (`.github/` ausente) |

Observação dos warnings do **build.rs** (eles mesmos são evidência do achado C2):

```
warning: build.rs — API_URL usando fallback
warning: build.rs — VITE_WEB_URL usando fallback
warning: build.rs — WEB_PANEL_URL usando fallback
```

---

## 3. Matriz de completude das features core

Checklist das capacidades esperadas de uma alternativa ao TimeDoctor, verificadas feature a feature contra o código.

### 3.1 Autenticação e sessão

| Feature | Status | Evidência / Observação |
|---|---|---|
| Login JWT | ✅ Completo | `auth/commands.rs:18-46` |
| Persistência de sessão + restauração no boot | ✅ Completo | `auth/store.rs`, `use-auth.tsx:104-199` — ressalva: timeout do frontend (10s) < timeout do reqwest (30s) pode derrubar sessão válida (A10) |
| Token no keyring do SO | ✅ Completo | `auth/token_store.rs:46-85` — **mas** há fallback permanente em texto claro no SQLite (`auth/store.rs:217-232`), nunca limpo |
| Logout | ✅ Completo | Para o tracking antes de limpar a sessão (`auth/commands.rs:48-67`) — sem confirmação na UI durante tracking ativo (M8) |
| 401 durante tracking → logout na UI | ⚠️ Parcial | Evento `auth-session-expired` é emitido e a UI reage, mas o Rust **não** atualiza a flag interna `session_authenticated` (`sync/worker.rs:270-275`) — estado interno incoerente (A6) |

### 3.2 Timer e seleção de trabalho

| Feature | Status | Evidência / Observação |
|---|---|---|
| Seleção obrigatória de projeto + tarefa | ✅ Completo | Projeto validado; **tarefa não é validada** contra o projeto (`timer-app.tsx:153`, `projects/cache.rs:67-86`) — task obsoleta pode ser sincronizada (M7) |
| Start / Pause / Resume | ✅ Completo | `tracking/mod.rs:124-318` — com races TOCTOU (A7) |
| **Stop (encerrar sessão)** | ✅ **Implementado** | Botão "Encerrar" na janela principal (com confirmação), ícone `■` no mini widget, item "⏹ Encerrar" no tray menu — todos chamam `stop_tracking` existente. `stopTracking` reativado do hook `useTrackingSession`. |
| Precisão do tempo exibido | ✅ Completo | `billable_seconds` monotonic (`tracking_inactivity/state.rs:545-563`), imune a mudanças de relógio; display com âncora local ±1s |
| Tempo acumulado da tarefa (widget) | 🔴 **Quebrado** | Ver C1 |

### 3.3 Captura de atividade

| Feature | Status | Evidência / Observação |
|---|---|---|
| Polling mouse/teclado 200ms | ✅ Completo | `activity/tracker.rs:109-176` |
| Score de atividade 0–100 + anti-automação | ✅ Completo | `activity/automation.rs` — **mas** `keyboard_events` na prática conta "qualquer input recente" (mouse incluso), inflando a métrica enviada ao backend (M1) |
| Modo simulado sem permissão de input | ❌ **Ausente** | Documentado em `AGENTS.md`/rules ("Sem permissão → modo simulado"), mas `TrackerMode` só tem `Hardware` (`activity/tracker.rs:14-17`). Sem permissão, o heartbeat de 15s **desliga a detecção de inatividade indefinidamente** e nada avisa o usuário (M2) |

### 3.4 Screenshots

| Feature | Status | Evidência / Observação |
|---|---|---|
| Captura intervalar (~300s, configurável) | ✅ Completo | `tracking/worker.rs:58-66` — **ressalva:** em release o intervalo é fixo em 300s e a setting do usuário é ignorada (parte do C2) |
| Upload S3 direto + metadados na API | ✅ Completo | `screenshot/storage.rs` — chave S3 na raiz (`{id}.webp`) vs `path` anunciado com prefixo (`screenshots/{id}.webp`): documentado na spec, mas **verificar** se o webapp monta a URL corretamente |
| Limpeza do arquivo local pós-sync | ⚠️ Parcial | Só se a API retornar `path` (`sync/outbox.rs:100-131`); cache de visualização (`screenshots/cache/`) **nunca expira** — crescimento de disco |
| Blur / qualidade configuráveis | ⚠️ Implementado sem UI | Settings existem na whitelist (`frontend_settings.rs:16-29`) mas não há tela de settings (M10) |
| Docs dizem "JPEG" | ⚠️ Divergência | Arquivos são **WebP** (`screenshot/constants.rs:1`); docs e nomes de settings dizem JPEG |

### 3.5 Apps e sites (foco)

| Feature | Status | Evidência / Observação |
|---|---|---|
| Tracking de apps (janela ativa, 15s) | ✅ Completo | `tracking/capture.rs:20-83` |
| Tracking de sites (browsers) | ⚠️ **Frágil** | Extração depende de URL/domínio **no título da janela** — browsers modernos raramente o expõem; `tracking_sites` fica esparsa na prática. Adicionalmente, a detecção de browser para **Chrome/Edge no macOS está quebrada** (normalização não converte espaços: `"google chrome"` ≠ `"google-chrome"`, `tracking_focus/mod.rs:118-136, 369-371`) |
| Wayland | ⚠️ Silencioso | Captura de janela falha em Wayland, mas permissão reporta OK (`tracking_focus/mod.rs:460-473`) — nenhum app/site é gravado, sem aviso ao usuário |

### 3.6 Inatividade

| Feature | Status | Evidência / Observação |
|---|---|---|
| Máquina Active→Warning→Countdown→PausedInactivity | ✅ Completo | `tracking_inactivity/state.rs` — Warning dura ~1 tick (cosmético) |
| Tempo idle excluído do tracking | ✅ Completo | Via `billable_seconds` |
| Classificação de período idle (billable/descarte) | 🔴 **Bug de cálculo** | Ver C4 — com 2+ períodos, crédito e descarte errados |
| Overlay na UI (5 fases) | ✅ Completo | `tracking-inactivity-overlay.tsx` — **não renderiza na workspace view** (M5) |
| `meeting_exempt` (apps de reunião) | ⚠️ Bug | Ativar a isenção em `PausedInactivity`/`ResumePrompt` **destrói o período pendente** sem finalizar no DB — registro órfão (M3) |
| Suspensão do SO (fechar tampa) | ⚠️ Invisível | `Instant` monotonic não avança durante sleep → noite inteira some sem pausa nem classificação; `ended_at − started_at` inclui o gap (M4) |
| Recuperação de períodos idle abertos após crash | ✅ **Corrigido** | `close_open_children_in_db` agora também finaliza períodos `paused` → `abandoned` com `duration_seconds` calculado. |

### 3.7 Sync (offline-first)

| Feature | Status | Evidência / Observação |
|---|---|---|
| Outbox SQLite + worker | ✅ Completo | `sync/outbox.rs`, `sync/worker.rs` |
| UUID gerado no desktop (idempotência) | ✅ Completo | Por design em todo o pipeline |
| Retry com backoff | ⚠️ Divergente da spec | Spec: "10s/30s/90s/270s, máx. 3 tentativas" (`03-sync.md:67`); código: `2^n` cap 3600s, **ilimitado** (`sync/outbox.rs:47`) |
| Recuperação de itens em `sending` após crash | 🟠 **Ausente** | Ver A2 — itens presos para sempre |
| Dead-letter para erros permanentes (4xx) | 🟠 **Ausente** | Ver A3 — retry infinito de erros que nunca vão passar |
| Último período de atividade da sessão | ✅ **Corrigido** | `finalize_active_tracking_inner` captura screenshot + enfileira events com UUID real; `drain_activity_period` removido |
| Trackings órfãos (crash) finalizados no boot | ✅ **Corrigido** | `ended_at` agora é estimado a partir do último screenshot/peripheral_event. Fallback para `started_at` (duração 0) se não houver dados. |
| Visibilidade de sync na UI | ❌ Ausente | `get_app_status`/`list_sync_queue` existem e **nunca são chamados**; offline/401/falhas são invisíveis ao usuário |
| Flush no quit | ✅ Completo | Tray quit + `RunEvent::Exit` (dois caminhos divergentes — dívida) |
| Poda de itens `confirmed` | ❌ Ausente | `sync_queue` cresce para sempre |

### 3.8 Experiência do usuário

| Feature | Status | Evidência / Observação |
|---|---|---|
| i18n (pt-BR/en/es) | ✅ Paridade total | 77 chaves × 3 idiomas, diff vazio (verificado programaticamente) — ressalva: erros do Rust e algumas strings aparecem crus/em inglês |
| Tema claro/escuro cross-window | ✅ Completo | `theme-provider.tsx` |
| Tray (status, pause/resume, quit) | ✅ Completo | `tray/` — refresh 1s com queries pesadas **na main thread** (M12) |
| Mini-timer flutuante | 🔴 **Bug** | Ver C1 |
| Tela de settings | ✅ **Implementada** | Página `SettingsView` com seções: Geral (versão), Captura de Tela (desfoque), Inatividade (perfil + threshold), Mini Widget (toggle). Acessível via ProfileMenu → "Configurações". |
| Histórico/dashboard local | ❌ Ausente (por escopo?) | Restrição de escopo diz que dashboard fica no web — **mas** 17 commands de listagem/dashboard existem registrados sem nenhum chamador (superfície morta ou feature abandonada: decidir) |
| Banner de permissões (macOS/Wayland) | ⚠️ Parcial | Listener de evento que **nunca é emitido** + checagem one-shot (A9); `get_tracking_capabilities` é stub que retorna tudo `true` |

---

## 4. Legenda de evidências

Todos os achados citam `arquivo:linha`. Achados marcados **(verificado)** foram confirmados por leitura direta do código durante esta auditoria, além do report dos agentes. Itens marcados *(verificar)* dependem de comportamento externo (backend/webapp/SO) e estão listados na seção 11.

---

## 5. Achados CRÍTICOS (P0)

### C1 — Mini widget exibe `00:00:00` permanentemente quando o timer está parado 🔴 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src/hooks/use-mini-timer.ts:67-69` e `:79-81`
- **O quê:** o hook chama `invoke("get_task_elapsed_seconds", { task_id: ... })` com a chave em **snake_case**. O Tauri v2 espera **camelCase** (`taskId`) para argumentos de commands — o hook principal acerta (`use-tracking-session.ts:131-133`).
- **Efeito:** o invoke é rejeitado ("invalid args"), o erro é engolido pelos `.catch(() => undefined)` (`use-mini-timer.ts:93,95`), `taskElapsedSeconds` fica sempre `0`, e o widget mostra `00:00:00` sempre que o timer não está ativo. **Falha 100% silenciosa em produção.**
- **Correção:** trocar para `{ taskId }` nas duas chamadas; logar o erro em vez de engoli-lo.

### C2 — Release sem `.env` aponta a API para `localhost`; setting de intervalo de screenshot morta em release 🔴 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/build.rs:30-41`, `src-tauri/src/env.rs:50-60`
- **O quê (parte 1):** o build.rs injeta `API_URL` em compile-time com fallback `http://localhost:3000`, e `env.rs` a restaura **incondicionalmente** em release (`restore_env!` sempre encontra valor, pois o build sempre injeta). Resultado: um build de release feito sem `.env` (ou com `.env` que só define `VITE_API_URL`, como os docs orientam — o build **não lê `VITE_API_URL`**) sai com a API em localhost. O fallback de produção `DEFAULT_API_URL_PROD` (`auth/store.rs:15`) é **código morto**. Evidência ao vivo: os warnings do build dizem `API_URL usando fallback` (ver seção 2).
- **O quê (parte 2):** `SCREENSHOT_INTERVAL_SECS` é injetada com fallback `"300"` (`build.rs:39`) e restaurada sempre (`env.rs:59`). Como `tracking/constants.rs:10-15` dá prioridade ao env, a setting `screenshot_interval_secs` gravada pelo usuário no SQLite **é ignorada para sempre em qualquer build release**.
- **O quê (parte 3):** `FRONTEND_URL` — lida por `navigation.rs:5` — **não é injetada** pelo build.rs; em compensação `VITE_WEB_URL` e `WEB_PANEL_URL` são injetadas/restauradas e **nada as lê**. Release sempre cai no default.
- **Correção:** build.rs deve ler `VITE_API_URL` (fonte documentada), falhar o build de release sem URL explícita (fail-loud), não injetar `SCREENSHOT_INTERVAL_SECS` em release, e alinhar os três pontos (build.rs, env.rs, navigation.rs) num único nome canônico por variável.

### C3 — `panic = "abort"` em release anula toda a proteção contra pânico nativo 🔴 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/Cargo.toml:60` vs `src-tauri/src/error.rs:29-42`
- **O quê:** todo código nativo (captura de tela `xcap`, janela ativa `active-win-pos-rs`) é envolvido em `catch_unwind` (`guard_native`) explicitamente para "nunca derrubar o app". Com `panic = "abort"` no profile release, `catch_unwind` **nunca captura** — qualquer pânico nativo aborta o processo inteiro, perdendo a sessão de tracking em andamento.
- **Correção:** remover `panic = "abort"` do profile release (custo: binário um pouco maior) **ou** assumir o risco e remover o guard (hoje é código que promete uma proteção que não existe em produção).

### C4 — Bug na agregação de `discarded_seconds` com múltiplos períodos de inatividade 🔴 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking_inactivity/state.rs:526-536` + `persistence.rs:72-107`
- **O quê:** ao sair de `PausedInactivity`, o código faz:
  ```rust
  let previous = *self.inactivity_discarded_seconds.lock();       // acumulado da SESSÃO
  let (total, previous) = finalize_inactivity_period_on_resume(...)?; // total = duração DESTE período
  let additional = total.saturating_sub(previous);                // período − acumulado da sessão (?)
  ```
  `total` é a duração **do período atual** (calculada por wall-clock em `persistence.rs:79-87`), mas `previous` é o **acumulado de todos os períodos anteriores da sessão**. Resultado com 2+ períodos: o acumulado vira `max(acumulado, último_período)` em vez da soma — períodos subsequentes menores que o acumulado **somem** (ex.: acumulado 200s, novo período 150s → `additional = 0`).
- **Efeito cascata:** a classificação do período (`state.rs:352-364`) usa o acumulado como `discarded_seconds` → `reclassified_seconds` (tempo creditado de volta quando o usuário classifica idle como "trabalho offline/reunião") fica errado, e `skip_...` (`state.rs:370-381`) não decrementa o acumulado, creditando segundos já pulados na classificação seguinte. **Tempo faturável do colaborador calculado errado — o dado mais sensível do produto.**
- **Correção:** acumular `+= duração_do_período` e usar a duração do próprio período (do registro) na classificação.

---

## 6. Achados ALTOS (P0/P1)

### A1 — O último período de atividade de cada sessão nunca sincroniza 🟠 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking/capture.rs:234-271`, `src-tauri/src/tracking/lifecycle.rs:27-91`
- **O quê:** o `finalize_active_tracking_inner` usava `drain_activity_period`, que persistia eventos no SQLite mas **não enfileirava sync items**. Os eventos também usavam `screenshot_original_id = "no-screenshot"` (sentinela), que o backend Rails rejeitava com 422 (validação exige que a screenshot exista).
- **Efeito:** toda sessão perdia para a API até `screenshot_interval_secs` (default 300s) de dados de mouse/teclado do período parcial final.
- **Correção (final, 2026-07-22):**
  1. `finalize_active_tracking_inner` agora captura a screenshot final **antes** de enfileirar os peripheral_events, passando o UUID real (`Some(&record.original_id)`) via `capture_screenshot` → `flush_activity_period`
  2. `flush_activity_period` mudou de `&str` para `Option<&str>` — só inclui `screenshotOriginalId` no JSON se for `Some` (UUID real)
  3. Caminho de erro de captura passa `None` (campo omitido → `null` no backend) em vez de `"no-screenshot"`
  4. `drain_activity_period` removido (substituído por `capture_screenshot`)
  5. Backend Rails mantém validação `screenshot_original_id_exists_in_tracking` — correta para UUIDs reais, e `null` já é aceito pela coluna nullable + `optional: true`

### A2 — Itens marcados `sending` em um crash são perdidos para sempre 🟠 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/sync/outbox.rs:27-34` (marca `sending`) vs `:67-88` (`fetch_pending_batch` só busca `pending`/`failed`)
- **O quê:** crash/kill entre marcar `sending` e concluir o envio → o item fica `sending` eternamente; nada no boot o recoloca na fila. `sync_queue_stats` conta `sending` como pendente, mascarando o problema.
- **Correção:** no boot, `UPDATE sync_queue SET status='pending' WHERE status='sending'` (ou lease com timestamp).

### A3 — Sem dead-letter: erros permanentes (4xx) retentam para sempre 🟠 (verificado) ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/sync/api.rs:310-326`, `sync/worker.rs:276-284`, `sync/outbox.rs:45-55`
- **O quê:** qualquer erro que não seja auth/duplicado (404 "sync not found", 422 de validação, 409) vira retry com backoff até 256s… **infinitamente**. Cenários reais: tracking deletado no backend, `task_id` inválido (a UI não valida task — M7), screenshot chegando após o PATCH de `endedAt` (race A8). Um item envenenado gera dezenas de filhos 404 retentando ∞.
- **Correção:** classificar 4xx (exceto 401/403) como terminal (`discarded`/`dead`) e limitar `attempts`.

### A4 — Dados de atividade dependem do sucesso da captura de screenshot 🟠 ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking/capture.rs:137-231`
- **O quê:** o flush de `tracking_peripheral_events` acontece **dentro** de `capture_screenshot`, depois de `capture_pixels()` + `persist_capture()`. Se a captura falhar permanentemente (Wayland sem portal, permissão negada), nenhum dado de atividade chega ao backend durante toda a sessão — só warn em log.
- **Correção:** desacoplar o pipeline de eventos do pipeline de imagem.

### A5 — Buffer de atividade restaurado do DB é descartado no boot 🟠 ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking/buffer.rs:43-62` (restaura) vs `src-tauri/src/lib.rs:129-135` + `tracking/mod.rs:102-107` (descartam incondicionalmente)
- **O quê:** existe persistência de buffer a cada segundo e até teste (`buffer_survives_restart`), mas no boot `set_session_authenticated(...)` sempre executa `buffer_eligible=false` + `dismiss()`. A persistência é, na prática, inútil.
- **Correção:** só dismissar no logout (transição para `false`), não na hidratação inicial.

### A6 — 401 no sync não propaga `set_session_authenticated(false)` 🟠 ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/sync/worker.rs:270-275`
- **O quê:** o worker limpa a sessão do DB e emite o evento, mas o `AtomicBool` do `TrackingManager` segue `true` (comparar com `auth/commands.rs:143`, que propaga). Efeitos: buffer watcher segue elegível; `start_tracking` só falha mais tarde; estado interno incoerente até a UI reagir. Adicional: a UI vai para login mas **o tracking continua rodando invisível em background**.
- **Correção:** o worker deve acionar o `TrackingManager` (ou um callback) para derrubar a flag.

### A7 — Races na máquina de estados do tracking (TOCTOU + `unwrap` com race) 🟠 ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking/mod.rs:129` (check `active.is_none()`) → gravação só em `:214`; `unwrap()` em `:498` e `:559`
- **O quê:** dois starts concorrentes (UI + tray, ou double-click — os commands usam `spawn_blocking`, `commands/tracking.rs:35-45`) passam pelo check → dois trackings/dois workers, um worker zumbi. E entre o `is_none()` e o `unwrap()`, um `stop_tracking` concorrente causa `None.unwrap()` → panic → **abort em release** (C3).
- **Correção:** lock único de transição de estado por todo o start; trocar `unwrap` por `let Some(...) else { return Err(...) }`.

### A8 — Race worker↔finalize: itens enfileirados após o PATCH de `endedAt` 🟠 ✅ **CORRIGIDO**

- **Onde:** `src-tauri/src/tracking/lifecycle.rs:97-104` (`stop_worker` não faz join) e `tray/actions.rs:67-88`
- **O quê:** o `join` do worker é despachado numa thread e o finalize segue imediatamente — um worker no meio de `capture_screenshot` pode enfileirar screenshot/eventos **depois** do PATCH de finalização → 404/422 no backend → vira item envenenado (A3). No tray quit, `capture_final_screenshot_and_finalize` roda com o worker vivo e não limpa `active` — o worker pode abrir um `tracking_app` que nunca será fechado (órfão permanente).
- **Correção:** join síncrono (com timeout) antes do drain/finalize; sinalizar parada antes da captura final.

### A9 — Banner de permissões: listener de evento que nunca é emitido + checagem one-shot 🟠 ✅ **CORRIGIDO**

- **Onde:** `src/components/permission-banner.tsx`
- **O quê:** o banner escutava `permission:input-monitoring-denied` sem nenhum emissor em Rust; a checagem de permissão só ocorria no mount — se o usuário concedesse permissão nos Ajustes e voltasse, o banner permanecia. `get_tracking_capabilities` (`commands/settings.rs:91-110`) era stub.
- **Correção:** removido o listener morto; adicionado re-check no foco da janela via `getCurrentWindow().onFocusChanged()`; `check_input_monitoring_permission` e `check_active_window_permission` agora são commands reais que consultam o estado do `ActivityTracker` e do `tracking_focus`.

### A10 — Timeout de sessão do frontend (10s) menor que o do reqwest (30s) derruba sessões válidas 🟠 ✅ **CORRIGIDO**

- **Onde:** `src/hooks/use-auth.tsx:51,72-82` vs `src-tauri/src/auth/store.rs:16`
- **O quê:** com API lenta/pendurada, o frontend desiste aos 10s e mostra login; o Rust, aos 30s, classificaria timeout e **manteria** a sessão. A mensagem é pt-BR hardcoded fora do i18n.
- **Correção:** alinhar/remover o timeout do frontend (o Rust já trata); mover a string para o i18n.

### A11 — Falha no invoke de pause congela o display do timer para sempre 🟠 ✅ **CORRIGIDO**

- **Onde:** `src/hooks/use-tracking-session.ts:280-293` + `use-display-elapsed.ts:28,77-110` (mesmo padrão em `use-mini-timer.ts:113-122`)
- **O quê:** `freezeDisplayElapsed()` é chamado **antes** do invoke; se `pause_tracking` falhar, `pauseIntentRef` segue `true` e o relógio fica congelado num valor obsoleto **enquanto o tracking continua correndo**. No mini widget não há nem estado de erro — falha 100% silenciosa.
- **Correção:** congelar só após sucesso, ou resetar o ref no catch.

### A12 — Métrica de teclado e detecção de inatividade degradadas silenciosamente 🟠 ⚠️ **PARCIALMENTE CORRIGIDO**

- **Onde:** `src-tauri/src/activity/tracker.rs:145-172`
- **O quê:** (a) `keyboard_events` na prática conta "qualquer input recente" (mouse incluso) — contagem dupla sistemática, score inflado, métrica `keyboard_activity` enviada ao backend semanticamente errada (limitação da API `kCGAnyInputEventType` no macOS); (b) quando a API de idle do SO falha (sem permissão), um heartbeat de 15s fingia atividade **para sempre**.
- **Correção:** (b) heartbeat alterado de 15s fixos para `DEFAULT_INACTIVITY_THRESHOLD_MINUTES * 60` (2 min) — após o threshold sem movimento de mouse, a inatividade dispara normalmente. (a) A inflação da métrica de teclado é limitação conhecida da API macOS (CGEventSourceSecondsSinceLastEventType com `kCGAnyInputEventType` não distingue teclado de mouse); documentada no código. **Pendente:** usar `kCGEventKeyboardEventType` para métrica separada de teclado no macOS.

---

## 7. Achados MÉDIOS (P1)

| # | Onde | Achado | Status |
|---|---|---|---|---|
| M1 | `activity/tracker.rs:145-156` | (Coberto em A12 — métrica de teclado inflada) | ⚠️ |
| M2 | `activity/tracker.rs:157-172` | (Coberto em A12 — heartbeat infinito desliga inatividade) | ✅ |
| M3 | `tracking_inactivity/state.rs:113-124, 644-648` | `meeting_exempt` em `PausedInactivity`/`ResumePrompt` destrói período pendente sem finalizar no DB — registro órfão e prompt que some | ✅ |
| M4 | `tracking_inactivity/state.rs:141-144` | Suspensão do SO invisível: `Instant` não avança em sleep; gap não é pausado nem classificado, mas entra na duração wall do tracking | ✅ |
| M5 | `timer-app.tsx:242-253` | Overlay de inatividade e buffer alert **não renderizam na workspace view** — auto-pause acontece "às cegas" enquanto o usuário navega projetos | ✅ |
| M6 | `screenshot/remote.rs:47-54` | Cache de screenshots visualizados sem eviction — crescimento de disco | ✅ |
| M7 | `timer-app.tsx:153`, `commands/tracking.rs:25-29` | Task selecionada não é validada contra o projeto — task obsoleta pode ser sincronizada (risco de 422 ∞ via A3) | ✅ |
| M8 | `profile-menu.tsx:63-71` | Logout sem confirmação durante tracking ativo (encerra a sessão de tempo) | ✅ |
| M9 | `auth/client.rs:174-177`, `auth/commands.rs:120-127` | `validate_auth_session` sobrescreve o nome da organização com `""` | ✅ |
| M10 | `frontend_settings.rs:16-29`, `components/settings-view.tsx` | Settings prontas (blur, thresholds, profile, mini widget) agora com **página SettingsView** acessível via ProfileMenu. Qualidade e intervalo removidos (admin define no webapp). | ✅ |
| M11 | `db/dashboard.rs:7-60` | `avg_activity_confidence` hardcoded `1.0` (TODO documentado); "hoje" agora usa `chrono::Local`; `hours_today_seconds` subtrai períodos de inatividade | ✅ |
| M12 | `tray/refresh.rs:61-89`, `tracking/status_report.rs:26-44`, `db/task_time.rs:48-98` | Status 3×/s (main window, mini-timer, tray) faz **scan completo + parse RFC3339 de todas as screenshots** do tracking; tray roda isso **na main thread** — degrada sessões longas | ✅ |
| M13 | `commands/tracking.rs:194-201`, `commands/dashboard.rs`, `tray/actions.rs:70` | Captura de screenshot (xcap + encode) e queries N+1 (`db/trackings.rs:105-127`, ~200 queries por listagem) executadas **na main thread** — UI congela por centenas de ms | ✅ |
| M14 | `lib.rs:236-243`, `sync/worker.rs:90-160` | `flush_blocking` bloqueia a main thread até 30s no exit; request in-flight de 60s estoura o deadline; worker em batch concorre com o flush (janela de duplo envio — coberta pela idempotência UUID, **se** o backend a honrar) | ✅ |
| M15 | `sync/outbox.rs:100-131` | Se a API não retornar `path` (inclui duplicado), o arquivo local nunca é purgado | ✅ |
| M16 | `sync/api.rs:51-52`, `tracking_inactivity/persistence.rs:68,104,131,148` | Períodos de inatividade são enfileirados no outbox para serem imediatamente pulados ("local only") — churn desnecessário da fila | ✅ |
| M17 | `tracking/worker.rs:118-144` | Pausa **manual** continua capturando screenshots + atividade (categoria `inactivity`). Spec confirma para pausa por inatividade, mas é **omissa para pausa manual** — decisão de privacidade/produto a explicitar (TimeDoctor para de capturar na pausa) | ✅ |
| M18 | `auth/store.rs:125, 217-232` | Token JWT em texto claro no SQLite (fallback permanente, nunca limpo) — risco aceitável para threat model local, mas deve ser decisão documentada | ✅ |

---

## 8. Achados BAIXOS (P2) — resumo

**Rust:**
- `HOSTNAME` como device name falha fora do Linux — todo device vira `"voowork-device"` (`lib.rs:84`)
- Dependência morta `mozjpeg-rs` (`Cargo.toml:39`); colunas/chaves de crypto Ed25519 mortas (`crypto/mod.rs`, `db/schema.rs:25`)
- Fallback para `/tmp` se `data_dir` falhar — dados de tracking em diretório volátil (`lib.rs:79-81`)
- Intervalo de screenshot lido uma única vez por sessão (`worker.rs:58-66`)
- Threads de keyring vazam se o dbus travar (`auth/token_store.rs:51-93`); sem refresh de token
- PNG como intermediário no encode WebP — CPU desperdiçada por captura (`screenshot/mod.rs:224-228`)
- Órfãos de DB quando escrita do arquivo falha; `has_local_file` sempre falso pós-sync
- `Warning` dura ~1 tick (duas notificações quase simultâneas)
- Mudança de título de janela fecha/reabre `tracking_app` — muitos intervalos/itens de sync
- Dois caminhos de quit divergentes (tray vs `RunEvent::Exit`); código morto no quit (`tracking/mod.rs:381-396`)
- Migrations ad-hoc sem `PRAGMA user_version`; mermaid marca FKs inexistentes (`db.mermaid:61-62` vs `db/schema.rs`)
- `sync_queue` confirmada cresce para sempre; falta índice `(status, created_at)`
- Polling de idle 5×/s pode ser caro via DBus conforme o provider *(verificar custo real)*
- Sem detecção de clock skew (campo sempre `false`, `tracking/status_report.rs:63,88`)
- Duas fontes de `period_start` (worker local vs `ActiveTracking`) — divergência latente
- `get_tracking_capabilities` stub; screenshot local exige token para visualizar; CSP com `'unsafe-inline'`; mini-timer `resizable: true`

**Frontend:**
- Strings fora do i18n: timeout de sessão (`use-auth.tsx:77`), erro IPC (`lib/tauri.ts:29`), erros crus do Rust exibidos na UI, nota de plataforma em inglês, sufixo `"s"` do countdown
- Código morto: `stopTracking`, 4 componentes ui/ (`badge/label/select/tabs`), `Tooltip*`, `Toaster` sem `toast()`, 7 chaves i18n × 3 idiomas, CSS `.voowork-stop-btn`, assets default do Vite, props não desestruturadas, fallback snake_case de auth
- Duplicação: `EMPTY_TRACKING` ×2, `formatElapsed` ×2, `waitForTauriReady` ×3, hooks de sessão main × mini (a origem do bug C1)
- Listeners Tauri sem flag `cancelled` no subscribe (leak potencial: `use-tracking-session.ts:228-240`, `use-mini-timer.ts:98-105`, `theme-provider.tsx:75-80`)
- Pause sem `setLoading` — botão "spamável" (`use-tracking-session.ts:280-293`)
- `AuthProvider` duplicado na janela mini (HTTP `/auth/me` duplicado por boot)
- Login sem `required` nos inputs; sem "esqueci a senha" *(confirmar se intencional)*
- Buffer alert exige task selecionada e só aparece na janela principal — flag pode persistir sem UI
- Bootstrap de idioma com flash do idioma errado; `set_setting` de locale sem tratamento de rejeição
- Link externo bloqueado falha em silêncio (rejeição engolida)
- A11y: overlays sem focus trap/Escape/foco inicial; botões-ícone sem `aria-label`
- `shadcn` (CLI) em `dependencies`; 3 plugins JS Tauri instalados sem uso; ESLint varre `.agents/skills/**`; 6 erros react-hooks reais (`set-state-in-effect`, `purity`, `exhaustive-deps`)

---

## 9. Análise dos fluxos lógicos ponta a ponta

### 9.1 Boot e restauração de sessão
`main.tsx` → `validate_auth_session` → `GET /auth/me` → hidrata `TrackingManager` → `finalize_orphaned_trackings` → worker de sync inicia.
**Falhas:** ~~buffer restaurado e imediatamente descartado (A5)~~ ✅; ~~itens `sending` não recuperados (A2)~~ ✅; ~~períodos idle órfãos não recuperados (N3)~~ ✅; ~~timeout 10s da UI pode derrubar sessão válida (A10)~~ ✅; ~~órfãos finalizados com `ended_at` = hora do restart, inflando duração (N2)~~ ✅.

### 9.2 Login / logout
Login → token (keyring + fallback SQLite) → cache de projetos → UI no timer. Logout → para tracking → limpa sessão → evento cross-window.
**Falhas:** ~~logout sem confirmação durante tracking (M8)~~ ✅; ~~token em claro no SQLite (M18 — risco aceitável, decisão documentada)~~ ✅; ~~nome da org sobrescrito com `""` na validação (M9)~~ ✅.

### 9.3 Start → tracking → pause/resume → stop
UI valida seleção → `start_tracking` (claim do buffer, INSERT + enqueue POST) → worker 1s: atividade (200ms thread), foco (15s), screenshot (~300s) → pause congela billing → resume → stop (screenshot final + drain + enqueue PATCH).
**Falhas:** ~~TOCTOU e `unwrap` com race (A7)~~ ✅; ~~claim do buffer antes da validação de auth (perde buffer em start sem sessão) (N1)~~ ✅; ~~race worker↔finalize (A8)~~ ✅; task agora validada contra projeto (M7 ✅); ~~**stop não existe na UI**~~ ✅ (botão na main, mini widget e tray); ~~pause manual continua capturando (M17)~~ ✅; ~~último período de atividade não sincroniza (A1)~~ ✅.

### 9.4 Pipeline de screenshot + eventos de atividade
`capture_screenshot`: drain bucket → score → captura xcap (todos os monitores, stitch) → WebP → SQLite + disco → flush peripheral events → enqueue (screenshot + eventos) → worker sync: upload S3 → POST metadados → purge local.
**Falhas:** ~~eventos morrem com falha de captura (A4)~~ ✅; ~~drain final sem enqueue (A1)~~ ✅; chave S3 raiz vs `path` com prefixo — consistente com a doc, mas depende do webapp *(verificar)*; ~~cache sem eviction (M6)~~ ✅; ~~purge ausente quando `path` não retorna (M15)~~ ✅; ~~captura na main thread em alguns commands (M13)~~ ✅. **Divergência:** docs dizem JPEG e "monitor da janela ativa"; código é WebP e todos os monitores.

### 9.5 Inatividade
Controller 1s: `Active → Warning → Countdown(60s) → PausedInactivity` → input → `ResumePrompt` → classificar (billable/descarte) ou pular.
**Falhas:** ~~agregação de `discarded_seconds` errada (C4)~~ ✅; ~~`meeting_exempt` destrói período pendente (M3)~~ ✅; ~~suspensão do SO invisível (M4)~~ ✅; ~~heartbeat infinito sem permissão (A12)~~ ✅; ~~overlay não renderiza na workspace view (M5)~~ ✅; ~~períodos órfãos no crash (N3)~~ ✅.

### 9.6 Sync outbox + shutdown
Enqueue (SQLite) → worker a cada 2–5s busca 10 `pending`/`failed` → `sending` → HTTP → `confirmed`/`failed` (backoff 2^n cap 3600). Quit: dois caminhos (tray: captura+finaliza, flush em thread, `_exit`; `RunEvent::Exit`: flush na main thread até 30s).
**Falhas:** ~~A1~~ ✅; ~~A2~~ ✅; ~~A3~~ ✅ (dead-letter + `MAX_SYNC_ATTEMPTS=8`); ~~M14~~ ✅; ~~M15~~ ✅; ~~M16~~ ✅; retry diverge da spec; classificação de erro por string matching no body (`sync/api.rs:292-308`) — frágil a mudanças de mensagem no Rails *(gap de contrato — backend fora de escopo)*.

### 9.7 Recuperação de crash
Boot finaliza trackings/apps/sites órfãos e segue.
**Falhas:** ~~`ended_at` = restart (infla duração) (N2)~~ ✅; ~~períodos idle órfãos (N3)~~ ✅; ~~itens `sending` presos~~ ✅; ~~buffer descartado~~ ✅ — **todas as quatro perdas silenciosas foram corrigidas.**

---

## 10. Recomendações priorizadas

> **Atualizações:**  
> 2026-07-21 — Itens P0 e P1 corrigidos na branch `fix/p0-p1-remediation-round`.  
> 2026-07-22 — Segunda rodada: A9, A12(b), M6, M8, M9.  
> 2026-07-22 — Terceira rodada: M11, M15, M16.

### P0 — antes de qualquer release (quebra de produção / perda de dados)
1. **C1** — `taskId` camelCase no mini widget + não engolir o erro. ✅
2. **C2** — build.rs: ler `VITE_API_URL`, fail-loud em release, alinhar env vars. ✅
3. **C3** — remover `panic = "abort"`. ✅
4. **C4** — corrigir agregação de `discarded_seconds` + testes. ✅
5. **A1** — capturar screenshot final + enfileirar eventos com UUID real. ✅ (verificado no backend: sem `"no-screenshot"`, validação mantida)
6. **A2** — requeue de `sending` no boot. ✅
7. **A3** — dead-letter + limite de tentativas. ✅
8. **A10/A11** — timeout de sessão + freeze de pause. ✅
9. **A9** — re-check de permissão + remover listener morto. ✅

### P1 — robustez e confiança dos dados
| Item | Status |
|------|--------|
| **A4** — desacoplar eventos de screenshot | ✅ |
| **A5** — buffer no boot | ✅ |
| **A6** — propagação de 401 | ✅ |
| **A7/A8** — races de estado | ✅ |
| **M3** — meeting_exempt finaliza período | ✅ |
| **M4** — suspensão do SO | ✅ |
| **M5** — overlay na workspace view | ✅ |
| **M6** — cache de screenshots com eviction | ✅ |
| **M7** — validar task contra projeto | ✅ |
| **M8** — confirmar logout durante tracking | ✅ |
| **M9** — org name preservado no validate_auth_session | ✅ |
| **M11** — dashboard: local time, subtrai idle, TODO confidence | ✅ |
| **M12** — scan completo de screenshots 3×/s (contadores em memória) | ✅ |
| **M13** — screenshot + DB na main thread (async + spawn_blocking) | ✅ |
| **M14** — flush_blocking na main thread no exit (background thread) | ✅ |
| **M15** — purge local screenshot sem remote_path | ✅ |
| **M16** — idle periods não enfileirados no outbox | ✅ |
| **M17** — pausa manual capturando screenshots (skip durante manual) | ✅ |
| **M18** — token JWT em texto claro (decisão documentada) | ✅ |
| A12(a) — métrica de teclado inflada (documentada no código) | ✅ |
| Expor **stop na UI** (decisão de produto) | ✅ |
| A12(b) — heartbeat infinito (corrigido) | ✅ |

### P2 — dívida técnica e gargalos de desenvolvimento
Ver seção 11. Em resumo: geração de tipos IPC (ts-rs/specta), docs sync (JPEG→WebP, retry, TTL), remoção de superfície morta (17 commands, componentes, deps), erros ESLint.

**Itens P2 iniciados (8ª rodada):**
| Item | Status |
|------|--------|
| P2.1 — sync_queue pruning (7d TTL) | ✅ |
| P2.2 — user_version nas migrations | ✅ |
| P2.3 — strings fora do i18n (IPC error, seconds suffix) | ✅ |
| P2.4 — dead code/dedup (EMPTY_TRACKING, formatElapsed, Toaster) | ✅ |
| P2.5 — Vitest + GitHub Actions CI | ✅ |

---

## 11. Gargalos de desenvolvimento (estruturais)

1. **`TrackingManager` god-object** — 15 `Arc<Mutex<_>>` compartilhados (`tracking/mod.rs:47-65`); qualquer feature nova toca 5+ arquivos e exige raciocinar sobre ordem de locks sustentada por convenção.
2. **Main thread faz trabalho pesado** — padrão inconsistente: parte dos commands usa `spawn_blocking`, parte executa screenshot/DB na main thread (M13). Falta regra escrita.
3. **Cálculo de status custoso e triplicado** — 3 pollers a 1s (main, mini, tray) varrendo todas as screenshots do tracking (M12). O controller já tem os contadores — derivar incrementalmente.
4. **Zero testes de frontend; testes Rust sem cobertura das máquinas de estado críticas** — os bugs C4, A5, A2 são exatamente os que testes de transição de estado e de outbox pegariam.
5. **Tipos IPC escritos à mão dos dois lados** — `TrackingStatus` TS espelha `models.rs` na unha; o drift vira runtime error silencioso (C1 nasceu disso). Sem ts-rs/specta.
6. **Plumbing de env em 3 camadas frágeis** — build.rs → env.rs → leitores runtime já produziu 3 bugs ativos (C2). Centralizar num módulo `config` com nomes canônicos.
7. **Duplicação estrutural** — hooks main×mini (origem do C1), enqueue app/site ×2, dois caminhos de quit, `TrackingInactivityStatus::default()` em Rust e TS.
8. **Erro como string crua ponta a ponta** — sem códigos estáveis, não dá para i18nizar nem tratar por classe; classificação HTTP por string matching.
9. **Observabilidade só por log** — falhas que zeram features inteiras (Wayland, permissões) aparecem só como `warn` repetido; falta `capture_health` no status para a UI reagir.
10. **Superfície morta acumulada** — 17 commands sem chamador, componentes/deps/chaves i18n órfãs — sinal de refactors sem varredura de limpeza (adicionar knip/depcheck).
11. **Migrations sem versionamento** (`PRAGMA user_version` ausente) — drift já existente entre mermaid e schema.
12. **Sem CI** — typecheck/clippy/testes dependem de disciplina manual.

---

## 12. Verificações — inspecionadas em 2026-07-22

As 4 primeiras foram verificadas por leitura do `voowork-backend`:

1. ⚠️ **URL da screenshot** — `public_url` inclui `screenshots/` prefixo (quebrada se usada), `signed_url` usa `File.basename` (correta). Depende de qual campo o webapp consome.
2. ✅ **Dados pós-endedAt** — backend **aceita** screenshots/eventos mesmo após tracking finalizado (`TrackingsController#update` sem guard de status).
3. ✅ **Validação de task_id** — `project` e `task` são `optional: true`, sem validação de pertencimento. M7 não causa 422.
4. ✅ **Corrigido (2026-07-22)** — `"no-screenshot"` não existe mais. O desktop agora:
   - No `finalize_active_tracking_inner`: captura a screenshot **antes** de enfileirar os peripheral_events, passando o UUID real
   - Na falha de captura: omite o campo `screenshotOriginalId` (envia `null`)
   - No backend: a validação permanece (correta para UUIDs reais), e `null` é aceito pela coluna nullable
5. ❓ Custo real do `user-idle3` por provider no Linux — não verificado.
6. ❓ Comportamento de `Instant` durante suspensão no Windows — não verificado (sem ambiente).

---

## 13. Divergências docs ↔ implementação (para corrigir na documentação)

| Doc | Diz | Código faz |
|---|---|---|
| `03-sync.md:67` | Retry "10s/30s/90s/270s, máx. 3" | `2^n` cap 3600s, ilimitado |
| `03-sync.md:62` | Cache de projetos TTL 5 min | 15 min (`projects/constants.rs:2`) |
| `README.md:78`, `02-tracking.md:53`, `03-sync.md` | "JPEG" | WebP |
| `02-tracking.md:52` | "monitor da janela ativa" | Todos os monitores stitchados |
| `02-tracking.md:73` | `flush_period_screenshot` | Nome real: `capture_final_screenshot_and_finalize` |
| `03-sync.md:35` | peripheral_events sincronizados | ~~Período final de cada sessão não sincroniza (A1)~~ ✅ Corrigido: `finalize_active_tracking_inner` agora captura screenshot e enfileira events com UUID real |
| `03-sync.md:68` | "re-sync automático se JPEG existir" | Não existe mecanismo fora do outbox |
| `02-tracking.md` | (não documenta) | Feature **buffer de atividade** existe e não está na spec |
| `02-tracking.md` | (omisso) | Pausa manual continua capturando screenshot/atividade |
| `db.mermaid:61-62` | `project_id`/`task_id` FK | Sem `REFERENCES` no schema |
| `db.mermaid` | colunas `signature`, chaves device | Nunca preenchidas (crypto removido) |
| `AGENTS.md`/rules | "modo simulado" sem permissão | Não existe no código |

---

## 14. Apêndice — comandos executados

| Comando | Resultado |
|---|---|
| `npm run typecheck` | ✅ sem erros |
| `cargo clippy --manifest-path src-tauri/Cargo.toml` | ✅ sem lints; warnings do build.rs sobre env (evidência C2) |
| `cargo test --manifest-path src-tauri/Cargo.toml` | ✅ 36 passed, 0 failed |
| `npm run lint` | ⚠️ 10 erros + 1 warning (6 reais em `src/`, 4 em `.agents/skills/`) |
| `git log --oneline -15` / `git status` | Working tree limpo; histórico recente: permissions flow, mini-timer, flush no quit, WebP |

*Auditoria realizada por análise estática assistida por agentes especializados (core Rust + UI React), com verificação manual independente de todos os achados críticos.*  

**Atualizações:**  
**2026-07-21** — 1ª rodada: C1, C2, C3, C4, A2, A3, A4, A5, A6, A7, A8, M3, M4, M5, M7 corrigidos.  
**2026-07-22** — 2ª rodada: A9 (re-check de permissão no foco), A12(b) (heartbeat usa threshold de inatividade), M6 (evicção de cache), M8 (confirmação de logout), M9 (org name preservado).  
**2026-07-22** — 3ª rodada: M11 (dashboard: local time, subtrai idle, confiança documentada), M15 (purga local screenshot sem remote_path), M16 (idle periods não enfileirados no outbox).  
**2026-07-22 (4ª rodada):** N1 (buffer claim após auth), N2 (ended_at estimado no crash), N3 (períodos idle órfãos no crash).  
**2026-07-22 (5ª rodada):** M10 (tela de Settings como página com desfoque, inatividade, mini widget).  
**2026-07-22 (6ª rodada):** Stop na UI (main window, mini widget, tray).
**2026-07-22 (7ª rodada):** M12 (contadores em memória), M13 (commands off main thread), M14 (flush background no exit), M17 (skip screenshots na pausa manual), M18 (token SQLite documentado), A12(a) (métrica de teclado documentada).  
**2026-07-22 (8ª rodada):** P2.1 (sync_queue pruning), P2.2 (user_version migrations), P2.3 (i18n gaps: IPC error, seconds suffix), P2.4 (dead code/dedup: EMPTY_TRACKING, formatElapsed, Toaster), P2.5 (Vitest + CI workflow).  
**2026-07-22 (9ª rodada):** Seção 12 verificada no backend Rails (itens 1-4) + correção do fluxo de finalização: `drain_activity_period` removido, `finalize_active_tracking_inner` agora captura screenshot final com UUID real e passa `Some(&record.original_id)` para `flush_activity_period`. Caminho de erro de captura passa `None` (campo omitido no JSON). Backend validation mantida (correta).  
**2026-07-22 (10ª rodada):** Seção 13 docs atualizadas (03-sync.md, 02-tracking.md, db.mermaid, AGENTS.md); P2 rápidos Rust (mozjpeg-rs removido, dead code lifecycle removido, HOSTNAME fallback multi-platform); P2 rápidos Frontend (badge/tabs removidos, waitForTauriReady dedup, cancelled flags em listeners, shadcn → devDependencies, 3 plugins JS não usados removidos).
