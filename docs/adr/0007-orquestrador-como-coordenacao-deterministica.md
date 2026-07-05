# ADR 0007 — UnifiedOrchestrator: coordenação determinística, avaliador honesto

- Status: aceita
- Data: 2026-07-05

## Contexto

A Onda 4b porta `orchestration/unified_orchestrator.py` (a lineage
canônica, ADR 0004) — o capstone que compõe agentes, consenso, planner,
memória, paralelo, autonomia e avaliação. Diferente das ondas anteriores,
o orquestrador **não é raciocínio** e não deve chamar o gateway
diretamente: ele coordena os agentes que fazem. A régua "Nada Fake" aqui
é só não fabricar valores que ele repassa — e ele consome saída real de
agente/consenso/avaliação. Ainda assim, a inspeção encontrou fabricação
em um subsistema que ele depende (o avaliador) e um bug de serialização
que apagava um sinal importante.

## Decisão

### 1. As adaptações mapeadas (ADR 0004/0006) foram aplicadas

- Consenso via `ConsensusResult.requires_human`, não o
  `consensus.get("consensus_strength") < 0.7` manual da origem.
- Propostas dos agentes envolvidas em `Proposal(confidence=..., content=...)`
  antes de `reach_consensus` (que espera `dict[str, Proposal]` tipado).
- 5 agentes reais instanciados (architect/developer/auditor/designer/ops),
  cada um recebendo `attach_memory` **e** `attach_gateway`; o planner
  recebe `attach_gateway`; a autonomia recebe `attach_permission_client`.
- ADR 0006: `execute_with_autonomy` é só o portão; o orquestrador chama
  `record_action` com o resultado **real** após a execução (a menos que a
  rejeição humana já tenha retornado antes, caso em que o próprio portão
  registrou a falha).

`_select_agent_for_step`/`_can_parallelize`/`_extract_parallel_tasks`/
`_update_learning` são dispatch determinístico (plumbing) — portados
fielmente, sem chamada de gateway.

### 2. `ContinuousEvaluator` fabricava a nota técnica

Mesmo bug do `create_plan`/`_decompose_task`: na origem,
`evaluate_technical_quality` devolvia `result.get("technical_score", 0.8)`,
mas nenhum agente real produz um campo `technical_score` — então o valor
era **sempre** o default 0.8, e o portão de replanejamento do orquestrador
(`technical_score < 0.6`) nunca disparava. Corrigido: a nota técnica
deriva dos campos reais que os agentes reportam (`confidence` quando
`success`, senão 0.0), e `improvement` é o delta contra a média histórica
real de notas — não um default. `business_score` (fabricado como 0.7 na
origem, sem nenhum sinal de negócio no resultado do agente) foi removido
em vez de mantido fabricado.

### 3. `requires_human` sumia na serialização do consenso

`ConsensusResult.requires_human` é uma `@property`, e `model_dump()` do
pydantic serializa só **campos**, não properties. O retorno do
orquestrador expunha o consenso via `model_dump()`, então o sinal de HITL
— exatamente o que o painel do squad na TUI (Onda 4c) precisa mostrar —
era silenciosamente perdido no dict de resultado. Corrigido com um
helper `_consensus_dict` que injeta `requires_human` explicitamente ao
lado dos campos. Descoberto pelo teste (`KeyError`), não em produção.

## Consequências

- Onda 4b completa: `forge_squad.evaluation.ContinuousEvaluator` +
  `forge_squad.orchestrator.UnifiedOrchestrator`, testados com um
  `RoutingGatewayClient` (fake que roteia por `requester`, tornando o
  fluxo multi-agente determinístico) e `ScriptedPermissionClient`. 11
  testes novos (103 no total do workspace Python).
- Os testes provam a coordenação: 5 agentes instanciados; consenso forte
  dispensa HITL e completa; consenso fraco dispara o portão, aprovação
  segue e negação aborta com `success=False`; `record_action` registra o
  resultado real (trust do orquestrador 0.5→0.52 em sucesso, 0.5→0.4 em
  rejeição). Não re-provam a derivação de cada agente (já coberta nos
  testes de agente) — focam na coordenação.
- A Onda 4c precisa: (a) impls gRPC reais de `GatewayClient`/
  `PermissionClient` satisfazendo os Protocols; (b) o `SquadService`
  Python (servidor) expondo `ExecuteTask` que roda o orquestrador e
  streama `SquadEvent` (proposta/consenso/handoff/HITL); (c) o lado Rust
  consumindo isso no `forge squad` com o fallback de 3 níveis.
