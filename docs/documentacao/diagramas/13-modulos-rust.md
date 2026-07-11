# 13 — Diagramas de módulo por crate (Rust)

Estrutura interna de cada um dos 14 crates: módulos (nós) e suas dependências internas
(setas). Complementa o [diagrama de pacotes (03)](03-pacotes.md), que mostra as
dependências *entre* crates. Inventário textual: [referência 10](../referencia/10-rust-crates.md).

---

## btv-domain

```mermaid
flowchart LR
    lib --> ports & run & tenant & chat & tool & event & user & persona & ledger_kind
    ports --> run & tenant & chat & tool
    run --> tenant & ports
    ledger_kind --> event
```

`ports` (traits + agregado `Run` + `DomainEvent`) é o centro; `chat`/`tool` são os tipos
neutros de provider que o runtime consome; `tenant` é transversal (todo método de repo o
recebe).

## btv-core

```mermaid
flowchart LR
    lib --> agent & agent_loop & compaction & permission & session
    agent_loop --> permission
    agent --> permission
    compaction -.->|LlmPort| domain[(btv-domain)]
    agent_loop -.->|LlmPort/ToolsPort| domain
```

`agent_loop` é o coração; `permission` é o motor de decisão; `agent` (perfis) produz
`PermissionEngine`; `compaction` resume histórico.

## btv-llm

```mermaid
flowchart LR
    lib --> gateway & provider & model_tier & rate_limit & scripted & pricing & sse & chat
    gateway --> provider & anthropic & openai & sse
    anthropic --> chat & sse
    openai --> chat & sse
    provider --> schemas[(btv-schemas::canonical)]
    model_tier -.->|ModelTier| domain[(btv-domain::chat)]
    rate_limit --> model_tier
```

`gateway` implementa `LlmPort` e despacha por `ProviderId` para os transportes
`anthropic`/`openai`; `rate_limit` e `scripted` sustentam decorators e testes.

## btv-tools

```mermaid
flowchart LR
    lib --> registry & read & grep & edit & bash & skill & mcp & lsp & sandbox & diff
    registry --> read & grep & edit & bash
    skill --> sandbox
    edit --> diff
    mcp -.->|rmcp| ext1[servidores MCP]
    lsp -.->|JSON-RPC| ext2[language servers]
    sandbox -.->|bollard| ext3[Docker]
```

`registry` implementa `ToolsPort`; `skill`/`mcp`/`lsp` são registrados por `btv-cli`;
`sandbox` só é alcançado via `skill` (terceiro).

## btv-store

```mermaid
flowchart LR
    lib --> ledger & btv & events & prompt_cache & prompt_library & rule_store & telemetry
    lib -->|feature pg| pg
    pg -.->|reusa DTO/verificação| ledger
    pg -.->|reusa mappers/pin_hash| btv
    ledger --> schemas[(btv-schemas::ledger)]
    btv --> domain[(btv-domain)]
```

`ledger`/`btv`/`events` são os adapters SQLite das ports; `pg` é o adapter Postgres que
**reusa** as funções compartilhadas do ledger e do btv → paridade.

## btv-verify

```mermaid
flowchart LR
    lib --> config & exec & parsers & prompt_integrity & vetter
    lib --> schemas[(btv-schemas::verification)]
    vetter --> config & exec & parsers
    lib -->|run_step| exec
    lib -->|apply| parsers
    exec -.->|process_group kill| os[(SO / libc)]
```

`exec` roda subprocessos com kill de grupo; `parsers` extraem findings; `vetter` reusa a
mesma máquina para decidir `Vet`/`Block`.

## btv-schemas

```mermaid
flowchart LR
    lib --> canonical & ledger & verification & experiment & handoff & persona & plan & review & squad_template & telemetry & workflow
    ledger --> canonical
    ledger --> domain[(btv-domain::TenantId)]
    review --> verification
    squad_template -.->|include_str| files["schemas/squad-templates/*.json"]
```

`canonical` é o hash `prompt-cache-key.v1` (twin de `hashing.py`); `verification` é o
contrato compartilhado com `btv-verify` **e** `review`.

## btv-sidecar

```mermaid
flowchart LR
    lib --> client & squad_client & memory_client & core_server & service & supervisor
    client & squad_client & memory_client --> proto[(btv-proto)]
    core_server --> proto
    supervisor -.->|spawn uv run| py[sidecar Python]
    service --> supervisor & client
    service --> squad_client & memory_client
    core_server -.->|CoreBackend| gw[Gateway/ToolRegistry via btv-cli]
```

Clientes (Rust→Python) + `core_server` (Python→Rust) + supervisores/pool (ciclo de vida).

## btv-server

```mermaid
flowchart LR
    lib --> guard & btv
    lib --> handlers
    subgraph handlers
        telemetria & prompts & ledger & admin & providers & verify & designer
    end
    lib --> doctor_console & lsp_console & sandbox_console
    lib -.->|AppState| store[(btv-store)]
    verify --> vfy[(btv-verify)]
    bin_loadgen[bin/loadgen] -.->|ScriptedGenerator| llm[(btv-llm)]
```

`lib::router` monta as rotas + SPA fallback + guard de Origin; `handlers/*` falam só com
`btv-store` (SQL cru proibido por lint T4).

## btv-cli (composition root)

```mermaid
flowchart TB
    main --> prepare & build_loop & run_dashboard
    prepare --> cache & rate_limit_gen
    cache & rate_limit_gen -.->|decoram| gateway[(btv-llm::Gateway)]
    build_loop --> skills
    skills -.->|monta| registry[(btv-tools)]
    run_dashboard --> web_agent & squad_agent & btv_agent & prompt_render & mcp_console & memory_console
    squad_agent --> sidecar & session
    btv_agent --> squad_agent & session
    web_agent --> session & tenant_extractor
    main --> squad & tui_app & convert
    squad --> sidecar
```

Onde tudo se amarra: `prepare` constrói o stack de generators, `build_loop` injeta tools +
permissões no `AgentLoop`, `run_dashboard` mescla os routers HTTP.

## btv-tui, btv-proto, btv-golden, btv-contract (módulo único)

Crates de um só arquivo (`lib.rs`), sem grafo interno relevante:

```mermaid
flowchart LR
    tui["btv-tui::lib\nDiffKind, Item, TuiState, render()"]
    proto["btv-proto::lib\nre-exporta módulos tonic gerados"]
    golden["btv-golden::lib\nVolatile, step(), check()"]
    contract["btv-contract::lib\nsuite_* + cross-adapter"]
    proto -.->|build.rs| protofiles["schemas/proto/*.proto"]
    contract -.->|genérico sobre| domain[(btv-domain::ports)]
    golden -.->|dev-dep| edge[btv-server + btv-cli]
```
