//! Servidor LSP **fixture** (Fase 6 Onda 5): fala o mínimo do protocolo LSP
//! (framing `Content-Length`, `initialize`, `textDocument/definition`) para o
//! teste de integração hermético do cliente LSP (`forge-tools::lsp`) — o que roda
//! em qualquer lugar, sem depender do rust-analyzer instalado. A prova de
//! semântica de verdade (definição derivada de código real) é o teste contra o
//! rust-analyzer REAL, no CI.
//!
//! O fixture responde `textDocument/definition` com uma Location **conhecida e
//! fixa** (linha 3, char 7): o teste assere que o cliente atravessa o processo,
//! extrai a posição e a devolve por igualdade — provando o framing/handshake/
//! ida-e-volta do cliente. Cargo compila este bin junto dos testes de forge-tools.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

fn write_msg(w: &mut impl Write, v: &Value) {
    let body = serde_json::to_vec(v).unwrap();
    write!(w, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    w.write_all(&body).unwrap();
    w.flush().unwrap();
}

fn read_msg(r: &mut impl BufRead) -> Option<Value> {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(rest) = t.strip_prefix("Content-Length:") {
            len = rest.trim().parse().ok()?;
        }
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn main() {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    while let Some(m) = read_msg(&mut reader) {
        let method = m.get("method").and_then(Value::as_str).unwrap_or("");
        let id = m.get("id").cloned();
        match method {
            "initialize" => {
                write_msg(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0", "id": id, "result": {
                            "capabilities": { "definitionProvider": true, "referencesProvider": true }
                        }
                    }),
                );
            }
            "textDocument/definition" => {
                let uri = m
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|t| t.get("uri"))
                    .cloned()
                    .unwrap_or(Value::Null);
                // Location conhecida e fixa: linha 3, char 7.
                write_msg(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0", "id": id, "result": [{
                            "uri": uri,
                            "range": {
                                "start": { "line": 3, "character": 7 },
                                "end": { "line": 3, "character": 11 }
                            }
                        }]
                    }),
                );
            }
            "shutdown" => write_msg(&mut writer, &json!({"jsonrpc":"2.0","id":id,"result":null})),
            "exit" => break,
            _ => {
                // Requests desconhecidos com id → responde null; notificações → ignora.
                if let Some(id) = id {
                    write_msg(&mut writer, &json!({"jsonrpc":"2.0","id":id,"result":null}));
                }
            }
        }
    }
}
