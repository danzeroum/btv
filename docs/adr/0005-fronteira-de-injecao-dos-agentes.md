# ADR 0005 — Fronteira de injeção dos agentes: GatewayClient e review_system opcional

- Status: aceita
- Data: 2026-07-05

## Contexto

A Onda 2 da Fase 4 (ADR 0004) precisa reescrever os 5 agentes reais para
chamar LLM via `CoreService.Generate` em vez de heurísticas locais — o
princípio "Nada Fake". Dois obstáculos de sequenciamento, confirmados
diretamente no código:

1. **`core.proto`/`llm.proto` ainda não têm stubs gerados.**
   `crates/forge-proto/build.rs` e `scripts/gen_proto_py.py` só compilam
   `promptforge.proto` hoje (`PROTOS = ["promptforge.proto"]`); a ativação
   do `CoreService`/`SquadService` é explicitamente escopo da Onda 4. Se os
   agentes da Onda 2 dependessem do client gRPC real, ficariam bloqueados
   até a Onda 4 existir — na ordem errada.
2. **`agents/developer_agent.py` importa `BuildToValueReviewSystem` de um
   pacote externo `buildtovalue`** (`from buildtovalue import
   BuildToValueReviewSystem`, instanciado no `__init__`). Portar fielmente
   arrastaria o review system (Fase 5, `forge_review`, ainda não existe)
   para dentro de um agente da Fase 4.

## Decisão

### 1. `forge_squad.gateway.GatewayClient` — Protocol desacoplado do transporte

```python
class GatewayClient(Protocol):
    async def generate(self, request: LlmRequest) -> LlmResponse: ...
```

`LlmRequest`/`LlmResponse` são modelos pydantic que espelham
`forge.llm.v1.LlmRequest`/`LlmChunk` (`schemas/proto/llm.proto`) — model,
mensagens, temperature, max_tokens, requester (nome do agente, para
telemetria/rate limiting no lado Rust) — mas não são as classes geradas
pelo protobuf. Mesmo princípio já usado em `consensus.py`: `Proposal`/
`ConsensusResult` são pydantic, desacoplados do `squad.proto` que os
carrega no wire. Quando a Onda 4 ativar o gRPC de verdade, um
`GrpcGatewayClient` concreto implementa o mesmo Protocol — os agentes não
mudam uma linha.

A construção exata de `messages_json` (envelope canônico
`prompt-cache-key.v1`, paridade com `forge_schemas::request_hash`) fica
para a Onda 4, quando o `GrpcGatewayClient` de fato serializa para o
wire. Por ora `LlmRequest.messages` é uma lista simples de `{"role",
"content"}`.

### 2. `ScriptedGatewayClient` — fake determinístico para testes

Mesmo princípio do "gerador roteirizado" já usado nos testes Rust do loop
de agente (`forge-core`): uma fila de `LlmResponse` pré-programadas,
consumida em ordem, que levanta `AssertionError` se o agente pedir mais
chamadas do que o teste programou. Testes da Onda 2 usam isso, não um
mock genérico — fica óbvio quando um agente chama o gateway mais (ou
menos) vezes do que o esperado.

### 3. `BaseAgent.attach_gateway(gateway)` — mesmo padrão de `attach_memory`

`BaseAgent` (portado de `agents/base_agent.py`) ganha `self.gateway:
Optional[GatewayClient] = None` e `attach_gateway()`, injetado
preguiçosamente pelo orquestrador (Onda 4), no mesmo lugar onde hoje
`UnifiedOrchestrator.__init__` já chama `agent.attach_memory(self.memory)`
para os 5 agentes.

### 4. `review_system` vira dependência opcional injetada

`DeveloperAgent.__init__` ganha `review_system: Optional[ReviewSystem] =
None` em vez de instanciar `BuildToValueReviewSystem` diretamente.
`generate_code()` pula a chamada de review quando `review_system` é
`None` e devolve o código gerado sem revisão — o wiring real acontece
quando `forge_review` existir (Fase 5), sem a Onda 2 esperar por ela.

## Consequências

- A Onda 2 fica testável e completa por si mesma, sem depender da Onda 4
  (gRPC) nem da Fase 5 (`forge_review`) existirem primeiro.
- `ArchitectAgent` é portado nesta ADR como implementação de referência:
  `reason_with_cot` na origem era 100% heurística fixa (os "passos" de
  Chain-of-Thought eram literais constantes, independentes do problema
  recebido) — a versão portada chama `self.gateway.generate(...)` de
  verdade, pedindo ao modelo um JSON estruturado
  (`problem_analysis`/`constraints`/`applicable_patterns`/`trade_offs`/
  `recommendation`/`confidence`), com fallback defensivo (confiança 0.0)
  se o parsing falhar — nunca lança exceção para uma resposta mal-formada
  do modelo. `create_plan`/`create_adr` continuam como transformação
  determinística sobre o resultado do raciocínio (não chamam o gateway de
  novo) — não são "fake", são bookkeeping mecânico sobre uma decisão que
  agora é real.
- Os outros 4 agentes (`developer`/`auditor`/`designer`/`ops`) seguem o
  mesmo padrão (`self.gateway.generate(...)` + parsing defensivo) em
  trabalho subsequente da Onda 2 — não portados nesta ADR.
