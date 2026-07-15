# 10 — Referência: os 14 crates Rust

Inventário por crate: propósito, dependências, tipos-chave (com caminho de arquivo),
implementações de trait/relações e padrões de concorrência.

Grafo de dependência: ver [diagrama de pacotes](../diagramas/03-pacotes.md#31-crates-rust-grafo-de-dependência-do-workspace).

---

## btv-domain — núcleo sem infraestrutura

`crates/btv-domain/src/`. Depende só de `serde`, `thiserror`, `uuid`. **Proibido** (por
`arch-lint`) depender de rusqlite/axum/tonic/reqwest.

- **`ports.rs`** — as traits que o runtime consome:
  - `LlmPort` (`generate(GenerateRequest, on_delta) -> Future<AssistantTurn, LlmError>`),
    re-exportada por btv-llm como `Generator`. `LlmError`: `NoProvider|AllFailed|RateLimited`.
  - `ToolsPort` (`specs()`, `get(name)`), supertrait `Send + Sync`.
  - `RunRepository`, `PersonaRepository`, `TemplatePublicationRepository`, `UserRepository`,
    `LedgerRepository` (associated `type Entry`), `EventStorePort` — **todo método recebe
    `&TenantContext`**; erros são `RepositoryError` (`NotFound`/`ConcurrencyConflict`/`Storage`),
    sem tipo de driver.
  - `RunStatus` (`Ativa|Concluida|Encerrada|Erro`, máquina `can_transition_to`), `DomainEvent`
    + `DomainEventKind` (8 variantes → kinds `btv.*` via `wire_kind()`), `RunError`.
- **`run.rs`** — agregado `Run` (11 campos de wire; `activate`/`approve_gate`/`transition_to`/
  `activation_event` são a **única porta de mutação**), `Deliverable`, `TaskId` (`sq{hex}`,
  newtype), `BriefingResposta`.
- **`tenant.rs`** — `TenantId` (newtype opaco sobre UUID, `LOCAL` = `...0001`, sem `Default`),
  `ActorId` (não-vazio por construção), `TenantContext` (`{tenant, actor}`).
- **`chat.rs`** — `Role`, `ContentBlock` (`Text|ToolUse|ToolResult`), `ChatMessage`,
  `ToolSpec`, `StopReason`, `Usage`, `AssistantTurn`, `GenerateRequest`, `ModelTier`
  (`Small|Medium|Large`, `compaction_threshold`).
- **`tool.rs`** — trait `Tool` (`name/description/input_schema/scope/run`), `ToolOutput`,
  `ToolError`, `DiffLine`.
- **`event.rs`**, **`user.rs`** (`User`, `PinCheck`), **`ledger_kind.rs`** (`LedgerKind`, `parse`),
  **`persona.rs`** (`CustomPersona`, `PersonaOverride`).

**Notas.** O agregado `Run` retorna `DomainEvent`; as traits não têm setters. Tenant
fail-closed é erro de compilação. Chat/Tool types nasceram em btv-llm/btv-tools e migraram
para o domínio no "D1t" (violação 4 do levantamento) para que btv-core dependa só das ports.

---

## btv-core — runtime de sessão (agent loop + permissões)

`crates/btv-core/src/`. Depende só de `btv-domain` (+ serde/thiserror). **Nenhum concreto
de btv-llm/btv-tools/btv-store entra aqui.**

- **`agent_loop.rs`** — `AgentLoop<'a, G: LlmPort>` (campos `generator`, `tools: &dyn
  ToolsPort`, `permissions`, `model/system`, `max_steps/max_tokens`). `continue_run` itera
  gerar→tool_use→permissão→executar→tool_result até `EndTurn`/`max_steps`. `LoopEvent`
  (`TextDelta|TurnCompleted|ToolStarted|ToolFinished|ToolDenied`), `PermissionResolver`
  (trait), `DenyAll`, `LoopError` (`Gateway|MaxSteps`), `LoopOutcome`, `TurnSummary`.
- **`permission.rs`** — `PermissionEngine` (`rules: Vec<Rule>`; `evaluate(tool, scope)`
  primeira regra compatível vence, senão `Ask`; `overlay(overrides)`; `read_only()`),
  `Rule`, `Decision` (`Allow|Ask|Deny`).
- **`agent.rs`** — `AgentProfile` (`permissions: fn() -> PermissionEngine`), consts `BUILD`
  (edit/bash `Ask`), `PLAN`/`GENERAL` (read-only).
- **`compaction.rs`** — `CompactionPolicy` (`for_tier`, `needs_compaction`, `is_safe_boundary`,
  `summarize`), `estimate_tokens` (chars/4). Fronteira segura = último turno do assistente
  sem tool_use pendente.
- **`session.rs`** — `DurableSession` (persistência do histórico), `SessionError`.

**Concorrência.** O loop é `async`; a permissão `Ask` chama o `PermissionResolver` (no CLI:
stdin; na web: `WebPermissionResolver` sobre um `mpsc`). Teste de DoD: fluxo completo com
mocks puros em <100ms.

---

## btv-llm — gateway LLM (adapter de LlmPort)

`crates/btv-llm/src/`. Deps notáveis: `reqwest`, `futures-util`, `serde_json`.

- **`gateway.rs`** — `Gateway` (`client: reqwest::Client`, `providers: Vec<ProviderConfig>`;
  `from_env()` detecta providers em ordem de fallback; `available()`). Implementa `LlmPort`
  (re-exporta `LlmPort as Generator`, `LlmError as GatewayError`). `call_provider` despacha
  por `ProviderId`.
- **`provider.rs`** — `ProviderId` (`Anthropic|Openai|Deepseek`), `LlmRequest` (`cache_key()`
  via `btv_schemas::request_hash`).
- **`anthropic.rs`/`openai.rs`** — transportes + `TurnAggregator` (Messages API vs Chat
  Completions; openai cobre OpenAI **e** DeepSeek). **Não são `Generator`s** — só um
  `Gateway` implementa a trait.
- **`rate_limit.rs`** — `RateLimiter` (sliding window; `for_tier`: Small=60/Medium=30/
  Large=15 por 600s; `acquire()` espera até `max_wait`), `RateLimitError`.
- **`model_tier.rs`** — `tier_from_id(model_id)` (regex sets em `OnceLock`, Large checado
  primeiro). `scripted.rs` — `ScriptedGenerator` (keyless, determinístico). `pricing.rs` —
  `estimate_cost_usd`/`price_for` (`None` quando não tabelado). `sse.rs` — `SseParser`.

**Decorators** (moram em `btv-cli`, mas fecham o stack): `CachedGenerator<G>` (`cache.rs`) e
`RateLimitedGenerator<G>` (`rate_limit_gen.rs`), compostos como
`CachedGenerator<RateLimitedGenerator<Gateway>>`. **Cache é o mais externo** — hit nunca
consome vaga nem token. Composição por valor genérico (sem `Box<dyn>`).

---

## btv-tools — ferramentas deterministas + contenção

`crates/btv-tools/src/`. Deps: `bollard` (Docker), `rmcp` (MCP), `ignore`/`grep` (ripgrep),
`libc`.

- **`registry.rs`** — `ToolRegistry` (`tools: Vec<Box<dyn Tool>>`; `default_set(root)` boxa
  os 4 built-ins; `register`; implementa `ToolsPort`).
- Ferramentas `dyn Tool`: `ReadTool` (`read.rs`), `GrepTool` (`grep.rs`), `EditTool`
  (`edit.rs`, `line_diff`), `BashTool` (`bash.rs`, drena stdout/stderr em threads +
  `try_wait`), `SkillTool` (`skill.rs`, `sandboxed: bool`), `McpTool` (`mcp.rs`), `LspTool`
  (`lsp.rs`, `LspQuery` = definition/references/diagnostics/symbol).
- **`sandbox.rs`** — `Sandbox` (bollard; rootfs read-only, `cap_drop ALL`,
  `no-new-privileges`, rede off, mem/cpu limitados; `ping()` fail-closed → `DaemonUnavailable`).
  **Não é `Tool`** — alcançado só por `SkillTool` quando `sandboxed`.
- **`mcp.rs`** — cliente rmcp (transporte stdio child-process). `McpSession` = thread
  dedicada com runtime `current_thread` + `mpsc`. `register_mcp_server` lê `.btv/mcp.toml`.
- **`lsp.rs`** — cliente LSP hand-rolled (zero-dep, JSON-RPC com `Content-Length`).
  `LspSession` = `std::thread` reader + `Condvar`. `register_lsp_server` lê `.btv/lsp.toml`.
- **`diff.rs`** — `line_diff`/`format_diff` (`DiffLine` re-exportado do domínio).

**Ponte async→sync.** `Tool::run` é síncrono; três estratégias: thread+runtime (Sandbox),
thread de sessão+`mpsc` (MCP), `std::thread`+`Condvar` (LSP). Bins de fixture:
`btv_lsp_fixture.rs`, `btv_mcp_fixture.rs`.

---

## btv-store — storage (SQLite/PG, ledger, telemetria)

`crates/btv-store/src/`. Deps: `rusqlite` (bundled), `sqlx` (feature `pg`).

- **`ledger.rs`** — `LedgerStore` (`conn: Connection`; hash-chain **por tenant**; `append`
  usa `TransactionBehavior::Immediate`; `verify_chain` detecta `BrokenChain`/`ForeignEntry`).
  Implementa `LedgerRepository`. `LedgerError`.
- **`btv.rs`** — `BtvStore` (product store `.btv/btv.db`: runs/deliverables/personas/
  template_pub/users/custom_personas). Implementa `RunRepository`, `PersonaRepository`,
  `TemplatePublicationRepository`, `UserRepository`. `pin_hash` (sha256, documentado como
  não-KDF). `VocabViolation` (doctor).
- **`events.rs`** — `EventStore` (concorrência otimista, `Conflict{expected,found}`).
  Implementa `EventStorePort` (LOCAL-only, fail-closed em tenant não-LOCAL).
- **`pg.rs`** (feature `pg`) — `PgStore` (`rt: Runtime` + `pool: PgPool`; async sob
  `rt.block_on` por operação; RLS via `set_config('app.tenant_id')` + `WHERE tenant_id`;
  ledger com retry otimista sobre `UNIQUE(tenant_id,seq)`; sessões SaaS `issue/resolve/
  revoke_session`). Reusa as funções compartilhadas do ledger → paridade de hash.
- **`telemetry.rs`** — `TelemetryStore` + `Telemetry` (handle `Arc<Mutex<...>>`, offline-first,
  erros nunca derrubam o caminho principal). `prompt_cache.rs`, `prompt_library.rs`,
  `rule_store.rs` (`RuleStore`, `RuleDecision`).

**Concorrência.** Cada store tem `Connection` bare (`&mut self` onde há transação); só
`Telemetry` é `Arc<Mutex<>>`. WAL + `busy_timeout` para concorrência entre conexões.

---

## btv-verify — pipeline determinístico + vetter

`crates/btv-verify/src/`. Deps: `btv-schemas`, `toml`, `libc`.

- **`lib.rs`** — `run_pipeline`/`run_pipeline_with_progress` (roda `StepSpec`s em ordem,
  callback de progresso), `StepSpec`, `Parser` (`CargoTest|ClippyJson|RuffJson`), `git_sha()`.
  `TIMEOUT_EXIT_CODE = 124`.
- **`config.rs`** — `VerifyConfig`/`StepConfig` (de `btv.toml`), `default_steps()`
  (test/lint/fmt).
- **`parsers.rs`** — `parse_cargo_test`/`parse_clippy_json`/`parse_ruff_json` → `Vec<Finding>`.
- **`exec.rs`** — `run_with_timeout` (subprocesso em `process_group(0)`; `kill_process_group`
  via `libc::kill(-pid, SIGKILL)` — mata netos também).
- **`vetter.rs`** — `SkillManifest`, `Decision` (`Vet|Block`), `vet_skill` (fail-closed,
  scan de padrões perigosos + mismatch de permissão), `list_skill_statuses`.
- **`prompt_integrity.rs`** — `validate_contract` (campos obrigatórios, ética, floor de
  qualidade, padrões perigosos; `PromptMode` Vitrine/Enterprise).

Os tipos de evidência (`VerificationEvidence`/`VerificationStep`/`Finding`/`Verdict`) moram
em `btv-schemas::verification`.

---

## btv-schemas — DTOs serializáveis + hash canônico

`crates/btv-schemas/src/`. Deps: `schemars`, `sha2`, `hex`, `btv-domain` (só por `TenantId`).

- **`canonical.rs`** — `canonical_json`, `sha256_hex`, `request_hash`, `validate_cache_key`,
  `CacheKeyError::NumeroProibido` (ADR 0032: rejeita floats `1.0` e não-finitos). **Twin
  Python:** `btv_promptforge/hashing.py`.
- **`ledger.rs`** — `LedgerEntry` (`chain_hash`/`hash_body`; `tenant: Option<TenantId>` entra
  no corpo hasheado), `OverrideMark`.
- **`verification.rs`** — `VerificationEvidence`/`VerificationStep`/`Finding`/`Verdict`
  (`derive_verdict`). Consumido por btv-verify **e** por `review.rs`.
- Demais DTOs (todos `schemars::JsonSchema`): `experiment.rs` (`ExperimentReport`, z-test
  hand-rolled + Bonferroni), `handoff.rs`, `persona.rs`, `plan.rs`, `review.rs`
  (`ValueReview::from_evidence`), `squad_template.rs` (`builtin_templates()` — 12 via
  `include_str!`), `telemetry.rs`, `workflow.rs` (`SquadWorkflow::validate_edges`).

---

## btv-proto — bindings tonic

`crates/btv-proto/`. `build.rs` compila `schemas/proto/*.proto` via tonic-build com protoc
vendorizado (`protoc-bin-vendored`). `lib.rs` re-exporta os módulos gerados (`core`, `llm`,
`squad`, `memory`, `promptforge`).

---

## btv-sidecar — ponte gRPC bidirecional

`crates/btv-sidecar/src/`. Deps: `btv-proto`, `tonic`, `hyper-util`, `tower`, `tokio`.

- **Clientes (Rust→Python):** `SidecarClient` (`client.rs`, PromptForge), `SquadClient`
  (`squad_client.rs`, `execute_task` → `Streaming<SquadEvent>`; `drain_stream` → `SquadRun::
  {Completed|Failed}`), `MemoryClient` (`memory_client.rs`).
- **Servidor (Python→Rust):** `CoreServer<B: CoreBackend>` (`core_server.rs`) serve
  `CoreService` — `generate` (stream via `mpsc`+`ReceiverStream`), `request_permission`,
  `run_tool` (os antigos `append_ledger`/`recall`/`remember` `Status::unimplemented` foram
  REMOVIDOS — ADR 0034). `serve_core` liga um `UnixListener`.
- **Supervisores:** `SidecarSupervisor`/`SquadSupervisor`/`MemorySupervisor` (`spawn` de
  `uv run -m ...`; `wait_ready`; **kill de grupo de processos** no `Drop`).
- **Serviço de longa duração (ADR 0019):** `SidecarService`/`MemoryService` (singletons,
  restart-on-crash), `SquadPool` (`slot_states`, `semaphore`, `SquadLease`).

**Transporte UDS:** `tower::service_fn` dial `UnixStream` + `hyper_util::rt::TokioIo`,
conexão lazy. `grpc.default_authority` ajustado no lado Python (achado ADR 0005).

---

## btv-server — borda axum (dashboard)

`crates/btv-server/src/`. Deps: `axum`, `tower-http`. **Não** depende de btv-cli/btv-sidecar.

- **`lib.rs`** — `router(...)` monta as rotas admin/telemetria/prompts/ledger/providers/
  verify/designer + `GET /api/btv/templates`; `AppState` (`Telemetry`, `Arc<Mutex<PromptLibrary>>`,
  `Arc<Mutex<LedgerStore>>`, `root`, `verify_job`); SPA fallback (`ServeDir` + `ServeFile`);
  layer `require_local_origin`. Serve `btv-web` na raiz e `web` em `/dev`.
- **`guard.rs`** — `require_local_origin` (403 em `Origin` não-local para não-GET),
  `origin_allowed`, `trusted_origin_hosts` (`BTV_TRUSTED_ORIGINS`).
- **`handlers/*`** — `telemetria`, `prompts`, `ledger`, `admin` (skills/experiment/ratelimit),
  `providers`, `verify` (job em background com `spawn_blocking` + `catch_unwind`), `designer`
  (valida `SquadWorkflow` + ledger).
- **Consoles** (routers additivos): `doctor_console.rs` (`GET /api/doctor` agrega 5 checks),
  `lsp_console.rs`, `sandbox_console.rs`. **`bin/loadgen.rs`** — alvo k6 (ScriptedGenerator,
  sem key).

---

## btv-cli — o binário `btv` (composition root)

`crates/btv-cli/src/`. Depende de ~12 crates. É onde tudo se amarra.

- **`main.rs`** — `Cli`/`Commands` (`Run`/`Chat`/`Tui`/`Verify`/`Squad`/`Dashboard`/
  `Experiment`/`Session`). `prepare()` constrói o generator (`Gateway` → `RateLimitedGenerator`
  → `CachedGenerator`, alias `CliGenerator`). `build_loop()` injeta `ToolRegistry` +
  `PermissionEngine` no `AgentLoop`. `CliResolver` implementa `PermissionResolver` (stdin).
  `run_dashboard` é o composition root HTTP (abre stores, mescla routers).
- **`session.rs`** — `Session` (grava `LoopEvent`s no ledger). **`sidecar.rs`** — `try_start`
  (degrada para `None` sem quebrar). **`squad.rs`** — `run_squad` (3 níveis: `try_squad` →
  `run_once` → `safe_mode`; `GatewayCoreBackend` implementa `CoreBackend`).
- **`web_agent.rs`** — sessão SSE de código (ADR 0016): `SessionEvent` (mirror de `LoopEvent`),
  `SessionHub`, `WebPermissionResolver`, permissão fail-closed (ADR 0017), rule CRUD auditado.
- **`squad_agent.rs`** — squad ao vivo pela web: `SquadHub`, `inject_cockpit_context`,
  `WebSquadCoreBackend`/`ScriptedSquadCoreBackend`, `start_squad_task` (motor compartilhado).
- **`btv_agent.rs`** — ativação da galeria/wizard: `ativar_squad_handler` (monta task +
  `PersonaSpec` roster + `PromptHash`), `aprovar_gate_handler`/`pedir_ajuste_handler`, 22
  rotas de produto.
- Demais: `cache.rs`/`rate_limit_gen.rs` (decorators), `tui_app.rs` (event loop crossterm),
  `mcp_console.rs`/`memory_console.rs`/`prompt_render.rs` (consoles), `convert.rs` (export
  OOXML puro), `tenant_extractor.rs` (borda de tenant, ADR 0029), `skills.rs`
  (`build_registry_with_vetting` — mounting point de skills/MCP/LSP).

---

## btv-tui — view ratatui pura

`crates/btv-tui/src/lib.rs`. Backend-agnóstico (event loop vive em `btv-cli::tui_app`).
`DiffKind`, `Item` (`User|Assistant|Tool|Diff|Notice`), `PermissionPrompt`, `TuiState`
(`items/streaming/input/status/permission/busy`; `finish_turn`), `render(frame, state)`.
Testado com `ratatui::backend::TestBackend`.

---

## btv-golden — harness de goldens HTTP (dev-dep)

`crates/btv-golden/src/lib.rs`. Compartilhado por `btv-server` e `btv-cli`. `Kind`
(`Str|Num|StrOuVazia`), `Volatile`/`vstr`/`vnum` (mascaramento de campos voláteis),
`GoldenRequest`/`GoldenResponse`/`GoldenStep`, `step(...)`, `check(flow, steps)` (compara com
`schemas/fixtures/http/*.golden.json`; reescreve só com `BTV_UPDATE_GOLDEN=1` e nunca sob `CI`).

---

## btv-contract — suíte dual-adapter (dev-dep)

`crates/btv-contract/src/lib.rs`. Genérica sobre uma factory de adapter. Suítes:
`suite_run_repository`, `suite_persona_repository`, `suite_template_publication_repository`,
`suite_user_repository`, `suite_ledger_repository`, e **`suite_ledger_determinismo_cross_adapter`**
(prova `(seq, prev_hash, entry_hash)` idênticos entre SQLite e Postgres). Roda idêntica nos
dois adapters — garante paridade local↔SaaS. Conhece só `btv_domain` (não `btv-schemas`).
