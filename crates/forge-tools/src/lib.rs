//! Ferramentas determinísticas da plataforma Forge.
//!
//! Princípio (fork do opencode): "o LLM orquestra; ferramentas
//! determinísticas verificam". Fase 1 (scaffold): contrato de ferramenta e
//! truncamento gerenciado de output. As implementações reais (grep via
//! crates `grep`/`ignore`, edit/patch, bash/PTY, webfetch) completam a
//! Fase 1; LSP/MCP/sandbox chegam na Fase 6.

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

/// Contrato de ferramenta: nome estável + execução com args JSON.
pub trait Tool {
    fn name(&self) -> &'static str;
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

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;
    impl Tool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
            let text = args
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArgs("campo 'text' obrigatório".into()))?;
            Ok(bound_output(text.to_string(), DEFAULT_OUTPUT_LIMIT))
        }
    }

    #[test]
    fn echo_roda_pelo_contrato() {
        let out = EchoTool.run(&serde_json::json!({"text": "oi"})).unwrap();
        assert_eq!(out.content, "oi");
        assert!(!out.truncated);
    }

    #[test]
    fn truncamento_respeita_fronteira_utf8() {
        let out = bound_output("aça".repeat(10), 5);
        assert!(out.truncated);
        assert!(out.content.len() <= 5);
        assert!(std::str::from_utf8(out.content.as_bytes()).is_ok());
    }

    #[test]
    fn args_invalidos_dao_erro() {
        assert!(EchoTool.run(&serde_json::json!({})).is_err());
    }
}
