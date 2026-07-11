# 14 — Referência: endpoints HTTP (REST + SSE)

A fronteira navegador↔Rust. Tudo bind em `127.0.0.1`; toda rota mutável passa por
`require_local_origin` (403 em `Origin` não-local para não-GET, ADR 0015). SSE = streaming
(`text/event-stream`). Handlers Rust em `btv-server` (dashboard) e `btv-cli`
(web_agent/squad_agent/btv_agent/consoles).

---

## 14.1 Sessão de código pelo navegador (`btv-cli/src/web_agent.rs`)

| Método + rota | Handler | Consumidor TS | Descrição |
|---|---|---|---|
| **SSE** `GET /api/session/{id}/events` | `sse_handler` | `web` `api/stream.ts::connectSessionEvents` | Stream de `SessionEvent` (snapshot-then-live) |
| `POST /api/session/{id}/message` | `send_message_handler` | `SessionContext::sendMessage` | `{message, model?, agent?}` |
| `POST /api/session/{id}/permission` | — | `SessionContext::resolvePermission` | `{request_id, allow}` (fail-closed timeout→Deny) |
| `GET /api/permissions/matrix` | `get_matrix_handler` | `web` `api/permissions.ts::fetchMatrix` | matriz build/plan × tool |
| `GET\|POST /api/permissions/rules` | `list_rules_handler`/`set_rule_handler` | `listRules`/`setRule` | overrides persistidos (auditados) |
| `DELETE /api/permissions/rules/{id}` | `revoke_rule_handler` | `revokeRule` | revoga override |

## 14.2 Squad ao vivo (`btv-cli/src/squad_agent.rs`)

| Método + rota | Consumidor TS | Descrição |
|---|---|---|
| `POST /api/squad/run` | `web`/`btv-web` `api/squad.ts::runSquad` | inicia squad `{task, model?}` → `{task_id}` |
| **SSE** `GET /api/squad/{task_id}/events` | `connectSquadEvents` | stream `SquadEventEnvelope` (finito, fecha no fim) |
| `POST /api/squad/{task_id}/hitl` | `resolveHitl` | `{allow}` |
| `POST /api/squad/{task_id}/message` | `postSquadMessage` | chat humano-como-membro (202 sem corpo) |
| `POST /api/squad/{task_id}/emergency-stop` | `emergencyStopSquad` | kill-switch |

## 14.3 Produto BuildToValue (`btv-cli/src/btv_agent.rs`, 22 rotas)

| Método + rota | Consumidor TS (`btv-web/api/btv.ts`) | Descrição |
|---|---|---|
| `POST\|GET /api/btv/squads` | `ativarSquad`/`listRuns` | ativar (`{template_id, briefing[], papeis_off[]}` → `{task_id, run_id}`) / listar `BtvRun` |
| `POST /api/btv/squads/{id}/gate` | `aprovarGate` | aprova gate `{etapa}` (`btv.gate_approved`) |
| `POST /api/btv/squads/{id}/ajuste` | `pedirAjuste` | "pedir ajuste" `{instrucao, etapa}` → cockpit (`btv.adjust_requested`) |
| `GET /api/btv/deliverables` | `listDeliverables` | lista `BtvDeliverable` |
| `GET /api/btv/deliverables/{id}/download` | `deliverableDownloadUrl` | download |
| `GET /api/btv/personas/{template_id}` | `fetchPersonas` | views + custom personas |
| `PUT\|DELETE /api/btv/personas/{template_id}/{papel}` | `setPersonaOverride`/`restorePersona` | override / restaura papel |
| `DELETE /api/btv/personas/{template_id}` | `restoreAllPersonas` | restaura todos |
| `POST\|PUT\|DELETE /api/btv/personas/{template_id}/custom[/{id}]` | `createCustomPersona`/`updateCustomPersona`/`deleteCustomPersona` | CRUD de persona própria |
| `POST /api/btv/designer/flows` | `Designer.tsx` (inline) | valida `squad.workflow.v1` + ledger (`btv.flow_saved`) → `{seq, diagram_sha256}` |
| `GET\|POST /api/btv/templates/publicacao`, `.../{id}/publicacao` | `fetchPublicacao`/`setPublicacao` | A5 publicação de template |
| `GET\|POST /api/btv/users`, `.../{id}/ativo`, `.../{id}/pin`, `.../{id}/verify-pin`, `DELETE .../{id}` | `fetchUsers`/`createUser`/`setUserAtivo`/`setUserPin`/`verifyUserPin`/`deleteUser` | A6 perfis + PIN |

## 14.4 Dashboard / admin (`btv-server/src/lib.rs` + handlers)

| Método + rota | Handler | Consumidor TS | Descrição |
|---|---|---|---|
| `GET /api/summary` | `telemetria::summary` | `admin.ts::fetchSummary` / `telemetry.ts::getSummary` | resumo de telemetria |
| `GET /api/events` | `telemetria::events` | `telemetry.ts::getEvents` | eventos recentes |
| `GET /api/models/usage` | `telemetria::model_usage` | `fetchModelUsage` | uso por modelo + custo estimado |
| `GET /api/skills` | `admin::skills` | `skills.ts::fetchSkills` | status do vetter |
| `GET /api/experiment/{nome}` | `admin::experiment` | `experiments.ts::fetchExperiment` | relatório A/B (z-test) |
| `GET /api/ratelimit` | `admin::rate_limits` | `ratelimit.ts::fetchRateLimits` | caps por tier |
| `GET /api/providers` | `providers::list_providers` | `providers.ts::fetchProviders` | providers configurados |
| `GET\|POST /api/prompts`, `.../{id}/favorite`, `DELETE .../{id}` | `prompts::*` | `prompts.ts::*` | biblioteca de prompts |
| `GET /api/ledger` | `ledger::list_ledger` | `ledger.ts::getLedger` / `admin.ts::fetchLedger` | ledger paginado |
| `POST /api/ledger/verify` | `ledger::verify_ledger` | `verifyChain`/`verifyLedger` | recomputa hash-chain |
| `POST /api/verify/run`, `GET /api/verify/{id}` | `verify::*` | `verify.ts::startVerifyRun`/`fetchVerifyStatus` | job /verify (202 + poll) |
| `POST /api/designer/workflow` | `designer::*` | `web` `designer.ts::saveWorkflow` | valida `squad.workflow.v1` + ledger |
| `GET /api/btv/templates` | `btv::list_templates` | `templates.ts::fetchTemplates` | os 12 templates embutidos |

## 14.5 Consoles pequenos

| Método + rota | Módulo Rust | Consumidor TS | Descrição |
|---|---|---|---|
| `GET /api/doctor` | `btv-server/doctor_console.rs` | `onboarding.ts::fetchDoctor` | 5 checks (providers/uv/docker/git/vocab) |
| `GET /api/lsp` | `btv-server/lsp_console.rs` | `lsp.ts::fetchLspServers` | language servers declarados (zero probe) |
| `GET /api/sandbox` | `btv-server/sandbox_console.rs` | `sandbox.ts::fetchSandbox` | perfil sandbox + ping |
| `GET /api/mcp` | `btv-cli/mcp_console.rs` | `mcp.ts::fetchMcpServers` | servidores MCP + preview de política |
| `GET /api/memory`, `POST /api/memory/recall` | `btv-cli/memory_console.rs` | `memory.ts::fetchMemoryMap`/`recallMemory` | mapa de memória + recall TF-IDF |
| `GET /api/prompt/generators`, `POST /api/prompt/render` | `btv-cli/prompt_render.rs` | `prompts.ts::listGenerators`/`renderPrompt` | PromptForge (sidecar) |

## 14.6 Load-test (`btv-server/src/bin/loadgen.rs`)

`GET /health`, `POST /generate` em `127.0.0.1:7900` — embrulha `ScriptedGenerator` (sem
key), alvo do k6 que valida o P95 do gateway. Não é produto.

---

## Notas

- **SSE** é o único streaming: sessão de código (só `web`) e eventos de squad (ambas SPAs).
  O stream de squad é **finito** (o servidor fecha em `SquadHub::finish_task`).
- O SPA fallback devolve **404 JSON honesto** para `/api/*` inexistente, e serve
  `index.html` (200) para rotas de navegação do cliente.
- A ativação de produto (`/api/btv/squads`) e o `/api/squad/run` compartilham o **mesmo
  motor** (`start_squad_task`).
