//! Ferramenta `grep`: busca por regex respeitando .gitignore (crate `ignore`).

use crate::{bound_output, required_str, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct GrepTool {
    pub root: PathBuf,
}

const MAX_MATCHES: usize = 200;

impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Busca um padrão (regex) nos arquivos do workspace, respeitando .gitignore. Retorna caminho:linha:conteúdo."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "expressão regular"},
                "path": {"type": "string", "description": "subdiretório ou arquivo (opcional)"}
            },
            "required": ["pattern"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["path"].as_str().unwrap_or(".").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let pattern = required_str(args, "pattern")?;
        let re = regex::Regex::new(pattern)
            .map_err(|e| ToolError::InvalidArgs(format!("regex: {e}")))?;
        let base = match args["path"].as_str() {
            Some(p) => self.root.join(p),
            None => self.root.clone(),
        };

        let mut matches = Vec::new();
        let walker = ignore::WalkBuilder::new(&base)
            .hidden(true)
            .require_git(false) // .gitignore vale mesmo fora de um repo git
            .build();
        'outer: for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue; // binário ou não-UTF8
            };
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    let rel = entry
                        .path()
                        .strip_prefix(&self.root)
                        .unwrap_or(entry.path());
                    matches.push(format!("{}:{}:{}", rel.display(), i + 1, line.trim_end()));
                    if matches.len() >= MAX_MATCHES {
                        matches.push(format!(
                            "... (limite de {MAX_MATCHES} ocorrências atingido)"
                        ));
                        break 'outer;
                    }
                }
            }
        }
        if matches.is_empty() {
            return Ok(ToolOutput {
                content: "nenhuma ocorrência".into(),
                truncated: false,
            });
        }
        Ok(bound_output(matches.join("\n"), DEFAULT_OUTPUT_LIMIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encontra_ocorrencias_e_respeita_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn alvo() {}\n").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/b.rs"), "fn alvo() {}\n").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        let tool = GrepTool {
            root: dir.path().to_path_buf(),
        };
        let out = tool.run(&json!({"pattern": "alvo"})).unwrap();
        assert!(out.content.contains("a.rs:1:"));
        assert!(!out.content.contains("target/"));
    }

    #[test]
    fn regex_invalida_da_erro_de_args() {
        let dir = tempfile::tempdir().unwrap();
        let tool = GrepTool {
            root: dir.path().to_path_buf(),
        };
        assert!(matches!(
            tool.run(&json!({"pattern": "("})),
            Err(ToolError::InvalidArgs(_))
        ));
    }
}
