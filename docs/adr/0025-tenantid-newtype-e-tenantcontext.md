# ADR 0025 — `TenantId` newtype e `TenantContext` fail-closed (modo local = tenant fixo)

- Status: proposta (aguardando revisão humana — portão G0 do plano DDD multitenant)
- Data: 2026-07-09

## Contexto

Não existe conceito de tenant em lugar nenhum do workspace (grep por
`tenant`/`TenantId` só encontra um falso positivo em
`btv-verify/src/prompt_integrity.rs`). Toda tabela de `btv-store` — `runs`,
`deliverables`, `persona_overrides`, `users`, o próprio `ledger` — é
implicitamente single-tenant; a única identidade é o perfil local
(`BtvUser`, sem auth) e o `actor: String` do ledger. Para o SaaS, o retrabalho
mais caro que dá para EVITAR é adicionar tenant DEPOIS de extrair
repositórios: reabriria todas as traits, adapters e testes da Trilha B (é a
ponderação nº 2 registrada no plano). E os dois bugs clássicos de
multitenancy nascem de decisões de tipo: tenant-como-`String` vazando por
parâmetros trocados, e um `Default`/`Option` que mascara o esquecimento do
filtro.

## Decisão

Tenant desde o dia 1, no sistema de tipos, com o modo local como caso
particular — decisão D1 do plano, aqui detalhada:

```rust
// btv-domain/src/tenant.rs (crate novo da Trilha A — deps: serde, thiserror, uuid)

/// Identidade de tenant — newtype opaco sobre UUID. Sem `From<String>`/
/// `From<Uuid>` implícitos: construção só por parse validado ou pelas
/// constantes — um id solto não "vira" tenant por acidente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TenantId(uuid::Uuid);

impl TenantId {
    /// Tenant fixo do modo local-first / self-hosted single-tenant.
    pub const LOCAL: TenantId = TenantId(uuid::uuid!("00000000-0000-0000-0000-000000000001"));
}

/// Quem agiu — alimenta o ledger (hoje o `actor: String` com prefixos
/// `web:`/`btv-cli:`; o newtype absorve essa convenção sem mudar o wire).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActorId(String);

/// Contexto obrigatório de TODA operação de repositório.
/// NÃO implementa `Default` — impossível esquecer o tenant por omissão;
/// construir um contexto é sempre uma decisão explícita do chamador.
pub struct TenantContext {
    pub tenant: TenantId,
    pub actor: ActorId,
}
```

**Regra fail-closed (lei de assinatura):** nenhum método de repositório da
Trilha B existe sem `&TenantContext`. Esquecer o filtro de tenant deixa de
ser um bug possível em runtime e vira erro de compilação. O lint T4
(`scripts/arch-lint.sh`) já vigia o crate; a revisão G1 das assinaturas é o
segundo portão.

**O modo local É um tenant, não a ausência de um.** O produto local-first
atual (identidade do produto, ADR 0001) roda com `TenantId::LOCAL` fixo,
resolvido na borda (CLI/`btv dashboard`) — o MESMO caminho de código serve os
dois modos, zero fork. O UUID `…0001` fixo torna a migração dos bancos locais
existentes um backfill determinístico (ADRs 0026/0027).

## Não-escopo explícito

- **Nenhuma migração de schema neste ADR** — colunas `tenant_id` entram com
  os adapters da Trilha B (ADR 0026), sob a suíte de contrato.
- **Nenhuma resolução de tenant por HTTP/auth** — sessão→`TenantContext` é o
  Contexto de Identidade da Trilha E (E1s), que só abre após o portão G2. Até
  lá, toda borda constrói `TenantContext` com `LOCAL` explícito.
- **`max_autonomy_level` continua descope** (ADR 0021) — este ADR não o
  ressuscita.

## Consequências

- O compilador vira o guarda de isolamento: assinatura sem contexto não
  existe, contexto sem tenant não constrói.
- `ActorId` tipado prepara o `DomainEvent` (Trilha A5: todo evento com
  `tenant` + `actor` obrigatórios) sem quebrar o `actor: String` do ledger no
  wire — serialização preservada, provada pelos property tests T3 e pelos
  goldens T1.
- O self-hosted pequeno não paga o custo do multitenant: uma indireção de
  trait e uma constante. O cenário SaaS ganha isolamento por construção antes
  de existir uma única linha de Postgres.
