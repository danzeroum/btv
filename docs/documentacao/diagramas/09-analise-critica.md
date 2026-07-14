# 09 — Análise crítica: coesão, acoplamento e oportunidades

Análise sobre coesão, acoplamento e oportunidades de melhoria arquitetural, derivada da
leitura estática de todo o código.

---

## 9.1 Pontos fortes (alta coesão, baixo acoplamento)

- **Fronteira de linguagem impecável (ADR 0001):** keys/disco/rede só no Rust; Python só
  raciocina e só conhece o socket. Reforçada por *máquina* (`arch-lint` falha o build se
  rusqlite/axum/tonic/reqwest entrarem em `btv-domain`).
- **Inversão de dependência consistente** nas três linguagens: Rust via traits
  `LlmPort`/`ToolsPort`/`*Repository`; Python via `typing.Protocol`
  (`GatewayClient`/`PermissionClient`/`ToolClient`); ambos com test-doubles (`Scripted*`)
  e adapters reais (`Grpc*`/`Gateway`) intercambiáveis sem tocar no núcleo.
- **Contrato single-source** em `schemas/` com o único algoritmo duplicado
  (`prompt-cache-key.v1`) protegido por fixtures de paridade e `buf breaking`
  (aditivo-only).
- **Segurança por design:** permissões avaliadas no Rust (não contornáveis pelo Python),
  HITL fail-closed, ledger append-only com hash-chain por tenant (anti-transplante),
  sandbox Docker fail-closed, vetter de skills.
- **Honestidade "Nada Fake" codificada:** `fake_marker` no ledger, veredito "sem
  significância" no experiment, posições `inferida` na esteira, custo `None` quando não
  tabelado, código morto tratado sem fingimento — `forgetting.py` foi removido (código
  morto eliminado) e `btv-eval` permanece placeholder vazio explicitamente descopado.
- **Dualidade de storage sem fork:** SQLite e Postgres+RLS atrás dos **mesmos traits**,
  com a suíte `btv-contract` provando paridade (inclusive determinismo de hash).

---

## 9.2 Pontos de atenção para refatoramento

| Tema | Observação | Sugestão |
|---|---|---|
| **`btv-cli` inchado** | O composition root acumula web_agent/squad_agent/btv_agent + engines HTTP (arquivos de 1000+ linhas). A borda axum já migra para `btv-server` (trilha C4). | Concluir a extração dos routers de produto (`btv_agent`) e do motor SSE de squad para crates dedicados; `btv-cli` deveria ser só wiring. |
| **`CoreService` com RPCs mortos** | `AppendLedger`/`Recall`/`Remember` são `Unimplemented` (direção errada, superados pelo `MemoryService`). | Remover num `.v2` com ADR (protos evoluem só aditivamente hoje) para não confundir leitores. |
| **`max_autonomy_level` não-wireado** | Campo trafega em `SquadTask` mas é ignorado ponta-a-ponta (ADR 0021). Débito consciente. | Wirear a autonomia progressiva de verdade, ou remover o campo do wire numa janela de breaking. |
| **Duplicação deliberada do guard de Origin** | `require_local_origin`/`ErrorBody` duplicados entre `btv-server` e `btv-cli::web_agent` (para evitar `server→cli`). | Extrair um crate `btv-web-edge` mínimo com o guard e os DTOs de erro, consumido por ambos. |
| **Ponte async→sync repetida** | Três estratégias em `btv-tools` (thread+runtime, thread de sessão, `std::thread`+condvar) e `rt.block_on` por operação em `PgStore`. | Documentar um ADR de "padrão de ponte" e considerar um helper compartilhado para Sandbox/MCP. |
| **`btv-eval` vazio** | Placeholder que pode enganar quem procura a avaliação. | Implementar o feeder do `LearningRouter` prometido, ou remover e apontar para `btv-schemas::experiment`. |
| **Designer não aplica ao orquestrador** | `squad.workflow.v1` é "salvo e validado", mas o squad real ainda usa 5 agentes fixos. | Fechar o loop: mapear o grafo salvo para um roster de `PersonaSpec` executável (a infra de roster já existe em `SquadTask`). |
| **Dois frontends com padrão duplicado** | `web/` e `btv-web/` têm `api/client.ts` idêntico e o mesmo padrão Context+reducer. | Considerar um pacote compartilhado de client HTTP/SSE + tipos de DTO (hoje os DTOs são espelhados manualmente em cada SPA). |

---

## 9.3 Métrica qualitativa de acoplamento

- **Núcleo de domínio (`btv-domain`, `btv-schemas`):** acoplamento *aferente* alto (muitos
  dependem dele), *eferente* mínimo — exatamente o desejado para um núcleo estável.
- **`btv-cli`:** acoplamento eferente altíssimo (~12 crates) — esperado num composition
  root, mas é o candidato nº 1 a decomposição.
- **Fronteira gRPC:** acoplamento reduzido ao contrato `.proto` — Rust e Python evoluem
  independentes desde que o wire seja aditivo (garantido por `buf`).
- **Frontend:** acoplamento ao backend concentrado nas camadas `api/*` (tabelas de
  endpoints na [referência 14](../referencia/14-endpoints-http.md)), com DTOs espelhando os
  contratos serde — um só lugar para absorver mudanças de wire.

---

## 9.4 Padrões arquiteturais detectados

| Padrão | Onde |
|---|---|
| **Ports & Adapters (Hexagonal)** | `btv-domain::ports` (Rust) e `Protocol`s (Python); adapters `Gateway`/`ToolRegistry`/`*Store`/`Grpc*Client` |
| **Decorator** | `CachedGenerator<RateLimitedGenerator<Gateway>>` |
| **Repository** | `RunRepository`/`LedgerRepository`/… com dois adapters (SQLite/PG) |
| **DDD tático** | Agregado `Run` (única porta de mutação), value-objects (`TaskId`/`TenantId`/`RunStatus`), eventos de domínio, context map (ADR 0024) |
| **Strategy** | `AgentProfile` (política de permissão como `fn`), `Parser` do verify |
| **Supervisor / Object Pool** | `SquadSupervisor`/`SquadPool` (sidecar de longa duração) |
| **Event sourcing (parcial)** | `EventStore` com concorrência otimista; ledger append-only hash-encadeado |
| **Plugin** | `btvDesignerPlugin` sobre a lib agnóstica `@bpmn-react/*` |
| **Function core / imperative shell** | `esteiraFromEvents` (pura) alimentada pelo `SquadRunContext` (efeitos SSE) |

---

## 9.5 Conclusão

A arquitetura é **madura e coerente**: a regra de fronteira Rust/Python é o eixo que tudo
respeita, verificada por máquina; a inversão de dependência é aplicada uniformemente nas
três linguagens; os contratos são single-source e testados por paridade/goldens/`buf`. As
oportunidades de refatoramento são majoritariamente de **higiene** (decompor `btv-cli`,
limpar RPCs mortos, extrair o guard duplicado) e de **fechar loops declaradamente abertos**
(autonomia progressiva, Designer→orquestrador) — nenhuma delas é uma falha estrutural, e
todas já estão registradas como débito consciente no `docs/DECISOES.md`/`pendencias.md` e
nas ADRs correspondentes.
