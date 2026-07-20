# Local Config Isolation (CRITICAL)

Este repositório tem configuração local de agentes (`opencode.jsonc`, `AGENTS.md`, `.opencode/`, `.cursor/`, `.agents/skills/`). Ela é a **única** fonte de verdade para agents, rules e skills.

1. **Nunca** use agents, rules ou skills globais (`~/.config/opencode/`, `~/.cursor/rules/`, `~/.agents/`). A config do repo é autoritativa.
2. **Nunca** trate config global como fallback ou complemento da config local.
3. **Use** apenas o que vive em `.opencode/`, `.cursor/`, `.agents/skills/` e `AGENTS.md`.
4. Se um recurso necessário não existir no repo, **crie-o localmente** — não importe de `~/`.
5. Skills baixadas via `skills.sh` vivem em `.agents/skills/` (lidas por OpenCode via `skills.paths` e por Cursor via auto-descoberta). Skills escritas à mão vão em `.opencode/skills/` e são espelhadas em `.cursor/skills/`.
