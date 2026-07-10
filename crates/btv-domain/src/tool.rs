//! Contrato de ferramenta e seus tipos de dado (D1t).
//!
//! O trait `Tool` e os tipos `ToolOutput`/`ToolError`/`DiffLine` nasceram
//! em `btv-tools` e MORAM no domínio desde o D1t: o loop de agente
//! (`btv-core`) executa ferramentas via `ToolsPort` sem conhecer as
//! implementações (bash/edit/sandbox/MCP/LSP continuam em `btv-tools`,
//! que re-exporta estes tipos para os consumidores existentes). O CÁLCULO
//! de diff (`line_diff`) é implementação e fica em `btv-tools`; aqui mora
//! só o tipo que o loop repassa aos observadores.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("argumentos inválidos: {0}")]
    InvalidArgs(String),
    #[error("falha de execução: {0}")]
    Execution(String),
}

/// Uma linha de diff (contexto/remoção/adição) — produzida pelo `edit` e
/// consumida pela TUI/web para o bloco colorido. Definição BYTE-IDÊNTICA à
/// que sempre morou em `btv-tools::diff` (movimento de fronteira não muda
/// wire — representação serde default de tuple-variant preservada).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLine {
    Context(String),
    Removed(String),
    Added(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    /// Quando o output excede o limite, ele é truncado e o restante vai
    /// para um arquivo gerenciado (Managed Tool Output File).
    pub truncated: bool,
    /// Caminho (relativo à raiz do workspace) do output completo, quando
    /// truncado e persistido por `bound_output_managed` (btv-tools).
    pub overflow_path: Option<String>,
    /// Diff de linhas, quando a ferramenta alterou um arquivo texto
    /// (hoje: `edit`).
    pub diff: Option<Vec<DiffLine>>,
}

/// Contrato de ferramenta: identidade estável, schema para o modelo,
/// escopo para o motor de permissões e execução com args JSON.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema dos argumentos, anunciado ao modelo.
    fn input_schema(&self) -> Value;
    /// Escopo avaliado pelo motor de permissões (caminho, comando...).
    fn scope(&self, args: &Value) -> String;
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError>;
}
