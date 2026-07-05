# Forge — Plataforma Unificada (Rust + Python)

**mix_btv_code** é o repositório-sede da plataforma **Forge**: um CLI/TUI de coding
agent que unifica as ideias de três repositórios num único sistema construído por
design em Rust e Python.

| Origem | O que traz |
|---|---|
| [opencode](https://github.com/danzeroum/opencode) | Runtime de sessão durável, agentes selecionáveis, permissões, ferramentas, TUI, **ModelTier** e **verificação determinística** (fork) |
| [prompte](https://github.com/danzeroum/prompte) | Geradores de prompt, knowledge base, quality linter, cache por hash, gateway LLM com fallback, telemetria |
| [BuildToValue](https://github.com/danzeroum/BuildToValue_AI_Agent_Specialization) | Squad multi-agente: orquestração, consenso ponderado, planejamento, HITL, fallback progressivo, ledger, review por valor |

- **Plano completo** (arquitetura, mapeamento 100%, roadmap 6 fases): [`docs/PLANO-PLATAFORMA-FORGE.md`](docs/PLANO-PLATAFORMA-FORGE.md)
- **Roadmap visual interativo**: [`docs/roadmap-forge.html`](docs/roadmap-forge.html) (autocontido — abra no navegador)
- **Decisão arquitetural central**: [`docs/adr/0001-arquitetura-rust-python-grpc.md`](docs/adr/0001-arquitetura-rust-python-grpc.md)
- **Histórico de decisões da junção**: [`docs/DECISOES.md`](docs/DECISOES.md)

## Layout

- `crates/` — núcleo Rust: `forge-cli` (binário `forge`), `forge-core`
  (sessões/permissões), `forge-llm` (gateway + ModelTier), `forge-tools`,
  `forge-verify`, `forge-store` (SQLite/ledger), `forge-schemas`,
  `forge-tui`/`forge-server`/`forge-proto` (fases 2–3).
- `python/` — sidecar de orquestração (uv workspace): `forge-squad`
  (consenso/planejamento/HITL), `forge-promptforge` (geradores/linter/hash),
  `forge-review`, `forge-eval`, `forge-proto-py`.
- `schemas/` — fonte única de contratos: protos gRPC, JSON Schemas versionados
  (`*.v1.schema.json`) e fixtures de paridade cross-language.

## Desenvolvimento

```sh
just test      # cargo test + pytest
just lint      # clippy + rustfmt
just verify    # test + lint (evidência JSON completa na Fase 5)
```

Sem `just`: `cargo test --workspace` e `cd python && uv sync && uv run pytest`.

## Estado

**Fase 1 concluída**: `forge run` (tarefa única) e `forge chat` (REPL
multi-turno) executam o loop de agente real — gateway LLM com streaming SSE
(Anthropic/OpenAI/DeepSeek, fallback automático, keys por env), cache de
prompts por hash (`prompt-cache-key.v1`, desative com `--no-cache`),
ferramentas read/grep/edit/bash sob permissão interativa e cada turno
registrado no ledger append-only (`.forge/forge.db`).

```sh
export ANTHROPIC_API_KEY=...   # ou DEEPSEEK_API_KEY / OPENAI_API_KEY
cargo run -p forge-cli -- run "corrija o teste X" --model claude-sonnet-5
cargo run -p forge-cli -- chat
```

Próxima: **Fase 2** — sessões duráveis (System Context/Epochs/compaction),
TUI ratatui e tier-gating completo do ModelTier.
