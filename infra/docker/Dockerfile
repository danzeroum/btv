# syntax=docker/dockerfile:1
#
# Imagem de TESTE / homologação do Forge — NÃO é deploy de produção.
# O produto é local-first (ver infra/README.md): isto apenas empacota o CLI
# `forge` + o sidecar Python numa imagem, para você rodar o Forge como
# FERRAMENTA dentro de um container (na sua VPS, via SSH). Não expõe um serviço
# web multiusuário; o `forge dashboard` segue amarrado a 127.0.0.1.
#
# Build (a partir da RAIZ do repositório):
#   docker build -f infra/docker/Dockerfile -t forge:test .
# Uso (shell interativo com o `forge` no PATH):
#   docker run --rm -it -e ANTHROPIC_API_KEY=sk-ant-... -v "$PWD":/work forge:test
#   # dentro do container:
#   forge verify        # self-teste (mesmo comando do CI)
#   forge run "..."     # tarefa única
#   forge chat          # sessão interativa
#   forge squad "..."   # squad multi-agente (usa o sidecar Python)
#
# Pegadinhas (detalhe em infra/docker/README.md): dashboard em 127.0.0.1,
# sandbox de skills de terceiro exige o socket do Docker (DinD), e a key só
# existe no processo Rust (ADR 0001).

# ---------- build ----------
FROM rust:1-bookworm AS build
WORKDIR /src
# protoc é vendorizado (protoc-bin-vendored), TLS é rustls e SQLite é bundled:
# o build NÃO precisa de protobuf-compiler / libssl-dev / libsqlite3-dev.
COPY . .
RUN cargo build --release -p forge-cli

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git \
 && rm -rf /var/lib/apt/lists/*

# uv: o sidecar (squad/promptforge) é spawnado pelo processo Rust via `uv run`.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"

WORKDIR /app
COPY --from=build /src/target/release/forge /usr/local/bin/forge

# Workspace Python + ambiente uv resolvido.
# CRÍTICO: FORGE_PYTHON_DIR. Sem ele, o sidecar procura um caminho de
# compile-time (`<crate>/../../python`) que NÃO existe nesta imagem, e o
# squad/promptforge degrada EM SILÊNCIO para agente-único. (Ver
# crates/forge-cli/src/sidecar.rs.)
COPY python /app/python
RUN cd /app/python && uv sync
ENV FORGE_PYTHON_DIR=/app/python

# Diretório onde o agente atua — monte o SEU projeto aqui (-v "$PWD":/work).
WORKDIR /work
ENTRYPOINT ["/bin/bash"]
