//! Telas admin pequenas: experimentos A/B, rate limits e skills (movido de
//! `lib.rs` na C2 — código intacto).

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use btv_llm::model_tier::ModelTier;
use btv_llm::rate_limit::RateLimiter;
use btv_schemas::experiment::{ExperimentReport, VariantStats};
use serde::Serialize;

use crate::{now_rfc3339, AppState, ErrorBody};

/// `GET /api/experiment/:nome` (Fase 7 Onda 9, A2) — relatório de A/B sobre a
/// telemetria real. Mesma validação que `run_experiment` já aplica na CLI
/// (`main.rs`): aceita 2+ variantes (multivariante, Bonferroni). `404` quando o experimento não
/// tem nenhum evento (`props.experiment` nunca bateu); `422` quando tem
/// eventos mas só 1 variante distinta (não dá pra comparar) — a requisição em
/// si é válida, é o experimento que não está no formato certo. Nenhum DTO
/// novo: `ExperimentReport` já deriva `Serialize`+`JsonSchema` (`experiment.v1`).
pub(crate) async fn get_experiment(
    State(state): State<AppState>,
    AxumPath(nome): AxumPath<String>,
) -> Response {
    let variants = state.telemetry.experiment_variants(&nome);
    if variants.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "experiment_not_found",
                format!("nenhum evento com props.experiment='{nome}' na telemetria"),
            )),
        )
            .into_response();
    }
    // Multivariante: 2+ variantes são aceitas (correção de Bonferroni). Só
    // uma variante (`len() == 1`) não é um experimento comparável → 422.
    if variants.len() < 2 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new(
                "experiment_needs_variants",
                format!(
                    "um experimento comparável exige >=2 variantes; '{nome}' tem {}",
                    variants.len()
                ),
            )),
        )
            .into_response();
    }
    let stats: Vec<VariantStats> = variants
        .into_iter()
        .map(|(v, n, s)| VariantStats::new(v, n, s))
        .collect();
    let report = ExperimentReport::from_variants(nome, "success_rate", stats, now_rfc3339());
    Json(report).into_response()
}

#[derive(Serialize)]
struct RateLimitTierEntry {
    tier: ModelTier,
    cap: usize,
    window_secs: u64,
}

/// `GET /api/ratelimit` (Fase 7 Onda 10, A4) — os TETOS configurados
/// (`RateLimiter::for_tier`), um por tier. **Não é uso ao vivo**: cada
/// requisição constrói um `RateLimiter` novo e vazio — o limitador que
/// realmente governa uma sessão vive dentro do processo `btv run`/`chat`/
/// `tui` daquela sessão (`RateLimitedGenerator`), um processo diferente do
/// `btv dashboard` que serve esta rota; não há estado compartilhado para
/// ler. A tela mostra isso explicitamente, não finge um "usado" que não
/// existe. Sem campo "models": `ModelTier` classifica por regex, não por uma
/// lista enumerável de ids — inventar uma lista de exemplo seria fabricar
/// dado (régua Nada Fake).
pub(crate) async fn rate_limits() -> impl IntoResponse {
    let entries: Vec<RateLimitTierEntry> = [ModelTier::Small, ModelTier::Medium, ModelTier::Large]
        .into_iter()
        .map(|tier| {
            let limiter = RateLimiter::for_tier(tier);
            RateLimitTierEntry {
                tier,
                cap: limiter.max_requests(),
                window_secs: limiter.window().as_secs(),
            }
        })
        .collect();
    Json(entries)
}

/// Lista as skills (built-in de `skills/` + terceiro de `.btv/skills/`) com o
/// status REAL do vetter — o que liga a tela admin `skills` ao mecanismo (o
/// mock `vetSkill` do frontend vira este fetch). Read-only: o vetter decide, o
/// usuário não sobrepõe (a régua fail-closed da fase).
pub(crate) async fn skills(State(state): State<AppState>) -> impl IntoResponse {
    use btv_verify::vetter::list_skill_statuses;
    let mut all = list_skill_statuses(&state.root.join("skills"), "builtin");
    all.extend(list_skill_statuses(
        &state.root.join(".btv").join("skills"),
        "third-party",
    ));
    Json(all)
}
