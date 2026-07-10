//! Providers do gateway (movido de `lib.rs` na C2 — código intacto).

use axum::response::{IntoResponse, Json};
use serde::Serialize;

/// Ordem fixa de fallback que `btv_llm::gateway::Gateway::from_env` usa
/// (Anthropic → DeepSeek → OpenAI). A antiga `btv_llm::FallbackChain` era
/// código morto (`Gateway::generate` nunca a consultava) e foi removida.
const KNOWN_PROVIDERS: [&str; 3] = ["anthropic", "deepseek", "openai"];

#[derive(Serialize)]
struct ProviderView {
    id: &'static str,
    /// Se a env var da key está definida e não-vazia — a MESMA checagem que
    /// `Gateway::from_env` faz para decidir se o provider entra na cadeia.
    configured: bool,
}

/// `GET /api/providers` (Fase 7 Onda 12, piso) — quais providers uma sessão
/// REAL (`btv run`/`chat`) conseguiria usar agora, lendo os mesmos env
/// vars que `Gateway::from_env` lê. Zero dependência nova (`btv-llm` já
/// é dependência do crate, via `model_tier`/`rate_limit`). Sem mutação: o
/// degrau (reordenar fallback, ajustar teto do rate limiter) fica de fora
/// desta onda — ver `pendencias.md` para o porquê (`FallbackChain` morto +
/// o dashboard não compartilha processo com nenhuma sessão real, mesmo
/// achado da Onda 10 sobre "uso ao vivo").
pub(crate) async fn list_providers() -> impl IntoResponse {
    let gateway = btv_llm::gateway::Gateway::from_env();
    let available: std::collections::HashSet<String> = gateway.available().into_iter().collect();
    let providers: Vec<ProviderView> = KNOWN_PROVIDERS
        .into_iter()
        .map(|id| ProviderView {
            id,
            configured: available.contains(id),
        })
        .collect();
    Json(providers)
}
