# Smoke test manual — voowork-desktop

Checklist rápido antes de release ou após mudanças em auth/tracking/sync.

## Pré-requisitos

```bash
# Terminal 1 — backend
cd voowork-backend && bin/rails server

# Terminal 2 — desktop
cd voowork-desktop && cp .env.example .env && npm run tauri dev
```

## Fluxo

| # | Passo | Esperado |
|---|-------|----------|
| 1 | Abrir app | Tela de login (sem credenciais pré-preenchidas) |
| 2 | Login inválido | Erro no formulário; botão com loading; **sem** reload da tela |
| 3 | Login válido | Transição para timer; sessão persistida |
| 4 | Reiniciar app | Sessão restaurada (token no keyring) |
| 5 | Selecionar projeto/tarefa e iniciar tracking | Timer ativo; tray atualizado |
| 6 | Pausar / retomar | Estado correto na UI |
| 7 | Parar tracking | Sync enfileira PATCH; worker processa |
| 8 | Link "Abrir painel web" | Abre URL permitida (`VITE_VOOWORK_WEB_URL` ou produção) |
| 9 | Logout | Volta ao login; token removido do keyring |
| 10 | Inatividade (opcional) | Overlay aparece após threshold configurado |

## Verificações técnicas

```bash
npm run typecheck
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml navigation::tests
```

## Dados locais

```
~/.local/share/voowork-desktop/
├── voowork-desktop.db
└── screenshots/
```
