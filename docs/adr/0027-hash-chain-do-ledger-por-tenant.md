# ADR 0027 — hash-chain do ledger POR tenant, com tenant dentro do hash

- Status: proposta (aguardando revisão humana — portão G0 do plano DDD multitenant)
- Data: 2026-07-09

## Contexto

A cadeia atual é GLOBAL: tabela `ledger (seq INTEGER PRIMARY KEY
AUTOINCREMENT, prev_hash, entry_hash, body)` em
`crates/btv-store/src/ledger.rs`; `entry_hash = sha256(prev_hash + JSON
canônico do corpo)` (`LedgerEntry::chain_hash`,
`crates/btv-schemas/src/ledger.rs`); o append roda sob `BEGIN IMMEDIATE`
(lock de escrita ANTES do `SELECT` do último hash — correção de corrida real,
provada por teste com 6 threads); `verify_chain()` percorre a tabela inteira.
`kind` é string livre (~23 kinds no código, 8 deles `btv.*`).

Em multitenancy uma cadeia global é inaceitável por dois motivos (decisão D3
do plano): (a) o cliente A infere atividade do cliente B pela própria
sequência — cada `seq`/`prev_hash` que "pula" é vazamento de metadado; (b)
exportar a trilha auditável de um tenant exigiria entregar hashes de entradas
de outros tenants para a cadeia fechar. E há um terceiro problema que a chave
composta sozinha NÃO resolve: se o tenant vive só em coluna, fora do corpo
hasheado, uma entrada pode ser **transplantada** de uma cadeia para outra
(reatribuída de tenant) sem quebrar hash nenhum — a coluna muda, o corpo
canônico não.

## Decisão

1. **Chave da cadeia `(tenant_id, seq)`** — `seq` monotônico POR tenant;
   `prev_hash` encadeia DENTRO do tenant (primeira entrada de cada tenant:
   `prev_hash = ""`). O `BEGIN IMMEDIATE` existente continua serializando o
   read-modify-write; a serialização passa a ser por tenant no Postgres
   (lock/advisory por tenant), global-mas-barata no SQLite local (um tenant
   só).
2. **Tenant DENTRO do corpo hasheado** (endurecimento aceito na revisão do
   G0): `LedgerEntry` ganha `tenant: Option<TenantId>` serializado no corpo, e
   `hash_body()` passa a incluí-lo quando presente. Reatribuir a entrada a
   outro tenant muda o corpo canônico ⇒ quebra `entry_hash` ⇒ `verify_chain`
   detecta. A coluna `tenant_id` vira índice/particionador; a VERDADE
   auditável fica no hash.
3. **`verify_chain(tenant)`** — verifica UMA cadeia, do primeiro `""` ao
   topo; `POST /api/ledger/verify` ganha escopo de tenant (no modo local,
   `TenantId::LOCAL` — mesma resposta de hoje, contrato protegido pelo golden
   T1).
4. **Export por tenant** — a trilha exportada de um tenant é uma cadeia
   completa e verificável ISOLADAMENTE (auditoria portátil do cliente), sem
   um único hash de outro tenant.
5. **Migração do legado local** — backfill da coluna com `TenantId::LOCAL`
   (UUID fixo, ADR 0025). As entradas ANTIGAS não têm `tenant` no corpo
   (`Option` ausente ⇒ corpo canônico byte-idêntico ao gravado) — **os hashes
   existentes permanecem válidos sem re-hash**, e a cadeia local vira a
   cadeia do tenant LOCAL com `seq`/`prev_hash` intactos. Entradas NOVAS
   nascem com `tenant` no corpo. Honestidade registrada: a proteção
   anti-transplante do item 2 vale para entradas novas; o legado local é
   single-tenant por construção (não havia outro tenant para onde
   transplantar), então a lacuna é teórica — mas fica declarada, não
   escondida.

Append-only permanece lei: nada de UPDATE/DELETE; a migração é `ALTER TABLE
… ADD COLUMN` + backfill, sem tocar em corpo ou hash de linha existente.

## Não-escopo explícito

- **Taxonomia/enum de `kind`**: a string livre é dívida conhecida (o
  `LedgerKind` tipado é a tarefa A3 da Trilha A, com round-trip provado por
  property test T3) — este ADR não a resolve.
- **Assinatura criptográfica do export** (provar origem além de integridade):
  fora do escopo; o export do item 4 prova integridade da cadeia, não
  autoria.
- **Retenção/eliminação por tenant (LGPD)**: tarefa de produto da Trilha E
  (E4s) — apagar tenant inteiro remove a cadeia INTEIRA dele (o que o
  desenho por tenant torna possível sem quebrar as demais), mas a política é
  decisão jurídica, não deste ADR.

## Consequências

- Isolamento de metadados: nenhum tenant enxerga sequência, volume ou hash de
  outro; incidentes de auditoria ficam isoláveis por cliente.
- `LedgerRepository` (ADR 0026) nasce com `append(ctx, …)`/
  `verify_chain(ctx)`/`export(ctx)` recebendo `&TenantContext` — o
  fail-closed do ADR 0025 cobre também a trilha auditável.
- O corpo canônico ganha um campo novo opcional: os property tests T3 e a
  paridade de fixtures (`schemas/fixtures/`) precisam provar que entradas SEM
  tenant continuam produzindo exatamente os hashes atuais — é o que mantém o
  modo local byte-compatível.
- Custo aceito: `verify_chain` global (todas as cadeias de uma instância
  SaaS) vira um loop por tenant — mais lento que a varredura única de hoje,
  irrelevante no modo local (um tenant) e paralelizável no SaaS.
