//! Evidência de verificação determinística (`verification-evidence.v1`).
//!
//! Origem: fork do opencode (`script/verify.ts` + ADR 0001): o LLM orquestra,
//! ferramentas determinísticas verificam — typecheck → test → lint → SAST —
//! e o resultado vira evidência JSON estruturada consumida pelo Auditor.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub tool: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationStep {
    /// Nome do passo (ex.: "typecheck", "test", "lint", "sast").
    pub name: String,
    /// Ferramenta executada (ex.: "cargo test", "pytest", "clippy").
    pub tool: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(default)]
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationEvidence {
    pub run_id: String,
    pub git_sha: String,
    pub steps: Vec<VerificationStep>,
    pub verdict: Verdict,
    pub produced_at: String,
}

impl VerificationEvidence {
    /// Veredito derivado dos passos: `Pass` só quando nenhum passo falhou.
    pub fn derive_verdict(steps: &[VerificationStep]) -> Verdict {
        if steps.iter().any(|s| s.exit_code != 0) {
            Verdict::Fail
        } else {
            Verdict::Pass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veredito_falha_se_qualquer_passo_falha() {
        let steps = vec![
            VerificationStep {
                name: "test".into(),
                tool: "cargo test".into(),
                exit_code: 0,
                duration_ms: 10,
                findings: vec![],
            },
            VerificationStep {
                name: "lint".into(),
                tool: "clippy".into(),
                exit_code: 1,
                duration_ms: 5,
                findings: vec![],
            },
        ];
        assert!(matches!(
            VerificationEvidence::derive_verdict(&steps),
            Verdict::Fail
        ));
    }
}
