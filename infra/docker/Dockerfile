# syntax=docker/dockerfile:1
#
# Imagem do BuildToValue — CLI `btv` + sidecar Python + as duas SPAs (produto
# btv-web na raiz, console web em /dev). O produto é local-first (ver
# infra/README.md): por padrão o `btv dashboard` amarra em 127.0.0.1 e você o
# usa como FERRAMENTA via SSH. A MESMA imagem serve o caminho de hospedagem
# opt-in (atrás de ingress COM auth): `btv dashboard --host 0.0.0.0` +
# `BTV_TRUSTED_ORIGINS` — ver "Hospedagem atrás de um ingress" no README e
# infra/docker/docker-compose.prod.yml. Nunca exponha a porta direto na internet.
#
# Build (a partir da RAIZ do repositório):
#   docker build -f infra/docker/Dockerfile -t btv:test .
# Uso (shell interativo com o `btv` no PATH):
#   docker run --rm -it -e DEEPSEEK_API_KEY=sk-... -v "$PWD":/work btv:test
#   # dentro do container (com DeepSeek, --model é OBRIGATÓRIO — ver docker/README.md):
#   btv verify                                   # self-teste (mesmo comando do CI)
#   btv run --model deepseek-chat "..."          # tarefa única
#   btv chat --model deepseek-chat               # sessão interativa
#   btv squad --model deepseek-chat "..."        # squad multi-agente (usa o sidecar Python)
#
# Pegadinhas (detalhe em infra/docker/README.md): dashboard em 127.0.0.1,
# sandbox de skills de terceiro exige o socket do Docker (DinD), e a key só
# existe no processo Rust (ADR 0001).

# ---------- build (frontend do dashboard) ----------
# web/dist NÃO é commitado (web/.gitignore) — precisa ser buildado. Sem este
# estágio, `btv dashboard` sobe mas não tem SPA pra servir (404 no `/`).
FROM node:22-bookworm-slim AS web-build
WORKDIR /web
# web/package.json não pina `packageManager` — instala o major confirmado
# (pnpm 10.x) direto, em vez de depender da resolução implícita do corepack.
RUN npm install -g pnpm@10
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# ---------- build (SPA primária: o produto BuildToValue em btv-web/) ----------
# O produto (galeria/wizard/squad ao vivo) é servido na RAIZ; o console `web/`
# vai para `/dev`. Depende do submódulo `vendor/bpmn` (Designer), buildado por
# `scripts/ensure-bpmn.mjs`. Requer `git submodule update --init --recursive`
# na VPS antes do build (o conteúdo de vendor/bpmn precisa estar no contexto).
# Layout recriado para os aliases relativos: btv-web em /btv-web, submódulo em
# /vendor/bpmn (ensure-bpmn resolve ../../vendor/bpmn; vite/tsconfig ../vendor/bpmn).
FROM node:22-bookworm-slim AS btvweb-build
RUN npm install -g pnpm@10
WORKDIR /btv-web
COPY btv-web/ ./
COPY vendor/bpmn/ /vendor/bpmn/
# `prebuild` roda ensure-bpmn (builda o dist ESM da lib bpmn) e então `vite build`.
RUN pnpm install --frozen-lockfile && pnpm build

# ---------- imagem única (build + runtime do Rust) ----------
# Deliberadamente UMA imagem só pro Rust, em vez de compilar em `rust:1-bookworm`
# e copiar o toolchain pra um runtime `debian:bookworm-slim` separado: essa
# mistura de bases já quebrou (linker `cc` incompatível entre o gcc/libc de uma
# imagem e o Rust linkado na outra). Aqui o mesmo ambiente que compila o binário
# é o que roda `btv verify` (cargo test/clippy/fmt) depois — zero risco de
# descompasso de ABI. Custo aceito: imagem maior; é imagem de teste, não de
# produção.
FROM rust:1-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git \
 && rm -rf /var/lib/apt/lists/*
# clippy/rustfmt não vêm no profile default do rustup — `btv verify` (os
# default_steps: cargo test/clippy/fmt) precisa deles.
RUN rustup component add clippy rustfmt

WORKDIR /src
COPY . .
RUN cargo build --release -p btv-cli \
 && cp target/release/btv /usr/local/bin/btv

# uv: o sidecar (squad/promptforge) é spawnado pelo processo Rust via `uv run`.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"

# Workspace Python + ambiente uv resolvido.
# CRÍTICO: BTV_PYTHON_DIR. Sem ele, o sidecar procura um caminho de
# compile-time (`<crate>/../../python`) que NÃO existe no container final (o
# repo real é montado em /work, não em /src), e o squad/promptforge degrada EM
# SILÊNCIO para agente-único. (Ver crates/btv-cli/src/sidecar.rs.)
RUN cd /src/python && uv sync
ENV BTV_PYTHON_DIR=/src/python

# Console de desenvolvedor (web/), servido aninhado em /dev.
COPY --from=web-build /web/dist /src/web/dist
ENV BTV_DEV_WEB_DIR=/src/web/dist
# SPA primária: o produto BuildToValue (btv-web), servido na raiz (/).
COPY --from=btvweb-build /btv-web/dist /src/btv-web/dist
ENV BTV_WEB_DIR=/src/btv-web/dist

# Diretório onde o agente atua — monte o SEU projeto aqui (-v "$PWD":/work).
# `btv verify` roda `cargo test/clippy/fmt` contra O QUE ESTIVER em /work
# (o projeto montado), não contra /src (a cópia usada só pra compilar o btv).
WORKDIR /work
# CMD (não ENTRYPOINT): `docker run btv:test` cai num shell interativo, mas
# `docker run btv:test btv dashboard --port 7878` SUBSTITUI o CMD inteiro e
# roda o binário direto — sem isso, um ENTRYPOINT fixo prefixaria "/bin/bash" na
# frente do comando e quebraria esse uso.
CMD ["/bin/bash"]
