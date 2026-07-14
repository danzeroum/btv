# Dicionário de Dados — `btv-store`, `btv-proto`, `btv-sidecar`

Mapa exaustivo de fluxo de dados (entrada / saída / intermediário / estado /
config / wire) dos três crates Rust de armazenamento, contrato gRPC e
sidecar. Taxonomia de Direção:

- **entrada** — parâmetro de função / leitura de coluna / RPC recebido.
- **saída** — retorno / escrita em coluna / evento emitido / RPC respondido.
- **intermediário** — variável local / buffer, mesmo quando descartado.
- **estado** — campo de struct persistido em memória.
- **config** — const / env var / valor de build.
- **wire** — coluna de DB / campo de mensagem proto / chave JSON serializada.

Arquivos de DB tocados pelo crate `btv-store` (modo local, prefixo `.btv/`):
`.btv/btv.db` (ledger + produto BuildToValue), `.btv/telemetry.db`
(telemetria), `.btv/events.db` (event store de sessões), além de
`prompt_cache`/`prompt_library`/`permission_rules` (arquivos SQLite próprios
conforme o caminho passado no `open`). Modo SaaS (feature `pg`): Postgres.

---

## crates/btv-store/src/lib.rs

Papel: raiz do crate de storage — declara os módulos e re-exporta os tipos públicos.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `btv`/`events`/`ledger`/`prompt_cache`/`prompt_library`/`rule_store`/`telemetry` | módulos | config | lib.rs → consumidores | módulos públicos sempre compilados |
| `pg` | módulo | config | `#[cfg(feature = "pg")]` | adapter Postgres SÓ sob a feature `pg`; build default (local) não compila nem exige Postgres |
| re-exports (`BtvStore`, `LedgerStore`, `EventStore`, `Telemetry`, `PromptCache`, `PromptLibrary`, `RuleStore`, …) | tipos | saída | módulos → API pública do crate | superfície estável do crate |

Fluxo: declara os 7+1 módulos de storage e reexporta seus tipos públicos; `pg` é gate por feature.

---

## crates/btv-store/src/ledger.rs

Papel: ledger append-only com hash-chain POR TENANT (ADR 0027) — INSERT-only, `verify_chain` detecta adulteração e transplante entre cadeias.

Tabela SQLite `ledger` (colunas — todas `wire`):

| Coluna | Tipo SQL | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | wire | banco → — | identidade física / ordem de inserção global (admin); NÃO é a verdade auditável |
| `tenant_id` | TEXT NOT NULL DEFAULT `00000000-…-001` | wire | `chain_tenant` → INSERT | UUID do tenant; DEFAULT = `LOCAL_TENANT` |
| `seq` | INTEGER NOT NULL | wire | `prev_seq + 1` → INSERT | monotônico POR tenant; `UNIQUE (tenant_id, seq)` |
| `prev_hash` | TEXT NOT NULL | wire | topo da cadeia → INSERT | hash da entrada anterior DENTRO do tenant (`""` na primeira) |
| `entry_hash` | TEXT NOT NULL | wire | `entry.chain_hash(prev_hash)` → INSERT | sha256 encadeado |
| `body` | TEXT NOT NULL | wire | `serde_json::to_string(&entry)` → INSERT | JSON canônico do `LedgerEntry` serializado com `seq: 0` (o seq real mora na coluna) |

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LOCAL_TENANT` | `const &str` | config | ledger.rs | `"00000000-0000-0000-0000-000000000001"` — tenant do modo local; DEFAULT das colunas |
| `LedgerError` | enum | saída | funções → chamador | variantes `Storage`, `Serde`, `BrokenChain{seq,expected,found}`, `ForeignEntry{seq,body_tenant,chain_tenant}` |
| `LedgerStore.conn` | `Connection` | estado | — | conexão rusqlite única |
| `open(path)` `path` | `&str` | entrada | chamador → `Connection::open` | liga `journal_mode=WAL` + `busy_timeout=10s` (corrida CLI×dashboard, ADR/Onda 6) |
| `init` DDL | SQL | config | `CREATE TABLE IF NOT EXISTS ledger` | schema com `UNIQUE (tenant_id, seq)` |
| `migrate_legacy` `tem_tenant` | `bool` | intermediário | `pragma_table_info` | pré-tenant → REBUILD `ledger_b3`: `INSERT … SELECT seq, LOCAL, seq, prev_hash, entry_hash, body` (sem re-hash) |
| `append` `entry` | `LedgerEntry` (mut) | entrada→saída | chamador → INSERT / retorno | preenche `prev_hash`/`entry_hash`/`seq` |
| `chain_tenant` | `String` | intermediário | `entry.tenant` ?? `LOCAL_TENANT` | porta legada sem tenant cai no LOCAL |
| `tx` (TransactionBehavior::Immediate) | `Transaction` | intermediário | conn | pega lock de escrita ANTES do SELECT do topo — read-modify-write atômico entre conexões |
| `topo` (`SELECT seq, entry_hash … ORDER BY seq DESC LIMIT 1`) | `Option<(u64,String)>` | intermediário | ledger → `(prev_seq, prev_hash)` | topo da cadeia do tenant; `unwrap_or((0, ""))` |
| `body` | `String` | intermediário/wire | serde → coluna `body` | corpo canônico |
| `LedgerEntry` (campos wire, de btv-schemas) | struct | wire | body JSON | `seq`, `prev_hash`, `entry_hash`, `kind`, `actor`, `payload`, `override`, `fake_marker`, `ts`, `tenant` (skip_serializing) |
| `recent(limit, actor)` | `u32`, `Option<&str>` | entrada | → `recent_in_chain(LOCAL_TENANT,…)` | porta legada fixa na cadeia LOCAL |
| `recent_in_chain` SQL | query | intermediário | `WHERE tenant_id=?3 AND (?2 IS NULL OR json_extract(body,'$.actor')=?2) ORDER BY seq DESC LIMIT ?1` | filtro de actor por `json_extract` NA MESMA query do LIMIT |
| `entry.seq` (pós-leitura) | `u64` | saída | coluna `seq` → entry | coluna sempre manda no seq (body tem `seq:0`) |
| `verify_chain()` `tenants` | `Vec<String>` | intermediário | `SELECT DISTINCT tenant_id` | loop por tenant; retorna total de entradas |
| `verify_tenant_chain` rows | `(seq,prev_hash,entry_hash,body)` | intermediário | ledger → `verifica_cadeia_rows` | por cadeia, ordem `seq` ascendente |
| `export_chain(tenant)` | `Vec<LedgerEntry>` | saída | ledger → export portátil (ADR 0027 item 4) | cadeia completa do tenant, seq 1→topo |
| `payload_wire(kind)` | `serde_json::Value` | saída/wire | `DomainEventKind` → JSON pt | chaves em pt (`task_id`,`template_versao`,`nome`,`papeis`,`personas_proprias`,`prompt_hashes`(assimetria `custom`),`refs`,`etapa`,`gates_aprovados`,`instrucao`,`gate_liberado`,`deliverable_id`,`formato`,`trilha`,`papel`,`prompt_sha256`,`publicado`,`blocos`,`diagram_sha256`,`versao_semantica`,`snapshot_hash`,`audit_head`,`audit_len`,`id`) |
| `entry_de_dominio(ctx, event)` | fn | saída | `DomainEvent` → `LedgerEntry` | fail-closed: `event.tenant != ctx.tenant` ⇒ `RepositoryError::Storage`; grava `tenant` NO CORPO hasheado (anti-transplante ADR 0027 item 2) |
| `verifica_cadeia_rows(tenant, rows)` `expected_prev` | `String` | intermediário | recomputa `entry.chain_hash(expected_prev)` | compara `prev_hash`/`entry_hash`; detecta `ForeignEntry` (body de outro tenant) e `BrokenChain` |
| `LedgerRepository::{append,recent,verify_chain,export}` | trait impl | entrada/saída | `&TenantContext` + evento | porta tipada de domínio; erros viram `RepositoryError` |
| `kinds_fora_do_vocabulario()` | `Vec<VocabViolation>` | saída | `json_extract(body,'$.kind')` → `LedgerKind::parse` | doctor: typo de kind vira diagnóstico com linha (`certification` é exclusão consciente) |

Fluxo: `LedgerEntry` (entrada) → `hash_body`/`chain_hash(prev)` computa `entry_hash` sob `BEGIN IMMEDIATE` → `serde_json` gera `body` canônico (`seq:0`) → INSERT `(tenant_id, prev_seq+1, prev_hash, entry_hash, body)`; leitura reidrata o `seq` da coluna; `verify_chain`/`verifica_cadeia_rows` recomputa a cadeia por tenant e checa anti-transplante.

---

## crates/btv-store/src/btv.rs

Papel: persistência do produto BuildToValue em `.btv/btv.db` — runs, deliverables, personas, publicação de templates, perfis locais (PIN); é também o adapter SQLite das traits de `btv-domain::ports`.

Tabela `runs` (colunas — `wire`):

| Coluna | Tipo | Observação |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | numeração física |
| `task_id` | TEXT NOT NULL | `sq{hex}`; `UNIQUE (tenant_id, task_id)` |
| `template_id` | TEXT NOT NULL | arquétipo |
| `template_versao` | TEXT NOT NULL | versão do template |
| `nome` | TEXT NOT NULL | rótulo da ativação |
| `briefing_json` | TEXT NOT NULL | briefing serializado |
| `papeis_json` | TEXT NOT NULL | papéis serializados |
| `status` | TEXT NOT NULL | `ativa`/`concluida`/`erro`/`encerrada` (`RunStatus`) |
| `gates_aprovados` | INTEGER NOT NULL DEFAULT 0 | contador de gates HITL |
| `created_ts` / `updated_ts` | TEXT NOT NULL | RFC3339 |
| `tenant_id` | TEXT NOT NULL DEFAULT LOCAL | `UNIQUE (tenant_id, task_id)` |

Tabela `deliverables`: `id`, `run_id`, `task_id`, `template_id`, `nome`, `path`, `formato`, `versao`, `trilha`, `created_ts`, `tenant_id` (todas `wire`).
Tabela `persona_overrides`: `template_id`, `papel`, `prompt`, `updated_ts`, `tenant_id` — PK `(tenant_id, template_id, papel)`.
Tabela `template_pub`: `template_id`, `publicado` (INTEGER 0/1), `updated_ts`, `tenant_id` — PK `(tenant_id, template_id)`.
Tabela `users`: `id`, `nome`, `email`, `papel`, `ativo` (INTEGER DEFAULT 1), `created_ts`, `pin_hash` (TEXT nullable), `tenant_id`.
Tabela `custom_personas`: `id`, `template_id`, `nome`, `prompt`, `updated_ts`, `tenant_id`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LOCAL_TENANT` | `const &'static str` | config | btv.rs | `00000000-…-001`; DEFAULT das colunas `tenant_id` |
| `BtvStore.conn` | `Connection` | estado | — | WAL ligado no `open` (corrida CLI×dashboard) |
| `BtvStoreError` | enum | saída | | `Storage`, `NotFound` |
| `migrate_legacy` | fn | intermediário | pré-tenant → REBUILD | `ADD COLUMN tenant_id DEFAULT LOCAL` em deliverables/users/custom_personas; REBUILD de runs/persona_overrides/template_pub (constraint muda) |
| `ALTER TABLE users ADD COLUMN pin_hash` | migração defensiva | config | init | erro de coluna duplicada ignorado |
| `insert_run(task_id, template_id, template_versao, nome, briefing_json, papeis_json, now)` | params | entrada→saída | → INSERT `status='ativa'` | retorna `last_insert_rowid()` |
| `max_run_task_seq()` | `u64` | saída | `SELECT task_id … strip_prefix("sq") → u64 from_str_radix(_,16)` | maior seq `sq{hex}` (semeia o contador por-processo no arranque; evita colisão UNIQUE após redeploy) |
| `reconcile_stale_runs(now)` | `usize` | saída | `UPDATE … status='encerrada' WHERE status='ativa'` | runs zumbis (processo morto) marcadas encerradas no arranque |
| `set_status(task_id, status: RunStatus, now)` | params | entrada | UPDATE | grava `status.as_str()`; silencioso p/ task_id desconhecido |
| `list_runs()` | `Vec<BtvRun>` | saída | `SELECT RUN_COLS … WHERE tenant_id=LOCAL ORDER BY id DESC` | mais recente primeiro |
| `increment_gates(task_id, now)` | — | entrada | `UPDATE … gates_aprovados+1` | compõe procedência U4 |
| `get_run_by_task(task_id)` | `Option<BtvRun>` | saída | list_runs → find | |
| `insert_deliverable(run_id, task_id, template_id, nome, path, formato, versao, trilha, now)` | params | entrada→saída | INSERT | retorna rowid |
| `list_deliverables()` / `get_deliverable(id)` | `Vec/Option<BtvDeliverable>` | saída | `SELECT DELIVERABLE_COLS` | |
| `set_persona_override(template_id, papel, prompt, now)` | params | entrada | INSERT … ON CONFLICT(tenant_id,template_id,papel) DO UPDATE | upsert |
| `delete_persona_override` / `clear_persona_overrides` | — | entrada | DELETE | escopo tenant LOCAL |
| `list_persona_overrides(template_id)` | `Vec<PersonaOverride>` | saída | SELECT | |
| `insert/update/delete/list_custom_persona(s)` | params/`Vec<CustomPersona>` | entrada/saída | INSERT/UPDATE/DELETE/SELECT | |
| `set_template_publicado(template_id, publicado, now)` | params | entrada | INSERT … ON CONFLICT DO UPDATE | grava `publicado as i64` |
| `list_template_pub()` | `Vec<(String,bool)>` | saída | SELECT | `publicado != 0` |
| `insert_user(nome, email, papel, pin, now)` | params | entrada→saída | INSERT | `pin` presente → `pin_hash(now,email,nome,pin)`; ausente = perfil aberto |
| `pin_hash` (arg intermediário) | `Option<String>` | intermediário | `pin.filter(!empty).map(pin_hash)` | nunca grava PIN em claro |
| `set_user_ativo(id, ativo)` | — | entrada | UPDATE | `NotFound` se 0 linhas |
| `delete_user(id)` | — | entrada | DELETE | `NotFound` se 0 linhas |
| `set_user_pin(id, pin)` | — | entrada | recomputa salt de `created_ts,email,nome` persistidos | limpa se `None`/vazio |
| `verify_user_pin(id, pin)` | `PinCheck` | saída | `pin_hash(...)==stored` | `NoPin`/`Ok`/`Wrong`; `NotFound` se id inexistente |
| `list_users()` | `Vec<BtvUser>` | saída | SELECT | `has_pin = pin_hash.is_some()` — NUNCA vaza o hash |
| `RUN_COLS` / `DELIVERABLE_COLS` | `const &str` | config | ordem do SELECT ↔ `row_to_*` | |
| `parse_tenant_col(row, idx)` | `TenantId` | intermediário | coluna → `TenantId::parse` | fail-closed: formato inválido é ERRO (`FromSqlConversionFailure`), não tenant fabricado |
| `row_to_run` / `row_to_deliverable` | fn | intermediário | linha → struct | `TaskId::parse`/`RunStatus::parse` fail-closed |
| `storage(e)` | fn | intermediário | `rusqlite::Error` → `RepositoryError::Storage` | tradução na fronteira das traits |
| `exige_mesmo_tenant(ctx, dono, o_que)` | fn | intermediário | guard | fail-closed na escrita: `dono != ctx.tenant` ⇒ recusa (rollback do lote) |
| `upsert_run(conn, run)` | fn | entrada | UPDATE-then-INSERT por `(tenant_id, task_id)` | `id` ignorado; identidade = task_id no tenant |
| `RunRepository::{get,list,save,save_with_deliverables,list_deliverables,get_deliverable,max_task_seq}` | trait impl | entrada/saída | `&TenantContext` | `save_with_deliverables` é transação real (run+entregas juntos ou rollback) |
| `PersonaRepository::*` | trait impl | entrada/saída | `&TenantContext` | `updated_ts` = `strftime` do banco (port não carrega relógio) |
| `TemplatePublicationRepository::{set_published,list_published}` | trait impl | entrada/saída | | |
| `UserRepository::{list,create,remove,set_active,set_pin,verify_pin}` | trait impl | entrada/saída | `create` lê `now` do banco (salt + coluna com MESMO valor) | |
| `pin_hash(created_ts, email, nome, pin)` | `String` | saída | `btv_schemas::sha256_hex("{created_ts}|{email}|{nome}|{pin}")` | salt = created_ts+email+nome (recomputável, sem coluna própria); sha256 simples honesto, NÃO KDF |

Fluxo: parâmetros de insert/list viram linhas nas 6 tabelas escopadas por `tenant_id` (DEFAULT LOCAL na API legada, `ctx.tenant` explícito nas traits); `pin_hash` deriva `sha256(created_ts|email|nome|pin)` e só o hash é persistido; leituras parseiam `TaskId/RunStatus/TenantId` fail-closed.

---

## crates/btv-store/src/events.rs

Papel: event store append-only com concorrência otimista (porte do opencode, ADR 0002) — base das sessões duráveis (`.btv/events.db`).

Tabela `event_sequence`: `aggregate_id` TEXT PK, `seq` INTEGER NOT NULL, `owner_id` TEXT (todas `wire`).
Tabela `event`: `id` TEXT PK, `aggregate_id` TEXT NOT NULL FK→event_sequence ON DELETE CASCADE, `seq` INTEGER NOT NULL, `type` TEXT NOT NULL, `data` TEXT NOT NULL. Índices: UNIQUE `(aggregate_id, seq)`, `(aggregate_id, type, seq)`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SCHEMA_DDL` | `const &str` | config | init | mesmo DDL do opencode; `IF NOT EXISTS` idempotente |
| `EventError` | enum | saída | | `Conflict{aggregate_id,expected,found}`, `Storage`, `Serde` |
| `EventStore.conn` | `Connection` | estado | | `open`: WAL + synchronous NORMAL + foreign_keys ON + busy_timeout 5s |
| `EventInput` / `StoredEvent` | structs (de btv-domain::event) | wire | re-export | `EventInput{kind,data}`, `StoredEvent{id,aggregate_id,seq,kind,data}` |
| `append(aggregate_id, expected_head, events)` | params | entrada→saída | tx | retorna nova head |
| `found` (`SELECT seq FROM event_sequence`) | `i64` | intermediário | | `unwrap_or(0)`; se `found != expected_head` ⇒ `Conflict` |
| `new_head` | `i64` | intermediário/wire | `found + events.len()` → `event_sequence.seq` | upsert `ON CONFLICT DO UPDATE seq=excluded.seq` |
| `event_id(aggregate_id, seq)` | `String` | intermediário/wire | `evt_{nanos:024x}_{aggregate_id}_{seq}` → coluna `id` | prefixo temporal preserva ordem lexicográfica |
| `event.data` | `Value` | wire | `serde_json::to_string` → coluna `data` | |
| `read(aggregate_id, from_seq)` | `Vec<StoredEvent>` | saída | `WHERE aggregate_id=?1 AND seq>?2 ORDER BY seq` | leitura incremental; `data` reidratado por `from_str` |
| `head_seq(aggregate_id)` | `i64` | saída | SELECT seq | 0 se ausente |
| `aggregates()` | `Vec<String>` | saída | `SELECT aggregate_id … ORDER BY rowid DESC` | mais recentes primeiro |
| `EventStorePort::{append,read,head_seq}` | trait impl | entrada/saída | `&TenantContext` | adapter do modo LOCAL |
| `exige_local(ctx)` | fn | intermediário | guard | fail-closed: `ctx.tenant != TenantId::LOCAL` ⇒ recusa (schema não isola tenant) |
| `porta(e)` | fn | intermediário | `Conflict → ConcurrencyConflict{expected,found}`; resto `Storage` | mapeamento de erro do domínio |

Fluxo: `append` lê a head atual, compara com `expected_head` (divergência ⇒ `Conflict`/`ConcurrencyConflict`), upserta `event_sequence.seq = found + len`, e insere cada `event` com `id` temporal e `data` JSON; o índice UNIQUE `(aggregate_id, seq)` é a base da concorrência otimista.

---

## crates/btv-store/src/telemetry.rs

Papel: telemetria offline-first (`.btv/telemetry.db`) — grava eventos localmente e agrega sob demanda (summary / model_usage / experiment_variants). Nada sai da máquina.

Tabela `telemetry_event`: `id` INTEGER PK AUTOINCREMENT, `name` TEXT NOT NULL, `session_id` TEXT NOT NULL, `props` TEXT NOT NULL (JSON), `ts` TEXT NOT NULL (todas `wire`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `TelemetryRecord{name, session_id, props: Value, ts}` | struct | wire/saída | linha → registro | `props` é JSON livre |
| `TelemetrySummary{total_events, by_name: HashMap, cache_hit_rate: Option<f64>}` | struct | saída | agregação | |
| `ModelUsage{model, calls, cache_hits, cache_misses, input_tokens, output_tokens}` | struct | saída | agregação | `model` de `props.model`; tokens de `props.input_tokens`/`output_tokens` |
| `record(name, session_id, props, ts)` | params | entrada | INSERT | grava `props.to_string()` |
| `recent(limit)` | `Vec<TelemetryRecord>` | saída | `ORDER BY id DESC LIMIT ?1` | `props` reidratado; falha de parse ⇒ `Value::Null` |
| `summary()` `by_name` | `HashMap<String,u64>` | intermediário/saída | `GROUP BY name` | |
| `cache_hit_rate` | `Option<f64>` | saída | `hits/(hits+misses)` | `cache.hit`/`cache.miss`; `None` sem chamadas |
| `experiment_variants(experiment)` | `Vec<(String,u64,u64)>` | saída | JSON1 `json_extract(props,'$.variant'/'$.experiment'/'$.success')` | `(variante, n, sucessos)`; `success = 1` conta; `WHERE experiment` + variant NOT NULL, GROUP BY variant |
| `model_usage()` | `Vec<ModelUsage>` | saída | `CASE WHEN name='llm.call'/'cache.hit'/'cache.miss'` + `SUM(json_extract input/output_tokens)` | agrupado por `props.model`; tokens negativos clampados a 0 |
| `Telemetry(Arc<Mutex<TelemetryStore>>)` | struct | estado | handle cloneável thread-safe | compartilhado por cache/rate-limit/dashboard |
| `Telemetry::record` | — | entrada | lock → record | falha NUNCA quebra caminho principal (log em stderr, descartada) |
| `Telemetry::{recent,summary,experiment_variants,model_usage}` | saída | | | `unwrap_or_default()` em falha (vazio) |

Fluxo: `record` serializa `props` (JSON) e insere `(name, session_id, props, ts)`; agregações rodam sob demanda — `summary` conta por nome e deriva cache-hit-rate, `model_usage`/`experiment_variants` usam `json_extract` (JSON1) para agrupar; o handle `Telemetry` engole erros para blindar o caminho principal.

---

## crates/btv-store/src/prompt_cache.rs

Papel: cache de respostas de LLM por hash de request (`prompt-cache-key.v1`) — requests idênticos não voltam à rede.

Tabela `prompt_cache`: `hash` TEXT PRIMARY KEY, `response` TEXT NOT NULL, `created_at` TEXT NOT NULL (todas `wire`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `PromptCache.conn` | `Connection` | estado | | |
| `get(hash)` | `Option<String>` | saída | `SELECT response WHERE hash=?1` | `QueryReturnedNoRows → None` |
| `put(hash, response, created_at)` | params | entrada | `INSERT OR REPLACE` | escrita idempotente |

Fluxo: `hash` (sha256 do JSON canônico do request, `btv_schemas::request_hash`) → chave; `response` (turno serializado) → valor; put é INSERT OR REPLACE.

---

## crates/btv-store/src/prompt_library.rs

Papel: biblioteca de prompts salvos (origem: prompte) — salvar, favoritar, tags, reusar prompts renderizados.

Tabela `prompt_library`: `id` INTEGER PK AUTOINCREMENT, `name`, `generator`, `fields` (TEXT JSON), `rendered`, `tags` (TEXT JSON array), `favorite` (INTEGER DEFAULT 0), `created_at` (todas `wire`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SavedPrompt{id, name, generator, fields: Value, rendered, tags: Vec<String>, favorite, created_at}` | struct | wire/saída | linha → registro | |
| `save(name, generator, fields, rendered, tags, created_at)` | params | entrada→saída | INSERT `favorite=0` | `fields.to_string()`, `tags` via `to_string` (fallback `"[]"`) |
| `list(tag)` | `Vec<SavedPrompt>` | saída | `ORDER BY id DESC` + filtro em Rust por tag | `is_none_or` inclui todos se tag ausente |
| `get(id)` | `Option<SavedPrompt>` | saída | SELECT | |
| `toggle_favorite(id)` | `Option<bool>` | saída | UPDATE `favorite=!current` | `None` se id inexistente |
| `delete(id)` | `bool` | saída | DELETE | `true` se removeu |
| `row_to_prompt` | fn | intermediário | linha → struct | `fields`/`tags` reidratados (fallback `Null`/`default`) |

Fluxo: `save` serializa `fields`(JSON)/`tags`(JSON array) e insere; leituras reidratam os campos JSON; `favorite` é 0/1 alternável.

---

## crates/btv-store/src/rule_store.rs

Papel: persistência das regras de permissão editadas pelo usuário (matriz build/plan×tool + overrides "sempre") — só armazenamento; a avaliação fica em `btv_core::PermissionEngine`.

Tabela `permission_rules`: `id` INTEGER PK AUTOINCREMENT, `profile` TEXT NOT NULL, `tool` TEXT NOT NULL, `scope_prefix` TEXT (nullable), `decision` TEXT NOT NULL, `created_at` TEXT NOT NULL (todas `wire`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RuleStoreError::Storage` | enum | saída | | |
| `RuleDecision` (Allow/Ask/Deny) | enum | wire | serde `snake_case` | `as_str`/`FromStr` ↔ `"allow"`/`"ask"`/`"deny"` |
| `RuleRecord{id, profile, tool, scope_prefix: Option, decision, created_at}` | struct | wire/saída | linha → registro | `scope_prefix` `skip_serializing_if None` |
| `RuleStore.conn` | `Connection` | estado | | WAL desde criação (leitura em avaliação × escrita da matriz) |
| `set(profile, tool, scope_prefix, decision, created_at)` | params → `RuleRecord` | entrada→saída | DELETE-then-INSERT na tx | substitui MESMA chave (`profile+tool+scope_prefix IS ?`), não acumula duplicatas |
| `get(id)` | `Option<RuleRecord>` | saída | SELECT | usado para logar auditoria antes de remover |
| `remove(id)` | `bool` | saída | DELETE | idempotente |
| `list_all()` | `Vec<RuleRecord>` | saída | `ORDER BY id DESC` | alimenta lista "rules ativas" |
| `list_for_profile(profile)` | `Vec<RuleRecord>` | saída | `ORDER BY (scope_prefix IS NULL) ASC, id DESC` | escopo específico vence default; alimenta `PermissionEngine::overlay` |
| `row_to_record` | fn | intermediário | linha → struct | decisão inválida ⇒ fallback `Ask` (fail-safe) |

Fluxo: `set` remove a chave `(profile, tool, scope_prefix)` existente e reinsere `decision` (`as_str`) numa transação; leituras ordenam escopo-específico primeiro para o overlay de permissão; decisão desconhecida na leitura degrada para `Ask`.

---

## crates/btv-store/src/pg.rs (feature `pg`)

Papel: adapter Postgres do modo SaaS (ADR 0026/0027/0028) — MESMAS traits do SQLite, RLS por `app.tenant_id` + `WHERE tenant_id` explícito (defesa em profundidade), append de ledger com retry otimista, e sessões SaaS (ADR 0029).

Migrations embutidas (`migrations_pg/`, todas colunas `wire`, RLS FORCE):
`runs`/`deliverables`/`persona_overrides`/`custom_personas`/`template_pub`/`users` — mesmo shape do SQLite, `tenant_id` TEXT sem default; `ledger` = `(tenant_id, seq)` + `body` TEXT (nunca JSONB). Cada uma: `ENABLE ROW LEVEL SECURITY` + `POLICY tenant_isolation … USING/WITH CHECK (tenant_id = current_setting('app.tenant_id', true))`.
Tabela `sessions` (SEM RLS de tenant — exceção declarada): `token_hash` TEXT PK, `tenant_id` TEXT NOT NULL, `user_id` TEXT, `created_ts` TIMESTAMPTZ DEFAULT now(), `absolute_deadline` TIMESTAMPTZ (created+30d), `idle_deadline` TIMESTAMPTZ (LEAST(now()+24h, absolute)), `revoked_at` TIMESTAMPTZ nullable; INDEX `sessions_por_tenant (tenant_id)`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `MAX_TENTATIVAS_APPEND` | `const usize = 64` | config | append otimista | teto de retries sob contenção |
| `NOW_UTC_SQL` | `const &str` | config | `to_char(now() at tz 'utc', 'YYYY-…"Z"')` | `updated_ts` de escrituração RFC3339 |
| `BTV_PG_MAX_CONNECTIONS` | env var | config | `pool_max_connections()` default 4 | tamanho do pool; inválido/ausente → 4 |
| `BTV_PG_TEST_URL` | env var | config | harness de teste | ausente ⇒ teste PULA (não passa fingindo) |
| `PgStore{rt: tokio Runtime (current-thread), pool: PgPool}` | struct | estado | | traits síncronas × sqlx async ⇒ `block_on` por op |
| `storage(e)` | fn | intermediário | `Display → RepositoryError::Storage` | |
| `fixa_tenant(tx, tenant)` | fn | wire | `SELECT set_config('app.tenant_id', $1, true)` | local à transação (evapora no COMMIT/ROLLBACK); RLS = 2ª linha de defesa |
| `eh_conflito_unique(e)` | `bool` | intermediário | SQLSTATE `23505` | corrida do `UNIQUE (tenant_id, seq)` |
| `linha_para_run` / `linha_para_deliverable` | fn | intermediário | `PgRow → Run/Deliverable` | `TaskId`/`RunStatus`/`TenantId::parse` fail-closed |
| `connect(url)` / `connect_with(opts)` | entrada | | `PgConnectOptions` → pool + `sqlx::migrate!("./migrations_pg")` | migrations idempotentes (`_sqlx_migrations`) |
| `RunRepository::{get,list,save,save_with_deliverables,list_deliverables,get_deliverable,max_task_seq}` | trait impl | entrada/saída | tx com `fixa_tenant` + `WHERE tenant_id=$1` | `save_with_deliverables` = transação real (rollback fail-closed no meio do lote) |
| `upsert_run(tx, run)` | fn | entrada | `INSERT … ON CONFLICT (tenant_id, task_id) DO UPDATE` | `id` sobrevive; identidade = task_id no tenant |
| `PersonaRepository::*` | trait impl | entrada/saída | `ON CONFLICT … DO UPDATE`, `NOW_UTC_SQL`; `RETURNING id` no insert | `update/delete_custom` → `NotFound` se 0 linhas |
| `TemplatePublicationRepository::{set_published,list_published}` | trait impl | entrada/saída | `publicado` BOOL | |
| `UserRepository::{list,create,remove,set_active,set_pin,verify_pin}` | trait impl | entrada/saída | `create` lê `NOW_UTC_SQL` (salt+coluna), `pin_hash` reusa `crate::btv::pin_hash` | `has_pin = pin_hash.is_some()` |
| `LedgerRepository::append` (retry otimista) | trait impl | entrada→saída | loop ≤64: relê topo `(seq, entry_hash)` → `chain_hash` → INSERT | perdedor do `23505` faz rollback e relê topo novo; sem lock de sessão (sobrevive a pooler transacional) |
| `body` (append) | `String` | intermediário/wire | `serde_json::to_string(&entry)` (`seq:0`) → coluna `body` | corpo canônico IDÊNTICO ao SQLite (paridade cross-adapter) |
| `LedgerRepository::recent` | trait impl | saída | `WHERE ($2::text IS NULL OR body::jsonb ->> 'actor' = $2) ORDER BY seq DESC LIMIT $3` | filtro de actor via `body::jsonb ->>` na MESMA query |
| `LedgerRepository::{verify_chain,export}` | trait impl | saída | rows → `verifica_cadeia_rows` (função COMPARTILHADA com SQLite) | mesma verificação hash+anti-transplante |
| `SessaoResolvida{tenant, user_id}` | struct | saída | auth → `TenantContext` (`actor=user:{user_id}`) | |
| `TokenEmitido{token, token_hash}` | struct | saída | emissão | token em claro existe SÓ aqui, uma vez |
| `SessaoAdmin{token_hash, user_id, absolute_deadline, idle_deadline}` | struct | saída | visão administrativa | NUNCA carrega o token |
| `gerar_token()` | fn | intermediário→saída | `getrandom` 32 bytes CSPRNG → `base64url NO_PAD` c/ prefixo `btvs_` → `sha256_hex` | token forte não precisa de KDF (razão invertida do `pin_hash`) |
| `issue_session(tenant, user_id)` | `TokenEmitido` | saída | INSERT `sessions` `absolute=now()+30d`, `idle=now()+24h` | grava SÓ o hash; ato de OPERADOR (sem `TenantContext`) |
| `resolve_session(token)` | `Option<SessaoResolvida>` | entrada→saída | `UPDATE sessions SET idle=LEAST(now()+24h, absolute) WHERE token_hash=sha256(token) AND revoked IS NULL AND now()<absolute AND now()<idle RETURNING tenant_id,user_id` | valida+renova numa query; SEM `fixa_tenant` (lookup anterior ao contexto); inválido ⇒ `None` fail-closed |
| `revoke_session(ctx, token_hash)` | `bool` | entrada→saída | `UPDATE … revoked_at=now() WHERE token_hash AND tenant_id AND revoked IS NULL` | tenant-escopado por WHERE (revogar hash de outro tenant = no-op) |
| `list_sessions(ctx)` | `Vec<SessaoAdmin>` | saída | `WHERE tenant_id AND revoked IS NULL ORDER BY created_ts DESC` | nunca devolve token |
| `harness` (`PgIsolado`, `abrir_isolado`, `disponivel`) | test-only | intermediário | schema isolado + role `btv_app_teste` NOSUPERUSER NOBYPASSRLS | RLS SE APLICA (superuser bypassaria); advisory lock `48484` serializa criação de role |

Fluxo: cada operação abre transação, fixa `app.tenant_id` via `set_config(..., true)` (RLS) e roda com `WHERE tenant_id` explícito; o append do ledger reencadeia via retry otimista sobre `23505` (relê topo `(seq, entry_hash)` → `chain_hash` → INSERT), com `body` canônico byte-idêntico ao SQLite; sessões emitem token CSPRNG (32B → base64url `btvs_` → sha256), guardam só `token_hash` + prazos, e `resolve_session` valida-e-renova numa única query fail-closed.

---

## crates/btv-proto/Cargo.toml

Papel: manifesto do crate de stubs gRPC.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| deps `tonic`, `prost`, `serde` | crates | config | | `serde` para serde direto nos tipos gerados (sem DTO espelho) |
| build-deps `tonic-build`, `protoc-bin-vendored` | crates | config | build.rs | protoc vendorizado |

Fluxo: declara as dependências de runtime (tonic/prost/serde) e de build (tonic-build + protoc vendorizado).

---

## crates/btv-proto/build.rs

Papel: gera os stubs tonic a partir de `schemas/proto/*.proto` no build.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `protoc_bin_vendored::protoc_bin_path()` | path | config | → env `PROTOC` | evita exigir protobuf de sistema |
| `proto_dir` | path | config | `CARGO_MANIFEST_DIR/../../schemas/proto` | include path |
| `protos` | `[&str; 5]` | entrada | `llm/core/squad/promptforge/memory.proto` | ordem: `llm` folha, `core` importa `llm` |
| `cargo:rerun-if-changed` | diretiva | config | por proto | rebuild quando `.proto` muda |
| `type_attribute(".btv.squad.v1", derive Serialize)` | config | wire | `SquadEvent` → SSE direto | Onda 4 |
| `type_attribute(".btv.promptforge.v1", derive Serialize)` | config | wire | `GeneratorInfo`/`GeneratorField` → JSON HTTP | Onda 5 |
| `type_attribute(".btv.memory.v1", derive Serialize)` | config | wire | `MemorySummary`/`MemoryMatch` → JSON HTTP | Onda 8 |
| `compile_protos(protos, [proto_dir])` | chamada | saída | → código gerado tonic (client+server) | |

Fluxo: seta `PROTOC` vendorizado, marca rerun-if-changed nos 5 protos, aplica `derive(Serialize)` a squad/promptforge/memory e compila client+server via `tonic_build`.

---

## crates/btv-proto/src/lib.rs

Papel: inclui os stubs gerados espelhando a hierarquia de pacotes protobuf e expõe aliases curtos.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `btv::{llm,core,squad,promptforge,memory}::v1` | módulos | wire | `tonic::include_proto!("btv.*.v1")` | aninhamento espelha `super::super::llm::v1::…` do prost |
| aliases `promptforge`/`llm`/`core`/`squad`/`memory` | módulos | saída | reexport `crate::btv::*::v1::*` | superfície estável (btv-sidecar depende) |

Fluxo: `include_proto!` injeta os tipos gerados sob `btv::<pkg>::v1`; aliases de topo dão caminhos curtos aos consumidores.

---

## schemas/proto/llm.proto (`btv.llm.v1`)

Papel: tipos do gateway LLM compartilhados por CoreService e SquadService.

| Mensagem.campo | Tipo proto | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LlmRequest.model` | string=1 | wire | Python → Core.Generate | modelo LLM |
| `LlmRequest.messages_json` | string=2 | wire | | JSON canônico (contrato prompt-cache-key.v1) |
| `LlmRequest.temperature` | optional double=3 | wire | | |
| `LlmRequest.max_tokens` | optional uint32=4 | wire | | |
| `LlmRequest.requester` | string=5 | wire | | identifica o agente p/ telemetria + rate limit |
| `LlmChunk` (oneof payload) | msg | wire | Core → Python (stream) | `text_delta` string=1 \| `usage` Usage=2 \| `error` string=3 |
| `Usage.input_tokens` | uint64=1 | wire | | |
| `Usage.output_tokens` | uint64=2 | wire | | |
| `Usage.cache_hit` | bool=3 | wire | | |
| `Usage.provider` | string=4 | wire | | |

Fluxo: `LlmRequest` (Python→Core) dispara geração; a resposta streama como `LlmChunk` (`text_delta`* → `usage` → ou `error`).

---

## schemas/proto/core.proto (`btv.core.v1`)

Papel: `CoreService` — RPCs que o núcleo Rust expõe ao sidecar Python (keys/permissões/ledger só no core). Importa `llm.proto`.

| Item | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Generate(LlmRequest) → stream LlmChunk` | rpc | wire | Python → Rust | geração via gateway |
| `RunTool(ToolCall) → ToolResult` | rpc | wire | Python → Rust | execução sujeita a permissões |
| `AppendLedger(LedgerAppend) → LedgerAck` | rpc | wire | Python → Rust | (Unimplemented no server atual) |
| `Recall(RecallRequest) → RecallResponse` | rpc | wire | Python → Rust | dormente (direção errada; ver memory.proto) |
| `Remember(RememberRequest) → RememberAck` | rpc | wire | Python → Rust | dormente |
| `RequestPermission(PermissionRequest) → PermissionDecision` | rpc | wire | Python → Rust | HITL, decisão vem da TUI |
| `ToolCall.tool` / `.args_json` / `.scope` | string 1/2/3 | wire | | `scope` é informativo — Rust SEMPRE recalcula `Tool::scope` de `args_json` (nunca fonte de verdade p/ Allow/Ask/Deny) |
| `ToolResult.content` / `.truncated` / `.exit_code` | string=1 / bool=2 / int32=3 | wire | Rust → Python | `exit_code`: 0=ok, 1=erro/args inválidos/tool desconhecida, -1=negado por permissão/humano |
| `LedgerAppend.kind` / `.actor` / `.payload_json` / `.fake_marker` | string 1/2/3 / optional string=4 | wire | | |
| `LedgerAck.seq` / `.entry_hash` | uint64=1 / string=2 | wire | | |
| `RecallRequest.agent` / `.query` / `.limit` | string 1/2 / uint32=3 | wire | | |
| `RecallResponse.memories_json` | repeated string=1 | wire | | |
| `RememberRequest.agent` / `.memory_json` | string 1/2 | wire | | |
| `RememberAck.stored` | bool=1 | wire | | |
| `PermissionRequest.tool` / `.scope` / `.reason` / `.confidence` | string 1/2/3 / double=4 | wire | | `confidence` gatilho HITL (< 0.3/0.5) |
| `PermissionDecision.decision` (enum UNSPECIFIED/ALLOW/DENY) / `.operator_note` | enum=1 / optional string=2 | wire | Rust → Python | |

Fluxo: o Python chama de volta o Rust por `Generate` (LLM streaming), `RunTool` (execução real — scope recalculado no Rust), `RequestPermission` (HITL); `AppendLedger`/`Recall`/`Remember` existem no contrato mas o server responde `Unimplemented`.

---

## schemas/proto/squad.proto (`btv.squad.v1`)

Papel: `SquadService` — serviço que o sidecar Python expõe ao Rust; `ExecuteTask` devolve stream de `SquadEvent`.

| Item | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ExecuteTask(SquadTask) → stream SquadEvent` | rpc | wire | Rust → Python | eventos ao vivo (propostas/votos/consenso/handoff/HITL/chat) |
| `Health(HealthRequest) → HealthResponse` | rpc | wire | | |
| `SquadTask.task_id` / `.description` / `.decision_type` | string 1/2/3 | wire | Rust → Python | |
| `SquadTask` tag 4 (`max_autonomy_level`) | `reserved` | wire | | REMOVIDO (ADR 0033) — era ignorado ponta-a-ponta (ADR 0021); tag/nome reservados p/ não reusar |
| `SquadTask.verification_evidence` | VerificationEvidence=5 | wire | | tipado (D3t); ausente ⇒ fail-closed no Python |
| `SquadTask.model` | string=6 | wire | | vazio = herda default do pool/`--model` |
| `SquadTask.roster` | repeated PersonaSpec=7 | wire | | personas reais (U7); vazio = elenco fixo |
| `SquadTask.tenant_id` / `.actor` | string 8/9 | wire | | multitenant propagado (nunca inventado); vazio = pré-D2t |
| `PersonaSpec.papel` / `.prompt` / `.funcao` / `.ordem` / `.custom` | string 1/2/3 / uint32=4 / bool=5 | wire | | `funcao`: plan/produce/review/validate/deliver; `custom` = persona própria |
| `SquadEvent.task_id` / `.ts` / `.tenant_id` / `.actor` | string 1/2/10/11 | wire | Python → Rust | tenant/actor ecoados VERBATIM do SquadTask |
| `SquadEvent.payload` (oneof) | | wire | | `proposal`=3 \| `consensus`=4 \| `handoff`=5 \| `hitl`=6 \| `step`=7 \| `error`=8 (string) \| `chat`=9 |
| `ChatMessage.author` / `.author_role` / `.text` / `.in_reply_to` | string 1/2/3/4 | wire | | `author_role`: AGENT/HUMAN/SYSTEM |
| `Proposal.agent` / `.confidence` / `.content_json` | string=1 / double=2 / string=3 | wire | | |
| `Consensus.decision_maker` / `.strength` / `.decision_json` / `.requires_human` | string=1 / double=2 / string=3 / bool=4 | wire | | |
| `Handoff.phase` (enum START/ACK/COMPLETE/ERROR) / `.from_agent` / `.to_agent` / `.contract` / `.payload_digest` | enum=1 / string 2-5 | wire | | |
| `HitlEscalation.reason` / `.confidence` | string=1 / double=2 | wire | | |
| `StepResult.step_id` / `.success` / `.summary` | string=1 / bool=2 / string=3 | wire | | |
| `VerificationEvidence.run_id` / `.git_sha` / `.steps` / `.verdict` / `.produced_at` | string 1/2 / repeated=3 / Verdict=4 / string=5 | wire | | espelha `verification-evidence.v1` |
| `VerificationStep.name` / `.tool` / `.exit_code` / `.duration_ms` / `.findings` | string 1/2 / int32=3 / uint64=4 / repeated=5 | wire | | |
| `VerificationFinding.tool` / `.severity` / `.message` / `.file` / `.line` | string 1/2/3 / optional string=4 / optional uint64=5 | wire | | `optional` preserva presença (espelha `skip_serializing_if` Rust) |
| `Verdict` (enum UNSPECIFIED/PASS/FAIL/SKIPPED) | enum | wire | | UNSPECIFIED = fail-closed |
| `HealthResponse.ready` / `.version` | bool=1 / string=2 | wire | | |

Fluxo: Rust envia `SquadTask` (com roster/model/tenant/evidência tipada) → Python streama `SquadEvent`s (oneof proposal/consensus/handoff/hitl/step/chat/error) ecoando tenant/actor verbatim.

---

## schemas/proto/memory.proto (`btv.memory.v1`)

Papel: `MemoryService` — direção OPOSTA de CoreService: o Python serve (dono do corpus episódico + índice TF-IDF), o Rust chama. Sem `Remember` (só o orquestrador grava, em processo).

| Item | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Health` / `Recall` / `List` | rpcs | wire | Rust → Python | Recall = busca léxica TF-IDF (não semântica) |
| `RecallRequest.query` / `.k` | string=1 / uint32=2 | wire | | |
| `MemoryMatch.id` / `.agent` / `.decision_json` / `.timestamp` / `.score` | string 1/2/3/4 / double=5 | wire | Python → Rust | `decision_json` = mesmo formato de `remember_decision` |
| `RecallResponse.matches` | repeated MemoryMatch=1 | wire | | |
| `ListRequest.agent` / `.limit` | optional string=1 / uint32=2 | wire | | |
| `MemorySummary.agent` / `.count` / `.latest_decision_json` / `.latest_timestamp` / `.top_confidence` | string=1 / uint32=2 / string=3 / string=4 / double=5 | wire | | NUNCA inclui tendência de esquecimento (forgetting.py foi removido) |
| `ListResponse.agents` | repeated MemorySummary=1 | wire | | |

Fluxo: Rust chama `Recall`(query,k)→`MemoryMatch`s pontuados e `List`(agent,limit)→`MemorySummary`s; o Python é o único dono do corpus.

---

## schemas/proto/promptforge.proto (`btv.promptforge.v1`)

Papel: `PromptForgeService` — sidecar Python expõe geradores declarativos, quality linter e render (origem: prompte); NÃO gera texto de LLM (regra ADR 0001).

| Item | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Health` / `Lint` / `Render` / `ListGenerators` | rpcs | wire | Rust → Python | |
| `LintRequest.prompt` | string=1 | wire | | |
| `LintIssue.rule` / `.message` | string 1/2 | wire | | |
| `LintReport.score` / `.grade` / `.issues` | double=1 / string=2 / repeated=3 | wire | Python → Rust | |
| `RenderRequest.generator` / `.fields` | string=1 / map<string,string>=2 | wire | Rust → Python | |
| `RenderResponse.prompt` | string=1 | wire | | |
| `GeneratorField.name` / `.label` / `.required` / `.placeholder` | string=1 / string=2 / bool=3 / string=4 | wire | | |
| `GeneratorInfo.name` / `.category` / `.fields` | string=1 / string=2 / repeated=3 | wire | | serde direto p/ `GET /api/prompt/generators` |
| `HealthResponse.ready` / `.version` | bool=1 / string=2 | wire | | |

Fluxo: Rust chama `Lint`(prompt)→`LintReport`, `Render`(generator,fields)→prompt, `ListGenerators`→`GeneratorInfo`s.

---

## crates/btv-sidecar/src/lib.rs

Papel: raiz do crate sidecar — declara módulos e reexporta clientes/supervisores/serviços.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| módulos `client`/`core_server`/`memory_client`/`service`/`squad_client`/`supervisor` | módulos | config | | |
| reexports (`SidecarClient`, `CoreBackend`/`CoreServer`/`serve_core`, `MemoryClient`/`MemorySupervisor`, `MemoryService`/`SidecarService`/`SquadLease`/`SquadPool`, `SquadClient`/`SquadRun`/`SquadSupervisor`/`drain_stream`, `SidecarSupervisor`) | tipos | saída | | superfície pública |

Fluxo: agrega os 6 módulos e reexporta a API do sidecar.

---

## crates/btv-sidecar/src/client.rs

Papel: cliente gRPC do `PromptForgeService` sobre Unix Domain Socket.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SidecarError` | enum | saída | | `Unavailable(String)`, `Rpc(Box<tonic::Status>)` |
| `SidecarClient.inner` | `PromptForgeServiceClient<Channel>` | estado | | |
| `connect(path)` | `PathBuf` → Self | entrada | UDS via `tower::service_fn` + `UnixStream::connect` | URI placeholder `http://sidecar.invalid`; conexão lazy |
| `health()` | `(bool, String)` | wire/saída | RPC Health | `(ready, version)` |
| `lint(prompt)` | `LintReport` | wire | RPC Lint | |
| `render(generator, fields: HashMap)` | `String` | wire | RPC Render | retorna `.prompt` |
| `list_generators()` | `Vec<GeneratorInfo>` | wire | RPC ListGenerators | |
| `socket_ready(path)` | `bool` | intermediário | `path.exists()` | poll de prontidão do supervisor |

Fluxo: `connect` abre um `Channel` tonic sobre UDS (connector custom); os métodos empacotam os requests proto e devolvem os campos relevantes das respostas.

---

## crates/btv-sidecar/src/core_server.rs

Papel: servidor `CoreService` — lado Rust do laço bidirecional; o squad Python chama de volta `Generate`/`RequestPermission`/`RunTool`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `CoreBackend` (trait) | | | injetável | `generate(&LlmRequest) → Result<(String, Usage), String>`, `request_permission(&PermissionRequest) → bool`, `run_tool(&ToolCall) → ToolResult` |
| `CoreServer.backend` | `Arc<B>` | estado | | |
| `generate` `(tx, rx)` | `mpsc::channel(8)` | intermediário | backend → stream | task spawnada; sucesso envia `TextDelta` + `Usage`; erro envia `Error` |
| `GenerateStream` | `ReceiverStream<Result<LlmChunk, Status>>` | wire/saída | | tipo do stream de resposta |
| `send` closure | fn | intermediário | payload → `LlmChunk{payload:Some}` | inline p/ evitar `result_large_err` |
| `request_permission` `approved` | `bool` | intermediário | backend → `Decision::Allow/Deny` | `operator_note: None` |
| `run_tool` | `ToolResult` | wire/saída | backend | erro de domínio vira payload, não `Status` |
| `append_ledger`/`recall`/`remember` | `Status::unimplemented` | saída | | honestamente não usados |
| `serve_core(backend, socket_path)` | fn | entrada | `UnixListener::bind` → `UnixListenerStream` → tonic Server | remove socket antigo antes do bind |

Fluxo: `serve_core` sobe o `CoreService` num UDS; `Generate` roda o backend numa task e empurra `LlmChunk`s por um `mpsc(8)`/`ReceiverStream`; `RequestPermission`/`RunTool` traduzem o retorno do backend em decisão/resultado; RPCs não usados respondem `Unimplemented`.

---

## crates/btv-sidecar/src/supervisor.rs

Papel: sobe e supervisiona o sidecar Python `btv_promptforge.server`, esperando o health check (fallback progressivo).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SidecarSupervisor{child: Child, socket_path: PathBuf}` | struct | estado | | |
| `spawn(python_workspace_dir, socket_path)` | args | entrada | `uv run python -m btv_promptforge.server --socket <path>` | cria dir do socket + remove socket antigo; `stdin/stdout=null`, `stderr=piped`, `kill_on_drop`, `process_group(0)` (unix) |
| `pid()` | `Option<u32>` | saída | `child.id()` | prova estabilidade/troca de PID |
| `kill()` | — | entrada | `libc::kill(-pid, SIGKILL)` (grupo) + `child.kill()` | mata uv+python (evita órfão) |
| `wait_ready(timeout)` | `SidecarClient` | saída | poll socket + health até deadline | `try_wait` → morte precoce inclui stderr no erro; sleep 50ms |
| `Drop` | — | — | `libc::kill(-pid, SIGKILL)` síncrono | evita Python órfão (uv reforka) |

Fluxo: `spawn` lança `uv run … --socket <path>` como líder de grupo; `wait_ready` faz poll do socket + Health até ficar `ready` ou timeout/morte; `kill`/`Drop` sinalizam o grupo inteiro.

---

## crates/btv-sidecar/src/squad_client.rs

Papel: cliente + supervisor do `btv_squad.server`; `ExecuteTask` devolve stream de `SquadEvent`; o supervisor sobe o processo com dois sockets (`--socket` + `--core-socket`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SquadClient.inner` | `SquadServiceClient<Channel>` | estado | UDS `http://squad.invalid` | |
| `connect(path)` | Self | entrada | UDS connector | |
| `health()` | `(bool, String)` | wire | RPC | |
| `execute_task(task: SquadTask)` | `Streaming<SquadEvent>` | wire/saída | RPC ExecuteTask | |
| `SquadRun` | enum | saída | `Completed(Vec<SquadEvent>)` \| `Failed{events, reason}` | base do fallback progressivo |
| `drain_stream(stream)` `events` | `Vec<SquadEvent>` | intermediário→saída | loop `stream.message()` | `error` no stream → `Failed`; quebra de transporte (`Err(Status)`) → `Failed`; `Ok(None)` → `Completed` |
| `SquadSupervisor{child, socket_path}` | struct | estado | | |
| `spawn(python_workspace_dir, socket_path, core_socket, model)` | args | entrada | `uv run python -m btv_squad.server --socket <s> --core-socket <c> --model <m>` | cria dir + remove socket; `process_group(0)`, `kill_on_drop`, `stderr=piped` |
| `pid()` / `kill()` / `wait_ready(timeout)` | | saída | idem supervisor.rs | `kill` sinaliza grupo (uv+python) |
| `Drop` | — | | `libc::kill(-pid, SIGKILL)` | evita órfão |

Fluxo: `SquadSupervisor::spawn` sobe o servidor com socket próprio + core-socket + model; `SquadClient::execute_task` retorna o `Streaming<SquadEvent>` que `drain_stream` coleta em `SquadRun::Completed`/`Failed` (dispara fallback squad→agente-único→safe-mode).

---

## crates/btv-sidecar/src/memory_client.rs

Papel: cliente + supervisor do `btv_squad.memory_server` (ADR 0022) — só lê o corpus episódico (sem `--core-socket`; o Python é dono do dado).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `MemoryClient.inner` | `MemoryServiceClient<Channel>` | estado | UDS `http://memory.invalid` | |
| `connect(path)` / `health()` | | entrada/wire | | |
| `recall(query, k)` | `RecallResponse` | wire/saída | RPC Recall | |
| `list(agent: Option<String>, limit)` | `ListResponse` | wire/saída | RPC List | |
| `MemorySupervisor{child, socket_path}` | struct | estado | | |
| `spawn(python_workspace_dir, socket_path, memory_dir: Option<&Path>)` | args | entrada | `uv run python -m btv_squad.memory_server --socket <path> [--memory-dir <dir>]` | `memory_dir=None` em produção (resolução relativa `.btv/squad-memory`, simétrica ao `SquadServicer`); `Some` = corpus isolado de teste |
| `pid()` / `kill()` / `wait_ready(timeout)` / `Drop` | | | idem outros supervisores | grupo de processo p/ evitar órfão |

Fluxo: `MemorySupervisor::spawn` sobe o memory_server (sem core-socket); `MemoryClient::recall`/`list` fazem leitura TF-IDF/agrupamento sobre o JSONL episódico.

---

## crates/btv-sidecar/src/service.rs

Papel: camada de serviço de longa duração (ADR 0019) — processos que sobem uma vez e ficam vivos entre requisições, com health-check + restart-on-crash; `SidecarService`/`MemoryService` singleton, `SquadPool` com semáforo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SidecarState{supervisor, client}` | struct | estado | | par vivo do PromptForge |
| `SidecarService{python_workspace_dir, socket_path, ready_timeout, state: Mutex<Option<SidecarState>>}` | struct | estado | | singleton serializado |
| `client()` | `SidecarClient` | saída | reusa se health OK; senão `spawn`+`wait_ready` (PID novo) | serializado por `tokio::sync::Mutex` (política declarada, stateless) |
| `current_pid()` / `kill_current()` | `Option<u32>` / — | saída/entrada | | restart sob demanda / injeção de falha |
| `MemoryState{supervisor, client}` / `MemoryService{…, memory_dir: Option<PathBuf>, …}` | struct | estado | | singleton (leitura stateless) — deliberadamente NÃO usa o `SquadPool` |
| `MemoryService::{client,current_pid,kill_current}` | | saída/entrada | idem `SidecarService` | |
| `SquadSlotState{supervisor, client}` | struct | estado | por slot | |
| `SquadPool{python_workspace_dir, core_socket, model, ready_timeout, slot_sockets: Vec<PathBuf>, slot_states: Vec<Mutex<Option<SquadSlotState>>>, semaphore: Arc<Semaphore>, free: std::sync::Mutex<Vec<usize>>}` | struct | estado | | pool gated por semáforo de capacidade fixa |
| `new(…, socket_dir, …, capacity, …)` | args | entrada | `slot_sockets = socket_dir/squad-slot-{i}.sock` | `assert capacity>0`; `Semaphore::new(capacity)`; `free = 0..capacity` |
| `capacity()` | `usize` | saída | | |
| `acquire(self: &Arc<Self>)` `permit` | `OwnedSemaphorePermit` | intermediário | `semaphore.acquire_owned()` | espera se todos ocupados (nunca > capacity processos) |
| `guard` (`SlotGuard`) `slot` | `usize` | intermediário | `free.pop()` | invariante 1:1 permit↔slot; devolve slot em erro/panic |
| `SquadLease{pool, slot, client, _permit}` | struct | saída | | posse do slot; `Drop` devolve slot a `free` |
| `pid_of(slot)` / `kill_slot(slot)` | `Option<u32>` / — | saída/entrada | | observabilidade / injeção de falha |
| `SlotGuard{pool, slot: Option<usize>}` + `disarm()` | struct | intermediário | | desarma no sucesso (lease assume posse); `Drop` devolve slot |

Fluxo: `SidecarService`/`MemoryService` mantêm um único processo vivo (health-check reusa, falha ⇒ novo PID); `SquadPool::acquire` pega um permit do `Semaphore`, retira um índice de `free` sob `SlotGuard` (invariante 1:1), garante o processo do slot vivo (spawn+`wait_ready` se morto) e devolve um `SquadLease` que ao `Drop` recicla o slot.
