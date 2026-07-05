//! Ferramentas determinísticas da plataforma Forge.
//!
//! Princípio (fork do opencode): "o LLM orquestra; ferramentas
//! determinísticas verificam". Fase 1: read, grep, edit e bash reais sob o
//! motor de permissões; LSP/MCP/webfetch/sandbox chegam nas Fases 2–6.

pub mod bash;
pub mod edit;
pub mod grep;
pub mod read;
pub mod registry;

pub use registry::ToolRegistry;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("argumentos inválidos: {0}")]
    InvalidArgs(String),
    #[error("falha de execução: {0}")]
    Execution(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    /// Quando o output excede o limite, ele é truncado e o restante vai
    /// para um arquivo gerenciado (Managed Tool Output File — Fase 2).
    pub truncated: bool,
}

/// Contrato de ferramenta: identidade estável, schema para o modelo,
/// escopo para o motor de permissões e execução com args JSON.
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// JSON Schema dos argumentos, anunciado ao modelo.
    fn input_schema(&self) -> Value;
    /// Escopo avaliado pelo motor de permissões (caminho, comando...).
    fn scope(&self, args: &Value) -> String;
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError>;
}

/// Limite padrão de bytes devolvidos inline ao contexto do modelo.
pub const DEFAULT_OUTPUT_LIMIT: usize = 32 * 1024;

/// Trunca o output em uma fronteira de char válida, sinalizando truncamento.
pub fn bound_output(content: String, limit: usize) -> ToolOutput {
    if content.len() <= limit {
        return ToolOutput {
            content,
            truncated: false,
        };
    }
    let mut cut = limit;
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    ToolOutput {
        content: content[..cut].to_string(),
        truncated: true,
    }
}

pub(crate) fn required_str<'a>(args: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    args.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArgs(format!("campo '{field}' obrigatório")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncamento_respeita_fronteira_utf8() {
        let out = bound_output("aça".repeat(10), 5);
        assert!(out.truncated);
        assert!(out.content.len() <= 5);
        assert!(std::str::from_utf8(out.content.as_bytes()).is_ok());
    }
}
