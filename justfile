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

# Pré-push CANÔNICO — espelha o CI comando-a-comando com exit direto POR
# CONSTRUÇÃO (`just` aborta na primeira linha não-zero). Mata a classe de
# mascaramento de exit-code por pipe (`... | tail && echo OK` devolve o exit do
# `tail`, não do gate) que mordeu duas vezes: juiz mecânico > disciplina
# lembrada. Dogfood: o próprio `btv verify` do produto roda o pipeline
# determinístico (cargo test --workspace + clippy + fmt); os complementos que
# o CI cobre mas o `btv verify` default não (arch-lint, clippy da feature `pg`)
# entram como gates próprios, um por linha. A suíte pg-contra-Postgres-real
# (`cargo test -p btv-store --features pg`) exige um Postgres local e roda à
# parte quando o passo toca store/pg — não é gate de todo push.
preflight:
    cargo run -p btv-cli --quiet -- verify --out /tmp/btv-preflight-evidence.json
    ./scripts/arch-lint.sh
    cargo clippy -p btv-store --features pg -- -D warnings
    cargo clippy -p btv-cli --features pg -- -D warnings

# Regenera os stubs gRPC de schemas/proto/*.proto. O lado Rust roda
# automaticamente a cada build (build.rs do btv-proto, via protoc
# vendorizado — sem exigir protoc de sistema); o lado Python precisa ser
# gerado explicitamente com grpcio-tools.
gen-proto: gen-proto-py
    cargo build -p btv-proto

gen-proto-py:
    cd python && uv run python ../scripts/gen_proto_py.py

# Checa mudança breaking nos protos contra a main LOCAL (Trilha T5 da
# migração DDD; no CI o baseline é o commit-base do PR). Requer buf.
proto-breaking:
    buf breaking schemas/proto --against '.git#branch=main,subdir=schemas/proto'

# Regenera as fixtures de paridade de hash com a implementação Python de referência.
gen-fixtures:
    cd python && uv run python ../scripts/gen_fixtures.py
