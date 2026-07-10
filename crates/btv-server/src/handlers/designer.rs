//! Rota do Squad Designer (movido de `lib.rs` na C2 — código intacto).
//! Fora da lista indicativa de E8: rota única com semântica própria
//! ("salvar honesto") em vez de mistura com o módulo `ledger`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use btv_schemas::workflow::SquadWorkflow;
use serde::Serialize;

use crate::{db_error, now_rfc3339, AppState, ErrorBody};

#[derive(Serialize)]
struct SaveWorkflowResponse {
    seq: u64,
    workflow_id: &'static str,
}

/// `POST /api/designer/workflow` (Fase 7 Onda 14) — valida o grafo do Squad
/// Designer contra `squad.workflow.v1` (schema + integridade de arestas via
/// `SquadWorkflow::validate_edges`) e grava no ledger (mesmo
/// `LedgerStore::append` que toda outra escrita de auditoria já usa — zero
/// mudança de ledger). "Salvar honesto": confirma que o grafo foi validado e
/// persistido, nunca que o orquestrador passou a usá-lo — os 5 agentes
/// fixos do `UnifiedOrchestrator` continuam decidindo, sem reescrita nesta
/// fase (aplicar o grafo real é trabalho futuro).
pub(crate) async fn save_workflow(
    State(state): State<AppState>,
    Json(workflow): Json<SquadWorkflow>,
) -> Response {
    if let Err(e) = workflow.validate_edges() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new("invalid_workflow", e)),
        )
            .into_response();
    }
    let payload = match serde_json::to_value(&workflow) {
        Ok(v) => v,
        Err(e) => return db_error(e),
    };
    let entry = btv_schemas::ledger::LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: "designer.workflow_saved".into(),
        actor: "web:designer".into(),
        payload,
        r#override: None,
        fake_marker: None,
        ts: now_rfc3339(),
        // Porta legada (sem contexto): cai na cadeia LOCAL, corpo idêntico
        // ao de sempre (decisão do B2 reusada no B3 — pendencias.md).
        tenant: None,
    };
    let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
    match ledger.append(entry) {
        Ok(saved) => (
            StatusCode::CREATED,
            Json(SaveWorkflowResponse {
                seq: saved.seq,
                workflow_id: "squad.workflow.v1",
            }),
        )
            .into_response(),
        Err(e) => db_error(e),
    }
}
