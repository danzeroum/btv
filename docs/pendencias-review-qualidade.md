# Pendências do review de qualidade de código

> **Nota:** este arquivo é o registro do trabalho de **review de qualidade**
> (sessão de code-review). É separado do `pendencias.md` da raiz (o log de
> trabalho de 151KB do projeto) para não colidir com ele. Aqui ficam: o que foi
> entregue, o que foi **deliberadamente adiado** e por quê, e as **decisões do
> dono** na auditoria da rodada.

> **Governança registrada:** o **auto-merge** usado nos PRs #58–#61 foi *desvio
> sancionado desta rodada* (autorizado no arranque do loop), válido para lote de
> correções com juízes automáticos fortes — **não é novo default**. O rito da
> campanha (merge do dono) volta a valer, em especial para qualquer coisa que
> toque **contrato, ADR ou comportamento visível**. Por isso o PR do guard do
> hash (ADR 0032) **não** é auto-mergeado: fica para o dono.

## 1. Entregue (3 PRs mergeados em `main`)

Origem: `docs/REVIEW-QUALIDADE-CODIGO.md` (curado, com diffs e rito) +
`docs/REVIEW-AUDITORIA-COMPLETA.md` (varredura dos 337 arquivos). Cada mudança
foi analisada quanto a impacto micro/macro e passou pelo gate equivalente à CI
(clippy `-D warnings`, fmt, `cargo test --workspace`, arch-lint, pytest, e
tsc/vitest quando tocou TS). Todos os 13 jobs da CI verdes antes de cada merge.

### PR #58 — 2 bugs de alta severidade + lote curado
- **`btv-tools/bash.rs`** — deadlock de pipe: stdout/stderr eram lidos só após
  o `wait`; saída > ~64KB enchia o buffer, travava e virava **timeout falso**.
  Agora dois threads drenam os pipes em paralelo. Teste-que-morde.
- **`btv-sidecar/service.rs`** — `SquadPool::acquire` vazava o `slot` da free-list
  em falha de spawn/wait_ready; após `capacity` falhas, `free.pop().expect` derrubava
  o processo. `SlotGuard` devolve o slot em qualquer saída antecipada. Teste-que-morde.
- **C1 (`btv-cli/btv_agent.rs`)** — erro de store nas personas (`unwrap_or_default`)
  degradava em silêncio (procedência/hash mentindo) → propaga 500. Goldens verdes.
- **`btv-squad/_json.py`** — regex gulosa `\{.*\}`+DOTALL corrigida (raw_decode) e
  parse unificado de 6 agentes. **`security.py`** — serialização do guard unificada
  (fecha bypass). **`memory.py`** — cache do corpus do recall (invalida por mtime+tamanho).
  **`hitl.py`** — magic numbers → constantes.
- **`btv-tools/lsp.rs`** — `retry_until_ready` (DRY). **`btv-cli/squad_agent.rs`** —
  `wait_for_socket` (fail-closed). **`btv-web`** — `.catch` faltantes, `'marina'`
  hardcoded removido, `hhmm` extraído. **`btv-squad`** — `DEFAULT_MODEL` (12-Factor).

### PR #59 — erros observáveis + 12-Factor
- `web_agent.rs`: `persist_new().unwrap_or(0)` engolia falha de persistência → loga.
- `btv_agent.rs`: `papeis_json` malformado silencioso → loga o run afetado.
- 3 sidecars: `VERSION` via `importlib.metadata` (era literal triplicado); log level
  por `BTV_LOG_LEVEL`.

### PR #60 — config hardcoded por env
- `pg.rs`: pool `max_connections(4)` → `BTV_PG_MAX_CONNECTIONS`.
- `web_agent.rs`: `max_steps`/`max_tokens`/`context_window`/modelo padrão → env
  (`BTV_WEB_MAX_STEPS`/`BTV_WEB_MAX_TOKENS`/`BTV_WEB_CONTEXT_WINDOW`/`BTV_DEFAULT_MODEL`).
  Defaults preservam o comportamento.

## 2. Deliberadamente adiado (por recomendação própria)

### 2.1 Refactor mecânico `erro()`/`lock_store()` (~96 sítios em `btv_agent.rs`) — **RECUSADO (Q2)**
DRY cosmético de **alto churn** e baixo valor: colapsar os ~65 blocos
`(...).into_response()` e os ~31 `lock().unwrap_or_else(|e| e.into_inner())` num
helper. **Risco desproporcional:** os corpos de erro são pinados **byte-a-byte**
por golden (`squad_activation_errors.golden.json`); qualquer drift quebra o CI, e
os 96 sítios são varredura manual propensa a erro. O ganho é só de legibilidade.

**Decisão da auditoria (Q2): recusado com razão (não "adiado").** A distinção
importa para o backlog — adiado tem gatilho e vence; recusado tem razão e
encerra. **Cláusula de reabertura:** se algum dia os corpos de erro *precisarem*
mudar (regravação justificada de golden por outra razão), o refactor pega carona
aí de graça.

### 2.2 Splits estruturais (god-objects / mega-funções)
Decompor `ativar_squad_handler` (~244 linhas), `BtvStore`/`PgStore` (API dupla,
6 agregados) e separar o agregado `Run` de `ports.rs`. **Isto reforça uma
pendência JÁ REGISTRADA** em `pendencias.md:2182` (`[residual — decomposição dos
3 grandes, projeto pós-campanha]`), cujo **gatilho declarado** (`:2189-2190`) é o
"SEGUNDO consumidor do motor (modo saas em processo separado, ou worker
headless)". Enquanto não houver esse segundo consumidor, é "custo sem comprador".
Além disso, a API dupla do `BtvStore` é uma **decisão registrada** (`pendencias.md:1829`,
B2 — a porta do modo local com escopo fixo no tenant LOCAL): colapsá-la sem
preservar esse escopo reintroduziria vazamento cross-tenant. **Não abrir backlog
concorrente** — segue a pendência existente.

### 2.3 Restrição numérica do hash `prompt-cache-key.v1` — **APROVADO E IMPLEMENTADO (Q1)**
`hashing.py`/`canonical.rs` documentavam (só em prosa) que floats com fração zero
(`1.0`) são proibidos no v1 — mas nada os rejeitava, então um produtor JS (`1`) e
o Python/Rust (`1.0`) divergiam silenciosamente na chave de cache (cache-miss
cross-produtor = custo real de API pago sem sinal).

**Decisão da auditoria (Q1): SIM — próximo trabalho de mérito** (único deferido
com impacto de *correção*). **Implementado nesta PR:** ADR 0032 + validador
compartilhado (`validate_cache_key`/`CacheKeyError` nos dois lados) chamado por
`request_hash` (que passa a falhar: `Result` em Rust, `raise` em Python); o
`CachedGenerator` degrada pulando o cache em chave proibida; fixtures ganharam
`reject_cases` (regeneração não-circular, hashes válidos byte-idênticos) e os
2 testes de paridade agora provam a recusa nos dois lados. **Merge é do dono**
(contrato/ADR — ver nota de governança no topo).

### 2.4 Baixa severidade remanescente
O restante do `docs/REVIEW-AUDITORIA-COMPLETA.md` (dos 239 achados, após os
entregues acima) é majoritariamente nit de baixa severidade: magic numbers/strings
soltos, naming, DRY menor por tela/módulo, ternários aninhados no front, `now_rfc3339`
reimplementado por crate, `current_dir` repetido em 5 handlers do `web_agent`.
São melhorias incrementais de manutenibilidade sem impacto de correção/segurança —
melhor tratadas em lotes pequenos por módulo do que forçadas num churn amplo.
Catálogo completo (arquivo:linha) já está em `REVIEW-AUDITORIA-COMPLETA.md §3`.

## 3. Decisões do dono (auditoria da rodada) — registradas

1. **Q1 — Guard numérico do hash: SIM.** Implementado nesta PR (§2.3, ADR 0032).
   Único deferido com impacto de correção; feito no rito completo, **merge do
   dono** por ser contrato. *Vencimento observado:* quanto mais produtores de
   cache-key surgirem, mais caro fica o buraco — por isso foi priorizado.
2. **Q2 — Refactor `erro()`/`lock_store()`: NÃO** (§2.1). Promovido de "adiado"
   para **recusado com razão**, com cláusula de reabertura (pega carona se um
   golden precisar ser regravado por outra razão).
3. **Q3 — Defaults de config (#60): sem ação agora, adiado com gatilho.** Os
   valores preservam o comportamento byte-a-byte; a pergunta "servem ao alvo de
   deploy?" só tem resposta quando existir alvo de deploy. **Pertence ao pacote
   de lançamento SaaS** (junto de E3s/E5s — observabilidade e quotas), onde os
   números viram decisão de capacidade. Revisitar lá.
4. **Q4 — Splits estruturais: mantém o gatilho, sem antecipação** (§2.2). Extração
   sem segundo consumidor é custo sem comprador; a API dupla do `BtvStore` é
   decisão registrada (B2). Quando o modo saas precisar do motor em processo
   separado, a decomposição paga a si mesma e vira campanha própria com seus
   portões.

**Frentes vivas depois desta PR:** só a **bifurcação SaaS** (que engole Q3 e,
eventualmente, Q4). O guard do hash (Q1) fecha aqui, aguardando só o merge do
dono. O repositório segue verde; o resto são escolhas de evolução.
