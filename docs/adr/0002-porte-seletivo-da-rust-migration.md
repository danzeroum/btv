# ADR 0002 — Porte seletivo da branch `rust-migration` do opencode

- Status: aceita
- Data: 2026-07-05

## Contexto

A branch `rust-migration` do fork `danzeroum/opencode` contém uma migração
incremental (strangler-fig) do backend TypeScript do opencode para Rust:
~40 mil linhas em 14 crates, convivendo com o monorepo TS no mesmo SQLite
("TS migra, Rust verifica"), com paridade de contrato OpenAPI e cutover
rota a rota. O usuário pediu avaliação sobre copiá-la integralmente para
este repositório.

## Decisão

**Não copiar a branch inteira; portar módulos selecionados.** A cópia
integral traria o monorepo TS do opencode, um segundo workspace cargo
conflitante e a maquinaria de coexistência TS↔Rust — um problema que o
Forge, greenfield, não tem. Os módulos coerentes com o Forge foram
portados e adaptados:

| Origem (rust-migration) | Destino no Forge | Adaptação |
|---|---|---|
| `opencode-db`/`opencode-events` — EventStore sqlx com schema `event`/`event_sequence`, índice único `(aggregate_id, seq)`, WAL + pragmas | `forge-store::events` | rusqlite (sync, coerente com o stack do CLI); sem journal de migrations do TS — o Rust é dono do schema; convenção de versão `nome.N` no `type` mantida |
| Modelos/repos de sessão (`session_input`, `session_context_epoch`) e `EVENTSTORE_PLAN.md` | `forge-core::session` (`DurableSession`) | sessão = agregado de eventos `message.1`; replay reconstrói o histórico; concorrência otimista detecta escritores simultâneos |
| `opencode-tools::grep` — busca com as bibliotecas do ripgrep (`grep` + `ignore`) | `forge-tools::grep` | mesma semântica de matching; mantidos `require_git(false)`, teto de ocorrências e caminhos relativos do Forge |
| `opencode-tools::files::edit_file` — `replace_all` com exigência de unicidade sem ele | `forge-tools::edit` | flag `replace_all` no schema da ferramenta |
| `deny.toml` + job cargo-deny do `rust.yml` | `deny.toml` + job `deny` no `ci.yml` | lista de licenças herdada (incl. CDLA-Permissive-2.0 do webpki-roots, que já usamos via reqwest/rustls) |
| Decisão TLS rustls+ring (`OWNER_DECISIONS.md` §4) | já adotada na Fase 1 (`reqwest` com `rustls-tls`) | — |

**Não portados (incoerentes com o Forge):** o seam de proxy reverso e o
gate `openapi-diff` (paridade com contrato HTTP legado que não temos — o
equivalente nosso é `buf breaking` nos protos), o verificador de journal
de migrations (não há segundo escritor de schema), e os crates
`opencode-server/effect/integration/proto` (acoplados ao contrato do
opencode).

## Consequências

- As sessões duráveis da Fase 2 nascem sobre um event store com design já
  validado na rust-migration, sem inventar schema novo.
- Context Epochs e compaction (próximos passos da Fase 2) serão eventos
  no mesmo agregado — `epoch.started.1`, `compaction.applied.1` — sem
  migração de schema.
- O CI ganha um gate de supply-chain (cargo-deny) igual ao da origem.
- A `rust-migration` permanece intocada no opencode como referência.
