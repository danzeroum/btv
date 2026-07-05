# ADR 0006 — Planejamento real e HITL como portão, não executor

- Status: aceita
- Data: 2026-07-05

## Contexto

A Onda 3 da Fase 4 (ADR 0004) porta `planning`/`parallel`/`hitl`. Antes de
portar, a mesma checagem de "tem chamador real?" do ADR 0004 foi repetida
— e revelou duas colisões de nome adicionais na origem, mais um problema
de design no HITL que ecoa o mesmo risco já tratado no `AuditorAgent`
(ADR 0005).

## Decisão

### 1. `planning/adaptive_planner.py::AdaptivePlanner` é o único real

`src/planning/` tem **duas classes chamadas `AdaptivePlanner`** em
arquivos diferentes, e uma terceira classe de planejamento não
relacionada:

| Arquivo | Veredito | Evidência |
|---|---|---|
| `planning/adaptive_planner.py::AdaptivePlanner` | **Portar** | É o importado por `unified_orchestrator.py` (`from src.planning.adaptive_planner import AdaptivePlanner`); tem `create_adaptive_plan`/`replan_from_point`, os métodos que o orquestrador de fato chama. |
| `planning/adaptive_replanner.py::AdaptivePlanner` | **Descartar** | Mesmo nome de classe, interface totalmente diferente (`execute_with_replanning`). Sem chamador em lugar nenhum do repositório — e seu `_execute_plan` é um placeholder que sempre devolve `{"success": True}`, nunca uma execução real. |
| `planning/hierarchical_planner.py::HierarchicalPlanner` | **Descartar** | Sem chamador. `decompose_goal`/`generate_alternatives`/`evaluate_paths` são 100% fixos (confiança sempre 0.7/0.6/0.5/0.8, "alternativas" geradas por concatenação de string). |

Mesmo padrão do ADR 0004 (`SafeAgentBase`/`SquadOrchestrator` descartados)
e da Onda 1 (`utils/observability.py`/`utils/tool_utils.py`/`safety/guardrails.py`
descartados): confirmado por leitura direta do código-fonte, não por
suposição.

### 2. `AdaptivePlanner._decompose_task` era o mesmo bug do `create_plan`

Na origem, `_decompose_task` devolvia passos com descrições **fixas**
("Analyze requirements and constraints", "Implement solution"...) para
qualquer tarefa — só a contagem de passos variava com
`task.get("priority") == "high"`. `_calculate_plan_confidence` era uma
fórmula fixa (`0.8` menos penalidades por tamanho/duração). Exatamente o
bug já corrigido no `ArchitectAgent.create_plan` (ADR 0005) — decomposição
de tarefa é decisão real, não bookkeeping. `forge_squad.planning.AdaptivePlanner`
pede ao gateway a decomposição de verdade (`create_adaptive_plan`) e os
passos de recuperação (`replan_from_point`), testado com o padrão de
igualdade (duas tarefas diferentes ⇒ dois planos com valores exatos
diferentes, não só `!=`).

`analyze_failure` (classificação de exceção por substring — "timeout"/
"memory"/desconhecido) continua determinístico — é evidência de entrada
para o replanejamento, não uma decisão fabricada, mesmo espírito do
`AuditorAgent.check_security`/`check_quality`.

### 3. `parallel/resource_manager.py::ParallelResourceManager` fica determinístico

Infraestrutura legítima (semáforo + `asyncio.gather`) — plumbing, não
raciocínio. Portado fielmente, sem nenhuma chamada de gateway. Forçar
uma decisão de LLM sobre "quantas tarefas rodar em paralelo" seria
fabricar uma decisão onde já existe resposta mecânica correta — mesmo
princípio do `sandbox`/`security` da Onda 1.

### 4. `ProgressiveAutonomyManager`: dois placeholders escondiam fabricação

`hitl/progressive_autonomy.py` tinha dois pontos de fabricação:

- `_request_human_approval` sempre devolvia `{"approved": True}` — um
  carimbo automático de aprovação humana, o mesmo risco já enfrentado no
  veredito do `AuditorAgent`.
- `_execute_action` sempre devolvia `{"success": True}` — mas o único
  chamador real (`UnifiedOrchestrator.execute_complex_task`) **nunca lê
  esse resultado**, só verifica `approval.get("executed")` como portão
  booleano; a execução de verdade acontece depois, em
  `_execute_plan_steps`. Era fabricação morta: nem alimentava uma decisão
  real, só existia por simetria de código.

Corrigido com o mesmo padrão do `GatewayClient` (ADR 0005), porque
`core.proto`/`RequestPermission` também não tem stubs gerados ainda (só
`promptforge.proto` compila hoje — mesmo obstáculo de sequenciamento):
`forge_squad.permission.PermissionClient` (`Protocol` async
`request_permission(PermissionRequest) -> PermissionDecision`, pydantic,
espelhando `core.proto`) + `ScriptedPermissionClient` (fake roteirizado).

Além disso, **`execute_with_autonomy` virou só o portão** — decide se a
ação precisa de aprovação e, se precisar, pergunta de verdade via
`PermissionClient`; não finge executar nada. `record_action` (que ajusta
o score de confiança) fica público, para quem de fato executa a ação
chamar depois com o resultado real — separando "decidir se pode" de
"fazer", mesmo espírito do `PermissionResolver` do `forge-core` em Rust
(decide sim/não; quem roda a ferramenta é outra camada). Isso muda o
contrato original (a origem atualizava o score de confiança dentro do
próprio portão, com um resultado fabricado) — mudança deliberada, não
uma migração literal, porque preservar o contrato original teria
preservado a fabricação.

## Consequências

- Onda 3 completa: `forge_squad.planning.AdaptivePlanner`,
  `forge_squad.parallel.ParallelResourceManager`,
  `forge_squad.hitl.ProgressiveAutonomyManager` +
  `forge_squad.permission.{PermissionClient,ScriptedPermissionClient}`.
- 19 testes novos (92 no total do workspace Python), padrão de igualdade
  mantido para tudo que deriva do gateway.
- A Onda 4 (`UnifiedOrchestrator` + `SquadService` real) precisa: (a)
  implementar `GrpcGatewayClient`/`GrpcPermissionClient` concretos
  satisfazendo os Protocols já definidos; (b) ajustar o call site do
  orquestrador para chamar `record_action` com o resultado real da
  execução, já que o portão não faz mais isso sozinho.
