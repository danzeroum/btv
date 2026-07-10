//! B4: o adapter Postgres julgado pela MESMA suíte de contrato que julga o
//! SQLite desde B2 — este arquivo é SÓ a instanciação com a factory
//! (coordenada 1 da revisão do B4: a suíte não muda para acomodar o PG; se
//! algum caso precisasse de ajuste, isso seria achado sobre a suíte —
//! idiossincrasia vazada — e iria para revisão antes de qualquer mudança).
//!
//! Sem `BTV_PG_TEST_URL`, cada teste PULA com aviso barulhento (harness) —
//! nunca passa fingindo; o job `pg` do CI roda contra um Postgres real.
//! Testes ESPECÍFICOS de PG (RLS adversarial, concorrência do retry
//! otimista) moram em `src/pg.rs`, nunca aqui nem na suíte.
#![cfg(feature = "pg")]

use btv_schemas::ledger::LedgerEntry;
use btv_store::pg::harness;
use btv_store::LedgerStore;

#[test]
fn adapter_pg_passa_o_contrato_de_run_repository() {
    if !harness::disponivel() {
        return;
    }
    btv_contract::suite_run_repository(|| harness::abrir_isolado().expect("PG do harness").store);
}

#[test]
fn adapter_pg_passa_o_contrato_de_persona_repository() {
    if !harness::disponivel() {
        return;
    }
    btv_contract::suite_persona_repository(|| {
        harness::abrir_isolado().expect("PG do harness").store
    });
}

#[test]
fn adapter_pg_passa_o_contrato_de_ledger_repository() {
    if !harness::disponivel() {
        return;
    }
    btv_contract::suite_ledger_repository(|| {
        harness::abrir_isolado().expect("PG do harness").store
    });
}

/// Coordenada 4 da revisão do B4: os MESMOS appends pelo SQLite e pelo PG
/// produzem a MESMA sequência de `(seq, prev_hash, entry_hash)` — paridade
/// local↔SaaS no nível criptográfico, não só comportamental.
#[test]
fn sqlite_e_pg_produzem_a_mesma_sequencia_de_hashes() {
    if !harness::disponivel() {
        return;
    }
    btv_contract::suite_ledger_determinismo_cross_adapter(
        || LedgerStore::open_in_memory().expect("SQLite em memória"),
        || harness::abrir_isolado().expect("PG do harness").store,
        |e: &LedgerEntry| (e.seq, e.prev_hash.clone(), e.entry_hash.clone()),
    );
}
