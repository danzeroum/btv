//! Evento de handoff entre padrões/agentes (`handoff-event.v1`).
//!
//! Origem: matriz de orquestração do BuildToValue (`orchestration-matrix.md`)
//! — todo handoff emite `start`/`ack`/`complete`/`error` com telemetria.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffPhase {
    Start,
    Ack,
    Complete,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffEvent {
    pub event: HandoffPhase,
    pub task_id: String,
    pub from_agent: String,
    pub to_agent: String,
    /// Nome do contrato trafegado (ex.: "plan.v1", "proposal.v1").
    pub contract: String,
    /// sha256 do payload trocado (o payload em si vai no ledger).
    pub payload_digest: String,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fases_serializam_em_snake_case() {
        let json = serde_json::to_string(&HandoffPhase::Complete).unwrap();
        assert_eq!(json, "\"complete\"");
    }
}
