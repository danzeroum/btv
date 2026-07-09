//! Job de `/verify` em background com polling (movido de `lib.rs` na C2 —
//! código intacto, incluindo o check-and-reserve atômico e o `catch_unwind`).

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::{now_rfc3339, AppState, ErrorBody};

pub(crate) type VerifyJobSlot = Arc<Mutex<Option<VerifyJob>>>;

#[derive(Clone)]
pub(crate) struct VerifyJob {
    run_id: String,
    status: VerifyJobStatus,
}

#[derive(Clone)]
enum VerifyJobStatus {
    Running {
        step: usize,
        total: usize,
    },
    Done {
        evidence: btv_schemas::verification::VerificationEvidence,
    },
    /// Panic dentro do pipeline (bug interno no glue — os passos em si nunca
    /// panicam, devolvem `Result`). Sem este estado o slot ficaria "running"
    /// para sempre, sem crash visível em lugar nenhum (risco aceito na Onda
    /// 11, fechado na validação de pendencias.md).
    Failed {
        message: String,
    },
}

#[derive(Serialize)]
struct VerifyRunStarted {
    run_id: String,
}

/// `POST /api/verify/run` (Fase 7 Onda 11) — dispara o pipeline `/verify`
/// em background (`spawn_blocking`, os passos são subprocessos reais e
/// bloqueantes) usando a MESMA config que `btv verify`: `btv.toml` na
/// raiz do workspace, ou `default_steps()` (espelha o job `rust` do CI) na
/// ausência dele — não uma segunda fonte de verdade sobre o que roda. A
/// resposta imediata é só o `run_id`; o cliente acompanha via `GET
/// /api/verify/:id` (polling). Execuções concorrentes são serializadas: só
/// 1 job por vez — `409` com o `run_id` já em andamento em vez de dois
/// pipelines disputando o mesmo `target/`.
pub(crate) async fn run_verify_start(State(state): State<AppState>) -> Response {
    let run_id = new_verify_run_id();
    {
        // Check-and-reserve ATÔMICO sob um ÚNICO lock: verifica se há job
        // rodando e, no mesmo escopo, reserva o slot. Antes eram dois locks
        // separados — entre eles o mutex era liberado, então dois POST
        // concorrentes liam `None` e AMBOS reservavam (dois pipelines no mesmo
        // `target/` + o slot do primeiro sobrescrito), retornando os dois `202`.
        // É a corrida que o teste `verify-concurrency` pegou de forma
        // intermitente; o `409` só é garantido com a checagem-e-reserva atômica.
        let mut guard = state.verify_job.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = guard.as_ref() {
            if matches!(job.status, VerifyJobStatus::Running { .. }) {
                return (
                    StatusCode::CONFLICT,
                    Json(VerifyRunStarted {
                        run_id: job.run_id.clone(),
                    }),
                )
                    .into_response();
            }
        }
        *guard = Some(VerifyJob {
            run_id: run_id.clone(),
            status: VerifyJobStatus::Running { step: 0, total: 0 },
        });
    }

    let job_slot = Arc::clone(&state.verify_job);
    let root = state.root.clone();
    let run_id_for_task = run_id.clone();
    tokio::task::spawn_blocking(move || {
        let config_path = root.join("btv.toml");
        let steps = match btv_verify::config::load_config(&config_path) {
            Ok(Some(cfg)) => cfg.to_step_specs(),
            _ => btv_verify::config::default_steps(),
        };
        let sha = verify_git_sha(&root).unwrap_or_else(|| "unknown".to_string());
        let produced_at = now_rfc3339();
        let progress_slot = Arc::clone(&job_slot);
        // `catch_unwind`: um panic aqui seria engolido pelo `JoinHandle`
        // descartado do `spawn_blocking` e o job ficaria "running" eterno.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            btv_verify::run_pipeline_with_progress(
                &run_id_for_task,
                &sha,
                &produced_at,
                &steps,
                move |step, total, _completed| {
                    let mut guard = progress_slot.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = guard.as_mut() {
                        job.status = VerifyJobStatus::Running { step, total };
                    }
                },
            )
        }));
        settle_verify_job(&job_slot, result);
    });

    (StatusCode::ACCEPTED, Json(VerifyRunStarted { run_id })).into_response()
}

/// Registra o desfecho do job no slot — inclusive um panic capturado por
/// `catch_unwind` (vira `Failed`, nunca "running" eterno).
fn settle_verify_job(
    job_slot: &VerifyJobSlot,
    result: std::thread::Result<btv_schemas::verification::VerificationEvidence>,
) {
    let mut guard = job_slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(job) = guard.as_mut() {
        job.status = match result {
            Ok(evidence) => VerifyJobStatus::Done { evidence },
            Err(panic) => VerifyJobStatus::Failed {
                message: panic_to_message(panic.as_ref()),
            },
        };
    }
}

fn panic_to_message(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic sem mensagem".to_string()
    }
}

/// `GET /api/verify/:id` — status do job (polling). `404` se não houver
/// nenhum job, ou se `id` não bater com o job atual (só 1 slot — um job novo
/// substitui o anterior, e um reinício do servidor perde o registro).
pub(crate) async fn get_verify_status(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let guard = state.verify_job.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(job) if job.run_id == id => match &job.status {
            VerifyJobStatus::Running { step, total } => Json(serde_json::json!({
                "status": "running",
                "run_id": job.run_id,
                "step": step,
                "total": total,
            }))
            .into_response(),
            VerifyJobStatus::Done { evidence } => Json(serde_json::json!({
                "status": "done",
                "run_id": job.run_id,
                "evidence": evidence,
                // Review por valor DERIVADO da evidência real (technical/
                // security + gates duros) — substitui o mock do frontend.
                "review": btv_schemas::review::ValueReview::from_evidence(evidence),
            }))
            .into_response(),
            VerifyJobStatus::Failed { message } => Json(serde_json::json!({
                "status": "failed",
                "run_id": job.run_id,
                "message": message,
            }))
            .into_response(),
        },
        _ => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "verify_run_not_found",
                format!("run '{id}' não encontrado"),
            )),
        )
            .into_response(),
    }
}

fn new_verify_run_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("run-{:x}", nanos & 0xffff_ffff_ffff)
}

/// Mesma lógica de `btv-cli`'s `git_sha()` (duplicada, não importada —
/// direção de dependência oposta), só que com `current_dir` explícito em vez
/// de confiar no cwd ambiente do processo: o dashboard resolve tudo contra
/// `state.root`, não o cwd real do binário.
fn verify_git_sha(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um panic dentro do pipeline (bug interno de glue) assenta o slot em
    /// `Failed` com a mensagem do panic — nunca um "running" eterno que o
    /// polling não consegue distinguir de um job vivo. (Movido de `lib.rs`
    /// junto com o código na C2.)
    #[test]
    fn panic_no_pipeline_assenta_o_job_em_failed_nao_running_eterno() {
        let slot: VerifyJobSlot = Arc::new(Mutex::new(Some(VerifyJob {
            run_id: "vrun-panic".into(),
            status: VerifyJobStatus::Running { step: 0, total: 0 },
        })));
        let result =
            std::panic::catch_unwind(|| -> btv_schemas::verification::VerificationEvidence {
                panic!("bug interno de glue")
            });
        settle_verify_job(&slot, result);
        let guard = slot.lock().unwrap();
        match &guard.as_ref().unwrap().status {
            VerifyJobStatus::Failed { message } => {
                assert!(message.contains("bug interno de glue"));
            }
            _ => panic!("esperava VerifyJobStatus::Failed"),
        }
    }
}
