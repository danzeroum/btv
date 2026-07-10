//! Representação de conversa e tool-use, neutra de provider (D1t).
//!
//! Estes tipos nasceram em `btv-llm::chat` e MORAM no domínio desde o D1t
//! (violação 4 do levantamento): o loop de agente (`btv-core`) trabalha só
//! com eles via `LlmPort`, sem conhecer HTTP/provider — a conversão para o
//! formato de cada provider (Anthropic Messages / OpenAI Chat Completions)
//! continua dentro do gateway (`btv-llm`), que os re-exporta para os
//! consumidores existentes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// Bloco de conteúdo de uma mensagem.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    /// Pedido do modelo para executar uma ferramenta.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// Resultado devolvido ao modelo.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }
}

/// Especificação de ferramenta anunciada ao modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema dos argumentos.
    pub input_schema: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Other,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Turno completo do assistente, agregado a partir do stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurn {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
    /// Provider que atendeu a chamada (para telemetria/ledger).
    pub provider: String,
}

impl AssistantTurn {
    /// Pedidos de ferramenta contidos no turno.
    pub fn tool_uses(&self) -> Vec<(&str, &str, &Value)> {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.as_str(), name.as_str(), input))
                }
                _ => None,
            })
            .collect()
    }

    /// Texto concatenado do turno.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Pedido de geração vindo do loop de agente.
#[derive(Debug, Clone)]
pub struct GenerateRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
    pub temperature: Option<f64>,
}

/// Classe de capacidade do modelo — decide política (compaction, rate
/// limit por tier). SÓ o enum mora aqui: a CLASSIFICAÇÃO de um model id em
/// tier (`tier_from_id`, regex sobre nomes de mercado) é conhecimento de
/// provider e fica em `btv-llm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Small,
    Medium,
    Large,
}

impl ModelTier {
    /// Fração da janela de contexto em que a compaction dispara: modelos
    /// small compactam antecipadamente (~75%), demais no padrão (~90%).
    pub fn compaction_threshold(self) -> f64 {
        match self {
            ModelTier::Small => 0.75,
            _ => 0.90,
        }
    }
}
