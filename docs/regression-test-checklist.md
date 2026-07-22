# Checklist de Testes Regressivos — Voowork Desktop

Checklist completo para validação regressiva de todas as funcionalidades do agente desktop antes de releases ou após mudanças significativas.

**Como usar:** execute os itens na ordem das seções. Marque `[x]` apenas quando o comportamento observado for idêntico ao esperado. Registre falhas com passos de reprodução, versão do app e plataforma.

**Legenda de ambiente:** 🐧 Linux · 🪟 Windows · 🍎 macOS (marque apenas as plataformas suportadas pela release)

---

## 0. Preparação do ambiente

- [ ] Backend Rails rodando e acessível em `VITE_API_URL`
- [ ] S3/Garage configurado (`S3_ENDPOINT`, `S3_REGION`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`, `S3_BUCKET`)
- [ ] Usuário de teste com pelo menos 1 projeto e 1 tarefa atribuídos
- [ ] Banco local em estado conhecido (`~/.local/share/voowork-desktop/voowork-desktop.db`)
- [ ] Verificações estáticas passando:
  - [ ] `npm run typecheck`
  - [ ] `npm run lint`
  - [ ] `npm test` (vitest)
  - [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
  - [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -D warnings`
  - [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Build de produção funciona: `npm run build` e `npm run tauri build`

---

## 1. Autenticação

### 1.1 Login

- [ ] Login com credenciais válidas autentica e abre a tela principal
- [ ] Token e perfil são persistidos no SQLite (`settings`)
- [ ] Token é armazenado no keyring do SO
- [ ] Cache de projetos é sincronizado automaticamente após o login
- [ ] Login com senha incorreta exibe **apenas** a mensagem da API (ex.: "E-mail ou senha inválidos"), sem prefixos técnicos
- [ ] Login com e-mail inexistente exibe mensagem de erro amigável
- [ ] Login com API fora do ar exibe erro de conexão sem quebrar a UI
- [ ] Campos vazios / formato de e-mail inválido são validados antes do submit
- [ ] Estado de loading no botão durante a autenticação (sem duplo submit)

### 1.2 Sessão

- [ ] Fechar e reabrir o app com sessão válida → entra direto na tela principal (sem novo login)
- [ ] No boot, `validate_auth_session` chama `GET /api/v1/auth/me` e confirma o token
- [ ] Token expirado/inválido no boot → redireciona para login
- [ ] API retornando 401 durante sync → evento `auth-session-expired` dispara logout automático na UI
- [ ] `get_auth_state` retorna o estado correto sem chamada de rede

### 1.3 Logout

- [ ] Logout limpa a sessão local (SQLite + keyring)
- [ ] Logout com tracking ativo é tratado (finaliza ou bloqueia, conforme comportamento esperado)
- [ ] Após logout, a tela de login é exibida e dados do usuário anterior não aparecem

---

## 2. Projetos e tarefas

- [ ] `list_projects` retorna os projetos do usuário (member: atribuídos; admin: todos)
- [ ] Tarefas de cada projeto são carregadas (`GET /api/v1/projects/:id/tasks`)
- [ ] `sync_projects` força atualização do cache
- [ ] Cache respeita TTL de 15 minutos (não refetch antes disso)
- [ ] Mudança de `organization_id` reseta o cache automaticamente
- [ ] Seleção de projeto + tarefa persiste entre reinícios do app
- [ ] Projeto sem tarefas / usuário sem projetos → UI lida graciosamente (estado vazio)
- [ ] Projeto desativado no backend some após refresh do cache

---

## 3. Timer — ciclo de vida do tracking

### 3.1 Iniciar

- [ ] `start_tracking` com projeto + tarefa inicia a sessão
- [ ] `tracking_id` (UUID v4) é gerado localmente e gravado no SQLite
- [ ] Item é enfileirado na `sync_queue` e `POST /api/v1/trackings` usa o **mesmo** UUID
- [ ] Timer começa a contar na UI
- [ ] Não é possível iniciar um segundo tracking sem parar o atual
- [ ] `restart_tracking` (trocar projeto/tarefa) funciona sem perder dados da sessão anterior

### 3.2 Durante a sessão

- [ ] `get_tracking_status` (polling da UI) retorna estado e tempo decorrido corretos
- [ ] `get_task_elapsed_seconds` acumula tempo por tarefa corretamente
- [ ] Contador do timer não drifta após longos períodos (30min+)
- [ ] Timer sobrevive a minimizar/fechar a janela (app continua via tray)

### 3.3 Pausar / retomar (manual)

- [ ] `pause_tracking` pausa o contador e indica estado pausado na UI
- [ ] Durante pausa manual, screenshots são **puladas** (compatível TimeDoctor)
- [ ] `resume_tracking` retoma a contagem de onde parou
- [ ] Tempo pausado **não** conta como tempo trabalhado

### 3.4 Parar

- [ ] `stop_tracking` finaliza a sessão
- [ ] Screenshot final é capturada com UUID real (fail-loud, sem placeholder)
- [ ] Peripheral events do período final são enfileirados com o mesmo UUID
- [ ] Apps/sites abertos são fechados (com `ended_at`)
- [ ] `PATCH /api/v1/trackings/:id` é enviado com `status: inactive` e `ended_at`
- [ ] UI volta ao estado "sem tracking ativo"

### 3.5 Fechamento do app (quit/tray)

- [ ] Ao fechar com tracking ativo: screenshot final + fechamento de apps/sites + PATCH de finalização
- [ ] Worker é parado com join síncrono (sem cortar no meio de um envio)
- [ ] Trackings órfãos (crash/kill anterior) são finalizados no próximo boot com `ended_at` estimado

---

## 4. Captura de atividade (mouse/teclado)

- [ ] Com permissão de input: movimentar o mouse incrementa contagem de eventos
- [ ] Com permissão de input: digitar incrementa contagem de eventos de teclado
- [ ] Activity score (0–100) reflete o volume de eventos (threshold 500)
- [ ] `drain_bucket()` esvazia o bucket a cada ciclo de screenshot (sem acúmulo entre períodos)
- [ ] Sem permissão de input: app opera em **modo degradado** (heartbeat + threshold de inatividade), sem crash
- [ ] Sem permissão: inatividade dispara após ~2min sem movimento de mouse
- [ ] Não existe "modo simulado" silencioso — o estado de permissão é visível na UI (permission banner)

---

## 5. Captura de foco (apps e sites)

- [ ] A cada 15s, a janela ativa é capturada (app + título)
- [ ] Trocar de app fecha o `tracking_app` anterior e abre um novo
- [ ] Trocar de site no browser fecha o `tracking_site` anterior e abre um novo
- [ ] Apps/sites têm `started_at`/`ended_at` coerentes (sem sobreposição)
- [ ] Apps de comunicação (Zoom, Teams, Slack) marcam `meeting_exempt` e suspendem a inatividade
- [ ] Sem permissão de captura de janela ativa: comportamento degradado sem crash

---

## 6. Screenshots

- [ ] Screenshot capturada no intervalo configurado (padrão ~300s; `SCREENSHOT_INTERVAL_SECS` em dev, mín. 10s)
- [ ] Captura cobre **todos os monitores** (stitch) via `xcap`
- [ ] Arquivo WebP é gravado em disco e INSERT no SQLite
- [ ] Pipeline completo dispara junto: drain do bucket → score → captura → persistência → peripheral events → enqueue
- [ ] Durante `PausedInactivity`: screenshots continuam, marcadas `time_category = 'inactivity'`
- [ ] Durante pausa manual: screenshots são puladas
- [ ] Falha de captura (permissão de tela negada) não derruba o tracking — erro tratado e logado

---

## 7. Inatividade

### 7.1 Máquina de estados

- [ ] Transição `Active → Warning` após threshold de inatividade
- [ ] `Warning → Countdown` (60s) com overlay visível
- [ ] `Countdown → PausedInactivity` ao fim da contagem sem input
- [ ] Input do usuário durante Warning/Countdown cancela e volta para `Active`
- [ ] Timer de tracking **pausa** em `PausedInactivity`

### 7.2 Overlay e prompts

- [ ] Overlay de inatividade aparece nas fases corretas (Warning/Countdown/PausedInactivity/ResumePrompt/ManualWorkCheck)
- [ ] `confirm_still_working` no Countdown mantém o tracking ativo
- [ ] Ao detectar input após `PausedInactivity`: transição para `ResumePrompt` pedindo classificação
- [ ] `classify_tracking_inactivity_period` registra a classificação do período
- [ ] `classify_paused_inactivity_period` classifica período pausado por inatividade
- [ ] `skip_tracking_inactivity_classification` pula a classificação sem travar o estado
- [ ] `dismiss_inactivity_period` descarta o período corretamente
- [ ] `get_tracking_inactivity_config` retorna a configuração vigente

### 7.3 Work check manual

- [ ] `confirm_manual_work` confirma trabalho durante `ManualWorkCheck`
- [ ] `dismiss_manual_work_check` trata a dispensa corretamente

### 7.4 Exceções

- [ ] App de comunicação em foco (`meeting_exempt`) suspende a detecção de inatividade
- [ ] Períodos de inatividade ficam **somente locais** (`tracking_inactivity_period`, sem sync)

---

## 8. Buffer de atividade

- [ ] Após login (sem tracking), o buffer acumula o primeiro minuto de atividade
- [ ] Buffer persiste no SQLite a cada segundo
- [ ] Ao iniciar o tracking, o buffer é "claimado" como primeiro período de atividade
- [ ] Sem iniciar tracking em 1 minuto, o buffer é descartado
- [ ] `dismiss_activity_buffer` descarta o buffer manualmente (buffer alert na UI)
- [ ] Buffer sobrevive a restart do app (com sessão auth válida)
- [ ] Buffer alert aparece na UI quando há buffer pendente e some após ação do usuário

---

## 9. Sync (offline-first)

### 9.1 Outbox

- [ ] Todas as entidades gravam primeiro no SQLite e enfileiram na `sync_queue`
- [ ] Status transitam corretamente: `pending → sending → confirmed`
- [ ] Worker processa até 10 itens por lote (5s fila vazia / 2s após lote)

### 9.2 Entidades sincronizadas

- [ ] `tracking` → POST no start, PATCH no stop
- [ ] `tracking_screenshot` → POST de metadados após upload S3
- [ ] `tracking_peripheral_event` → POST com contagens de mouse/teclado
- [ ] `tracking_app` → POST quando o app é fechado
- [ ] `tracking_site` → POST quando o site é fechado
- [ ] `tracking_inactivity_period` → **nunca** sincroniza (local only)

### 9.3 Retry e recuperação

- [ ] Erro transitório (5xx/rede): retry exponencial 2s→4s→…→cap 3600s, máx. 8 tentativas
- [ ] Após 8 tentativas: item vai para dead-letter
- [ ] Erro 4xx (exceto 401/403): dead-letter imediato (terminal)
- [ ] Erro 401: emite `auth-session-expired` e para o worker
- [ ] App offline durante tracking: tudo persiste local e sincroniza ao reconectar (sem duplicar)
- [ ] IDs gerados no desktop são preservados pelo backend (idempotência)

### 9.4 Screenshots — S3 + metadados

- [ ] Upload direto do WebP para S3/Garage com chave `{screenshot_id}.{ext}`
- [ ] Path remoto correto: `screenshots/{screenshot_id}.{ext}`
- [ ] Metadados enviados à API com `{ id, original_id, captured_at, path }`
- [ ] `path` retornado pela API é armazenado em `path` e `remote_path`
- [ ] Após sync bem-sucedido, o WebP local é apagado
- [ ] Falha no upload S3 mantém arquivo local e re-tenta depois

---

## 10. Dashboard e histórico (UI)

- [ ] `get_dashboard_summary` retorna totais corretos do dia/período
- [ ] `get_activity_chart` retorna dados do gráfico de atividade coerentes com os trackings
- [ ] `list_trackings` lista sessões com filtros/paginação funcionando
- [ ] `list_tracking_screenshots` lista screenshots da sessão
- [ ] `get_tracking_screenshot_image` exibe a imagem corretamente
- [ ] `list_tracking_apps` e `list_tracking_sites` exibem apps/sites da sessão
- [ ] `list_tracking_peripheral_events` exibe eventos de atividade da sessão
- [ ] `list_tracking_inactivity_periods` exibe períodos de inatividade locais
- [ ] `list_sync_queue` exibe a fila de sync (estados pending/failed/confirmed visíveis)
- [ ] Dados na UI batem com o SQLite local e, após sync, com o painel web

---

## 11. Interface e experiência

- [ ] `timer-app`: layout correto nos estados sem tracking / ativo / pausado
- [ ] `workspace-view`: navegação entre seções funciona
- [ ] `profile-menu`: exibe dados do usuário e ações (logout, painel web)
- [ ] `open_web_panel` abre `FRONTEND_URL` no browser
- [ ] `open_external_url` respeita o guard de links externos (`external-link-guard`)
- [ ] `settings-view`: configurações carregam e salvam (`get_setting` / `set_setting`)
- [ ] `open_data_directory` abre a pasta de dados locais no gerenciador de arquivos
- [ ] `get_app_version` exibe a versão correta do build
- [ ] `get_app_status` retorna estado geral coerente
- [ ] Tema claro/escuro (`theme-toggle`) aplica e persiste
- [ ] Troca de idioma (`language-toggle`) traduz toda a UI (sem chaves faltando)
- [ ] Estados vazios e de erro têm mensagens adequadas (sem telas quebradas)

---

## 12. Tray (bandeja do sistema)

- [ ] Ícone do tray aparece na área de notificação
- [ ] Menu do tray exibe ações corretas conforme o estado (tracking ativo/inativo)
- [ ] Labels do tray seguem o idioma selecionado
- [ ] Clique esquerdo no tray abre/foca a janela principal (`open_main_window`)
- [ ] Fechar a janela não mata o app (continua no tray)
- [ ] Quit pelo tray finaliza tracking ativo corretamente (ver 3.5)

---

## 13. Mini timer widget

- [ ] Mini widget exibe o tempo da sessão em formato compacto
- [ ] `begin_mini_widget_drag` permite arrastar o widget
- [ ] Posição do widget persiste entre sessões
- [ ] `reset_mini_widget_position` restaura a posição padrão
- [ ] Ações do widget (pause/resume/stop) refletem no estado global e na janela principal

---

## 14. Permissões e plataforma

- [ ] `check_input_monitoring_permission` detecta corretamente a permissão de input
- [ ] `check_active_window_permission` detecta permissão de janela ativa
- [ ] `open_system_settings_input_monitoring` abre as configurações do SO no painel correto
- [ ] `open_system_settings_screen_recording` abre as configurações de gravação de tela
- [ ] `get_tracking_capabilities` reflete as capacidades reais da plataforma
- [ ] `get_platform_info` retorna SO/versão corretos
- [ ] Permission banner aparece quando falta permissão e some após concessão
- [ ] Sem permissão de input: modo degradado (heartbeat) sem falso "tracking ativo"

### 14.1 Matriz de plataforma

| Cenário | 🐧 Linux | 🪟 Windows | 🍎 macOS |
|---------|:-------:|:---------:|:--------:|
| Login + sessão | [ ] | [ ] | [ ] |
| Tracking completo (start → stop) | [ ] | [ ] | [ ] |
| Captura de atividade real | [ ] | [ ] | [ ] |
| Captura de janela ativa | [ ] | [ ] | [ ] |
| Screenshot multi-monitor | [ ] | [ ] | [ ] |
| Overlay de inatividade | [ ] | [ ] | [ ] |
| Tray + menu | [ ] | [ ] | [ ] |
| Modo degradado sem permissões | [ ] | [ ] | [ ] |
| Sync offline → online | [ ] | [ ] | [ ] |

---

## 15. Dados locais e schema

- [ ] SQLite abre em modo WAL sem corrupção após kill do processo
- [ ] Migrations são aditivas e rodam limpas em banco existente (upgrade de versão)
- [ ] Banco novo (primeiro uso) é criado com schema completo
- [ ] Nenhum dado de `idle_period`/`tracking_inactivity_period` vaza para a API
- [ ] `~/.local/share/voowork-desktop/` contém apenas DB + screenshots pendentes de sync
- [ ] Screenshots sincronizadas são removidas do disco local

---

## 16. Robustez

- [ ] Kill forçado com tracking ativo → no próximo boot o tracking órfão é finalizado
- [ ] Queda de rede no meio do sync → fila retoma automaticamente
- [ ] API lenta/timeout → UI não congela (chamadas assíncronas)
- [ ] DB bloqueado/cheio → erro tratado com mensagem, sem panic
- [ ] Relógio do sistema alterado durante tracking → comportamento definido e sem crash
- [ ] Suspensão/hibernação do SO durante tracking → retomada coerente

---

## 17. Smoke pós-release (rápido — 10 min)

Para validações rápidas após hotfix, execute no mínimo:

1. [ ] Login com usuário válido
2. [ ] Iniciar tracking em um projeto/tarefa
3. [ ] Aguardar 1 ciclo de screenshot (ou usar `SCREENSHOT_INTERVAL_SECS=10`)
4. [ ] Verificar atividade + app/site + screenshot no histórico local
5. [ ] Pausar e retomar
6. [ ] Forçar inatividade (ou simular ausência de input) e confirmar overlay
7. [ ] Parar o tracking e conferir PATCH na API
8. [ ] Verificar sync completo no painel web (tempo, screenshot, atividade)
9. [ ] Logout

---

## Referências

- [Visão geral do produto](README.md)
- [Feature: Autenticação](features/01-authentication.md)
- [Feature: Tracking](features/02-tracking.md)
- [Feature: Sync](features/03-sync.md)
- [Schema do banco local](db.mermaid)
