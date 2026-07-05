//! Entrada do ledger append-only (`ledger-entry.v1`).
//!
//! Governança BuildToValue: overrides são marcados e mocks declaram-se
//! via `fake_marker` ("Nada Fake") — ambos campos de primeira classe.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{canonical_json, sha256_hex};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OverrideMark {
    pub marked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LedgerEntry {
    /// Sequência monotônica, atribuída pelo storage.
    pub seq: u64,
    /// Hash da entrada anterior ("" para a primeira).
    pub prev_hash: String,
    /// sha256 de `prev_hash + JSON canônico do corpo` (calculado pelo storage).
    pub entry_hash: String,
    /// Tipo do evento (ex.: "session.start", "consensus.reached", "tool.run").
    pub kind: String,
    /// Quem produziu a entrada (agente, usuário, sistema).
    pub actor: String,
    /// Corpo livre do evento.
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#override: Option<OverrideMark>,
    /// Presente quando o payload contém dados simulados ("Nada Fake").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fake_marker: Option<String>,
    /// Timestamp RFC3339.
    pub ts: String,
}

impl LedgerEntry {
    /// Corpo canônico usado no cálculo do hash da cadeia (exclui os campos
    /// derivados `seq`, `prev_hash` e `entry_hash`).
    pub fn hash_body(&self) -> String {
        let body = serde_json::json!({
            "kind": self.kind,
            "actor": self.actor,
            "payload": self.payload,
            "override": self.r#override,
            "fake_marker": self.fake_marker,
            "ts": self.ts,
        });
        canonical_json(&body)
    }

    /// Hash encadeado da entrada.
    pub fn chain_hash(&self, prev_hash: &str) -> String {
        sha256_hex(&format!("{prev_hash}{}", self.hash_body()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> LedgerEntry {
        LedgerEntry {
            seq: 1,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: "session.start".into(),
            actor: "user".into(),
            payload: json!({"task": "corrigir teste"}),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:00Z".into(),
        }
    }

    #[test]
    fn hash_e_deterministico_e_encadeado() {
        let e = entry();
        let h1 = e.chain_hash("");
        assert_eq!(h1, entry().chain_hash(""));
        let h2 = e.chain_hash(&h1);
        assert_ne!(h1, h2);
    }
}
