# 06 — Diagramas de Sequência

Cinco fluxos de negócio principais. Para a tabela completa de endpoints, ver
[referência de endpoints](../referencia/14-endpoints-http.md).

---

## 6.1 Ativação de squad pela galeria (o fluxo transversal completo)

**Escopo:** `btv-web` → `btv_agent` (Rust) → motor de squad → orquestrador Python →
callbacks `CoreService` → SSE de volta à esteira. Atravessa **as três linguagens**.

```mermaid
sequenceDiagram
    autonumber
    participant UI as btv-web (SquadRunContext)
    participant AX as axum btv_agent (Rust)
    participant SA as squad_agent::start_squad_task
    participant SUP as SquadPool / Supervisor
    participant PY as SquadServicer (Python)
    participant OR as UnifiedOrchestrator
    participant CS as CoreServer (Rust)
    participant GW as Gateway → LLM

    UI->>AX: POST /api/btv/squads {template_id, briefing, papeis_off}
    AX->>AX: monta task description + PersonaSpec roster + PromptHash
    AX->>SA: start_squad_task(...)
    SA->>SA: /verify (evidência tipada) + BtvStore.insert_run + ledger append
    SA-->>AX: {task_id, run_id}
    AX-->>UI: 200 {task_id, run_id}
    UI->>AX: GET /api/btv/squads/{task_id}/events (SSE)
    SA->>SUP: acquire() slot
    SUP->>PY: ExecuteTask(SquadTask) [gRPC/UDS, stream]
    PY->>OR: execute_complex_task(task, event_sink)
    loop por passo do plano
        OR->>CS: CoreService.Generate(LlmRequest) [callback gRPC]
        CS->>GW: generate(req)  (Cached→RateLimited→Gateway)
        GW-->>CS: AssistantTurn (stream de chunks)
        CS-->>OR: text + usage
        opt precisa de ferramenta
            OR->>CS: RunTool(ToolCall)
            CS->>CS: recomputa scope, PermissionEngine.evaluate
            CS-->>OR: ToolResult(exit_code)
        end
        OR-->>PY: event dict (proposal/consensus/step)
        PY-->>SA: SquadEvent (stream)
        SA-->>UI: SSE SquadEventEnvelope
        UI->>UI: esteiraFromEvents → avança esteira/feed/chat
    end
    opt gate HITL
        OR->>CS: RequestPermission(confidence<0.3/0.5)
        Note over UI,AX: usuário clica "Aprovar" ou "Pedir ajuste"
        UI->>AX: POST /api/btv/squads/{id}/gate | /ajuste
        AX->>SA: resolve_hitl(true) [+ push_user_message no cockpit]
        CS-->>OR: PermissionDecision ALLOW
    end
    PY-->>SA: fim do stream
    SA->>SA: transição de status + registra deliverables
```

**Notas.** Um único `ExecuteTask` carrega o loop bidirecional inteiro. O "pedir ajuste"
injeta a instrução como turno `user` no próximo `Generate` (cockpit real). A esteira nunca
regride, exceto na regressão visual de 2 passos do ajuste (rotulada `inferida`).

---

## 6.2 Sessão de código pelo navegador com permissão HITL (SSE + fail-closed)

**Escopo:** console `web` → `web_agent` (axum) → `AgentLoop` → `WebPermissionResolver`.

```mermaid
sequenceDiagram
    autonumber
    participant UI as web (SessionContext)
    participant AX as web_agent (axum)
    participant HUB as SessionHub
    participant LOOP as AgentLoop
    participant RES as WebPermissionResolver
    participant TOOL as ToolRegistry

    UI->>AX: GET /api/session/{id}/events (SSE snapshot-then-live)
    UI->>AX: POST /api/session/{id}/message {message, model, agent}
    AX->>HUB: try_start (single-actor, 409 se ocupado)
    AX->>LOOP: continue_run (spawn_blocking)
    loop passos
        LOOP-->>HUB: LoopEvent::TextDelta → publish → SSE
        alt tool_use com Decision::Ask
            LOOP->>RES: resolve(tool, scope)
            RES->>HUB: request_permission (bloqueia em mpsc)
            HUB-->>UI: SSE PermissionRequested
            UI->>AX: POST /api/session/{id}/permission {request_id, allow}
            AX->>HUB: resolve_permission
            HUB-->>RES: allow/deny (ou timeout → Deny fail-closed)
        end
        RES-->>LOOP: bool
        LOOP->>TOOL: run(args) se permitido
        LOOP-->>HUB: ToolFinished/ToolDenied → SSE
    end
    LOOP->>AX: dual-persist (ledger + DurableSession)
    HUB-->>UI: SSE Done
```

**Notas.** Permissão sobre a rede é **fail-closed** (ADR 0017): expira → `Deny`. O
`SessionHub` garante ator único (ADR 0018) e faz replay snapshot-então-live (ADR 0016).
Toda mutação de matriz de permissão é auditada no ledger como override marcado.

---

## 6.3 Geração LLM através do stack de decorators (cache × rate-limit)

```mermaid
sequenceDiagram
    autonumber
    participant L as AgentLoop
    participant C as CachedGenerator
    participant R as RateLimitedGenerator
    participant G as Gateway
    participant P as Provedor (HTTPS)
    participant T as Telemetry

    L->>C: generate(req, on_delta)
    C->>C: request_hash (prompt-cache-key.v1)
    alt cache HIT
        C->>T: cache.hit
        C-->>L: replay do texto (NÃO toca rate-limit)
    else cache MISS
        C->>R: generate(req)
        R->>R: limiter.acquire() (tier-gated)
        alt limite excedido
            R-->>C: GatewayError::RateLimited
        else
            R->>G: generate(req)
            G->>P: POST /v1/messages | /chat/completions (SSE)
            P-->>G: chunks → TurnAggregator
            G-->>R: AssistantTurn
            R->>T: llm.call (tokens reais)
            R-->>C: turn
            C->>C: cache.put + cache.miss
        end
    end
```

**Notas.** A ordem dos decorators garante que um hit de cache nunca consuma vaga de
rate-limit nem token (o cache é o mais externo).

---

## 6.4 Pipeline `/verify` determinístico (job em background)

```mermaid
sequenceDiagram
    autonumber
    participant UI as SPA (Verify screen)
    participant AX as handlers::verify
    participant JOB as VerifyJobSlot
    participant VP as run_pipeline_with_progress
    participant EX as exec::run_with_timeout

    UI->>AX: POST /api/verify/run
    AX->>JOB: check-and-reserve (409 se já rodando)
    AX->>VP: spawn_blocking (config btv.toml ou default_steps)
    AX-->>UI: 202 {run_id}
    loop cada StepSpec (test/lint/fmt/sast)
        VP->>EX: subprocesso em process_group(0)
        alt timeout
            EX->>EX: kill(-pid, SIGKILL) (grupo inteiro)
            EX-->>VP: exit_code=124 + Finding sintético
        else
            EX-->>VP: Output + parser (cargo_test/clippy_json/ruff_json)
        end
        VP->>JOB: on_step → progresso (polling)
    end
    VP->>JOB: derive_verdict (Pass só se todos exit_code==0)
    UI->>AX: GET /api/verify/{id} (poll)
    AX-->>UI: done {evidence, ValueReview}
```

**Notas.** O kill de **grupo de processos** (`SIGKILL` em `-pid`) resolve a lição da Fase
4d: `uv run`/`cargo` re-forkam, e matar só o filho direto orfanaria netos. A mesma máquina
alimenta o skill-vetter (`Vet`/`Block`).

---

## 6.5 Append no ledger com hash-chain por tenant

```mermaid
sequenceDiagram
    autonumber
    participant APP as Serviço (squad/designer/session)
    participant LR as LedgerStore (LedgerRepository)
    participant DB as SQLite (WAL)

    APP->>LR: append(ctx, DomainEvent)
    LR->>DB: BEGIN IMMEDIATE (pega write-lock antes de ler)
    LR->>DB: SELECT topo da cadeia DO TENANT (seq, entry_hash)
    LR->>LR: prev_hash = topo; entry_hash = chain_hash(prev + corpo canônico)
    Note over LR: tenant entra no corpo hasheado (anti-transplante)
    LR->>DB: INSERT (tenant_id, seq+1, prev_hash, entry_hash, body)
    LR->>DB: COMMIT
    LR-->>APP: seq
```

**Notas.** `TransactionBehavior::Immediate` pega o write-lock **antes** de ler o topo da
cadeia, tornando o read-modify-write atômico entre conexões concorrentes (CLI/squad vs.
dashboard). Nunca há UPDATE/DELETE — overrides são novas entradas marcadas. No adapter
Postgres (`PgStore`) o mesmo append usa retry otimista sobre `UNIQUE(tenant_id, seq)`.
