//! B2 (DoD: "dados existentes migram sem perda"): constrói um `.btv/btv.db`
//! com o schema PRÉ-tenant (o DDL exato que `BtvStore::init` criava antes do
//! B2), popula com linhas no formato de produção, e prova que o `open`
//! migra: dados intactos, `tenant_id` backfillado com o UUID LOCAL
//! determinístico (ADR 0026), API legada E traits lendo o mesmo banco.

use btv_domain::ports::{RunRepository, RunStatus};
use btv_domain::{ActorId, TaskId, TenantContext, TenantId};
use btv_store::btv::BtvStore;

const LOCAL: &str = "00000000-0000-0000-0000-000000000001";

/// O DDL que `BtvStore::init` executava ANTES do B2 (copiado do histórico —
/// commit do A6/PIN), sem nenhuma coluna de tenant.
const DDL_PRE_TENANT: &str = "CREATE TABLE runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL UNIQUE,
    template_id TEXT NOT NULL,
    template_versao TEXT NOT NULL,
    nome TEXT NOT NULL,
    briefing_json TEXT NOT NULL,
    papeis_json TEXT NOT NULL,
    status TEXT NOT NULL,
    gates_aprovados INTEGER NOT NULL DEFAULT 0,
    created_ts TEXT NOT NULL,
    updated_ts TEXT NOT NULL
);
CREATE TABLE deliverables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    task_id TEXT NOT NULL,
    template_id TEXT NOT NULL,
    nome TEXT NOT NULL,
    path TEXT NOT NULL,
    formato TEXT NOT NULL,
    versao TEXT NOT NULL,
    trilha TEXT NOT NULL,
    created_ts TEXT NOT NULL
);
CREATE TABLE persona_overrides (
    template_id TEXT NOT NULL,
    papel TEXT NOT NULL,
    prompt TEXT NOT NULL,
    updated_ts TEXT NOT NULL,
    PRIMARY KEY (template_id, papel)
);
CREATE TABLE template_pub (
    template_id TEXT PRIMARY KEY,
    publicado INTEGER NOT NULL,
    updated_ts TEXT NOT NULL
);
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nome TEXT NOT NULL,
    email TEXT NOT NULL,
    papel TEXT NOT NULL,
    ativo INTEGER NOT NULL DEFAULT 1,
    created_ts TEXT NOT NULL,
    pin_hash TEXT
);
CREATE TABLE custom_personas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id TEXT NOT NULL,
    nome TEXT NOT NULL,
    prompt TEXT NOT NULL,
    updated_ts TEXT NOT NULL
);";

/// Popula o banco antigo pelo formato de PRODUÇÃO (as mesmas strings que os
/// handlers sempre gravaram) e devolve o path.
fn banco_legado(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("btv.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(DDL_PRE_TENANT).unwrap();
    conn.execute_batch(
        r#"INSERT INTO runs (task_id, template_id, template_versao, nome, briefing_json,
                             papeis_json, status, gates_aprovados, created_ts, updated_ts)
           VALUES ('sq1', 'editorial', 'v1.4', 'Newsletter de julho',
                   '[{"label":"Pauta","resposta":"logística verde"}]',
                   '["Pauteiro","Redator"]', 'concluida', 2,
                   '2026-07-01T10:00:00Z', '2026-07-01T11:00:00Z'),
                  ('sq2', 'bi', 'v2.1', 'Painel Q3', '[]', '["Analista"]',
                   'ativa', 0, '2026-07-02T09:00:00Z', '2026-07-02T09:00:00Z');
           INSERT INTO deliverables (run_id, task_id, template_id, nome, path, formato,
                                     versao, trilha, created_ts)
           VALUES (1, 'sq1', 'editorial', 'artigo.md', '/w/artigo.md', 'MD', 'v1',
                   'Redator → Revisor · 2 gates', '2026-07-01T10:50:00Z');
           INSERT INTO persona_overrides (template_id, papel, prompt, updated_ts)
           VALUES ('editorial', 'Redator', 'voz da casa', '2026-07-01T08:00:00Z');
           INSERT INTO template_pub (template_id, publicado, updated_ts)
           VALUES ('editorial', 1, '2026-07-01T08:00:00Z');
           INSERT INTO users (nome, email, papel, ativo, created_ts)
           VALUES ('Ana', 'ana@x', 'usuario', 1, '2026-07-01T07:00:00Z');
           INSERT INTO custom_personas (template_id, nome, prompt, updated_ts)
           VALUES ('editorial', 'Ghost', 'escreva', '2026-07-01T08:30:00Z');"#,
    )
    .unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn banco_pre_tenant_migra_sem_perda_e_com_backfill_local() {
    let dir = tempfile::tempdir().unwrap();
    let path = banco_legado(&dir);

    // O open migra (ADD COLUMN + rebuild das tabelas cuja PK muda).
    let store = BtvStore::open(&path).unwrap();

    // API legada: tudo que existia continua existindo, byte a byte.
    let runs = store.list_runs().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].task_id, TaskId::new(2), "mais recente primeiro");
    assert_eq!(runs[0].status, RunStatus::Ativa);
    let sq1 = &runs[1];
    assert_eq!(sq1.nome, "Newsletter de julho");
    assert_eq!(sq1.status, RunStatus::Concluida);
    assert_eq!(sq1.gates_aprovados, 2);
    assert_eq!(
        sq1.briefing_json,
        r#"[{"label":"Pauta","resposta":"logística verde"}]"#
    );
    assert_eq!(sq1.tenant, TenantId::LOCAL, "backfill determinístico");

    let entregas = store.list_deliverables().unwrap();
    assert_eq!(entregas.len(), 1);
    assert_eq!(entregas[0].run_id, sq1.id, "vínculo run→entrega preservado");
    assert_eq!(entregas[0].trilha, "Redator → Revisor · 2 gates");

    assert_eq!(
        store.list_persona_overrides("editorial").unwrap()[0].prompt,
        "voz da casa"
    );
    assert_eq!(
        store.list_template_pub().unwrap(),
        vec![("editorial".to_string(), true)]
    );
    assert_eq!(store.list_users().unwrap()[0].nome, "Ana");
    assert_eq!(
        store.list_custom_personas("editorial").unwrap()[0].nome,
        "Ghost"
    );

    // As TRAITS leem o mesmo banco com ctx LOCAL — e o run migrado é
    // invisível para outro tenant (isolamento fail-closed pós-migração).
    let ctx = TenantContext::local(ActorId::new("test:migracao").unwrap());
    let lido = store
        .get(&ctx, "sq1")
        .unwrap()
        .expect("sq1 no tenant LOCAL");
    assert_eq!(lido.gates_aprovados, 2);
    let outro = TenantContext::new(
        TenantId::parse("00000000-0000-0000-0000-0000000000b2").unwrap(),
        ActorId::new("test:outro").unwrap(),
    );
    assert!(store.get(&outro, "sq1").unwrap().is_none());
    assert_eq!(store.max_task_seq(&ctx).unwrap(), 2);
    drop(store);

    // Coluna a coluna: TODA linha de TODA tabela recebeu o UUID LOCAL.
    let conn = rusqlite::Connection::open(&path).unwrap();
    for tabela in [
        "runs",
        "deliverables",
        "persona_overrides",
        "template_pub",
        "users",
        "custom_personas",
    ] {
        let fora: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {tabela} WHERE tenant_id <> ?1"),
                rusqlite::params![LOCAL],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fora, 0, "{tabela}: linha sem backfill LOCAL");
        let total: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |r| r.get(0))
            .unwrap();
        assert!(total > 0, "{tabela}: a migração não pode perder linhas");
    }
    drop(conn);

    // Idempotência: reabrir um banco JÁ migrado não migra de novo nem perde
    // nada (o detector por pragma_table_info devolve cedo).
    let store = BtvStore::open(&path).unwrap();
    assert_eq!(store.list_runs().unwrap().len(), 2);
    assert_eq!(store.list_deliverables().unwrap().len(), 1);
}

#[test]
fn escrita_legada_pos_migracao_continua_caindo_no_tenant_local() {
    let dir = tempfile::tempdir().unwrap();
    let path = banco_legado(&dir);
    let store = BtvStore::open(&path).unwrap();

    // Os writers legados (sem ctx) seguem funcionando após a migração — o
    // DEFAULT da coluna os coloca no tenant LOCAL, onde os leitores (legado
    // e trait) os encontram.
    store
        .insert_run(
            "sq3",
            "editorial",
            "v1.4",
            "Pós-migração",
            "[]",
            "[]",
            "2026-07-09T00:00:00Z",
        )
        .unwrap();
    store
        .set_status("sq3", RunStatus::Concluida, "2026-07-09T01:00:00Z")
        .unwrap();
    assert_eq!(store.max_run_task_seq(), 3);

    let ctx = TenantContext::local(ActorId::new("test:migracao").unwrap());
    let run = store.get(&ctx, "sq3").unwrap().expect("visível pela trait");
    assert_eq!(run.status, RunStatus::Concluida);
    assert_eq!(run.tenant, TenantId::LOCAL);
}
