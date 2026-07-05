//! Provider OpenAI-compatível (Chat Completions, streaming SSE).
//!
//! Cobre OpenAI e DeepSeek (mesmo protocolo, base URL diferente). Tal como
//! no provider Anthropic, a agregação é separada do transporte para ser
//! testável com fixtures.

use crate::chat::{
    AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, Role, StopReason, Usage,
};
use serde_json::{json, Value};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com";
pub const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";

/// Monta o corpo do POST /v1/chat/completions.
pub fn build_request_body(req: &GenerateRequest) -> Value {
    let mut messages = vec![json!({"role": "system", "content": req.system})];
    for msg in &req.messages {
        messages.extend(message_to_json(msg));
    }
    let mut body = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
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
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect(),
        );
    }
    body
}

/// Uma mensagem interna pode virar mais de uma no formato OpenAI
/// (tool_results viram mensagens `role: tool` separadas).
fn message_to_json(msg: &ChatMessage) -> Vec<Value> {
    match msg.role {
        Role::User => {
            let mut out = Vec::new();
            let mut texts = Vec::new();
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => texts.push(text.clone()),
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let body = if *is_error {
                            format!("[erro] {content}")
                        } else {
                            content.clone()
                        };
                        out.push(
                            json!({"role": "tool", "tool_call_id": tool_use_id, "content": body}),
                        );
                    }
                    ContentBlock::ToolUse { .. } => {}
                }
            }
            if !texts.is_empty() {
                out.push(json!({"role": "user", "content": texts.join("\n")}));
            }
            out
        }
        Role::Assistant => {
            let text = msg
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            let tool_calls: Vec<Value> = msg
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => Some(json!({
                        "id": id,
                        "type": "function",
                        "function": {"name": name, "arguments": input.to_string()},
                    })),
                    _ => None,
                })
                .collect();
            let mut m = json!({"role": "assistant"});
            m["content"] = if text.is_empty() {
                Value::Null
            } else {
                json!(text)
            };
            if !tool_calls.is_empty() {
                m["tool_calls"] = Value::Array(tool_calls);
            }
            vec![m]
        }
    }
}

/// Agrega os chunks do Chat Completions num turno completo.
pub struct TurnAggregator {
    provider: String,
    text: String,
    tool_calls: Vec<PartialCall>,
    finish_reason: Option<String>,
    usage: Usage,
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

impl TurnAggregator {
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
            text: String::new(),
            tool_calls: Vec::new(),
            finish_reason: None,
            usage: Usage::default(),
        }
    }

    /// Processa um `data` do stream ("[DONE]" é ignorado). Retorna o delta
    /// de texto, se houver.
    pub fn handle(&mut self, data: &str) -> Option<String> {
        if data.trim() == "[DONE]" {
            return None;
        }
        let value: Value = serde_json::from_str(data).ok()?;
        if let Some(u) = value.get("usage").filter(|u| !u.is_null()) {
            self.usage.input_tokens = u["prompt_tokens"]
                .as_u64()
                .unwrap_or(self.usage.input_tokens);
            self.usage.output_tokens = u["completion_tokens"]
                .as_u64()
                .unwrap_or(self.usage.output_tokens);
        }
        let choice = value["choices"].get(0)?;
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish_reason = Some(reason.to_string());
        }
        let delta = &choice["delta"];
        if let Some(calls) = delta["tool_calls"].as_array() {
            for call in calls {
                let index = call["index"].as_u64().unwrap_or(0) as usize;
                while self.tool_calls.len() <= index {
                    self.tool_calls.push(PartialCall::default());
                }
                let partial = &mut self.tool_calls[index];
                if let Some(id) = call["id"].as_str() {
                    partial.id = id.to_string();
                }
                if let Some(name) = call["function"]["name"].as_str() {
                    partial.name.push_str(name);
                }
                if let Some(args) = call["function"]["arguments"].as_str() {
                    partial.arguments.push_str(args);
                }
            }
        }
        let text = delta["content"].as_str()?.to_string();
        if text.is_empty() {
            return None;
        }
        self.text.push_str(&text);
        Some(text)
    }

    pub fn finish(self) -> AssistantTurn {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(ContentBlock::Text { text: self.text });
        }
        for call in self.tool_calls {
            content.push(ContentBlock::ToolUse {
                id: call.id,
                name: call.name,
                input: serde_json::from_str(&call.arguments)
                    .unwrap_or(Value::Object(Default::default())),
            });
        }
        let stop_reason = match self.finish_reason.as_deref() {
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            Some("stop") | None => StopReason::EndTurn,
            _ => StopReason::Other,
        };
        AssistantTurn {
            content,
            stop_reason,
            usage: self.usage,
            provider: self.provider,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agrega_texto_e_tool_calls_fragmentados() {
        let mut agg = TurnAggregator::new("deepseek");
        let events = [
            r#"{"choices":[{"delta":{"content":"Vou "}}]}"#,
            r#"{"choices":[{"delta":{"content":"verificar."}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"grep","arguments":"{\"pat"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"tern\":\"todo\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":30,"completion_tokens":12}}"#,
            "[DONE]",
        ];
        let mut deltas = String::new();
        for e in events {
            if let Some(d) = agg.handle(e) {
                deltas.push_str(&d);
            }
        }
        let turn = agg.finish();
        assert_eq!(deltas, "Vou verificar.");
        assert_eq!(turn.stop_reason, StopReason::ToolUse);
        assert_eq!(turn.usage.input_tokens, 30);
        let uses = turn.tool_uses();
        assert_eq!(uses[0].1, "grep");
        assert_eq!(uses[0].2["pattern"], "todo");
    }

    #[test]
    fn tool_result_vira_mensagem_role_tool() {
        let msg = ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "3 ocorrências".into(),
                is_error: false,
            }],
        };
        let out = message_to_json(&msg);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["role"], "tool");
        assert_eq!(out[0]["tool_call_id"], "call_1");
    }
}
