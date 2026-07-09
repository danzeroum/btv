//! Review por valor DERIVADO da evidência real do `/verify` (validação de
//! pendencias.md — antes 100% mock no frontend, `VALUE_SCORE`/`VALUE_GATE`/
//! `REVIEWERS` fabricados). Porte fiel das partes **determinísticas** do
//! `btv_review` (Python, Fase 5): as dimensões `technical` e `security` saem de
//! sinal real (`exit_code`/`findings` dos passos) e os **gates duros** decidem
//! `critical finding` / `veredito fail` / `piso de segurança` — regras que
//! nenhuma média alta "salva".
//!
//! **Honestidade (o que NÃO é portado):** as dimensões `performance` e `value`
//! do `btv_review` não têm sinal determinístico nesta fase — dependem de
//! avaliação de agente/humano. Este review **não as fabrica**: reporta só o que
//! deriva da evidência (`technical`/`security` + gates) e sinaliza que a
//! certificação plena (média ponderada das 4 dimensões, `value_score > 0.7`)
//! exige as duas dimensões de agente, que este caminho não wireia.

use crate::verification::{Verdict, VerificationEvidence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Piso duro da dimensão `security` (mesma semântica do `SECURITY_FLOOR` do
/// `btv_review.gates`).
pub const SECURITY_FLOOR: f64 = 0.5;

/// Qual gate duro reprovou (`None` = nenhum gate disparou).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateTriggered {
    CriticalFinding,
    VerifyFail,
    SecurityFloor,
}

/// Review por valor derivado da evidência. Só as dimensões determinísticas —
/// `performance`/`value` ficam de fora **de propósito** (ver doc do módulo).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValueReview {
    /// Fração de passos do `/verify` que passaram (`exit_code == 0`).
    pub technical: f64,
    /// `1.0` menos penalidade por finding, piso `0.0`.
    pub security: f64,
    /// Os gates duros passaram? (`false` se algum reprovou). NÃO é a
    /// certificação plena — essa exige as dimensões de agente ausentes.
    pub gates_passed: bool,
    /// Gate que reprovou, se houver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_triggered: Option<GateTriggered>,
    pub reason: String,
}

fn severity_penalty(sev: &str) -> f64 {
    match sev {
        "critical" | "error" => 0.4,
        "warning" => 0.1,
        _ => 0.05,
    }
}

impl ValueReview {
    /// Deriva o review das dimensões determinísticas + gates duros, na mesma
    /// ordem do `btv_review.gates.evaluate`.
    pub fn from_evidence(evidence: &VerificationEvidence) -> Self {
        let steps = &evidence.steps;
        let technical = if steps.is_empty() {
            0.5
        } else {
            steps.iter().filter(|s| s.exit_code == 0).count() as f64 / steps.len() as f64
        };
        let security = if steps.is_empty() {
            0.5
        } else {
            let penalty: f64 = steps
                .iter()
                .flat_map(|s| &s.findings)
                .map(|f| severity_penalty(&f.severity))
                .sum();
            (1.0 - penalty).max(0.0)
        };

        let has_critical = steps
            .iter()
            .flat_map(|s| &s.findings)
            .any(|f| f.severity == "critical");

        let (gates_passed, gate_triggered, reason) = if has_critical {
            (
                false,
                Some(GateTriggered::CriticalFinding),
                "finding de severidade crítica na evidência de verificação".to_string(),
            )
        } else if evidence.verdict == Verdict::Fail {
            (
                false,
                Some(GateTriggered::VerifyFail),
                "veredito fail na evidência de verificação".to_string(),
            )
        } else if security < SECURITY_FLOOR {
            (
                false,
                Some(GateTriggered::SecurityFloor),
                format!("segurança abaixo do piso ({security:.2} < {SECURITY_FLOOR})"),
            )
        } else {
            (
                true,
                None,
                "gates duros passaram (technical/security derivados da evidência)".to_string(),
            )
        };

        Self {
            technical,
            security,
            gates_passed,
            gate_triggered,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::{Finding, VerificationStep};

    fn ev(steps: Vec<VerificationStep>) -> VerificationEvidence {
        let verdict = VerificationEvidence::derive_verdict(&steps);
        VerificationEvidence {
            run_id: "r".into(),
            git_sha: "sha".into(),
            steps,
            verdict,
            produced_at: "t".into(),
        }
    }

    fn step(exit: i32, findings: Vec<Finding>) -> VerificationStep {
        VerificationStep {
            name: "s".into(),
            tool: "t".into(),
            exit_code: exit,
            duration_ms: 1,
            findings,
        }
    }

    fn finding(sev: &str) -> Finding {
        Finding {
            tool: "t".into(),
            severity: sev.into(),
            message: "m".into(),
            file: None,
            line: None,
        }
    }

    #[test]
    fn tudo_verde_passa_os_gates() {
        let r = ValueReview::from_evidence(&ev(vec![step(0, vec![]), step(0, vec![])]));
        assert_eq!(r.technical, 1.0);
        assert_eq!(r.security, 1.0);
        assert!(r.gates_passed);
        assert!(r.gate_triggered.is_none());
    }

    #[test]
    fn finding_critico_reprova_por_gate_mesmo_com_technical_alto() {
        // 1 de 1 passo passou (technical 1.0), mas há finding crítico → gate.
        let r = ValueReview::from_evidence(&ev(vec![step(0, vec![finding("critical")])]));
        assert_eq!(r.technical, 1.0);
        assert!(!r.gates_passed);
        assert_eq!(r.gate_triggered, Some(GateTriggered::CriticalFinding));
    }

    #[test]
    fn passo_falho_reprova_por_verify_fail() {
        let r = ValueReview::from_evidence(&ev(vec![step(0, vec![]), step(1, vec![])]));
        assert_eq!(r.technical, 0.5);
        assert!(!r.gates_passed);
        assert_eq!(r.gate_triggered, Some(GateTriggered::VerifyFail));
    }

    #[test]
    fn muitos_findings_derrubam_seguranca_abaixo_do_piso() {
        // 2 findings error (0.4 cada) = 0.8 penalidade → security 0.2 < 0.5.
        let r = ValueReview::from_evidence(&ev(vec![step(
            0,
            vec![finding("error"), finding("error")],
        )]));
        assert!((r.security - 0.2).abs() < 1e-9);
        assert!(!r.gates_passed);
        assert_eq!(r.gate_triggered, Some(GateTriggered::SecurityFloor));
    }
}
