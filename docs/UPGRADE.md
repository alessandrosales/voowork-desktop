# Upgrade do SQLite local

## Migração `sessions` / `screenshots` legadas

A partir da refatoração **session → tracking**, o agente remove tabelas legadas na migration 3:

- `sessions`
- `screenshots`
- `app_focus_events`
- `activity_ticks`
- `project_cache`

### Impacto

| Cenário | Resultado |
|---------|-----------|
| Instalação nova | Sem impacto |
| Upgrade de build antigo com dados em `sessions` | **Dados legados não são migrados** — trackings/screenshots antigos são descartados |
| Upgrade com DB já no schema `trackings` | Sem impacto |

### Recomendação

Antes de atualizar em produção:

1. Encerre trackings ativos no app antigo (se ainda existir).
2. Aguarde sync concluir (`sync_queue` vazia ou sem itens `pending`).
3. Opcional: copie `~/.local/share/voowork-desktop/voowork-desktop.db` como backup.

### JWT e credenciais

- O **access token** passa a ficar no **credential store do SO** (keyring), não mais em plaintext na tabela `settings`.
- Na primeira execução após o upgrade, tokens legados em `settings.auth_access_token` são migrados automaticamente e apagados do SQLite.
