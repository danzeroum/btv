//! Pipeline de verificação determinística (`/verify`).
//!
//! Porta do `script/verify.ts` do fork do opencode: roda passos
//! configuráveis (typecheck → test → lint → SAST) como subprocessos com
//! timeout e produz `verification-evidence.v1` — evidência estruturada que
//! o Auditor do squad consome no lugar de opinião de LLM.
//!
//! Fase 1 (scaffold): execução sequencial de passos e montagem da
//! evidência. Timeouts, SAST e o skill-vetter completam a Fase 5.

use forge_schemas::verification::{VerificationEvidence, VerificationStep};
use std::process::Command;
use std::time::Instant;

/// Um passo declarado do pipeline (comando + args).
#[derive(Debug, Clone)]
pub struct StepSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
}

/// Executa os passos em ordem e monta a evidência. Passos após uma falha
/// ainda rodam — a evidência registra todos os resultados.
pub fn run_pipeline(
    run_id: &str,
    git_sha: &str,
    produced_at: &str,
    steps: &[StepSpec],
) -> VerificationEvidence {
    let executed: Vec<VerificationStep> = steps
        .iter()
        .map(|spec| {
            let start = Instant::now();
            let exit_code = Command::new(&spec.program)
                .args(&spec.args)
                .output()
                .map(|out| out.status.code().unwrap_or(-1))
                .unwrap_or(-1);
            VerificationStep {
                name: spec.name.clone(),
                tool: format!("{} {}", spec.program, spec.args.join(" "))
                    .trim()
                    .to_string(),
                exit_code,
                duration_ms: start.elapsed().as_millis() as u64,
                findings: vec![],
            }
        })
        .collect();

    let verdict = VerificationEvidence::derive_verdict(&executed);
    VerificationEvidence {
        run_id: run_id.to_string(),
        git_sha: git_sha.to_string(),
        steps: executed,
        verdict,
        produced_at: produced_at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_schemas::verification::Verdict;

    #[test]
    fn passo_verdadeiro_passa_e_falso_falha() {
        let evidence = run_pipeline(
            "run-1",
            "deadbeef",
            "2026-07-05T00:00:00Z",
            &[
                StepSpec {
                    name: "ok".into(),
                    program: "true".into(),
                    args: vec![],
                },
                StepSpec {
                    name: "falha".into(),
                    program: "false".into(),
                    args: vec![],
                },
            ],
        );
        assert!(matches!(evidence.verdict, Verdict::Fail));
        assert_eq!(evidence.steps.len(), 2);
        assert_eq!(evidence.steps[0].exit_code, 0);
        assert_ne!(evidence.steps[1].exit_code, 0);
    }

    #[test]
    fn evidencia_serializa_para_json() {
        let evidence = run_pipeline("run-2", "cafebabe", "2026-07-05T00:00:00Z", &[]);
        let json = serde_json::to_value(&evidence).unwrap();
        assert_eq!(json["run_id"], "run-2");
        assert_eq!(json["verdict"], "pass");
    }
}
