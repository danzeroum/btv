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
    /// Chave de cache do request (contrato `prompt-cache-key.v1`). `Err` quando
    /// a entrada viola a restrição numérica do v1 (ex.: `temperature` 1.0) — o
    /// chamador degrada pulando o cache em vez de gerar uma chave divergente.
    pub fn cache_key(&self) -> Result<String, btv_schemas::CacheKeyError> {
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
        assert!(req.cache_key().is_ok(), "temperatura 0.7 é válida");
    }

    #[test]
    fn cache_key_rejeita_temperatura_float_inteira() {
        let req = LlmRequest {
            model: "deepseek-chat".into(),
            messages: serde_json::json!([{"role": "user", "content": "oi"}]),
            temperature: Some(1.0),
            max_tokens: None,
        };
        assert!(req.cache_key().is_err(), "temperatura 1.0 é proibida no v1");
    }
}
