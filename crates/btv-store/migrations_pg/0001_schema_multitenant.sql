-- B4 (ADR 0026): schema multitenant do modo SaaS.
--
-- `tenant_id` é TEXT com o UUID canônico do `TenantId` do domínio
-- (minúsculo, hifenizado) para espelhar BYTE A BYTE o que o adapter SQLite
-- grava — a suíte de contrato e o teste de determinismo cross-adapter
-- cobram a paridade. SEM default de coluna: no modo SaaS todo acesso
-- carrega o tenant do contexto (a "porta legada" sem contexto é a porta do
-- modo LOCAL, que é SQLite — decisão registrada do B2; ela não existe aqui).
--
-- Row-Level Security em TODA tabela (defesa em profundidade do ADR 0026:
-- mesmo um SQL com bug não vaza entre tenants). FORCE aplica a policy até
-- ao DONO das tabelas — o role de aplicação não escapa dela.
-- `current_setting('app.tenant_id', true)` devolve NULL quando a sessão
-- não fixou tenant ⇒ a policy avalia NULL ⇒ ZERO linhas visíveis
-- (fail-closed: sessão sem tenant vê nada, nunca tudo).
--
-- Só as tabelas que as traits desta fase servem (RunRepository,
-- PersonaRepository, LedgerRepository) — `template_pub`/`users` entram
-- quando a Trilha E as puser atrás de porta (tabela morta seria cobertura
-- de fachada).

CREATE TABLE runs (
    id BIGSERIAL PRIMARY KEY,
    task_id TEXT NOT NULL,
    template_id TEXT NOT NULL,
    template_versao TEXT NOT NULL,
    nome TEXT NOT NULL,
    briefing_json TEXT NOT NULL,
    papeis_json TEXT NOT NULL,
    status TEXT NOT NULL,
    gates_aprovados BIGINT NOT NULL DEFAULT 0,
    created_ts TEXT NOT NULL,
    updated_ts TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    UNIQUE (tenant_id, task_id)
);

CREATE TABLE deliverables (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL,
    task_id TEXT NOT NULL,
    template_id TEXT NOT NULL,
    nome TEXT NOT NULL,
    path TEXT NOT NULL,
    formato TEXT NOT NULL,
    versao TEXT NOT NULL,
    trilha TEXT NOT NULL,
    created_ts TEXT NOT NULL,
    tenant_id TEXT NOT NULL
);

CREATE TABLE persona_overrides (
    template_id TEXT NOT NULL,
    papel TEXT NOT NULL,
    prompt TEXT NOT NULL,
    updated_ts TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    PRIMARY KEY (tenant_id, template_id, papel)
);

CREATE TABLE custom_personas (
    id BIGSERIAL PRIMARY KEY,
    template_id TEXT NOT NULL,
    nome TEXT NOT NULL,
    prompt TEXT NOT NULL,
    updated_ts TEXT NOT NULL,
    tenant_id TEXT NOT NULL
);

-- A cadeia (tenant_id, seq) do ADR 0027. `body` é TEXT, nunca JSONB: o
-- corpo canônico é HASHEADO byte a byte — JSONB reordena chaves e
-- normaliza a representação, o que quebraria `verify_chain` silenciosamente.
-- O UNIQUE (tenant_id, seq) é o MECANISMO de serialização do append por
-- tenant (ADR 0028): o perdedor da corrida leva 23505 e reencadeia no topo
-- novo — nenhum lock de sessão (sobrevive a pooler em modo transação).
CREATE TABLE ledger (
    id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    seq BIGINT NOT NULL,
    prev_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL,
    body TEXT NOT NULL,
    UNIQUE (tenant_id, seq)
);

ALTER TABLE runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE runs FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON runs FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE deliverables ENABLE ROW LEVEL SECURITY;
ALTER TABLE deliverables FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON deliverables FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE persona_overrides ENABLE ROW LEVEL SECURITY;
ALTER TABLE persona_overrides FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON persona_overrides FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE custom_personas ENABLE ROW LEVEL SECURITY;
ALTER TABLE custom_personas FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON custom_personas FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

ALTER TABLE ledger ENABLE ROW LEVEL SECURITY;
ALTER TABLE ledger FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON ledger FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));
