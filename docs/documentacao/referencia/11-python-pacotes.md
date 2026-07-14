# 11 — Referência: os 4 pacotes Python

Workspace uv em `python/` (`pyproject.toml`, membros `packages/*`; dev deps `pytest`,
`grpcio-tools`). Diagrama de classes: ver
[05-classes §5.6](../diagramas/05-classes.md#56-squad-python--orquestrador-agentes-e-subsistemas-btv_squad).

**Fato transversal (o que sustenta tudo):**
- Python **serve** `SquadService` (`btv_squad/server.py`), `PromptForgeService`
  (`btv_promptforge/server.py`) e `MemoryService` (`btv_squad/memory_server.py`).
- Python **chama de volta** o Rust via `CoreService` (`CoreServiceStub`): `Generate` (LLM),
  `RequestPermission` (HITL), `RunTool`. **Nunca chama LLM direto.**

---

## btv-proto-py — stubs gRPC gerados

`python/packages/btv-proto-py/src/btv_proto/`. Gerado de `schemas/proto/*` (nunca editado à
mão). Dep externa: `grpcio`. Módulos `{core,llm,memory,promptforge,squad}_pb2[_grpc].py`.

- `core_pb2_grpc` — **CoreService** (Rust-served): `Generate`, `RunTool`, `AppendLedger`,
  `Recall`, `Remember`, `RequestPermission`.
- `squad_pb2_grpc` — **SquadService** (Python-served): `ExecuteTask` (server-stream), `Health`.
- `promptforge_pb2_grpc` — **PromptForgeService** (Python-served): `Health/Lint/Render/ListGenerators`.
- `memory_pb2_grpc` — **MemoryService** (Python-served): `Health/Recall/List`.

---

## btv-promptforge — prompts (generators, lint, hash)

`python/packages/btv-promptforge/src/btv_promptforge/`. Deps: `pydantic`, `grpcio`,
`btv-proto-py`.

- **`server.py`** — `PromptForgeServicer` (serve `PromptForgeService`; `async Health/Lint/
  Render/ListGenerators`). `serve(socket_path)`, `main()` (`--socket`).
- **`generators.py`** — `Field`/`Generator` (pydantic; `render(data)`), const `GENERATORS`
  (`code-review`, `bug-fix`).
- **`lint.py`** — `LintIssue`/`LintReport` (`@property grade` A/B/C/D), `lint_prompt`.
- **`hashing.py`** — **o twin Python de `btv-schemas::canonical`**: `canonical_json`,
  `sha256_hex`, `request_hash`, `validate_cache_key`, `CacheKeyError(ValueError)`,
  `_reject_forbidden_numbers`. Paridade via `schemas/fixtures/`. Puro stdlib.

---

## btv-review — review por valor

`python/packages/btv-review/src/btv_review/`. Deps: `pydantic`, `btv-promptforge` (só para
`canonical_json`/`sha256_hex`).

- **`reviewers.py`** — `technical_score(evidence)` (fração de passos `/verify` com
  `exit_code==0`), `security_score(evidence)`. Só 2 das 4 dimensões são deterministas;
  `performance`/`value` ficam ao chamador (honesto, sem fonte fabricada).
- **`score.py`** — `ReviewScores` (4 dims), `value_score` (média ponderada, `WEIGHTS`),
  `APPROVAL_THRESHOLD=0.7`.
- **`gates.py`** — `ReviewVerdict`, `evaluate(scores, evidence)`: gates duros em ordem
  (critical finding → verdict "fail" → `security < SECURITY_FLOOR(0.5)` → senão média).
  **É o "regras duras sobrepõem a média".**
- **`certification.py`** — `Certification`, `certify(...)`, `evidence_hash` (via
  `btv_promptforge.hashing`). Python só **produz**; o Rust registra no ledger.

---

## btv-squad — o sidecar multi-agente

`python/packages/btv-squad/src/btv_squad/`. Deps: `pydantic`, `grpcio`, `btv-proto-py`.
`docker` SDK é import opcional (`sandbox.py`).

### Servidores gRPC (superfícies servidas)

- **`server.py`** — `SquadServicer` (serve `SquadService`). `async ExecuteTask` **cria um
  canal UDS de volta para o Rust** (`grpc.aio.insecure_channel(..., options=[("grpc.default_
  authority","localhost")])`), constrói `GrpcGatewayClient`/`GrpcPermissionClient`/
  `GrpcToolClient`, instancia `UnifiedOrchestrator`, e bombeia os event dicts do orquestrador
  como `SquadEvent`. `_to_squad_event`, `_verification_evidence_from_request` (fail-closed).
  **É aqui que Python-serve-gRPC encontra Python-chama-Rust na mesma chamada.**
- **`memory_server.py`** — `MemoryServicer` (serve `MemoryService`, read-only sobre
  `AgentMemorySystem`; `Recall`→`recall_similar`, `List`→`_summarize_by_agent`). **Sem
  `Remember`** (só o orquestrador grava, em processo). `forgetting.py` foi removido.

### Clientes de volta ao Rust (`grpc_clients.py`)

Cada um embrulha `CoreServiceStub` e satisfaz um Protocol (ADR 0005):
`GrpcGatewayClient` (→`CoreService.Generate`), `GrpcPermissionClient` (→`RequestPermission`),
`GrpcToolClient` (→`RunTool`).

### Orquestrador e agentes

- **`orchestrator.py`** — `UnifiedOrchestrator` (`__init__` **compõe** planner/router/
  parallel/memory/evaluator/consensus/autonomy/sandbox/chain + dict de 5 agentes;
  `execute_complex_task`, `_get_squad_proposals`, `_execute_plan_steps`, `_select_agent_
  for_step`, `_attempt_recovery`).
- **`agents/base.py`** — `BaseAgent(ABC)` (`@abstractmethod async execute`; `system_with_
  persona`, `attach_memory/attach_gateway`). Os 5 chamam LLM só via `self.gateway.generate`:
  - `ArchitectAgent` (`reason_with_cot`, `create_plan`, `create_adr`)
  - `DeveloperAgent` (`implement_task(use_tools)` — **ReAct real** via `ToolClient.run_tool`,
    `_MAX_REACT_STEPS=12`)
  - `AuditorAgent` (`validate_results(evidence)`; gate duro `_claims_completion_without_write_
    evidence` reprova "completado" sem tool_call mutante antes de chamar o gateway)
  - `DesignerAgent`, `OpsAgent`.

### Subsistemas (alvos de composição)

`consensus.py` (`WeightedConsensusEngine`, `ConsensusResult.requires_human` `@property`
strength<0.7), `planning.py` (`AdaptivePlanner`), `hitl.py` (`ProgressiveAutonomyManager`,
`@dataclass`), `memory.py` (`AgentMemorySystem`, JSONL em `.btv/squad-memory`), `recall.py`
(`rank` TF-IDF cosine, puro stdlib), `routing.py` (`LearningRouter`), `evaluation.py`
(`ContinuousEvaluator`), `parallel.py` (`ParallelResourceManager`, semáforo), `chains.py`
(`ResilientPromptChain`).

### Contratos/Protocols (seams ADR 0005)

`gateway.py` (`GatewayClient` Protocol + `LlmRequest`/`LlmResponse` + `ScriptedGatewayClient`),
`permission.py` (`PermissionClient` + `Scripted*`), `tool_client.py` (`ToolClient` + `Scripted*`).
Em produção entram os `Grpc*`; em teste os `Scripted*` — nenhum agente muda.

### Espelhos de wire e segurança

`verification.py` (`VerificationEvidence` pydantic frozen, `from_proto`/`to_wire_dict`),
`tenant.py` (`TenantContext` frozen, `@field_validator`), `security.py` (`SecurityConfig`,
defesa em profundidade — Rust é a autoridade final), `sandbox.py` (`SecureToolSandbox`,
`DockerSandbox`), `_json.py` (`extract_json_object`).

---

## Threads de relação para diagramar

1. **gRPC bidirecional num fluxo:** `SquadServicer.ExecuteTask` → canal UDS ao Rust →
   `Grpc*Client` (`CoreServiceStub`) → injetados no `UnifiedOrchestrator` → agentes chamam
   `gateway.generate` = `CoreService.Generate`; autonomy chama `RequestPermission`; developer
   ReAct chama `RunTool`. Orquestrador emite event dicts → `_to_squad_event` → stream `SquadEvent`.
2. **Injeção por Protocol (ADR 0005):** `Scripted*` em teste, `Grpc*` em produção.
3. **Composição:** o orquestrador segura uma instância de cada subsistema + dict de 5 agentes.
4. **Herança:** 5 agentes → `BaseAgent(ABC)`; modelos → `pydantic.BaseModel`; 3 servicers →
   seus `*_pb2_grpc.*Servicer`.
5. **Import cross-package:** `btv-review.certification` → `btv_promptforge.hashing`.
