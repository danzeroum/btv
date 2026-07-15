# 13 — Referência: contratos gRPC e JSON Schemas

A camada de cola entre Rust, Python e TypeScript. Fonte única em `schemas/`.

---

## 13.1 Serviços gRPC (`schemas/proto/*.proto`)

Convenção de pacote: `btv.<domínio>.v1`. Compilados no Rust por `crates/btv-proto/build.rs`
(tonic-build + protoc vendorizado); stubs Python por `scripts/gen_proto_py.py` (grpcio-tools).
Transporte: **gRPC sobre Unix Domain Socket**. `buf.yaml` impõe checagem de breaking
(aditivo-only).

### `core.proto` — `CoreService` · **Rust SERVE, Python CHAMA**

Keys, permissões, disco, ledger só no Rust. Importa `llm.proto`.

| RPC | Request | Response | Streaming |
|---|---|---|---|
| `Generate` | `LlmRequest` | `LlmChunk` | **server-stream** |
| `RunTool` | `ToolCall` | `ToolResult` | unário |
| `RequestPermission` | `PermissionRequest` | `PermissionDecision` | unário (HITL) |

> Os RPCs `AppendLedger`/`Recall`/`Remember` (e suas mensagens) foram REMOVIDOS do
> `CoreService` (ADR 0034 — 2ª quebra de wire assinada): eram stubs `Unimplemented` na
> direção errada, superados pelo `MemoryService` (ADR 0022).

- `ToolCall{tool, args_json, scope}` — o Rust **sempre recalcula** o scope real de `args_json`
  via `Tool::scope`; o campo `scope` nunca é fonte de verdade para Allow/Ask/Deny.
- `ToolResult{content, truncated, exit_code}` — `0`=sucesso, `1`=erro de exec, `-1`=negado.
- `PermissionRequest{tool, scope, reason, confidence}` — `confidence < 0.3/0.5` dispara HITL.
- `PermissionDecision{Decision(UNSPECIFIED/ALLOW/DENY), operator_note?}`.

### `llm.proto` — tipos compartilhados (sem serviço)

`LlmRequest{model, messages_json (JSON canônico, prompt-cache-key.v1), temperature?, max_tokens?,
requester}`. `LlmChunk{oneof: text_delta | Usage | error}`. `Usage{input_tokens, output_tokens,
cache_hit, provider}`.

### `promptforge.proto` — `PromptForgeService` · **Python SERVE, Rust CHAMA**

`Health`, `Lint(LintRequest)→LintReport{score,grade,issues}`, `Render(RenderRequest{generator,
map fields})→RenderResponse`, `ListGenerators→ListGeneratorsResponse{GeneratorInfo[]}`. Não
gera texto de LLM (exclusivo do gateway Rust).

### `squad.proto` — `SquadService` · **Python SERVE, Rust CHAMA**

| RPC | Request | Response | Streaming |
|---|---|---|---|
| `ExecuteTask` | `SquadTask` | `SquadEvent` | **server-stream** |
| `Health` | `HealthRequest` | `HealthResponse` | unário |

- `SquadTask{task_id, description, decision_type, verification_evidence (TIPADA — breaking
  assinado, ADR 0030), model, roster: PersonaSpec[] (U7), tenant_id, actor}`. A tag 4
  (`max_autonomy_level`) foi REMOVIDA e reservada (ADR 0033 — 2ª quebra de wire assinada;
  era ignorada ponta-a-ponta, ADR 0021).
- `PersonaSpec{papel, prompt, funcao(plan|produce|review|validate|deliver), ordem, custom}`.
- `SquadEvent{task_id, ts, tenant_id, actor, oneof: Proposal|Consensus|Handoff|HitlEscalation|
  StepResult|error|ChatMessage}`.
- `Proposal{agent, confidence, content_json}`, `Consensus{decision_maker, strength,
  decision_json, requires_human}`, `Handoff{Phase(START/ACK/COMPLETE/ERROR), from/to_agent,
  contract, payload_digest}`, `ChatMessage{author, author_role(AGENT/HUMAN/SYSTEM), text,
  in_reply_to}`.
- `VerificationEvidence{run_id, git_sha, VerificationStep[], Verdict(UNSPECIFIED/PASS/FAIL/
  SKIPPED), produced_at}` (espelha o JSON schema; UNSPECIFIED = fail-closed).

### `memory.proto` — `MemoryService` · **Python SERVE, Rust CHAMA** (direção OPOSTA de CoreService)

Python é dono do corpus JSONL + índice TF-IDF (`btv_squad`). `Health`, `Recall(RecallRequest
{query, k})→RecallResponse{MemoryMatch[]}` (TF-IDF **léxico, não semântico**),
`List(ListRequest{agent?, limit})→ListResponse{MemorySummary[]}`. Escrita de memória só em
processo (`AgentMemorySystem.remember_decision`), nunca pela rede — por isso os antigos
`CoreService.Recall/Remember` (direção errada) foram REMOVIDOS (ADR 0034).

---

## 13.2 JSON Schemas (`schemas/json/*.v1.schema.json`)

Todos draft 2020-12, `$id` sob `https://btv.buildtovalue.dev/schemas/`. Fixtures em
`schemas/fixtures/`.

| Arquivo | Contrato | Modela | Consumido por |
|---|---|---|---|
| `prompt-cache-key.v1` | cache-key | sha256 de JSON canônico de `{messages, temperature}`; v1 rejeita floats de fração zero (ADR 0032) | **Rust + Python (dual)** |
| `verification-evidence.v1` | evidência /verify | `{run_id, git_sha, steps[], verdict, produced_at}` | **Rust (btv-verify) → Python auditor** (+ tipada no squad.proto) |
| `squad-template.v1` | modelo de galeria | `{id, nome, categoria, cor, onda, papeis[≤8], formatos, perguntas, gates}` | **Rust (embutido) → SPA TS** |
| `experiment.v1` | relatório A/B | z-tests pareados + Bonferroni; verdito honesto | **Rust-only** (ADR 0014) |
| `squad-workflow.v1` | grafo do Designer | `{nodes[], edges[]}` | **TS (Designer) → Rust (valida+ledger)** |
| `ledger-entry.v1` | entrada do ledger | `{seq, prev_hash, entry_hash, kind, actor, payload, override, fake_marker, ts, tenant?}` | **Rust (btv-store)** |
| `persona.v1` | persona como conteúdo | `{id, display_name, domain, mental_models, core_principles, autonomy(L1-L5 descritivo), ...}` | **Rust + Python**; TS (U7) |
| `plan.v1` | manifesto de entrega | `{prerequisites, execution_sequence, success_criteria, budget, rollback_strategy}` | **Python (planning)** |
| `prompt-template.v1` | generator declarativo | `{name, category, fields}` | **Python (PromptForge)** |
| `telemetry-event.v1` | evento offline-first | `{name, session_id, props, ts}` | **Rust (btv-store)**; TS lê agregados |
| `handoff-event.v1` | handoff entre agentes | `{event, task_id, from/to_agent, contract, payload_digest, ts, error}` | **Python (matriz)** |

Fixtures HTTP golden: `schemas/fixtures/http/*.golden.json` (deliverables, ledger, personas,
squad_activation, templates, …) + `wire-strings.v1.json`.

---

## 13.3 Os 12 templates (`schemas/squad-templates/`)

`bi · design · editorial · educacao · imagem · juridico · musica · operacoes · pesquisa ·
podcast · sales · video` (`.json`). Cada um é uma instância `squad-template.v1` (papéis,
formatos com flag `binario`, perguntas do wizard, gates, categoria, cor, onda, versão,
publicado). **Embutidos no binário Rust** via `include_str!` em `btv-schemas/src/squad_template.rs`;
servidos em `GET /api/btv/templates`. Consumidos pela galeria (U1), wizard (U2), admin (A5).
Personas humanas relacionadas em `schemas/personas/<template>/*.json` (instâncias `persona.v1`).

---

## 13.4 O contrato de hash dual — `prompt-cache-key.v1`

Duas implementações paralelas de um algoritmo, em paridade por fixtures:
- **Rust:** `crates/btv-schemas/src/canonical.rs` — `request_hash`, `CacheKeyError::NumeroProibido`.
- **Python:** `python/packages/btv-promptforge/src/btv_promptforge/hashing.py` — `request_hash`,
  `CacheKeyError(ValueError)`.

Algoritmo: sha256 hex (minúsculo) do JSON canônico de `{"messages": ..., "temperature": ...}`.
Canônico = chaves ordenadas em todos os níveis, separadores compactos `(",",":")`, UTF-8 cru
(`ensure_ascii=False`). **Restrição numérica v1 (ADR 0032, agora ENFORÇADA):** floats com
fração zero (`1.0`) e não-finitos são rejeitados — JS emite `1`, Rust/Python emitem `1.0`, o
que divergiria entre produtores. Paridade verificada por `schemas/fixtures/prompt-cache-key.v1.json`
(inclui `reject_cases`); testes de paridade devem passar nos DOIS lados. Bench criterion:
`cargo bench -p btv-schemas --bench canonical`.
