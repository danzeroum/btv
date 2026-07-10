# E1s — esboço de execução (identidade e tenant na borda, ADR 0029 aceito)

Preparado com a onda C3.1 drenando; destrava no fechamento dela. Quebra em
PRs na disciplina de sempre (um passo por PR, DoD explícita, prova-que-morde).

## PR E1s.1 — tabela `sessions` + política do bootstrap (item 5)

Migration PG (`0002_sessions.sql`): `sessions(token_hash TEXT PK,
tenant_id TEXT NOT NULL, user_id TEXT NOT NULL, expires_at, idle_deadline,
created_ts)` — SEM RLS de tenant (a exceção única e nomeada), com a
política do item 5 no lugar: acesso de auth SÓ por igualdade de
`token_hash`; nada de enumeração no caminho pré-contexto. Métodos em
`btv-store::pg`: `resolve_session(token_hash) -> Option<(TenantId, UserId)>`
e as operações administrativas (listar/revogar) TENANT-ESCOPADAS.
DoD: teste adversarial da tabela (dump não autentica — só hashes; query de
auth não aceita outro predicado) + suíte de contrato se nascer port.

## PR E1s.2 — extractor na borda (modo local byte-idêntico)

Extractor axum que resolve TODO request em `TenantContext` ANTES do
handler — **peça ÚNICA** (um `FromRequestParts` num módulo próprio),
reutilizada por todos os consumidores: nenhum handler ganha `if
BTV_MODE` próprio; a resolução por modo vive SÓ no extractor. Seis ifs
espalhados seriam a versão de borda do SQL-em-handler que o T4-B proíbe —
e a cura é a mesma: peça única + juiz MECÂNICO. O lint **T4-D** nasce
junto com este PR (viabilidade confirmada: o T4-B já é grep-por-fronteira;
varrer `BTV_MODE` fora do módulo do extractor é o mesmo mecanismo), com
prova-que-morde na criação: um `if BTV_MODE` plantado num handler →
arch-lint reprova nomeando o arquivo. `BTV_MODE=local` (default): LOCAL implícito, actor por canal —
goldens T1/T3 verdes SEM regravação (o critério de aceitação do ADR).
`BTV_MODE=saas`: token opaco (cookie HttpOnly+SameSite=Strict ou Bearer)
→ hash → `resolve_session` → contexto; sem sessão = 401/403, NUNCA
fallback para LOCAL. Exceções nomeadas: health check + assets do SPA.
DoD: recusa fail-closed provada rota a rota (mutáveis E leitura).

## PR E1s.3 — a troca da fonte nos seis consumidores estrangulados

Os handlers da C3.1 trocam `TenantContext::local(...)` fixo pelo contexto
do extractor — a costura que a onda preparou. Modo local: mesmo valor,
mesmo wire (goldens são os juízes). NENHUMA mudança abaixo da borda.
DoD: goldens sem regravação + grep de que nenhum handler estrangulado
constrói contexto próprio.

## PR E1s.4 — teste adversarial da borda (entrega própria, espírito B4)

Requisição forjando `X-Tenant-Id`/cookie de outro tenant não lê nem
escreve NADA fora da própria sessão; sessão expirada/revogada = recusa.
Prova-que-morde dupla: remover a checagem de expiração → teste reprova;
remover o extractor de UMA rota (borda furada) → teste reprova NESSA rota
— o adversarial varre as rotas, não uma amostra.

## Decisões que o ADR delimitou sem fixar (nomeadas de antemão)

1. **Formato do token**: proposta — 256 bits de CSPRNG, base64url, prefixo
   `btvs_` (grepável em logs/vazamentos). Sem decisão do dono: técnica.
2. **Onde vive o hash**: proposta — SHA-256 simples do token (o token JÁ É
   aleatório de alta entropia; KDF caro protege senha fraca, não token
   forte — mesma honestidade do pin_hash, agora com a razão inversa).
   Técnica.
3. **TTL/renovação** — a ÚNICA com sabor de produto, empacotada como
   escolha binária para o dono no PR E1s.1:
   (a) **absoluta 30d + ociosidade 24h renovável** (padrão SaaS: sessão
   viva enquanto usada, morre no abandono) — recomendada;
   (b) **absoluta curta 24h sem renovação** (mais estrita; re-login
   diário). Ambas com revogação por linha (logout real) do item 4.

## Fora da E1s (re-declarado do ADR)

Login UI/cadastro/reset; billing; SSO/OIDC; multi-tenant por usuário.
