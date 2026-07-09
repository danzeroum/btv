# ADR 0026 — dupla persistência: SQLite e Postgres+RLS atrás das MESMAS traits

- Status: proposta (aguardando revisão humana — portão G0 do plano DDD multitenant)
- Data: 2026-07-09

## Contexto

Toda a persistência hoje é `rusqlite` direto, sem trait no meio:
`LedgerStore` (WAL + `BEGIN IMMEDIATE` no append), `BtvStore` (6 tabelas do
produto em `.btv/btv.db`), `Telemetry`, `PromptLibrary`, `RuleStore`. Não há
`sqlx`/Postgres em lugar nenhum. Para o SaaS é preciso Postgres com isolamento
forte; mas o SQLite **é o modo local** — identidade do produto, não legado a
migrar. Tratar Postgres como "substituto" (como propunha um dos levantamentos)
abandonaria o local-first; manter só SQLite inviabiliza o multitenant
operado. A decisão D2 do plano rejeita os dois extremos.

Uma restrição REAL do código atual entra no registro para não ser esquecida:
**não existe transação atravessando run+deliverable+ledger** — a ativação
(`btv_agent.rs`) faz `insert_run` e `append_ledger` como operações
independentes (falha de ledger só gera `eprintln!`), e o watcher grava cada
entrega + entrada de ledger separadamente. São dois arquivos SQLite distintos
com conexões próprias; a "atomicidade" atual é por boa vontade.

## Decisão

**Dois adapters permanentes atrás das mesmas traits de repositório — não uma
migração de banco.**

| Modo | Adapter | Isolamento de tenant |
|---|---|---|
| Local-first / self-hosted pequeno | `Sqlite*Repository` (o código atual de `BtvStore`/`LedgerStore` refatorado) | `TenantId::LOCAL` fixo; arquivo, WAL e comportamento idênticos aos de hoje |
| SaaS multitenant | `Pg*Repository` (novo, `sqlx` + migrations versionadas) | coluna `tenant_id NOT NULL` em toda tabela **+ Row-Level Security** (policy por tabela sobre `current_setting('app.tenant_id')`) — defesa em profundidade: mesmo um SQL com bug não vaza entre tenants |

1. **Traits no domínio** (`btv-domain`, Trilha B1): `RunRepository` (agregado
   Run+Entregas, transacional), `PersonaRepository`, `LedgerRepository`,
   `EventStorePort` — **todo método recebe `&TenantContext`** (ADR 0025).
   Assinaturas passam por revisão humana (portão G1) ANTES de qualquer
   implementação.
2. **Suíte de contrato dual-adapter**: um ÚNICO conjunto de testes genéricos
   sobre as traits roda contra os dois adapters — é o contrato que garante
   paridade de comportamento, no espírito da paridade de hash Rust×Python que
   o repo já pratica (`schemas/fixtures/`). Postgres no CI via container
   (mesmo padrão do job `sandbox`, que exige o daemon Docker de verdade);
   localmente sem Postgres a metade PG pula com aviso barulhento, nunca passa
   fingindo.
3. **Teste adversarial de RLS** (Definição de Pronto de B4): conexão com
   `app.tenant_id = A` não lê linhas do tenant B nem com SQL adulterado.
4. **Migração do legado local**: colunas `tenant_id` entram no adapter SQLite
   com backfill `TenantId::LOCAL` (UUID fixo do ADR 0025) — determinística,
   sem perda; o modo local continua byte-compatível (E2E antigos + goldens T1
   são a prova).
5. **A transação que falta**: `RunRepository` nasce transacional para
   run+entregas (mesmo arquivo/conexão). A atomicidade run↔ledger — dois
   stores, hoje dois arquivos — NÃO é prometida aqui: registrada como
   restrição conhecida, a resolver por outbox/idempotência quando a Trilha B
   desenhar o `EventStorePort`. Prometer atomicidade cross-store agora seria
   fake.
6. **Config de modo** (B5): `BTV_MODE=local` (default — SQLite, tenant LOCAL,
   comportamento idêntico ao atual) vs `BTV_MODE=saas` (PG, tenant do
   contexto de auth da Trilha E); `btv doctor` reporta o modo.

## Não-escopo explícito

- **Nada de `sqlx`/Postgres nesta semana** — este ADR autoriza o desenho; a
  implementação é a Trilha B, gated por G1 (assinaturas) e G2 (go/no-go SaaS
  com RLS provada).
- **`Telemetry`/`PromptLibrary`/`RuleStore`** ficam fora da primeira leva de
  traits: contextos Supporting/Generic (ADR 0024) só entram quando a Trilha E
  precisar de medição por tenant.
- **Nenhuma promessa de HA/replicação/backup gerenciado** — `infra/` continua
  esqueleto honesto (Fase 6).

## Consequências

- O self-hosted pequeno mantém exatamente o que tem: um arquivo SQLite, WAL,
  zero serviço externo — agora coberto para sempre pela mesma suíte de
  contrato que valida o modo SaaS.
- Os goldens T1 (já na main antes de qualquer refatoração — lei da Trilha T)
  protegem o contrato HTTP durante a troca do miolo por trait.
- Custo aceito: manter dois adapters é manutenção dupla no CRUD — pago
  conscientemente porque cada um É um modo do produto, não um estágio de
  migração.
