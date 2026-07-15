# Autenticação

Login JWT contra a API Rails. Sessão local no SQLite; frontend nunca chama HTTP diretamente.

## Fluxo

1. Usuário preenche e-mail/senha em `compact-login.tsx`.
2. React chama `invoke("login", { request: { email, password } })`.
3. Rust (`auth/commands.rs`) → `POST /api/v1/auth/login`.
4. Token e perfil persistidos em `settings` via `auth/store.rs`.
5. Cache de projetos sincronizado após login.
6. No boot, `validate_auth_session` chama `GET /api/v1/auth/me`.

## Commands Tauri

| Command | Descrição |
|---------|-----------|
| `login` | Autentica e persiste sessão |
| `logout` | Limpa sessão local |
| `get_auth_state` | Estado atual (sem rede) |
| `validate_auth_session` | Valida token com a API |

## Mensagens de erro na UI

O usuário vê **apenas a mensagem da API**, sem prefixos técnicos.

```
API 401 → "E-mail ou senha inválidos"
```

**Não** exibir: `auth error: E-mail ou senha inválidos`

### Implementação

1. `auth/http_errors.rs` — `error_message_from_body` lê `errors`, `error` ou `message` do JSON da API.
2. `AgentError::Auth(String)` em `error.rs` — `#[error("{0}")]` (sem prefixo).
3. Tauri serializa o erro como string para o frontend.
4. `use-auth.tsx` — `setError(err.message)`.

Credenciais inválidas: API pode retornar 401 sem body; o fallback em `raw_body_message` usa `E-mail ou senha inválidos`.

## Sessão expirada durante sync

O `SyncWorker` emite o evento Tauri `auth-session-expired`. O `AuthProvider` escuta e faz logout automático na UI.

## Variáveis

| Variável | Uso |
|----------|-----|
| `VOOWORK_API_URL` | Base da API para login e validação |

Ver também: [BACKEND_INTEGRATION.md](../BACKEND_INTEGRATION.md).
