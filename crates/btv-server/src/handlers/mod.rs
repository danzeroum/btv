//! Handlers HTTP do dashboard, por área — C2 da Trilha C do plano DDD
//! multitenant: decomposição MECÂNICA do `lib.rs` (2.322 linhas), movimento
//! puro com zero mudança de contrato (goldens T1 como rede; nenhuma fixture
//! regravada).
//!
//! A lista segue o levantamento E8 (`{telemetria,providers,ledger,admin,
//! verify}`) com dois módulos além da lista indicativa: `prompts` (CRUD de 4
//! rotas com helpers próprios) e `designer` (rota do Squad Designer) — cada
//! um pequeno demais para justificar mistura com área alheia. O módulo `btv`
//! (rota do catálogo de templates) já era módulo próprio e fica onde está.
//!
//! Regra vigiada pelo lint T4-B do CI: nenhum SQL cru aqui — persistência só
//! por tipos de `btv-store` (hoje) ou pelos ports do domínio (Trilha B).

pub(crate) mod admin;
pub(crate) mod designer;
pub(crate) mod ledger;
pub(crate) mod prompts;
pub(crate) mod providers;
pub(crate) mod telemetria;
pub(crate) mod verify;

// Vocabulário COMPARTILHADO de resposta dos handlers (e da guarda de
// Origin) — movido de `lib.rs` na C2 junto com os donos de uso.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Corpo de erro uniforme `{error, code}` das rotas mutáveis. É a **fonte
/// única** do contrato de erro: o agente web do `btv-cli` (`web_agent`)
/// reexporta este tipo (`pub use btv_server::ErrorBody`) em vez de manter uma
/// cópia — a dependência vai `btv-cli → btv-server` (B6 do roadmap). Campos
/// privados; construir só por `ErrorBody::new`.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    error: String,
    code: String,
}

impl ErrorBody {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            code: code.to_string(),
        }
    }
}

pub(crate) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

pub(crate) fn db_error(message: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody::new("prompt_library_error", message.to_string())),
    )
        .into_response()
}
