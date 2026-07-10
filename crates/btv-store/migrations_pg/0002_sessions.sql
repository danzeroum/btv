-- E1s.1 (ADR 0029 aceito): a tabela de sessões do modo SaaS.
--
-- ESTA TABELA É A EXCEÇÃO LEGÍTIMA E ÚNICA à regra "toda query é
-- tenant-escopada" (item 5 do ADR, nomeada no aceite do dono). O motivo é
-- estrutural, não uma folga: o lookup que resolve o token acontece ANTES de
-- existir `TenantContext`, então `app.tenant_id` NÃO está fixado no caminho
-- de autenticação. Se esta tabela tivesse RLS de tenant como as outras
-- cinco, `resolve_session` (sem `app.tenant_id`) veria ZERO linhas e a
-- autenticação inteira falharia fail-closed. Por isso: SEM RLS de tenant
-- aqui, e a política do item 5 no lugar dela —
--   (a) o caminho de AUTH acessa a tabela SÓ por igualdade de `token_hash`
--       (nenhuma query enumera/lista/filtra sessões por outro predicado);
--   (b) o token em si NUNCA é persistido — só o SHA-256 dele; um dump da
--       tabela não autentica ninguém (não há como reverter o hash ao token);
--   (c) as operações ADMINISTRATIVAS (listar/revogar) rodam DEPOIS da auth,
--       com `TenantContext`, e são tenant-escopadas por WHERE explícito como
--       qualquer outra.
-- Qualquer OUTRA tabela que um dia precise de leitura pré-contexto passa por
-- ADR próprio — esta exceção não é categoria, é UMA linha nomeada.

CREATE TABLE sessions (
    -- SHA-256 hex do token opaco apresentado. PK: o acesso de auth é por
    -- igualdade exata desta coluna, nada mais.
    token_hash TEXT PRIMARY KEY,
    tenant_id  TEXT NOT NULL,
    user_id    TEXT NOT NULL,
    created_ts        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- TTL decisão (a) do dono: absoluta 30d + ociosidade 24h RENOVÁVEL.
    absolute_deadline TIMESTAMPTZ NOT NULL,   -- created + 30d — teto que a renovação nunca ultrapassa
    idle_deadline     TIMESTAMPTZ NOT NULL,   -- renovada a cada resolve (LEAST(now()+24h, absolute))
    -- Revogação por linha (logout real, item 4): NULL = ativa; setada = morta.
    revoked_at        TIMESTAMPTZ
);

-- Índice para as operações administrativas tenant-escopadas (listar/revogar
-- as sessões de UM tenant) — NUNCA usado no caminho de auth (que é por PK).
CREATE INDEX sessions_por_tenant ON sessions (tenant_id);
