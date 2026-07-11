# 12 — Máquinas de estado

Os estados e transições dos tipos que os têm. Fonte anotada por diagrama.

---

## 12.1 `RunStatus` — ciclo de vida de uma squad

Fonte: `crates/btv-domain/src/ports.rs` (`RunStatus::can_transition_to`). A mutação só
acontece pelo agregado (`Run::transition_to`); transição inválida retorna
`RunError::InvalidTransition` **sem** mudar o estado.

```mermaid
stateDiagram-v2
    [*] --> Ativa : Run::activate
    Ativa --> Concluida : tarefa entregue
    Ativa --> Encerrada : encerramento / reconciliação de zumbi
    Ativa --> Erro : falha
    Concluida --> [*]
    Encerrada --> [*]
    Erro --> [*]
    note right of Ativa
        approve_gate só é válido em Ativa
        (incrementa gates_aprovados)
    end note
    note right of Concluida
        Estados terminais: nenhuma transição de saída.
        Concluida→Ativa retorna InvalidTransition.
    end note
```

---

## 12.2 Decisão de permissão de ferramenta

Fonte: `crates/btv-core/src/permission.rs` (`PermissionEngine::evaluate`) +
`agent_loop.rs` (`run_tool`). `Ask` delega ao `PermissionResolver` (CLI: stdin; web:
`WebPermissionResolver` fail-closed).

```mermaid
stateDiagram-v2
    [*] --> Avaliando : tool_use pedido
    Avaliando --> Allow : regra compatível = Allow
    Avaliando --> Deny : regra compatível = Deny
    Avaliando --> Ask : sem regra (default)
    Ask --> Allow : resolver = true
    Ask --> Deny : resolver = false / timeout (fail-closed)
    Allow --> Executa : tool.run(args)
    Deny --> ToolDenied : tool_result is_error
    Executa --> [*]
    ToolDenied --> [*]
```

---

## 12.3 Fase de handoff entre agentes

Fonte: `schemas/proto/squad.proto` (`Handoff.Phase`) + `btv-schemas::handoff`.

```mermaid
stateDiagram-v2
    [*] --> START : from_agent inicia
    START --> ACK : to_agent aceita o contrato
    ACK --> COMPLETE : entrega o payload_digest
    ACK --> ERROR : falha durante a execução
    START --> ERROR : recusa / erro imediato
    COMPLETE --> [*]
    ERROR --> [*]
```

---

## 12.4 Veredito do `/verify`

Fonte: `crates/btv-schemas/src/verification.rs` (`Verdict`, `derive_verdict`).

```mermaid
stateDiagram-v2
    [*] --> Executando : run_pipeline
    Executando --> Pass : todos os steps exit_code==0
    Executando --> Fail : qualquer step != 0 (inclui timeout=124)
    Executando --> Skipped : nenhum step aplicável
    Pass --> [*]
    Fail --> [*] : btv verify sai com codigo != 0
    Skipped --> [*]
    note right of Fail
        UNSPECIFIED no wire = tratado como fail-closed
    end note
```

---

## 12.5 Degradação progressiva do squad (3 níveis)

Fonte: `crates/btv-cli/src/squad.rs` (`run_squad`) + `btv-sidecar::drain_stream`
(`SquadRun::{Completed|Failed}`).

```mermaid
stateDiagram-v2
    [*] --> Nivel1_Squad : try_squad (Python multi-agente)
    Nivel1_Squad --> Concluido : stream Completed
    Nivel1_Squad --> Nivel2_Agente : SquadEvent Error OU transporte quebrado (kill -9)
    Nivel2_Agente --> Concluido : run_once (single-agent Rust)
    Nivel2_Agente --> Nivel3_SafeMode : falha
    Nivel3_SafeMode --> Concluido : read-only, sem escrita
    Concluido --> [*]
```

---

## 12.6 Ciclo de vida do sidecar supervisionado

Fonte: `crates/btv-sidecar/src/{supervisor.rs, service.rs}` (ADR 0019).

```mermaid
stateDiagram-v2
    [*] --> Spawning : spawn (uv run -m ...) em process_group(0)
    Spawning --> WaitReady : socket criado
    WaitReady --> Ready : health() = (true, version)
    WaitReady --> Dead : processo morreu antes do prazo
    Ready --> Serving : atende RPCs
    Serving --> Dead : crash / kill
    Dead --> Spawning : restart-on-crash (SidecarService/MemoryService)
    Serving --> [*] : Drop → kill(-pid, SIGKILL) do grupo
    note right of Ready
        SquadPool: um processo por slot,
        gating por semáforo (SquadLease)
    end note
```

---

## 12.7 Sessão de código web (ator único)

Fonte: `crates/btv-cli/src/web_agent.rs` (`SessionHub`, ADRs 0016/0018).

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Busy : try_start (POST /message)
    Idle --> Rejeitada409 : outra sessão já ativa (ator único)
    Busy --> AguardandoPermissao : tool_use = Ask
    AguardandoPermissao --> Busy : resolve (allow/deny)
    AguardandoPermissao --> Busy : timeout → Deny (fail-closed)
    Busy --> Idle : finish_busy (Done, dual-persist)
    Rejeitada409 --> Idle
```
