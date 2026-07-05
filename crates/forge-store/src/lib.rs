//! Storage durável da plataforma Forge (SQLite via rusqlite).
//!
//! Fase 1 (scaffold): ledger append-only com hash-chain verificável.
//! Sessões, biblioteca de prompts, memória do squad e buffer de telemetria
//! ganham tabelas próprias ao longo das Fases 1–3.

pub mod ledger;

pub use ledger::LedgerStore;
