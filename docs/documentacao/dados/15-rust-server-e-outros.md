# Dicionário de Dados — `btv-server`, `btv-tui`, `btv-golden`, `btv-contract`

Mapa exaustivo de fluxo de dados (entrada / saída / intermediário / estado / config / wire)
dos crates Rust `crates/btv-server`, `crates/btv-tui`, `crates/btv-golden` e
`crates/btv-contract`. Cada seção documenta um arquivo `.rs`.

**Taxonomia de Direção:** `entrada` (param / corpo HTTP / leitura) · `saída` (retorno /
resposta HTTP / escrita) · `intermediário` (local / buffer, mesmo descartado) · `estado`
(campo de struct / AppState) · `config` (const / env) · `wire` (JSON HTTP / DB).

---

## crates/btv-server/src/lib.rs

Papel: wiring do dashboard axum — `AppState`, `router()`, `serve()`, resolução de diretórios de SPA por env var, e a suíte de testes de integração.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `AppState.telemetry` | `Telemetry` | estado | `router()` param → handlers de telemetria/admin/experiment | handle SQLite `.btv/telemetry.db` (offline-first); clonável (Clone deriva) |
| `AppState.prompt_library` | `Arc<Mutex<PromptLibrary>>` | estado | `router()` param → handlers de prompts | biblioteca `.btv/prompt_library.db`; lock por request |
| `AppState.ledger` | `Arc<Mutex<LedgerStore>>` | estado | `router()` param → handlers ledger/designer | `.btv/btv.db`, hash-chain append-only |
| `AppState.root` | `PathBuf` | estado | `router()` param → handlers verify/admin(skills) | raiz do workspace; resolve `btv.toml`, `skills/`, `.btv/skills/`, `git rev-parse` |
| `AppState.verify_job` | `VerifyJobSlot` = `Arc<Mutex<Option<VerifyJob>>>` | estado | inicializado `None` em `router()` → handlers verify | slot único em memória; reinício perde job (documentado na tela) |
| `telemetry` (param) | `Telemetry` | entrada | `router()`/`serve()` → `AppState` | injetado por `btv-cli::run_dashboard` |
| `prompt_library` (param) | `Arc<Mutex<PromptLibrary>>` | entrada | idem | idem |
| `ledger` (param) | `Arc<Mutex<LedgerStore>>` | entrada | idem | idem |
| `root` (param) | `impl AsRef<Path>` | entrada | `router()` → `AppState.root` | convertido para `PathBuf` |
| `web_dir` (param) | `impl AsRef<Path>` | entrada / config | `router()` → `ServeDir` da SPA primária | build de `btv-web/dist` (BuildToValue como SPA raiz) |
| `index_html` | `PathBuf` | intermediário | `web_dir.join("index.html")` → fallback do `ServeDir` | preserva status 200 do index em rotas SPA client-side |
| `serve_dir` | `ServeDir` (fallback `ServeFile`) | intermediário | assets estáticos → fallback do router | padrão SPA (fallback, não `not_found_service`) |
| `dev_dir` | `PathBuf` | intermediário | `default_dev_console_dir()` → montagem `/dev` | console dev `web/dist` |
| `dev_console` | `Option<ServeDir>` | intermediário | `dev_dir.join("index.html").exists()` → `nest_service("/dev")` | só monta se o build existe (sem 500, sem fake) |
| `router` (retorno) | `axum::Router` | saída | `router()` → `serve()`/testes/`merged_router` | com `AppState` + camada `guard::require_local_origin` |
| `addr` (param) | `SocketAddr` | entrada / config | `serve()` → `TcpListener::bind` | escuta só em `127.0.0.1` |
| fallback `/api/*` | resposta 404 JSON | saída / wire | rota `/api/*` desconhecida → `ErrorBody{code:"route_not_found"}` | não cai no SPA (evita confundir clientes de API) |
| fallback não-`/api` | resposta SPA | saída | rota desconhecida → `serve_dir.oneshot` (index.html) | navegação client-side |
| `BTV_WEB_DIR` | env var | config | `default_web_dir()` → diretório SPA primária | precedência: env → `btv-web/dist` |
| `BTV_DEV_WEB_DIR` | env var | config | `default_dev_console_dir()` → diretório console dev | precedência: env → `web/dist` (`base:'./'`) |

Rotas montadas em `router()` (método → handler):

| Rota | Método | Handler | Direção | Observação |
|------|--------|---------|---------|------------|
| `/api/summary` | GET | `telemetria::summary` | wire | resumo de telemetria |
| `/api/events` | GET | `telemetria::events` | wire | eventos recentes (`?limit`) |
| `/api/skills` | GET | `admin::skills` | wire | status do vetter (builtin + third-party) |
| `/api/prompts` | GET / POST | `prompts::list_prompts` / `create_prompt` | wire | CRUD biblioteca |
| `/api/prompts/{id}/favorite` | POST | `prompts::favorite_prompt` | wire | toggle favorito |
| `/api/prompts/{id}` | DELETE | `prompts::delete_prompt` | wire | remove |
| `/api/ledger` | GET | `ledger::list_ledger` | wire | leitura paginada (`?limit`,`?actor`) |
| `/api/ledger/verify` | POST | `ledger::verify_ledger` | wire | verifica hash-chain |
| `/api/models/usage` | GET | `telemetria::model_usage` | wire | uso + custo estimado por modelo |
| `/api/experiment/{nome}` | GET | `admin::get_experiment` | wire | relatório A/B |
| `/api/ratelimit` | GET | `admin::rate_limits` | wire | tetos por tier |
| `/api/providers` | GET | `providers::list_providers` | wire | providers configurados |
| `/api/verify/run` | POST | `verify::run_verify_start` | wire | dispara pipeline |
| `/api/verify/{id}` | GET | `verify::get_verify_status` | wire | polling de progresso |
| `/api/designer/workflow` | POST | `designer::save_workflow` | wire | salva grafo validado |
| `/api/btv/templates` | GET | `btv::list_templates` | wire | 12 modelos embutidos |
| `/dev` (nested) | * | `ServeDir(web/dist)` | wire | console dev, só se build existe |

Fluxo: params (telemetria/library/ledger/root/web_dir) → `AppState` clonável + `ServeDir` da SPA → `Router` com 17 rotas de API, fallback esperto (`/api/*`→404 JSON, resto→SPA), `/dev` opcional aninhado, e camada de guarda de Origin sobre tudo.

---

## crates/btv-server/src/handlers/mod.rs

Papel: vocabulário compartilhado dos handlers — `ErrorBody` (agora `pub`: **fonte única** do contrato de erro, reexportada pelo `btv-cli::web_agent`, B6), helpers `now_rfc3339()`, `db_error()`, e declaração dos submódulos por área.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `ErrorBody.error` | `String` | saída / wire | mensagem → corpo JSON de erro | forma uniforme das rotas mutáveis |
| `ErrorBody.code` | `String` | saída / wire | código curto → corpo JSON | ex.: `route_not_found`, `forbidden_origin` |
| `ErrorBody::new(code, message)` | fn | intermediário | `(code, message)` → `ErrorBody` | `error`=message, `code`=code |
| `now_rfc3339()` retorno | `String` | intermediário | `OffsetDateTime::now_utc()` → RFC3339 | fallback `"1970-01-01T00:00:00Z"` se format falhar |
| `db_error(message)` retorno | `Response` | saída / wire | erro de store → 500 `ErrorBody{code:"prompt_library_error"}` | resposta uniforme de erro de persistência |

Fluxo: submódulos `{admin, designer, ledger, prompts, providers, telemetria, verify}` mais o vocabulário de resposta (`ErrorBody`) e dois helpers (timestamp de servidor + erro de DB) reusados por todos.

---

## crates/btv-server/src/handlers/telemetria.rs

Papel: handlers de telemetria (`summary`, `events`) e uso por modelo (`model_usage`) com custo estimado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `summary` retorno | `Json<...>` | saída / wire | `state.telemetry.summary()` → corpo JSON | inclui `total_events` |
| `EventsQuery.limit` | `Option<u32>` | entrada / wire | query string `?limit` → `recent()` | default 50 |
| `events` retorno | `Json<Vec<...>>` | saída / wire | `state.telemetry.recent(limit)` → array de eventos | |
| `ModelUsageEntry.model` | `String` | saída / wire | `u.model` → JSON | id do modelo |
| `ModelUsageEntry.tier` | `ModelTier` | saída / wire | `tier_from_id(&u.model)` → JSON | derivado, não fabricado (`small`/`medium`/`large`) |
| `ModelUsageEntry.calls` | `u64` | saída / wire | agregação de `llm.call` | |
| `ModelUsageEntry.cache_hits` | `u64` | saída / wire | agregação `cache.hit` | |
| `ModelUsageEntry.cache_misses` | `u64` | saída / wire | agregação `cache.miss` | |
| `ModelUsageEntry.input_tokens` | `u64` | saída / wire | soma tokens reais | |
| `ModelUsageEntry.output_tokens` | `u64` | saída / wire | soma tokens reais | |
| `ModelUsageEntry.provider` | `Option<&'static str>` | saída / wire | `pricing::price_for(model).provider` | `None`/omitido se sem preço (`skip_serializing_if`) |
| `ModelUsageEntry.estimated_cost_usd` | `Option<f64>` | saída / wire | `pricing::estimate_cost_usd(model,in,out)` | ESTIMATIVA (tokens × preço tabelado); omitido se sem preço |
| `ModelUsageResponse.entries` | `Vec<ModelUsageEntry>` | saída / wire | map de `telemetry.model_usage()` | |
| `ModelUsageResponse.total_estimated_cost_usd` | `f64` | saída / wire | soma dos `cost` presentes | acumulado em `total` |
| `ModelUsageResponse.pricing_as_of` | `&'static str` | saída / wire | `pricing::AS_OF` | data de referência da tabela |
| `total` | `f64` | intermediário | acumulador local no map | somado só quando `cost` é `Some` |

Fluxo: `state.telemetry` → `summary()`/`recent(limit)`/`model_usage()`; o `model_usage` enriquece cada linha com `tier` (regex), `provider` e `estimated_cost_usd` (tokens reais × tabela estática) e soma o total, marcando a data da tabela.

---

## crates/btv-server/src/handlers/ledger.rs

Papel: leitura paginada (`list_ledger`) e verificação da hash-chain (`verify_ledger`) do ledger. Contrato pinado pelo golden T1 `ledger`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `LedgerQuery.limit` | `Option<u32>` | entrada / wire | query `?limit` → `recent()` | default 50 |
| `LedgerQuery.actor` | `Option<String>` | entrada / wire | query `?actor` → `recent()` | filtro resolvido no SQL, combinado com LIMIT |
| `list_ledger` retorno | `Json<Vec<LedgerEntry>>` | saída / wire | `ledger.recent(limit, actor)` → array | mais recente primeiro; erro → `db_error` |
| lock do ledger | `MutexGuard` | intermediário | `state.ledger.lock()` | `unwrap_or_else(into_inner)` (recupera de poison) |
| `VerifyResponse.ok` | `bool` | saída / wire | resultado de `verify_chain()` | `true`=íntegra, `false`=corrompida |
| `VerifyResponse.verified` | `u64` | saída / wire | contagem verificada | em `BrokenChain{seq}`: `seq.saturating_sub(1)` |
| `VerifyResponse.error` | `Option<String>` | saída / wire | `"cadeia corrompida na seq {seq}"` | omitido se `None` (`skip_serializing_if`) |
| `verify_ledger` corrupção | corpo 200 `ok:false` | saída / wire | `LedgerError::BrokenChain` → JSON | corrupção é `ok:false` no corpo, NÃO status de erro |

Fluxo: query `?limit&actor` → `LedgerStore::recent` (SQL faz filtro+limit) devolvendo `Vec<LedgerEntry>` mais novo primeiro; `verify_chain` percorre a cadeia e reporta integridade no corpo (`ok`/`verified`/`error`), distinguindo "servidor falhou" (500) de "dado adulterado" (200 `ok:false`).

---

## crates/btv-server/src/handlers/prompts.rs

Papel: CRUD da biblioteca de prompts (`list_prompts`, `create_prompt`, `favorite_prompt`, `delete_prompt`) sobre `.btv/prompt_library.db`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `ListPromptsQuery.tag` | `Option<String>` | entrada / wire | query `?tag` → `library.list()` | filtro por tag exata |
| `list_prompts` retorno | `Json<Vec<...>>` | saída / wire | `library.list(tag)` → array | mais recentes primeiro; erro → `db_error` |
| `CreatePromptBody.name` | `String` | entrada / wire | corpo POST → `library.save` | |
| `CreatePromptBody.generator` | `String` | entrada / wire | corpo POST → `save` | ex.: `code-review` |
| `CreatePromptBody.fields` | `Value` (`#[serde(default)]`) | entrada / wire | corpo POST → `save` | JSON livre; default `Null` |
| `CreatePromptBody.rendered` | `String` | entrada / wire | corpo POST → `save` | prompt já renderizado (render é rota separada) |
| `CreatePromptBody.tags` | `Vec<String>` (`default`) | entrada / wire | corpo POST → `save` | |
| `created_at` | `String` | intermediário / wire | `now_rfc3339()` → `save` | gerado pelo SERVIDOR, nunca confiado ao corpo |
| `id` | `i64` | intermediário | `library.save(...)` → `library.get(id)` | id atribuído pelo store |
| `create_prompt` retorno | `(201, Json<saved>)` | saída / wire | `library.get(id)` → registro completo | `favorite:false`, `created_at` presente |
| `favorite_prompt` param `id` | `i64` (path) | entrada / wire | path `/{id}` → `toggle_favorite` | |
| `favorite_prompt` retorno | `Json{favorite:bool}` | saída / wire | `toggle_favorite(id)` → estado novo | `404` se id inexistente |
| `delete_prompt` param `id` | `i64` (path) | entrada / wire | path `/{id}` → `library.delete` | |
| `delete_prompt` retorno | `204 NO_CONTENT` | saída | `delete(id)==true` | `404` se inexistente |
| `prompt_not_found()` | `Response` | saída / wire | → `ErrorBody{code:"prompt_not_found"}` | 404 uniforme |

Fluxo: corpo/query/path → `Arc<Mutex<PromptLibrary>>` (lock por request); create gera `created_at` no servidor, salva, relê pelo id e devolve o registro completo (201); favorite/delete operam por id de path com 404 fail-closed.

---

## crates/btv-server/src/handlers/providers.rs

Papel: `GET /api/providers` — quais providers uma sessão real conseguiria usar, lendo os env vars que `Gateway::from_env` lê.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `KNOWN_PROVIDERS` | `[&str;3]` | config | const → iteração | `["anthropic","deepseek","openai"]` (ordem de fallback) |
| `ProviderView.id` | `&'static str` | saída / wire | cada provider → JSON | |
| `ProviderView.configured` | `bool` | saída / wire | `available.contains(id)` → JSON | mesma checagem de key que `Gateway::from_env` |
| `gateway` | `Gateway` | intermediário | `Gateway::from_env()` | lê env vars de API key |
| `available` | `HashSet<String>` | intermediário | `gateway.available()` → contains | providers com key definida e não-vazia |
| `list_providers` retorno | `Json<Vec<ProviderView>>` | saída / wire | map de `KNOWN_PROVIDERS` | sem mutação (reordenar fallback fica de fora) |
| `ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` / `OPENAI_API_KEY` | env vars | config | lidas por `Gateway::from_env` | decidem `configured` |

Fluxo: `Gateway::from_env()` lê as 3 env vars de key → `available()` (HashSet) → mapeia `KNOWN_PROVIDERS` para `ProviderView{id,configured}`; nunca fabrica estado ao vivo (dashboard não compartilha processo com sessão real).

---

## crates/btv-server/src/handlers/designer.rs

Papel: `POST /api/designer/workflow` — valida grafo `squad.workflow.v1` e grava no ledger ("salvar honesto").

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `workflow` (param) | `SquadWorkflow` | entrada / wire | corpo POST → `validate_edges()` | schema `squad.workflow.v1` (nodes+edges) |
| erro de validação | `422 ErrorBody{code:"invalid_workflow"}` | saída / wire | `validate_edges()` Err → resposta | cita o id (ex.: aresta pra nó `fantasma`) |
| `payload` | `Value` | intermediário / wire | `serde_json::to_value(&workflow)` → `LedgerEntry.payload` | erro de serialização → `db_error` |
| `entry.kind` | `String` | intermediário / wire | const `"designer.workflow_saved"` → ledger | kind `designer.*` |
| `entry.actor` | `String` | intermediário / wire | const `"web:designer"` → ledger | |
| `entry.ts` | `String` | intermediário / wire | `now_rfc3339()` | timestamp do servidor |
| `entry.tenant` | `Option<...>` | intermediário | `None` | cai na cadeia LOCAL (porta legada sem contexto) |
| `entry.seq`/`prev_hash`/`entry_hash` | 0 / vazio / vazio | intermediário | preenchidos por `append` | hashes atribuídos pelo store |
| `SaveWorkflowResponse.seq` | `u64` | saída / wire | `saved.seq` → JSON | seq real do ledger, não fabricado no cliente |
| `SaveWorkflowResponse.workflow_id` | `&'static str` | saída / wire | const `"squad.workflow.v1"` → JSON | |
| `save_workflow` sucesso | `201 Json<SaveWorkflowResponse>` | saída / wire | `ledger.append(entry)` Ok | validação ANTES do append (grafo inválido não grava nada) |

Fluxo: corpo `SquadWorkflow` → `validate_edges()` (422 fail-closed se aresta pendente, sem gravar) → serializa payload → `LedgerEntry{kind:designer.workflow_saved, actor:web:designer, tenant:None}` → `append` → devolve `seq` real + `workflow_id`. "Salvar honesto": não afirma que o orquestrador passou a usar o grafo.

---

## crates/btv-server/src/handlers/admin.rs

Papel: telas admin pequenas — experimentos A/B (`get_experiment`), rate limits (`rate_limits`), skills (`skills`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `get_experiment` param `nome` | `String` (path) | entrada / wire | path `/{nome}` → `experiment_variants` | |
| `variants` | `Vec<(String,n,s)>` | intermediário | `telemetry.experiment_variants(nome)` | variante, total, sucessos |
| experimento sem eventos | `404 ErrorBody{code:"experiment_not_found"}` | saída / wire | `variants.is_empty()` | `props.experiment` nunca bateu |
| 1 variante só | `422 ErrorBody{code:"experiment_needs_variants"}` | saída / wire | `variants.len() < 2` | não comparável (exige ≥2, Bonferroni) |
| `stats` | `Vec<VariantStats>` | intermediário | `VariantStats::new(v,n,s)` | |
| `report` | `ExperimentReport` | saída / wire | `from_variants(nome,"success_rate",stats,now)` | deriva `Serialize`+`JsonSchema` (`experiment.v1`); campos: `p_value`, `verdict`, `winner`, `variants[]`, `comparisons` |
| `RateLimitTierEntry.tier` | `ModelTier` | saída / wire | iteração dos 3 tiers → JSON | |
| `RateLimitTierEntry.cap` | `usize` | saída / wire | `limiter.max_requests()` → JSON | teto configurado |
| `RateLimitTierEntry.window_secs` | `u64` | saída / wire | `limiter.window().as_secs()` → JSON | |
| `rate_limits` retorno | `Json<Vec<RateLimitTierEntry>>` | saída / wire | map de `[Small,Medium,Large]` via `for_tier` | NÃO é uso ao vivo (limiter novo e vazio a cada req); sem campo "models" |
| `skills` retorno | `Json<Vec<...>>` | saída / wire | `list_skill_statuses(root/skills,"builtin")` + `list_skill_statuses(root/.btv/skills,"third-party")` | status real do vetter; read-only (fail-closed) |

Fluxo: `get_experiment` consulta a telemetria real (404 sem eventos, 422 com 1 variante, senão `ExperimentReport` multivariante); `rate_limits` expõe os 3 tetos de `RateLimiter::for_tier` (config, não uso); `skills` concatena vetting de skills built-in + terceiro.

---

## crates/btv-server/src/handlers/verify.rs

Papel: job de `/verify` em background com polling — estado `VerifyJob`/`VerifyJobSlot`, `run_verify_start` (dispara), `get_verify_status` (polling), `settle_verify_job` (assenta desfecho/panic).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `VerifyJobSlot` | `Arc<Mutex<Option<VerifyJob>>>` | estado | `AppState.verify_job` | slot único em memória |
| `VerifyJob.run_id` | `String` | estado / wire | `new_verify_run_id()` → resposta/polling | `run-{nanos hex}` |
| `VerifyJob.status` | `VerifyJobStatus` | estado | mutado pelo callback de progresso e `settle` | Running/Done/Failed |
| `VerifyJobStatus::Running{step,total}` | struct-variant | estado / wire | callback de progresso → JSON `status:"running"` | progresso real (`step` crescente) |
| `VerifyJobStatus::Done{evidence}` | struct-variant | estado / wire | pipeline Ok → JSON `status:"done"` | `VerificationEvidence` |
| `VerifyJobStatus::Failed{message}` | struct-variant | estado / wire | panic capturado → JSON `status:"failed"` | evita "running" eterno |
| `VerifyRunStarted.run_id` | `String` | saída / wire | → corpo 202/409 | |
| `run_verify_start` reserva | `202 Json{run_id}` ou `409 Json{run_id}` | saída / wire | check-and-reserve ATÔMICO sob 1 lock | 409 devolve o `run_id` do job já ativo (evita 2 pipelines no mesmo `target/`) |
| `config_path` | `PathBuf` | intermediário | `root.join("btv.toml")` | |
| `steps` | `Vec<StepSpec>` | intermediário | `load_config()` Ok → `to_step_specs()`, senão `default_steps()` | mesma config que `btv verify` (espelha job CI) |
| `sha` | `String` | intermediário | `verify_git_sha(root)` → evidência | `"unknown"` se falhar |
| `produced_at` | `String` | intermediário | `now_rfc3339()` → evidência | |
| callback de progresso | closure `(step,total,_completed)` | intermediário | `run_pipeline_with_progress` → muta slot `Running{step,total}` | via `progress_slot` clone |
| `result` | `thread::Result<VerificationEvidence>` | intermediário | `catch_unwind(AssertUnwindSafe)` | panic não engolido pelo JoinHandle descartado |
| `get_verify_status` param `id` | `String` (path) | entrada / wire | path `/{id}` → match `run_id==id` | |
| polling running | `Json{status,run_id,step,total}` | saída / wire | `Running` → JSON | |
| polling done | `Json{status,run_id,evidence,review}` | saída / wire | `Done` → JSON | `review`=`ValueReview::from_evidence` (derivado, substitui mock do front) |
| polling failed | `Json{status,run_id,message}` | saída / wire | `Failed` → JSON | |
| id desconhecido | `404 ErrorBody{code:"verify_run_not_found"}` | saída / wire | nenhum job ou `run_id != id` | |
| `new_verify_run_id()` | `String` | intermediário | `SystemTime nanos & 0xffff_ffff_ffff` → hex | `run-{hex}` |
| `verify_git_sha(root)` | `Option<String>` | intermediário | `git rev-parse HEAD` (cwd=`root`) | trim; resolve contra `state.root`, não cwd do processo |
| `panic_to_message(panic)` | `String` | intermediário | downcast `&str`/`String` | fallback `"panic sem mensagem"` |

Fluxo: POST reserva atomicamente o slot único (409 se já rodando) e faz `spawn_blocking` do pipeline real (`btv.toml` ou `default_steps()`), atualizando `Running{step}` via callback; `catch_unwind` converte panic em `Failed`; GET `/{id}` faz polling, devolvendo running/done(+evidence+review derivado)/failed, ou 404 se id não bate.

---

## crates/btv-server/src/btv.rs

Papel: `GET /api/btv/templates` — serve o catálogo embutido dos 12 modelos de squad.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `builtin_templates()` | `&'static [SquadTemplate]` | config / saída | `btv_schemas::squad_template::builtin_templates()` | fonte única no crate de contratos (C1); re-export de compat |
| `list_templates` retorno | `Json<&'static [SquadTemplate]>` | saída / wire | catálogo → corpo JSON | 12 modelos da galeria (U1), wizard (U2), tabela admin (A5) |

Fluxo: este módulo só SERVE o catálogo estático `builtin_templates()` (embutido no binário) como JSON; a ativação (`btv-cli::btv_agent`) o consome direto de `btv-schemas`, não via este crate.

---

## crates/btv-server/src/bin/loadgen.rs

Papel: binário `btv-loadgen` — servidor HTTP mínimo que embrulha `ScriptedGenerator` (sem key) e expõe `POST /generate` como alvo do load-test k6.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `BTV_LOADGEN_PORT` | env var | config | parse `u16` → `SocketAddr` | default 7900; escuta só `127.0.0.1` |
| `AppState.generator` | `Arc<ScriptedGenerator>` | estado | `ScriptedGenerator::echo("resposta de carga, sem key real")` | sem provider, sem API key |
| `port` | `u16` | intermediário / config | env parse → bind | fallback 7900 |
| `addr` | `SocketAddr` | intermediário | `([127,0,0,1], port)` | |
| `health` retorno | `&'static str` | saída / wire | `GET /health` → `"ok"` | |
| `GenerateRequest` (interno) | struct | intermediário | canned: model="scripted", system="", messages=[], tools=[], max_tokens=64, temperature=None | corpo do request HTTP é IGNORADO de propósito |
| `sink` | closure `|_:&str|{}` | intermediário | descarta stream | mede caminho, não input |
| `turn` | (retorno de `generate`) | intermediário | `generator.generate(req,&mut sink)` | `.expect` (scripted não falha) |
| `generate` retorno | `Json{text}` | saída / wire | `turn.text()` → JSON | resposta canned; mede overhead nosso (serialização/agregação), isolado de rede |

Fluxo: sobe axum com `ScriptedGenerator::echo` fixo; `POST /generate` ignora o corpo, roda o gerador roteirizado (sem key) e devolve `{text}` — o k6 martela isso para validar o P95 do caminho do gateway.

---

## crates/btv-server/src/doctor_console.rs

Papel: `GET /api/doctor` — agrega 5 checagens (providers, uv, docker, git, vocabulário) numa resposta; router aditivo `.merge()`ável.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `DoctorCheck.id` | `&'static str` | saída / wire | cada check → JSON | `providers`/`uv`/`docker`/`git`/`vocabulario` |
| `DoctorCheck.ok` | `bool` | saída / wire | resultado da checagem → JSON | |
| `DoctorCheck.detail` | `String` | saída / wire | texto humano → JSON | |
| `DoctorView.checks` | `Vec<DoctorCheck>` | saída / wire | vetor das 5 checagens (ordem fixa) → JSON | |
| `KNOWN_PROVIDERS` | `[&str;3]` | config | const | `["anthropic","deepseek","openai"]` (dup intra-crate, candidata a dedup) |
| `providers_check` | `DoctorCheck` | intermediário | `Gateway::from_env().available()` count | ok=`configured>0`; detail `"{n}/3 provider(s) configurado(s)"` |
| `uv_check_with_path(path_override)` | `DoctorCheck` | intermediário | `uv --version` (`status.success()`) | PATH injetável p/ teste; uv quebrado = ausente |
| `git_check` | `DoctorCheck` | intermediário | `btv_verify::git_sha()` → HEAD[..8] | reusa helper compartilhado |
| `docker_check` | `DoctorCheck` (async) | intermediário | `Sandbox::ping().await` | daemon Docker alcançável |
| `vocabulario_check(stores)` | `DoctorCheck` | intermediário | `btv.linhas_fora_do_vocabulario()` + `ledger.kinds_fora_do_vocabulario()` | aponta LINHA ofensora (`tabela[linha].coluna = valor (erro)`) |
| `fora` | `Vec<VocabViolation>` | intermediário | append das duas varreduras | vazio → ok=true |
| `VocabViolation.{tabela,linha,coluna,valor,erro}` | campos | wire | scan SQL (btv-store) → detail | diagnóstico acionável, não contador |
| `DoctorStores.ledger` | `Arc<Mutex<LedgerStore>>` | estado / entrada | injetado por `btv dashboard` | mesma instância dos handlers BTV |
| `DoctorStores.btv` | `Arc<Mutex<BtvStore>>` | estado / entrada | injetado | idem, nenhuma conexão paralela |
| `get_doctor` retorno | `Json<DoctorView>` | saída / wire | 5 checagens reexecutadas por request | sem cache (custo baixo) |
| `router(stores)` | `Router` | saída | `/api/doctor` GET + state | aditivo |

Fluxo: `DoctorStores` (ledger+btv injetados) → `get_doctor` reexecuta a cada request providers(env)/uv(subprocess)/docker(ping)/git(sha)/vocabulário(scan SQLite) e agrega em `DoctorView{checks[5]}` de ordem fixa.

---

## crates/btv-server/src/lsp_console.rs

Papel: `GET /api/lsp` — enumera `.btv/lsp.toml` para exibição, sem subir nenhum processo de language server.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `LspConsoleState.root` | `PathBuf` | estado / entrada | `router(root)` → `read_server_configs` | raiz do workspace |
| `LspServerView.id` | `String` | saída / wire | `c.id` → JSON | |
| `LspServerView.command` | `String` | saída / wire | `c.command` → JSON | |
| `LspServerView.args` | `Vec<String>` | saída / wire | `c.args` → JSON | |
| `servers` | `Vec<LspServerView>` | intermediário | `btv_tools::lsp::read_server_configs(root)` map | zero probe: nunca sobe o processo |
| `list_lsp` retorno | `Json<Vec<LspServerView>>` | saída / wire | servers → corpo | lista vazia se sem `.btv/lsp.toml` (fail-soft) |
| `router(root)` | `Router` | saída | `/api/lsp` GET + state | aditivo |

Fluxo: `read_server_configs(root)` lê `.btv/lsp.toml` (declarado, não iniciado) → mapeia para `LspServerView{id,command,args}`; sem config devolve `[]`. Nunca introspecta se algum outro processo já subiu o servidor (mostrar "rodando" fabricado seria pior).

---

## crates/btv-server/src/sandbox_console.rs

Papel: `GET /api/sandbox` — perfil de confinamento do sandbox Docker (defaults + constantes hardcoded) e resultado real de `Sandbox::ping()`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `SandboxProfileView.image` | `String` | saída / wire | `profile.image` → JSON | campo real de `Sandbox::new` |
| `SandboxProfileView.network_disabled` | `bool` | saída / wire | `profile.network_disabled` → JSON | |
| `SandboxProfileView.mem_limit_mb` | `u64` | saída / wire | `profile.mem_limit_mb` → JSON | |
| `SandboxProfileView.cpu_quota` | `f64` | saída / wire | `profile.cpu_quota` → JSON | |
| `SandboxProfileView.timeout_secs` | `u64` | saída / wire | `profile.timeout.as_secs()` → JSON | |
| `SandboxProfileView.rootfs_readonly` | `bool` | saída / wire | const `true` | literal de `run_with` (não campo do struct) |
| `SandboxProfileView.cap_drop_all` | `bool` | saída / wire | const `true` | literal de `run_with` (cap-drop ALL) |
| `SandboxProfileView.no_new_privileges` | `bool` | saída / wire | const `true` | literal de `run_with` |
| `SandboxView.profile` | `SandboxProfileView` | saída / wire | `Sandbox::new(PathBuf::new())` → view | |
| `SandboxView.ping` | `bool` | saída / wire | `Sandbox::ping().await` | `false` fail-closed sem daemon; zero probe extra (não sobe container) |
| `get_sandbox` retorno | `Json<SandboxView>` | saída / wire | profile + ping | |
| `router()` | `Router` | saída | `/api/sandbox` GET | aditivo (sem state) |

Fluxo: `Sandbox::new(PathBuf::new())` fornece o perfil (image/network/mem/cpu/timeout) e as 3 constantes de confinamento hardcoded de `run_with` são expostas como literais; `ping()` reporta a saúde real do daemon sem subir container.

---

## crates/btv-server/tests/golden_http.rs

Papel: golden tests dos fluxos HTTP `GET /api/btv/templates` e `GET /api/ledger` — comparação byte-a-byte via `btv-golden`. Ambos 100% determinísticos (sem campos voláteis).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `fixture_web_dir()` | `TempDir` | intermediário | `index.html` mínimo | router exige `web_dir` |
| `app_com_ledger(ledger)` | `(Router, TempDir)` | intermediário | `btv_server::router(...)` in-memory | telemetria/library em memória |
| `drive(app,method,uri)` retorno | `(u16, Option<String>×2, Value)` | intermediário / wire | `oneshot` → status, content-type, content-disposition, body | body: JSON parse ou `{text}` fallback |
| `status` | `u16` | intermediário / wire | `resp.status().as_u16()` | comparado no golden |
| `content_type`/`content_disposition` | `Option<String>` | intermediário / wire | headers | |
| `body` | `Value` | intermediário / wire | bytes → JSON | `Null` se vazio |
| `entrada(kind,actor,payload,ts)` | `LedgerEntry` | intermediário / wire | ts FIXO → hashes determinísticos | `seq=0`, hashes vazios (preenchidos por append) |
| `golden_templates` | teste | saída | `/api/btv/templates` → fixture `templates` | 12 modelos, sem voláteis |
| `golden_ledger` | teste | saída | 2 steps: `/api/ledger` + `?actor=humano&limit=2` → fixture `ledger` | ts fixos → `entry_hash`/`prev_hash` exatos (não voláteis) |
| fixtures | `schemas/fixtures/http/{templates,ledger}.golden.json` | wire | resposta REAL gravada | mudança no algoritmo da cadeia = mudança de contrato |

Fluxo: monta o router real in-process com stores em memória, dispara os GETs via `oneshot`, captura status+headers+body e compara com a fixture gravada; timestamps fixos tornam os hashes da cadeia determinísticos, então nenhum campo é mascarado.

---

## crates/btv-golden/src/lib.rs

Papel: comparador de contrato golden (dev-dependency de `btv-server` e `btv-cli`) — `Volatile`/`Kind`, `GoldenRequest`/`GoldenResponse`/`GoldenStep`, mascaramento de campos voláteis, `check()` com regravação gated.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `Kind::Str` | enum-variant | config | tipo esperado de volátil | string não-vazia → `<volatil>` |
| `Kind::Num` | enum-variant | config | | número → `-1` |
| `Kind::StrOuVazia` | enum-variant | config | | vazio preservado (info: início de cadeia), não-vazio → `<volatil>` |
| `Volatile.path` | `&'static str` | config | pointer com curinga (`/task_id`, `/*/ts`) → `walk` | `*` percorre array/objeto |
| `Volatile.kind` | `Kind` | config | | |
| `vstr`/`vnum`/`vstr_ou_vazia` | fns | intermediário | construtores de `Volatile` | |
| `GoldenRequest.method` | `String` | wire | passo → fixture | |
| `GoldenRequest.path` | `String` | wire | passo → fixture | id volátil já substituído pelo chamador |
| `GoldenRequest.body` | `Option<Value>` | wire | corpo do request → fixture | omitido se `None` |
| `GoldenResponse.status` | `u16` | wire | resposta → fixture | |
| `GoldenResponse.content_type` | `Option<String>` | wire | | omitido se `None` |
| `GoldenResponse.content_disposition` | `Option<String>` | wire | | omitido se `None` |
| `GoldenResponse.body` | `Value` | wire | corpo (com voláteis mascarados) → fixture | |
| `GoldenStep.{name,request,response}` | struct | wire | passo montado → `check` | |
| `step(...)` | `GoldenStep` | intermediário | aplica voláteis ao body (panic se ausente/tipo/vazio) | volátil NÃO é opcional |
| `check(flow, steps)` | fn | entrada/saída | `captured{flow,steps}` × fixture | panic no 1º ponto de divergência |
| `captured` | `Value` | intermediário | `{flow, steps}` | serializado |
| `BTV_UPDATE_GOLDEN` | env var | config | presença → regrava fixture | grava pretty JSON em disco |
| `CI` | env var | config | presença junto de UPDATE → panic | proíbe regravar no gate |
| `fixture_path(flow)` | `PathBuf` | intermediário | `CARGO_MANIFEST_DIR/../../schemas/fixtures/http/{flow}.golden.json` | |
| `raw`/`expected` | `String`/`Value` | intermediário | lê fixture do disco → compara | panic se ausente/corrompida |
| `apply_volatile`/`walk`/`replace_leaf`/`check_and_replace` | fns | intermediário | navega pointer + curinga, valida e substitui | erro detalhado por caminho |
| `first_diff(a,b,at)` | `Option<String>` | intermediário | 1º caminho divergente (só p/ msg de erro) | detecta novo/removido/valor/tamanho |

Fluxo: o chamador monta `GoldenStep`s a partir da resposta REAL, `step()` mascara campos voláteis declarados (exigindo presença/tipo/não-vazio), e `check()` serializa `{flow,steps}` e compara por igualdade profunda com `schemas/fixtures/http/<flow>.golden.json` — ou regrava sob `BTV_UPDATE_GOLDEN` (proibido no CI). `first_diff` aponta a divergência.

---

## crates/btv-tui/src/lib.rs

Papel: modelo de estado + render puro da TUI (ratatui) — `TuiState`, `Item`, `PermissionPrompt`, `DiffKind`, `render()`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `DiffKind::{Context,Removed,Added}` | enum | estado | linha de diff → cor no render | decorado só com cor (não conhece semântica; `btv-cli` converte) |
| `Item::User(String)` | enum-variant | estado | transcript → linha `você ▸` (Cyan bold) | |
| `Item::Assistant(String)` | enum-variant | estado | transcript → linhas `btv ▸`/indent (Yellow) | multi-linha |
| `Item::Tool{name,detail,ok}` | struct-variant | estado | transcript → `⚒/✗ name: detail` (Green/Red) | |
| `Item::Diff(Vec<(DiffKind,String)>)` | struct-variant | estado | transcript → linhas com prefixo `  `/`- `/`+ ` | |
| `Item::Notice(String)` | enum-variant | estado | transcript → `· text` (DarkGray) | |
| `PermissionPrompt.tool` | `String` | estado | modal → `"permitir {tool} em {scope}?"` | |
| `PermissionPrompt.scope` | `String` | estado | modal | renderizado com `{:?}` |
| `TuiState.items` | `Vec<Item>` | estado | mutado pelo event loop (btv-cli) → render | transcript |
| `TuiState.streaming` | `String` | estado | turno corrente aberto → linhas `btv ▸` | esvaziado por `finish_turn` |
| `TuiState.input` | `String` | estado / entrada | texto do usuário → linha de entrada | |
| `TuiState.status` | `String` | estado | barra de status | prefixo `⋯` se busy |
| `TuiState.permission` | `Option<PermissionPrompt>` | estado | modal s/n por cima | |
| `TuiState.busy` | `bool` | estado | render do status | |
| `finish_turn()` | fn | intermediário | `streaming` → push `Item::Assistant` | `mem::take` esvazia o streaming |
| `render(frame,state)` | fn | saída | `TuiState` → widgets ratatui | consome items/streaming/input/status/permission/busy |
| `lines` | `Vec<Line>` | intermediário | items+streaming → parágrafo | |
| `visible`/`skip` | `usize` | intermediário | altura da área → mostra só o final | scroll implícito (últimas linhas) |
| layout | `[transcript, status, input]` | intermediário | `Layout::vertical` Min(3)/Length(1)/Length(3) | |
| `centered(area,w,h)` | `Rect` | intermediário | centraliza modal | clamp a área |

Fluxo: `btv-cli` traduz `LoopEvent`s do agente em mutações de `TuiState` (items/streaming/input/status/permission/busy); `render` (puro, testável com `TestBackend`) desenha transcript (com scroll para o final), barra de status, linha de entrada e, quando pendente, o modal de permissão por cima.

---

## crates/btv-contract/src/lib.rs

Papel: suíte de contrato dual-adapter (Trilha B, ADR 0026) — testa o CONTRATO das traits `btv-domain::ports` que SQLite e Postgres passam iguais; fixtures `run_novo`/`entrega_nova`/`evento_gate`, `TenantContext` A/B, e as suítes por repositório.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|------|------|---------|------------------|----------------------------|
| `ctx_a()` | `TenantContext` | intermediário / entrada | `TenantContext::local(ActorId "contract:a")` | tenant LOCAL |
| `ctx_b()` | `TenantContext` | intermediário / entrada | `TenantContext::new(TenantId ...b2, ActorId "contract:b")` | 2º tenant para isolamento |
| `run_novo(ctx,seq,nome)` | `Run` | intermediário / wire | fixture → `repo.save` | `id=0` (atribuído no save), `task_id=TaskId::new(seq)`, `template_id="editorial"`, `template_versao="v1.4"`, `briefing_json`/`papeis_json` fixos, `status=Ativa`, `gates_aprovados=0`, ts fixos, `tenant=ctx.tenant` |
| `entrega_nova(ctx,run_id,seq,nome)` | `Deliverable` | intermediário / wire | fixture → `save_with_deliverables` | `id=0`, `path=/tmp/{nome}`, `formato="MD"`, `versao="v1"`, `trilha="Redator · 1 gate(s)"`, ts fixo, `tenant=ctx.tenant` |
| `evento_gate(ctx,actor,ts)` | `DomainEvent` | intermediário / wire | fixture → `repo.append` | `actor` explícito (adapter NÃO reescreve actor/ts), `kind=GateApproved{task_id:1,stage,gates_approved:1}` |
| `make` (factory) | `impl FnMut()->R` | entrada | cada caso → adapter FRESCO | estado isolado por caso |
| `suite_run_repository` | fn genérica | saída | `RunRepository` → asserts | save/get round-trip, isolamento fail-closed, task_id coexiste por tenant, upsert por (tenant,task_id), list mais-recente-primeiro, `save_with_deliverables` transacional (rollback), fail-closed no run alheio, `max_task_seq` por tenant |
| `suite_persona_repository` | fn genérica | saída | `PersonaRepository` → asserts | overrides set/list/delete/clear + upsert, isolamento por tenant, personas próprias CRUD com ids não-vazantes |
| `suite_template_publication_repository` | fn genérica | saída | `TemplatePublicationRepository` → asserts | publicar/despublicar upsert, isolamento fail-closed |
| `suite_user_repository` | fn genérica | saída | `UserRepository` → asserts | CRUD + ciclo de PIN (`NoPin`/`Ok`/`Wrong`, hash nunca sai), `set_active`, isolamento fail-closed |
| `suite_ledger_repository` | fn genérica | saída | `LedgerRepository` → asserts | cadeias independentes por tenant (seq 1..N), appends INTERCALADOS, `verify_chain`/`export`/`recent` isolados, filtro de actor + limite, fail-closed na escrita |
| `suite_ledger_determinismo_cross_adapter` | fn genérica | saída | 2 adapters (A,B) + `projeta` → asserts | mesmos appends → MESMA `(seq,prev_hash,entry_hash)`; paridade CRIPTOGRÁFICA local↔SaaS; `Entry` unificado (não compila se divergir) |
| `TenantContext.tenant` / `.actor` | campos | estado / wire | escopo de todas as operações | isolamento por tenant |
| `RunStatus::{Ativa,Concluida}` | enum | wire | transições do agregado | `approve_gate`/`transition_to` |
| `PinCheck::{NoPin,Ok,Wrong}` | enum | saída | veredito de `verify_pin` | hash nunca sai da porta |

Fluxo: fixtures determinísticas (`run_novo`/`entrega_nova`/`evento_gate`, ts fixos) + `TenantContext` A/B alimentam suítes genéricas que recebem uma factory de adapter FRESCO por caso; cada suíte prova round-trip, isolamento fail-closed entre tenants, transacionalidade e (no ledger) determinismo cross-adapter das triplas de hash — garantindo paridade SQLite↔Postgres (local↔SaaS).

---
