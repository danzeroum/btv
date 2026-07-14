# Roadmap de melhorias transversais — backend + frontend + testes/CI

> **Propósito.** Consolidar num só lugar as oportunidades de melhoria já
> dispersas pela documentação (análise crítica, mapeamentos, review de
> qualidade, handoff) mais os achados de uma varredura estática nova, num
> **backlog priorizado com status de governança explícito**.
>
> **Regra.** Este documento **não reabre decisões registradas**. Ele cataloga
> e prioriza; cada item carrega o status que a governança do projeto já lhe
> deu (adiado-com-gatilho, recusado-com-razão, gated). Itens que tocam
> **contrato / ADR / comportamento visível** exigem **merge do dono** e seguem
> o rito da campanha (um passo por PR, DoD + prova-que-morde —
> `PADROES-DA-CAMPANHA.md`).

## Fontes

- `docs/documentacao/diagramas/09-analise-critica.md` **§9.2** — a tabela
  curada de oportunidades de refatoração (fonte autoritativa).
- `docs/documentacao/mapeamentos/03-failure-modes.md` e
  `04-cobertura-de-testes.md` — lacunas de erro e de teste.
- `docs/pendencias-review-qualidade.md` **§2–3** — o que foi adiado/recusado e
  as decisões do dono na última auditoria.
- `docs/handoff/desenvolvimento/HANDOFF-BUILDTOVALUE.md` — o roadmap *forward*
  (produto novo) e os descopes explícitos a não portar.

## Legenda de status

| Símbolo | Significado |
|---|---|
| 🟢 | quick-win seguro (baixo risco, sem tocar contrato) |
| 🟡 | precisa ADR e/ou janela breaking (protos evoluem só aditivamente) |
| 🔒 | **gated** — gatilho de reabertura já registrado; não antecipar |
| ✅ | **já resolvido** — a varredura confirmou que a pendência caducou |

> **Nota de honestidade.** A varredura estática que alimentou este roadmap
> **corrigiu três itens que a documentação ainda listava como abertos** mas já
> estão fechados no código (ver §8). Preferiu-se registrá-los a apagá-los, para
> que a próxima leitura da doc antiga não os reintroduza.

---

## 1. Backend — higiene e arquitetura

| ID | O quê | Evidência | Esforço | Status |
|---|---|---|---|---|
| **B1** | Decompor `btv-cli` (composition root inchado) | `web_agent.rs` 2022 LOC, `btv_agent.rs` 1825 LOC; acoplamento eferente ~12 crates | Alto | 🔒 |
| **B2** | Remover RPCs mortos do `CoreService` | `AppendLedger`/`recall`/`remember` = `Unimplemented` em `crates/btv-sidecar/src/core_server.rs:116-128`; superados pelo `MemoryService` (ADR 0022) | Médio | 🟡 |
| **B4** | Resolver `btv-eval` vazio | `python/packages/btv-eval/src/btv_eval/__init__.py` (5 linhas, placeholder) | Médio | 🟡 |
| **B6** | Extrair guard de `Origin` duplicado | `require_local_origin` em `crates/btv-cli/src/web_agent.rs`, `crates/btv-server/src/lib.rs` e `crates/btv-server/src/guard.rs` | Médio | 🟡 |
| **B7** | Padronizar ponte async→sync | 3 estratégias em `btv-tools` + `rt.block_on` por op em `crates/btv-store/src/pg.rs` | Médio | 🟡 |
| **B8** | Auditar `let _ =` que possam engolir `Result` | 42 em `crates/btv-cli/src/`, ~18 em `btv-tools`, ~14 em `btv-sidecar` | Baixo | 🟢 |
| **B10** | Considerar mutex sem poisoning no session hub | `.lock().expect("...poisoned")` em `web_agent.rs` (idiomático, mas num serviço de longa duração o poisoning cascateia) | Baixo | 🟢 (opcional) |

**Detalhamento dos não-óbvios:**

- **B1 · `btv-cli` — gated, não esquecido.** É o candidato nº 1 a decomposição
  (§9.2), mas está **explicitamente gated no 2º consumidor do motor** (modo SaaS
  em processo separado / worker headless) — `pendencias.md:2182,2189-2190`. A API
  dupla do `BtvStore` é decisão B2 registrada (`pendencias.md:1829`); colapsá-la
  sem preservar o escopo LOCAL reintroduziria vazamento cross-tenant. Alinhado à
  trilha **C4** (ADR 0031: mover os módulos-roteadores para `btv-server`). **Não
  abrir backlog concorrente** — quando o 2º consumidor existir, a decomposição
  paga a si mesma e vira campanha própria.
- **B2 · RPCs mortos.** Direção errada (o Python nunca chama `AppendLedger`/
  `Recall`/`Remember`; a memória correta é o `MemoryService`, ADR 0022). Remover
  exige `.v2` + ADR porque **protos evoluem só aditivamente** hoje — casa bem com
  B4 e uma eventual janela de limpeza de contrato.
- **B4 · `btv-eval`.** Placeholder que **engana quem procura a avaliação**.
  Decisão binária: (a) implementar o feeder do `LearningRouter` prometido, ou
  (b) remover o pacote e apontar para `btv-schemas::experiment` + o handler de
  experimentos já existente em `btv-server`. A opção (b) é a mais honesta se não
  há dono para a avaliação contínua.
- **B6 · Guard de `Origin`.** Duplicação deliberada (para evitar `server→cli`).
  A cura registrada em §9.2 é um crate **`btv-web-edge`** mínimo com o guard e os
  DTOs de erro, consumido por ambos — e casa com a redefinição de fronteira do
  ADR 0031.
- **B7 · Ponte async→sync.** Documentar um **ADR de "padrão de ponte"** e
  considerar um helper compartilhado para Sandbox/MCP/PgStore, hoje com três
  estratégias distintas.
- **B8 · `let _ =`.** É o **único lugar onde um `Result` significativo *poderia*
  ser engolido** (o resto do tratamento de erro é `thiserror` em libs / `anyhow`
  no binário, sem swallowing sistemático). Sweep de robustez de maior valor:
  confirmar que cada sítio descarta só um envio de canal / handle de task, nunca
  um erro relevante.

---

## 2. Frontend — unificação e UX

| ID | O quê | Evidência | Esforço | Status |
|---|---|---|---|---|
| **F1** | Portar `primitives/` + `AsyncStatus` para `btv-web` e adotar `useAsyncAction` | `btv-web/src/hooks/useAsyncAction.ts` existe mas tem **zero adoção**; estados idle/loading/error/empty re-implementados à mão por tela (ex. `Minhas.tsx`) | Médio | 🟢 |
| **F2** | Tokenizar hexes hardcoded | ~100 hexes em `.tsx` (`#a54334`, `#f7e7e3`…) que furam os tokens de `btv-web/src/styles/global.css`; adicionar `--ok-bg`/`--err-bg`/`--warn-bg` | Baixo | 🟢 |
| **F3** | Robustecer build do submodule bpmn | `vendor/bpmn` pode vir não-inicializado; Designer depende dele via alias + `scripts/ensure-bpmn.mjs`; `@bpmn-react/*` fora do `package.json` | Baixo | 🟢 |
| **F4** | Trocar `window.alert/confirm` por UI in-app | `btv-web/src/state/SquadRunContext.tsx:209` (alert), `btv-web/src/components/screens/admin/Usuarios.tsx:75` (confirm) | Baixo | 🟢 |
| **F5** | a11y + e2e de UI fino | Wizard, Vivo/cockpit e Designer sem e2e de UI; faltam labels a11y | Médio | 🟢/🟡 |
| **F6** | Extrair primitives de API compartilhadas | `client.ts`/`useAsyncAction.ts`/`ApiError` copiados entre `web/` e `btv-web/` — `btv-web/src/api/client.ts:2` cita o original de `web/` | Médio | 🟡 |

**Detalhamento:**

- **F1 · maior alavanca de frontend.** O console `web/` **já resolveu este exato
  problema** com o primitive `AsyncStatus` + hook. Portá-lo para `btv-web` e
  adotar o `useAsyncAction` já presente elimina a maior parte dos **380 usos de
  `style={{}}` (em 19 arquivos)** e unifica o tratamento de estado — hoje cada
  tela re-inventa idle/loading/error/empty com `useState`/`.then`/`.catch`.
- **F2** respeita o `btv-web/src/brand-lint.test.ts` (o guardrail que garante que
  a terracota `--decision` só aparece em telas de gate/aprovação).
- **F3** — o job CI `btv-web` já faz checkout **com submodules**; a fragilidade é
  para clone local sem `git submodule update --init`. Basta documentar/robustecer
  (o `ensure-bpmn.mjs` pode falhar fechado com mensagem clara).
- **F6** casa com o item §9.2 "dois frontends com padrão duplicado". Um pacote
  interno compartilhado (client HTTP/SSE + tipos de DTO) remove o drift dos DTOs
  hoje espelhados à mão em cada SPA.

---

## 3. Testes & CI — cobertura e exercício

| ID | O quê | Evidência | Esforço | Status |
|---|---|---|---|---|
| **T1** | Medir cobertura no CI | Não há `tarpaulin`/`pytest-cov`/`lcov` (`04-cobertura-de-testes.md` §4.6) — o mapa hoje é presença/tipo, não % | Baixo | 🟢 |
| **T2** | Cobrir ramos de erro de alto risco | `04 §4.5`: `SessionHub` (timeout→Deny, 409 ator único), `inject_cockpit_context`, HITL expirado/emergency-stop | Médio | 🟢 |
| **T3** | Falha real de rede/timeout do `Gateway` | Hoje só mock (`04 §4.2`, risco "médio"); servidor HTTP fake lento | Baixo | 🟢 |
| **T4** | Fechar o "exercise gap" da Fase 4 | consenso→ledger wired+unit mas nunca exercitado e2e; fecho determinístico via `ScriptedGenerator` sem key | Baixo | 🟢 |
| **T5** | e2e de UI do `btv-web` | Wizard/Vivo/Designer sem e2e de UI (ver F5) | Médio | 🟢 |

> **Contexto honesto (do próprio mapa §4.6):** o CI **não** roda cobertura, mas
> roda `verify` (self-hosting) e `sandbox` (Docker real) — garantia de
> *comportamento* onde a % de linha não é medida. T1 troca os `⟨medir⟩` da doc
> por números reais, não substitui essas garantias.

---

## 4. Fechar loops abertos (maior valor de produto)

| ID | O quê | Evidência | Esforço | Status |
|---|---|---|---|---|
| **L1** | Designer → orquestrador | `crates/btv-schemas/src/workflow.rs:5`: o `squad.workflow.v1` é "salvo e validado", mas o squad "continua com os 5 agentes fixos, sem reescrita nesta [fase]". Infra de roster já existe (`PersonaSpec` em `btv_agent.rs:269-275`) | Alto | 🟡 |
| **L2** | Decidir `max_autonomy_level` | Campo trafega em `SquadTask` (`schemas/proto/squad.proto`, usado em `squad.rs`/`squad_agent.rs`) mas é **ignorado ponta-a-ponta** (ADR 0021) | Médio | 🟡 |

- **L1** fecha o loop mais citado do lado de produto (§9.2 + `LEVANTAMENTO-UI-DESIGNER.md`
  + HANDOFF): mapear o grafo salvo para um roster de `PersonaSpec` executável.
  Precisa ADR (passa a *aplicar* de verdade, mudando comportamento visível).
- **L2** é o loop aberto mais repetido (ADR 0021, `DECISOES.md`, HANDOFF).
  Decisão **binária do dono**: **wirear** a autonomia progressiva de verdade
  (`ProgressiveAutonomyManager`) **ou remover o campo** numa janela breaking —
  hoje ele é débito consciente que só confunde quem lê o proto.

---

## 5. Documentação

| ID | O quê | Status |
|---|---|---|
| **D1** | Drift de nomes `forge-*` → `btv-*` nas docs históricas (preservado de propósito como registro, mas é risco de leitura) | 🟢 nota-guia no topo do índice `docs/documentacao/README.md` — **não renomear** |
| **D2** | `PLANO-DDD-MULTITENANT.md` referenciado por ADRs 0024–0031 mas ausente (só sobra `MIGRACAO-DDD-ENCERRAMENTO.md`) | 🟢 registrar a lacuna ou reconstituir um stub apontando ao encerramento |
| **D3** | ADRs **0030** e **0032** ainda "proposta/proposto" — aguardam merge do dono (tocam contrato: evidência tipada no wire e guard do hash) | 🟡 pendência de governança, não de código |

---

## 6. Roadmap *forward* (produto novo) — apenas ponteiro

O `HANDOFF-BUILDTOVALUE.md` define o caminho de produto (squads de IA para
profissionais não técnicos), **que é produto novo, não continuação de plano
antigo**. Em resumo (sem re-especificar):

1. **Usuário como membro ativo do squad** — chat + RPC `AwaitUserTurn` (maior prioridade).
2. **Contratos `persona.v1` + `plan.v1`** + subsistema de export (DOCX/XLSX/PDF/MusicXML), editor de entrega Monaco, job worker (`SKIP LOCKED`).
3. **Governança** — HMAC por entrada no ledger, gates de 4 estados + piso crítico, kill-switch, separação produzir≠revisar≠aprovar, versionamento/expiração de template.
4. **Espinha operacional** — dep-graph/health/logging, ponderação de confiança de 4 fatores no consenso, Decision→ADR, modo Ollama local `$0`.

**Descopes explícitos a NÃO portar** (do HANDOFF): o loop de auto-promoção
L1–L5 do SquadIAds (reforça ADR 0021), roteamento winner-take-all, constantes de
marketing de governança, o core executável original do BuildToValue, circuitos
ZK/Noir de "decisões silenciosas".

---

## 7. Priorização consolidada

Trilha sugerida por risco/rito (respeitando os gates):

- **Onda quick-win (🟢, sem tocar contrato):** B8, B10, F1, F2, F3, F4, T1, T2, T3, T4, T5, D1, D2.
  - Maior valor imediato: **F1** (unificação de estado no frontend) e **B8** (sweep de robustez).
- **Onda com ADR / janela breaking (🟡):** B2, B4, B6, B7, F6, L1, L2, D3.
  - Agrupar B2+B4 numa mesma limpeza de contrato (`.v2` + ADR).
  - L1 e L2 são as de maior valor de produto; ambas dependem de decisão do dono.
- **Gated (🔒):** B1 — aguarda o 2º consumidor do motor. Não abrir agora.

**Rito.** Contrato/ADR/comportamento visível ⇒ **merge do dono**. Cada passo é
um PR com DoD explícita e prova-que-morde. A "bifurcação SaaS"
(`E1S-ESBOCO.md`, `C3-INVENTARIO-STRANGLER.md`) é a frente viva que engole os
gates de config (Q3) e, eventualmente, a decomposição de B1.

---

## 8. Itens que a varredura confirmou **já resolvidos** (doc desatualizada)

A documentação e/ou notas de exploração ainda mencionavam estes como abertos; a
leitura do código atual mostra que caducaram. Registrados para não serem
reintroduzidos:

| Item | Situação real |
|---|---|
| `FallbackChain` "morto" em `btv-llm` | ✅ **já removido** — `crates/btv-llm/src/provider.rs:40`: "`FallbackChain` foi removida (validação de pendencias.md): era código morto". |
| `btv_squad/forgetting.py` "código morto a remover" | ✅ **não existe** no tree atual (a memória viva é `memory_server.py`). |
| 2 `panic!("bug interno de glue")` em `btv-server/handlers/verify.rs` | ✅ **são código de teste** (`#[cfg(test)] mod tests`, linha 213) e o caminho de produção **já trata panic graciosamente** via `settle_verify_job` + `catch_unwind` + `panic_to_message`. Nada a fazer. |

> Lição: a auto-análise do projeto é honesta e detalhada, mas alguns itens de
> "dead code" já foram limpos sem baixa na doc. Vale um passe periódico de
> reconciliação doc↔código (ou um grep no CI para os símbolos declarados mortos).
