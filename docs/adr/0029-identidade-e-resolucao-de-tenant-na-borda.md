# ADR 0029 — modelo de identidade e resolução de tenant na borda HTTP

- Status: proposta (aguardando revisão humana — abre a Trilha E, decisão
  assinada pelo dono no G2; nós não implementamos decisão não tomada)
- Data: 2026-07-10

## Contexto

O G2 atestou o fundamento multitenant (adapters duais, RLS adversarial,
ledger por tenant) e declarou explicitamente o elo ausente: **nenhuma rota
constrói um `TenantContext` a partir de uma sessão real** — os handlers de
`btv-server` operam implicitamente como o modo local. A identidade que
existe hoje é a do produto local: `BtvUser` com PIN opcional
(`sha256(salt|pin)` simples, declarado honesto para um dashboard em
`127.0.0.1` — não é um cofre de rede), perfis locais SEM auth (decisão da
Fase 7), e a guarda de `Origin`/`Host` (ADR 0015) protegendo rotas mutáveis
contra CSRF/DNS-rebinding. Nada disso identifica um TENANT numa requisição.

Este ADR decide o MODELO — quem é o chamador, de onde vem o tenant, o que
acontece sem ele — antes de qualquer código (E1s implementa o que for
aceito aqui). É deliberadamente o espelho HTTP das decisões que a Trilha B
já tomou embaixo: porta sem contexto = modo local; contexto de outro
tenant = recusa fail-closed.

## Decisão proposta

1. **`TenantContext` nasce na borda, uma vez por requisição, e desce por
   parâmetro.** Um extractor de autenticação (middleware axum) resolve a
   requisição em `TenantContext { tenant, actor }` ANTES do handler; o
   handler nunca monta contexto por conta própria. Nenhum estado global,
   nenhuma thread-local: o contexto é dado explícito, como as assinaturas
   do G1 já exigem dos repositórios.

2. **Resolução por modo** (`BTV_MODE`, ADR 0026 item 6):
   - **`local` (default):** o extractor resolve TODO request em
     `TenantId::LOCAL`, com `actor` derivado do perfil local ativo quando
     houver (`user:<id>`) ou do canal (`web:dashboard`) — comportamento
     de hoje, byte-idêntico (goldens T1 são os juízes). O PIN local
     continua o que sempre foi: conveniência de perfil, não autenticação.
   - **`saas`:** a requisição carrega um **token de sessão opaco**
     (cookie `HttpOnly`+`SameSite=Strict` ou header `Authorization:
     Bearer`), emitido por um fluxo de login fora deste ADR. O token
     resolve, via storage do modo saas, para `(tenant_id, user_id)` →
     `TenantContext`. O tenant vem SEMPRE da sessão autenticada — NUNCA de
     um header/query/JSON manipulável pelo cliente (`X-Tenant-Id` é
     explicitamente vetado: seria o transplante do B3 em versão HTTP).

3. **Fail-closed no modo saas, sem exceção nas mutáveis:** rota mutável sem
   sessão autenticada válida = **401/403, nunca fallback para LOCAL** — o
   espelho HTTP da decisão da porta legada do B2 (a porta sem contexto é a
   porta do modo local, e o modo saas NÃO tem porta sem contexto). Rotas de
   leitura seguem a mesma regra; as únicas exceções nomeadas são o health
   check e os assets estáticos do SPA (conteúdo público do binário). A
   guarda de `Origin`/`Host` do ADR 0015 permanece por baixo, inalterada.

4. **Sessões do modo saas são estado do servidor** (tabela `sessions` no
   Postgres, com RLS como as demais): token opaco aleatório com hash em
   repouso, expiração absoluta + ociosidade, revogação por linha (logout
   real). JWT auto-contido fica registrado como alternativa REJEITADA
   nesta fase: revogação imediata vale mais que stateless para um produto
   com HITL e permissões ao vivo, e evita a classe inteira de bugs de
   validação de assinatura/claims.

5. **A tabela de sessões é a exceção LEGÍTIMA E ÚNICA à regra "toda query
   é tenant-escopada" — nomeada, com política própria.** O lookup que
   resolve o token acontece ANTES de existir `TenantContext`, então essa
   tabela não pode ser protegida por `app.tenant_id` como as outras. A
   política que a substitui: (a) acesso SÓ por igualdade com o hash do
   token apresentado — nenhuma query enumera, lista ou filtra sessões por
   qualquer outro predicado no caminho de autenticação; (b) o token em si
   nunca é persistido (só o hash), então nem um dump da tabela autentica
   ninguém; (c) toda operação ADMINISTRATIVA sobre sessões (listar as
   sessões do usuário, revogar em massa) acontece DEPOIS da autenticação e
   é tenant-escopada como qualquer outra. Qualquer outra tabela que um dia
   precise de leitura pré-contexto passa por ADR próprio — esta exceção
   não é categoria, é UMA linha nomeada.

6. **O ator é parte da identidade:** `actor = user:<uuid>` do usuário
   autenticado — entra nos eventos de domínio e no ledger como já hoje
   (`DomainEvent.actor`), dando trilha por pessoa DENTRO do tenant.

## Não-escopo explícito

- **UI de login / cadastro / reset de senha** — E1s entrega o extractor,
  o modelo de sessão e as recusas; o fluxo visual vem depois.
- **Billing, planos, quotas por tenant** — Trilha E posterior.
- **SSO/OIDC** — a estrutura (token opaco → contexto) não o impede;
  decisão futura com caso de uso real.
- **Multi-tenant por usuário (um usuário em N tenants)** — modelo desta
  fase é 1 sessão → 1 tenant; trocar de tenant = nova sessão.

## Consequências

- E1s ganha um contrato implementável e testável: extractor → contexto →
  repositórios já tenantizados (Trilha A/B) sem NENHUMA mudança abaixo da
  borda.
- O modo local não muda um byte de comportamento (critério de aceitação:
  goldens T1/T3 verdes sem regravação).
- Teste adversarial da borda entra na DoD da E1s, no espírito do B4:
  requisição forjando `X-Tenant-Id`/cookie de outro tenant não lê nem
  escreve NADA fora da própria sessão.
