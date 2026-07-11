# 11 — Modelo de dados (Entidade-Relacionamento)

O esquema de persistência. Fonte: `crates/btv-store/src/*`. Todas as tabelas de produto
carregam `tenant_id` (default `LOCAL`, ADR 0025). SQLite local por padrão; o mesmo esquema
lógico no Postgres (feature `pg`) com RLS por tenant.

Contratos serializados correspondentes: [`ledger-entry.v1`, `telemetry-event.v1`](../referencia/13-contratos-grpc-e-schemas.md#132-json-schemas-schemasjsonv1schemajson).

---

## 11.1 Produto BuildToValue (`.btv/btv.db` — `BtvStore`)

```mermaid
erDiagram
    RUNS ||--o{ DELIVERABLES : "produz"
    RUNS {
        i64 id PK
        string task_id "sq{hex}, único por tenant"
        string template_id
        string template_versao
        string nome
        string briefing_json
        string papeis_json
        string status "ativa|concluida|encerrada|erro"
        i64 gates_aprovados
        string created_ts
        string updated_ts
        string tenant_id
    }
    DELIVERABLES {
        i64 id PK
        i64 run_id FK
        string task_id
        string template_id
        string nome
        string path
        string formato
        string versao
        string trilha
        string created_ts
        string tenant_id
    }
    PERSONA_OVERRIDES {
        string template_id
        string papel
        string prompt
        string tenant_id
    }
    CUSTOM_PERSONAS {
        i64 id PK
        string template_id
        string nome
        string prompt
        string created_ts
        string tenant_id
    }
    TEMPLATE_PUB {
        string template_id
        bool publicado
        string tenant_id
    }
    USERS {
        i64 id PK
        string nome
        string email
        string papel
        string pin_hash "nunca sai do adapter"
        bool ativo
        string created_ts
        string tenant_id
    }
```

**Notas.** `RUNS`→`DELIVERABLES` é a única relação forte (entrega só existe sob um run), e
`RunRepository::save_with_deliverables` é a **unidade transacional** (run+entregas commitam
juntos). `PERSONA_OVERRIDES`/`CUSTOM_PERSONAS`/`TEMPLATE_PUB` são chaveadas por
`template_id` (os templates em si são estáticos, embutidos no binário). `USERS.pin_hash`
nunca cruza a fronteira — `verify_pin` compara dentro do adapter e devolve `PinCheck`.

---

## 11.2 Ledger append-only (`.btv/btv.db` — `LedgerStore`)

```mermaid
erDiagram
    LEDGER {
        i64 id PK "ordem física de inserção"
        string tenant_id "default LOCAL"
        u64 seq "monotônico POR tenant"
        string prev_hash "'' na primeira entrada"
        string entry_hash "sha256(prev_hash + corpo canônico)"
        string body "kind, actor, payload, override, fake_marker, ts, tenant"
    }
```

**Chave lógica:** `(tenant_id, seq)` `UNIQUE`. Nunca há `UPDATE`/`DELETE` — overrides são
novas entradas com `override.marked=true`. O `tenant` entra no **corpo hasheado**
(anti-transplante: reatribuir a outro tenant quebra `entry_hash`). `verify_chain` recomputa
a cadeia por tenant. No Postgres, o append usa retry otimista sobre `UNIQUE(tenant_id, seq)`
(ADR 0028) e as funções de DTO/verificação são **compartilhadas** com o SQLite → paridade
criptográfica (provada por `btv-contract`).

---

## 11.3 Event store de sessão (`.btv/events.db` — `EventStore`)

```mermaid
erDiagram
    EVENT_SEQUENCE ||--o{ EVENT : "agrega"
    EVENT_SEQUENCE {
        string aggregate_id PK
        i64 seq "head atual (concorrência otimista)"
        string owner_id
    }
    EVENT {
        string id PK "aggregate_id + seq"
        string aggregate_id FK
        i64 seq
        string type "name.N (versão embutida)"
        string data "JSON"
    }
```

**Notas.** `append(aggregate_id, expected_head, events)` falha com `ConcurrencyConflict` se
`expected_head` divergir do `seq` atual. Adapter **LOCAL-only** (fail-closed em tenant
não-LOCAL — o esquema não tem coluna de tenant; sessões SaaS nascem no Postgres).

---

## 11.4 Telemetria, cache e biblioteca (`.btv/telemetry.db` e afins)

```mermaid
erDiagram
    TELEMETRY_EVENT {
        i64 id PK
        string name "llm.call | cache.hit | cache.miss | ..."
        string session_id
        string props "JSON (model, tokens, ...)"
        string ts
    }
    PROMPT_CACHE {
        string hash PK "prompt-cache-key.v1"
        string response
        string created_at
    }
    PROMPT_LIBRARY {
        i64 id PK
        string name
        string generator
        string fields "JSON"
        string rendered
        string tags "JSON"
        bool favorite
        string created_at
    }
    PERMISSION_RULES {
        i64 id PK
        string profile "build | plan"
        string tool
        string scope_prefix "opcional"
        string decision "allow | ask | deny"
        string created_at
    }
```

**Notas.** `TELEMETRY_EVENT` alimenta o dashboard (summary/model_usage) e o relatório A/B
(`experiment_variants` agrega por `props.experiment`). `PROMPT_CACHE` é chaveado pelo hash
canônico (o cache do decorator externo). `PERMISSION_RULES` são os overrides persistidos
que o `PermissionEngine.overlay` aplica por cima do perfil.

---

## 11.5 Sessões SaaS (Postgres — `PgStore`, feature `pg`)

```mermaid
erDiagram
    SESSIONS {
        string token_hash PK "sha256; token em claro existe uma vez"
        string user_id
        string tenant_id
        string absolute_deadline
        string idle_deadline "renovado no resolve"
    }
```

**Notas.** Tabela **exclusiva do modo SaaS** (LOCAL não tem sessões, por isso não há
`SessionsPort` nem análogo SQLite). Token = 256-bit CSPRNG → base64url com prefixo `btvs_`;
só o `token_hash` é gravado. `resolve_session` valida-e-renova o idle deadline numa única
query, fail-closed para `None`.

---

## Visão consolidada — quem escreve o quê

| Tabela | Escritores | Contrato |
|---|---|---|
| `runs` / `deliverables` | `btv_agent` (ativação), `squad_agent` (status/entregas) | agregado `Run`/`Deliverable` |
| `ledger` | squad, designer, sessão, permissão | `DomainEvent` → `ledger-entry.v1` |
| `event` | `DurableSession` (sessão de código) | `EventInput`/`StoredEvent` |
| `telemetry_event` | decorators do gateway, dashboard | `telemetry-event.v1` |
| `prompt_cache` | `CachedGenerator` | `prompt-cache-key.v1` |
| `permission_rules` | matriz de permissão (web) | `Rule` |
| `sessions` (PG) | `issue_session` (operador SaaS) | — |
