-- C3.3a (ADR 0026): `template_pub` entra atrás de porta
-- (`TemplatePublicationRepository`). Foi ADIADA de propósito no 0001 — o
-- comentário lá dizia "template_pub/users entram quando a Trilha E as puser
-- atrás de porta; tabela morta seria cobertura de fachada". A onda C3.3
-- estrangula a publicação de templates pela porta, então a tabela nasce
-- AGORA, com a trait que a serve.
--
-- Mesmo shape do adapter SQLite (B2): PK (tenant_id, template_id), sem default
-- de coluna (no saas todo acesso carrega o tenant do contexto). RLS FORCE como
-- as demais tabelas — fail-closed por defesa em profundidade (ADR 0026).
CREATE TABLE template_pub (
    template_id TEXT NOT NULL,
    publicado BOOLEAN NOT NULL,
    updated_ts TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    PRIMARY KEY (tenant_id, template_id)
);

ALTER TABLE template_pub ENABLE ROW LEVEL SECURITY;
ALTER TABLE template_pub FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON template_pub FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true));
