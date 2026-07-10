//! B3 (ADR 0027 item 5, análogo ao `migracao_pre_tenant.rs` do B2):
//! constrói um ledger com o schema PRÉ-B3 (`seq INTEGER PRIMARY KEY
//! AUTOINCREMENT`, cadeia global, sem coluna de tenant) contendo uma
//! hash-chain REAL, e prova que o `open` migra: a cadeia global VIRA a
//! cadeia do tenant LOCAL com `seq`/`prev_hash`/`entry_hash` byte-idênticos
//! — nenhum corpo tocado, nenhum hash recomputado — e `verify_chain` passa
//! sobre os hashes originais, pela porta legada E pela trait.

use btv_domain::ports::LedgerRepository;
use btv_domain::{ActorId, TenantContext, TenantId};
use btv_schemas::ledger::LedgerEntry;
use btv_store::{LedgerError, LedgerStore};

const LOCAL: &str = "00000000-0000-0000-0000-000000000001";

/// O DDL exato que `LedgerStore::init` executava antes do B3.
const DDL_PRE_B3: &str = "CREATE TABLE ledger (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    prev_hash  TEXT NOT NULL,
    entry_hash TEXT NOT NULL,
    body       TEXT NOT NULL
);";

fn entrada(kind: &str, actor: &str, ts: &str) -> LedgerEntry {
    LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: kind.into(),
        actor: actor.into(),
        payload: serde_json::json!({"task": "migração B3"}),
        r#override: None,
        fake_marker: None,
        ts: ts.into(),
        tenant: None,
    }
}

/// Constrói o banco antigo com uma cadeia REAL de 3 entradas, encadeada
/// pelo MESMO algoritmo do append de produção (`chain_hash` sobre o corpo
/// canônico + serialização com `seq: 0`) — a compatibilidade byte a byte
/// do corpo sem tenant é provada à parte pelo teste de hash congelado em
/// `btv-schemas` (valores computados no código pré-B3).
fn ledger_legado(dir: &tempfile::TempDir) -> (String, Vec<(u64, String, String)>) {
    let path = dir.path().join("btv.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(DDL_PRE_B3).unwrap();

    let mut prev = String::new();
    let mut gravadas = Vec::new();
    for (i, (kind, actor)) in [
        ("session.start", "btv-cli:s1"),
        ("tool.run", "btv-cli:s1"),
        ("btv.squad_activated", "web:btv"),
    ]
    .iter()
    .enumerate()
    {
        let mut e = entrada(kind, actor, &format!("2026-07-0{}T00:00:00Z", i + 1));
        e.prev_hash = prev.clone();
        e.entry_hash = e.chain_hash(&prev);
        conn.execute(
            "INSERT INTO ledger (prev_hash, entry_hash, body) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                e.prev_hash,
                e.entry_hash,
                serde_json::to_string(&e).unwrap()
            ],
        )
        .unwrap();
        let seq = conn.last_insert_rowid() as u64;
        gravadas.push((seq, e.prev_hash.clone(), e.entry_hash.clone()));
        prev = e.entry_hash;
    }
    (path.to_str().unwrap().to_string(), gravadas)
}

#[test]
fn ledger_pre_b3_migra_com_hashes_intactos_e_verify_passa() {
    let dir = tempfile::tempdir().unwrap();
    let (path, originais) = ledger_legado(&dir);

    // O open migra (REBUILD com backfill LOCAL, corpo/hashes intocados).
    let store = LedgerStore::open(&path).unwrap();

    // A verificação legada passa sobre os hashes ORIGINAIS.
    assert_eq!(store.verify_chain().unwrap(), 3);

    // E a trait verifica a MESMA cadeia como a cadeia do tenant LOCAL;
    // outro tenant não vê nada (fail-closed pós-migração).
    let ctx = TenantContext::local(ActorId::new("test:migracao").unwrap());
    assert_eq!(LedgerRepository::verify_chain(&store, &ctx).unwrap(), 3);
    let outro = TenantContext::new(
        TenantId::parse("00000000-0000-0000-0000-0000000000b3").unwrap(),
        ActorId::new("test:outro").unwrap(),
    );
    assert_eq!(LedgerRepository::verify_chain(&store, &outro).unwrap(), 0);
    assert!(LedgerRepository::export(&store, &outro).unwrap().is_empty());

    // seq/prev_hash/entry_hash byte-idênticos aos gravados pré-migração,
    // e as entradas legadas seguem SEM tenant no corpo (Option ausente).
    let export = LedgerRepository::export(&store, &ctx).unwrap();
    assert_eq!(export.len(), 3);
    for (lida, (seq, prev, hash)) in export.iter().zip(&originais) {
        assert_eq!(lida.seq, *seq);
        assert_eq!(&lida.prev_hash, prev);
        assert_eq!(&lida.entry_hash, hash);
        assert!(lida.tenant.is_none(), "legado não ganha tenant no corpo");
    }
    drop(store);

    // Coluna backfillada com o UUID LOCAL em toda linha.
    let conn = rusqlite::Connection::open(&path).unwrap();
    let fora: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ledger WHERE tenant_id <> ?1",
            rusqlite::params![LOCAL],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fora, 0);
    drop(conn);

    // Idempotência + continuidade: reabrir não migra de novo, e um append
    // novo pela porta legada ENCADEIA no topo migrado (prev_hash = último
    // hash original) com o seq seguinte.
    let mut store = LedgerStore::open(&path).unwrap();
    let nova = store
        .append(entrada(
            "session.start",
            "btv-cli:s2",
            "2026-07-09T00:00:00Z",
        ))
        .unwrap();
    assert_eq!(nova.seq, 4);
    assert_eq!(nova.prev_hash, originais[2].2, "encadeia no topo migrado");
    assert_eq!(store.verify_chain().unwrap(), 4);
}

#[test]
fn adulteracao_pos_migracao_e_detectada_na_seq_certa() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = ledger_legado(&dir);
    let store = LedgerStore::open(&path).unwrap();
    drop(store);

    // Adultera o payload da seq 2 direto no arquivo (ataque retroativo).
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE ledger SET body = replace(body, 'migração B3', 'adulterado') WHERE seq = 2",
        [],
    )
    .unwrap();
    drop(conn);

    let store = LedgerStore::open(&path).unwrap();
    assert!(
        matches!(
            store.verify_chain(),
            Err(LedgerError::BrokenChain { seq: 2, .. })
        ),
        "a quebra aponta a seq adulterada, não outra"
    );
}
