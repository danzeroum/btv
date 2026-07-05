# Forge (mix_btv_code)

CLI/TUI de coding agent unificando opencode + prompte + BuildToValue.
Núcleo **Rust** (workspace cargo em `crates/`) + sidecar **Python** (workspace uv em
`python/`), integrados por gRPC sobre Unix Domain Socket (ativação na Fase 3).

## Comandos

```sh
cargo test --workspace                 # testes Rust (inclui paridade de hash)
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
cd python && uv sync && uv run pytest  # testes Python
just test | just lint | just verify    # atalhos (requer just)
```

## Regra de fronteira (ADR 0001 — docs/adr/)

- **Rust**: tudo que toca disco/rede/processo/segredo ou roda a cada keystroke
  (CLI/TUI, sessões, gateway LLM, ferramentas, permissões, verify, storage).
  API keys existem SÓ no processo Rust.
- **Python**: tudo que decide o próximo passo por raciocínio de agente
  (squad, PromptForge, review, eval). Python NUNCA chama provedores LLM
  diretamente — sempre via `CoreService.Generate` (gRPC).
- Sem PyO3 no caminho principal (tokio×asyncio); sidecar supervisado com
  fallback progressivo: squad → agente-único → safe-mode read-only.

## Regras de contrato

- Fonte única em `schemas/` (protos gRPC + `*.v1.schema.json` + fixtures).
- Mudança breaking = novo arquivo `.v2` + ADR novo; protos evoluem só aditivamente.
- O hash de cache de prompt (`prompt-cache-key.v1`) tem implementação dupla:
  `crates/forge-schemas/src/canonical.rs` (Rust) × 
  `python/packages/forge-promptforge/src/forge_promptforge/hashing.py` (Python).
  Qualquer mudança exige regenerar `schemas/fixtures/` (`scripts/gen_fixtures.py`)
  e os testes de paridade dos DOIS lados devem passar.
- Ledger é append-only com hash-chain (`crates/forge-store/src/ledger.rs`) —
  nunca UPDATE/DELETE; overrides são novas entradas marcadas.

## Roadmap e estado

Plano completo em `docs/PLANO-PLATAFORMA-FORGE.md` (6 fases). Estado atual:
scaffold da Fase 1 (contratos, ModelTier, permissões, ledger, /verify mínimo,
consenso ponderado). Próximo marco da Fase 1: loop de agente real no `forge run` —
providers HTTP (Anthropic/OpenAI/DeepSeek) com streaming SSE no `forge-llm`,
ferramentas read/grep/edit/bash reais e sessão persistida.

## Convenções

- Código e comentários em português (padrão do projeto); identificadores em inglês.
- Testes unitários junto do módulo (Rust `#[cfg(test)]`; Python `tests/` por pacote).
- CI em `.github/workflows/ci.yml`: cargo test/clippy/fmt + pytest + gitleaks (bloqueante).
