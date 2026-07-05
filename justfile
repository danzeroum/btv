# Orquestrador de build da plataforma Forge.
# Requer: cargo (rustup), uv (https://docs.astral.sh/uv), e para gen-proto: buf.

default: test

build:
    cargo build --workspace
    cd python && uv sync

test:
    cargo test --workspace
    cd python && uv run pytest

lint:
    cargo clippy --workspace -- -D warnings
    cargo fmt --all --check

# Pipeline de verificação determinística (evidência JSON — Fase 5 completa o /verify).
verify: test lint

# Regenera stubs gRPC a partir de schemas/proto (ativado na Fase 3).
gen-proto:
    @echo "tonic-build (Rust) + betterproto (Python) — ativado na Fase 3"

# Regenera as fixtures de paridade de hash com a implementação Python de referência.
gen-fixtures:
    cd python && uv run python ../scripts/gen_fixtures.py
