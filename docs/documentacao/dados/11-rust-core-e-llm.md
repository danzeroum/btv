# 11 — Mapa de dados: runtime e gateway Rust (btv-core, btv-llm)

Dicionário de dados exaustivo dos crates `crates/btv-core` (runtime de sessão:
loop de agente, permissões, compaction, sessão durável, perfis) e
`crates/btv-llm` (gateway LLM: providers HTTP, SSE, agregação de turno, tiers,
preços, rate limit, gerador roteirizado). Cobre TODO dado que circula —
entrada, saída, intermediário, estado, config e wire — inclusive locais
descartados.

**Escopo.** Só os dois crates. Os tipos de conversa (`ChatMessage`,
`ContentBlock`, `AssistantTurn`, `GenerateRequest`, `StopReason`, `Usage`,
`ToolSpec`, `Role`, `ModelTier`) MORAM em `btv-domain::chat` (fora do escopo),
mas atravessam quase toda função aqui; são marcados `wire` quando serializados
e referenciados pelos campos que importam. Uma nota de referência dos campos
desses tipos está ao fim, por conveniência (sem seção `##` própria — não são
arquivos destes crates).

**Legenda de Direção.**
- `entrada` — parâmetro / argumento / leitura de fonte externa.
- `saída` — retorno / escrita / evento emitido.
- `intermediário` — local / buffer / acumulador (mesmo que descartado).
- `estado` — campo de struct que persiste entre chamadas.
- `config` — constante / env var / valor default.
- `wire` — proto / JSON / SQLite / serde (formato de fronteira).

---

## `crates/btv-core/src/lib.rs`
Raiz do crate: declara módulos e re-exporta a API pública do runtime.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `agent`, `agent_loop`, `compaction`, `permission`, `session` | `pub mod` | config | fonte → árvore de módulos | declaração dos 5 módulos do crate |
| `AgentProfile`, `BUILD`, `GENERAL`, `PLAN` | re-export de `agent` | saída | módulo → consumidores | perfis selecionáveis |
| `AgentLoop`, `DenyAll`, `LoopError`, `LoopEvent`, `LoopOutcome`, `PermissionResolver`, `TurnSummary` | re-export de `agent_loop` | saída | módulo → CLI/squad | superfície do loop |
| `estimate_tokens`, `CompactionPolicy` | re-export de `compaction` | saída | módulo → chamadores | heurística + política |
| `Decision`, `PermissionEngine`, `Rule` | re-export de `permission` | saída | módulo → chamadores | motor de permissões |
| `DurableSession`, `SessionError` | re-export de `session` | saída | módulo → CLI | sessão durável |

**Fluxo:** nenhum dado runtime — só reexporta símbolos; a Fase 1 entrega
permissões+perfis+loop, o System Context completo fica para a Fase 2.

---

## `crates/btv-core/src/agent.rs`
Perfis de agente selecionáveis (build/plan/general), cada um com uma fábrica de `PermissionEngine`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AgentProfile.name` | `&'static str` | estado/config | constante → CLI/ledger | rótulo do perfil ("build"/"plan"/"general") |
| `AgentProfile.description` | `&'static str` | estado/config | constante → UI | texto de ajuda |
| `AgentProfile.permissions` | `fn() -> PermissionEngine` | estado/config | ponteiro de função → chamador | fábrica invocada com `(P.permissions)()` |
| `full_access()` retorno | `PermissionEngine` | saída | fábrica → perfil BUILD | 4 regras: read=Allow, grep=Allow, edit=Ask, bash=Ask |
| `BUILD` | `const AgentProfile` | config | fonte → runtime | acesso total; edit/bash sob confirmação |
| `PLAN` | `const AgentProfile` | config | fonte → runtime | só leitura; `permissions = PermissionEngine::read_only` |
| `GENERAL` | `const AgentProfile` | config | fonte → runtime | subagente de exploração; também `read_only` |
| `Rule { tool, scope_prefix, decision }` (×4 em `full_access`) | `Rule` | intermediário | literal → `rules` vec | `scope_prefix: None` em todas |
| (tests) `perms` / `evaluate("edit","x")` | `Decision` | intermediário | fábrica → asserção | prova PLAN=Deny, BUILD=Ask |

**Fluxo:** `AgentProfile.permissions` (ponteiro de fn) → invocação → `PermissionEngine` com regras fixas → consumido pelo `AgentLoop.permissions`.

---

## `crates/btv-core/src/agent_loop.rs`
Coração da Fase 1: loop `mensagens → GenerateRequest → AssistantTurn → tool_use → permissão → tool_result → repete` até `end_turn` ou `max_steps`. Genérico sobre `LlmPort`; executa tools via `ToolsPort`; emite `LoopEvent`.

### Tipos e estado

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LoopEvent::TextDelta(&str)` | evento (borrow) | saída | `generate` on_delta → observador | delta de texto em streaming |
| `LoopEvent::TurnCompleted { provider, input_tokens, output_tokens }` | evento | saída | `turn` → observador | `provider: String`, tokens `u64` copiados de `turn.usage` |
| `LoopEvent::ToolStarted { name, scope }` | evento | saída | `run_tool` → observador | ferramenta autorizada prestes a rodar |
| `LoopEvent::ToolFinished { name, ok, summary, diff }` | evento | saída | `run_tool` → observador | `summary: String` (1ª linha, 80 chars), `diff: Option<Vec<DiffLine>>` |
| `LoopEvent::ToolDenied { name, scope }` | evento | saída | `run_tool` → observador | negada pela política/usuário |
| `DenyAll` | struct unit | config | fonte → `resolve` | resolver não-interativo: sempre `false` |
| `LoopError::Gateway(LlmError)` | enum (from) | saída | `?` do gateway → chamador | `#[from] LlmError` |
| `LoopError::MaxSteps(usize)` | enum | saída | loop esgotado → chamador | carrega `max_steps` |
| `AgentLoop.generator` | `&'a G: LlmPort` | estado/entrada | injeção → `generate` | gateway real ou roteirizado |
| `AgentLoop.tools` | `&'a dyn ToolsPort` | estado/entrada | injeção → `specs`/`get` | porta de ferramentas |
| `AgentLoop.permissions` | `PermissionEngine` | estado | perfil → `run_tool` | avalia cada tool |
| `AgentLoop.model` | `String` | estado/config | CLI → `GenerateRequest.model` | id do modelo |
| `AgentLoop.system` | `String` | estado/config | CLI → `GenerateRequest.system` | system prompt |
| `AgentLoop.max_steps` | `usize` | config | CLI → guarda do `for` | teto de iterações |
| `AgentLoop.max_tokens` | `u32` | config | CLI → `GenerateRequest.max_tokens` | teto de tokens de saída |
| `LoopOutcome.final_text` | `String` | saída | `summary.final_text` → chamador | texto do último turno |
| `LoopOutcome.steps` | `usize` | saída | contador → chamador | nº de passos gastos |
| `LoopOutcome.messages` | `Vec<ChatMessage>` | saída | histórico completo → chamador | conversa inteira (owned) |
| `TurnSummary.final_text` | `String` | saída | `turn.text()` → `run` | texto do turno final da rodada |
| `TurnSummary.steps` | `usize` | saída | `step` → `run` | passos da rodada |

### Fluxo de dados dentro de `run` / `continue_run` / `run_tool`

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `task` (param de `run`) | `&str` | entrada | chamador → `ChatMessage::user_text` | vira 1ª mensagem `user` |
| `messages` (inicial em `run`) | `Vec<ChatMessage>` | intermediário | `vec![user_text(task)]` → `continue_run` | mutado por referência |
| `messages` (param de `continue_run`) | `&mut Vec<ChatMessage>` | entrada/saída | REPL → loop | histórico acumulado in-place |
| `resolver` | `&mut dyn PermissionResolver + Send` | entrada | CLI/teste → `run_tool` | decide os `Ask` |
| `on_event` | `&mut dyn FnMut(LoopEvent) + Send` | saída | loop → CLI/ledger | callback de eventos |
| `specs` | `Vec<ToolSpec>` | intermediário | `self.tools.specs()` → `GenerateRequest.tools` | clonado a cada passo (`specs.clone()`) |
| `step` | `usize` (1..=max_steps) | intermediário | contador do `for` | vira `TurnSummary.steps` |
| `req` (`GenerateRequest`) | struct | intermediário/wire | campos do loop → `generator.generate` | `model/system/messages.clone()/tools/max_tokens` + `temperature: None` |
| `sink` | closure `\|d:&str\|` | intermediário | fecha sobre `on_event` → `generate` | mapeia delta → `LoopEvent::TextDelta` |
| `turn` (`AssistantTurn`) | struct | intermediário | `generate` → processamento | fonte de content/stop_reason/usage/provider |
| `turn.provider/usage.*` | `String`/`u64` | saída | `turn` → `TurnCompleted` | clonado ao emitir evento |
| `tool_uses` | `Vec<(String,String,Value)>` | intermediário | `turn.tool_uses()` → loop de execução | `(id,name,input)` clonados de borrows → owned |
| `ChatMessage { Assistant, turn.content.clone() }` | struct | intermediário/wire | push em `messages` | turno do assistente entra no histórico |
| condição de parada | `turn.stop_reason != ToolUse \|\| tool_uses.is_empty()` | intermediário | decisão → return | encerra rodada com `TurnSummary` |
| `results` | `Vec<ContentBlock>` | intermediário | `run_tool` por tool → `ChatMessage User` | acumula `ToolResult` |
| `ContentBlock::ToolResult { tool_use_id: id, content: result.0, is_error: result.1 }` | wire | intermediário | tupla de `run_tool` → bloco | reata o resultado ao `id` do pedido |
| `ChatMessage { User, results }` | struct | intermediário/wire | push em `messages` | tool_results voltam ao modelo no próximo passo |
| retorno `Err(LoopError::MaxSteps)` | erro | saída | `for` esgotado → chamador | nenhum `end_turn` em `max_steps` |

### `run_tool(name, input, resolver, on_event) -> (String, bool)`

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `name` | `&str` | entrada | tool_use → `tools.get` | id da ferramenta |
| `input` | `&Value` | entrada | tool_use → `tool.scope`/`tool.run` | argumentos JSON |
| `tool` | `&dyn Tool` | intermediário | `self.tools.get(name)` | `None` → `("ferramenta desconhecida: {name}", true)` |
| `scope` | `String` | intermediário | `tool.scope(input)` → `permissions.evaluate` | ex.: caminho/comando |
| `allowed` | `bool` | intermediário | `match Decision` → guarda | Allow→true, Deny→false, Ask→`resolver.resolve` |
| ramo negado | `(String, bool=true)` | saída | → `ToolDenied` + tool_result erro | `"permissão negada para {name} em {scope:?}"` |
| `out` (`ToolOutput`) | struct | intermediário | `tool.run(input)` Ok → processamento | content/truncated/overflow_path/diff |
| `summary` | `String` | intermediário/saída | `out.content.lines().next().take(80)` → `ToolFinished` | 1ª linha, ≤80 chars |
| `content` | `String` (mut) | saída | `out.content` → tool_result | anexa nota de truncamento se `out.truncated` |
| nota de overflow | texto | intermediário | `out.overflow_path` → `content.push_str` | `"[output truncado; completo em {path} …]"` ou `"[output truncado]"` |
| retorno Ok | `(content, false)` | saída | → `ContentBlock::ToolResult` | resultado bem-sucedido |
| ramo Err | `(e.to_string(), true)` | saída | `tool.run` Err → tool_result erro | `ToolFinished { ok:false, summary:e, diff:None }` |

**Tests (mocks puros, DoD <100ms):** `Scripted`/`Counting` (LlmPort roteirizado, `turns: Mutex<Vec<AssistantTurn>>`, `seen: Mutex<Vec<usize>>` conta `req.messages.len()`), `MockTool` (`ran: AtomicBool`), `MockTools`, `AllowAll` (resolve=true). Provam: fluxo completo, negação sem execução, tool desconhecida, nota de truncamento, histórico entre turnos (`seen == [1,3]`), e `MaxSteps(5)`.

**Fluxo:** `task/messages` → (por passo) `GenerateRequest{messages.clone, tools=specs.clone}` → `generate` (emite `TextDelta`, retorna `AssistantTurn`) → `TurnCompleted` → push `Assistant(turn.content)` → se `ToolUse`: para cada `tool_use` `run_tool` (scope→`evaluate`→`Decision`→resolver) → `ContentBlock::ToolResult` → push `User(results)` → repete; senão retorna `final_text=turn.text()`. Estouro → `LoopError::MaxSteps`.

---

## `crates/btv-core/src/permission.rs`
Motor de permissões por (ferramenta, escopo): regras ordenadas, primeira compatível vence, sem regra → `Ask`. Decisões vivem só no Rust (não contornáveis pelo sidecar).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Decision::{Allow,Ask,Deny}` | enum | wire/saída | `evaluate` → `run_tool` | `#[serde(rename_all="snake_case")]`; `Copy` |
| `Rule.tool` | `String` | estado/wire | config/perfil → `evaluate` | nome da ferramenta |
| `Rule.scope_prefix` | `Option<String>` | estado/wire | config → `evaluate` | `#[serde(skip_serializing_if="Option::is_none")]`; casa por `starts_with` |
| `Rule.decision` | `Decision` | estado/wire | config → retorno | decisão da regra |
| `PermissionEngine.rules` | `Vec<Rule>` | estado/wire | perfil → `evaluate` | avaliadas em ordem; `Default`=vazio |
| `evaluate(tool, scope)` params | `&str, &str` | entrada | `run_tool`/overlay → laço | tool+scope a decidir |
| laço de `evaluate` | iteração sobre `&rules` | intermediário | rules → decisão | pula `rule.tool != tool`; `scope_prefix Some` sem `starts_with` → continua |
| retorno de `evaluate` | `Decision` | saída | regra ou fallback | sem match → `Decision::Ask` |
| `overlay(overrides)` param | `&[Rule]` | entrada | matriz/ "sempre" persistido → merge | Fase 7 Onda 2 |
| `rules` (em overlay) | `Vec<Rule>` (mut) | intermediário | `overrides.to_vec()` + `self.rules` | overrides checadas primeiro (vencem o default do perfil) |
| retorno de `overlay` | `PermissionEngine` | saída | → novo engine | não duplica `evaluate` |
| `read_only()` retorno | `PermissionEngine` | saída | fábrica → PLAN/GENERAL/safe-mode | read=Allow, grep=Allow, edit=Deny, bash=Ask |

**Fluxo:** `(tool, scope)` → `evaluate` percorre `rules` (com `overlay` colocando overrides na frente) → primeira regra compatível por tool+prefixo devolve `Decision`; nenhuma → `Ask`.

---

## `crates/btv-core/src/compaction.rs`
Compaction de contexto em fronteiras seguras (Fase 2): estima tokens por `chars/4`, dispara por threshold tier-gated, resume via gateway sem ferramentas.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `estimate_tokens(messages)` param | `&[ChatMessage]` | entrada | histórico → soma | fonte da contagem |
| `chars` | `usize` | intermediário | flat_map sobre blocks → soma | `Text`→`text.len()`; `ToolUse`→`name.len()+input.to_string().len()`; `ToolResult`→`content.len()` |
| retorno | `usize` | saída | `chars / 4` → política | heurística do fork (BPE real won't-do) |
| `CompactionPolicy.context_window_tokens` | `usize` | estado/config | tier/modelo → `needs_compaction` | janela do modelo |
| `CompactionPolicy.threshold` | `f64` | estado/config | `tier.compaction_threshold()` → limite | small 0.75, demais 0.90 |
| `for_tier(tier, window)` params | `ModelTier, usize` | entrada | chamador → construção | monta a política |
| `needs_compaction(messages)` | `bool` | saída | `estimate_tokens >= limit` | `limit = (window as f64 * threshold) as usize` |
| `limit` | `usize` | intermediário | window×threshold → comparação | descartado após comparar |
| `is_safe_boundary(messages)` | `bool` | saída | último `ChatMessage` → decisão | true se role=Assistant e sem `ContentBlock::ToolUse` pendente |
| `summarize` params | `&G: LlmPort, &str model, &[ChatMessage]` | entrada | política → gateway | pede resumo |
| `prompt_messages` | `Vec<ChatMessage>` | intermediário | `messages.to_vec()` + push user | acrescenta pedido de resumo |
| prompt de resumo | `&str` (literal) | config | fonte → `user_text` | "Resuma esta conversa … objetivo, decisões, arquivos tocados e estado atual, pendências. Seja denso e factual; não invente nada." |
| `GenerateRequest` do resumo | struct | intermediário/wire | política → `generate` | `system:"Você resume conversas de trabalho de um coding agent."`, `tools: vec![]`, `max_tokens: 2048`, `temperature: None` |
| `turn` | `AssistantTurn` | intermediário | `generate` → `turn.text()` | on_delta descartado (`&mut \|_\| {}`) |
| retorno de `summarize` | `Result<String, LlmError>` | saída | `turn.text()` → baseline da época | texto do resumo |

**Fluxo:** `messages` → `estimate_tokens` (`chars/4`) → `needs_compaction` (≥ window×threshold) e `is_safe_boundary` → `summarize` monta `GenerateRequest` sem tools → `AssistantTurn.text()` vira a baseline resumida (consumida por `DurableSession::compact`).

---

## `crates/btv-core/src/session.rs`
Sessão durável (Fase 2): a conversa é um agregado de eventos `message.1`; reabrir replaya o histórico; concorrência otimista por `head`; compaction inicia nova época.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SESSION_STARTED` = `"session.started.1"` | `const &str` | config/wire | fonte → `EventInput.kind` | kind do 1º evento |
| `MESSAGE` = `"message.1"` | `const &str` | config/wire | fonte → kind | kind de cada mensagem persistida |
| `EPOCH_STARTED` = `"epoch.started.1"` | `const &str` | config/wire | fonte → kind | kind do marco de época |
| `SessionStore` (trait alias) | trait | config | fonte → bound | fixa `NewEvent=EventInput`, `StoredEvent=domain::StoredEvent` |
| `SessionError::Store(RepositoryError)` | enum (from) | saída | port → chamador | `#[from]` |
| `SessionError::Malformed { session_id, seq, reason }` | enum | saída | serde falho → chamador | evento inválido no replay/serialização |
| `DurableSession.store` | `S: SessionStore` | estado | injeção → append/read/head_seq | driver de eventos |
| `DurableSession.ctx` | `TenantContext` | estado/entrada | dono da sessão → toda op no port | no CLI local é `TenantContext::local` |
| `DurableSession.session_id` | `String` (pub) | estado | chamador → aggregate_id | id do agregado |
| `DurableSession.messages` | `Vec<ChatMessage>` (pub) | estado/saída | replay + turnos → loop | histórico corrente |
| `DurableSession.head` | `i64` | estado/wire | store → append(expected_head) | nº do último evento (concorrência otimista) |
| `DurableSession.persisted` | `usize` | estado | replay/persist → slice | quantas msgs já persistidas |
| `DurableSession.epoch` | `usize` | estado | replay/compact → `epoch()` | época atual (0=nunca compactada) |
| `open` params | `store, ctx, session_id, task_hint, model` | entrada | CLI → abertura | reconstrói ou cria |
| `head` (em open) | `i64` | intermediário | `store.head_seq` | 0 → sessão nova |
| evento inicial (open, head==0) | `EventInput::new(SESSION_STARTED, json!({"task": task_hint, "model": model}))` | wire | novo → append(expected_head=0) | grava criação |
| laço de replay | `for event in store.read(...,0)` | intermediário | eventos → messages | `match event.kind` |
| `message` (replay) | `ChatMessage` | intermediário/wire | `serde_json::from_value(event.data)` | falha → `Malformed{seq,reason}` |
| `epoch += 1; messages.clear()` | efeito | intermediário | `EPOCH_STARTED` no replay | replay recomeça do resumo |
| `persisted` (open) | `usize` | intermediário | `messages.len()` após replay | baseline de persistência |
| `compact(summary)` param | `&str` | entrada | `CompactionPolicy::summarize` → nova época | só em fronteira segura |
| `baseline` | `ChatMessage` | intermediário/wire | `user_text("[Contexto resumido…]\n{summary}")` | vira único item do histórico |
| `baseline_event` | `Value` | wire | `to_value(&baseline)` | serializado p/ `MESSAGE` |
| append de compact | `[EventInput(EPOCH_STARTED,{"summary":summary}), EventInput(MESSAGE, baseline_event)]` | wire | 2 eventos atômicos → store | mesmo append (atomicidade) |
| efeitos de compact | `head`, `epoch+=1`, `messages=vec![baseline]`, `persisted=1` | estado | → nova época | histórico em memória trocado |
| `persist_new` | `Result<usize, SessionError>` | saída | `messages[persisted..]` → append | nº de eventos gravados |
| `new` | `Vec<EventInput>` | intermediário/wire | mensagens novas → `to_value` | vazio → `Ok(0)` (idempotente) |
| `count` | `usize` | intermediário/saída | `new.len()` → retorno | quantos gravou |
| `head` (após append) | `i64` | estado | `store.append(ctx,id,head,new)` | conflito → `ConcurrencyConflict` |
| `resumed_messages()` | `usize` | saída | `persisted` → CLI | quantas vieram do replay |

**Tests (MemStore em memória, `Arc<Mutex<HashMap<String,Vec<StoredEvent>>>>`):** `append` valida `found == expected_head` (senão `ConcurrencyConflict`), gera `StoredEvent{id:"evt_{seq}",aggregate_id,seq,kind,data}`; provam sobrevivência à reabertura, conflito concorrente, compaction/nova época, e replay de tool_use/tool_result.

**Fluxo:** `ChatMessage` → `persist_new` serializa as novas (`> persisted`) em `EventInput(MESSAGE)` → `store.append(expected_head=head)` (otimista) → head avança. `open` faz `read` e replaya: `message.1`→push, `epoch.started.1`→`epoch++`+`clear`. `compact` grava `EPOCH_STARTED`+baseline atomicamente e reduz o histórico à baseline.

---

## `crates/btv-llm/src/lib.rs`
Raiz do gateway: módulos + re-exports; nota de que as API keys vivem só neste processo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| módulos `anthropic, chat, gateway, model_tier, openai, pricing, provider, rate_limit, scripted, sse` | `pub mod` | config | fonte → árvore | 10 módulos |
| re-export `chat::{AssistantTurn,ChatMessage,ContentBlock,GenerateRequest,StopReason,ToolSpec,Usage}` | saída | módulo → consumidores | tipos de conversa (do domínio) |
| re-export `gateway::{Gateway,GatewayError,Generator}` | saída | módulo → CLI | gateway + porta |
| re-export `model_tier::{tier_from_id,ModelTier}` | saída | módulo → política | classificação |
| re-export `provider::{LlmRequest,ProviderId}` | saída | módulo → cache | request + id |
| re-export `rate_limit::{RateLimitError,RateLimiter}` | saída | módulo → decorator | rate limit |
| re-export `scripted::ScriptedGenerator` | saída | módulo → bench/k6 | gerador sem key |

**Fluxo:** sem dado runtime — reexporta a API do gateway.

---

## `crates/btv-llm/src/chat.rs`
Fachada: re-exporta os tipos de conversa de `btv-domain::chat` sob os nomes históricos (o loop os consome via `LlmPort`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, Role, StopReason, ToolSpec, Usage` | re-export | saída | `btv_domain::chat` → consumidores | zero transformação; conversão provider fica no gateway |

**Fluxo:** só re-exporta; os tipos e seus campos serde moram em `btv-domain::chat` (ver nota final).

---

## `crates/btv-llm/src/gateway.rs`
Gateway HTTP: detecta providers por env, monta corpo por provider, faz POST com streaming SSE, agrega o turno e implementa fallback na ordem da cadeia.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `GatewayError` / `Generator` | re-export de `btv_domain::ports::{LlmError, LlmPort}` | saída | domínio → CLI | nomes históricos; `Gateway` implementa a porta |
| `ProviderConfig.id` | `ProviderId` | estado | `from_env` → `call_provider` | qual provider |
| `ProviderConfig.api_key` | `String` | estado/**segredo** | env var → header HTTP | `x-api-key`/`bearer_auth`; SÓ neste processo |
| `ProviderConfig.base_url` | `String` | estado/config | const do provider → URL | ex.: `https://api.anthropic.com` |
| `Gateway.client` | `reqwest::Client` | estado | `build_http_client` → `call_provider` | cliente com timeouts |
| `Gateway.providers` | `Vec<ProviderConfig>` | estado | `from_env` → `generate` | vazio → `NoProvider` |
| **env** `BTV_LLM_CONNECT_TIMEOUT_SECS` | `u64` | config/entrada | `build_http_client` `secs()` | default **30** |
| **env** `BTV_LLM_READ_TIMEOUT_SECS` | `u64` | config/entrada | `build_http_client` `secs()` | default **120** (ociosidade, não corta stream ativo) |
| `secs(var, default)` | closure | intermediário | `env::var(var).parse()` | fallback ao default |
| `build_http_client` retorno | `reqwest::Client` | saída | builder → `Gateway.client` | `connect_timeout`+`read_timeout`; falha→`Client::new()` |
| **env** `ANTHROPIC_API_KEY` | `String` | config/entrada/**segredo** | `from_env` candidato 1 | filtrado por `!is_empty()` |
| **env** `DEEPSEEK_API_KEY` | `String` | config/entrada/**segredo** | `from_env` candidato 2 | base `DEEPSEEK_BASE_URL` |
| **env** `OPENAI_API_KEY` | `String` | config/entrada/**segredo** | `from_env` candidato 3 | base `OPENAI_BASE_URL` |
| `candidates` | array de 3 tuplas `(ProviderId, env, base)` | intermediário/config | fonte → `filter_map` | ordem = cadeia de fallback (Anthropic→DeepSeek→OpenAI) |
| `providers` (from_env) | `Vec<ProviderConfig>` | intermediário/saída | `filter_map` sobre candidates | só os com key não-vazia |
| `available()` | `Vec<String>` | saída | providers → CLI | `provider_name` de cada |
| `call_provider` params | `&ProviderConfig, &GenerateRequest, &mut on_delta` | entrada | `generate` → HTTP | uma tentativa por provider |
| `(url, request)` | `(String, RequestBuilder)` | intermediário | match `cfg.id` | Anthropic→`/v1/messages`+`x-api-key`+`anthropic-version`; outros→`/v1/chat/completions`+bearer |
| corpo Anthropic | `Value` | wire/saída | `anthropic::build_request_body(req)` → `.json()` | ver anthropic.rs |
| corpo OpenAI/DeepSeek | `Value` | wire/saída | `openai::build_request_body(req)` → `.json()` | ver openai.rs |
| `resp` | `reqwest::Response` | intermediário | `request.send().await` | erro → `format!("{url}: {e}")` |
| ramo de erro HTTP | `String` | saída | status≠2xx → `Err` | `body.chars().take(300)`; `"{url}: HTTP {status}: {body}"` |
| `parser` | `SseParser` | intermediário/estado | novo → `push(chunk)` | acumula bytes |
| `stream` | `resp.bytes_stream()` | intermediário | HTTP → laço `next().await` | chunk erro → `"stream: {e}"` |
| `agg` (Anthropic) | `anthropic::TurnAggregator` | intermediário/estado | eventos → `finish()` | agrega blocos |
| `agg` (OpenAI) | `openai::TurnAggregator` | intermediário/estado | `new(provider_name)` → `finish()` | agrega texto/tool_calls |
| `event.data` | `String` | intermediário/wire | `parser.push` → `agg.handle` | payload SSE |
| `delta` | `Option<String>` | intermediário | `agg.handle` → `on_delta(&delta)` | streaming de texto |
| retorno `call_provider` | `Result<AssistantTurn, String>` | saída | `agg.finish()` | Ok=turno; Err=string |
| `generate` (impl `Generator`) params | `GenerateRequest, &mut on_delta` | entrada | loop de agente → gateway | fallback aqui |
| `failures` | `Vec<String>` | intermediário | erros por provider → `AllFailed` | `"{provider_name}: {e}"` |
| retorno `generate` | `Result<AssistantTurn, GatewayError>` | saída | 1º Ok ou `AllFailed(join " \| ")` | `providers` vazio → `NoProvider` |
| `provider_name(id)` | `&'static str` | saída | id → rótulo | "anthropic"/"deepseek"/"openai" |

**Fluxo:** `from_env` lê `*_API_KEY` (na ordem Anthropic→DeepSeek→OpenAI) → `providers`. `generate` itera providers: `call_provider` monta URL+headers+corpo (por `ProviderId`), `send`, checa status, e faz `bytes_stream → SseParser.push → TurnAggregator.handle` (emitindo `on_delta`) → `finish()` = `AssistantTurn`. 1º sucesso retorna; todos falham → `AllFailed`; nenhum provider → `NoProvider`.

---

## `crates/btv-llm/src/provider.rs`
Contrato de provider: ids + `LlmRequest` com chave de cache (`prompt-cache-key.v1`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ProviderId::{Anthropic,Openai,Deepseek}` | enum | wire | config → gateway | `#[serde(rename_all="snake_case")]` |
| `LlmRequest.model` | `String` | estado/wire | chamador → cache/request | id do modelo |
| `LlmRequest.messages` | `Value` | estado/wire | chamador → `cache_key` | JSON de mensagens |
| `LlmRequest.temperature` | `Option<f64>` | estado/wire | chamador → cache | `skip_serializing_if="Option::is_none"` |
| `LlmRequest.max_tokens` | `Option<u32>` | estado/wire | chamador → request | `skip_serializing_if="Option::is_none"` |
| `cache_key()` retorno | `Result<String, CacheKeyError>` | saída | `btv_schemas::request_hash` | `Err` se v1 violado (ex.: `temperature 1.0`) |
| `temperature` (em cache_key) | `Value` | intermediário | `map(json!)` ou `Value::Null` | normaliza p/ o hash |
| (removido) `FallbackChain` | — | — | — | código morto; ordem real em `Gateway::from_env` |

**Fluxo:** `LlmRequest{messages, temperature}` → `cache_key` normaliza temperature → `btv_schemas::request_hash` → hash `prompt-cache-key.v1` (Err ⇒ chamador pula o cache).

---

## `crates/btv-llm/src/anthropic.rs`
Provider Anthropic (Messages API): monta corpo `/v1/messages` e agrega os eventos SSE num `AssistantTurn`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `DEFAULT_BASE_URL` = `"https://api.anthropic.com"` | `const &str` | config | fonte → `from_env` | base padrão |
| `API_VERSION` = `"2023-06-01"` | `const &str` | config/wire | fonte → header `anthropic-version` | versão da API |
| `build_request_body(req)` | `Value` | saída/wire | `GenerateRequest` → corpo POST | monta JSON |
| `messages` (body) | `Vec<Value>` | intermediário/wire | `req.messages.map(message_to_json)` | mensagens convertidas |
| `body` | `Value` | intermediário/wire | `json!({model,max_tokens,system,messages,stream:true})` | `stream: true` fixo |
| `body["temperature"]` | `Value` | wire | `req.temperature` (se Some) | condicional |
| `body["tools"]` | `Value::Array` | wire | `req.tools` (se não vazio) | `{name,description,input_schema}` por tool |
| `message_to_json(msg)` | `Value` | intermediário/wire | `ChatMessage` → `{role,content}` | role User→"user", Assistant→"assistant" |
| bloco `Text` | `Value` | wire | `{type:"text",text}` | |
| bloco `ToolUse` | `Value` | wire | `{type:"tool_use",id,name,input}` | |
| bloco `ToolResult` | `Value` | wire | `{type:"tool_result",tool_use_id,content,is_error}` | |
| `TurnAggregator.blocks` | `Vec<PartialBlock>` | estado | eventos → `finish` | Text(String) ou ToolUse{id,name,json} |
| `TurnAggregator.stop_reason` | `Option<StopReason>` | estado | `message_delta` → `finish` | default `EndTurn` |
| `TurnAggregator.usage` | `Usage` | estado | `message_start`/`message_delta` → `finish` | input/output tokens |
| `PartialBlock::Text(String)` | enum | intermediário/estado | acumula `text_delta` | texto parcial |
| `PartialBlock::ToolUse{id,name,json}` | enum | intermediário/estado | acumula `input_json_delta` | JSON de args parcial |
| `handle(data)` param | `&str` | entrada/wire | evento SSE JSON → agregação | retorna delta de texto |
| `value` | `Value` | intermediário | `from_str(data)` | inválido → `None` |
| `message_start` | efeito | intermediário/wire | `message.usage.input_tokens` → `usage` | `as_u64().unwrap_or(0)` |
| `content_block_start` | efeito | intermediário/wire | `content_block.type` → push block | tool_use→ToolUse; senão Text(text inicial) |
| `content_block_delta`/`text_delta` | `Some(String)` | saída/wire | `delta.text` → `on_delta` + append no último Text | delta emitido |
| `content_block_delta`/`input_json_delta` | efeito | intermediário/wire | `delta.partial_json` → append no ToolUse.json | não emite delta |
| `message_delta` | efeito | intermediário/wire | `delta.stop_reason` map + `usage.output_tokens` | end_turn/tool_use/max_tokens/other |
| `finish()` retorno | `AssistantTurn` | saída | blocks → content | `PartialBlock::ToolUse.json` → `from_str` (falha→objeto vazio); `provider:"anthropic"` |

**Fluxo:** `GenerateRequest → build_request_body` (JSON Messages, `stream:true`) → HTTP; resposta SSE: `handle(data)` acumula em `blocks/usage/stop_reason` e emite deltas de texto → `finish()` = `AssistantTurn{content, stop_reason, usage, provider:"anthropic"}`.

---

## `crates/btv-llm/src/openai.rs`
Provider OpenAI-compatível (Chat Completions, cobre OpenAI e DeepSeek): monta corpo `/v1/chat/completions` e agrega os chunks.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `OPENAI_BASE_URL` = `"https://api.openai.com"` | `const &str` | config | fonte → `from_env` | base OpenAI |
| `DEEPSEEK_BASE_URL` = `"https://api.deepseek.com"` | `const &str` | config | fonte → `from_env` | mesmo protocolo, base diferente |
| `build_request_body(req)` | `Value` | saída/wire | `GenerateRequest` → corpo | injeta `system` como 1ª msg |
| `messages` (body) | `Vec<Value>` | intermediário/wire | `[{role:"system",content:req.system}]` + convertidas | `message_to_json` pode expandir |
| `body` | `Value` | intermediário/wire | `json!({model,max_tokens,messages,stream:true,stream_options:{include_usage:true}})` | usage no stream |
| `body["temperature"]` | `Value` | wire | `req.temperature` (Some) | condicional |
| `body["tools"]` | `Value::Array` | wire | `req.tools` (não vazio) | `{type:"function",function:{name,description,parameters}}` |
| `message_to_json(msg)` | `Vec<Value>` | intermediário/wire | 1 msg → 0..N | User com ToolResult vira msg `role:"tool"` separada |
| ramo User | `Vec<Value>` | intermediário/wire | Text→acumula `texts`; ToolResult→`{role:"tool",tool_call_id,content}` | `is_error`→`"[erro] {content}"`; `ToolUse` ignorado no User |
| `texts.join("\n")` | `Value` | wire | textos User → `{role:"user",content}` | só se houver texto |
| ramo Assistant | `Vec<Value>` (1) | intermediário/wire | Text concatenado + tool_calls | `content: Null` se vazio |
| `tool_calls` (Assistant) | `Vec<Value>` | intermediário/wire | `ToolUse` → `{id,type:"function",function:{name,arguments:input.to_string()}}` | args como string JSON |
| `TurnAggregator.provider` | `String` | estado | `new(provider)` → `finish` | rótulo ("openai"/"deepseek") |
| `TurnAggregator.text` | `String` | estado | `delta.content` → acumula | texto do turno |
| `TurnAggregator.tool_calls` | `Vec<PartialCall>` | estado | chunks → `finish` | por índice |
| `TurnAggregator.finish_reason` | `Option<String>` | estado | `choice.finish_reason` → stop_reason | último visto |
| `TurnAggregator.usage` | `Usage` | estado | `value.usage` → `finish` | `prompt_tokens`/`completion_tokens` |
| `PartialCall{id,name,arguments}` | struct | intermediário/estado | fragmentos → tool_use | `Default` |
| `handle(data)` param | `&str` | entrada/wire | chunk SSE → agregação | `"[DONE]"` → `None` |
| `value` | `Value` | intermediário | `from_str(data)` | inválido → `None` |
| `usage` (chunk) | efeito | intermediário/wire | `value.usage` não-null → tokens | `unwrap_or(existente)` |
| `choice` | `&Value` | intermediário | `value.choices[0]` | ausente → `None` |
| `finish_reason` | efeito | intermediário/wire | `choice.finish_reason` → estado | string |
| `calls` (delta) | efeito | intermediário/wire | `delta.tool_calls[]` → `tool_calls[index]` | cresce o vec por `index`; append id/name/arguments |
| `text` (delta) | `Option<String>` | saída/wire | `delta.content` → `on_delta` + `self.text` | vazio → `None` |
| `finish()` retorno | `AssistantTurn` | saída | text+tool_calls → content | stop: tool_calls→ToolUse, length→MaxTokens, stop/None→EndTurn, _→Other; args `from_str`(falha→objeto vazio) |

**Fluxo:** `GenerateRequest → build_request_body` (system como 1ª msg; ToolResult→`role:"tool"`; `stream_options.include_usage`) → HTTP; chunks: `handle` acumula `text`/`tool_calls[index]`/`usage`/`finish_reason` e emite deltas → `finish()` = `AssistantTurn{provider}`.

---

## `crates/btv-llm/src/model_tier.rs`
Classificação de model id → `ModelTier` por regex (large checado antes de small).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ModelTier` | re-export de `btv_domain::chat` | saída | domínio → política | enum mora no domínio |
| `TierRules.small` | `Vec<Regex>` | config/estado | `OnceLock` → `tier_from_id` | haiku, mini, flash, nano, lite, small, `\b\d+b\b`, `3\.5-turbo` |
| `TierRules.large` | `Vec<(Regex, Option<Regex>)>` | config/estado | `OnceLock` → `tier_from_id` | padrão + exclusão (substitui lookahead) |
| padrões large | regex | config | fonte → match | opus, sonnet, `gpt-4\.1`, `gpt-4o`(excl `gpt-4o-mini`), `gpt-5`(excl `gpt-5-(mini\|nano)`), `gemini-[\d.]+-pro`, `-(70\|72\|123\|235\|405)b\b` |
| `rules()` | `&'static TierRules` | intermediário/estado | `OnceLock::get_or_init` | compila regex uma vez |
| `tier_from_id(model_id)` param | `&str` | entrada | modelo → classificação | case-insensitive |
| `id` | `String` | intermediário | `model_id.to_lowercase()` | normaliza |
| laço large | iteração | intermediário | `pattern.is_match && !exclusion.is_match` → Large | prioridade |
| laço small | `bool` | intermediário | `small.iter().any(is_match)` → Small | após large |
| retorno | `ModelTier` | saída | → Small/Medium/Large | default `Medium` |

**Fluxo:** `model_id` → lowercase → regex large (com exclusão) → senão regex small → senão `Medium`. Alimenta `CompactionPolicy::for_tier` e `RateLimiter::for_tier`.

---

## `crates/btv-llm/src/pricing.rs`
Tabela estática de preços por modelo (USD/1M tokens) e estimativa de custo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AS_OF` = `"2026-01"` | `const &str` | config | fonte → UI | data de referência (honestidade) |
| `ModelPrice.provider` | `&'static str` | config/saída | tabela → UI | rótulo do provider |
| `ModelPrice.input_per_mtok` | `f64` | config | tabela → custo | USD/1M input |
| `ModelPrice.output_per_mtok` | `f64` | config | tabela → custo | USD/1M output |
| `PRICES` | `&[(&str, ModelPrice)]` | config | fonte → `price_for` | ordem = prioridade; específico antes (haiku, opus, sonnet, deepseek, gpt-4o-mini, gpt-4o, gpt-4.1) |
| `price_for(model)` param | `&str` | entrada | id → busca | `to_ascii_lowercase` |
| `m` | `String` | intermediário | lowercase → `contains(key)` | casa por substring |
| retorno `price_for` | `Option<ModelPrice>` | saída | 1º match ou `None` | sem preço → não fabrica custo |
| `estimate_cost_usd(model, input_tokens, output_tokens)` params | `&str, u64, u64` | entrada | tokens reais → custo | usa `price_for` |
| `cost` | `f64` | intermediário/saída | `(in/1e6)*input + (out/1e6)*output` | estimativa |
| retorno | `Option<f64>` | saída | custo ou `None` | modelo sem preço → `None` |

**Fluxo:** `model` → `price_for` (substring, ordem) → `ModelPrice`; `(tokens/1M)×preço` → custo estimado USD (ou `None`).

---

## `crates/btv-llm/src/rate_limit.rs`
Rate limiter de janela deslizante, tier-gated (salvaguarda de custo, não anti-abuso).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RateLimitError.{max_requests,window,max_wait}` | struct (thiserror) | saída | `poll` → chamador | espera excederia `max_wait` |
| `RateLimiter.max_requests` | `usize` | estado/config | `new`/`for_tier` → `poll` | teto por janela |
| `RateLimiter.window` | `Duration` | estado/config | idem → `poll` | janela deslizante |
| `RateLimiter.max_wait` | `Duration` | estado/config | idem → `poll` | teto de espera |
| `RateLimiter.timestamps` | `Mutex<VecDeque<Instant>>` | estado | `poll` push/pop → `acquire` | marcas das chamadas admitidas |
| `new(max_requests, window, max_wait)` | ctor | entrada | chamador → struct | fila vazia |
| `for_tier(tier)` | `Self` | entrada/config | tier → limites | Small(60), Medium(30), Large(15), todos `window=600s`, `max_wait=window` |
| `poll()` | `Result<Option<Duration>, RateLimitError>` | intermediário/saída | estado → `acquire` | expira antigos, decide vaga/espera/erro |
| `now` | `Instant` | intermediário | `Instant::now()` | referência da janela |
| `ts` (lock) | `MutexGuard<VecDeque>` | intermediário | fila → poda | `pop_front` enquanto `>= window` |
| ramo com vaga | `Ok(None)` | saída | `len < max_requests` → `push_back(now)` | admite já |
| `wait` | `Duration` | intermediário/saída | `window - (now - front)` | tempo até liberar |
| ramo espera longa | `Err(RateLimitError)` | saída | `wait > max_wait` | não trava o CLI |
| `acquire()` | `Result<(), RateLimitError>` | saída | loop `poll` + `sleep(wait)` | até vaga ou erro |
| `max_requests()` | `usize` | saída | config → CLI (A4) | getter (estado encapsulado) |
| `window()` | `Duration` | saída | config → CLI (A4) | getter |

**Fluxo:** `acquire` → `poll`: poda `timestamps` fora da `window`; se `len<max_requests` empurra `now` e libera; senão calcula `wait` — se `≤max_wait` dorme e repete, se `>max_wait` erra. Composto por baixo do cache no `btv-cli` (fora deste escopo).

---

## `crates/btv-llm/src/sse.rs`
Parser incremental de Server-Sent Events: bytes arbitrários → `SseEvent` completos.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SseEvent.event` | `Option<String>` | saída/wire | `parse_block` → gateway | linha `event:` |
| `SseEvent.data` | `String` | saída/wire | `parse_block` → `agg.handle` | linhas `data:` unidas por `\n` |
| `SseParser.buffer` | `String` | estado | `push` acumula → drena | bytes ainda não delimitados |
| `push(chunk)` param | `&[u8]` | entrada | `bytes_stream` → parser | `from_utf8_lossy` |
| `events` | `Vec<SseEvent>` | intermediário/saída | laço `find("\n\n")` → retorno | eventos completos |
| `raw` | `String` | intermediário | `buffer.drain(..pos+2)` | 1 bloco |
| `parse_block(block)` | `Option<SseEvent>` | intermediário/saída | bloco → evento | `None` se sem data e sem event |
| `event` (local) | `Option<String>` | intermediário | `strip_prefix("event:")` | trim |
| `data_lines` | `Vec<String>` | intermediário | `strip_prefix("data:")` | `strip_prefix(' ')` opcional; `strip_suffix('\r')` (CRLF) |

**Fluxo:** `push(bytes)` → append no `buffer` → enquanto houver `"\n\n"`, drena o bloco e `parse_block` (coleta `event:`/`data:`, tolera `\r`) → `Vec<SseEvent>` para o `TurnAggregator`.

---

## `crates/btv-llm/src/scripted.rs`
`ScriptedGenerator`: implementa o `Generator` real sem key (turnos canned em sequência) — para benches, k6 e roteiro de cenário.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ScriptedGenerator.turns` | `Mutex<Vec<AssistantTurn>>` | estado | ctor → `generate` | sequência canned |
| `ScriptedGenerator.index` | `AtomicUsize` | estado | `generate` `fetch_add` → seleção | avança por chamada |
| `echo(text)` | `Self` | entrada | texto → `from_turn` | turno Text, `usage=0`, `provider:"scripted"` |
| `from_turn(turn)` | `Self` | entrada | turno → `from_sequence(vec![turn])` | 1 elemento |
| `from_sequence(turns)` | `Self` | entrada | Vec → struct | `assert!(!turns.is_empty())` |
| `generate` params | `_req: GenerateRequest, on_delta` | entrada | chamador → geração | req ignorado |
| `turns` (lock) | guard | intermediário | Mutex → `turns[i]` | thread-safe |
| `i` | `usize` | intermediário | `index.fetch_add(1).min(len-1)` | grampeia no último (repete p/ sempre) |
| `turn` | `AssistantTurn` | saída | `turns[i].clone()` | emite `on_delta(turn.text())` |
| retorno | `Result<AssistantTurn, GatewayError>` | saída | → chamador | sempre `Ok` |

**Fluxo:** `echo/from_turn/from_sequence` guardam `turns` → `generate` seleciona `turns[min(index++, len-1)]`, emite `on_delta(text)` e devolve o clone (determinístico, sem rede/key).

---

## `crates/btv-core/benches/compaction.rs`
Bench criterion do hot path de contexto (`estimate_tokens`, `needs_compaction`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `historico(n)` | `Vec<ChatMessage>` | intermediário | n → mensagens sintéticas | `user_text("mensagem {i}: …")` |
| `msgs` (200) | `Vec<ChatMessage>` | entrada | `historico(200)` → bench | carga fixa |
| `policy` | `CompactionPolicy` | config | `for_tier(Large, 200_000)` | política do bench |
| `estimate_tokens_200` / `needs_compaction_200` | bench | saída | `b.iter` + `black_box` | baseline do hot path |

**Fluxo:** histórico de 200 msgs → `estimate_tokens`/`needs_compaction` medidos com `black_box` (job `bench` do CI).

---

## `crates/btv-llm/benches/gateway.rs`
Bench criterion do caminho do gateway via `ScriptedGenerator` (sem key).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `req()` | `GenerateRequest` | intermediário | fonte → `generate` | `model:"scripted"`, `max_tokens:256`, `temperature:Some(0.7)` |
| `rt` | `tokio Runtime` | intermediário | current_thread → `block_on` | driver async |
| `generator` | `ScriptedGenerator` | config | `echo("resposta do gateway, sem key real")` | mesmo do k6 |
| `scripted_generate` | bench | saída | `b.iter` + `black_box(req())` | overhead do nosso lado, sem rede |

**Fluxo:** `ScriptedGenerator::echo` → `generate(black_box(req()))` medido — serialização/agregação/streaming isolada da latência de rede.

---

## Nota de referência: tipos de conversa (moram em `btv-domain::chat`, fora do escopo)

Atravessam quase todo dado dos dois crates; campos serde (`wire`) relevantes:

- `Role` — `{User, Assistant}`, `serde snake_case`.
- `ContentBlock` — `#[serde(tag="type", rename_all="snake_case")]`: `Text{text}`, `ToolUse{id,name,input:Value}`, `ToolResult{tool_use_id,content,is_error}`.
- `ChatMessage{role, content:Vec<ContentBlock>}`; helper `user_text`.
- `ToolSpec{name, description, input_schema:Value}`.
- `StopReason` — `{EndTurn, ToolUse, MaxTokens, Other}`, snake_case.
- `Usage{input_tokens:u64, output_tokens:u64}` (Default).
- `AssistantTurn{content, stop_reason, usage, provider:String}`; métodos `tool_uses() -> Vec<(&str,&str,&Value)>` e `text() -> String`.
- `GenerateRequest{model, system, messages, tools, max_tokens:u32, temperature:Option<f64>}` (NÃO serde — struct interna).
- `ModelTier{Small,Medium,Large}` (Serialize); `compaction_threshold()` = 0.75 (Small) / 0.90 (demais).
