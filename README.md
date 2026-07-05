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

Scaffold da **Fase 1** do roadmap: workspaces compilando, contratos iniciais
definidos e testados (incl. paridade de hash Rust×Python), ledger append-only com
hash-chain funcionando, ModelTier portado. Próximo marco: loop de agente real no
`forge run` (providers HTTP com streaming no gateway).
