# Plano-mestre: Fase 7 — o navegador como forma primária de uso

> Documento de execução, no formato dos planos anteriores: fatos ancorados no código
> real (verificado no main pós-Fase 6, roadmap das 6 fases concluído), ondas com
> fronteira verificável, decisões de contrato viram ADR. Produzido por 2 agentes
> Explore (arquitetura de streaming/permissão; veredito por tela das 9 telas mock),
> 1 agente Plan (ordem de ondas), e revisado por uma passada de gerência que ancorou
> 4 achados extras no código antes de aprovar (ver §1).
>
> **Este documento substitui `docs/PLANO-INTEGRACAO-FRONTEND.md`** (branch
> `claude/frontend-backend-integration-7u5mti`, nunca mergeado — nenhuma PR aberta
> para ele). Mesmo território, mesma pesquisa de base (o servidor expõe só 3 rotas
> GET, o resto é mock), mas com granularidade de 12 ondas e fronteira de teste
> executável por onda, em vez de 5 ondas mais largas. Ideias concretas dessa outra
> varredura foram absorvidas aqui: a guarda de `Origin`/`Host`, o wiring de MCP real
> + telemetria por modelo (Onda 7), o contrato de erro `{error, code}` via
> `fetchJson()`, o truque `ScriptedGenerator`/modo roteirizado para e2e sem API key,
> e o filtro `?actor=` do ledger. O documento antigo recebeu uma nota de "superseded"
> no topo, nesta mesma entrega — nenhum dos dois deve divergir dali em diante.

## 0. Contexto e critérios de conclusão

O roadmap original (6 fases) está concluído — `CLAUDE.md`/`docs/PLANO-PLATAFORMA-FORGE.md`
já declaram isso, e o próprio `CLAUDE.md` antecipa: "o que vier depois é produto novo,
não plano antigo". O Forge foi testado de ponta a ponta numa VPS via Docker e, depois
de navegar o dashboard web, veio o pedido: "quero o frontend todo funcional como o
CLI, ele será a forma principal de usar" — refinado depois para "faça apenas um plano
para o frontend funcionar corretamente com o máximo das funcionalidades disponíveis
no backend".

O frontend (`web/src/`) já existe, bem-acabado — 13 telas, duas personas, sistema de
componentes próprio (documentado em `docs/LEVANTAMENTO-UI-DESIGNER.md`) — mas hoje é
**95% vitrine**: `crates/forge-server` expõe só 3 rotas GET read-only; o resto lê de
`web/src/api/*.ts` com `simulateLatency()`.

A Fase 7 define quatro critérios literais de conclusão:

1. **As 13 telas existentes operam sobre dados reais** — `grep simulateLatency
   web/src/api` vazio (sobra só em fixture de teste).
2. **Toda rota mutável está protegida contra CSRF/DNS-rebinding local** — uma
   requisição `Origin: https://evil.example` para qualquer método ≠ GET recebe
   `403`; sem `Origin` (curl/CLI), passa.
3. **O job `web` (Playwright real, `web/tests/e2e-integration/`) roda verde no CI**
   — hoje o harness existe mas não é exercitado por nenhum workflow.
4. **Nenhuma tela finge fazer algo que o backend não faz** — onde o backend ainda não
   sustenta o comportamento (autonomia progressiva, mutação de providers), a tela
   declara isso explicitamente em vez de simular.

A postura local-first/single-user (`forge-server` amarrado a `127.0.0.1`) não muda —
esta fase não é "virar SaaS multiusuário". Mas o navegador como forma **primária** de
uso introduz uma superfície de risco nova mesmo em localhost: qualquer aba aberta no
mesmo navegador pode tentar `POST http://127.0.0.1:7878/api/...` — inclusive a rota
que aprova execução de `bash`. O critério nº 2 existe por causa disso, não por
paranoia de produção.

## 1. O que foi verificado (2 Explore + 1 Plan + revisão de gerência)

**A peça dura, por que isto não é wiring simples:**

- `PermissionResolver` (`crates/forge-core/src/agent_loop.rs:40-42`) é **síncrono**
  (`fn resolve(&mut self, tool: &str, scope: &str) -> bool`), dyn-dispatched. As duas
  implementações interativas **bloqueiam a thread** até um humano responder:
  `CliResolver` (stdin, `forge-cli/src/main.rs:867-887`) e `TuiResolver`
  (`forge-cli/src/tui_app.rs:114-135`) — esta é o precedente de **forma** a replicar:
  publica o pedido num `mpsc::UnboundedSender<TuiMsg>` não-bloqueante e bloqueia em
  `std_mpsc::Receiver<bool>::recv()` até a UI (loop de render em thread própria)
  responder pelo canal pareado. O agent loop roda em `tokio::spawn` separado da UI.
- `LoopEvent<'a>` (`agent_loop.rs:15-37`) só é `Debug`, não `Serialize`, e não é
  `'static` (`&'a str`). Não existe hoje nenhum DTO owned+serializável — precisa ser
  criado (o `TuiMsg` é o precedente de forma, mas também não é serializável).
- **Achado do agente Plan:** `run_tool` (`agent_loop.rs:183-249`, a chamada exata é a
  linha 197: `Decision::Ask => resolver.resolve(name, &scope)`) roda **sincronamente
  dentro do caminho async do loop**, sem `spawn_blocking`. Inofensivo no CLI/TUI (um
  processo, um usuário); num servidor com N sessões, uma permissão pendente por
  minutos prenderia uma worker-thread do reactor Tokio — o suficiente disso trava
  requisições não relacionadas (inclusive o `/api/summary` do dashboard atual).
  Mitigação: cada sessão viva roda em `spawn_blocking`, nunca `tokio::spawn` comum —
  decisão explícita, não detalhe de implementação.
- `forge-server` (`crates/forge-server/src/lib.rs:30-46`) hoje só depende de
  `forge-store`/`forge-verify`/`forge-llm` (este último só para o bin `loadgen`, não
  o dashboard). **Não depende** de `forge-core`, `forge-tools`, `forge-sidecar`,
  `forge-proto`. `forge-cli` já depende de tudo. Isso decide onde o código novo mora
  (ver Onda 1). `axum::response::sse` já vem compilado (axum 0.8, zero dep nova);
  `tokio-stream` já pinado no workspace (feature `"sync"` a mais para
  `BroadcastStream`, se fan-out multi-aba usar `tokio::sync::broadcast`).
- Squad: CLI drena `Streaming<SquadEvent>` por polling manual
  (`forge-cli/src/squad.rs:240-269`). O HITL do squad é uma chamada gRPC do Python de
  volta ao Rust (`CoreService.RequestPermission`), hoje um `spawn_blocking` sobre
  stdin (`forge-cli/src/squad.rs:76-99`). `SquadEvent` (prost/tonic-build) não tem
  serde hoje — adicionar via `.type_attribute` é baixo risco e, diferente do
  `LoopEvent`, **não precisa de DTO espelho** (o JSON do proto vai direto no SSE).
- `DurableSession` (`.forge/sessions.db`) e o `Session`-ledger do CLI
  (`.forge/forge.db`) são stores **diferentes**, hoje abertos lado a lado por
  `prepare`/`build_loop`/`open_durable` (`forge-cli/src/main.rs:374-459`) — é a
  receita que um handler HTTP precisa replicar. Só `Telemetry` tem hoje um handle
  `Arc<Mutex<...>>` compartilhável — `LedgerStore`/`EventStore`/`PromptLibrary`
  precisam do mesmo wrapper (replicar o padrão, não inventar).
- `run_pipeline` (`forge-verify`) é síncrono; passos default somam ~8.5min
  sequenciais — não cabe num request/response HTTP síncrono.
- `Gateway::generate` usa array hardcoded, nunca consulta `FallbackChain`
  (`forge-llm/src/provider.rs:41-63`), que **existe mas é código morto**.
  `RateLimiter` não tem API de mutação nem getter de uso.
- **Achado forte:** `max_autonomy_level` do proto (`schemas/proto/squad.proto:20`),
  hardcoded `3` em `forge-cli/src/squad.rs:219`, é **recebido e nunca lido** em
  `python/packages/forge-squad` (grep confirma zero uso) — dado morto ponta a ponta.
  Plugar um seletor de UI nesse campo sem tocar o Python seria fake-wiring.
- `LedgerStore::open` (`crates/forge-store/src/ledger.rs:29`) **não liga WAL**, ao
  contrário de `EventStore::open` (`events.rs:87-89`, que liga `journal_mode=WAL` +
  `synchronous=NORMAL`) — CLI e servidor web tocando `.forge/forge.db` ao mesmo tempo
  pode dar "database is locked". Bug de concorrência latente, exposto pela primeira
  vez por esta fase.
- Já existe uma suíte Playwright real (`web/tests/e2e-integration/
  telemetry-real-backend.spec.ts` + `web/scripts/run-integration-server.mjs`) que
  sobe um `forge dashboard` real (build real, sqlite real) e prova a tela contra dado
  semeado por fora do browser — **não roda em CI hoje** (sem job `web` em
  `.github/workflows/ci.yml`). Estender esse harness, não inventar outro.
- `ScriptedGenerator` público (Fase 6 Onda 8, `crates/forge-llm/src/scripted.rs`) já
  tem `from_turn(turn)` (linha 39) aceitando um turno arbitrário com `tool_use` — mas
  tanto `echo` quanto `from_turn` devolvem **sempre o mesmo turno** a cada chamada
  (`generate` clona `self.turn`); não há sequenciamento (turno 1 ≠ turno 2 ≠ turno 3
  em chamadas sucessivas do mesmo generator). Isso já existe, mas só como o tipo
  **privado** `Scripted` dentro de `agent_loop.rs`'s `#[cfg(test)] mod tests`
  (`turns: Mutex<Vec<AssistantTurn>>`, consumido com `remove(0)` a cada chamada). A
  fronteira dos testes desta fase precisa promover esse padrão de fila para uma
  variante pública reusável (mesma promoção que `echo`/`from_turn` já fizeram) — não
  inventar sequenciamento do zero.

**Inventário do Grupo B (o escopo desta fase — 11 lacunas reais):**

| # | Tela/ação | Backend real hoje |
|---|---|---|
| 1 | Ledger (admin) | `LedgerStore` (falta leitura paginada) |
| 2 | Verify (admin) | `run_pipeline` (síncrono, 8.5min) |
| 3 | Providers (admin) | `Gateway`/`RateLimiter` (sem mutação) |
| 4 | Skills — matriz de permissão (admin) | `PermissionEngine`/`AgentProfile` (hardcoded) |
| 5 | Sessão de código (user) | `AgentLoop`/`DurableSession` |
| 6 | Permissão ao vivo (user) | `PermissionResolver` |
| 7 | Squad ao vivo (user) | `SquadService` gRPC |
| 8 | Prompts — biblioteca (user) | `PromptLibrary` |
| 9 | Prompts — render (user) | `PromptForgeService` (sidecar) |
| 10 | Modelo & Onboarding (user) | nenhuma persistência hoje |
| 11 | Designer (user) | novo: schema `squad.workflow.v1` |

O item #4 (gap na matriz de permissão de `skills.ts`) não é Grupo A, mas também não é
inédito: já aparecia en passant no `PLANO-INTEGRACAO-FRONTEND.md` concorrente (Onda 2,
que o deferia para **read-only**, adiando edição "para quando houver persistência de
config com ADR próprio"). O que este documento decide diferente, deliberadamente, é
permitir a edição — mas com trilha de auditoria (ver Onda 2) em vez de deixá-la
read-only.

**Não-escopo explícito:** Grupo A do `LEVANTAMENTO-UI-DESIGNER.md` (Experimentos A/B
como tela dedicada, Mapa de memória/RAG, quota de rate-limit editável, gestão de
skill de terceiro, status LSP) é **design novo**, não wiring — fica fora, candidato a
fase seguinte. **Exceção deliberada — 2 dos 7 itens de Grupo A são wiring puro, sem
nenhum design novo, e sua ausência contradiz a régua "Nada Fake" do próprio plano:**
Console MCP (a tela Skills hoje exibe saúde **fabricada** de servidores
filesystem/git/postgres que não existem, `web/src/api/skills.ts`) e telemetria por
modelo (uma consulta a mais sobre dado que a Telemetria — já 100% real — já carrega
em `props.model`). Os dois entram via a Onda 7 nova, com fronteira read-only. A
Experimentos A/B **não** entra por essa mesma exceção porque, diferente dos outros
dois, exige uma tela inteira nova — é design, não wiring.

**Princípio de recorte:** priorizar o que o backend **já oferece** sobre inventar
capacidade nova. Isso guia duas ondas:
- **Providers** — o piso é uma tela **real, só leitura** do que `Gateway`/
  `RateLimiter` já sabem (zero engenharia nova). A mutação (reordenar fallback via
  o `FallbackChain` já existente-mas-morto; ajustar teto de rate-limit) é um degrau
  a mais, ainda modesto porque o tipo já existe — decidir o corte exato no início da
  onda, não assumir de saída.
- **Modelo & Agente** — em vez de um store de preferências persistidas novo (que não
  existe em lugar nenhum do repo), usar o que o backend já faz: `--model`/`--agent`
  **por chamada**, igual ao CLI. A UI manda esses parâmetros por sessão/tarefa; não
  inventa "seleção persistida entre sessões" a menos que o produto peça depois.

## 2. Arquitetura das ondas (a lógica da ordem)

Só há **uma dependência dura**: Onda 1 → Onda 2. Tudo mais é majoritariamente
independente (ver §5).

### Onda 1 — Fundação web

DTO owned+`Serialize` espelhando `LoopEvent` (mora em `forge-cli`, ao lado de
`TuiMsg` — `forge-core` continua UI-agnóstico); rota SSE genérica por `session_id`
(`axum::response::sse`, zero dep nova); um `PermissionResolver` novo que publica o
pedido no mesmo SSE e aguarda a resposta via `POST /api/session/:id/permission` —
mesmo desenho do `TuiResolver`, rodando em `spawn_blocking` (mitiga o esgotamento de
worker-threads). Código novo entra em `forge-cli` como um `Router` `.merge()`ado ao
`forge_server::router()` existente — **`forge-server` ganha zero dependência nova**,
o crate estável/em-produção-real (túnel SSH) fica intocado. Flag de opt-in
(`--web-agent`) até o fecho. Job `web` novo no CI já aqui (o harness Playwright já
existe, só não roda em CI).

**Segurança de mutação (bloqueante desde esta onda):** middleware único validando
`Origin`/`Host` contra localhost em todo método ≠ GET — ausência de `Origin`
permitida (curl/CLI seguem funcionando sem navegador), `Origin` de outra origem →
`403`. Esta é a rota que literalmente aprova execução de `bash`; sem essa guarda,
qualquer site aberto no mesmo navegador poderia disparar `POST
http://127.0.0.1:7878/api/session/:id/permission`.

**Contrato de erro e cliente HTTP:** `client.ts` ganha `fetchJson()` real (checa
`r.ok`, lança `ApiError` com código); toda rota nova responde erro como JSON único
`{error, code}` — fim do padrão "assume sucesso" que os módulos mock têm hoje.

**Pedidos de permissão sobrevivem a navegador fechado:** o pedido pendente vive em
estado do servidor (não só publicado uma vez no SSE) — quem conectar depois (aba
nova, ou a mesma aba reconectando) recebe o pedido ainda pendente via snapshot. Sem
isso, fechar o navegador no meio de uma aprovação perde o evento e o pedido fica
órfão até o timeout.

**Teto de sessões vivas:** cada sessão ocupa uma thread do pool `spawn_blocking`
enquanto viver — limite configurável (ex.: 8 sessões simultâneas), `429` acima do
teto.

*Decisões→ADR:* forma do DTO; contrato SSE — nomes de evento e semântica de
reconexão (**snapshot do estado atual, reconstruído do `DurableSession`, + eventos ao
vivo daí em diante**; `Last-Event-ID`/replay fino explicitamente fora de escopo);
timeout de permissão pendente sem resposta (fail-closed, `Deny` após prazo); teto de
sessões simultâneas.

*Fronteira:* servidor axum real em porta efêmera, generator sequenciado novo pede
`Ask` e encerra; cliente HTTP real (reqwest + `bytes_stream`) recebe o SSE, um `POST`
resolve, sequência de eventos verificada por igualdade contra o esperado. Segundo
teste: sem resposta, o resolver expira em `Deny` sozinho (prazo encurtado via config
de teste). Terceiro teste: requisição `POST` com `Origin: https://evil.example`
recebe `403`; a mesma requisição sem `Origin` passa. Quarto teste: conectar o SSE
**depois** do pedido de permissão já existir — o cliente ainda vê o pedido pendente
(prova o snapshot-then-live, não só o caminho feliz de "já estava conectado").

### Onda 2 — Sessão de código + Permissão ao vivo (o marco da fase)

`POST /api/session/:id/message` replica a receita `prepare`/`build_loop`/
`open_durable` e transmite via SSE; `Sessao.tsx` troca mock por `EventSource` real
(hook novo, `useEventSource`); `Permissao.tsx` reflete o pedido pendente real.
Empacotado junto (reusa `PermissionEngine`/`Rule`, já `Serialize`): a matriz de
permissão build/plan×tool (`togglePermissionCell`) vira persistida — o item #4 do
inventário, com a decisão de edição (não read-only) tomada acima.

**`skills.ts` perde o fallback silencioso:** `fetchSkills()` hoje cai em `SKILLS`
mock quando o `fetch` falha (`web/src/api/skills.ts:37-45`) — isso mascara um
backend quebrado atrás de dado falso. Vira estado de erro explícito no `AsyncStatus`
existente; mock só sobrevive em teste unitário.

**Trilha de auditoria da matriz de permissão:** afrouxar permissão pelo navegador é a
mutação mais sensível deste plano. Toda gravação/remoção de `Rule` vira uma entrada
no ledger (override marcado, mesmo padrão append-only já existente); a UI lista as
rules ativas com botão de revogar; o escopo da rule (`tool` + `scope_prefix`) aparece
explícito no modal antes de confirmar — nunca um clique único e opaco.

*Decisões→ADR:* concorrência multi-aba — sessão = ator único por `session_id`,
turnos serializados, segunda tentativa concorrente recebe `409` (não corrupção); o
terceiro estado `"always"` do frontend grava uma `Rule` de override, não só resolve
o pedido atual; mutação de política de permissão sempre deixa rastro no ledger.

*Fronteira:* Playwright real — dashboard sobe com generator sequenciado (sem key),
mensagem → pedido de `bash` real aparece na tela `Permissão` → clica "Permitir" →
texto final + "ledger íntegro: N" batem com leitura direta do `.forge/forge.db`.
Segundo teste: duas abas na mesma sessão, a segunda escrita concorrente recebe erro
claro, não corrompe o histórico. Terceiro teste: editar uma célula da matriz grava
uma `Rule`, aparece uma entrada nova no ledger, e o botão "revogar" a remove da lista
ativa. Quarto teste: backend fora do ar mostra estado de erro explícito na tela
Skills, não o array mock.

### Onda 3 — Sidecar Python como serviço de longa duração

Hoje `SidecarSupervisor`/`SquadClient` spawnam `uv run ...` com `kill_on_drop` —
ciclo de vida por-invocação-CLI. Um supervisor-serviço novo (distinto do
supervisor-CLI existente, que continua intacto) mantém o processo vivo entre
requisições, com health-check e restart-on-crash. Zero dependência de Onda 1 — é
sobre supervisão de processo, não HTTP.

*Decisões→ADR:* instância única compartilhada para PromptForge (stateless,
serializar um `render` por vez é aceitável); pool pequeno com limite para Squad
(execução longa, um processo só serializaria squads concorrentes).

*Fronteira:* supervisor real atende 3 requisições sequenciais sem reabrir o
processo (PID estável) → `SIGKILL` no meio (mesmo padrão de `squad_e2e.rs`) →
detecta a queda, sobe processo novo (PID diferente), próxima requisição atendida
sem o servidor Rust reiniciar.

### Onda 4 — Squad ao vivo *(depende de Onda 1 + Onda 3)*

`POST /api/squad/run` via `SquadService.ExecuteTask` (usa o supervisor-serviço),
transmite `SquadEvent` como SSE — sem DTO espelho (serde direto no tipo gerado pelo
proto). O gate HITL troca stdin por `POST /api/squad/:task_id/hitl`, mesma forma da
ponte de permissão (incluindo persistência de pedido pendente, ADR 0016/0017).

*Fronteira:* Playwright — squad real (Python, sem key) mostra agentes mudando de
estado ao vivo (não array estático), gate HITL resolvido pela UI, `squad.consensus`
conferido direto no ledger.

### Onda 5 — Prompts

Metade CRUD (`GET/POST /api/prompts`, listar/salvar/favoritar/remover sobre
`PromptLibrary`) é wiring puro, zero dependência — pode sair antes até da Onda 1
fechar. Metade `render` depende do supervisor-serviço (Onda 3).

*Fronteira:* CRUD — teste HTTP direto confere sqlite. Render — texto devolvido pela
rota bate com chamada gRPC direta ao mesmo sidecar (paridade, não só "200 OK").

### Onda 6 — Ledger (vitória rápida, zero dependência)

Leitura paginada nova sobre `LedgerStore` (precedente exato em
`TelemetryStore::recent`) + `GET /api/ledger?limit&actor` + `POST
/api/ledger/verify` sobre `verify_chain()` já existente. Liga WAL em
`LedgerStore::open` (bug de concorrência latente, exposto agora). O filtro `?actor=`
entra desde o primeiro corte — a tela mock já filtra por ator; entregar a rota sem
isso forçaria filtro client-side sobre um dump completo, regressão de UX disfarçada
de "ligado".

*Fronteira:* semeia N entradas via `LedgerStore::append` por fora do browser, a
tela mostra exatamente essas N (hash prev/curr por igualdade); `?actor=X` devolve só
as entradas de X; verificação mostra `ok:true, verified:N`.

### Onda 7 — MCP real + Telemetria por modelo (zero dependência)

Fecha os 2 itens de Grupo A que são wiring puro, sem design novo (ver §1). Mesma
classe de esforço da Onda 6 — paralelizável desde o dia 1.

- `GET /api/mcp`: enumera `.forge/mcp.toml` (`load_mcp_servers`) e chama
  `list_tools_blocking` (`forge-tools/src/mcp.rs:65`) por servidor em
  `spawn_blocking` com timeout curto; status `up`/`down` honesto. Substitui
  `MCP_SERVERS`/`reconnectMcp` mock em `web/src/api/skills.ts` (linhas 11-15,
  59-65). Fecha a pendência já registrada em `pendencias.md` (Onda 4 da Fase 6:
  "Frontend MCP não ligado").
- Telemetria por modelo: nova consulta em `TelemetryStore` agrupando por
  `props.model` via `json_extract` — mesmo padrão de `experiment_variants`
  (`telemetry.rs:117-139`, que já agrupa por `props.variant`/`props.experiment`) — +
  um card na tela Telemetria, que já é 100% real.

Sem ADR novo — as duas rotas são read-only, sem contrato novo.

*Fronteira:* fixture com 2 servidores MCP declarados (1 respondendo, 1 fora do ar)
mostra status real na tela Skills, não mais um array estático; o card de telemetria
por modelo bate, por igualdade, com uma agregação manual dos mesmos eventos
semeados.

### Onda 8 — Verify (job em background, zero dependência)

`POST /api/verify/run` roda `run_pipeline` em `spawn_blocking`, devolve `run_id`;
`GET /api/verify/:id` via polling (hook `usePolling` já existe). Callback de
progresso por passo é extensão nova em `forge-verify` (hoje só devolve no fim).

**Execuções concorrentes são serializadas:** um job de verify por vez — um segundo
`POST /api/verify/run` com job ativo recebe `409` com o `run_id` corrente, em vez de
disputar o mesmo `target/` e workspace. O estado do job vive em memória
(`Arc<Mutex<...>>`) — reinício do servidor perde o job em andamento; aceitável para
um produto local-first, mas documentado explicitamente na tela (não é surpresa).

*Fronteira:* pipeline fixture com passos curtos; status muda "rodando"→"passo N de
M" conforme completam, termina no veredito certo — prova progresso real, não
placeholder. Segundo teste: dois `POST /api/verify/run` em sequência rápida — o
segundo recebe `409` com o `run_id` do primeiro, não um job novo.

### Onda 9 — Providers (piso leitura real; mutação como degrau)

Piso = view real de providers configurados + limites por tier (zero engenharia
nova). Degrau = reordenar fallback consumindo o `FallbackChain` já existente (hoje
morto), introspecção+ajuste de teto no `RateLimiter` (precisa de API de mutação
nova).

*Fronteira (piso):* a tela reflete exatamente `Gateway::available()` e as
constantes de tier — sem fabricar `used/cap`. *Fronteira (degrau, se entrar):*
reordena via POST, dispara com `ScriptedGenerator` (um provider falha de propósito)
e confere que a ordem de tentativa observada é a nova, não a antiga.

### Onda 10 — Modelo & Onboarding

Modelo/agente = parâmetro por sessão/tarefa (mirroring do CLI — sem store de
preferência novo, ver princípio de recorte em §1). `GET /api/doctor` agrega
checagens já existentes mas espalhadas (env vars do gateway, `uv --version`, ping ao
Docker via bollard, git). **Autonomia explicitamente escopada, não implementada por
padrão**: dado que `max_autonomy_level` é ignorado ponta a ponta hoje, a UI pode
mandar o valor real (deixa de ser hardcoded `3`) mas a tela **declara** que o
orquestrador ainda não respeita esse teto — a menos que o produto priorize também a
mudança Python nesta onda (decidir no início, não por composição tácita).

*Decisões→ADR:* se a mudança Python de autonomia entra nesta fase ou vira pendência
re-declarada (mesmo padrão da pendência de consenso→ledger da Fase 6).

*Fronteira:* Doctor contra fixture sem `uv` no PATH mostra o item ausente (gêmeo
negativo, não "tudo verde" sempre). Se autonomia entrar: dois `SquadTask` com
`max_autonomy_level` diferentes produzem **comportamento diferente** (aprovação
pedida num caso, não no outro) — não só "o campo viajou".

### Onda 11 — Designer (salvar honesto)

`POST /api/designer/workflow` valida contra `squad.workflow.v1` novo (JSON Schema +
tipo Rust + fixture golden, padrão de `experiment.v1`) e grava no ledger
(`LedgerStore::append` já aceita payload livre, zero mudança de ledger). Tela troca
"aplica na próxima forge squad" por "salvo e validado; aplicação real é trabalho
futuro". **Orquestrador Python continua com os 5 agentes fixos — sem reescrita
nesta fase.**

*Fronteira:* grafo salvo é lido direto do ledger e valida contra o schema (fixture
com caso inválido); grafo malformado (aresta para nó inexistente) é rejeitado com
erro claro, não salvo silenciosamente.

### Onda 12 — Fecho

README/CLAUDE.md/PLANO-PLATAFORMA-FORGE.md declaram a Fase 7 concluída (ou o estado
honesto do que ficou — nomeadamente autonomia, se descoped); ADRs 0015+ citados;
flag `--web-agent` vira default; reconciliação explícita do `LEVANTAMENTO-UI-DESIGNER.md`
(Grupo B fechado, Grupo A permanece próximo trabalho de design salvo as 2 exceções
já fechadas na Onda 7).

**Critério mecânico de pronto:** `grep simulateLatency web/src/api` vazio (sobra só
em fixture de teste) — adicionado à verificação desde o início da fase, não só
conferido no fecho.

*Fronteira:* documentos contam a mesma história (grep); job `web` do CI verde há N
PRs seguidos; nenhuma pendência descoped vive fora dos documentos; `docs/PLANO-INTEGRACAO-FRONTEND.md`
segue com sua nota de superseded, sem conteúdo divergente deste documento.

## 3. Decisões de contrato previstas (ADRs)

- **0015** — local-first/single-user permanece; **e** fixa o modelo de ameaça do
  navegador (guarda de `Origin`/`Host` em toda rota mutável — Onda 1).
- **0016** — DTO de evento + contrato SSE: nomes de evento, e a semântica de
  reconexão (snapshot do estado atual + eventos ao vivo daí em diante;
  `Last-Event-ID`/replay fino fora de escopo) e persistência de pedidos pendentes no
  servidor (sobrevivem a navegador fechado).
- **0017** — timeout de permissão pendente, fail-closed (`Deny` após prazo).
- **0018** — sessão-ator, concorrência multi-aba; inclui a trilha de auditoria de
  mutações da matriz de permissão (toda gravação/remoção de `Rule` vira entrada no
  ledger).
- **0019** — sidecar como serviço: instância única vs. pool, restart-on-crash.
- **0020** — topologia de processo (código novo em `forge-cli`, router aditivo,
  flag de opt-in) e teto de sessões vivas simultâneas (429 acima do limite).
- **0021** — escopo da autonomia progressiva nesta fase (`max_autonomy_level`).

Schema novo: `squad.workflow.v1` (Designer, Onda 11).

Decisões mais mecânicas (serialização de execuções concorrentes do `/verify`,
paginação/filtro do ledger, remoção do fallback mock de `skills.ts`) não geram ADR
próprio — são detalhes de implementação de uma onda, não mudança de contrato ou
fronteira.

## 4. Riscos da fase

| Risco | Mitigação |
|---|---|
| Site aberto no mesmo navegador dispara mutação em `127.0.0.1` (CSRF/DNS-rebinding) | Middleware de `Origin`/`Host` em toda rota ≠ GET desde a Onda 1 (ADR 0015) |
| Permissão pendente nunca respondida trava um `spawn_blocking` para sempre | Timeout configurável, `Deny` default (ADR 0017) |
| Resolver síncrono em `tokio::spawn` comum esgota worker-threads sob N sessões | Sempre `spawn_blocking` por sessão viva + teto de sessões simultâneas (ADR 0020) |
| Pedido de permissão se perde se o navegador fechar antes de resolver | Estado do pedido vive no servidor, reemitido a quem conectar depois (snapshot-then-live, ADR 0016) |
| Duas abas na mesma sessão corrompem histórico | Sessão = ator único, turnos serializados, erro claro na 2ª escrita concorrente |
| Duas execuções de `/verify` disputam o mesmo `target/` e workspace | Um job por vez; segundo POST recebe `409` com o `run_id` corrente (Onda 8) |
| Sidecar Python não sobrevive a servidor de longa duração | Supervisor-serviço dedicado (Onda 3), testado com `SIGKILL`, antes de Squad/Prompts depender |
| Fase 7 quebra o `forge-server` hoje estável/em produção (túnel SSH) | Código novo em `forge-cli`, zero dep nova em `forge-server`, flag opt-in até o fecho |
| `max_autonomy_level` vira wiring de fachada (campo viaja, Python ignora) | Escopo explícito (ADR 0021); fronteira exige provar comportamento diferente |
| Suíte e2e real não roda em CI, regressão passa despercebida | Job `web` já na Onda 1, não no fecho |
| Grupo A entra de carona e infla o escopo | Não-escopo explícito, com 2 exceções deliberadas e nomeadas (Onda 7) |
| Verify (8.5min) parece travado sem sinal de progresso | Progresso por passo desde o primeiro corte da Onda 8 |
| Afrouxar permissão pelo navegador sem rastro do que mudou | Toda mutação de `Rule` vira entrada no ledger + lista de revogação na UI (Onda 2) |
| Dois planos de integração vivos contam histórias diferentes | Este documento substitui `PLANO-INTEGRACAO-FRONTEND.md`, que recebe nota de superseded na mesma entrega |

## 5. Sequência e paralelismo

Zero dependência entre si, podem rodar em paralelo desde o dia 1: Onda 3
(sidecar-serviço), Onda 5-CRUD (Prompts), Onda 6 (Ledger), Onda 7 (MCP + telemetria
por modelo), Onda 8 (Verify), Onda 9 (Providers), Onda 10 sem autonomia, Onda 11
(Designer). Onda 1→2 é a única cadeia dura. Onda 4 depende de 1+3. Onda 5-render
depende só de 3. Onda 12 é sempre última, e só depois da decisão de autonomia (Onda
10) estar resolvida ou formalmente re-declarada.

## Verificação

- O documento cobre as 11 lacunas do inventário de Grupo B, as 2 exceções de Grupo A
  (Onda 7), as 12 ondas + fecho, as 7 ADRs previstas, e a tabela de riscos.
- `grep -nE "Onda ([1-9]|1[012])" docs/PLANO-FASE-7-frontend-primario.md` lista as
  12 ondas na ordem esperada.
- `grep -n "Grupo A" docs/PLANO-FASE-7-frontend-primario.md` confirma o não-escopo
  explícito e suas 2 exceções nomeadas.
- `grep -n "max_autonomy_level" docs/PLANO-FASE-7-frontend-primario.md` confirma o
  achado do dado morto está registrado, não escondido.
- `grep -n "Origin" docs/PLANO-FASE-7-frontend-primario.md` confirma a guarda de
  CSRF/DNS-rebinding está na Onda 1, não differida.
