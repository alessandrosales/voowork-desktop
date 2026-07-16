# Autenticação

Login JWT contra a API Rails. Sessão persistida em SQLite; frontend nunca chama HTTP.

## Fluxo

1. Usuário preenche e-mail/senha no formulário de login.
2. React chama `invoke("login", { request: { email, password } })`.
3. Rust (`auth/commands.rs`) → `POST /api/v1/auth/login`.
4. Token e perfil salvos em `settings` no SQLite.
5. Cache de projetos sincronizado após login.
6. No boot, `validate_auth_session` → `GET /api/v1/auth/me`.

## Commands Tauri

| Command | Descrição |
|---------|-----------|
| `login` | Autentica e persiste sessão |
| `logout` | Limpa sessão local |
| `get_auth_state` | Estado atual (sem rede) |
| `validate_auth_session` | Valida token com a API |

## Sessão expirada

O `SyncWorker` emite o evento `auth-session-expired` se a API retornar 401. O frontend escuta e faz logout automático.

## Mensagens de erro na UI

O usuário vê apenas a mensagem da API, sem prefixos técnicos:

```
API 401 → UI mostra "E-mail ou senha inválidos"
```

Implementação: `auth/http_errors.rs` extrai o erro do JSON; `AgentError::Auth` usa `#[error("{0}")]` (sem prefixo).

## Código

| Arquivo | Função |
|---------|--------|
| `auth/client.rs` | HTTP client (login, fetch_me) |
| `auth/commands.rs` | Commands Tauri |
| `auth/store.rs` | Persistência em SQLite + keyring |
| `auth/http_errors.rs` | Parsing de erros da API |
| `auth/token_store.rs` | Token no keyring do SO |
