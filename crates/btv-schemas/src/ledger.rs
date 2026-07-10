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
    /// Sequência monotônica, atribuída pelo storage. Desde B3 (ADR 0027) a
    /// monotonia é POR tenant — a cadeia local legada vira a cadeia do
    /// tenant LOCAL com os mesmos números.
    pub seq: u64,
    /// Hash da entrada anterior ("" para a primeira DA CADEIA DO TENANT).
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
    /// Dono da entrada na cadeia por tenant (ADR 0027, item 2): quando
    /// presente, entra no CORPO HASHEADO — reatribuir a entrada a outro
    /// tenant quebra `entry_hash` (anti-transplante). `None` nas entradas
    /// legadas e na porta sem contexto: corpo canônico byte-idêntico ao que
    /// sempre foi gravado, hashes existentes permanecem válidos sem re-hash
    /// (teste de compatibilidade congelada abaixo). Aditivo no contrato
    /// `ledger-entry.v1` (campo opcional — documentos antigos seguem
    /// validando).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub tenant: Option<btv_domain::TenantId>,
}

impl LedgerEntry {
    /// Corpo canônico usado no cálculo do hash da cadeia (exclui os campos
    /// derivados `seq`, `prev_hash` e `entry_hash`). O `tenant` entra SÓ
    /// quando presente — `"tenant": null` mudaria o corpo canônico de toda
    /// entrada legada e invalidaria os hashes já gravados.
    pub fn hash_body(&self) -> String {
        let mut body = serde_json::json!({
            "kind": self.kind,
            "actor": self.actor,
            "payload": self.payload,
            "override": self.r#override,
            "fake_marker": self.fake_marker,
            "ts": self.ts,
        });
        if let Some(tenant) = &self.tenant {
            body["tenant"] = serde_json::json!(tenant);
        }
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
            tenant: None,
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

    /// B3, compatibilidade CONGELADA (consequência declarada do ADR 0027):
    /// entrada SEM tenant produz exatamente os hashes de antes da mudança.
    /// Os valores esperados foram computados no código de `main` PRÉ-B3
    /// (commit ee6abc7, worktree separado) — não são derivados desta
    /// implementação, então uma regressão de canonicalização não passa por
    /// auto-consistência.
    #[test]
    fn entrada_sem_tenant_produz_os_hashes_pre_b3_congelados() {
        let e = LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: "session.start".into(),
            actor: "user".into(),
            payload: json!({"task": "corrigir teste", "n": 1}),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:00Z".into(),
            tenant: None,
        };
        assert_eq!(
            e.hash_body(),
            r#"{"actor":"user","fake_marker":null,"kind":"session.start","override":null,"payload":{"n":1,"task":"corrigir teste"},"ts":"2026-07-05T00:00:00Z"}"#,
            "corpo canônico sem tenant é byte-idêntico ao pré-B3"
        );
        assert_eq!(
            e.chain_hash(""),
            "1f4285b34c479db7ef1291e9a3702d565b4cd664a0da2f3f8a733dd239c7b2d8"
        );
        assert_eq!(
            e.chain_hash("abc123"),
            "5aef8583f7c9949e2c050151c4d63fd13c45909eee905646b28a3d463bbb45d9"
        );
    }

    /// B3, anti-transplante (ADR 0027 item 2): o tenant DENTRO do corpo
    /// hasheado — reatribuir a entrada a outro tenant muda o corpo canônico
    /// e portanto o hash. É a propriedade que a chave composta sozinha não
    /// dá (coluna muda, corpo não).
    #[test]
    fn tenant_no_corpo_muda_o_hash_e_reatribuicao_e_detectavel() {
        let mut e = entry();
        let sem_tenant = e.chain_hash("");
        e.tenant = Some(btv_domain::TenantId::LOCAL);
        let local = e.chain_hash("");
        assert_ne!(sem_tenant, local, "tenant presente entra no hash");
        assert!(
            e.hash_body()
                .contains(r#""tenant":"00000000-0000-0000-0000-000000000001""#),
            "o corpo canônico carrega o UUID do tenant"
        );
        e.tenant =
            Some(btv_domain::TenantId::parse("00000000-0000-0000-0000-0000000000b3").unwrap());
        assert_ne!(local, e.chain_hash(""), "trocar o tenant troca o hash");
    }
}
