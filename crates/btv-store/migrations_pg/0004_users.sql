-- C3.4 (ADR 0026): `users` (perfis locais A6) entram atrás de porta
-- (`UserRepository`). Como o `template_pub` (0003), foi ADIADA de propósito no
-- 0001 ("template_pub/users entram quando uma porta as servir; tabela morta
-- seria cobertura de fachada"). A onda de encerramento C3.4 estrangula os
-- handlers de users pela porta, então a tabela nasce AGORA.
--
-- Espelha o adapter SQLite (B2): `id` global (BIGSERIAL), `pin_hash` opcional
-- (o hash NUNCA sai do adapter — a porta expõe só `verify_pin`/`has_pin`),
-- `tenant_id` sem default (no saas todo acesso carrega o tenant do contexto).
-- RLS FORCE como as demais tabelas — fail-closed por defesa em profundidade.
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    nome TEXT NOT NULL,
    email TEXT NOT NULL,
    papel TEXT NOT NULL,
    ativo BOOLEAN NOT NULL DEFAULT true,
    created_ts TEXT NOT NULL,
    pin_hash TEXT,
    tenant_id TEXT NOT NULL
);

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON users FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));
