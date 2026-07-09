# ADR 0024 — mapa de contextos DDD e classificação Core/Supporting/Generic

- Status: proposta (aguardando revisão humana — portão G0 do plano DDD multitenant)
- Data: 2026-07-09

## Contexto

O produto vai evoluir para SaaS multitenant sem abandonar o modo local-first
(`PLANO-DDD-MULTITENANT.md`, decisões D1–D5). Hoje as fronteiras do workspace
são **técnicas**, não de negócio: `btv-server` é transporte, `btv-store` é
persistência, `btv-cli` é agregação — e as rotas do produto BuildToValue vivem
em `crates/btv-cli/src/btv_agent.rs` (1373 linhas) enquanto o ledger é servido
por `crates/btv-server/src/lib.rs`. Para saber ONDE o `tenant_id` corta (ADR
0025), quais repositórios nascem primeiro (ADR 0026) e o que é investimento
central versus commodity, é preciso um mapa de contextos explícito. Este ADR o
registra; ele é o critério que o lint arquitetural de CI
(`scripts/arch-lint.sh`, Trilha T4) e o futuro crate `btv-domain` passam a
seguir.

## Decisão 1 — os subdomínios e sua classificação

| # | Contexto | Onde vive hoje | Classe |
|---|---|---|---|
| 1 | **Execução de squads** — orquestração, consenso, HITL, cockpit | `python/packages/btv-squad` (UnifiedOrchestrator), `crates/btv-sidecar`, `crates/btv-cli/src/{squad,squad_agent}.rs`, proto `btv.squad.v1` | **Core** |
| 2 | **Produto BTV** — runs, entregas, personas, templates, publicação | `crates/btv-store/src/btv.rs` (6 tabelas), `crates/btv-cli/src/btv_agent.rs`, `crates/btv-server/src/btv.rs` (catálogo embutido) | **Core** |
| 3 | **Ledger / auditoria** — trilha append-only com hash-chain | `crates/btv-store/src/ledger.rs`, `crates/btv-schemas/src/ledger.rs` | **Core** |
| 4 | **PromptForge** — biblioteca/render/cache de prompts | `python/packages/btv-promptforge`, `crates/btv-store/src/{prompt_library,prompt_cache}.rs`, proto `btv.promptforge.v1` | Supporting |
| 5 | **Verificação e review** — `/verify`, vetting, certificação | `crates/btv-verify`, `python/packages/{btv-review,btv-eval}` | Supporting |
| 6 | **Identidade e permissões** — perfis locais + PIN, permission engine, guarda de Origin | `users` em `btv-store/src/btv.rs`, `crates/btv-core/src/permission.rs`, `crates/btv-store/src/rule_store.rs` | Supporting (vira a porta de entrada do tenant no modo SaaS — Trilha E) |
| 7 | **Memória / recall do squad** — TF-IDF léxico, MemoryService | `btv_squad/{memory,recall}.py`, proto `btv.memory.v1` (ADRs 0013/0022) | Supporting |
| 8 | **Telemetria, experimentos e admin** | `crates/btv-store/src/telemetry.rs`, `crates/btv-schemas/src/{telemetry,experiment}.rs` | Generic |
| 9 | **Gateway LLM** — providers, rate limit, cache por hash | `crates/btv-llm` (keys SÓ aqui — ADR 0001, inegociável) | Generic |

Classificação orienta investimento: os três **Core** recebem primeiro os tipos
de domínio (Trilha A), os repositórios (Trilha B) e o corte de tenant. Generic
não ganha modelagem DDD — ganha, no máximo, `tenant_id` em colunas quando a
Trilha E precisar de medição por tenant.

## Decisão 2 — unificação da convenção de nomes

A convenção existente (CLAUDE.md: código e comentários em português,
identificadores em inglês) vira lei única com uma fronteira precisa:

- **Identificadores NOVOS de código** (crates, traits, newtypes, funções):
  inglês — `TenantId`, `TenantContext`, `RunRepository`, `DomainEvent`.
- **Campos de contrato serializado que JÁ são português** (`nome`, `cor`,
  `papeis`, `briefing`, `etapa`, `instrucao`, `formato`, `trilha` — presentes
  em `squad-template.v1`, nas tabelas de `btv.rs` e nas respostas HTTP agora
  congeladas pelos golden tests de T1): **permanecem em português**. Mudá-los
  seria mudança breaking de API/fixture sem ganho de domínio — exatamente o
  que a Trilha T existe para impedir por acidente.
- Tipos novos de domínio que ENVOLVEM esses contratos usam o nome do contrato
  no campo serializado (`#[serde(rename)]` quando o identificador interno
  divergir), nunca renomeiam o wire format.

## Não-escopo explícito

- **Não reorganizar os serviços gRPC por contexto** (`core/llm/squad/
  promptforge/memory.proto`): o contrato atual funciona e o custo de um
  re-corte agora é retrabalho puro. Fica registrado como evolução futura
  possível DESTE mapa; até lá, o gate `buf breaking` (Trilha T5) impede
  regressão silenciosa de contrato.
- **Não mover código entre crates nesta fase**: o mapa é conceitual; a
  decomposição física é a Trilha C, endpoint a endpoint (D5), nunca por
  arquivo.
- **Não tocar no Designer BPMN (U5) nem em Personas (U7) além da tipagem** —
  áreas ainda em fluxo (cautela herdada do levantamento); extrair contexto de
  área instável gera retrabalho.

## Consequências

- O lint T4 ganha seu critério: `btv-domain` (Trilha A) pertence aos contextos
  Core e não pode importar infraestrutura; handler HTTP não contém SQL.
- Os ADRs seguintes referenciam contextos deste mapa, não crates: 0025 corta
  tenant nos três Core; 0026 dá repositórios ao Produto BTV e ao Ledger; 0027
  é específico do contexto Ledger.
- A fronteira Rust×Python (ADR 0001) e os ports Python existentes (ADR 0005 —
  `GatewayClient`/`PermissionClient`/`ToolClient` são Protocols) permanecem
  intactos: o mapa não os redesenha, os reconhece.
