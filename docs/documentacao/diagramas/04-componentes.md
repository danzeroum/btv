# 04 — Diagrama de Componentes

**Objetivo:** componentes de runtime e suas interfaces de comunicação (HTTP, gRPC,
subprocessos, arquivos).
**Escopo:** processos vivos + storages + serviços externos.

---

```mermaid
flowchart TB
    classDef proc fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef store fill:#065f46,stroke:#6ee7b7,color:#fff
    classDef ext fill:#7c2d12,stroke:#fdba74,color:#fff
    classDef ui fill:#374151,stroke:#d1d5db,color:#fff

    subgraph BROWSER["Navegador"]
        SPA1["btv-web (SPA raiz /)"]:::ui
        SPA2["web (console /dev)"]:::ui
    end

    subgraph RUSTPROC["Processo Rust — btv (127.0.0.1)"]
        AXUM["axum edge\nbtv-server + web_agent\n+ squad_agent + btv_agent"]:::proc
        LOOP["btv-core::AgentLoop\n+ PermissionEngine"]:::proc
        GATE["btv-llm::Gateway\nCached→RateLimited→Gateway"]:::proc
        REG["btv-tools::ToolRegistry\nread/grep/edit/bash/skill/mcp/lsp"]:::proc
        VER["btv-verify\nrun_pipeline"]:::proc
        CORESRV["btv-sidecar::CoreServer\n«serve CoreService»"]:::proc
        SUP["Supervisores/Pool\nSquad/PromptForge/Memory"]:::proc
    end

    subgraph PYPROC["Processo(s) Python — sidecar (uv run)"]
        SQSRV["SquadServicer\n«serve SquadService»"]:::proc
        PFSRV["PromptForgeServicer\n«serve PromptForgeService»"]:::proc
        MEMSRV["MemoryServicer\n«serve MemoryService»"]:::proc
        ORCH["UnifiedOrchestrator\n+ 5 BaseAgent"]:::proc
    end

    subgraph STORAGE["Storage local (.btv/)"]
        SQLITE[("SQLite\nbtv.db · telemetry.db · events.db")]:::store
        JSONL[("JSONL episódico\nsquad-memory/")]:::store
        FILES[("arquivos de entrega\n+ tool-output overflow")]:::store
    end

    PG[("Postgres + RLS\n(feature pg, SaaS)")]:::store
    LLMEXT["Provedores LLM\nAnthropic/DeepSeek/OpenAI"]:::ext
    DOCKER["Docker daemon\n(bollard sandbox)"]:::ext
    MCPEXT["Servidores MCP\n(rmcp, stdio)"]:::ext
    LSPEXT["Language servers\n(LSP, stdio)"]:::ext

    SPA1 -. "REST + SSE" .-> AXUM
    SPA2 -. "REST + SSE" .-> AXUM
    AXUM --> LOOP & VER
    LOOP --> GATE & REG
    GATE -. HTTPS .-> LLMEXT
    REG -. bind mount /work .-> DOCKER
    REG -. stdio JSON-RPC .-> MCPEXT
    REG -. stdio JSON-RPC .-> LSPEXT
    LOOP & AXUM --> SQLITE
    GATE --> SQLITE
    AXUM -. "feature pg" .-> PG

    SUP -->|"spawn: uv run -m ...\n--core-socket"| SQSRV & PFSRV & MEMSRV
    SUP -. "gRPC/UDS (Rust chama)" .-> SQSRV & PFSRV & MEMSRV
    SQSRV --> ORCH
    ORCH -. "gRPC/UDS (Python chama de volta)\nGenerate·RunTool·RequestPermission" .-> CORESRV
    CORESRV --> GATE & REG
    MEMSRV --> JSONL
    REG --> FILES
```

---

## Interfaces (contratos de comunicação)

| Origem → Destino | Protocolo | Superfície |
|---|---|---|
| SPA → axum | HTTP REST + **SSE** | `/api/*`; `Origin/Host` guard fail-closed |
| axum → AgentLoop | in-process | `continue_run` (spawn_blocking) |
| Gateway → LLM | HTTPS (SSE) | Anthropic Messages / OpenAI Chat Completions |
| Rust `btv-sidecar` → Python | **gRPC/UDS** | `SquadService.ExecuteTask` (stream), `PromptForgeService`, `MemoryService` |
| Python `UnifiedOrchestrator` → Rust `CoreServer` | **gRPC/UDS** | `CoreService.Generate` (stream), `RunTool`, `RequestPermission` |
| ToolRegistry → Docker/MCP/LSP | bollard / stdio JSON-RPC | sandbox, tools externas, definição/refs/diag |
| tudo → storage | rusqlite / sqlx | `.btv/*.db` (WAL) ou Postgres+RLS |

## Detalhe do transporte UDS

Todos os clientes gRPC usam o mesmo padrão (tonic + hyper-util + tower): um `Endpoint`
com URI-placeholder (`http://sidecar.invalid`) e um `tower::service_fn` que disca um
`tokio::net::UnixStream` embrulhado em `hyper_util::rt::TokioIo`. A conexão é **lazy** — a
primeira RPC é que falha se o socket não estiver pronto (daí o loop de health-check dos
supervisores). O `grpc.default_authority` é ajustado no lado Python porque não há host
real sobre UDS (achado de interop registrado no ADR 0005).

## Ciclo de vida dos processos Python

`btv-sidecar` tem três supervisores (`SquadSupervisor`, `SidecarSupervisor`,
`MemorySupervisor`) que fazem `spawn` de `uv run python -m <módulo> --socket ...`, esperam
o socket + health check, e **matam o grupo de processos inteiro** no drop
(`libc::kill(-pid, SIGKILL)`) — porque `uv run` re-forka o Python e matar só o `uv`
orfanaria o servidor. A camada de serviço de longa duração (ADR 0019) mantém singletons
(`SidecarService`, `MemoryService`) e um pool limitado (`SquadPool`) com restart-on-crash.

## Notas de design

O ponto mais sutil é o **loop bidirecional numa única chamada**: `SquadService.ExecuteTask`
(Rust→Python) roda o orquestrador que, na mesma execução, chama de volta `CoreService`
(Python→Rust) para gerar texto, rodar ferramentas e pedir permissão — as keys nunca saem
do Rust. Fallback progressivo (`SquadRun::Failed` no `drain_stream`): squad → agente-único
→ safe-mode read-only.
