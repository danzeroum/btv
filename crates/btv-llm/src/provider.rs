//! Contrato de provider e cadeia de fallback do gateway.
//!
//! Origem: proxy seguro do prompte (DeepSeek → OpenAI, keys só no servidor).
//! A cadeia é configurável; a implementação HTTP real (streaming SSE, retry)
//! completa a Fase 1 do roadmap.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Anthropic,
    Openai,
    Deepseek,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

impl LlmRequest {
    /// Chave de cache do request (contrato `prompt-cache-key.v1`).
    pub fn cache_key(&self) -> String {
        let temperature = self
            .temperature
            .map(|t| serde_json::json!(t))
            .unwrap_or(Value::Null);
        btv_schemas::request_hash(&self.messages, &temperature)
    }
}

/// Ordem de tentativa de providers; o primeiro que responder vence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackChain {
    pub providers: Vec<ProviderId>,
}

impl Default for FallbackChain {
    fn default() -> Self {
        Self {
            providers: vec![
                ProviderId::Anthropic,
                ProviderId::Deepseek,
                ProviderId::Openai,
            ],
        }
    }
}

impl FallbackChain {
    /// Próximo provider após uma falha, se houver.
    pub fn next_after(&self, failed: &ProviderId) -> Option<&ProviderId> {
        let idx = self.providers.iter().position(|p| p == failed)?;
        self.providers.get(idx + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_avanca_na_cadeia() {
        let chain = FallbackChain::default();
        assert_eq!(
            chain.next_after(&ProviderId::Anthropic),
            Some(&ProviderId::Deepseek)
        );
        assert_eq!(chain.next_after(&ProviderId::Openai), None);
    }

    #[test]
    fn cache_key_e_estavel() {
        let req = LlmRequest {
            model: "deepseek-chat".into(),
            messages: serde_json::json!([{"role": "user", "content": "oi"}]),
            temperature: Some(0.7),
            max_tokens: None,
        };
        assert_eq!(req.cache_key(), req.cache_key());
    }
}
