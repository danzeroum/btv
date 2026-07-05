# ADR 0004 — Lineage canônica do squad multi-agente (BuildToValue)

- Status: aceita
- Data: 2026-07-05

## Contexto

Antes de começar a Fase 4 (migração do squad multi-agente), inspecionamos
`src/` do `BuildToValue_AI_Agent_Specialization` arquivo a arquivo (via
GitHub API, sem clone) para montar o checklist de porte. A inspeção revelou
que o repositório contém **três orquestradores** e **duas hierarquias de
agente** que não coexistem por acidente — são gerações sucessivas da mesma
ideia, e só a mais recente está de fato ligada aos 5 agentes reais.

## Decisão

### Só um orquestrador é canônico

| Arquivo | Veredito | Evidência |
|---|---|---|
| `src/orchestration/unified_orchestrator.py` (`UnifiedOrchestrator`) | **Portar** | Instancia os 5 agentes reais, chama `WeightedConsensusEngine.reach_consensus`, `AdaptivePlanner`, `LearningRouter`, `ProgressiveAutonomyManager`, `AgentMemorySystem`, `ParallelResourceManager`, `SecureToolSandbox`, `ContinuousEvaluator` — bate exatamente com o fluxo do plano (recall→plano→propostas→consenso→execução→auditoria→memória→aprendizado→recuperação). |
| `src/orchestrator.py` (`AgentOrchestrator`) | **Descartar** | Usa `SafeAgentBase` (execução simulada, ver abaixo), `RAGTool` e `MCPServer` (ambos Fase 6) e `evaluation.continuous_eval.ContinuousEvaluator` (versão descartada, ver abaixo). Nenhum agente real o utiliza. |
| `src/protocols/squad_orchestrator.py` (`SquadOrchestrator`) | **Descartar** | Só compõe `ArchitectAgent` + `DeveloperAgent` via `reason_with_cot`/`react_loop`. Esses métodos já existem nos próprios agentes (confirmado em `architect_agent.py`) e são acessíveis pelo `UnifiedOrchestrator` via `.execute()` — nada se perde ao não portar este arquivo. |

### Só uma hierarquia de agente é usada de verdade

- **`src/agents/base_agent.py` (`BaseAgent`) é a ABC a portar.** Confirmado
  por leitura direta de `unified_orchestrator.py`: os 5 agentes
  instanciados (`ArchitectAgent`, `DeveloperAgent`, `AuditorAgent`,
  `DesignerAgent`, `OpsAgent`) herdam dela. Contrato: `__init__(agent_type)`
  gera `agent_id`/`created_at`, `confidence_threshold = 0.7`,
  `attach_memory()` injeta o backend de memória (o orquestrador chama isso
  para os 5 depois de instanciá-los), `execute()` é abstrato e assíncrono,
  `validate_confidence()`/`log_decision()` são helpers concretos.
- **`src/core/safe_agent_base.py` (`SafeAgentBase`) não tem nada para
  migrar como código.** Nenhum agente real herda dela. Seu `execute()`
  devolve literalmente `f"executed::{task}"` — é uma simulação, não uma
  chamada real. Seus guardrails (`InputSanitizer`, `OutputValidator`,
  `EthicalBoundaryChecker`, `ResourceLimiter`) são todos
  `def validate(self, pattern): return bool(pattern)` — sempre `True` para
  qualquer string não vazia. Não há lógica de segurança real ali.
  **Mesmo que houvesse, guardrails não pertenceriam ao Python** — o ADR
  0001 já decidiu que permissões são do Rust, não-contornáveis pelo
  sidecar (`forge-core::permission` + `forge-verify`). O único valor deste
  arquivo é a **taxonomia** (as 4 categorias de guardrail), útil como
  checklist de spec para a Fase 5 (`/verify`), não como código Python a
  portar na Fase 4.

### Colisão de nomes: dois `ContinuousEvaluator`

`src/evaluation/continuous_eval.py` e `src/evaluation/continuous_evaluator.py`
definem classes homônimas, mas são módulos diferentes com contratos
diferentes:

- `continuous_evaluator.py` — **canônico**. É o importado por
  `unified_orchestrator.py`. Assíncrono, por-agente:
  `evaluate_agent_performance(agent_name, task_result)` devolve
  `{technical_score, business_score, improvement}` e acumula métricas
  rolling (`task_success_rate`, `average_confidence`, `user_satisfaction`,
  `error_rate`, `response_time_p95`).
- `continuous_eval.py` — descartado (só usado pelo `orchestrator.py`
  descartado), avalia por trajetória inteira em vez de por agente. Vale
  resgatar a ideia das **3 dimensões de avaliação**
  (conclusão/eficiência/compliance) como entrada de design para os quality
  gates do `forge_review` (Fase 5) — não como código a portar agora.

### Achado adicional: são 5 agentes reais, não 8

O mapeamento 100% do plano (`docs/PLANO-PLATAFORMA-FORGE.md`, tabela da
seção "Mapeamento completo") lista "Architect/Dev/Auditor/Designer/Ops/
Supervisor/Exploration/Recovery" como a cobertura de ideias do BuildToValue.
Isso descreve o conjunto de arquivos que existem no repositório de origem,
não o conjunto que o `UnifiedOrchestrator` de fato instancia. Na prática:

- **5 agentes wireados**: `architect`, `developer`, `auditor`, `designer`,
  `ops` — os mesmos nomes usados em `WeightedConsensusEngine.agent_weights`
  (`forge_squad/consensus.py`, já migrado), o que confirma que a
  correspondência é 1:1 e nenhuma adaptação de nomenclatura é necessária.
- **`SupervisorAgent`** (903 bytes) **não é instanciado pelo
  `UnifiedOrchestrator`**, mas tem um chamador real: `src/main.py`, o único
  entrypoint `__main__` do repositório — um demo que injeta um
  orquestrador stub via `type("Orchestrator", (), {...})()` e uma lista de
  especialistas igualmente stub. Mais importante: `SupervisorAgent` **não
  segue o contrato de `BaseAgent`** — é um `@dataclass` solto (não herda de
  nada), com `run(task: str) -> str` (não o `execute(task: Dict)` async
  abstrato da ABC) e construtor `(orchestrator: Agent, specialists:
  list[Agent])`, delegando para `orchestrator.arun(task)` e depois para
  cada especialista via `agent.arun(summary)`. Ou seja, é desenhado como um
  **coordenador de nível superior sobre outros agentes/orquestradores**,
  não como um par que o `UnifiedOrchestrator` instancia ao lado dos 5
  reais. Adotá-lo é **design novo** (um supervisor sobre o squad), não
  porte — a interface nem é compatível com a que a Onda 2 vai portar.
- **`ExplorationAgent`** (488 bytes) e **`RecoveryAgent`** (499 bytes) não
  têm chamador algum, nem em `main.py` nem no orquestrador — a recuperação
  real é o método `UnifiedOrchestrator._attempt_recovery`, que reexecuta a
  tarefa simplificada (`priority: "low", simplified: True`) em vez de
  delegar a um agente dedicado. Portar isso é portar o método, não a
  classe `RecoveryAgent`.

### Adaptações de interface confirmadas para o porte (não são bugs a corrigir, são o trabalho da Onda 4)

1. `unified_orchestrator.py` chama `self.consensus.reach_consensus(proposals, "architecture")`
   e trata o retorno como **dict** (`consensus.get("consensus_strength", 0.0)`),
   com o limiar `0.7` hardcoded no próprio orquestrador. O
   `forge_squad/consensus.py` já migrado devolve um `ConsensusResult`
   pydantic com `.consensus_strength` **e a property `.requires_human`**
   (mesmo limiar, `HITL_ESCALATION_THRESHOLD = 0.7`, centralizado). Ao
   portar, o check vira `consensus.requires_human` — elimina a duplicação
   do número mágico `0.7` que hoje existe em três lugares
   (`BaseAgent.confidence_threshold`, o `< 0.7` do orquestrador, e o
   `HITL_ESCALATION_THRESHOLD` do consenso).
2. `reach_consensus` já migrado espera `dict[str, Proposal]` (pydantic),
   mas `unified_orchestrator.py` monta `proposals[key] = {"confidence":
   ..., "proposal": {...}}` como dict solto — o porte precisa envolver
   cada resultado de agente em `Proposal(confidence=..., content=...)`.

## Consequências

- A Onda 2 da Fase 4 porta exatamente `agents/base_agent.py` +
  5 agentes (`architect_agent.py`, `developer_agent.py`,
  `auditor_agent.py`, `designer_agent.py`, `ops_agent.py`) — não 8, e não
  `safe_agent_base.py`.
- A Onda 4 porta só `orchestration/unified_orchestrator.py` +
  `evaluation/continuous_evaluator.py`, com as duas adaptações de
  interface acima já mapeadas.
- `orchestrator.py`, `core/safe_agent_base.py`, `protocols/squad_orchestrator.py`,
  `evaluation/continuous_eval.py`, `tools/rag_tool.py` e
  `protocols/mcp_server.py` **não são portados na Fase 4** —
  `rag_tool.py`/`mcp_server.py` ficam para a Fase 6 (RAG, MCP completo),
  os demais são lineage superada sem uso real.
- `exploration_agent.py`/`recovery_agent.py` ficam como candidatos a
  **trabalho novo**, não migração, se decidirmos dar ao squad um
  explorador/recuperador dedicados no futuro — não bloqueiam a Fase 4.
  `supervisor_agent.py` também não bloqueia a Fase 4, mas por um motivo
  mais específico: sua interface (`run()`, construtor
  `orchestrator`/`specialists`, sem herdar de `BaseAgent`) é
  incompatível com a dos 5 agentes que a Onda 2 vai portar — não é "só"
  trabalho novo, é uma decisão de arquitetura separada (um coordenador
  acima do squad) a tomar quando/se ela surgir.
