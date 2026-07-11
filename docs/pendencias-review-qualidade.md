# Pendências do review de qualidade de código

> **Nota:** este arquivo é o registro do trabalho de **review de qualidade**
> (sessão de code-review). É separado do `pendencias.md` da raiz (o log de
> trabalho de 151KB do projeto) para não colidir com ele. Aqui ficam: o que foi
> entregue, o que foi **deliberadamente adiado** e por quê, e as **dúvidas de
> produto** que aguardam decisão do dono.

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

### 2.1 Refactor mecânico `erro()`/`lock_store()` (~96 sítios em `btv_agent.rs`)
DRY cosmético de **alto churn** e baixo valor: colapsar os ~65 blocos
`(...).into_response()` e os ~31 `lock().unwrap_or_else(|e| e.into_inner())` num
helper. **Risco desproporcional:** os corpos de erro são pinados **byte-a-byte**
por golden (`squad_activation_errors.golden.json`); qualquer drift quebra o CI, e
os 96 sítios são varredura manual propensa a erro. O ganho é só de legibilidade.
Fica catalogado; se feito, é um PR isolado com os goldens como rede.

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

### 2.3 Restrição numérica do hash `prompt-cache-key.v1` (decisão de contrato)
`hashing.py`/`canonical.rs` documentam (só em prosa) que floats com fração zero
(`1.0`) são proibidos no v1 — mas nada os rejeita, então um produtor JS (`1`) e
o Python/Rust (`1.0`) divergem silenciosamente na chave de cache. Adicionar um
guard que **rejeite** muda comportamento e toca o **contrato**: exige ADR novo +
regenerar `schemas/fixtures/` + os 2 testes de paridade verdes. É trabalho de
contrato com decisão de produto — **ver §3**.

### 2.4 Baixa severidade remanescente
O restante do `docs/REVIEW-AUDITORIA-COMPLETA.md` (dos 239 achados, após os
entregues acima) é majoritariamente nit de baixa severidade: magic numbers/strings
soltos, naming, DRY menor por tela/módulo, ternários aninhados no front, `now_rfc3339`
reimplementado por crate, `current_dir` repetido em 5 handlers do `web_agent`.
São melhorias incrementais de manutenibilidade sem impacto de correção/segurança —
melhor tratadas em lotes pequenos por módulo do que forçadas num churn amplo.
Catálogo completo (arquivo:linha) já está em `REVIEW-AUDITORIA-COMPLETA.md §3`.

## 3. Dúvidas de produto — aguardam sua decisão

1. **Guard numérico do hash (§2.3):** quer que eu abra um ADR + implemente a
   rejeição de `1.0`/NaN/Inf no `prompt-cache-key.v1` (com regeneração de
   fixtures e paridade)? É a única pendência com impacto de **correção**
   (cache-miss cross-produtor silencioso), mas mexe em contrato.
2. **Refactor mecânico `erro()`/`lock_store()` (§2.1):** vale o PR de baixo valor
   / alto churn, ou deixamos como está? (Recomendação: deixar.)
3. **Defaults de config agora env-ováveis (#60):** os defaults preservados
   (`BTV_PG_MAX_CONNECTIONS=4`, `BTV_WEB_MAX_STEPS=30`, `BTV_WEB_MAX_TOKENS=4096`,
   `BTV_WEB_CONTEXT_WINDOW=200000`) fazem sentido para o seu alvo de deploy, ou
   quer outros defaults?
4. **Splits estruturais (§2.2):** confirma que ficam presos ao gatilho "segundo
   consumidor" de `pendencias.md:2182` (recomendação), ou quer antecipá-los?

Enquanto não houver decisão, o estado atual é coerente e verde — nada aqui bloqueia
o funcionamento; são escolhas de evolução.
