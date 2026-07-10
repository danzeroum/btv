# Migração DDD multitenant — encerramento

Reconciliação do levantamento de julho de 2026 com o estado real ao fim da
campanha. Cada item do diagnóstico recebe um veredito — **fechado**,
**redefinido** ou **diferido** — com o PR e/ou ADR que o resolveu, e os desvios
contados com suas razões. Tom factual: os números falam sozinhos.

- **Escopo:** PRs #15–#56 na branch `claude/ddd-multitenant-migration-gljv4t`.
- **Invariante da campanha:** wire local byte-idêntico do primeiro ao último
  dia; zero regressão de contrato (os goldens de HTTP/ledger nunca regravaram
  por acidente, só por regravação declarada e auditável).

## O diagnóstico de julho, item a item

| # | Diagnóstico (julho) | Veredito | Onde se resolveu |
|---|---------------------|----------|------------------|
| 1 | **Nenhum conceito de tenant em lugar nenhum** — nem tipo, nem isolamento | **fechado** | `TenantId` newtype + `TenantContext` sem `Default` (A1/A2, #17; ADR 0025); isolamento fail-closed na leitura provado pela suíte de contrato (B2, #23) |
| 2 | **God Crate**: `btv-cli` é CLI **e** servidor HTTP, com ativação+gates+ledger+persona+entrega no mesmo arquivo | **fechado** (emissão/persistência) + **redefinido** (axum-no-CLI) | Estrangulamento endpoint a endpoint pela porta de domínio (C3.1–C3.4, #31–#49); `append_ledger` **deletado** (#49). A parte "axum no CLI" foi **redefinida** — ver item 9 |
| 3 | **Cadeia de ledger global única** `(seq, prev_hash, entry_hash)` | **fechado** | Cadeia **por tenant** `(tenant_id, seq)`, tenant no corpo hasheado, `verify`/`export` por tenant (B3, #24; ADR 0027) |
| 4 | **Persistência única** (só SQLite) | **fechado** | Dois adapters permanentes sobre as MESMAS traits: SQLite (#23) e Postgres+RLS (#25; ADRs 0026/0028), julgados pela suíte de contrato única + teste adversarial de RLS |
| 5 | **Sem fronteiras DDD** (domínio, portas, agregados) | **fechado** | Crate `btv-domain` (A, #17); value objects `RunStatus`/`TaskId`/`LedgerKind`/`Briefing` (#21); agregado `Run` com `approve_gate`/`transition_to` reais (#22); portas de repositório (G1, #20) |
| 6 | **Sem transação atravessando run+deliverable+ledger** | **parcial / diferido** | `save_with_deliverables` torna run+entrega atômicos na forma da API (B2, #23). A atomicidade com o **ledger** continua fora — restrição declarada no ADR 0026 (item 5): outbox/idempotência é decisão de Trilha B futura |
| 7 | **Python sem tipo de tenant** — strings soltas no wire | **fechado** | `tenant_id`+`actor` no `SquadTask`, ecoados VERBATIM em todo `SquadEvent` (D2t, #28); `TenantContext` Pydantic parse-don't-validate (D4t, #29) |
| 8 | **Gateway LLM / runtime acoplado a concretos** | **fechado** | `btv-core` só-portas (`LlmPort`/`ToolsPort`), loop testável com mocks <100ms (D1t, #27); arch-lint T4-C garante mecanicamente |
| 9 | **btv-cli importa axum** (era a métrica de "separar CLI de servidor") | **redefinido** | Os 4 consoles-folha migraram a `btv-server` (C4-1..4, #51–#55). Os 3 "grandes" são o **MOTOR** do produto (agent-loop, squad, borda), não roteadores — a fronteira real é **console/dashboard vs motor**, não axum vs CLI. **ADR 0031**, guarda **T4-E** ativo (#56) |

## Portões humanos (rito de aceite)

| Portão | O que decidiu | PR |
|--------|---------------|----|
| **G0** | ADRs 0024–0028 aceitos com citação (mapa de contextos, TenantId, dupla persistência, ledger por tenant, serialização PG) | #26 |
| **G1** | Assinaturas das traits + esqueleto do agregado, revisão humana SEM impl | #20 |
| **G2** | Go/no-go SaaS: checklist postado, aceite do dono | #26 |
| **G3** | Janela de deploy coordenado: **D3t** (o único breaking de wire, assinado; #50) + **C4** (consolidação, #51–#56) | #50, #56 |
| **ADR 0029** | Identidade/resolução de tenant na borda (E1s) | #28 |
| **ADR 0031** | Redefinição da fronteira (console vs motor), fecho do C4 | #56 |

## Desvios declarados (o que NÃO se fez pelo caminho reto, e por quê)

- **C3.4 agrupou `export_generated` + `user_removed` por FORMA DE TRABALHO, não
  por costura** (#48/#49) — desvio declarado para que o commit final da onda
  deletasse `append_ledger` e cumprisse a profecia do próprio rustdoc.
- **`skills` NÃO migrou para `btv-server`** (#54) — o recon corrigiu a premissa:
  `skills.rs` é código de agent-loop (build do `ToolRegistry`), não console. Só
  os **leitores de config** multi-consumidores foram extraídos para `btv-tools`
  (dono do tipo), destravando lsp/mcp.
- **Os 3 grandes NÃO foram movidos nem decompostos** (#56, ADR 0031) — são o
  motor do produto; movê-los arrastaria agent-loop/sidecar para o crate de
  dashboard. O T4 foi **redefinido** em vez de cumprido ao pé da letra.
- **`template_pub`/`users` diferidos do schema PG** até uma porta os servir
  (B4) — evita tabela órfã sem consumidor; entraram com as portas (#46/#48).
- **`max_autonomy_level` e `btv_squad/forgetting.py`** — código morto confirmado
  por grep, descopados explicitamente (não fingir feature que o orquestrador
  ignora ponta-a-ponta; ADR 0021).

## O que fica (decisões futuras com dono, não sobras)

Registradas com gatilho em `pendencias.md`:

1. **Decomposição dos 3 grandes** (motor → crate abaixo, routers finos em cima).
   Gatilho: quando o motor precisar de um SEGUNDO consumidor (ex.: modo saas num
   processo separado do CLI). Até lá, custo sem comprador. Campanha própria, com
   seu G0.
2. **Atomicidade run+deliverable+LEDGER** (outbox/idempotência) — Trilha B
   futura (ADR 0026 item 5).
3. **Tipar o miolo do `btv-review`** (`gates.py`/`certification.py` ainda com
   `dict[str, Any]` pós-validação) — quando o review ganhar evolução funcional
   (residual do D3t).
4. **Pacote de lançamento SaaS** (login, `ROTAS_LIVRES`, Trilha E operacional) —
   **bifurcação de PRODUTO do dono**, não continuação desta campanha: tem custo
   real e só paga a si mesmo se houver clientes do outro lado. Nasce, quando e se
   o dono decidir, como esta nasceu — esboço, portões, rito.

## Números

- **42 PRs** (#15–#56), todos com merge humano.
- **8 ADRs** novos (0024–0031), todos aceitos com citação do portão.
- **1 breaking de wire** na campanha inteira (D3t), assinado no PR.
- **0 regressões de contrato** — goldens byte-idênticos, exceto regravações
  declaradas e isoladas (uma linha `tenant` por estrangulamento).
