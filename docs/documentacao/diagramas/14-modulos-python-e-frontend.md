# 14 — Diagramas de módulo: Python e frontend

Estrutura interna dos 5 pacotes Python e das 2 SPAs. Inventário textual:
[referência 11 (Python)](../referencia/11-python-pacotes.md) e
[referência 12 (TypeScript)](../referencia/12-typescript-frontend.md).

---

## Python

### btv-squad (o pacote central)

```mermaid
flowchart TB
    subgraph servers["servidores gRPC"]
        server["server.py\nSquadServicer"]
        memsrv["memory_server.py\nMemoryServicer"]
    end
    subgraph clients["clientes de volta ao Rust"]
        gc["grpc_clients.py\nGrpc{Gateway,Permission,Tool}Client"]
    end
    subgraph orch["orquestração"]
        orchestrator["orchestrator.py\nUnifiedOrchestrator"]
        agents["agents/*\n5 x BaseAgent"]
    end
    subgraph subs["subsistemas"]
        consensus & planning & hitl & memory & recall & routing & evaluation & parallel & chains
    end
    subgraph seams["Protocols (ADR 0005)"]
        gateway_p["gateway.py"] & permission_p["permission.py"] & tool_client["tool_client.py"]
    end
    subgraph wire["espelhos de wire + segurança"]
        verification & tenant & security & sandbox & _json
    end

    server --> orchestrator
    server --> gc
    server --> verification & tenant
    orchestrator --> agents & consensus & planning & hitl & memory & routing & evaluation & parallel & chains
    agents --> gateway_p & tool_client
    hitl --> permission_p
    memory --> recall
    memsrv --> memory
    gc -.->|implementa| gateway_p & permission_p & tool_client
    gc --> proto[(btv_proto)]
    server --> proto
```

`server.py` é o ponto onde Python-serve-gRPC encontra Python-chama-Rust: injeta os
`Grpc*Client` no `UnifiedOrchestrator`, que compõe os subsistemas e os 5 agentes.

### btv-promptforge · btv-review · btv-proto-py · btv-eval

```mermaid
flowchart LR
    subgraph promptforge["btv-promptforge"]
        pf_server["server.py"] --> generators & lint
        hashing["hashing.py\n(twin de canonical.rs)"]
    end
    subgraph review["btv-review"]
        gates --> score
        certification --> gates
        certification -.->|reusa| hashing
        reviewers -.->|derivam de| evidence[(VerificationEvidence)]
    end
    protopy["btv-proto-py\nstubs gerados de schemas/proto"]
    eval["btv-eval\n(placeholder vazio)"]
    pf_server --> protopy
```

`hashing.py` é reusado por `btv-review.certification` (mesmo esquema canônico). `btv-eval`
é placeholder; a avaliação A/B real vive no Rust.

---

## Frontend

### btv-web (produto)

```mermaid
flowchart TB
    main --> App
    App --> AppProvider --> ToastProvider --> TemplatesProvider --> SquadRunProvider --> Shell
    Shell --> screens_user & screens_admin & Wizard
    screens_user --> Designer
    Designer --> btvPlugin
    btvPlugin -.->|alias| bpmn[(vendor/bpmn)]
    screens_user & screens_admin --> primitives["components/primitives\nAsyncStatus/Modal/Toast"]
    primitives --> useAsync["hooks/useAsyncAction"]
    SquadRunProvider -->|useToast| ToastProvider
    SquadRunProvider --> esteira["lib/esteira.ts\nesteiraFromEvents"]
    SquadRunProvider --> api_squad["api/squad"] & api_btv["api/btv"]
    TemplatesProvider --> api_templates["api/templates"]
    screens_admin --> api_admin["api/admin"]
    api_squad & api_btv & api_templates & api_admin --> client["api/client\nfetchJson"]
    client -.->|HTTP/SSE| rust[(Rust edge)]
```

### web (console dev)

```mermaid
flowchart TB
    main2[main] --> App2[App]
    App2 --> AppProvider2[AppProvider] --> ToastProvider --> SessionProvider --> Shell2[Shell]
    Shell2 --> user["screens/user\n(Sessao/Permissao/Squad/Prompts/...)"]
    Shell2 --> admin["screens/admin\n(Telemetria/Ledger/Verify/Mcp/Lsp/...)"]
    user --> Designer2["Designer/ (hand-rolled)\nreducer + geometry"]
    SessionProvider --> stream["api/stream\nconnectSessionEvents (SSE)"]
    user & admin --> api22["api/* (21 módulos)"]
    api22 --> client2["api/client\nfetchJson"]
    stream & client2 -.->|HTTP/SSE| rust2[(Rust edge)]
```

**Nota.** As duas SPAs compartilham o padrão Context+reducer e um `api/client` idêntico. A
diferença de arquitetura é o Designer (`btv-web`: sobre `@bpmn-react/*`; `web`: board
próprio) e o hook central (`btv-web`: `SquadRunContext`+esteira; `web`: `SessionContext`).
