# 08 — Integridade e segurança

| Campo | Valor |
|-------|-------|
| **Status** | `real` |
| **Prioridade** | `P0` |

## Visão geral

Camada invisível na UI que protege a integridade dos dados capturados pelo agente. Roda inteiramente no core Rust — o colaborador não vê alertas nem painéis de fraude; os dados chegam à API com evidências de autenticidade.

## Proteções implementadas

| Proteção | Módulo | Descrição |
|----------|--------|-----------|
| Hash chain | `integrity/hash_chain.rs` | Encadeamento de hashes em sessions e activity_ticks |
| Clock skew | `clock/mod.rs` | Detecta manipulação de relógio do sistema |
| Automação | `activity/automation.rs` | Sinaliza mouse jigglers e padrões artificiais |
| SHA-256 screenshots | `screenshot/mod.rs` | Hash na captura, antes de qualquer processamento |
| Assinatura Ed25519 | `crypto/mod.rs` | Cada payload de sync assinado pelo dispositivo |
| Validação pré-sync | `sync/mod.rs` | Cadeia quebrada bloqueia envio |

## Hash chain

```
genesis → sessão₁ → tick₁ → tick₂ → tick₃ → ...
```

Cada registro inclui `prev_hash` e `record_hash`. Edição direta no SQLite quebra a cadeia → sessão marcada `suspicious`.

## Clock skew

A cada tick, compara delta de `Instant` (monotônico) vs `SystemTime` (wall-clock). Divergência grande gera flag em sessão e tick — indica possível alteração manual da hora.

## Detecção de automação

Sinais em `activity/automation.rs`:

- Intervalos perfeitamente regulares entre eventos
- Posições idênticas repetidas
- Variância muito baixa nos deltas de tempo

Resultado: `activity_score_confidence` (0.1–1.0) e `automation_flags`. **v1 sinaliza, não bloqueia** o tracking.

## Screenshot integrity

- SHA-256 calculado no momento da captura.
- Hash incluído no payload de sync.
- Correlação com `activity_tick_id` do mesmo intervalo.

## Fila de sync append-only

A tabela `sync_queue` não reescreve payloads após envio — apenas atualiza status de confirmação.

## Proteção do banco local

| Medida | Status |
|--------|--------|
| Diretório protegido do SO (`~/.local/share/voowork-agent/`) | ✅ |
| Criptografia em repouso (SQLCipher) | ⏳ Planejado |
| Chave privada Ed25519 nunca sai do dispositivo | ✅ |
| Keychain do SO para chave privada | ⏳ Planejado |

## Arquivos principais

| Módulo | Arquivo |
|--------|---------|
| Hash chain | `src-tauri/src/integrity/hash_chain.rs` |
| Clock | `src-tauri/src/clock/mod.rs` |
| Automação | `src-tauri/src/activity/automation.rs` |
| Cripto | `src-tauri/src/crypto/mod.rs` |
| Sync | `src-tauri/src/sync/mod.rs` |

## Comportamento esperado (alvo)

- [ ] SQLCipher no banco local
- [ ] Chave privada no keychain do SO
- [ ] Validação server-side da hash chain na API
- [ ] Bloqueio opcional de sync para sessões `suspicious`

## Relacionado

- [04-activity-monitoring.md](./04-activity-monitoring.md)
- [05-screenshots.md](./05-screenshots.md)
- [06-sync-and-offline.md](./06-sync-and-offline.md)
- [07-device-registration.md](./07-device-registration.md)
