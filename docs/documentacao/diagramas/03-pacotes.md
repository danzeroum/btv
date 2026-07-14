# 03 — Diagrama de Pacotes

**Objetivo:** organização lógica e dependências entre crates Rust, pacotes Python e
módulos TypeScript. Setas = "depende de".

---

## 3.1 Crates Rust (grafo de dependência do workspace)

```mermaid
flowchart TB
    classDef domain fill:#065f46,stroke:#6ee7b7,color:#fff
    classDef core fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef infra fill:#374151,stroke:#d1d5db,color:#fff
    classDef edge fill:#7c2d12,stroke:#fdba74,color:#fff
    classDef test fill:#4c1d95,stroke:#c4b5fd,color:#fff

    DOM["btv-domain\n«ports, entities, events»"]:::domain
    SCH["btv-schemas\n«DTOs + canonical hash»"]:::domain

    CORE["btv-core\n«AgentLoop + PermissionEngine»"]:::core
    LLM["btv-llm\n«Gateway (LlmPort)»"]:::infra
    TOOLS["btv-tools\n«ToolRegistry (ToolsPort)»"]:::infra
    STORE["btv-store\n«SQLite/PG, ledger»"]:::infra
    VERIFY["btv-verify\n«pipeline + vetter»"]:::infra
    PROTO["btv-proto\n«tonic bindings»"]:::infra
    SIDE["btv-sidecar\n«gRPC bridge»"]:::infra

    SERVER["btv-server\n«axum dashboard»"]:::edge
    CLI["btv-cli\n«binário btv — composition root»"]:::edge
    TUI["btv-tui\n«view ratatui pura»"]:::edge

    GOLD["btv-golden\n«HTTP golden (dev-dep)»"]:::test
    CONTRACT["btv-contract\n«suíte dual-adapter (dev)»"]:::test

    SCH --> DOM
    CORE --> DOM
    LLM --> DOM
    TOOLS --> DOM
    STORE --> DOM & SCH
    VERIFY --> SCH
    SIDE --> PROTO
    SERVER --> STORE & SCH & LLM & VERIFY & TOOLS
    CLI --> CORE & LLM & TOOLS & STORE & VERIFY & SIDE & SERVER & TUI & PROTO & SCH & DOM
    CONTRACT --> DOM
    GOLD -.->|dev-dep| SERVER & CLI
    PROTO -->|build.rs compila\nschemas/proto/*.proto| PROTOFILES[["schemas/proto"]]
```

### Camadas

- **Domínio (verde):** `btv-domain` (ports + agregados + eventos, zero infraestrutura),
  `btv-schemas` (DTOs serializáveis + hash canônico). Núcleo estável.
- **Runtime (azul):** `btv-core` (o loop de agente e o motor de permissões).
- **Infraestrutura (cinza):** adapters e I/O — `btv-llm`, `btv-tools`, `btv-store`,
  `btv-verify`, `btv-proto`, `btv-sidecar`.
- **Borda (laranja):** `btv-server` (axum), `btv-cli` (composition root), `btv-tui`
  (view pura).
- **Teste (roxo, dev-deps):** `btv-golden` (goldens HTTP), `btv-contract` (suíte
  dual-adapter sobre as ports).

### Notas de design

- **`btv-domain` é o núcleo sem infraestrutura** — não depende de rusqlite/axum/tonic/
  reqwest (verificado por máquina no job `arch-lint`). É a raiz de acoplamento aferente.
- **Inversão de dependência**: `btv-core::AgentLoop` conhece só `LlmPort`/`ToolsPort`;
  `Gateway` (btv-llm) e `ToolRegistry` (btv-tools) são adapters injetados por `btv-cli`.
- **`btv-cli` é o composition root** — depende de quase tudo. A dependência corre
  `cli → server` (nunca o contrário), o que força `btv-golden`/`btv-contract` a serem
  dev-deps compartilhadas.
- **Ciclo evitado**: `btv-schemas → btv-domain` (só por `TenantId`); `btv-domain` **não**
  depende de `btv-schemas` (ADR 0027).

---

## 3.2 Pacotes Python (workspace uv)

```mermaid
flowchart TB
    classDef pkg fill:#1e3a8a,stroke:#93c5fd,color:#fff
    classDef gen fill:#374151,stroke:#d1d5db,color:#fff

    SQUAD["btv-squad\n«orquestrador + 5 agentes + servers»"]:::pkg
    PF["btv-promptforge\n«generators, lint, hashing»"]:::pkg
    REVIEW["btv-review\n«4 reviewers, gates, cert»"]:::pkg
    PROTOPY["btv-proto-py\n«stubs gRPC gerados»"]:::gen

    SQUAD --> PROTOPY
    PF --> PROTOPY
    REVIEW --> PF
    SQUAD -.->|MemoryService,SquadService| PROTOPY
```

**Notas.** `btv-proto-py` é o contrato de fio comum (gerado de `schemas/proto/*`, nunca
editado à mão). `btv-review.certification` reusa `btv_promptforge.hashing` (mesmo esquema
canônico do cache-key para `evidence_hash`). A avaliação A/B real vive em Rust
(`btv-schemas::experiment`); o antigo `btv-eval` (placeholder vazio) foi removido (Onda 5, B4).

---

## 3.3 Módulos das SPAs TypeScript

```mermaid
flowchart LR
    classDef ctx fill:#065f46,stroke:#6ee7b7,color:#fff
    classDef api fill:#7c2d12,stroke:#fdba74,color:#fff
    classDef ui fill:#1e3a8a,stroke:#93c5fd,color:#fff

    subgraph BTVWEB["btv-web/ (produto, raiz /)"]
        BW_APP["App + Shell"]:::ui
        BW_CTX["state/*: AppContext,\nSquadRunContext, TemplatesContext"]:::ctx
        BW_API["api/*: client, btv, squad,\ntemplates, admin"]:::api
        BW_LIB["lib/esteira.ts\n(esteiraFromEvents)"]:::ctx
        BW_DES["designer/btvPlugin.tsx\n(+ @bpmn-react/*)"]:::ui
        BW_APP --> BW_CTX --> BW_API
        BW_CTX --> BW_LIB
        BW_APP --> BW_DES
    end
    subgraph WEB["web/ (console dev, /dev)"]
        W_APP["App + Shell"]:::ui
        W_CTX["state/*: AppContext,\nSessionContext"]:::ctx
        W_API["api/* (21 módulos)"]:::api
        W_DES["Designer/ (reducer + geometry,\nhand-rolled)"]:::ui
        W_APP --> W_CTX --> W_API
        W_APP --> W_DES
    end
    BW_API & W_API -->|fetch + EventSource\n127.0.0.1:7878| RUST["Rust HTTP edge"]
    BW_DES -->|alias vite| BPMN["vendor/bpmn"]
```

**Notas.** As duas SPAs compartilham o mesmo padrão (Context + reducer, fetch nativo +
EventSource, `api/client.ts` idêntico). A diferença crítica é o Designer: `web/` é board
hand-rolled (reducer + geometria); `btv-web/` é construído sobre `@bpmn-react/*` (submódulo
`vendor/bpmn`) via alias vite + `resolve.dedupe` de react.
