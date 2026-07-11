# 10 — Modelo C4 (Contexto → Contêiner → Componente)

O [modelo C4](https://c4model.com) dá a visão de arquitetura em níveis de zoom. Aqui os
quatro níveis; o **Nível 4 (Código)** é o [diagrama de classes](05-classes.md). Usa-se
flowchart (renderização robusta no GitHub) em vez da sintaxe C4 experimental.

---

## C4 — Nível 1: Contexto do sistema

Quem usa o BuildToValue e com que sistemas externos ele fala.

```mermaid
flowchart TB
    classDef person fill:#1f2937,stroke:#9ca3af,color:#fff
    classDef sys fill:#0e7490,stroke:#67e8f9,color:#fff
    classDef ext fill:#374151,stroke:#d1d5db,color:#fff

    PROF(["👤 Profissional não técnico"]):::person
    DEV(["👤 Desenvolvedor / Operador / Admin"]):::person

    BTV["<b>BuildToValue</b>\nPlataforma de squads de IA\n(núcleo Rust + sidecar Python + 2 SPAs)"]:::sys

    LLM["Provedores LLM\nAnthropic / DeepSeek / OpenAI"]:::ext
    DOCKER["Docker daemon\n(sandbox de skills)"]:::ext
    MCP["Servidores MCP\n(tools de terceiro)"]:::ext
    LSP["Language servers\n(rust-analyzer, ...)"]:::ext
    PG["Postgres\n(modo SaaS opcional)"]:::ext

    PROF -->|"galeria/wizard, esteira ao vivo,\nbiblioteca, designer (navegador)"| BTV
    DEV -->|"CLI/TUI, console dev,\nsessão de código, admin"| BTV
    BTV -->|"HTTPS (geração)"| LLM
    BTV -->|"bollard (contenção)"| DOCKER
    BTV -->|"stdio JSON-RPC"| MCP
    BTV -->|"stdio JSON-RPC"| LSP
    BTV -.->|"sqlx (feature pg)"| PG
```

**Notas.** Um único sistema de software com duas classes de usuário. As saídas de rede
(LLM/MCP/LSP/Docker) partem **só** do processo Rust; o navegador nunca alcança nenhum
sistema externo diretamente.

---

## C4 — Nível 2: Contêineres

As unidades executáveis/implantáveis e os armazenamentos.

```mermaid
flowchart TB
    classDef cont fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef store fill:#065f46,stroke:#6ee7b7,color:#fff
    classDef ext fill:#374151,stroke:#d1d5db,color:#fff
    classDef ui fill:#7c2d12,stroke:#fdba74,color:#fff

    subgraph BROWSER["Navegador"]
        SPA1["<b>btv-web</b> (SPA)\nReact 19 · produto · raiz /"]:::ui
        SPA2["<b>web</b> (SPA)\nReact 19 · console · /dev"]:::ui
    end

    CORE["<b>Processo btv</b> (Rust)\nCLI/TUI · axum edge · AgentLoop ·\nGateway · ToolRegistry · verify ·\nstorage · CoreServer (gRPC)"]:::cont
    SIDE["<b>Sidecar</b> (Python, uv run)\nSquadService · PromptForge ·\nMemory · UnifiedOrchestrator"]:::cont

    SQLITE[("SQLite\n.btv/btv.db · telemetry.db · events.db")]:::store
    JSONL[("Corpus episódico\n.btv/squad-memory/*.jsonl")]:::store
    FILES[("Arquivos de entrega\n+ tool-output overflow")]:::store
    PG[("Postgres + RLS\n(SaaS, feature pg)")]:::store

    LLM["Provedores LLM"]:::ext
    DOCKER["Docker / MCP / LSP"]:::ext

    SPA1 & SPA2 -->|"HTTP REST + SSE\n127.0.0.1"| CORE
    CORE <-->|"gRPC / UDS\n(CoreService ⇄ Squad/PromptForge/Memory)"| SIDE
    CORE -->|"rusqlite"| SQLITE
    CORE -->|"tools (edit/bash)"| FILES
    SIDE -->|"grava/lê"| JSONL
    CORE -->|"HTTPS"| LLM
    CORE -->|"bollard / stdio"| DOCKER
    CORE -.->|"sqlx"| PG
```

**Notas.** Dois contêineres de processo (Rust, Python) + duas SPAs (contêineres de
navegador) + armazenamentos locais. O SaaS acrescenta só o Postgres — **sem** novo
contêiner de aplicação (mesmos traits, outro adapter).

---

## C4 — Nível 3: Componentes do contêiner Rust (`btv`)

Zoom no processo Rust: os crates como componentes e suas ligações.

```mermaid
flowchart TB
    classDef edge fill:#7c2d12,stroke:#fdba74,color:#fff
    classDef core fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef infra fill:#374151,stroke:#d1d5db,color:#fff
    classDef dom fill:#065f46,stroke:#6ee7b7,color:#fff

    AXUM["axum edge\n(btv-server + web_agent\n+ squad_agent + btv_agent)"]:::edge
    TUI["btv-tui + tui_app"]:::edge
    CLI["btv-cli (composition root)"]:::edge

    LOOP["btv-core\nAgentLoop + PermissionEngine\n+ CompactionPolicy"]:::core

    GW["btv-llm\nGateway + decorators"]:::infra
    TOOLS["btv-tools\nToolRegistry + sandbox/MCP/LSP"]:::infra
    STORE["btv-store\nLedgerStore/BtvStore/EventStore/PgStore"]:::infra
    VER["btv-verify\npipeline + vetter"]:::infra
    SIDE["btv-sidecar\nCoreServer + clients + supervisores"]:::infra
    PROTO["btv-proto (tonic)"]:::infra

    DOM["btv-domain (ports + agregados)"]:::dom
    SCH["btv-schemas (DTOs + hash)"]:::dom

    CLI --> AXUM & TUI & LOOP & GW & TOOLS & STORE & VER & SIDE
    AXUM --> LOOP & VER & STORE
    LOOP -->|LlmPort| GW
    LOOP -->|ToolsPort| TOOLS
    LOOP -->|"repos"| STORE
    GW --> SCH
    STORE --> SCH & DOM
    VER --> SCH
    SIDE --> PROTO
    LOOP --> DOM
    GW --> DOM
    TOOLS --> DOM
```

---

## C4 — Nível 3: Componentes do contêiner Python (sidecar)

```mermaid
flowchart TB
    classDef srv fill:#7c2d12,stroke:#fdba74,color:#fff
    classDef orch fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef sub fill:#374151,stroke:#d1d5db,color:#fff
    classDef port fill:#065f46,stroke:#6ee7b7,color:#fff

    SQ["SquadServicer"]:::srv
    MEMS["MemoryServicer"]:::srv
    PF["PromptForgeServicer"]:::srv

    OR["UnifiedOrchestrator"]:::orch
    AG["5 agentes (BaseAgent)"]:::orch

    CONS["WeightedConsensusEngine"]:::sub
    PLAN["AdaptivePlanner"]:::sub
    HITL["ProgressiveAutonomyManager"]:::sub
    MEM["AgentMemorySystem + recall (TF-IDF)"]:::sub

    GC["GatewayClient / PermissionClient / ToolClient\n(Protocols)"]:::port
    GRPC["Grpc*Client → CoreServiceStub"]:::port

    SQ --> OR
    OR --> AG & CONS & PLAN & HITL & MEM
    AG --> GC
    HITL --> GC
    GRPC -.->|implementa| GC
    GRPC -.->|"gRPC/UDS de volta ao Rust"| RUST["CoreService (Rust)"]
    MEMS --> MEM
```

---

## C4 — Nível 3: Componentes de uma SPA (`btv-web`)

```mermaid
flowchart TB
    classDef ui fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef ctx fill:#065f46,stroke:#6ee7b7,color:#fff
    classDef api fill:#7c2d12,stroke:#fdba74,color:#fff

    SHELL["Shell (Sidebar/Topbar)"]:::ui
    SCREENS["Screens U1–U7 / A1–A6\n+ Wizard"]:::ui
    DES["Designer + btvPlugin\n(@bpmn-react/*)"]:::ui

    APPC["AppContext"]:::ctx
    TPL["TemplatesContext"]:::ctx
    RUNC["SquadRunContext\n(esteiraFromEvents)"]:::ctx

    APIC["api/client (fetchJson)"]:::api
    APIB["api/btv · squad · templates · admin"]:::api

    SHELL --> SCREENS --> RUNC & TPL & APPC
    SCREENS --> DES
    RUNC --> APIB
    TPL --> APIB
    APIB --> APIC
    APIC -->|"HTTP/SSE"| RUST["Rust edge"]
```

**Nota.** O contêiner `web` (console `/dev`) tem estrutura análoga, trocando
`SquadRunContext`+bpmn por `SessionContext`+Designer hand-rolled e ~22 módulos `api/*`.

---

## Nível 4 — Código

O detalhamento de classes/structs/traits está no
[diagrama de classes (05)](05-classes.md), com o inventário textual completo na
[referência Rust (10)](../referencia/10-rust-crates.md),
[Python (11)](../referencia/11-python-pacotes.md) e
[TypeScript (12)](../referencia/12-typescript-frontend.md).
