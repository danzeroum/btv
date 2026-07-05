# ADR 0003 — Primeira ativação do gRPC: PromptForgeService

- Status: aceita
- Data: 2026-07-05

## Contexto

O ADR 0001 definiu gRPC sobre Unix Domain Socket como o canal Rust↔Python,
mas adiou a implementação. A Fase 3 do roadmap pede a primeira ativação
real desse canal, carregando o PromptForge (geradores declarativos,
quality linter — origem: prompte) para dentro do fluxo do agente.

## Decisão

1. **Contrato**: `schemas/proto/promptforge.proto` define
   `PromptForgeService` com `Health`, `Lint`, `Render` e `ListGenerators`.
   Nenhum desses RPCs gera texto de LLM — a regra de ouro do ADR 0001
   (Python nunca fala com provedores) permanece intacta.

2. **Geração de código sem exigir toolchain de sistema**:
   - Rust: `forge-proto/build.rs` usa `tonic-build` com o `protoc`
     vendorizado pelo crate `protoc-bin-vendored`, em vez do binário de
     sistema — o build funciona em qualquer máquina com Rust, sem
     `apt install protobuf-compiler`.
   - Python: `scripts/gen_proto_py.py` usa `grpcio-tools` (não
     `betterproto`, como o ADR 0001 cogitava) — mais maduro e mantido;
     o script corrige o import absoluto que o `grpc_tools.protoc` gera
     por padrão para import relativo (`from . import ...`), já que os
     stubs vivem dentro do pacote `forge_proto`.

3. **Arquitetura do lado Rust**: novo crate `forge-sidecar` com
   `SidecarClient` (fala `PromptForgeService` sobre UDS via
   `tonic::transport::Endpoint::connect_with_connector` + `UnixStream`) e
   `SidecarSupervisor` (`spawn` + `wait_ready`: sobe `uv run python -m
   forge_promptforge.server --socket <path>`, faz poll do socket + health
   check até um timeout, e mata o processo quando dropado via
   `kill_on_drop`).

4. **Degradação graciosa de primeira classe**: `forge-cli::sidecar::try_start()`
   devolve `Option<(SidecarSupervisor, SidecarClient)>` — `None` se o
   workspace Python não existir, `uv` não estiver no PATH, ou o sidecar
   não responder a tempo. `run`, `chat` e `tui` funcionam integralmente
   sem o sidecar; o único efeito da ausência é lint/geradores desativados
   (aviso não bloqueante, nunca erro fatal).

5. **Servidor Python**: `forge_promptforge.server` — `grpc.aio.server()`
   sobre `unix://<socket>`, implementando o servicer sobre os módulos
   puros já existentes (`lint_prompt`, `GENERATORS`). Roda com
   `python -m forge_promptforge.server --socket <path>`.

## Consequências

- O primeiro RPC real do sistema é consultivo (lint) e opt-in
  (`/prompt` no chat) — baixo risco, alto valor de validação da
  arquitetura antes da Fase 4 trazer o squad multi-agente pelo mesmo canal.
- Testes em duas camadas: `forge-sidecar/tests/client_over_uds.rs` (mock
  Rust, rápido, sempre roda) e `forge-sidecar/tests/python_sidecar.rs`
  (processo Python real, pula graciosamente se `uv`/workspace ausentes).
  O CI (`rust` job) instala `uv` e roda `uv sync` antes dos testes para
  exercitar o caminho real.
- `core.proto`/`squad.proto` (já escritos na Fase 1) continuam como
  especificação para a Fase 4 (squad); `forge-proto/build.rs` compila
  hoje só `promptforge.proto` — os demais entram quando o `SquadService`
  for implementado.
