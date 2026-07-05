//! Provider Anthropic (Messages API, streaming SSE).
//!
//! A agregação de eventos → `AssistantTurn` é separada do transporte HTTP
//! para ser testável com fixtures (modo "cassette" chega com o replay da
//! Fase 6; aqui os testes cobrem o agregador).

use crate::chat::{
    AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, Role, StopReason, Usage,
};
use serde_json::{json, Value};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2023-06-01";

/// Monta o corpo do POST /v1/messages.
pub fn build_request_body(req: &GenerateRequest) -> Value {
    let messages: Vec<Value> = req.messages.iter().map(message_to_json).collect();
    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "system": req.system,
        "messages": messages,
        "stream": true,
    });
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if !req.tools.is_empty() {
        body["tools"] = Value::Array(
            req.tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect(),
        );
    }
    body
}

fn message_to_json(msg: &ChatMessage) -> Value {
    let role = match msg.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let content: Vec<Value> = msg
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => json!({"type": "text", "text": text}),
            ContentBlock::ToolUse { id, name, input } => {
                json!({"type": "tool_use", "id": id, "name": name, "input": input})
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": content,
                "is_error": is_error,
            }),
        })
        .collect();
    json!({"role": role, "content": content})
}

/// Agrega os eventos SSE da Messages API num turno completo.
#[derive(Default)]
pub struct TurnAggregator {
    blocks: Vec<PartialBlock>,
    stop_reason: Option<StopReason>,
    usage: Usage,
}

enum PartialBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        json: String,
    },
}

impl TurnAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processa um evento (`data` já em JSON). Retorna o delta de texto,
    /// se houver, para streaming no terminal.
    pub fn handle(&mut self, data: &str) -> Option<String> {
        let value: Value = serde_json::from_str(data).ok()?;
        match value["type"].as_str()? {
            "message_start" => {
                self.usage.input_tokens = value["message"]["usage"]["input_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                None
            }
            "content_block_start" => {
                let block = &value["content_block"];
                match block["type"].as_str().unwrap_or("") {
                    "tool_use" => self.blocks.push(PartialBlock::ToolUse {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        json: String::new(),
                    }),
                    _ => self.blocks.push(PartialBlock::Text(
                        block["text"].as_str().unwrap_or("").to_string(),
                    )),
                }
                None
            }
            "content_block_delta" => {
                let delta = &value["delta"];
                match delta["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let text = delta["text"].as_str().unwrap_or("").to_string();
                        if let Some(PartialBlock::Text(t)) = self.blocks.last_mut() {
                            t.push_str(&text);
                        }
                        Some(text)
                    }
                    "input_json_delta" => {
                        if let Some(PartialBlock::ToolUse { json, .. }) = self.blocks.last_mut() {
                            json.push_str(delta["partial_json"].as_str().unwrap_or(""));
                        }
                        None
                    }
                    _ => None,
                }
            }
            "message_delta" => {
                self.stop_reason = value["delta"]["stop_reason"].as_str().map(|s| match s {
                    "end_turn" => StopReason::EndTurn,
                    "tool_use" => StopReason::ToolUse,
                    "max_tokens" => StopReason::MaxTokens,
                    _ => StopReason::Other,
                });
                if let Some(out) = value["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = out;
                }
                None
            }
            _ => None,
        }
    }

    pub fn finish(self) -> AssistantTurn {
        let content = self
            .blocks
            .into_iter()
            .map(|b| match b {
                PartialBlock::Text(text) => ContentBlock::Text { text },
                PartialBlock::ToolUse { id, name, json } => ContentBlock::ToolUse {
                    id,
                    name,
                    input: serde_json::from_str(&json).unwrap_or(Value::Object(Default::default())),
                },
            })
            .collect();
        AssistantTurn {
            content,
            stop_reason: self.stop_reason.unwrap_or(StopReason::EndTurn),
            usage: self.usage,
            provider: "anthropic".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ToolSpec;

    #[test]
    fn agrega_texto_e_tool_use() {
        let mut agg = TurnAggregator::new();
        let events = [
            r#"{"type":"message_start","message":{"usage":{"input_tokens":42}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Vou ler o arquivo."}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_1","name":"read"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"src/main.rs\"}"}}"#,
            r#"{"type":"content_block_stop","index":1}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":17}}"#,
            r#"{"type":"message_stop"}"#,
        ];
        let mut deltas = String::new();
        for e in events {
            if let Some(d) = agg.handle(e) {
                deltas.push_str(&d);
            }
        }
        let turn = agg.finish();
        assert_eq!(deltas, "Vou ler o arquivo.");
        assert_eq!(turn.stop_reason, StopReason::ToolUse);
        assert_eq!(turn.usage.input_tokens, 42);
        assert_eq!(turn.usage.output_tokens, 17);
        let uses = turn.tool_uses();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].1, "read");
        assert_eq!(uses[0].2["path"], "src/main.rs");
    }

    #[test]
    fn corpo_do_request_inclui_tools_e_system() {
        let req = GenerateRequest {
            model: "claude-sonnet-5".into(),
            system: "seja objetivo".into(),
            messages: vec![ChatMessage::user_text("oi")],
            tools: vec![ToolSpec {
                name: "read".into(),
                description: "lê arquivo".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
            max_tokens: 1024,
            temperature: Some(0.2),
        };
        let body = build_request_body(&req);
        assert_eq!(body["system"], "seja objetivo");
        assert_eq!(body["tools"][0]["name"], "read");
        assert_eq!(body["stream"], true);
        assert_eq!(body["temperature"], 0.2);
    }
}
