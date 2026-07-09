//! Decorator de rate limiting do gateway, gated por `ModelTier`
//! (`btv_llm::RateLimiter`) — a ideia dos tiers anon/auth do prompte
//! aplicada ao custo dos modelos. Fica por baixo do `CachedGenerator`:
//! um hit de cache nunca consome uma vaga do limitador.

use crate::session::now_rfc3339;
use btv_llm::chat::{AssistantTurn, GenerateRequest};
use btv_llm::gateway::{GatewayError, Generator};
use btv_llm::RateLimiter;
use btv_store::Telemetry;

pub struct RateLimitedGenerator<G: Generator> {
    inner: G,
    limiter: RateLimiter,
    telemetry: Option<Telemetry>,
}

impl<G: Generator> RateLimitedGenerator<G> {
    pub fn new(inner: G, limiter: RateLimiter, telemetry: Option<Telemetry>) -> Self {
        Self {
            inner,
            limiter,
            telemetry,
        }
    }
}

impl<G: Generator + Sync> Generator for RateLimitedGenerator<G> {
    async fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        self.limiter
            .acquire()
            .await
            .map_err(|e| GatewayError::RateLimited(e.to_string()))?;
        // `model` capturado antes de `req` ser movido para `generate`; o
        // `llm.call` é registrado DEPOIS, com os tokens reais da resposta —
        // é o que permite estimar custo por modelo (tokens × preço). Só
        // chamadas bem-sucedidas contam (as que consomem tokens de verdade;
        // um hit de cache nem chega aqui, pois o `CachedGenerator` fica por
        // cima).
        let model = req.model.clone();
        let result = self.inner.generate(req, on_delta).await;
        if let (Some(t), Ok(turn)) = (&self.telemetry, &result) {
            t.record(
                "llm.call",
                "cli",
                serde_json::json!({
                    "model": model,
                    "input_tokens": turn.usage.input_tokens,
                    "output_tokens": turn.usage.output_tokens,
                }),
                &now_rfc3339(),
            );
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btv_llm::model_tier::ModelTier;

    struct EchoGen;
    impl Generator for EchoGen {
        async fn generate(
            &self,
            _req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            Ok(AssistantTurn {
                provider: "echo".into(),
                content: vec![],
                stop_reason: btv_llm::chat::StopReason::EndTurn,
                usage: btv_llm::chat::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
        }
    }

    #[tokio::test]
    async fn registra_telemetria_de_chamada() {
        let telemetry = Telemetry::open_in_memory().unwrap();
        let gen = RateLimitedGenerator::new(
            EchoGen,
            RateLimiter::for_tier(ModelTier::Large),
            Some(telemetry.clone()),
        );
        gen.generate(
            GenerateRequest {
                model: "x".into(),
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: 16,
                temperature: None,
            },
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(telemetry.summary().by_name.get("llm.call"), Some(&1));
    }
}
