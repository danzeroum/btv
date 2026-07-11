# 01 — Mapa de dependências de build e features

**Pergunta:** quando eu altero/removo um crate ou pacote, quais outros são afetados?
**Entrada:** `Cargo.toml` (workspace), `pyproject.toml`, `package.json`, `schemas/proto/*`.
**Base:** 100% estático.

---

## 1.1 Grafo de dependências Rust (crates internos)

```mermaid
graph TD
    subgraph nucleo["núcleo (estável)"]
        DOM[btv-domain]
        SCH[btv-schemas]
    end
    CORE[btv-core]
    LLM[btv-llm]
    TOOLS[btv-tools]
    STORE[btv-store]
    VERIFY[btv-verify]
    PROTO[btv-proto]
    SIDE[btv-sidecar]
    SERVER[btv-server]
    TUI[btv-tui]
    CLI[btv-cli]
    GOLD[btv-golden]
    CONTRACT[btv-contract]

    SCH --> DOM
    CORE --> DOM
    LLM --> DOM
    TOOLS --> DOM
    STORE --> DOM
    STORE --> SCH
    VERIFY --> SCH
    SIDE --> PROTO
    SERVER --> STORE
    SERVER --> SCH
    SERVER --> LLM
    SERVER --> VERIFY
    SERVER --> TOOLS
    CLI --> CORE
    CLI --> LLM
    CLI --> TOOLS
    CLI --> STORE
    CLI --> VERIFY
    CLI --> SIDE
    CLI --> SERVER
    CLI --> TUI
    CLI --> PROTO
    CLI --> SCH
    CLI --> DOM
    CONTRACT -. dev-dep .-> DOM
    GOLD -. dev-dep .-> SERVER
    GOLD -. dev-dep .-> CLI
    PROTO -. build.rs compila .-> PROTOFILES[schemas/proto/*.proto]
```

## 1.2 Tabela de impacto — crates Rust

| Crate | Depende de (internos) | Afeta se removido/movido | Deps externas notáveis |
|---|---|---|---|
| **btv-domain** | (nenhum) | **TODOS** (é a raiz de ports/agregados) | serde, thiserror, uuid |
| **btv-schemas** | btv-domain | btv-store, btv-verify, btv-server, btv-cli | schemars, sha2, hex |
| **btv-core** | btv-domain | btv-cli | thiserror |
| **btv-llm** | btv-domain | btv-server, btv-cli | reqwest, futures-util |
| **btv-tools** | btv-domain | btv-server, btv-cli | bollard, rmcp, ignore, grep, libc |
| **btv-store** | btv-domain, btv-schemas | btv-server, btv-cli | rusqlite, sqlx (feature `pg`) |
| **btv-verify** | btv-schemas | btv-server, btv-cli | toml, libc |
| **btv-proto** | (nenhum) | btv-sidecar, btv-cli | tonic, prost, tonic-build (+ protoc) |
| **btv-sidecar** | btv-proto | btv-cli | tonic, hyper-util, tower, tokio |
| **btv-server** | btv-store, btv-schemas, btv-llm, btv-verify, btv-tools | btv-cli | axum, tower-http |
| **btv-tui** | (nenhum) | btv-cli | ratatui, crossterm |
| **btv-cli** | ~12 crates (composition root) | **binário `btv`** (topo — nada depende dele) | clap, tokio |
| **btv-golden** | (dev) btv-server, btv-cli | testes de golden HTTP | — |
| **btv-contract** | (dev) btv-domain | testes dual-adapter | — |

**Regra de leitura:** o impacto de remover um crate é a **coluna "Afeta se removido"**
(seus dependentes transitivos). Remover `btv-domain` quebra tudo; remover `btv-cli` não
quebra nenhum crate (só o binário).

## 1.3 Features de build

**Só existe UMA feature no workspace: `pg`** (declarada em `btv-store` e `btv-cli`). Não há
features `uds`/`serde` como flags de build — UDS e serde são sempre compilados.

| Feature | Declarada em | O que compila | Afeta |
|---|---|---|---|
| `pg` | btv-store, btv-cli | adapter Postgres (`sqlx`, `PgStore`, `migrations_pg`), subcomando `btv session`, resolução de tenant SaaS | Sem `pg`: só SQLite (`.btv/*.db`); com `pg`: adiciona Postgres+RLS. **Mutuamente compatíveis** (o build com `pg` NÃO remove SQLite — os dois adapters coexistem). |

Pontos exatos que dependem de `pg` (`#[cfg(feature = "pg")]`): `btv-store/src/lib.rs`,
`btv-cli/src/main.rs` (4 sítios), `btv-cli/src/tenant_extractor.rs`, e o teste
`btv-store/tests/contract_pg.rs`.

## 1.4 Dependências nativas (Python)

| Pacote | Deps | Nativa? |
|---|---|---|
| **btv-proto-py** | `grpcio>=1.60` | **sim** (grpcio traz extensão C/protobuf) |
| **btv-promptforge** | `pydantic>=2`, `grpcio>=1.60`, btv-proto-py | sim (grpcio; pydantic v2 tem core em Rust) |
| **btv-squad** | `pydantic>=2`, `grpcio>=1.60`, btv-proto-py | sim (grpcio) — mas o núcleo de raciocínio (orquestrador/consenso/recall) só usa `pydantic`; `docker` é import **opcional** |
| **btv-review** | `pydantic>=2`, btv-promptforge | **não** (só pydantic; puro) |
| **btv-eval** | (nenhuma) | não (placeholder vazio) |

**Impacto cruzado Python:** mexer em `btv-proto-py` afeta `btv-promptforge` e `btv-squad`
(consumidores dos stubs). Mexer em `btv-promptforge.hashing` afeta `btv-review`
(reusa `canonical_json`/`sha256_hex`) **e** a paridade com o Rust (`btv-schemas::canonical`).

## 1.5 Dependências do frontend

Ambas as SPAs: `react`/`react-dom` `^19`, `vite ^8`, `vitest ^4`, `@playwright/test`,
`oxlint`. **Só `btv-web`** depende de `@bpmn-react/*` — não via npm, mas por **alias vite**
ao submódulo `vendor/bpmn`. Mexer no submódulo `vendor/bpmn` afeta só `btv-web` (o Designer).

## 1.6 Impacto cruzado entre linguagens (via `schemas/`)

O acoplamento Rust↔Python↔TS passa por `schemas/`:

| Se você alterar… | Quebra em Rust | Quebra em Python | Quebra em TS |
|---|---|---|---|
| `schemas/proto/*.proto` | btv-proto (recompila) → sidecar/cli | btv-proto-py (regenera) → squad/promptforge | DTOs que espelham proto (squad events) |
| `prompt-cache-key.v1` (algoritmo) | btv-schemas::canonical | btv-promptforge::hashing | — (só produtores Rust/Python) |
| `schemas/json/*.v1.schema.json` | DTOs de btv-schemas (contract tests) | modelos pydantic espelhados | DTOs TS espelhados |
| `schemas/squad-templates/*.json` | btv-schemas (embutido via `include_str!`) | — | galeria/wizard consomem via API |

## 1.7 Exemplo: "se eu remover/mover `btv-verify`, o que quebra?"

- **Rust:** `btv-server` (handler `/api/verify`, doctor console) e `btv-cli`
  (subcomando `btv verify`, o `/verify` que o squad roda antes de disparar, o skill-vetter).
- **Python:** indiretamente — o `AuditorAgent` julga sobre a `VerificationEvidence` que o
  Rust produz; sem o pipeline, a evidência fica ausente e o auditor reprova fail-closed
  (não "quebra", degrada honestamente).
- **Contratos:** o schema `verification-evidence.v1` e o campo tipado `verification_evidence`
  em `squad.proto` perdem produtor.
