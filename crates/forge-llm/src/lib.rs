//! Gateway LLM da plataforma Forge.
//!
//! Fase 1 (scaffold): classificação `ModelTier` (porta do fork do opencode),
//! contrato de provider e cadeia de fallback. As chamadas HTTP reais
//! (Anthropic/OpenAI/DeepSeek, streaming SSE) chegam ainda na Fase 1 do
//! roadmap; as API keys vivem exclusivamente neste processo — o sidecar
//! Python só conhece o socket gRPC.

pub mod model_tier;
pub mod provider;

pub use model_tier::{tier_from_id, ModelTier};
pub use provider::{FallbackChain, LlmRequest, ProviderId};
