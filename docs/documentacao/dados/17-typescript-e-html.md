# 17 — Dicionário de dados: frontends TypeScript/React (`web/` e `btv-web/`)

Mapa exaustivo dos dados que circulam nos DOIS SPAs em `/home/user/btv`:

- **`web/`** — console dev, servido pelo `btv dashboard` sob `/dev` (`base: './'`, sem roteamento por URL). 21 módulos `api/*`.
- **`btv-web/`** — produto BuildToValue, SPA raiz `/`. 5 módulos `api/*`; Designer sobre a lib agnóstica `@bpmn-react/*` (submodule `vendor/bpmn`).

Ambos falam com o MESMO backend real via proxy `/api → http://127.0.0.1:7878`. Nenhuma API key vive no navegador (fronteira ADR 0001: keys só no processo Rust).

**Taxonomia de Direção:** `entrada` (resposta HTTP / evento SSE / prop / input do usuário) · `saída` (corpo de request / valor exposto por context / render) · `intermediário` (useState/useMemo/reducer/derivado, mesmo descartado) · `estado` (context state / ref persistente) · `config` (const/env de vite/proxy) · `wire` (DTO que espelha contrato do backend — origem Rust/proto anotada).

---

## web/ (console dev · /dev)

### web/index.html
Shell HTML mínimo; monta React em `#root` via `/src/main.tsx`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `lang="pt-br"`, `<title>btv — dashboard</title>` | atributos HTML | config | arquivo → documento | idioma pt-BR (padrão do projeto) |
| `#root` | div | config | HTML → `createRoot` | ponto de montagem do SPA |
| `/favicon.svg` | asset | config | disco → `<link rel=icon>` | ícone da aba |

Fluxo: HTML estático → `main.tsx` monta `<App/>` em `#root`.

### web/vite.config.ts
Config do bundler/testes; define o proxy `/api` e `base` relativo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `base: './'` | string | config | vite → build | assets relativos: funciona montado na raiz (testes) e sob `/dev` (dashboard) |
| `server.proxy['/api']` = `http://127.0.0.1:7878` | string | config | vite dev → backend | toda rota `/api` vai ao `btv dashboard` real (btv-server + router btv-cli mesclado) |
| `test.environment: 'jsdom'`, `exclude` e2e | objeto | config | vite → vitest | separa unit (vitest) de e2e/integração (playwright) |

Fluxo: proxy fixo faz o browser conversar com o backend local sem CORS.

### web/src/styles/themes.ts
Paletas de tema (config de cores) + chaves de localStorage.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ThemeId` (`default`/`veneziana`/`ultramarino`/`marmore`/`afresco`) | union | config | const → AppContext/useTheme | 5 temas renascentistas |
| `THEMES: Record<ThemeId, ThemePalette>` | mapa | config | const → useTheme (CSS vars) | 18 custom properties hex por tema (verbatim README §8.3) |
| `ACCENTS`, `THEME_LIST`, `LITERAL_COLORS` | arrays/const | config | const → seletores/render | accents sobrepõem só `--rust`; literais fora do tema |
| `THEME_STORAGE_KEY='btv_theme'`, `ACCENT_STORAGE_KEY='btv_accent'` | string | config | const → localStorage | chaves de persistência |

Fluxo: consts puras consumidas por `useTheme` (aplica CSS vars) e `AppContext` (persiste).

### web/src/types/domain.ts
DTOs de contrato e tipos de UI compartilhados. Vários marcados `wire`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Persona` (`user`/`admin`) | union | intermediário | UI → AppContext | perfil ativo |
| `UserScreenId` / `AdminScreenId` / `ScreenId` | union | intermediário | nav → AppContext.screen | 8 telas user, 12 admin |
| `NavItem{id,icon,label,hint}` | interface | config | nav.ts → Sidebar | item de menu |
| `AgentProfile`(`build`/`plan`), `ModelTierId`(`small`/`medium`/`large`) | union | estado→saída | Modelo → SendMessageBody | vão ao corpo de `POST .../message` |
| `ModelTier{id,models,label}` | interface | config | models.ts → Modelo | tiers estáticos |
| `LedgerOverrideMark{marked,reason?}` | interface | **wire** | espelha `btv_schemas::ledger::OverrideMark` | override é campo de 1ª classe, nunca inferido no cliente |
| `LedgerEntry{seq,prev_hash,entry_hash,kind,actor,payload:unknown,override?,fake_marker?,ts}` | interface | **wire** | espelha `btv_schemas::ledger::LedgerEntry` | resposta de `GET /api/ledger` serializada direto |
| `ProviderInfo{id,configured}` | interface | **wire** | resposta `GET /api/providers` | `configured` real (mesmo env var de `Gateway::from_env`) |
| `SkillEntry{id,status:'aprovado'|'bloqueado'|'em_analise',detail,source}` | interface | **wire** | resposta `GET /api/skills` (vetter) | `source` = builtin/third-party |
| `PermissionMatrixDecision` (`allow`/`ask`/`deny`) | union | **wire** | matriz/regras | decisão de permissão |
| `PermissionMatrixRow{tool,build,plan}` | interface | **wire** | `GET /api/permissions/matrix` | matriz efetiva por perfil |
| `PermissionRuleRecord{id,profile,tool,scope_prefix?,decision,created_at}` | interface | **wire** | espelha `btv_store::RuleRecord` | override persistido |
| `DesignerNodeKind`, `DesignerNodeParam{k,v}`, `DesignerNode{id,x,y,kind,name,role,color,icon,sub,params[],removable}`, `DesignerEdge{from,to,label?}` | interfaces | intermediário/saída | reducer → `POST /api/designer/workflow` | grafo do Squad Designer (validado contra `squad.workflow.v1`) |

Fluxo: fonte de tipos; os `wire` são o contrato exato do backend Rust.

### web/src/api/client.ts
Cliente HTTP base: `fetchJson<T>` + `ApiError`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ApiError{message,code?}` | classe | entrada→intermediário | corpo `{error,code}` → throw | `code` fallback `http_<status>` / `network_error` |
| `ApiErrorBody{error?,code?}` | interface | **wire** | corpo de erro de toda rota real | Fase 7 Onda 1 |
| `fetchJson<T>(url, init?)` corpo | genérico T | entrada | `fetch` → chamador | `response.text()`; corpo vazio (202/204) → `undefined` sem `.json()` (evita SyntaxError — bug real Onda 15) |

Fluxo: `fetch` → checa `r.ok` → parse tolerante a corpo vazio → `T` ou lança `ApiError`.

### web/src/api/stream.ts
Cliente SSE da sessão de código (`EventSource`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SessionEvent` (union tag `type`) | DTO | **wire** entrada | espelha `btv-cli::web_agent::SessionEvent` (envelope autoral) | variantes abaixo |
| `text_delta{text}` | variante | entrada | SSE → SessionContext | delta de streaming do agente |
| `turn_completed{provider,input_tokens,output_tokens}` | variante | entrada | SSE → SessionContext | fecha turno |
| `tool_started{name,scope}` / `tool_finished{name,ok,summary,diff:DiffLine[]|null}` / `tool_denied{name,scope}` | variantes | entrada | SSE → transcript | execução de ferramenta |
| `permission_requested{request_id,tool,scope}` | variante | entrada | SSE → PendingPermission | gate interativo |
| `done{ledger_verified}` / `error{message}` | variantes | entrada | SSE → busy/lastError | terminal |
| `DiffLine{Context?,Removed?,Added?}` | interface | **wire** | linha de diff da ferramenta | |
| `connectSessionEvents(sessionId,handlers)` | função | config | abre `GET /api/session/:id/events` | `EventSource` reconecta sozinho; servidor reemite snapshot |
| `newSessionId()` | string | intermediário | `crypto.randomUUID()` | id estável por aba (não persiste em reload) |

Fluxo: SSE contínuo → handlers no SessionContext atualizam transcript/pending.

### web/src/api/session.ts
Só tipos de transcript (mocks removidos na Onda 15).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ToolCallStatus` (`running`/`ok`/`error`) | union | intermediário | evento → turn | |
| `TranscriptTurn{id,kind:'user'|'agent'|'tool'|'diff'|'lint',text,toolStatus?}` | interface | intermediário | SessionContext → Sessao | linha do transcript (estado local) |

Fluxo: forma dos turnos que o SessionContext acumula.

### web/src/api/squad.ts
Cliente do squad ao vivo (SSE `SquadEvent` cru, sem DTO espelho).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SquadProposal{agent,confidence,content_json}` | **wire** | entrada | espelha `btv_proto::squad::SquadEvent::Proposal` | `content_json` = JSON cru por agente |
| `SquadConsensus{decision_maker,strength,decision_json,requires_human}` | **wire** | entrada | proto Consensus | |
| `SquadHandoff{phase:0..4,from_agent,to_agent,contract,payload_digest}` | **wire** | entrada | proto Handoff; `HANDOFF_PHASE_LABELS` (i32 cru → rótulo) | |
| `SquadHitl{reason,confidence}` | **wire** | entrada | proto Hitl | pedido de escalonamento |
| `SquadStep{step_id,success,summary}` | **wire** | entrada | proto Step | |
| `SquadChatMessage{author,author_role:'AGENT'|'HUMAN'|'SYSTEM',text,in_reply_to?}` | **wire** | entrada | proto ChatMessage | |
| `SquadEventPayload` (union externally-tagged) / `SquadEventEnvelope{task_id,ts,payload|null}` | **wire** | entrada | serde Serialize de tonic | tag pelo nome da variante Rust |
| `runSquad(task, model?)` → `RunSquadResponse{task_id}` | request/resp | saída→entrada | `POST /api/squad/run` body `{task,model?}` | `model` por-tarefa; omitido = default do pool |
| `resolveHitl(taskId, allow)` | request | saída | `POST /api/squad/:id/hitl` body `{allow}` | resolve gate |
| `postSquadMessage(taskId, text)` | request | saída | `POST /api/squad/:id/message` body `{text}` | 202 sem corpo → `fetch` cru (não `fetchJson`) |
| `emergencyStopSquad(taskId, reason?)` | request | saída | `POST /api/squad/:id/emergency-stop` body `{reason}` | kill-switch, 200 sem corpo |
| `connectSquadEvents(taskId,handlers)` | função | config | `GET /api/squad/:id/events` | tarefa FINITA: fecha no 1º `onConnectionError` |

Fluxo: `runSquad` → `task_id` → SSE `SquadEvent` acumulado; ações (hitl/message/stop) por POST.

### web/src/api/permissions.ts
Matriz build/plan×tool e overrides (router btv-cli).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `fetchMatrix()` → `PermissionMatrixRow[]` | resp | entrada | `GET /api/permissions/matrix` | |
| `listRules()` → `PermissionRuleRecord[]` | resp | entrada | `GET /api/permissions/rules` | |
| `setRule(profile,tool,decision,scopePrefix?)` → `PermissionRuleRecord` | request/resp | saída→entrada | `POST /api/permissions/rules` body `{profile,tool,scope_prefix,decision}` | sem prefix = regra de matriz; com prefix = "sempre" restrito ao escopo |
| `revokeRule(id)` | request | saída | `DELETE /api/permissions/rules/:id` | |

Fluxo: leitura da matriz/regras + gravação/revogação de overrides.

### web/src/api/prompts.ts
Biblioteca de prompts (CRUD em btv-server) + render/generators (sidecar PromptForge).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SavedPrompt{id,name,generator,fields:Record,rendered,tags[],favorite,created_at}` | **wire** | entrada | `GET/POST /api/prompts` | prompt salvo (btv-store) |
| `GeneratorField{name,label,required,placeholder}` | **wire** | entrada | espelha `btv_proto::promptforge::GeneratorField` | |
| `GeneratorInfo{name,category,fields[]}` | **wire** | entrada | espelha `GeneratorInfo` (sidecar real) | |
| `listLibrary(tag?)` → `SavedPrompt[]` | resp | entrada | `GET /api/prompts?tag=` | |
| `SavePromptInput{name,generator,fields,rendered,tags}` / `savePrompt` | request/resp | saída→entrada | `POST /api/prompts` | |
| `toggleFavorite(id)` → `{favorite}` | resp | saída→entrada | `POST /api/prompts/:id/favorite` | |
| `removePrompt(id)` | request | saída | `DELETE /api/prompts/:id` | |
| `listGenerators()` → `GeneratorInfo[]` | resp | entrada | `GET /api/prompt/generators` | via sidecar |
| `renderPrompt(generator,fields)` → string | request/resp | saída→entrada | `POST /api/prompt/render` body `{generator,fields}` → `{prompt}` | extrai `.prompt` |

Fluxo: geradores+biblioteca carregados; render via sidecar; salvar/fav/rm no store.

### web/src/api/ledger.ts
Ledger append-only (leitura + verificação de cadeia).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `getLedger(limit=50,actor?)` → `LedgerEntry[]` | resp | entrada | `GET /api/ledger?limit=&actor=` | filtro por actor resolvido em SQL |
| `VerifyResult{ok,verified,error?}` | **wire** | entrada | `POST /api/ledger/verify` | `ok:false` = cadeia corrompida (não é erro HTTP) |
| `verifyChain()` → `VerifyResult` | resp | entrada | recomputa cadeia inteira | |

Fluxo: lista paginada + verificação sob demanda.

### web/src/api/verify.ts
Pipeline `/verify` em background (start + polling).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `GateTriggered` (`critical_finding`/`verify_fail`/`security_floor`) | union | **wire** | review | gate duro |
| `ValueReview{technical,security,gates_passed,gate_triggered?,reason}` | **wire** | entrada | espelha `btv_schemas::review::ValueReview` | só dims determinísticas (perf/value dependem de agente, não fabricadas) |
| `Finding{tool,severity,message,file?,line?}` / `VerificationStep{name,tool,exit_code,duration_ms,findings[]}` | **wire** | entrada | evidência | |
| `Verdict`(`pass`/`fail`/`skipped`), `VerificationEvidence{run_id,git_sha,steps[],verdict,produced_at}` | **wire** | entrada | espelha `btv_schemas::verification::VerificationEvidence` | |
| `startVerifyRun()` → `VerifyRunStarted{run_id}` | resp | saída→entrada | `POST /api/verify/run` | trata 202 (novo) e 409 (job ativo) igual |
| `VerifyStatus` (union `running`/`done`/`failed`) | **wire** | entrada | `GET /api/verify/:id` (polling) | `failed` = panic capturado no servidor |
| `fetchVerifyStatus(runId)` → `VerifyStatus` | resp | entrada | polling 500ms | |

Fluxo: POST → `run_id` → polling até `done`/`failed`.

### web/src/api/providers.ts
Providers configurados (read-only).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `fetchProviders()` → `ProviderInfo[]` | resp | entrada | `GET /api/providers` | reusa `Gateway::from_env().available()`; degrau é código morto (nota) |

Fluxo: reflete env vars reais do gateway.

### web/src/api/models.ts
Consts de tier/autonomia (sem backend — seletores locais).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `MODEL_TIERS: ModelTier[]` | const | config | → Modelo | small/medium/large + nomes de modelo |
| `primaryModelName(tier)` → string | função | intermediário | tier → nome (split `·`) | vai como `model` ao enviar mensagem |
| `AUTONOMY_LEVELS[]` | const | config | → Modelo | só informativo (didático): o campo `max_autonomy_level` foi REMOVIDO do wire (ADR 0033); nunca foi enviado |

Fluxo: escolha em Modelo → AppContext → corpo de mensagem.

### web/src/api/designer.ts
Salvar grafo do Squad Designer.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `saveWorkflow({nodes,edges})` → `SaveWorkflowResult{seq,workflowId}` | request/resp | saída→entrada | `POST /api/designer/workflow` body `{nodes,edges}` → `{seq,workflow_id}` | valida contra `squad.workflow.v1`, grava no ledger; "salvo e validado" (não aplica ao squad real) |

Fluxo: grafo do reducer → validação+ledger → `seq` real.

### web/src/api/experiments.ts
Relatório A/B sobre telemetria.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ExperimentVerdict` (`significant`/`inconclusive`/`insufficient_data`) | union | **wire** | veredito | teste z, nunca vencedor sem significância |
| `VariantStats{variant,n,successes,rate}` | **wire** | entrada | espelha `btv_schemas::experiment::VariantStats` | |
| `ExperimentReport{experiment,metric,variants[],verdict,winner?,p_value,comparisons,produced_at}` | **wire** | entrada | espelha `ExperimentReport` | `comparisons` = m*(m-1)/2 (Bonferroni) |
| `fetchExperiment(nome)` → `ExperimentReport` | resp | entrada | `GET /api/experiment/:nome` | dados semeados (instrumentação pendente) |

Fluxo: busca por nome → relatório com veredito honesto.

### web/src/api/lsp.ts
Language servers declarados (enumeração, zero probe).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LspServerInfo{id,command,args[]}` | **wire** | entrada | `GET /api/lsp` (`.btv/lsp.toml`) | sempre "declarado, não iniciado" |

Fluxo: lista config, nunca sobe processo.

### web/src/api/mcp.ts
Console MCP (sondagem ao vivo + preview de política).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `McpToolPolicyPreview{build,plan}` / `McpToolInfo{name,description,policy}` | **wire** | entrada | política real (override Onda 2) | |
| `McpServerInfo{id,command,status:'online'|'offline',error?,tools[]}` | **wire** | entrada | `GET /api/mcp` (`.btv/mcp.toml`) | sonda cada servidor de verdade |
| `fetchMcpServers()` → `McpServerInfo[]` | resp | entrada | | |

Fluxo: sonda servidores + calcula política efetiva.

### web/src/api/memory.ts
Mapa de memória do squad + recall TF-IDF.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `MemorySummary{agent,count,latest_decision_json,latest_timestamp,top_confidence}` | **wire** | entrada | espelha `btv_proto::memory::MemorySummary` | sem coluna de esquecimento (nada calcula) |
| `MemoryMatch{id,agent,decision_json,timestamp,score}` | **wire** | entrada | espelha `MemoryMatch` | |
| `fetchMemoryMap(agent?,limit=50)` → `MemorySummary[]` | resp | entrada | `GET /api/memory?agent=&limit=` | |
| `recallMemory(query,k=5)` → `MemoryMatch[]` | request/resp | saída→entrada | `POST /api/memory/recall` body `{query,k}` | recuperação léxica (RAG rotulado, não semântico) |

Fluxo: mapa por agente + busca léxica.

### web/src/api/modelUsage.ts
Uso e custo estimado por modelo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ModelUsageEntry{model,tier,calls,cache_hits,cache_misses,input_tokens,output_tokens,provider?,estimated_cost_usd?}` | **wire** | entrada | `GET /api/models/usage` | custo = tokens reais × preço tabelado; ausente sem preço |
| `ModelUsageResponse{entries[],total_estimated_cost_usd,pricing_as_of}` | **wire** | entrada | | `pricing_as_of` = data de referência (estimativa envelhece) |
| `fetchModelUsage()` → `ModelUsageResponse` | resp | entrada | | |

Fluxo: agrega telemetria + tabela de preços estática.

### web/src/api/onboarding.ts
Doctor (4 checagens) + clipboard.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `DoctorCheck{id:'providers'|'uv'|'docker'|'git',ok,detail}` | **wire** | entrada | `GET /api/doctor` (`doctor_console.rs`) | 4 checagens reais agregadas |
| `fetchDoctor()` → `DoctorCheck[]` | resp | entrada | extrai `.checks` de `{checks}` | |
| `copyToClipboard(text)` | ação | saída | `navigator.clipboard.writeText` | |

Fluxo: doctor real; providers só resumo agregado (nunca key mascarada).

### web/src/api/ratelimit.ts
Tetos de rate limit por tier (read-only).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RateLimitEntry{tier,cap,window_secs}` | **wire** | entrada | `GET /api/ratelimit` | tetos configurados, NÃO uso ao vivo (processo separado) |
| `fetchRateLimits()` → `RateLimitEntry[]` | resp | entrada | | |

Fluxo: tetos por tier; sem `RateLimiter` compartilhado para ler.

### web/src/api/sandbox.ts
Perfil de confinamento Docker + saúde do daemon.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SandboxProfile{image,network_disabled,mem_limit_mb,cpu_quota,timeout_secs,rootfs_readonly,cap_drop_all,no_new_privileges}` | **wire** | entrada | `GET /api/sandbox` | rootfs/caps/priv são consts hardcoded |
| `SandboxInfo{profile,ping}` | **wire** | entrada | `Sandbox::ping()` real | `ping:false` fail-closed sem daemon |
| `fetchSandbox()` → `SandboxInfo` | resp | entrada | | |

Fluxo: perfil + ping; tela read-only.

### web/src/api/skills.ts
Status do vetter (read-only).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `fetchSkills()` → `SkillEntry[]` | resp | entrada | `GET /api/skills` (`btv-verify::vetter`) | status decidido pelo vetter (fail-closed); sem fallback mock |

Fluxo: lista skills com status real do vetting.

### web/src/api/telemetry.ts
Único módulo com `fetch` próprio (não `fetchJson`); fala com btv-server.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Summary{total_events,cache_hit_rate:number|null,by_name:Record<string,number>}` | **wire** | entrada | `GET /api/summary` | agregados de telemetria |
| `EventRow{ts,name,session_id,props:unknown}` | **wire** | entrada | `GET /api/events?limit=` | evento cru |
| `getSummary()` / `getEvents(limit=50)` | resp | entrada | `fetch` direto (lança `Error`, não `ApiError`) | |

Fluxo: summary + eventos recentes, polling na tela Telemetria.

### web/src/state/SessionContext.tsx
Estado da sessão de código, vivo para a aba (montado acima da troca de tela).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `sessionId` | string | estado | `newSessionId()` (useState init) | estável por aba |
| `transcript: TranscriptTurn[]` | estado | estado | eventos SSE → render Sessao | acumulado por reducer de eventos |
| `streamingText` / `streamingRef` | string / ref | intermediário/estado | `text_delta` → UI | ref acumula deltas, vira turno em `turn_completed` |
| `pending: PendingPermission{requestId,tool,scope}|null` | estado | estado→saída | `permission_requested` → Permissao | sobrevive à navegação |
| `busy` / `lastError` / `ledgerVerified` | estado | estado | eventos `done`/`error` | `ledgerVerified` = contagem do último `verify()` |
| `sendMessage(message, opts?{model,agent})` | ação | saída | `POST /api/session/:id/message` body `{message,model?,agent?}` | insere turno `user` otimista; model/agent do AppContext |
| `resolvePermission(allow)` | ação | saída | `POST /api/session/:id/permission` body `{request_id,allow}` | limpa `pending` |
| `nextTurnId(prefix)`, `turnCounter` | contador módulo | intermediário | → id de turno | monotônico |

Fluxo: SSE alimenta transcript/pending; envio e resolução por POST.

### web/src/state/AppContext.tsx
Estado global de navegação/tema/modelo (reducer + persistência).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AppState{persona,screen,theme,accent,modelTier,agentProfile}` | estado | estado | reducer → Shell/telas | modelTier default `large`, agentProfile `build` |
| `AppAction` (SET_PERSONA/SCREEN/THEME/ACCENT/MODEL_TIER/AGENT_PROFILE) | union | saída | dispatch → reducer | |
| `readPersisted()` → `{theme,accent}` | função | entrada | localStorage → initState | degrada silenciosamente sem localStorage |
| SET_THEME/SET_ACCENT | efeito | saída | reducer → localStorage | grava chave; SET_PERSONA ajusta screen se não pertence |

Fluxo: reducer central; tema/accent persistem; modelTier/agentProfile lidos por Sessao/Modelo.

### web/src/components/primitives/Toast.tsx
Context de toasts efêmeros.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ToastItem{id,kind:'success'|'error',message}` | interface | intermediário | push → render | auto-remove em 4s |
| `items` / `idRef` | estado / ref | estado | push → lista fixa canto | id incremental |
| `push(kind,message)` | ação | saída | telas → toast | exposto pelo context |

Fluxo: `useToast().push` → item temporário no canto inferior.

### web/src/hooks/useAsyncAction.ts · usePolling.ts
Máquinas de estado assíncrono reutilizadas.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AsyncState<T>` (`idle`/`loading`/`success{data}`/`error{error}`) | union | intermediário | fn → AsyncStatus | shape comum |
| `useAsyncAction(fn)` → `{state,run,reset}` | hook | intermediário | chamada → estado | rethrow no erro |
| `usePolling(fn,intervalMs)` → `AsyncState<T>` | hook | intermediário | `setInterval` → estado | só 1º tick fica `loading` (evita flash) |

Fluxo: envelopam chamadas api em estado de UI.

### web/src/lib/nav.ts · screenMeta.ts · screenComponents.tsx
Config de navegação e metadados de tela.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `USER_NAV` / `ADMIN_NAV` / `NAV_BY_PERSONA` / `DEFAULT_SCREEN` | const | config | → Sidebar/AppContext | 8 user, 12 admin |
| `screenBelongsToPersona(persona,screen)` | função | intermediário | SET_PERSONA | mantém/reseta screen |
| `SCREEN_META: Record<ScreenId,ScreenMeta{kicker,title,note,accent,chromeIcon,chromeRight}>` | const | config | → Shell/WindowChrome | cabeçalho por tela |
| `SCREEN_COMPONENTS: Record<ScreenId,ComponentType>` | const | config | → Shell | mapa tela→componente |

Fluxo: consts puras roteando persona/screen para UI.

### web/src/state/useTheme.ts
Aplica CSS vars do tema em `#btv-root`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `useTheme(rootRef,theme,accent)` | efeito | saída | `THEMES[theme]` → `el.style.setProperty` | 18 vars; `accent` sobrepõe `--rust` |

Fluxo: troca de theme/accent → re-aplica custom properties.

### web/src/components/screens/user/Designer/templates.ts
Constantes do board e templates de nó (pesos fiéis a `consensus.py`/`hitl.py`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `BOARD_WIDTH/HEIGHT`, `CARD_W/H`, `PILL_W/H` | const | config | → geometry/reducer | dimensões |
| `TEMPLATES: Record<string,Template>` (Architect/Developer/Auditor/Designer/Ops/Consenso/Gate HITL) | const | config | Palette → ADD_NODE | params = pesos reais (não inventados) |
| `TEMPLATE_KEYS`, `initialNodes()`, `initialEdges()` | const/função | intermediário | → initDesignerState | grafo inicial (8 nós, 9 arestas) |

Fluxo: catálogo de nós e grafo semente do Designer.

### web/src/components/screens/user/Designer/reducer.ts
Reducer do editor de grafo (estado local descartável).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `DesignerState{nodes,edges,mode,selectedNode,pendingConnect,dragId,grabDX/DY,addCount,wfSaved,lastSavedSeq}` | estado | intermediário | reducer → Designer/Board | `lastSavedSeq` = `seq` real do ledger pós-save |
| `DesignerAction` (SET_MODE/DRAG_*/CONNECT_CLICK/ADD_NODE/REMOVE_NODE/SELECT_NODE/RESET/MARK_SAVED) | union | saída | dispatch → reducer | |
| `clamp(x,y,kind)` | função | intermediário | DRAG_MOVE | prende ao board |
| MARK_SAVED{seq} | ação | entrada→estado | `saveWorkflow.seq` → `lastSavedSeq` | qualquer edição zera `wfSaved` |

Fluxo: interações → mutações de grafo; salvar marca `wfSaved`+`seq`.

### web/src/components/screens/user/Designer/geometry.ts
Cálculo puro de arestas (derivado, sem estado).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Box`/`Point`, `nodeBox(n)`, `intersectRectBorder(box,toward)` | funções | intermediário | nós → pontos de borda | escala por max(|dx|/hw,|dy|/hh) |
| `ComputedEdge{key,x1,y1,x2,y2,amber,label?,labelX?,labelY?}` | interface | intermediário | `computeEdges(nodes,edges)` → EdgesOverlay | `amber` se toca role/id `hitl` |

Fluxo: grafo → geometria SVG das setas.

### web/src/components/screens/user/Designer/Designer.tsx
Monta o editor; consome `wfSaved`/`lastSavedSeq`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `useReducer(designerReducer, initDesignerState)` | estado | estado | → Toolbar/Board/Palette/PropertiesPanel | |
| banner `wfSaved`/`lastSavedSeq` | render | saída | estado → UI | "salvo → ledger seq N → aplicação futura" |

Fluxo: reducer central do Designer distribuído aos filhos.

### web/ telas de usuário (dados consumidos/emitidos)

| Arquivo | Dados que consome (entrada) | Dados que emite (saída) |
|---|---|---|
| `screens/user/Sessao.tsx` | SessionContext (transcript/streamingText/busy/ledgerVerified/lastError), AppState (modelTier/agentProfile), `fetchMatrix`, `fetchProviders`; input local `input` | `sendMessage(text,{model,agent})`; navegação (SET_SCREEN modelo/skills); `activeProvider` derivado (1º configurado) |
| `screens/user/Squad.tsx` | `runSquad`+SSE `SquadEventEnvelope[]` (estado `events`); memos derivados: `proposals` (dedup por agente), `consensus` (último), `executionLog` (Handoff/Step), `hitlEvents`+`pendingHitl`, `chat` (Chat filtrado), `errorMessage`; `slotHint` (timeout 5s, fila capacidade 1); input `task`/`chatDraft` | `resolveHitl(allow)`, `postSquadMessage`, `emergencyStopSquad`; toasts |
| `screens/user/Prompts.tsx` | `listGenerators`+`listLibrary` (Promise.all), estado `library`/`activeGenerator`/`fieldValues`/`preview`; `renderPrompt` | `savePrompt(input)`, `toggleFavorite`, `removePrompt`; `handleUseSaved` reidrata campos; clipboard |
| `screens/user/Modelo.tsx` | AppState (modelTier/agentProfile), `MODEL_TIERS`/`AUTONOMY_LEVELS` | dispatch SET_MODEL_TIER / SET_AGENT_PROFILE (só AppContext, aplica na próxima mensagem); autonomia é informativa |
| `screens/user/Permissao.tsx` | SessionContext.pending, AppState.agentProfile; atalhos `s`/`n` | `resolvePermission(allow)`; `handleAlways` → `setRule(profile,tool,'allow',scope)` (grava antes de resolver) |
| `screens/user/Onboarding.tsx` | `fetchDoctor` (checks providers/uv/docker/git) | `copyToClipboard(code)`; dispatch SET_SCREEN sessao |
| `screens/user/Sugestoes.tsx` | `PROPOSALS[]` const (roadmap, alguns `delivered`) | dispatch SET_SCREEN para tela relacionada |

### web/ telas admin (dados consumidos/emitidos)

| Arquivo | Dados que consome (entrada) | Dados que emite (saída) |
|---|---|---|
| `screens/admin/Telemetria.tsx` | `usePolling(loadAll,5000)`: `getSummary`+`getEvents(50)`; derivados `rate`, `bars` (by_name ordenado) | — (read-only) |
| `screens/admin/Ledger.tsx` | `getLedger(50,actor?)` → `entries`; `actors` derivado (só busca sem filtro); `verifyChain` | `handleActorFilter` refaz busca; `verify.run()`; `selected` (detalhe JSON) |
| `screens/admin/Verify.tsx` | `startVerifyRun` → `activeRunId`; `VerifyPoller` (usePolling 500ms) → `progress`/`evidence`/`review`; `expandedStep` | `handleRun`; toasts por veredito; render de steps/findings e Gauge de segurança |
| `screens/admin/Experimentos.tsx` | input `nome`; `fetchExperiment(nome)` → `ExperimentReport` | `handleBuscar`; render StatTiles/Table/veredito |
| `screens/admin/Mcp.tsx` | `fetchMcpServers` → `McpServerInfo[]` (status+tools+policy) | `state.run()` (atualizar/sondar) |
| `screens/admin/Memoria.tsx` | `fetchMemoryMap(agent?)` → `map`; `recallMemory(query,5)` → matches; `agentFilter`/`query` | `handleAgentFilter`, `handleRecall` |
| `screens/admin/Modelos.tsx` | `fetchModelUsage` → entries; derivados `totalCalls`/`top`/`hitRate`/`fmtUsd` | — (read-only) |
| `screens/admin/Providers.tsx` | `fetchProviders`+`fetchRateLimits` | — (read-only) |
| `screens/admin/RateLimits.tsx` | `fetchRateLimits` → limits | — (read-only) |
| `screens/admin/Sandbox.tsx` | `fetchSandbox` (profile+ping); `fetchSkills` filtrado `third-party` | — (read-only) |
| `screens/admin/Lsp.tsx` | `fetchLspServers` → servers | — (read-only) |
| `screens/admin/Skills.tsx` | `fetchSkills`; `loadPermissions` (`fetchMatrix`+`listRules`); `pendingChange`/`NEXT_DECISION` (allow→ask→deny→allow) | `setRule(profile,tool,to)`, `revokeRule(id)`; modal de confirmação |

### web/src/components/shell/{Shell,Sidebar,Topbar}.tsx
Layout; consomem AppState.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `screen`/`theme`/`accent` (Shell) | estado | entrada | AppState → useTheme + render | `stageClass` = `surf`/`term` por `ADMIN_SURFACE_SCREENS` |
| `persona`/`screen` (Sidebar/Topbar) | estado | entrada→saída | AppState → nav; dispatch SET_SCREEN/SET_PERSONA | render de menu por persona |

Fluxo: Shell escolhe componente/meta por `screen`; Sidebar/Topbar navegam.

---

## btv-web/ (produto BuildToValue · raiz /)

### btv-web/index.html
Shell HTML; carrega fontes do handoff (degrada offline).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `<title>BuildToValue</title>`, `lang="pt-br"`, `#root` | HTML | config | → main.tsx | |
| `<link>` Google Fonts (Bricolage/Instrument Sans/Spline Sans Mono) | asset externo | config | rede → CSS | offline degrada p/ fallbacks de global.css (local-first) |

Fluxo: HTML → monta SPA raiz.

### btv-web/vite.config.ts
Proxy `/api`, aliases da lib bpmn (submodule), dedupe React.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `server.proxy['/api']` = `http://127.0.0.1:7878` | string | config | vite → backend | mesmo backend do console |
| `resolve.dedupe: ['react','react-dom']` | array | config | vite → bundle | força cópia única (React 19) — evita React #525 |
| `resolve.alias` `@bpmn-react/{core,react,registry,styles.css}` → `vendor/bpmn/packages/*/dist/esm` | array | config | submodule dist ESM → import | lib consumida sem publicar no npm |
| `test`/`exclude` | objeto | config | vite → vitest | separa unit de e2e |

Fluxo: aliases resolvem a lib bpmn pinada; dedupe une React.

### btv-web/src/types/domain.ts
Tipos de UI do produto (espelhos ficam em `api/`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Persona`(`user`/`admin`), `ScreenId` (U1..U7, A1..A6) | union | intermediário | UI → AppContext | `vivo` (U3) sem item fixo de menu |
| `NavItem{id,icon,label,hint}` | interface | config | nav → Sidebar | |
| `ActiveSquadInfo{nome,cor,status:'em produção'|'aguardando você'|'concluída',gateAberto}` | interface | estado→saída | SquadRunContext → Shell/Topbar/Sidebar | recorte da squad ativa que o shell exibe |

Fluxo: forma dos dados de navegação e chip de squad ativa.

### btv-web/src/api/client.ts
Mesmo `fetchJson`/`ApiError` do console (com tratamento 202/204).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ApiError{message,code?}`, `ApiErrorBody{error?,code?}` | classe/interface | entrada | corpo de erro → throw | idêntico ao `web/` |
| `fetchJson<T>(url,init?)` | genérico | entrada | fetch → T | corpo vazio → `undefined` |

Fluxo: cliente HTTP base compartilhado em espírito com o console.

### btv-web/src/api/squad.ts
Idêntico ao `web/api/squad.ts` EXCETO `runSquad(task)` sem `model`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SquadProposal/Consensus/Handoff/Hitl/Step/ChatMessage`, `SquadEventPayload`, `SquadEventEnvelope{task_id,ts,payload|null}` | **wire** | entrada | espelha `btv_proto::squad::SquadEvent` | mesma união externally-tagged do console |
| `HANDOFF_PHASE_LABELS` | const | intermediário | i32 phase → rótulo | |
| `runSquad(task)` → `{task_id}` | request/resp | saída→entrada | `POST /api/squad/run` body `{task}` | SEM `model` (diferença vs console) — usado por ▶ Testar do Designer |
| `resolveHitl`, `postSquadMessage`, `emergencyStopSquad`, `connectSquadEvents` | funções | saída/config | mesmas rotas `/api/squad/:id/*` | |

Fluxo: motor cru do squad (compartilhado com `/api/btv/*`).

### btv-web/src/api/btv.ts
Rotas do PRODUTO (`btv-cli::btv_agent`): ativação, runs, gates, entregas, personas.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RespostaBriefing{label,resposta}` | interface | saída | Wizard → payload | resposta de pergunta |
| `AtivarSquadPayload{template_id,nome?,briefing[],refs[],papeis_off:number[]}` | request | saída | `POST /api/btv/squads` | monta briefing + papéis desligados |
| `AtivarSquadResponse{task_id,run_id}` | resp | entrada | | run persistido + task do motor |
| `ativarSquad(payload)` | função | saída→entrada | | |
| `BtvRun{id,task_id,template_id,template_versao,nome,briefing_json,papeis_json,status:'ativa'|'concluida'|'encerrada'|'erro',created_ts,updated_ts}` | **wire** | entrada | espelha `btv_store::BtvRun` (`GET /api/btv/squads`) | `papeis_json` reparseado em `abrirRun` |
| `listRuns()` → `BtvRun[]` | resp | entrada | | |
| `aprovarGate(taskId,etapa)` | request | saída | `POST /api/btv/squads/:id/gate` body `{etapa}` | ledger `btv.gate_approved` |
| `pedirAjuste(taskId,instrucao,etapa)` | request | saída | `POST /api/btv/squads/:id/ajuste` body `{instrucao,etapa}` | instrução vira contexto do cockpit (`btv.adjust_requested`) |
| `BtvDeliverable{id,run_id,task_id,template_id,nome,path,formato,versao,trilha,created_ts}` | **wire** | entrada | espelha `btv_store::BtvDeliverable` (`GET /api/btv/deliverables`) | artefato real gravado por ferramenta |
| `listDeliverables()` → `BtvDeliverable[]` | resp | entrada | | |
| `deliverableDownloadUrl(id)` | string | saída | `/api/btv/deliverables/:id/download` | href de export |
| `PersonaView{papel,prompt,padrao,editado}`, `CustomPersona{id,template_id,nome,prompt}`, `PersonasResponse{template_id,personas[],proprias[]}` | **wire** | entrada | `GET /api/btv/personas/:tid` | `prompt` = efetivo (override ?? padrão) |
| `setPersonaOverride(tid,papel,prompt)` (PUT), `restorePersona`/`restoreAllPersonas` (DELETE), `createCustomPersona`→id (POST), `updateCustomPersona` (PUT), `deleteCustomPersona` (DELETE) | requests | saída | `/api/btv/personas/:tid[/...]` | overrides REAIS (entram no hash de procedência) |

Fluxo: ativação monta briefing → run; gates/ajuste = HITL real; personas = overrides reais.

### btv-web/src/api/templates.ts
Os 12 modelos embutidos (`squad-template.v1`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `CategoriaSquad` (`conteudo`/`analise`/`criativa`/`operacoes`) | union | **wire** | filtro galeria | |
| `FormatoEntrega{nome,binario}` | **wire** | entrada | | `binario:true` = export direto indisponível (sem conversor) |
| `PerguntaBriefing{label,placeholder}` | **wire** | entrada | → Wizard step 0 | |
| `SquadTemplate{id,nome,categoria,cor,onda:1|2|3,versao,publicado,descricao,papeis[],formatos[],perguntas[],gates[]}` | **wire** | entrada | espelha `btv_schemas::squad_template::SquadTemplate` | servido de 12 JSONs embutidos |
| `fetchTemplates()` → `SquadTemplate[]` | resp | entrada | `GET /api/btv/templates` | |

Fluxo: catálogo único carregado uma vez (TemplatesContext).

### btv-web/src/api/admin.ts
Clients de Administração A1–A6 (rotas reais + btv novas).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `TelemetrySummary{total_events,by_name,cache_hit_rate|null}` / `fetchSummary` | **wire**/resp | entrada | `GET /api/summary` | A1 |
| `ModelUsageEntry{model,tier,calls,input_tokens,output_tokens,provider?,estimated_cost_usd?}`, `ModelUsageResponse{entries[],total_estimated_cost_usd,pricing_as_of}` / `fetchModelUsage` | **wire**/resp | entrada | `GET /api/models/usage` | A1 custo estimado |
| `LedgerEntry{seq,prev_hash,entry_hash,kind,actor,payload:Record,ts}` / `fetchLedger(40)` / `verifyLedger`→`{ok,verified}` | **wire**/resp | entrada | `GET /api/ledger`, `POST /api/ledger/verify` | A2 (payload tipado como Record, sem override/fake_marker) |
| `ProviderInfo{id,configured}` / `fetchProviders`; `RateLimitEntry{tier,cap,window_secs}` / `fetchRateLimits` | **wire**/resp | entrada | `GET /api/providers`, `/api/ratelimit` | A3 |
| `Decision`(`allow`/`ask`/`deny`), `MatrixRow{tool,build,plan}` / `fetchMatrix`; `RuleRecord{id,profile,tool,scope_prefix?,decision,created_at}` / `fetchRules`; `setRule(profile,tool,decision)` (POST), `revokeRule(id)` (DELETE) | **wire**/requests | entrada/saída | `/api/permissions/*` | A4 (matriz efetiva `btv_core::{BUILD,PLAN}` + overrides) |
| `TemplatePub{template_id,publicado}` / `fetchPublicacao`; `setPublicacao(tid,publicado)` (POST) | **wire**/request | entrada/saída | `/api/btv/templates[/:id]/publicacao` | A5 |
| `BtvUser{id,nome,email,papel,ativo,has_pin}` / `fetchUsers`; `createUser(nome,email,papel,pin?)`→id, `setUserAtivo(id,ativo)`, `deleteUser(id)`, `setUserPin(id,pin)`, `PinReason`(`no_pin`/`ok`/`wrong`), `verifyUserPin(id,pin)`→`{ok,reason}` | **wire**/requests | entrada/saída | `/api/btv/users[/...]` | A6 perfis locais; PIN hash nunca exposto |

Fluxo: 6 telas admin sobre rotas reais; custo/uso ao vivo nunca fabricados (notas).

### btv-web/src/state/TemplatesContext.tsx
Os 12 modelos carregados uma vez, compartilhados.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `TemplatesState` (`loading`/`error{error}`/`ready{templates[],byId:Map}`) | estado | estado | `fetchTemplates` → galeria/wizard/personas/admin | `byId` derivado (Map id→template) |
| `raw{templates?,error?}` | estado | intermediário | fetch → useMemo | cancelamento em unmount |

Fluxo: 1 fetch → estado compartilhado por todas as telas que usam templates.

### btv-web/src/state/AppContext.tsx
Navegação + persona + accent + squad ativa + wizard.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AppState{persona,screen,accent:string|null,squad:ActiveSquadInfo|null,wizardTemplateId:string|null}` | estado | estado | reducer → Shell/telas | |
| `AppAction` (SET_PERSONA/SCREEN/ACCENT/SQUAD/OPEN_WIZARD/CLOSE_WIZARD) | union | saída | dispatch → reducer | |
| `BRAND_SWATCHES` | const | config | → GearDrawer | 5 cores de marca |
| `ACCENT_STORAGE_KEY='btv_accent'`, `readPersistedAccent()` | const/função | entrada | localStorage → initState | |
| SET_PERSONA lógica | efeito | intermediário | squad? → 'vivo' | user com squad cai em Ao vivo; SET_SQUAD null saindo de 'vivo' volta à galeria |
| `appReducer`/`appInitState` (export) | função | — | usados em teste | |

Fluxo: reducer central; squad ativa e wizard governam navegação contextual.

### btv-web/src/state/SquadRunContext.tsx
Estado completo da execução viva (SSE + esteira + gate + cockpit). Núcleo do produto.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RunState{template,nome,etapas:Etapa[],taskId,teste,events:SquadEventEnvelope[],acoes:AcaoLocal[],streamEnded}` | estado | estado | ativação/SSE → Vivo | `events` acumula SSE; `acoes` = ações locais do usuário |
| `view: EsteiraView|null` | derivado | saída | `esteiraFromEvents(etapas,events,acoes,streamEnded)` (useMemo) | posição da esteira |
| `feed: FeedItem[]` | derivado | saída | `feedFromEvents(events)` | atividade recente-primeiro |
| `chat: (SquadChatMessage&{ts})[]` | derivado | saída | events Chat + `hhmm(ts)` | autor REAL do agente do motor |
| `conectar(taskId)` | função | config | `connectSquadEvents` → setRun events | append por task; `disconnectRef` (ref persistente) |
| `ativar(template,payload)` | ação | saída→entrada | `ativarSquad` → `task_id`; `makeEtapas` | cria RunState, conecta, navega 'vivo', fecha wizard |
| `abrirRun(btvRun,template)` | ação | entrada→estado | reparseia `papeis_json` → papeisOff; reconecta SSE | reemite snapshot |
| `ativarTeste(nome,etapas,descricao)` | ação | saída→entrada | `runSquad(descricao)` (motor cru) | `templateStub` sintético `teste:true` |
| `aprovar()` | ação | saída | `aprovarGate(taskId,etapa)` → `acoes:[gate_aprovado]` | etapa = nome da etapa atual; falha → `finalizarSessaoObsoleta` |
| `ajustar(instrucao)` | ação | saída | `pedirAjuste(taskId,instrucao,etapa)` → `acoes:[ajuste]` | gate expira ~5min (ADR 0017) |
| `enviarChat(texto)` | ação | saída | `postSquadMessage` | fala volta pelo stream |
| `encerrar()` | ação | saída | `emergencyStopSquad` → setRun(null) | |
| efeito `SET_SQUAD` | efeito | saída | run+view → AppContext.squad{nome,cor,status,gateAberto} | reflete chip/sidebar |
| `finalizarSessaoObsoleta(msg)` | função | intermediário | erro de gate → alert + limpa | gate obsoleto (task inexistente) |

Fluxo: `ativar/abrirRun/ativarTeste` cria run → SSE alimenta `events` → memos (view/feed/chat) → Vivo; gate/cockpit = HITL real.

### btv-web/src/lib/esteira.ts
Funções puras que mapeiam eventos reais → esteira de apresentação (honestidade por construção).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Etapa{nome,papel,gate?}` | interface | intermediário | template → esteira | |
| `makeEtapas(template,papeisOff)` → `Etapa[]` | função | intermediário | papéis ligados `p(i)=on[min(i,len-1)]` | 8 etapas fixas (Briefing→Exportação); gates em Rascunho/Entrega |
| `AcaoLocal{kind:'gate_aprovado'|'ajuste',afterEventIndex}` | interface | estado | ação usuário → esteira | ordenada vs eventos por índice |
| `EsteiraView{idx,gateOpen,done,erro:string|null,inferida}` | interface | saída | esteira → Vivo/Minhas | `inferida` = posição deduzida (rotulada na UI) |
| `esteiraFromEvents(etapas,events,acoes,streamEnded)` → `EsteiraView` | função pura | entrada→saída | eventos+ações → posição | ver mapeamento abaixo |
| `HANDOFF_LABEL` | const | intermediário | phase → texto feed | |
| `FeedItem{ts,txt}`, `feedFromEvents(events)` → `FeedItem[]` | função | entrada→saída | eventos → feed (reverse) | mostra agente REAL do motor (não papel do template) |
| `AtividadeAtual{agente,desde}`, `atividadeAtual(events)` | função | entrada→saída | último Handoff→agente(≠orchestrator) ou Proposal | torna congelada visível |

Mapeamento de `esteiraFromEvents` (idx inicial = 1 após briefing; nunca regride EXCETO ajuste):
- **`Error`** → `erro` setado, `gateOpen=false`, congela.
- **`Hitl`** → abre o próximo gate ainda não passado (`gateIdxs[min(gatesPassados,...)]` se ≥ idx); `inferida=false` (sinal direto).
- **`Consensus`** → `avancar(2,false)` (planejamento decidido → produção).
- **`Step` `final_validation`+success** → avança para depois de "Validação" (`inferida=false`); outro Step success → `avancar(2,true)`.
- **ação `gate_aprovado`** (com gate aberto) → `gatesPassados+1`, fecha gate, `avancar(idx+1,true)` (inferido: orquestrador não emite "gate resolvido").
- **ação `ajuste`** → fecha gate, `idx=max(1,idx-2)`, `inferida=true` (única regressão).
- **gate aberto + Step/Consensus chegando** → fecha por inferência (replay de snapshot pós-reload).
- **`Proposal`/`Handoff`/`Chat`** → informativos, não movem posição.
- fim de stream sem erro/gate → `done=true`, `idx=etapas.length`.

Fluxo: eventos reais do orquestrador → posição+feed+atividade, com o que é inferido rotulado.

### btv-web/src/lib/entregas.ts · time.ts · nav.ts · screenMeta.ts
Utilitários puros.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `runSemArtefatoReal(status,numEntregas)` → bool | função | intermediário | run → aviso | `concluida` + 0 entregas = narrou sem gravar arquivo |
| `hhmm(ts)` → `HH:MM` | função | intermediário | ISO → feed/chat | regex `T(dd):(dd)` |
| `USER_NAV`(5)/`ADMIN_NAV`(6)/`NAV_BY_PERSONA`/`DEFAULT_SCREEN` | const | config | → Sidebar/AppContext | `screenBelongsToPersona`: `vivo` é do user mas sem item |
| `SCREEN_META: Record<ScreenId,{kicker,title,note,accent}>` | const | config | → Shell | `vivo` usa cor da squad em runtime |

Fluxo: helpers de formatação, navegação e cabeçalho.

### btv-web/src/state/useBrand.ts
Aplica `--brand`/`--brandink` no root.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `useBrand(rootRef,accent)` | efeito | saída | accent → `el.style.setProperty('--brand'/'--brandink')` | `null` remove (volta ao padrão de global.css) |

Fluxo: cor do GearDrawer → CSS var na hora.

### btv-web/src/designer/btvPlugin.tsx
Plugin de domínio do produto sobre a lib bpmn agnóstica (config do vocabulário).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `BLOCO_META: Record<string,{cor,icon,label}>` | const | config | → shapes/esteira | 10 blocos (squad:role/tool/service/data/approval/output + gateways/eventos) |
| `NODE_TYPES: NodeTypeDefinition[]` | const | config | → registry | mapeia a tags BPMN 2.0 (userTask/serviceTask/manualTask/sendTask/dataObjectReference) |
| `EDGE_STYLES: Record<string,EdgeStyle>` | const | config | → render de setas | sequenceFlow/sim/nao/dados (cores por finalidade) |
| `cardShape(tipo)` | função (SVG) | render | node → `<g>` | borda dupla (service), cilindro (data), gate terracota |
| `btvDesignerPlugin: BpmnPlugin{id,nodeTypes,shapes,edgeStyles,paletteGroups,paletteItems}` | objeto | config | → BpmnEditor/registryComDominio | a lib nunca menciona BTV |

Fluxo: vocabulário BuildToValue injetado na lib bpmn genérica.

### btv-web/src/designer/bases.ts
Diagramas semente para o Designer.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `BlocoSpec{tipo,nome,x,y,props?}`, `montar(nome,blocos,cadeia)` → `BpmnDiagram` | funções | intermediário | specs → diagram (createNode/createEdge) | encadeia sequencialmente |
| `baseInicial()` (5 blocos/4 setas), `baseVazia()`, `baseDoModelo(template)` | funções | intermediário | → Designer.setDiagram | `baseDoModelo`: Início→papéis zigue-zague→gates→exportador→Fim |

Fluxo: fornecem o diagrama inicial editável.

### btv-web/src/designer/flow.ts
Travessia do grafo → esteira/descrição (para ▶ Testar e preview).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ordemDoFluxo(diagram)` → `BpmnNode[]` | função | intermediário | diagram → ordem | começa no Início sem entrada; segue setas ≠"nao"; evita ciclos; órfãos por x |
| `etapasDoFluxo(diagram)` → `Etapa[]` | função | intermediário | ordem → etapas | tipo de nó → papel (role/tool→ferramenta/service→API/data→base/approval→gate) |
| `descricaoDoFluxo(nome,diagram)` → string | função | saída | ordem → texto | plano de trabalho REAL passado ao motor (`runSquad`), inclui prompt/endpoint/fonte por nó |

Fluxo: grafo desenhado → etapas da esteira e descrição executável.

### btv-web/ telas de usuário

| Arquivo | Dados que consome (entrada) | Dados que emite (saída) |
|---|---|---|
| `screens/user/Inicio.tsx` (U1 galeria) | `useTemplates` (12 modelos), filtro local `cat` (`CategoriaSquad|todas`) | dispatch OPEN_WIZARD{templateId} |
| `components/wizard/Wizard.tsx` (U2) | `template` (do TemplatesContext via `wizardTemplateId`); estado local `step`/`papeisOff`/`refs:RefItem[]`/`answers[]`/`ativando`/`erroAtivacao` | `ativar(template,{briefing:[{label,resposta}],refs:string[],papeis_off:number[]})` → ativação real; `PAPEL_DESCS` const exportada |
| `screens/user/Vivo.tsx` (U3) | SquadRunContext (run/view/feed/chat), `atividadeAtual(events)`, `listDeliverables` (conta `artefatosDaTask` por task); `filaHint` (timeout 5s, capacidade 1) | `aprovar()`, `ajustar(txt)`, `enviarChat(v)`, `encerrar()`; render esteira/gate/cockpit/feed; aviso "sem artefato real" se done+0 |
| `screens/user/Biblioteca.tsx` (U4) | `listDeliverables` via `useAsyncAction` → `AsyncStatus` (F1); `useTemplates.byId` (cor/formatos); agrupa por `template_id` | href `deliverableDownloadUrl(id)`; `emBreve` = binário sem conversor (png/midi) desabilita export |
| `screens/user/Minhas.tsx` (U6) | `carregarMinhas` (`Promise.all[listRuns, listDeliverables]`) via `useAsyncAction` → `AsyncStatus` (idle/loading/error/success unificados, F1); SquadRunContext (liveRun/view); derivados `status`/`pct`/`isGate`/`numEntregas`/`semArtefato` | ações por status: SET_SCREEN vivo/biblioteca, `abrirRun(r,template)` (reconectar), OPEN_WIZARD (reativar); tokens de feedback (`--ok-bg`/`--err-bg`) no lugar de hexes |
| `screens/user/Personas.tsx` (U7) | `fetchPersonas(templateId)` → `data{personas,proprias}`; `useTemplates`; estado `templateId` | `setPersonaOverride`(onBlur), `restorePersona`/`restoreAllPersonas`, `createCustomPersona`/`updateCustomPersona`/`deleteCustomPersona`; recarrega após cada |
| `screens/user/Designer.tsx` (U5) | `useTemplates`, estado `diagram:BpmnDiagram`/`nome`/`base`/`audit:AuditItem[]`/`salvo`/`erro`; memos `etapas`/`ordem`; `AuditLedger`+`VersionRegistry` (refs da lib) | `POST /api/btv/designer/flows` body `{nome,diagram,versao_semantica,snapshot_hash,audit_head,audit_len}` → `{seq,diagram_sha256}` (salvar+ledger `btv.flow_saved`); `ativarTeste(nome,etapas,descricao)` (▶ Testar); `registrar()` alimenta AuditLedger hash-encadeado |

### btv-web/ telas admin

| Arquivo | Dados que consome (entrada) | Dados que emite (saída) |
|---|---|---|
| `screens/admin/Telemetria.tsx` (A1) | `fetchSummary`+`listRuns`+`listDeliverables`+`fetchModelUsage` (Promise.all); derivados `ativas`/`porTemplate`/custo | — (read-only; custo = estimativa, nota) |
| `screens/admin/Ledger.tsx` (A2) | `fetchLedger(40)` → entries; `useTemplates.byId` (`squadDe`); `verificado` | `verifyLedger()` (verificar integridade); `humano` derivado (kind gate ou actor regex) |
| `screens/admin/Providers.tsx` (A3) | `fetchProviders`+`fetchRateLimits` | — (read-only; uso ao vivo não fabricado, NotaHonesta) |
| `screens/admin/Permissoes.tsx` (A4) | `fetchMatrix`+`fetchRules`; `confirmar` (mudança pendente); `TOOL_DESC` | `setRule(profile,tool,decision)` (allow→deny→ask→allow), `revokeRule(id)`; vale imediatamente |
| `screens/admin/Modelos.tsx` (A5) | `fetchPublicacao`+`fetchLedger(100)` (filtra `btv.flow_saved`); `useTemplates` | `setPublicacao(id,!publicado)`; fluxos do Designer como rascunhos no topo |
| `screens/admin/Usuarios.tsx` (A6) | `fetchUsers` → users; refs de inputs; estado `ativo`/`desafio`/`pinErro`/`removendo` | `createUser`, `setUserAtivo`, `deleteUser`, `verifyUserPin`; remoção via `ConfirmModal` (substitui `window.confirm` — F4); PIN opcional (1º user = admin) |
| `screens/admin/comum.tsx` | props locais | `StatCard`/`Pill`/`Toggle`/`ErroBox`/`NotaHonesta` (primitivos de render, sem dados; `Pill`/`ErroBox` usam tokens `--ok-bg`/`--warn-bg`/`--err-bg`) |

### btv-web/src/components/shell/{Shell,Topbar,Sidebar,GearDrawer}.tsx
Layout; consomem AppState + squad ativa.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `screen`/`accent`/`squad` (Shell) | estado | entrada | AppState → useBrand + render | `accentColor` = cor da squad em 'vivo'; monta `SCREEN_COMPONENTS[screen]` + `WizardOverlay` |
| `persona`/`squad` (Topbar) | estado | entrada→saída | AppState → chip `squad-chip{cor,nome,status}`; dispatch SET_PERSONA | chip só em persona user |
| `persona`/`screen`/`squad` (Sidebar) | estado | entrada→saída | nav + seção "squad ativa" (Ao vivo/Entregas, badge `gateAberto`); dispatch SET_SCREEN | rodapé "Marina L." é placeholder (A6 real em outra onda) |
| `accent`/`BRAND_SWATCHES` (GearDrawer) | estado/const | saída | dispatch SET_ACCENT; atalhos SET_SCREEN personas/minhas | ajustes locais (marca troca `--brand` na hora) |

Fluxo: Shell roteia por `screen`; Topbar/Sidebar refletem squad ativa; GearDrawer muda marca.

### btv-web/src/components/primitives/{AsyncStatus,Modal,Toast}.tsx
Primitivas de feedback e estado assíncrono do produto (espelham as do console `web/`, nos tokens do btv-web). Sem terracota — feedback nunca usa a cor de decisão.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AsyncStatus{state,onRetry?,erroPrefixo?,children}` | componente | entrada→saída | `AsyncState<T>` (do `useAsyncAction`) → render | idle/loading/error/success unificados; erro usa `--err-bg`/`--err-line`/`--err-ink` + retry |
| `ConfirmModal{aberto,titulo?,mensagem,confirmarLabel?,onConfirmar,onCancelar}` | componente | entrada→saída | estado da tela → diálogo | confirmação destrutiva (`--err`, não terracota); substitui `window.confirm` |
| `ToastProvider` / `useToast().push(kind,msg)` | contexto | entrada→saída | qualquer componente → pilha fixed bottom-right | `kind` = success/error/warn; auto-dismiss 4.5s; substitui `window.alert` |

### btv-web/src/main.tsx · App.tsx
Bootstrap e árvore de providers.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `createRoot(#root).render(<App/>)` | — | config | main → DOM | StrictMode |
| `<AppProvider><ToastProvider><TemplatesProvider><SquadRunProvider><Shell/>` | árvore | config | App → contexts | `ToastProvider` no topo (feedback global, usado pelo `SquadRunContext` via `useToast` — sem `window.alert`); templates e squad ativa acima da troca de tela |

Fluxo: providers aninhados garantem toast global + templates + squad viva sobrevivendo à navegação.

---

## Notas transversais (fronteira navegador ↔ backend)

- **Dois estilos de SSE:** sessão (`stream.ts::SessionEvent`, envelope autoral, reconecta infinito) vs squad (`squad.ts::SquadEventEnvelope`, `SquadEvent` cru de tonic, tarefa FINITA, fecha no 1º erro). O produto `btv-web` só usa o segundo.
- **Corpo vazio 202/204:** `fetchJson` (ambos) e os POSTs de squad (`postSquadMessage`/`emergencyStopSquad`) tratam corpo vazio sem `.json()` — bug real corrigido na Onda 15.
- **`model`/`agent`:** só o console (`web/`) envia `model`/`agent` por mensagem/tarefa; o produto (`btv-web`) roda o motor sem parametrizar modelo pela UI.
- **Honestidade "Nada Fake":** vários DTOs carregam a tensão explicitamente — `ValueReview` (só dims determinísticas), rate limits (tetos, não uso ao vivo), memória (léxico, não semântico), custo (estimativa envelhecível), esteira `inferida` (posição deduzida rotulada), "sem artefato real" (run sem arquivo gravado).
- **Ledger:** o console tipa `LedgerEntry` com `override?`/`fake_marker?` (auditoria completa); o produto tipa `payload` como `Record` e omite esses campos (só leitura A2).
