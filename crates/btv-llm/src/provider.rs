//! Contrato de provider do gateway (ids + request com chave de cache).
//!
//! Origem: proxy seguro do prompte (DeepSeek → OpenAI, keys só no servidor).
//! A ordem de fallback real mora em `Gateway::from_env`.

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

// `FallbackChain` foi removida (validação de pendencias.md): era código morto
// desde a origem — `Gateway::generate` itera `self.providers` direto e nunca
// consultou `next_after`. A ordem real de fallback vive em `Gateway::from_env`
// (Anthropic → DeepSeek → OpenAI).

#[cfg(test)]
mod tests {
    use super::*;

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
