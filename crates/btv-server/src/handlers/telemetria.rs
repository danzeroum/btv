//! Telemetria e uso por modelo (movido de `lib.rs` na C2 — código intacto).

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use btv_llm::model_tier::{tier_from_id, ModelTier};
use serde::{Deserialize, Serialize};

use crate::AppState;

pub(crate) async fn summary(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.telemetry.summary())
}

#[derive(Deserialize)]
pub(crate) struct EventsQuery {
    limit: Option<u32>,
}

pub(crate) async fn events(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> impl IntoResponse {
    Json(state.telemetry.recent(q.limit.unwrap_or(50)))
}

#[derive(Serialize)]
struct ModelUsageEntry {
    model: String,
    tier: ModelTier,
    calls: u64,
    cache_hits: u64,
    cache_misses: u64,
    input_tokens: u64,
    output_tokens: u64,
    /// Provider dono do preço tabelado (rótulo), `None` se sem preço.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'static str>,
    /// Custo estimado em USD (tokens reais × preço tabelado). `None` quando o
    /// modelo não tem preço na tabela — nunca fabricado.
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_cost_usd: Option<f64>,
}

#[derive(Serialize)]
struct ModelUsageResponse {
    entries: Vec<ModelUsageEntry>,
    /// Soma dos custos estimados dos modelos COM preço tabelado (USD).
    total_estimated_cost_usd: f64,
    /// Data de referência da tabela de preços (a estimativa envelhece).
    pricing_as_of: &'static str,
}

/// `GET /api/models/usage` (Fase 7 Onda 7, A5; custo na validação de
/// pendencias.md) — agrega os eventos reais (`llm.call`/`cache.hit`/
/// `cache.miss`, gravados com `props.model` e, no `llm.call`, os tokens reais
/// da resposta) por modelo. `tier` é derivado aqui via `tier_from_id`; o custo
/// estimado vem de `pricing::estimate_cost_usd` (tokens reais × preço tabelado
/// estático — uma ESTIMATIVA, com `pricing_as_of`, nunca um custo ao vivo).
pub(crate) async fn model_usage(State(state): State<AppState>) -> impl IntoResponse {
    let mut total = 0.0;
    let entries: Vec<ModelUsageEntry> = state
        .telemetry
        .model_usage()
        .into_iter()
        .map(|u| {
            let cost =
                btv_llm::pricing::estimate_cost_usd(&u.model, u.input_tokens, u.output_tokens);
            if let Some(c) = cost {
                total += c;
            }
            ModelUsageEntry {
                tier: tier_from_id(&u.model),
                provider: btv_llm::pricing::price_for(&u.model).map(|p| p.provider),
                estimated_cost_usd: cost,
                model: u.model,
                calls: u.calls,
                cache_hits: u.cache_hits,
                cache_misses: u.cache_misses,
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
            }
        })
        .collect();
    Json(ModelUsageResponse {
        entries,
        total_estimated_cost_usd: total,
        pricing_as_of: btv_llm::pricing::AS_OF,
    })
}
