//! B2: o adapter SQLite julgado pela suíte de contrato dual-adapter
//! (`btv-contract`). Os MESMOS testes julgarão o adapter Postgres em B4 —
//! este arquivo é só a instanciação: os casos moram na suíte, que testa o
//! CONTRATO das traits (comportamento observável), nunca idiossincrasia de
//! um adapter (regra da revisão do A4).

use btv_store::btv::BtvStore;

#[test]
fn adapter_sqlite_passa_o_contrato_de_run_repository() {
    btv_contract::suite_run_repository(|| BtvStore::open_in_memory().expect("adapter fresco"));
}

#[test]
fn adapter_sqlite_passa_o_contrato_de_persona_repository() {
    btv_contract::suite_persona_repository(|| BtvStore::open_in_memory().expect("adapter fresco"));
}
