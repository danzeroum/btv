# ADR 0028 — serialização do append por tenant no Postgres: UNIQUE + retry otimista

- Status: proposta (decisão diferida PELO ADR 0027 item 1 para a Trilha B4;
  aguardando o aceite do dono junto com o PR do B4)
- Data: 2026-07-10

## Contexto

O ADR 0027 definiu a cadeia do ledger como `(tenant_id, seq)` e deferiu
deliberadamente o MECANISMO que serializa o read-modify-write do append por
tenant no Postgres — "um ADR sobre hash-chain não é o lugar para se
comprometer de passagem com advisory locks". As duas alternativas
registradas lá: (a) `pg_advisory_xact_lock` sobre um hash do tenant;
(b) constraint `UNIQUE (tenant_id, seq)` + retry no conflito. No SQLite o
`BEGIN IMMEDIATE` global-mas-barato continua (um tenant só no modo local) —
esta decisão é exclusiva do adapter PG.

## Decisão

**(b) — `UNIQUE (tenant_id, seq)` + retry otimista.** O append lê o topo da
cadeia do tenant, encadeia o hash e insere; quem perde a corrida leva
SQLSTATE 23505, faz rollback e relê o topo NOVO (limite de tentativas alto e
declarado — esgotá-lo é erro explícito, nunca corrupção). Implementado em
`btv-store::pg` (`LedgerRepository::append`).

Razões, na ordem que pesou:

1. **Sobrevive a connection pooling em modo transação** (o deployment
   provável do SaaS — pgbouncer/transaction mode): locks de SESSÃO ficam
   presos à sessão do pooler, não à transação do chamador. A variante
   transacional (`pg_advisory_xact_lock`) não sofre disso, mas aí entra a
   razão 2.
2. **`pg_advisory_xact_lock` recebe `bigint`** — serializar por tenant
   exigiria hashear o UUID para 64 bits, e colisão de hash acoplaria
   tenants distintos no MESMO lock: ainda correto, mas uma serialização
   cruzada espúria que nenhum teste pegaria (só latência inexplicável em
   produção). A constraint não tem esse modo de falha: a unicidade é sobre
   o valor REAL do tenant.
3. **Sem estado fora da linha**: a correção mora na constraint declarada no
   schema — visível no `\d ledger`, coberta pelo RLS, sem protocolo
   implícito que um caller novo possa esquecer de seguir.
4. **Convive com RLS e com qualquer pooler** sem regra especial.

Custo aceito: sob contenção alta no MESMO tenant, o perdedor paga uma
transação perdida + releitura (o lock pagaria espera). Para o perfil de
escrita do ledger (eventos de governança, não hot path), o retry é barato e
o pior caso está limitado e explícito.

## Juiz

Como o 0027 exigiu: cadeias independentes provadas por teste com tenants
concorrentes — `pg::tests::appends_concorrentes_de_pools_separados_mantem_as_cadeias_por_tenant`
(pools/conexões SEPARADOS por thread, dois tenants intercalados, cadeias
1..N sem buraco e sem fork; a prova-que-morde do PR do B4 removeu o retry e
o teste reprovou com o 23505 cru — ou seja, a corrida é real e é o retry
quem a segura). A suíte de contrato (`suite_ledger_repository`) roda
idêntica sobre os dois adapters.

## Consequências

- O adapter PG não carrega nenhum lock explícito; qualquer evolução (ex.:
  particionamento por tenant) herda a serialização da constraint.
- Se um dia o perfil de escrita mudar para contenção contínua no mesmo
  tenant, a alternativa (a) na variante transacional continua registrada —
  a troca é local ao `append` e a MESMA suíte julga.
