//! Evento de telemetria offline-first (`telemetry-event.v1`).
//!
//! Origem: prompte — eventos são enfileirados localmente (SQLite) e
//! descarregados em lote; o dashboard agrega chamadas LLM, cache hit rate,
//! rate limited e erros.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryEvent {
    /// Nome do evento (ex.: "llm.call", "cache.hit", "rate.limited").
    pub name: String,
    /// Sessão a que o evento pertence.
    pub session_id: String,
    /// Propriedades livres do evento.
    #[serde(default)]
    pub props: Value,
    pub ts: String,
}
