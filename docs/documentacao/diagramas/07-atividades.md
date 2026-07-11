# 07 — Diagramas de Atividades

Os dois workflows genuinamente concorrentes / com múltiplos caminhos do sistema.

---

## 7.1 Orquestração do squad com consenso, HITL e fallback progressivo

**Escopo:** `UnifiedOrchestrator.execute_complex_task` (Python) + a degradação de 3 níveis
do lado Rust (`drain_stream` → `SquadRun::Failed`).

```mermaid
flowchart TD
    A([Início: SquadTask + evidência /verify]) --> B[AdaptivePlanner.create_adaptive_plan]
    B --> C[_get_squad_proposals: 5 agentes propõem em paralelo]
    C --> D[WeightedConsensusEngine.reach_consensus]
    D --> E{requires_human?\nstrength &lt; 0.7}
    E -- sim --> F[HitlEscalation → RequestPermission]
    F --> G{ALLOW?}
    G -- não --> H[[Aborta / safe-mode]]
    G -- sim --> I
    E -- não --> I[_execute_plan_steps]
    I --> J{_can_parallelize?}
    J -- sim --> K[ParallelResourceManager.execute_parallel_with_limits]
    J -- não --> L[_select_agent_for_step → agent.execute]
    K --> M
    L --> M{StepResult.success?}
    M -- não --> N[_attempt_recovery → replan_from_point]
    N --> O{recuperou?}
    O -- não --> P[[Falha: SquadEvent Error]]
    O -- sim --> I
    M -- sim --> Q{mais passos?}
    Q -- sim --> I
    Q -- não --> R[AgentMemorySystem.remember_decision]
    R --> S([Fim: stream encerra → Completed])

    P -.->|drain_stream: Failed| T[Nível 2: run_once single-agent Rust]
    T -.->|falha| U[Nível 3: safe_mode read-only]
```

**Notas.** O consenso é ponderado por expertise (`DEFAULT_AGENT_WEIGHTS`); `requires_human`
é `@property` (strength < `HITL_ESCALATION_THRESHOLD = 0.7`). Passos independentes rodam
em paralelo sob semáforo. O `_attempt_recovery` fecha o ciclo de replanejamento adaptativo.
A degradação de 3 níveis (squad → agente-único → safe-mode) é decidida no Rust pelo
`drain_stream`: um `SquadEvent::Error` in-band ou um `Err(Status)` de transporte (ex.:
Python morto por `kill -9`) vira `SquadRun::Failed`.

---

## 7.2 Ciclo de execução de ferramenta sob permissões (AgentLoop)

**Escopo:** `btv-core::AgentLoop::continue_run` + `run_tool`.

```mermaid
flowchart TD
    A([turn do modelo]) --> B{stop_reason == ToolUse\ne tem tool_uses?}
    B -- não --> Z([EndTurn: retorna final_text])
    B -- sim --> C[para cada tool_use]
    C --> D{tools.get name?}
    D -- não --> E[tool_result: desconhecida is_error]
    D -- sim --> F[scope = tool.scope args]
    F --> G{PermissionEngine.evaluate}
    G -- Allow --> J[executa tool.run]
    G -- Deny --> H[ToolDenied → tool_result erro]
    G -- Ask --> I{resolver.resolve?}
    I -- não --> H
    I -- sim --> J
    J --> K{Ok?}
    K -- sim --> L[ToolFinished ok + diff\ntrunca se preciso]
    K -- não --> M[ToolFinished erro]
    E & H & L & M --> N[push tool_results]
    N --> O{step &lt; max_steps?}
    O -- sim --> A
    O -- não --> P([LoopError::MaxSteps])
```

**Notas.** O `scope` é **sempre recomputado** de `args` via `Tool::scope` — o campo `scope`
do wire nunca é a fonte de verdade para `Allow`/`Ask`/`Deny` (defesa contra um sidecar
comprometido). Output truncado ganha uma nota com o caminho do arquivo gerenciado antes de
voltar ao modelo.

---

## Justificativa das ausências

Não há diagrama de atividades para outros fluxos porque eles ou são lineares (não têm
ramificação/paralelismo significativo — ex.: `/verify` já está no [diagrama de
sequência 6.4](06-sequencia.md#64-pipeline-verify-determinístico-job-em-background) com sua
única decisão de timeout), ou são puramente request/response (CRUD de personas, templates,
usuários). Os dois workflows acima concentram toda a concorrência (paralelismo de passos,
consenso, HITL, recuperação, fallback) e os múltiplos caminhos de decisão do sistema.
