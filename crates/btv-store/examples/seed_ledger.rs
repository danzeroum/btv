//! Semeia entradas reais num `.btv/btv.db`, usando o mesmo
//! `btv_store::LedgerStore::append` que produção usa — não é um hack de
//! SQL cru. Existe para testes de integração cross-process (ex.: o e2e de
//! `web/` que sobe um `btv dashboard` real e confirma que a tela de
//! Ledger reflete entradas gravadas por fora).
//!
//! Uso: cargo run -p btv-store --example seed_ledger -- <db_path> <kind> <actor> [payload_json] [ts]
//!
//! Chamado uma vez por entrada a semear (mesmo padrão de `seed_telemetry`) —
//! `LedgerStore::open` reabre o mesmo arquivo e encadeia a partir do que já
//! está lá, então múltiplas chamadas em sequência formam uma cadeia real.

use btv_schemas::ledger::LedgerEntry;
use btv_store::LedgerStore;

fn main() {
    let mut args = std::env::args().skip(1);
    let db_path = args
        .next()
        .expect("uso: seed_ledger <db_path> <kind> <actor> [payload_json] [ts]");
    let kind = args.next().expect("faltou <kind>");
    let actor = args.next().expect("faltou <actor>");
    let payload_json = args.next().unwrap_or_else(|| "{}".to_string());
    let ts = args
        .next()
        .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string());

    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).expect("payload_json inválido");

    let mut store = LedgerStore::open(&db_path).expect("falha ao abrir btv.db");
    let entry = store
        .append(LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: kind.clone(),
            actor,
            payload,
            r#override: None,
            fake_marker: None,
            ts,
            tenant: None,
        })
        .expect("falha ao gravar no ledger");

    println!("entrada '{kind}' gravada em {db_path} (seq={})", entry.seq);
}
