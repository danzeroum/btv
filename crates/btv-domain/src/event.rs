//! Tipos de dado do event store (D1t) — nasceram em `btv-store::events` e
//! moram no domínio para o `EventStorePort` (assinatura do G1) ter tipos
//! concretos nomeáveis pelos consumidores (a sessão durável de `btv-core`
//! constrói `EventInput` e faz replay de `StoredEvent` sem conhecer o
//! driver). Definições BYTE-IDÊNTICAS às originais (o replay lê JSON
//! persistido por versões antigas — wire não muda). `btv-store` re-exporta.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Evento novo a anexar; `id`/`seq` são atribuídos pelo store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventInput {
    /// Tipo do evento, com a versão embutida (ex.: `message.1`).
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

impl EventInput {
    pub fn new(kind: impl Into<String>, data: Value) -> Self {
        Self {
            kind: kind.into(),
            data,
        }
    }
}

/// Evento persistido; `(aggregate_id, seq)` é único.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: String,
    pub aggregate_id: String,
    pub seq: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}
