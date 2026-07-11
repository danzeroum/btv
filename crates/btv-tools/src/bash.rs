//! Ferramenta `bash`: executa um comando shell no workspace, com timeout.

use crate::{
    bound_output_managed, required_str, Tool, ToolError, ToolOutput, DEFAULT_OUTPUT_LIMIT,
};
use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct BashTool {
    pub root: PathBuf,
}

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// Drena um pipe do filho em uma thread dedicada. Precisa ser concorrente com
/// o `wait`: um comando cuja saída excede o buffer do pipe (~64KB no Linux)
/// bloquearia na escrita e nunca sairia se lêssemos só APÓS o processo
/// terminar — o loop de timeout o mataria por engano. Lendo em paralelo, o
/// pipe nunca enche. A thread termina ao receber EOF (quando o filho fecha o
/// pipe, seja por sair ou por ser morto no timeout).
fn drena<R: Read + Send + 'static>(fonte: Option<R>) -> Option<std::thread::JoinHandle<String>> {
    fonte.map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = r.read_to_string(&mut buf);
            buf
        })
    })
}

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Executa um comando shell (sh -c) na raiz do workspace e retorna stdout+stderr. Timeout padrão de 120s."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "comando a executar"},
                "timeout_ms": {"type": "integer", "description": "timeout em milissegundos (máx 600000)"}
            },
            "required": ["command"]
        })
    }

    fn scope(&self, args: &Value) -> String {
        args["command"].as_str().unwrap_or("").to_string()
    }

    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let command = required_str(args, "command")?;
        let timeout = Duration::from_millis(
            args["timeout_ms"]
                .as_u64()
                .unwrap_or(DEFAULT_TIMEOUT_MS)
                .min(MAX_TIMEOUT_MS),
        );

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::Execution(format!("spawn: {e}")))?;

        // Drena os pipes em paralelo com a espera (ver `drena`): sem isso, uma
        // saída grande encheria o buffer do pipe e o comando travaria.
        let out_thread = drena(child.stdout.take());
        let err_thread = drena(child.stderr.take());

        let start = Instant::now();
        let status = loop {
            match child
                .try_wait()
                .map_err(|e| ToolError::Execution(e.to_string()))?
            {
                Some(status) => break status,
                None if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ToolError::Execution(format!(
                        "timeout de {}ms excedido",
                        timeout.as_millis()
                    )));
                }
                None => std::thread::sleep(Duration::from_millis(25)),
            }
        };

        // O filho saiu: as threads de drenagem já viram (ou verão em seguida) o
        // EOF do pipe. `join` recolhe o que leram, na mesma ordem de antes
        // (stdout e depois stderr).
        let mut output = out_thread.and_then(|t| t.join().ok()).unwrap_or_default();
        if let Some(t) = err_thread {
            output.push_str(&t.join().unwrap_or_default());
        }
        if !status.success() {
            output.push_str(&format!("\n[exit code: {}]", status.code().unwrap_or(-1)));
        }
        bound_output_managed(&self.root, output, DEFAULT_OUTPUT_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> (tempfile::TempDir, BashTool) {
        let dir = tempfile::tempdir().unwrap();
        let tool = BashTool {
            root: dir.path().to_path_buf(),
        };
        (dir, tool)
    }

    #[test]
    fn executa_no_diretorio_do_workspace() {
        let (dir, tool) = tool();
        std::fs::write(dir.path().join("x.txt"), "").unwrap();
        let out = tool.run(&json!({"command": "ls"})).unwrap();
        assert!(out.content.contains("x.txt"));
    }

    #[test]
    fn falha_inclui_exit_code() {
        let (_dir, tool) = tool();
        let out = tool.run(&json!({"command": "exit 3"})).unwrap();
        assert!(out.content.contains("[exit code: 3]"));
    }

    #[test]
    fn timeout_mata_o_processo() {
        let (_dir, tool) = tool();
        let err = tool
            .run(&json!({"command": "sleep 5", "timeout_ms": 100}))
            .unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn saida_grande_nao_estoura_timeout() {
        let (_dir, tool) = tool();
        // 100KB de saída (> buffer do pipe ~64KB): com leitura só APÓS o wait,
        // o comando bloquearia na escrita e viraria timeout falso. Com a
        // drenagem concorrente deve retornar Ok, com timeout folgado.
        let out = tool
            .run(&json!({
                "command": "head -c 100000 /dev/zero | tr '\\0' a",
                "timeout_ms": 15000
            }))
            .unwrap();
        assert!(out.content.contains('a'));
        // 100KB > DEFAULT_OUTPUT_LIMIT (32KB) → trunca inline e persiste o resto.
        assert!(out.truncated);
    }
}
