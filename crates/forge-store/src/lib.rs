//! Storage durável da plataforma Forge (SQLite via rusqlite).
//!
//! Fase 1: ledger append-only com hash-chain verificável e cache de
//! prompts por hash. Sessões duráveis, biblioteca de prompts e buffer de
//! telemetria ganham tabelas próprias nas Fases 2–3.

pub mod events;
pub mod ledger;
pub mod prompt_cache;

pub use events::{EventError, EventInput, EventStore, StoredEvent};
pub use ledger::LedgerStore;
pub use prompt_cache::PromptCache;
