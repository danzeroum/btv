//! Leitura paginada e verificação da cadeia do ledger (movido de `lib.rs`
//! na C2 — código intacto). Contrato congelado pelo golden T1 `ledger`.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};

use crate::{db_error, AppState};

#[derive(Deserialize)]
pub(crate) struct LedgerQuery {
    limit: Option<u32>,
    actor: Option<String>,
}

/// `GET /api/ledger?limit=&actor=` — entradas mais recentes primeiro, mesmo
/// `.btv/btv.db` que a CLI grava via `LedgerStore::append`. O filtro por
/// `actor` é resolvido dentro de `LedgerStore::recent` (SQL, combinado com o
/// `LIMIT`), não aqui.
///
/// Cruzamento de goldens (declarado nos DOIS lados): o contrato DESTA ROTA
/// (status/shape/envelope) é pinado por `ledger.golden.json` (T1); os CORPOS
/// que ela serve, quando produzidos pelo fluxo real, são pinados por
/// `ledger_bodies.golden.json` (harness de `btv-cli`, C3.1) — que serializa
/// `Vec<LedgerEntry>` como o `Json(entries)` daqui. Wrapper/campo novo nesta
/// rota quebra LÁ no T1; mudança de corpo de emissor quebra no ledger_bodies.
pub(crate) async fn list_ledger(
    State(state): State<AppState>,
    Query(q): Query<LedgerQuery>,
) -> Response {
    let ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
    match ledger.recent(q.limit.unwrap_or(50), q.actor.as_deref()) {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => db_error(e),
    }
}

#[derive(Serialize)]
struct VerifyResponse {
    ok: bool,
    verified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// `POST /api/ledger/verify` — percorre a cadeia inteira. Uma corrupção é
/// sinalizada por `ok:false` no corpo, não por um status HTTP de erro: a
/// requisição em si teve sucesso, o que ela relata é que o *dado* está
/// corrompido — a distinção que a tela precisa pra diferenciar "servidor
/// falhou" de "alguém adulterou o ledger".
pub(crate) async fn verify_ledger(State(state): State<AppState>) -> Response {
    let ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
    match ledger.verify_chain() {
        Ok(verified) => Json(VerifyResponse {
            ok: true,
            verified,
            error: None,
        })
        .into_response(),
        Err(btv_store::ledger::LedgerError::BrokenChain { seq, .. }) => Json(VerifyResponse {
            ok: false,
            verified: seq.saturating_sub(1),
            error: Some(format!("cadeia corrompida na seq {seq}")),
        })
        .into_response(),
        Err(e) => db_error(e),
    }
}
