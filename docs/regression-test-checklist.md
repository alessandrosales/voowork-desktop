# Checklist de Testes Regressivos — Voowork Desktop

Checklist completo para validação regressiva de todas as funcionalidades do agente desktop antes de releases ou após mudanças significativas.

**Como usar:** execute os itens na ordem das seções. Marque `[x]` apenas quando o comportamento observado for idêntico ao esperado. Registre falhas com passos de reprodução, versão do app e plataforma.

**Legenda de ambiente:** 🐧 Linux · 🪟 Windows · 🍎 macOS (marque apenas as plataformas suportadas pela release)

**Foco de regressão recente (2026-07):** sessão expirada no boot sem congelar a UI; comandos IPC e ações do tray fora da main thread; ordenação do outbox (screenshot antes de peripheral events); mini-timer sem botão encerrar e com resize programático no GTK.

---



## 0. Preparação do ambiente

- [x] Backend Rails rodando e acessível em `VITE_API_URL`
- [x] S3/Garage configurado (`S3_ENDPOINT`, `S3_REGION`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`, `S3_BUCKET`)
- [x] Usuário de teste com pelo menos 1 projeto e 1 tarefa atribuídos
- [x] Banco local em estado conhecido (`~/.local/share/voowork-desktop/voowork-desktop.db`)
- [x] Verificações estáticas passando:
  - [x] `npm run typecheck`
  - [x] `npm run lint`
  - [x] `npm test` (vitest)
  - [x] `cargo check --manifest-path src-tauri/Cargo.toml`
  - [x] `cargo clippy --manifest-path src-tauri/Cargo.toml -D warnings`
  - [x] `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] Build de produção funciona: `npm run build` e `npm run tauri build`

---



## 1. Autenticação



### 1.1 Login

- [x] Login com credenciais válidas autentica e abre a tela principal
- [x] Token e perfil são persistidos no SQLite (`settings`)
- [x] Token é armazenado no keyring do SO
- [x] Cache de projetos é sincronizado automaticamente após o login
- [x] Login com senha incorreta exibe **apenas** a mensagem da API (ex.: "E-mail ou senha inválidos"), sem prefixos técnicos
- [x] Login com e-mail inexistente exibe mensagem de erro amigável
- [x] Login com API fora do ar exibe erro de conexão sem quebrar a UI
- [x] Campos vazios / formato de e-mail inválido são validados antes do submit
- [x] Estado de loading no botão durante a autenticação (sem duplo submit)



### 1.2 Sessão

- [ ] Fechar e reabrir o app com sessão válida → entra direto na tela principal (sem novo login)
- [ ] No boot, `validate_auth_session` chama `GET /api/v1/auth/me` e confirma o token
- [ ] Token expirado/inválido no boot → redireciona para login **sem congelar a UI** (sem diálogo "forçar saída" do GNOME)
- [ ] Token expirado no boot com tracking órfão no SQLite → app finaliza o tracking e limpa a sessão sem deadlock
- [ ] API retornando 401 durante sync → evento `auth-session-expired` dispara logout automático na UI
- [ ] `get_auth_state` retorna o estado correto sem chamada de rede e **não bloqueia** a main thread durante o lock do SQLite



### 1.3 Logout

- [ ] Logout limpa a sessão local (SQLite + keyring)
- [ ] Logout com tracking ativo: tray e profile-menu param a sessão antes de limpar auth (com confirmação na UI principal)
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



### 3.4 Parar (encerrar sessão de tracking)

> **UI:** não há botão "Encerrar" na janela principal nem no mini widget. O encerramento da sessão é feito pelo menu da bandeja (**⏹ Encerrar**).

- [ ] Item **⏹ Encerrar** no tray chama `stop_tracking` e finaliza a sessão
- [ ] Ação do tray **não congela** a janela principal nem o mini widget durante o finalize (roda fora da main thread)
- [ ] Screenshot final é capturada com UUID real (fail-loud, sem placeholder)
- [ ] Peripheral events do período final são enfileirados **depois** do screenshot na `sync_queue`
- [ ] Apps/sites abertos são fechados (com `ended_at`)
- [ ] `PATCH /api/v1/trackings/:id` é enviado com `status: inactive` e `ended_at`
- [ ] UI volta ao estado "sem tracking ativo" (timer principal, mini widget e tray)



### 3.5 Sair do app vs encerrar sessão


| Ação no tray   | Efeito                                                                   |
| -------------- | ------------------------------------------------------------------------ |
| **⏹ Encerrar** | Para o tracking ativo; o app **continua** rodando na bandeja             |
| **Sair**       | Screenshot final (se houver sessão) + flush do sync + encerra o processo |


- [ ] **Sair** com tracking ativo: `capture_final_screenshot_and_finalize` roda antes do `_exit`
- [ ] **Sair**: worker de sync é parado, `flush_blocking` envia itens pendentes, depois o processo termina
- [ ] **Sair** não exibe diálogo "forçar saída" por UI congelada (finalize fora da main thread)
- [ ] Fechar a janela principal **não** encerra o processo (app permanece no tray)
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
- [ ] Pipeline completo dispara junto: drain do bucket → score → captura → persistência → **enqueue screenshot** → peripheral events → enqueue PE
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
- [ ] **Ordem:** para cada ciclo de screenshot, o item `tracking_screenshot` entra na fila **antes** dos `tracking_peripheral_event` do mesmo período (evita 422 no backend)
- [ ] Empates de `created_at` na fila são desempatados por `rowid` (ordem estável de inserção)



### 9.2 Entidades sincronizadas

- [ ] `tracking` → POST no start, PATCH no stop
- [ ] `tracking_screenshot` → POST de metadados após upload S3
- [ ] `tracking_peripheral_event` → POST com contagens de mouse/teclado
- [ ] `tracking_app` → POST quando o app é fechado
- [ ] `tracking_site` → POST quando o site é fechado
- [ ] `tracking_inactivity_period` → **nunca** sincroniza (local only)



### 9.3 Retry e recuperação

- [ ] Erro transitório (5xx/rede): retry exponencial 2s→4s→…→cap 3600s, máx. 8 tentativas
- [ ] PE rejeitado com 422 por screenshot ainda não sincronizado → retry transitório (não vai para dead-letter imediato)
- [ ] Após 8 tentativas: item vai para dead-letter
- [ ] Erro 4xx (exceto 401/403 e o 422 transitório de screenshot): dead-letter imediato (terminal)
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
- [ ] Relatórios webapp (`project_time`, timeline) batem com API após deduplicação — ver [alignment/tracking-data-alignment.md](alignment/tracking-data-alignment.md) §5
- [ ] Bloco `is_live: true` na timeline corresponde ao único tracking `active` do usuário no PG
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

- [ ] `timer-app`: layout correto nos estados sem tracking / ativo / pausado (**sem** botão encerrar no header)
- [ ] Controles de start/pause/resume na janela principal funcionam; encerrar sessão só pelo tray
- [ ] `workspace-view`: navegação entre seções funciona
- [ ] `profile-menu`: exibe dados do usuário e ações (logout com confirmação se tracking ativo, painel web)
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
- [ ] Labels do tray seguem o idioma selecionado (pt-BR / en / es)
- [ ] Clique esquerdo no tray abre/foca a janela principal (`open_main_window`)
- [ ] Fechar a janela não mata o app (continua no tray)
- [ ] **▶ Iniciar / ⏸ Pausar / ▶ Retomar** no tray alternam o tracking sem congelar a UI
- [ ] Com overlay de inatividade ativo, toggle no tray abre a janela principal em vez de pausar
- [ ] **⏹ Encerrar** para a sessão sem fechar o app (ver 3.4)
- [ ] **Sair** finaliza tracking + flush do sync + encerra o processo (ver 3.5)
- [ ] **Fazer logout** no tray para tracking ativo, limpa sessão e abre a janela principal
- [ ] **Reposicionar timer** restaura posição padrão do mini widget
- [ ] Refresh do tray (tooltip/labels) continua responsivo durante finalize longo em background

---



## 13. Mini timer widget



### 13.1 Conteúdo e ações

- [ ] Widget exibe o tempo da sessão em formato `HH:MM:SS` (tabular)
- [ ] **Não** há botão encerrar no widget — apenas play/pause e arrastar
- [ ] Botão play/pause: inicia última seleção, pausa, retoma ou abre o app (fases de inatividade)
- [ ] Duplo clique no tempo abre a janela principal
- [ ] Ações do widget refletem no estado global e na janela principal (e vice-versa)



### 13.2 Arrastar e posição

- [ ] `begin_mini_widget_drag` no handle (ícone ⋮⋮) arrasta o widget pela área de notificação
- [ ] Arrastar pelo display do tempo também funciona (com threshold para não confundir com clique)
- [ ] **Um único** handle de drag visível — sem ícone fantasma ou região de drag duplicada
- [ ] Após pausar e retomar, o handle de drag permanece alinhado ao pill (não “flutua” fora do componente)
- [ ] Posição do widget persiste entre sessões
- [ ] `reset_mini_widget_position` / item do tray restaura a posição padrão



### 13.3 Tamanho da janela (🐧 GTK)

- [ ] Janela do mini-timer acompanha o tamanho do pill (`ResizeObserver` + `setSize`)
- [ ] Usuário **não** consegue redimensionar manualmente a janela (min/max travados após sync)
- [ ] Não há “janela fantasma” maior que o conteúdo visível
- [ ] Transição play ↔ pause redimensiona o pill sem artefatos visuais

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


| Cenário                                | 🐧 Linux | 🪟 Windows | 🍎 macOS |
| -------------------------------------- | -------- | ---------- | -------- |
| Login + sessão                         | [ ]      | [ ]        | [ ]      |
| Tracking completo (start → stop)       | [ ]      | [ ]        | [ ]      |
| Captura de atividade real              | [ ]      | [ ]        | [ ]      |
| Captura de janela ativa                | [ ]      | [ ]        | [ ]      |
| Screenshot multi-monitor               | [ ]      | [ ]        | [ ]      |
| Overlay de inatividade                 | [ ]      | [ ]        | [ ]      |
| Tray + menu (toggle / encerrar / sair) | [ ]      | [ ]        | [ ]      |
| Mini timer (drag, resize, pause)       | [ ]      | [ ]        | [ ]      |
| Modo degradado sem permissões          | [ ]      | [ ]        | [ ]      |
| Sync offline → online                  | [ ]      | [ ]        | [ ]      |


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
- [ ] API lenta/timeout → UI não congela (`spawn_blocking` nos comandos IPC que seguram o SQLite)
- [ ] Sessão revogada com tracking ativo → callback de auth não bloqueia `status()` nem o tray refresh
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
5. [ ] Pausar e retomar (janela principal e mini widget)
6. [ ] Arrastar o mini widget e confirmar tamanho/posição corretos após pause
7. [ ] Forçar inatividade (ou simular ausência de input) e confirmar overlay
8. [ ] **⏹ Encerrar** pelo tray e conferir PATCH na API
9. [ ] Verificar sync completo no painel web (tempo, screenshot, atividade)
10. [ ] Logout



### 17.1 Smoke de regressão crítica (boot + auth)

1. [ ] Com token expirado no keyring/SQLite, abrir o app → login sem UI congelada
2. [ ] Com tracking ativo, revogar sessão no backend → logout automático sem deadlock
3. [ ] **Sair** pelo tray com tracking ativo → screenshot final aparece no painel após sync

---



## Referências

- [Visão geral do produto](README.md)
- [Feature: Autenticação](features/01-authentication.md)
- [Feature: Tracking](features/02-tracking.md)
- [Feature: Sync](features/03-sync.md)
- [Alinhamento tracking ↔ API ↔ webapp](alignment/tracking-data-alignment.md)
- [Schema do banco local](db.mermaid)

