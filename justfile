# Orquestrador de build da plataforma BuildToValue.
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

# Lint arquitetural da migração DDD (Trilha T4): fronteiras de camada
# verificadas por máquina — ver scripts/arch-lint.sh.
arch-lint:
    ./scripts/arch-lint.sh

# Pipeline de verificação determinística (evidência JSON — Fase 5 completa o /verify).
verify: test lint

# Regenera os stubs gRPC de schemas/proto/*.proto. O lado Rust roda
# automaticamente a cada build (build.rs do btv-proto, via protoc
# vendorizado — sem exigir protoc de sistema); o lado Python precisa ser
# gerado explicitamente com grpcio-tools.
gen-proto: gen-proto-py
    cargo build -p btv-proto

gen-proto-py:
    cd python && uv run python ../scripts/gen_proto_py.py

# Regenera as fixtures de paridade de hash com a implementação Python de referência.
gen-fixtures:
    cd python && uv run python ../scripts/gen_fixtures.py
