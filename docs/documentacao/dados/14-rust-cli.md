# Dicionário de Dados — `crates/btv-cli`

Mapa exaustivo de fluxo de dados do crate `btv-cli` (o composition root do
BuildToValue): CLI/TUI, dashboard web (axum), agente web SSE, squad ao vivo,
produto BuildToValue, borda de tenant, decorators do gateway e conversores de
export.

**Taxonomia de Direção:** `entrada` (param/arg CLI/corpo HTTP/leitura) ·
`saída` (retorno/resposta HTTP/SSE/escrita) · `intermediário` (local/buffer,
mesmo descartado) · `estado` (campo de struct/hub) · `config` (const/env/flag
CLI) · `wire` (proto/JSON HTTP/SSE/DB).

Módulos de teste puro (mencionados em 1 linha, não tabelados):
- `src/test_support.rs` — `CWD_GUARD` (`tokio::sync::Mutex<()>` global) + `lock_cwd()`: serializa testes que mutam `current_dir` entre `web_agent`/`squad_agent`.
- `src/tenant_border_sweep.rs` — varredura adversarial (E1s.4): prova no router real que o layer universal gateia toda a superfície saas (fallback do SPA + token forjado → 401).
- `src/btv_agent_golden.rs` — golden tests HTTP do produto (`btv_agent::router`) com stores em memória, semeadura pelo caminho de produção, igualdade profunda (`BTV_UPDATE_GOLDEN=1`).

---

## src/main.rs

Papel: entrada do binário `btv` — parsing clap, `prepare()` (env→Gateway→decorators), `build_loop()`, subcomandos (Run/Chat/Tui/Verify/Squad/Dashboard/Experiment/Session), REPL de chat e `/prompt`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `Cli.command` | `Commands` | entrada | argv → `main()` match | raiz do parser clap |
| `RunOpts.model` | `String` (`--model`, default `claude-sonnet-5`) | config | CLI → `prepare`/`build_loop` | `tier_from_id` deriva `ModelTier` |
| `RunOpts.agent` | `String` (`--agent`, default `build`) | config | CLI → `build_loop` | `build`→`&BUILD`, `plan`→`&PLAN`, senão `bail!` |
| `RunOpts.yes` | `bool` (`--yes`) | config | CLI → `CliResolver.auto_yes` | auto-aprova permissões |
| `RunOpts.no_cache` | `bool` (`--no-cache`) | config | CLI → `prepare` | escolhe `PromptCache::open_in_memory` vs disco |
| `RunOpts.session` | `Option<String>` (`--session`) | config | CLI → `open_durable` | retoma/nomeia sessão; None gera `s{nanos:x}` |
| `RunOpts.context_window` | `usize` (`--context-window`, default 200_000) | config | CLI → `maybe_compact` (`CompactionPolicy`) | janela de tokens p/ compaction |
| `Commands::Run.task` | `String` (posicional) | entrada | CLI → `run_once` | descrição da tarefa única |
| `Commands::Verify.config` | `Option<PathBuf>` (`--config`) | config | CLI → `run_verify_pipeline` | default `root/btv.toml` |
| `Commands::Verify.out` | `Option<PathBuf>` (`--out`) | config | CLI → `run_verify` | default `.btv/evidence/<run_id>.json` |
| `Commands::Verify.format` | `VerifyFormat` (`--format`, default human) | config | CLI → impressão | Human vs Json |
| `Commands::Squad.task` | `String` | entrada | CLI → `squad::run_squad` | |
| `Commands::Dashboard.port` | `u16` (`--port`, default 7878) | config | CLI → `SocketAddr` | |
| `Commands::Dashboard.host` | `IpAddr` (`--host`, default `127.0.0.1`) | config | CLI → `SocketAddr` | bind loopback local-first |
| `Commands::Dashboard.no_web_agent` | `bool` (`--no-web-agent`, default false) | config | CLI → `run_dashboard(web_agent=!no_web_agent)` | agente web ligado por padrão |
| `Commands::Experiment.experiment` | `String` | entrada | CLI → `Telemetry::experiment_variants` | `props.experiment` |
| `Commands::Experiment.db` | `Option<PathBuf>` (`--db`) | config | CLI → `Telemetry::open` | default `.btv/telemetry.db` |
| `SessionCmd::Issue.{tenant,user,db_url}` | `String`/`Option<String>` (feature `pg`) | entrada/config | CLI → `PgStore::issue_session` | emite sessão; imprime token 1×; env `BTV_PG_URL` fallback |
| `emitido.token` | token opaco | saída | `PgStore` → stdout | banco guarda só o hash; aviso no stderr |
| `ANTHROPIC_API_KEY`/`DEEPSEEK_API_KEY`/`OPENAI_API_KEY` | env | config/entrada | ambiente → `Gateway::from_env` | keys só no processo Rust (ADR 0001) |
| `BTV_PG_URL` | env | config | ambiente → `run_session` | URL Postgres |
| `gateway` | `Gateway` | intermediário | `prepare` → decorators | `available()` valida providers, senão `bail!` |
| `cache` | `PromptCache` | intermediário/estado | `prepare` → `CachedGenerator` | disco `.btv/cache.db` ou in-memory |
| `telemetry` | `Option<Telemetry>` | estado | `prepare` → decorators | `.btv/telemetry.db`; `.ok()` (nunca derruba) |
| `limited` | `RateLimitedGenerator<Gateway>` | intermediário | `prepare` → `CachedGenerator` | `RateLimiter::for_tier(tier)` |
| `CliGenerator` | `CachedGenerator<RateLimitedGenerator<Gateway>>` | saída | `prepare` → subcomandos | pilha de decorators completa |
| `AgentLoop{generator,tools,permissions,model,system,max_steps:20,max_tokens:4096}` | struct | intermediário | `build_loop` → `continue_run`/`run` | `permissions=(profile.permissions)()`; `system=system_prompt(tier)` |
| `system_prompt` | `String` | config/wire | `tier` → AgentLoop.system | tier Small ganha "UMA ação por vez" |
| `DurableSession<EventStore>` | struct | estado | `open_durable` → `run_once`/`chat` | `.btv/sessions.db`; `TenantContext::local(actor "cli:run")` |
| `session_id` | `String` | estado | `open_durable` | `--session` ou `s{nanos & 0xffff_ffff_ffff:x}` |
| `Session` (ledger) | struct | estado | `Session::open` → `record`/`note`/`finish` | `.btv/btv.db` |
| `skill_vetting` | `Vec<SkillStatus>` | intermediário | `build_registry_with_vetting` → `record_skill_vetting` | vira ledger `skill.vetting` |
| `summary` (compaction) | `String` | intermediário/wire | `policy.summarize` → `durable.compact` | ledger `compaction.applied` {epoch, summary_chars} |
| `LoopEvent` | enum (emprestado) | intermediário | `continue_run` on_event → `print_event`+`session.record` | TextDelta/TurnCompleted/ToolStarted/ToolFinished/ToolDenied |
| `persisted` | `usize` | saída | `durable.persist_new()` → stderr | nº mensagens persistidas |
| `session.verify()` | `u64` | saída | ledger → stderr "ledger íntegro: N" | contagem da cadeia de hash |
| chat input `line` | `String` (stdin) | entrada | `read_line` → durable/session | vazio/`sair`/`exit`/`quit`/EOF encerra |
| `/compact` | comando | entrada | stdin → `maybe_compact(force=true)` | força nova época |
| `/prompt ...` | comando | entrada | stdin → `handle_prompt_command` | list/save/library/use/fav/rm/render |
| `library` | `PromptLibrary` | estado | `.btv/prompt_library.db` | biblioteca de prompts do chat |
| `/prompt save` campos | `HashMap<String,String>` + `tags` + `generator_name` | entrada→wire | tokens `chave=valor`, `tags=a,b` → `client.render`→`library.save` | `fields_json` gravado; exige sidecar |
| `ExperimentReport` | struct | saída/wire | `from_variants` → stdout | veredito Significant/Inconclusive/InsufficientData; `ALPHA`/`MIN_SAMPLES` |
| `VerificationEvidence` | struct | saída/wire | `run_verify_pipeline` → `.btv/evidence/<run_id>.json` | exit 1 se veredito `Fail` |
| `run_id` (verify) | `String` | intermediário | `run-{nanos & 0xffff_ffff_ffff:x}` | |
| `git_sha` | `String` | wire | `btv_verify::git_sha()` → evidência | `"unknown"` se ausente |
| `CliResolver.auto_yes` | `bool` | estado | resolve permissão terminal `[s/N]` | aceita s/sim/y/yes |
| exit code (verify Fail) | `1` | saída | `std::process::exit(1)` | gate de self-hosting |
| `default_hub`/`default_squad_pool`/routers merge | vários | intermediário | `run_dashboard` → `serve_with_agent` | compõe squad/btv/prompt/mcp/memory/sandbox/lsp/doctor routers |
| `tenant_resolver` | `TenantResolucao` | estado/config | `from_env(None)` → `BtvAgentState` + layer | resolvido 1× no arranque |
| `squad_hub.seed_task_seq` | `u64` | estado | `store.max_run_task_seq()` → hub | evita colisão `task_id` pós-restart |
| `reconcile_stale_runs` | `usize` | saída | store → stderr | runs `ativa` órfãs → `encerrada` no arranque |

Fluxo: argv→`Cli::parse`→match subcomando; para run/chat/tui/squad `prepare()` monta `Gateway→RateLimited→Cached` + valida providers e telemetria, `build_loop()` injeta perfil/permissões/system no `AgentLoop`; `continue_run` emite `LoopEvent`s que viram stdout + ledger `.btv/btv.db`; dashboard compõe todos os routers axum sob a guarda de Origin/tenant.

---

## src/cache.rs

Papel: decorator `CachedGenerator` — replay de turno por hash (`prompt-cache-key.v1`) sem tocar a rede, com telemetria `cache.hit`/`cache.miss`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `CachedGenerator.inner` | `G: Generator` | estado | wrap do `RateLimitedGenerator` | |
| `CachedGenerator.cache` | `Mutex<PromptCache>` | estado | `.btv/cache.db` | |
| `CachedGenerator.telemetry` | `Option<Telemetry>` | estado | | eventos de cache |
| `req` | `GenerateRequest` | entrada | `generate(req,...)` | model/system/tools/messages/temperature |
| `envelope` | `serde_json::Value` | intermediário/wire | `{model,system,tools,chat}` → `request_hash` | envelope canônico completo |
| `temperature` | `Value` | intermediário | `req.temperature` ou `Null` → hash | |
| `key` (cache_key) | `String` | intermediário/wire | `btv_schemas::request_hash` | `CacheKeyError` (ex.: temp 1.0) → segue sem cache (ADR 0032) |
| `hit`/`stored` | `Option<String>` | intermediário | `cache.get(&key)` → deserialize `AssistantTurn` | replay do turno |
| `turn.text()` replay | `String` | saída | cache → `on_delta` | hit também emite o texto |
| `turn.provider` | `String` | saída/wire | `"{provider}+cache"` | marca origem do hit |
| `cache.hit` | evento telemetria | saída/wire | `{model}` → `Telemetry::record` | |
| `cache.miss` | evento telemetria | saída/wire | `{model}` → `Telemetry::record` | antes de chamar inner |
| `serialized` | `String` | wire | `to_string(&turn)` → `cache.put(key,serialized,ts)` | grava novo turno |

Fluxo: `generate`→calcula `key` do envelope canônico; hit ⇒ replay + `cache.hit`; miss ⇒ `cache.miss`, chama `inner`, serializa e grava.

---

## src/rate_limit_gen.rs

Papel: decorator `RateLimitedGenerator` — adquire vaga do `RateLimiter` (tier-gated) antes de gerar; registra `llm.call` com tokens reais. Fica sob o `CachedGenerator` (hit nunca consome vaga).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `inner` | `G: Generator` | estado | wrap do `Gateway` | |
| `limiter` | `RateLimiter` | estado/config | `RateLimiter::for_tier(tier)` | sliding window |
| `telemetry` | `Option<Telemetry>` | estado | | `llm.call` |
| `limiter.acquire()` | `Result` | intermediário | erro → `GatewayError::RateLimited` | |
| `model` | `String` | intermediário | `req.model.clone()` (antes do move) | usado no evento pós-resposta |
| `turn.usage.{input,output}_tokens` | `u64` | wire | resposta → `llm.call` payload | estima custo por modelo |
| `llm.call` | evento telemetria | saída/wire | `{model,input_tokens,output_tokens}` | só chamadas OK contam |

Fluxo: `acquire`→ (falha vira `RateLimited`) → `inner.generate` → em sucesso registra `llm.call` com tokens.

---

## src/sidecar.rs

Papel: sobe o sidecar Python PromptForge com degradação graciosa (`try_start`→`None`); formata aviso de lint.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `START_TIMEOUT` | `Duration` (8s) | config | const → `wait_ready` | |
| `BTV_PYTHON_DIR` | env | config | `python_workspace_dir` | senão `CARGO_MANIFEST_DIR/../../python` |
| `socket` | `PathBuf` | intermediário/wire | `temp_dir/btv-sidecar-{pid}.sock` | UDS |
| `(SidecarSupervisor, SidecarClient)` | tupla | saída | `try_start` | `None` se sem `pyproject.toml`/spawn/health falhar |
| `LintReport{score,grade,issues}` | proto | entrada/wire | sidecar → `advisory` | |
| aviso de lint | `Option<String>` | saída | `advisory` → stderr | `None` se `score >= 0.9` |

Fluxo: acha workspace Python→spawn supervisor→`wait_ready`; `advisory` filtra reports bons.

---

## src/session.rs

Papel: `Session` grava eventos do loop no ledger append-only hash-chain (`.btv/btv.db`); helpers `append_override_entry`/`append_entry` para mutações fora do ciclo de tarefa.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `Session.store` | `LedgerStore` | estado | `.btv/btv.db` | |
| `Session.id` | `String` | estado | `s{unix_nanos & 0xffff_ffff_ffff:x}` | actor `btv-cli:{id}` |
| `now_rfc3339()` | `String` | intermediário/wire | `OffsetDateTime::now_utc` → `ts` | fallback epoch |
| `session.start` | LedgerEntry | saída/wire | `{task,model}` | no `open` |
| `LedgerEntry{seq:0,prev_hash:"",entry_hash:"",kind,actor,payload,override,fake_marker:None,ts,tenant:None}` | struct | wire | `append` → store | seq/hashes preenchidos pelo store; tenant LOCAL implícito (None) |
| `LoopEvent::TextDelta` | — | intermediário | `record` | ignorado (granularidade de turno) |
| `llm.turn` | LedgerEntry | saída/wire | TurnCompleted → `{provider,input_tokens,output_tokens}` | |
| `tool.run` | LedgerEntry | saída/wire | ToolStarted → `{tool,scope}` | |
| `tool.result` | LedgerEntry | saída/wire | ToolFinished → `{tool,ok,summary}` | |
| `tool.denied` | LedgerEntry | saída/wire | ToolDenied → `{tool,scope}` | |
| `user.turn` (via `note`) | LedgerEntry | saída/wire | chat/tui → `{chars}` | evento avulso |
| `session.end` | LedgerEntry | saída/wire | `finish` → `{success,steps}` | |
| `verify()` | `u64` | saída | `store.verify_chain()` | total de entradas |
| `append_override_entry` | fn → LedgerEntry `OverrideMark{marked:true}` | saída/wire | permissão web → ledger | mutação sensível marcada |
| `append_entry` | fn → LedgerEntry `override:None` | saída/wire | `RunTool` do squad → `squad.tool_run` | |

Fluxo: `open`(session.start)→`record`(mapeia LoopEvent→kind)→`finish`(session.end); falhas de ledger só vão ao stderr.

---

## src/web_agent.rs

Papel: fundação do agente web (Fase 7) — `SessionEvent` (DTO/SSE), `SessionHub` (estado de sessões, permissão via mpsc + timeout, teto), rotas de mensagem/permissão/matriz/regras, guarda de Origin, `merged_router`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `SessionEvent` | enum `#[serde(tag="type", snake_case)]` | saída/wire (SSE) | hub → cliente | contrato ADR 0016 |
| `SessionEvent::TextDelta{text}` | variante | wire | LoopEvent → SSE | |
| `SessionEvent::TurnCompleted{provider,input_tokens,output_tokens}` | variante | wire | | |
| `SessionEvent::ToolStarted{name,scope}` | variante | wire | | |
| `SessionEvent::ToolFinished{name,ok,summary,diff:Option<Vec<DiffLine>>}` | variante | wire | | inclui diff de `edit` |
| `SessionEvent::ToolDenied{name,scope}` | variante | wire | | |
| `SessionEvent::PermissionRequested{request_id,tool,scope}` | variante (server-only) | wire | `request_permission` → SSE | front resolve via POST |
| `SessionEvent::Done{ledger_verified:u64}` | variante | wire | `finish_task_ok` | contagem real `Session::verify()` |
| `SessionEvent::Error{message}` | variante | wire | | |
| `ErrorBody{error,code}` | struct | saída/wire | contrato de erro único de toda a fase | |
| `PendingPermission{request_id,responder:mpsc::Sender<bool>}` | struct | estado | | canal de resposta de permissão |
| `SessionState{log:Vec<SessionEvent>,tx:broadcast::Sender<SessionEvent>(256),pending,busy}` | struct | estado | por sessão | `busy`=ator único (409) |
| `SessionHub.sessions` | `Arc<Mutex<HashMap<String,SessionState>>>` | estado | todas as sessões vivas | |
| `SessionHub.max_sessions` | `usize` | config | `BTV_MAX_SESSIONS` (default 8) | teto → 429 |
| `SessionHub.permission_timeout` | `Duration` | config | `BTV_PERMISSION_TIMEOUT_SECS` (default 300) | fail-closed Deny |
| `SessionHub.next_request_id` | `Arc<AtomicU64>` | estado | `perm-{n}` | id de pedido |
| `ensure_session` | `Result<(),HubError>` | intermediário | cria se ausente; teto → `TooManySessions` | |
| `publish` | — | saída | evento → log + broadcast tx | |
| `subscribe` | `(Vec<SessionEvent>, Receiver)` | saída | snapshot + live | reconexão ADR 0016 |
| `request_permission` | `bool` | intermediário | publica PermissionRequested, `rx.recv_timeout` | timeout → false |
| `try_start`/`finish_busy` | `Result<(),()>` | estado | marca/libera `busy` | 409 se ocupado |
| `resolve_permission` | `Result<(),()>` | intermediário | valida request_id, `responder.send(allow)` | corrida timeout×resposta |
| `WebPermissionResolver{hub,session_id}` | struct (impl `PermissionResolver`) | estado | `resolve`→`request_permission` | ponte permissão↔HTTP |
| `SessionTaskSpec{tools,permissions,model,system,task,root}` | struct | entrada | → `spawn_session_task` | |
| `spawn_session_task` | fn (`spawn_blocking`) | — | roda AgentLoop, publica eventos, grava ledger | `max_steps` `BTV_WEB_MAX_STEPS`(30), `max_tokens` `BTV_WEB_MAX_TOKENS`(4096) |
| `spawn_message_task` | fn (`spawn_blocking`) | — | replica `btv run` (ledger + DurableSession) | aplica `overrides` sobre perfil |
| `SendMessageBody{message,model?,agent?}` | struct | entrada/wire | `POST /api/session/:id/message` | model/agent default web |
| `default_web_model()` | `String` | config | `BTV_DEFAULT_MODEL` (default `claude-sonnet-5`) | |
| `BTV_WEB_CONTEXT_WINDOW` | env (default 200_000) | config | `RunOpts.context_window` | |
| `BTV_SCRIPTED` | env | config | modo roteirizado (`ScriptedGenerator`) | e2e sem key; pede `bash` real |
| `scripted_turns_for(message)` | `Vec<AssistantTurn>` | intermediário/wire | tool_use `echo {message:?}` → end_turn | roteiro |
| `overrides` | `Vec<Rule>` | intermediário | `load_rule_overrides(root, agent)` | matriz + "sempre" |
| `rules_db_path`/`open_rule_store` | `.btv/rules.db` (`RuleStore`) | estado/wire | | fail-open se corrompido |
| `rule_record_to_core` | `Rule` | intermediário | RuleRecord → core `Rule` (Allow/Ask/Deny) | |
| `SetRuleBody{profile,tool,scope_prefix?,decision}` | struct | entrada/wire | `POST /api/permissions/rules` | `decision_from_wire` allow/ask/deny |
| `RuleRecord` | struct | saída/wire | `store.set` → JSON + ledger `permission_rule.set` (override) | |
| `list_rules_handler` | `Vec<RuleRecord>` | saída/wire | `GET /api/permissions/rules` | |
| `revoke_rule_handler` | 204/404 | saída | `DELETE /api/permissions/rules/:id` + ledger `permission_rule.revoke` | idempotente |
| `MATRIX_TOOLS` | `[&str;5]` | config | `["read","grep","edit","bash","webfetch"]` | |
| `MatrixRow{tool,build:Decision,plan:Decision}` | struct | saída/wire | `GET /api/permissions/matrix` | efetivo = default perfil + overrides |
| `WebAgentState{hub}` | struct | estado | axum State | |
| `router(hub)` | `Router` | saída | 6 rotas (`/api/session/{id}/events|message|permission`, `/api/permissions/matrix|rules|rules/{id}`) | |
| `merged_router` | `Router` | saída | dashboard.merge(router).merge(extra) + layers | guarda tenant (interna) + Origin (externa) |
| `serve_with_agent` | fn (9 args) | — | monta app + `TcpListener::bind(addr)` → `axum::serve` | |
| `sse_handler` | `Sse<Stream>` | saída/wire | `GET .../events` | snapshot.chain(live) → `to_sse_event` |
| `to_sse_event` | `Event` | wire | `Event::json_data(&e)` | |
| `ResolvePermissionBody{request_id,allow}` | struct | entrada/wire | `POST .../permission` | 200/404 |
| `require_local_origin` | middleware | intermediário | valida `Origin` em método≠GET | `btv_server::origin_allowed`+`trusted_origin_hosts`; `BTV_TRUSTED_ORIGINS`; sem Origin passa |

Fluxo: `POST message`→`ensure_session`(teto)→`try_start`(busy)→carrega overrides→(scripted ou `prepare`)→`spawn_*_task` roda AgentLoop em `spawn_blocking`, publicando `SessionEvent`s no broadcast que o `sse_handler` transmite; permissão faz round-trip HTTP via `WebPermissionResolver`+mpsc; regras persistem em `rules.db` com trilha no ledger.

---

## src/squad_agent.rs

Papel: squad ao vivo pelo navegador — `SquadHub` (estado de tarefas, HITL pending, cockpit inbox, tool_runs, kill-switch), `inject_cockpit_context`, `start_squad_task`, backends `CoreBackend` (real/scripted), rotas SSE + HITL + message + emergency-stop.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `PendingHitl{responder:oneshot::Sender<bool>}` | struct | estado | gate HITL | |
| `SquadTaskState.log` | `Vec<SquadEvent>` | estado | snapshot p/ reconexão | |
| `SquadTaskState.tx` | `Option<broadcast::Sender<SquadEvent>>(256)` | estado | live SSE; `None`=terminada (fecha stream) | |
| `SquadTaskState.pending` | `Option<PendingHitl>` | estado | HITL pendente | |
| `SquadTaskState.inbox` | `VecDeque<String>` | estado | mensagens do cockpit não consumidas | |
| `SquadTaskState.stopped` | `bool` | estado | kill-switch | |
| `SquadTaskState.abort` | `Option<AbortHandle>` | estado | aborta a task do stream | |
| `SquadTaskState.tool_runs` | `Vec<ToolRunNote>` | estado | trilha de ferramentas | matéria-prima das entregas BTV |
| `ToolRunNote{tool,scope,exit_code}` | struct pub(crate) | estado/intermediário | `note_tool_run` → `tool_runs` | `edit` exit 0 = arquivo real |
| `chat_event(task_id,author,author_role,text)` | `SquadEvent` | wire | eco de fala → SSE | tenant LOCAL, actor `web:squad` |
| `SquadHub.tasks` | `Arc<Mutex<HashMap<String,SquadTaskState>>>` | estado | todas as tarefas vivas | |
| `SquadHub.hitl_timeout` | `Duration` | config | `BTV_HITL_TIMEOUT_SECS` (default 300) | fail-closed Deny |
| `SquadHub.next_task_seq` | `Arc<AtomicU64>` | estado | `new_task`→`sq{n:x}` | |
| `new_task()` | `String` | saída | gera task_id + estado vazio | |
| `seed_task_seq(maior)` | — | estado | CAS acima do persistido | idempotente, nunca reduz |
| `publish` | — | saída | evento → log + broadcast | |
| `finish_task` | — | estado | dropa `tx` → SSE fecha | |
| `subscribe` | `(Vec<SquadEvent>, Option<Receiver>)` | saída | snapshot + live (`None` se terminada) | |
| `request_hitl` | `bool` | intermediário | `pending`=oneshot, `timeout(hitl_timeout)` | timeout → false |
| `resolve_hitl(task_id,allow)` | `Result<(),()>` | intermediário | `responder.send(allow)` | |
| `push_user_message(task_id,text)` | `Result<(),()>` | entrada→estado | inbox.push_back + eco `ChatMessage(HUMAN)` | |
| `take_user_message`/`drain_user_messages` | `Option<String>`/`Vec<String>` | intermediário | inbox → injeção | |
| `note_tool_run` | — | estado | após `core_run_tool` | scope de `edit`=path |
| `tool_runs(task_id)` | `Vec<ToolRunNote>` | saída | → registrar_entregas | |
| `register_abort` | — | estado | pós-spawn; aborta se já stopped | |
| `emergency_stop(task_id,reason)` | `Result<(),()>` | intermediário | marca stopped, aborta, nega HITL, publica Error, `finish_task` | idempotente |
| `is_stopped` | `bool` | intermediário | checagem Prioridade-Zero | |
| `WebSquadCoreBackend{generator,hub,task_id,root,tools,tool_permissions}` | struct (impl `CoreBackend`) | estado | Generate/RequestPermission/RunTool reais | |
| `ScriptedSquadCoreBackend{...}` | struct (impl `CoreBackend`) | estado | respostas por `requester`, confiança 0.5 (consenso fraco) | exercita HITL |
| `inject_cockpit_context(hub,task_id,req)` | `LlmRequest` | intermediário/wire | drena inbox → turnos `user` em `messages_json` | parse antes de drenar; formato inválido preserva inbox |
| `req.messages_json` | JSON string | wire | LlmRequest → array de mensagens | cada orientação vira `{role:"user",content:"[orientação do cockpit...] {text}"}` |
| `run_squad_task`/`run_squad_task_inner` | fn | — | sobe CoreService, roda /verify (roster vazio), acquire slot, drena stream | |
| `SOCKET_READY_TIMEOUT` | `Duration` (2s) | config | `wait_for_socket` | |
| `core_sock` | `PathBuf` | wire | `.btv/squad-pool-core.sock` | reusado sequencial (cap 1) |
| `verification_evidence` | `Option<proto::VerificationEvidence>` | wire | roster vazio→/verify; produto→None | fail-closed só p/ código |
| `SquadTask{task_id,description,decision_type:"architecture",max_autonomy_level:3,verification_evidence,model,roster,tenant_id:LOCAL,actor:"web:squad"}` | proto | saída/wire | `client.execute_task` | `max_autonomy_level` ignorado (ADR 0021) |
| `stream.message()` | `SquadEvent` | entrada/wire | orquestrador → publish | `Consensus` → ledger `squad.consensus`{decision_maker,strength,requires_human} |
| `failure` | `Option<String>` | intermediário | evento Error ou status → fallback | |
| `RunSquadBody{task,model?}` | struct | entrada/wire | `POST /api/squad/run` | model default `BTV_SQUAD_MODEL` |
| `RunSquadResponse{task_id}` | struct | saída/wire | 202 Accepted | |
| `SquadAgentState{hub,pool}` | struct | estado | axum State | |
| `start_squad_task(state,description,model?,roster)` | `Result<String,Box<Response>>` | intermediário | gera task_id, escolhe backend, spawn, register_abort | compartilhado com btv_agent |
| `squad_sse_handler` | `Sse<Stream>` | saída/wire | `GET /api/squad/{task_id}/events` | snapshot.chain(live/empty) |
| `ResolveHitlBody{allow}` | struct | entrada/wire | `POST .../hitl` | 200/404 |
| `PostMessageBody{text}` | struct | entrada/wire | `POST .../message` | 202 sem corpo; vazio → 400 |
| `EmergencyStopBody{reason?}` | struct | entrada/wire | `POST .../emergency-stop` | default "solicitado pelo operador" |
| `router(hub,pool)` | `Router` | saída | 5 rotas squad | |
| `squad_model()` | `String` | config | `BTV_SQUAD_MODEL` (default `claude-sonnet-5`) | |
| `default_squad_pool(root)` | `Arc<SquadPool>` | estado | cap 1, socket_dir `.btv/squad-pool`, timeout 30s | lazy |
| `default_hub()` | `SquadHub` | estado | `BTV_HITL_TIMEOUT_SECS` | |

Fluxo: `POST run`/ativação→`start_squad_task` escolhe backend (scripted/real) e spawn `run_squad_task`; um `serve_core` in-process expõe Generate/RunTool/RequestPermission ao Python via UDS; o stream `SquadEvent` é publicado no broadcast (SSE) e o consenso vai ao ledger; HITL/message/emergency-stop chegam por HTTP e mexem no `SquadHub`; cockpit injeta inbox no próximo `Generate`.

---

## src/btv_agent.rs

Papel: produto BuildToValue — ativação de squad pela galeria/wizard, gates com auditoria, personas (U7), designer, admin (publicação/usuários), download de entregas. 22 rotas sob `Tenant` extractor + ledger + `BtvStore`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `prompt_padrao(indice,papel,template_nome)` | `String` | config/intermediário | 4 arquétipos (abrir/produzir/revisar/validar) | índice>3 herda 4º |
| `RespostaBriefing{label,resposta}` | struct | entrada/wire | corpo de ativação | |
| `AtivarSquadBody{template_id,nome?,briefing[],refs[],papeis_off[]}` | struct | entrada/wire | `POST /api/btv/squads` | papeis_off = índices desligados |
| `AtivarSquadResponse{task_id,run_id}` | struct | saída/wire | 202 Accepted | |
| `BtvAgentState{squad,ledger,store,tenant_resolver}` | struct | estado | axum State; `FromRef<TenantResolucao>` | |
| `funcao_por_indice(i)` | `&str` | intermediário | 0=plan,1=produce,2=review,3=validate,_=produce | mapeia posição→função |
| `montar_descricao(...)` | `String` | intermediário/wire | briefing+refs+equipe(prompts efetivos)+entregas+gates → task description | inclui personas próprias |
| `papeis_ativos` | `Vec<(usize,&str)>` | intermediário | template.papeis filtrado por papeis_off | vazio → 422 no_roles |
| `overrides` | `HashMap<String,String>` | entrada→intermediário | `store.list_overrides(ctx,template.id)` | erro → 500 (nunca mente procedência) |
| `proprias` | `Vec<CustomPersona>` | entrada→intermediário | `store.list_custom(ctx,template.id)` | personas de produção |
| `prompt_efetivo(i,papel)` | closure→`String` | intermediário | override ?? prompt_padrao | entra em descrição + hash |
| `prompt_hashes` | `Vec<PromptHash{role,prompt_sha256,custom}>` | intermediário/wire | sha256 de cada prompt efetivo → ActivationFacts | procedência U7↔U4 |
| `roster` | `Vec<proto::PersonaSpec{papel,prompt,funcao,ordem,custom}>` | saída/wire | → `start_squad_task` | Python usa `prompt` como system do estágio |
| `task_id` | `String` | intermediário | `start_squad_task(...,None,roster)` | model=default do deploy |
| `briefing_json` | JSON string | wire | briefing → `Run::activate` | |
| `run` (`btv_domain::Run::activate`) | agregado | intermediário | ctx+task_id+template+nome+briefing+roles+ts | 422 se activate_error |
| `run_id` | `i64` | saída/wire | `store.save`+relê `store.get` → id numerado | fail-closed em id=0 |
| `ActivationFacts{custom_personas,prompt_hashes,refs}` | struct | wire | → `activation_event` → ledger | |
| `evento` (activation) | `DomainEvent` | saída/wire | `LedgerRepository::append(ctx,evento)` | `btv.*` |
| `spawn_status_watcher(state,task_id,ctx)` | fn | — | subscribe até canal fechar → `set_status` | ctx CAPTURADO da requisição |
| `RunStatus` | enum | saída/wire | kill-switch>erro>Concluida → store | |
| `registrar_entregas(ctx,store,ledger,hub,task_id,now)` | fn | — | arquivos escritos → deliverables + `btv.export_generated` | só em Concluida |
| `arquivos_escritos(runs)` | `Vec<String>` | intermediário | filtra `edit` exit 0, dedup preservando ordem | função pura |
| `insert_deliverable(...)` | `deliverable_id` | wire | → store + `DeliverableProduced` no ledger | trilha = papeis + gates |
| `trilha` | `String` | wire | `"{papeis} · {N} gate(s) aprovado(s) por você"` | |
| `list_runs_handler` | `Vec<Run>` | saída/wire | `GET /api/btv/squads` → `store.list(ctx)` | leitura pela porta |
| `GateBody{etapa?}` | struct | entrada/wire | `POST /api/btv/squads/{task_id}/gate` | |
| `aprovar_gate_handler` | 200/404 | saída | `resolve_hitl(true)` + `Run::approve_gate` → `btv.gate_approved` | |
| `AjusteBody{instrucao,etapa?}` | struct | entrada/wire | `POST .../ajuste` | vazio → 400 |
| `pedir_ajuste_handler` | 200/404 | saída | `push_user_message`(cockpit) + `resolve_hitl(true)` + `AdjustRequested` | negar HITL abortaria (comentário módulo) |
| `list_deliverables_handler` | `Vec<Deliverable>` | saída/wire | `GET /api/btv/deliverables` | |
| `download_deliverable_handler` | bytes/JSON | saída/wire | `GET /api/btv/deliverables/{id}/download` | texto direto; binário via `convert::convert`; sem conversor → 422; arquivo sumido → 404 |
| `binario`/`content`/`conv` | flags/bytes | intermediário | template.formatos → `convert` | content-type + content-disposition |
| `PersonaView{papel,prompt,padrao,editado}` | struct | saída/wire | `GET /api/btv/personas/{template_id}` | |
| `PersonasResponse{template_id,personas,proprias}` | struct | saída/wire | | |
| `PromptBody{prompt}` | struct | entrada/wire | `PUT .../personas/{template_id}/{papel}` | override + `PersonaUpdated`(sha256) |
| `delete_override_handler`/`clear_overrides_handler` | 200 | saída | `DELETE .../{papel}` / `DELETE .../{template_id}` | restaura padrão |
| `CustomPersonaBody{nome,prompt}` | struct | entrada/wire | POST/PUT custom | `create`→201{id}, `update`, `delete` |
| `SalvarFluxoBody{nome,diagram,versao_semantica?,snapshot_hash?,audit_head?,audit_len?}` | struct | entrada/wire | `POST /api/btv/designer/flows` | diagram opaco (BpmnDiagram) |
| `diagram_sha256` | `String` | wire | sha256 do diagrama canônico → `FlowSaved` | valida `nodes` não-vazio (422) |
| `salvar_fluxo_handler` | 201{seq,diagram_sha256} | saída/wire | `btv.flow_saved` no ledger | "salvo e validado", não aplica |
| `PublicacaoBody{publicado}` | struct | entrada/wire | `POST /api/btv/templates/{id}/publicacao` | `set_published` + `TemplatePublished` |
| `list_publicacao_handler` | `[{template_id,publicado}]` | saída/wire | `GET /api/btv/templates/publicacao` | overrides persistidos |
| `NovoUsuarioBody{nome,email,papel?,pin?}` | struct | entrada/wire | `POST /api/btv/users` | 201{id}; nome vazio → 422 |
| `list_users_handler` | `Vec<User>` | saída/wire | `GET /api/btv/users` | |
| `delete_user_handler` | 200/404 | saída | `DELETE /api/btv/users/{id}` + `UserRemoved` no ledger | |
| `AtivoBody{ativo}` | struct | entrada/wire | `POST .../users/{id}/ativo` | `set_active` |
| `SetPinBody{pin?}` | struct | entrada/wire | `POST .../users/{id}/pin` | vazio limpa PIN |
| `VerifyPinBody{pin}` | struct | entrada/wire | `POST .../users/{id}/verify-pin` | `{ok,reason}` no_pin/ok/wrong; hash nunca sai |
| `router(hub,pool,ledger,store,tenant_resolver)` | `Router` | saída | 22 rotas `/api/btv/*` | injeta `BtvAgentState` |

Fluxo: cada handler resolve `Tenant(ctx)` na borda→lê/escreve `BtvStore` pela porta de domínio→emite `DomainEvent` no ledger hash-chain; ativação monta descrição+roster dos prompts efetivos, dispara o motor real via `start_squad_task`, persiste o `Run` e agenda `spawn_status_watcher` que grava entregas reais na conclusão.

---

## src/squad.rs

Papel: comando `btv squad` (CLI) — `CoreBackend` real (Gateway), fallback 3 níveis (squad→agente-único→safe-mode), `core_run_tool`/`core_generate` compartilhados, `evidence_to_proto`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `TOOL_EXIT_OK/ERROR/DENIED` | `const i32` (0/1/-1) | config | convenção dos 3 backends | |
| `core_run_tool(tools,permissions,call,root,ask)` | `ToolResult` | intermediário/saída | `ToolCall` → executa sob permissão | recalcula scope via `Tool::scope` (nunca usa `call.scope` da rede) |
| `args` | `Value` | intermediário | `call.args_json` parse | inválido → exit ERROR |
| `scope` | `String` | intermediário | `tools.get(tool).scope(args)` | decisão de permissão |
| `allowed` | `bool` | intermediário | `permissions.evaluate` (Allow/Deny/Ask→`ask`) | |
| `PermissionRequest{tool,scope,reason,confidence:0.0}` | proto | wire | Ask → bridge HITL | |
| `ToolResult{content,truncated,exit_code}` | proto | saída/wire | → Python | overflow_path anexado ao content |
| `log_tool_run` | LedgerEntry `squad.tool_run` | saída/wire | `{tool,scope,exit_code,truncated}` | best-effort |
| `evidence_to_proto(e)` | `proto::VerificationEvidence` | intermediário/wire | schema canônico → proto tipado (D3t) | espelho 1:1 |
| `tool_scope(tools,call)` | `String` | intermediário | correlaciona execução → entrega | |
| `WireMsg{role,content}` | struct | entrada/wire | `messages_json` → chat | |
| `core_generate(generator,req)` | `(String,Usage)` | intermediário/wire | desempacota messages_json, chama Generator | system separado; assistant→`Role::Assistant`; resto→User |
| `GenerateRequest{model,system,messages,tools:[],max_tokens,temperature}` | struct | intermediário | → `generator.generate` | max_tokens default 4096 |
| `Usage{input_tokens,output_tokens,cache_hit,provider}` | proto | saída/wire | turn → Python | cache_hit = provider contém "+cache" |
| `GatewayCoreBackend{generator,auto_yes,root,tools,tool_permissions}` | struct (impl `CoreBackend`) | estado | HITL via stdin `[s/N]` | |
| `locate_python_dir()` | `Option<PathBuf>` | intermediário/config | `BTV_PYTHON_DIR` ou `python/pyproject.toml` subindo | reusado por squad_agent/prompt_render |
| `run_squad(generator,opts,root,task)` | `Result<()>` | — | 3 níveis de fallback | |
| `core_sock`/`squad_sock` | `PathBuf` | wire | `.btv/squad-core-{pid}.sock` / `squad-{pid}.sock` | UDS |
| `try_squad(...)` | `Result<(),String>` | intermediário | sobe supervisor, /verify, execute_task, drena | Err dispara fallback |
| `evidence` | `VerificationEvidence` | intermediário/wire | `/verify` antes do squad → SquadTask | fail-closed p/ auditor |
| `SquadTask{task_id:"s{pid:x}",description,decision_type:"architecture",max_autonomy_level:3,verification_evidence,model,roster:[],tenant_id:LOCAL,actor:"cli:squad"}` | proto | saída/wire | `client.execute_task` | |
| `render_and_record(stream,session)` | `SquadRun` | intermediário | drena + render + ledger | Completed/Failed |
| `render_event` | — | saída | SquadEvent → stderr | Proposal/Consensus/Handoff/Hitl/Step/Error/Chat |
| `squad.consensus` | LedgerEntry | saída/wire | Consensus → `{decision_maker,strength,requires_human,ts}` | |
| `safe_mode(task)` | — | saída | nível 3 → stderr | nenhuma escrita |

Fluxo: sobe `serve_core`(Gateway) sobre UDS→`try_squad` roda /verify e `execute_task`; `render_and_record` renderiza eventos e grava consenso; falha degrada squad→`run_once`→`safe_mode`.

---

## src/tui_app.rs

Papel: comando `btv tui` (ratatui) — loop de agente em task tokio conversando com a UI por canais (`TuiMsg`/`UiCommand`); modal de permissão; seletores de modelo/agente.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `MODEL_CHOICES` | `&[&str;6]` | config | seletor Ctrl+M | um por tier/provider |
| `TuiMsg` | enum | intermediário/wire | task agente → UI (mpsc unbounded) | Delta/TurnDone/Tool/Permission/Notice/Status/Idle/Fatal |
| `TuiMsg::Tool{name,detail,ok,diff}` | variante | wire | ToolFinished → item + diff | |
| `TuiMsg::Permission{tool,scope}` | variante | wire | resolver → modal | |
| `UiCommand` | enum | intermediário/wire | UI → task agente (mpsc unbounded) | Send/SetModel/SetAgent |
| `resp_tx/resp_rx` | `std_mpsc::channel<bool>` | intermediário | modal → `TuiResolver` | resposta de permissão |
| `apply(state,msg)` | fn (puro) | — | TuiMsg → TuiState | testável |
| `TuiResolver{evt_tx,resp_rx}` | struct (impl `PermissionResolver`) | estado | publica Permission, bloqueia até resp | |
| `TerminalGuard` | struct (Drop) | estado | restaura terminal | mesmo em erro |
| `status_line(opts)` | `String` | saída | modelo/agente/atalhos | |
| `sidecar_session`/`sidecar_client` | `Option` | estado | lint consultivo | |
| agente task: `ledger` `Session` | estado | `<tui>` no ledger | record_skill_vetting + user.turn | |
| `durable` | `DurableSession` | estado | `open_durable(<tui>)` | retomada notificada |
| `agent_loop` | `AgentLoop` | intermediário | reconstruído por turno (barato) | reflete modelo/agente correntes |
| `on_event` | closure | — | LoopEvent → ledger.record + TuiMsg | TextDelta→Delta, ToolFinished→Tool, etc. |
| key events | crossterm | entrada | teclado → UiCommand/modal | Ctrl+M modelo, Ctrl+G agente, Esc/Ctrl+C sai, Enter envia |
| `state.input` | `String` | estado/entrada | buffer de digitação | |

Fluxo: task tokio consome `UiCommand` (Send/SetModel/SetAgent), roda `continue_run` emitindo `TuiMsg` via mpsc; a thread principal (crossterm) desenha `TuiState`, captura teclado e responde permissão via `std_mpsc`.

---

## src/convert.rs

Papel: conversores de export por serialização determinística em Rust puro (sem sandbox) — DOCX/XLSX/PDF; SVG/MusicXML como texto; PNG/MIDI sem conversor (422).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `Converted{bytes,content_type,extension}` | struct | saída | `convert` → download handler | |
| `convert(formato,text)` | `Option<Converted>` | intermediário | formato (case-insensitive) → bytes | None p/ PNG/MIDI |
| `xml_escape`/`pdf_escape` | `String` | intermediário | escapa entidades | |
| `to_docx(text)` | `Vec<u8>` | intermediário | linhas → `<w:p>` → OOXML ZIP | `word/document.xml` + rels |
| `to_xlsx(text)` | `Vec<u8>` | intermediário | linha → `<row>` inlineStr | workbook + sheet1 |
| `to_pdf(text)` | `Vec<u8>` | intermediário | linhas → stream `Td`/`Tj` | 5 objetos + xref + trailer |
| `crc32(data)` | `u32` | intermediário | IEEE 802.3 poly | vetor teste 0xCBF43926 |
| `zip_stored(files)` | `Vec<u8>` | intermediário/wire | método 0 (stored) + CRC32 | local header + central dir + EOCD |

Fluxo: `download_deliverable_handler`→`convert(formato,texto)`→bytes servidos com content-type/disposition, ou 422 se sem conversor.

---

## src/tenant_extractor.rs

Papel: borda de tenant (ADR 0029/0026) — peça única que resolve `BTV_MODE`; extractor `Tenant`, layer universal `guarda_tenant`, `TenantResolucao`, `SessionResolver`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `ACTOR_LOCAL` | `const "web:btv"` | config | actor do modo local | byte-idêntico aos 6 handlers |
| `Mode` | enum {Local,Saas} | config/estado | propriedade do processo | resolvido 1× no arranque |
| `current_mode()` | `Mode` | config | `BTV_MODE` (=="saas" → Saas, senão Local) | única leitura de BTV_MODE (lint T4-D) |
| `SessionResolver` | trait | — | `resolve(token)→Option<(TenantId,String)>` | impl por PgStore (feature pg) |
| `TenantResolucao{mode,resolver}` | struct | estado/config | injetado no router + State | `from_env`/`new`/`local` |
| `Recusa` | enum | saída/wire | SemSessao/SessaoInvalida→401, SaasSemResolver→500 | `IntoResponse` |
| `extrair_token(headers)` | `Option<String>` | entrada | `Authorization: Bearer` ou cookie `btv_session` | não-vazio |
| `resolver_contexto(mode,headers,resolver)` | `Result<TenantContext,Recusa>` | intermediário | coração da borda | Local nunca recusa; Saas fail-closed |
| `TenantContext` | struct | saída | → handlers (actor `web:btv` local / `user:{id}` saas) | |
| `ROTAS_LIVRES` | `const &[&str]` (vazio) | config | allowlist sem sessão no saas | superfície inteira fail-closed |
| `autoriza_borda(mode,path,headers,resolver)` | `Result<(),Recusa>` | intermediário | layer universal | reusa resolver_contexto |
| `guarda_tenant(State(tr),req,next)` | middleware | intermediário | autoriza toda rota | Local = no-op |
| `Tenant(ctx)` | extractor | entrada | `from_request_parts` → handlers | `TenantResolucao: FromRef<S>` |

Fluxo: no arranque `from_env` lê `BTV_MODE` 1×; cada requisição passa pelo layer `guarda_tenant` (universal) e/ou extractor `Tenant` que resolve `(tenant,actor)` do token — local sempre LOCAL, saas fail-closed.

---

## src/skills.rs

Papel: loader de skills — descobre, veta (`vet_skill`) e registra como `SkillTool` no `ToolRegistry`; carrega servidores MCP/LSP declarados.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `build_registry(root)` | `ToolRegistry` | saída | ponto único de montagem | run/chat/tui |
| `build_registry_with_vetting(root)` | `(ToolRegistry, Vec<SkillStatus>)` | saída | + decisões p/ ledger sem re-vetar | fecha double-vet |
| `builtin_dir` | `<root>/skills/` | entrada | built-ins confiáveis (sem sandbox, vetados) | |
| `third_party_dir` | `<root>/.btv/skills/` | entrada | terceiros untrusted (sandbox Docker) | |
| `load_lsp_servers` | — | intermediário | `.btv/lsp.toml` → `lsp__<server>__{definition,references,diagnostics,symbol}` | fail-soft, lazy |
| `load_mcp_servers` | — | intermediário | `.btv/mcp.toml` → `mcp__<server>__<tool>` | fail-soft |
| `load_skills(registry,dir,sandboxed)` | `(usize, Vec<SkillStatus>)` | intermediário | descobre subdirs, veta, registra aprovados | |
| `SkillStatus{id,status,detail,source}` | struct | saída/wire | → ledger `skill.vetting` | inclusive bloqueadas |
| `result.decision` | `Decision` (Vet/Block) | intermediário | Block → não registra + loga findings critical | |
| `SkillManifest{name,description,entrypoint,...}` | struct | entrada/wire | `read_manifest` (skill.toml) | sem entrypoint/colisão → não registra |
| `SkillTool` | tool | estado | `registry.register` | `.sandboxed()` p/ terceiros |

Fluxo: `default_set`→carrega `skills/` (builtin) + `.btv/skills/` (sandboxed) vetando cada uma → MCP/LSP declarados; devolve registry + statuses p/ auditoria.

---

## src/mcp_console.rs

Papel: console MCP (A1) — `GET /api/mcp`: sonda cada servidor de `.btv/mcp.toml` e calcula preview de política real por tool.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `PROBE_TIMEOUT` | `const Duration` (5s) | config | probe bounded | tela de status |
| `McpConsoleState{root}` | struct | estado | axum State | |
| `ToolPolicyPreview{build,plan}` | struct | saída/wire | `PermissionEngine::evaluate` + overrides | |
| `McpToolView{name,description,policy}` | struct | saída/wire | tool → JSON | name `mcp__<server>__<tool>` |
| `McpServerView{id,command,status,error?,tools}` | struct | saída/wire | `GET /api/mcp` | online/offline |
| `configs` | `Vec<ServerConfig>` | entrada | `read_server_configs(root)` | |
| `build_engine`/`plan_engine` | `PermissionEngine` | intermediário | perfil + `load_rule_overrides` | senão sempre "ask" p/ mcp__* |
| probe result | tools/erro | intermediário | `list_tools_blocking` em `spawn_blocking` + timeout | join panic/timeout → offline |
| `scope` | `String` | intermediário | `mcp:<id>/<tool>` | p/ evaluate |

Fluxo: lê configs→sonda cada servidor (bounded)→monta status + preview de política combinando perfil const com overrides persistidos.

---

## src/memory_console.rs

Papel: mapa de memória (A3) — `GET /api/memory` + `POST /api/memory/recall` sobre `MemoryService` (TF-IDF, rotulado RAG por honestidade).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `MemoryAgentState{service:Arc<MemoryService>}` | struct | estado | axum State | |
| `sidecar_error_response(e)` | `Response` | saída | Unavailable→503, Rpc→502 | |
| `ListQuery{agent?,limit?}` | struct | entrada/wire | `GET /api/memory?agent=&limit=` | limit default 50 |
| `list_handler` | `resp.agents` | saída/wire | `client.list` → JSON | contagem/decisão reais, sem tendência de esquecimento |
| `RecallBody{query,k?}` | struct | entrada/wire | `POST /api/memory/recall` | k default 5 |
| `recall_handler` | `resp.matches` | saída/wire | `client.recall` → JSON | busca léxica TF-IDF |
| `default_memory_service(root)` | `Arc<MemoryService>` | estado | py_dir + `.btv/memory.sock`, memory_dir None, 30s | |

Fluxo: cada rota abre `service.client()` (503 se indisponível) e chama list/recall via gRPC ao sidecar de memória; serializa direto a resposta.

---

## src/prompt_render.rs

Papel: render de prompts (Onda 5) — `POST /api/prompt/render` + `GET /api/prompt/generators` sobre `SidecarService` PromptForge de longa duração.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `PromptAgentState{service:Arc<SidecarService>}` | struct | estado | axum State | |
| `sidecar_error_response(e)` | `Response` | saída | Unavailable→503, Rpc→502 | |
| `RenderBody{generator,fields:HashMap}` | struct | entrada/wire | `POST /api/prompt/render` | |
| `RenderResponseBody{prompt}` | struct | saída/wire | `client.render` → JSON | |
| `generators_handler` | `Vec<GeneratorInfo>` | saída/wire | `GET /api/prompt/generators` | serde direto no proto |
| `default_sidecar_service(root)` | `Arc<SidecarService>` | estado | py_dir + `.btv/promptforge.sock`, 30s | lazy |

Fluxo: rotas abrem `service.client()` (503 se indisponível) e delegam render/list_generators ao PromptForge via gRPC.
