//! CRUD da biblioteca de prompts (movido de `lib.rs` na C2 — código
//! intacto). Fora da lista indicativa de E8: 4 rotas com helpers próprios
//! justificam módulo separado em vez de mistura com área alheia.

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::Value;

use crate::{db_error, now_rfc3339, AppState, ErrorBody};

fn prompt_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody::new("prompt_not_found", "prompt inexistente")),
    )
        .into_response()
}

#[derive(Deserialize)]
pub(crate) struct ListPromptsQuery {
    tag: Option<String>,
}

/// `GET /api/prompts?tag=` — lista os prompts salvos (mais recentes
/// primeiro), opcionalmente filtrados por uma tag exata. Mesma biblioteca
/// (`.btv/prompt_library.db`) que o `/prompt library` do CLI já usa — não
/// uma segunda fonte de verdade.
pub(crate) async fn list_prompts(
    State(state): State<AppState>,
    Query(q): Query<ListPromptsQuery>,
) -> Response {
    let library = state
        .prompt_library
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match library.list(q.tag.as_deref()) {
        Ok(prompts) => Json(prompts).into_response(),
        Err(e) => db_error(e),
    }
}

#[derive(Deserialize)]
pub(crate) struct CreatePromptBody {
    name: String,
    generator: String,
    #[serde(default)]
    fields: Value,
    rendered: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// `POST /api/prompts` — salva um prompt já renderizado (o render em si é
/// `POST /api/prompt/render`, rota separada no router mesclado de
/// `btv-cli`). Devolve o registro completo; `created_at` é gerado pelo
/// servidor, nunca confiado ao corpo da requisição.
pub(crate) async fn create_prompt(
    State(state): State<AppState>,
    Json(body): Json<CreatePromptBody>,
) -> Response {
    let library = state
        .prompt_library
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let created_at = now_rfc3339();
    let id = match library.save(
        &body.name,
        &body.generator,
        &body.fields,
        &body.rendered,
        &body.tags,
        &created_at,
    ) {
        Ok(id) => id,
        Err(e) => return db_error(e),
    };
    match library.get(id) {
        Ok(Some(saved)) => (StatusCode::CREATED, Json(saved)).into_response(),
        Ok(None) => db_error("prompt salvo mas não encontrado logo em seguida"),
        Err(e) => db_error(e),
    }
}

/// `POST /api/prompts/:id/favorite` — inverte o favorito; `404` se o id não existir.
pub(crate) async fn favorite_prompt(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let library = state
        .prompt_library
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match library.toggle_favorite(id) {
        Ok(Some(favorite)) => Json(serde_json::json!({ "favorite": favorite })).into_response(),
        Ok(None) => prompt_not_found(),
        Err(e) => db_error(e),
    }
}

/// `DELETE /api/prompts/:id` — remove; `404` se o id não existir.
pub(crate) async fn delete_prompt(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let library = state
        .prompt_library
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match library.delete(id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => prompt_not_found(),
        Err(e) => db_error(e),
    }
}
