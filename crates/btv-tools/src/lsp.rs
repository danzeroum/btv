//! Cliente LSP (Fase 6 Onda 5): sobe o language server do projeto
//! (rust-analyzer/pyright conforme o workspace), fala o protocolo LSP **real**
//! (JSON-RPC com framing `Content-Length` sobre stdio) e expõe as consultas
//! semânticas — definição, referências, diagnósticos — como `dyn Tool` no
//! `ToolRegistry`, **sob o mesmo motor de permissões** que qualquer tool.
//!
//! O framing LSP é simples o bastante para não puxar dependência nenhuma: só
//! `serde_json` (que já é dep). Isso mantém o `cargo deny` leve e nos dá controle
//! total — provado por um probe contra o rust-analyzer de verdade (ver o teste de
//! integração real, `lsp_integration.rs`).
//!
//! **Sessão persistente (≠ MCP):** o language server é caro de subir (o
//! rust-analyzer indexa o workspace, ~segundos). Diferente do MCP (connect por
//! chamada), aqui a sessão é **preguiçosa e reusada**: sobe uma vez no primeiro
//! uso e as consultas seguintes reaproveitam o processo já indexado. O processo é
//! morto no `Drop` (lição do process-group da Fase 4 — nada de órfão).

use crate::{bound_output, Tool, ToolError, ToolOutput, ToolRegistry, DEFAULT_OUTPUT_LIMIT};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Quanto esperar o server indexar antes de desistir de uma consulta (o
/// rust-analyzer devolve resultado vazio enquanto indexa; a gente re-tenta).
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Orçamento para os diagnósticos assentarem (são empurrados de forma assíncrona
/// pelo server após o `didOpen`; não há sinal claro de "acabou").
const DIAG_BUDGET: Duration = Duration::from_secs(12);
/// Prazo de UM round-trip request/response. Com o reader de fundo drenando o
/// stdout continuamente, uma resposta que nunca chega vira erro em vez de
/// pendurar a thread da consulta para sempre.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Um language server declarado pelo usuário: o comando que o sobe via stdio, e
/// a raiz do workspace que ele deve analisar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub root: PathBuf,
}

/// As consultas que expomos como tool. Fixas (o LSP oferece um conjunto
/// conhecido), diferente do MCP onde as tools são anunciadas dinamicamente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspQuery {
    Definition,
    References,
    Diagnostics,
    /// Busca por NOME de símbolo (`workspace/symbol`): o server acha as
    /// ocorrências e resolve a posição — o agente não precisa saber
    /// linha/coluna de antemão (fecha a fricção do dogfooding da Onda 5, em
    /// que a tool de posição não compunha com as tools de conteúdo).
    Symbol,
}

impl LspQuery {
    fn as_str(self) -> &'static str {
        match self {
            LspQuery::Definition => "definition",
            LspQuery::References => "references",
            LspQuery::Diagnostics => "diagnostics",
            LspQuery::Symbol => "symbol",
        }
    }
}

/// Estado compartilhado entre a thread de fundo (que lê o stdout do server) e a
/// thread da consulta (que escreve requests e espera respostas). O reader drena
/// o stdout **continuamente** — assim `$/progress`/`publishDiagnostics` e demais
/// notificações nunca se acumulam no buffer do pipe do SO entre consultas
/// (endurecimento registrado na pendência da Onda 5).
#[derive(Default)]
struct Shared {
    inner: Mutex<SharedInner>,
    /// Sinalizado quando chega uma resposta nova ou o server morre.
    cv: Condvar,
    /// Últimos diagnósticos empurrados por URI (`publishDiagnostics`).
    diagnostics: Mutex<HashMap<String, Value>>,
}

#[derive(Default)]
struct SharedInner {
    /// Respostas recebidas mas ainda não consumidas, por `id` do request.
    responses: HashMap<i64, Result<Value, String>>,
    /// `Some(motivo)` quando o reader viu EOF/erro — desperta consultas presas.
    dead: Option<String>,
}

/// Um processo de language server vivo, com os canais e o estado de sessão.
struct LspProc {
    child: Child,
    /// `stdin` é compartilhado com a thread reader (que precisa responder
    /// requests do servidor no meio de um handshake) via `Mutex`.
    stdin: Arc<Mutex<ChildStdin>>,
    next_id: i64,
    /// URIs já abertas (`didOpen` só uma vez por documento — reabrir viola o
    /// protocolo).
    opened: HashSet<String>,
    shared: Arc<Shared>,
    reader: Option<JoinHandle<()>>,
}

impl Drop for LspProc {
    fn drop(&mut self) {
        // Mata o server — sem shutdown/exit handshake (pode travar); o kill é
        // o robusto. Nada de rust-analyzer órfão comendo CPU.
        let _ = self.child.kill();
        let _ = self.child.wait();
        // O kill fecha o stdout → o reader vê EOF e termina; junta pra não
        // deixar thread pendurada (a lição do process-group da Fase 4).
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

/// Loop da thread de fundo: lê cada mensagem do server e a roteia —
/// resposta → `responses` (desperta a consulta), request do servidor →
/// responde na hora (senão o handshake trava), notificação → guarda
/// diagnósticos e **descarta** o resto (drenagem contínua). EOF/erro marca a
/// sessão como morta.
fn reader_loop(
    mut stdout: BufReader<ChildStdout>,
    stdin: Arc<Mutex<ChildStdin>>,
    shared: Arc<Shared>,
) {
    loop {
        let m = match read_msg(&mut stdout) {
            Ok(m) => m,
            Err(e) => {
                let mut inner = shared.inner.lock().unwrap_or_else(|p| p.into_inner());
                inner.dead = Some(e);
                shared.cv.notify_all();
                return;
            }
        };
        let has_method = m.get("method").is_some();
        if !has_method {
            if let Some(id) = m.get("id").and_then(Value::as_i64) {
                let res = match m.get("error") {
                    Some(err) => Err(format!("{err}")),
                    None => Ok(m.get("result").cloned().unwrap_or(Value::Null)),
                };
                let mut inner = shared.inner.lock().unwrap_or_else(|p| p.into_inner());
                inner.responses.insert(id, res);
                shared.cv.notify_all();
            }
            continue;
        }
        if m.get("id").is_some() {
            // request do servidor → responde para não travar o handshake.
            if let Ok(mut w) = stdin.lock() {
                let _ = respond_server_request(&mut *w, &m);
            }
            continue;
        }
        // notificação — guarda diagnósticos, descarta o resto (nada entope).
        if m.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
            if let Some(params) = m.get("params") {
                if let Some(uri) = params.get("uri").and_then(Value::as_str) {
                    shared
                        .diagnostics
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(
                            uri.to_string(),
                            params.get("diagnostics").cloned().unwrap_or(json!([])),
                        );
                }
            }
        }
    }
}

/// A sessão LSP: preguiçosa (sobe no primeiro uso) e compartilhada pelas três
/// tools do mesmo server (`Arc<LspSession>`), para não subir três processos.
pub struct LspSession {
    config: LspServerConfig,
    proc: Mutex<Option<LspProc>>,
}

impl LspSession {
    pub fn new(config: LspServerConfig) -> Self {
        Self {
            config,
            proc: Mutex::new(None),
        }
    }

    /// URI `file://` canônica do arquivo (bate com o que o server devolve).
    fn uri_for(&self, file: &str) -> String {
        let p = PathBuf::from(file);
        let abs = if p.is_absolute() {
            p
        } else {
            self.config.root.join(p)
        };
        let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
        format!("file://{}", abs.display())
    }

    fn read_file(&self, file: &str) -> Result<String, String> {
        let p = PathBuf::from(file);
        let abs = if p.is_absolute() {
            p
        } else {
            self.config.root.join(p)
        };
        std::fs::read_to_string(&abs).map_err(|e| format!("ler {}: {e}", abs.display()))
    }

    /// Sobe o server e faz o handshake (`initialize`/`initialized`) se ainda não
    /// estiver de pé. Chamado sob o lock.
    fn ensure_started(slot: &mut Option<LspProc>, config: &LspServerConfig) -> Result<(), String> {
        if slot.is_some() {
            return Ok(());
        }
        let root = std::fs::canonicalize(&config.root).unwrap_or_else(|_| config.root.clone());
        let root_uri = format!("file://{}", root.display());

        let mut child = Command::new(&config.command)
            .args(&config.args)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("subir LSP '{}': {e}", config.command))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().ok_or("LSP sem stdin")?));
        let stdout = BufReader::new(child.stdout.take().ok_or("LSP sem stdout")?);

        // Reader de fundo já de pé ANTES do handshake: assim o próprio
        // `initialize` (e os requests que o server manda no meio dele) passam
        // pelo mesmo canal, e nada fica bloqueando o pipe.
        let shared = Arc::new(Shared::default());
        let reader = {
            let stdin = Arc::clone(&stdin);
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || reader_loop(stdout, stdin, shared))
        };

        let mut proc = LspProc {
            child,
            stdin,
            next_id: 0,
            opened: HashSet::new(),
            shared,
            reader: Some(reader),
        };

        proc.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "workspaceFolders": [{ "uri": root_uri, "name": "btv" }]
            }),
            REQUEST_TIMEOUT,
        )?;
        {
            let mut w = proc.stdin.lock().map_err(|_| "lock stdin LSP envenenado")?;
            write_msg(
                &mut *w,
                &json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
            )?;
        }

        *slot = Some(proc);
        Ok(())
    }

    fn ensure_open(proc: &mut LspProc, uri: &str, text: &str) -> Result<(), String> {
        if proc.opened.contains(uri) {
            return Ok(());
        }
        let lang = language_id(uri);
        {
            let mut w = proc.stdin.lock().map_err(|_| "lock stdin LSP envenenado")?;
            write_msg(
                &mut *w,
                &json!({
                    "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                        "textDocument": { "uri": uri, "languageId": lang, "version": 1, "text": text }
                    }
                }),
            )?;
        }
        proc.opened.insert(uri.to_string());
        Ok(())
    }

    /// `textDocument/definition` na posição (0-indexed, convenção LSP), com
    /// retry enquanto o server indexa. Devolve o JSON cru do resultado.
    pub fn definition(&self, file: &str, line: u64, character: u64) -> Result<Value, String> {
        self.position_query("textDocument/definition", file, line, character, false)
    }

    /// `textDocument/references` na posição (0-indexed), incluindo a declaração.
    pub fn references(&self, file: &str, line: u64, character: u64) -> Result<Value, String> {
        self.position_query("textDocument/references", file, line, character, true)
    }

    fn position_query(
        &self,
        method: &str,
        file: &str,
        line: u64,
        character: u64,
        include_declaration: bool,
    ) -> Result<Value, String> {
        let uri = self.uri_for(file);
        let text = self.read_file(file)?;
        let mut guard = self.proc.lock().map_err(|_| "lock LSP envenenado")?;
        Self::ensure_started(&mut guard, &self.config)?;
        let proc = guard.as_mut().expect("proc iniciado");
        Self::ensure_open(proc, &uri, &text)?;

        let mut params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });
        if include_declaration {
            params["context"] = json!({ "includeDeclaration": true });
        }

        // Enquanto o rust-analyzer indexa, a resposta vem vazia; re-tenta. O
        // mesmo vale para ContentModified/ServerCancelled: o server invalidou o
        // request no meio da indexação e a spec LSP manda o cliente re-tentar.
        let start = Instant::now();
        loop {
            match proc.request(method, params.clone(), REQUEST_TIMEOUT) {
                Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
                Err(e) => return Err(e),
                Ok(res) => {
                    if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
                        return Ok(res);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Busca símbolos por NOME (`workspace/symbol`): o server acha as
    /// ocorrências e resolve a posição de cada uma. Retry enquanto indexa
    /// (resultado vazio), como as consultas de posição.
    pub fn symbol(&self, name: &str) -> Result<Value, String> {
        let mut guard = self.proc.lock().map_err(|_| "lock LSP envenenado")?;
        Self::ensure_started(&mut guard, &self.config)?;
        let proc = guard.as_mut().expect("proc iniciado");
        let start = Instant::now();
        loop {
            match proc.request(
                "workspace/symbol",
                json!({ "query": name }),
                REQUEST_TIMEOUT,
            ) {
                Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
                Err(e) => return Err(e),
                Ok(res) => {
                    if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
                        return Ok(res);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Diagnósticos do arquivo. São empurrados de forma assíncrona pelo server
    /// após o `didOpen`; o reader de fundo os captura continuamente, então aqui
    /// só fazemos polling do stash compartilhado até assentar ou estourar o
    /// orçamento (não precisa mais bombear round-trips para drenar o pipe).
    pub fn diagnostics(&self, file: &str) -> Result<Value, String> {
        let uri = self.uri_for(file);
        let text = self.read_file(file)?;
        let mut guard = self.proc.lock().map_err(|_| "lock LSP envenenado")?;
        Self::ensure_started(&mut guard, &self.config)?;
        let proc = guard.as_mut().expect("proc iniciado");
        Self::ensure_open(proc, &uri, &text)?;

        let start = Instant::now();
        let mut first_seen: Option<Instant> = None;
        loop {
            if let Some(dead) = proc
                .shared
                .inner
                .lock()
                .map_err(|_| "lock LSP envenenado")?
                .dead
                .clone()
            {
                return Err(format!("servidor LSP encerrou: {dead}"));
            }
            let stashed = proc
                .shared
                .diagnostics
                .lock()
                .map_err(|_| "lock LSP envenenado")?
                .get(&uri)
                .cloned();
            match &stashed {
                Some(v) if !is_empty(v) => return Ok(stashed.unwrap()),
                Some(_) => {
                    // URI já reportada (talvez vazia = arquivo limpo). Dá um
                    // tempo curto para um diagnóstico tardio, senão devolve vazio.
                    let seen = first_seen.get_or_insert_with(Instant::now);
                    if seen.elapsed() > Duration::from_secs(3) {
                        return Ok(stashed.unwrap());
                    }
                }
                None => {}
            }
            if start.elapsed() > DIAG_BUDGET {
                return Ok(stashed.unwrap_or_else(|| json!([])));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

impl LspProc {
    /// Envia um request e dorme no condvar até o reader de fundo entregar a
    /// resposta com o `id` (ou o server morrer / estourar o timeout). O reader
    /// é quem lê o stdout — aqui só escrevemos e esperamos.
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        {
            let mut w = self.stdin.lock().map_err(|_| "lock stdin LSP envenenado")?;
            write_msg(
                &mut *w,
                &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
            )?;
        }
        let deadline = Instant::now() + timeout;
        let mut inner = self
            .shared
            .inner
            .lock()
            .map_err(|_| "lock LSP envenenado")?;
        loop {
            if let Some(res) = inner.responses.remove(&id) {
                return res.map_err(|e| format!("LSP {method}: {e}"));
            }
            if let Some(dead) = &inner.dead {
                return Err(format!("servidor LSP encerrou durante '{method}': {dead}"));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(format!("timeout LSP em '{method}'"));
            }
            let (guard, _timeout) = self
                .shared
                .cv
                .wait_timeout(inner, deadline - now)
                .map_err(|_| "lock LSP envenenado")?;
            inner = guard;
        }
    }
}

/// Uma consulta LSP exposta como `dyn Tool`. As três (definição/referências/
/// diagnósticos) de um mesmo server compartilham a `Arc<LspSession>`.
pub struct LspTool {
    full_name: String,
    kind: LspQuery,
    server_id: String,
    session: Arc<LspSession>,
}

impl Tool for LspTool {
    fn name(&self) -> &str {
        &self.full_name
    }
    fn description(&self) -> &str {
        match self.kind {
            LspQuery::Definition => {
                "LSP: definição do símbolo na posição (file, line, character — 0-indexed)"
            }
            LspQuery::References => {
                "LSP: referências do símbolo na posição (file, line, character — 0-indexed)"
            }
            LspQuery::Diagnostics => "LSP: diagnósticos (erros/avisos) do arquivo",
            LspQuery::Symbol => {
                "LSP: acha símbolos por NOME no workspace (workspace/symbol) — devolve nome:caminho:linha:coluna (0-indexed), sem precisar saber a posição de antemão"
            }
        }
    }
    fn input_schema(&self) -> Value {
        match self.kind {
            LspQuery::Diagnostics => json!({
                "type": "object",
                "properties": { "file": { "type": "string" } },
                "required": ["file"]
            }),
            LspQuery::Symbol => json!({
                "type": "object",
                "properties": { "name": { "type": "string", "description": "nome (ou prefixo) do símbolo" } },
                "required": ["name"]
            }),
            _ => json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "line": { "type": "integer", "minimum": 0 },
                    "character": { "type": "integer", "minimum": 0 }
                },
                "required": ["file", "line", "character"]
            }),
        }
    }
    fn scope(&self, args: &Value) -> String {
        let file = args.get("file").and_then(Value::as_str).unwrap_or("");
        format!("lsp:{}/{} {}", self.server_id, self.kind.as_str(), file)
    }
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        // `symbol` é por NOME, não por arquivo — tratado antes de exigir `file`.
        if self.kind == LspQuery::Symbol {
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArgs("campo 'name' obrigatório".into()))?;
            let res = self.session.symbol(name).map_err(ToolError::Execution)?;
            return Ok(bound_output(render_symbols(&res), DEFAULT_OUTPUT_LIMIT));
        }

        let file = args
            .get("file")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArgs("campo 'file' obrigatório".into()))?;

        let out = match self.kind {
            LspQuery::Diagnostics => {
                let res = self
                    .session
                    .diagnostics(file)
                    .map_err(ToolError::Execution)?;
                render_diagnostics(file, &res)
            }
            LspQuery::Symbol => unreachable!("symbol tratado acima"),
            _ => {
                let line = args.get("line").and_then(Value::as_u64).ok_or_else(|| {
                    ToolError::InvalidArgs("campo 'line' obrigatório (0-indexed)".into())
                })?;
                let character = args
                    .get("character")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        ToolError::InvalidArgs("campo 'character' obrigatório (0-indexed)".into())
                    })?;
                let res = if self.kind == LspQuery::Definition {
                    self.session.definition(file, line, character)
                } else {
                    self.session.references(file, line, character)
                }
                .map_err(ToolError::Execution)?;
                render_locations(&res)
            }
        };
        Ok(bound_output(out, DEFAULT_OUTPUT_LIMIT))
    }
}

/// Registra as quatro tools
/// (`lsp__<id>__{definition,references,diagnostics,symbol}`) de um server no
/// registry, compartilhando uma sessão preguiçosa. **Não sobe** o server — isso
/// é feito no primeiro uso (o server é caro). Guarda de colisão como os loaders
/// de skill/MCP. Devolve quantas foram registradas.
pub fn register_lsp_server(registry: &mut ToolRegistry, config: &LspServerConfig) -> usize {
    let session = Arc::new(LspSession::new(config.clone()));
    let mut n = 0;
    for kind in [
        LspQuery::Definition,
        LspQuery::References,
        LspQuery::Diagnostics,
        LspQuery::Symbol,
    ] {
        let full_name = format!("lsp__{}__{}", config.id, kind.as_str());
        if registry.get(&full_name).is_some() {
            eprintln!("  lsp tool '{full_name}' colide com um tool já registrado — pulada");
            continue;
        }
        registry.register(Box::new(LspTool {
            full_name,
            kind,
            server_id: config.id.clone(),
            session: session.clone(),
        }));
        n += 1;
    }
    n
}

// --- helpers de protocolo (framing Content-Length, sem dependências) ---

fn write_msg(w: &mut impl Write, v: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    w.write_all(&body).map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())
}

fn read_msg(r: &mut impl BufRead) -> Result<Value, String> {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("EOF do servidor LSP".into());
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(rest) = t.strip_prefix("Content-Length:") {
            len = rest
                .trim()
                .parse()
                .map_err(|_| "Content-Length inválido".to_string())?;
        }
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(|e| e.to_string())?;
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

/// Responde a um request do servidor (tem `id` e `method`) para não travar o
/// handshake. `workspace/configuration` espera um array (um item por pedido);
/// o resto aceita `null`.
fn respond_server_request(w: &mut impl Write, m: &Value) -> Result<(), String> {
    let (Some(id), Some(method)) = (m.get("id"), m.get("method").and_then(Value::as_str)) else {
        return Ok(());
    };
    let result = if method == "workspace/configuration" {
        let n = m
            .get("params")
            .and_then(|p| p.get("items"))
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(1);
        Value::Array(vec![Value::Null; n])
    } else {
        Value::Null
    };
    write_msg(w, &json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn is_empty(v: &Value) -> bool {
    v.is_null() || v.as_array().map(|a| a.is_empty()).unwrap_or(false)
}

/// Erros que a spec LSP define como re-tentáveis: `ContentModified` (-32801,
/// o server invalidou o request durante análise/indexação) e `ServerCancelled`
/// (-32802). O rust-analyzer devolve o primeiro enquanto indexa.
fn is_retryable_lsp_error(e: &str) -> bool {
    e.contains("-32801") || e.contains("-32802")
}

fn language_id(uri: &str) -> &'static str {
    if uri.ends_with(".rs") {
        "rust"
    } else if uri.ends_with(".py") {
        "python"
    } else {
        "plaintext"
    }
}

/// Converte `file:///a/b.rs` no caminho `/a/b.rs` para exibição.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

/// Renderiza `Location | Location[] | LocationLink[]` como linhas
/// `caminho:line:character` (0-indexed, convenção LSP). Robusto ao shape.
fn render_locations(v: &Value) -> String {
    let items: Vec<Value> = match v {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        other => vec![other.clone()],
    };
    let mut lines = Vec::new();
    for it in items {
        // Location: {uri, range}; LocationLink: {targetUri, targetSelectionRange|targetRange}
        let uri = it
            .get("uri")
            .or_else(|| it.get("targetUri"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let range = it
            .get("range")
            .or_else(|| it.get("targetSelectionRange"))
            .or_else(|| it.get("targetRange"));
        let (line, ch) = range
            .and_then(|r| r.get("start"))
            .map(|s| {
                (
                    s.get("line").and_then(Value::as_u64).unwrap_or(0),
                    s.get("character").and_then(Value::as_u64).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        lines.push(format!("{}:{}:{}", uri_to_path(uri), line, ch));
    }
    if lines.is_empty() {
        "(nenhum resultado)".to_string()
    } else {
        lines.join("\n")
    }
}

/// Renderiza o resultado de `workspace/symbol` (`SymbolInformation[]` ou
/// `WorkspaceSymbol[]`) como linhas `nome:caminho:linha:coluna` (0-indexed).
fn render_symbols(v: &Value) -> String {
    let items: Vec<Value> = match v {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        other => vec![other.clone()],
    };
    let mut lines = Vec::new();
    for it in items {
        let name = it.get("name").and_then(Value::as_str).unwrap_or("?");
        let loc = it.get("location");
        let uri = loc
            .and_then(|l| l.get("uri"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let (line, ch) = loc
            .and_then(|l| l.get("range"))
            .and_then(|r| r.get("start"))
            .map(|s| {
                (
                    s.get("line").and_then(Value::as_u64).unwrap_or(0),
                    s.get("character").and_then(Value::as_u64).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        lines.push(format!("{}:{}:{}:{}", name, uri_to_path(uri), line, ch));
    }
    if lines.is_empty() {
        "(nenhum símbolo)".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_diagnostics(file: &str, v: &Value) -> String {
    let Some(arr) = v.as_array() else {
        return "(sem diagnósticos)".to_string();
    };
    if arr.is_empty() {
        return format!("{file}: sem diagnósticos");
    }
    let mut lines = Vec::new();
    for d in arr {
        let (line, ch) = d
            .get("range")
            .and_then(|r| r.get("start"))
            .map(|s| {
                (
                    s.get("line").and_then(Value::as_u64).unwrap_or(0),
                    s.get("character").and_then(Value::as_u64).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        let sev = match d.get("severity").and_then(Value::as_u64) {
            Some(1) => "error",
            Some(2) => "warning",
            Some(3) => "info",
            Some(4) => "hint",
            _ => "diag",
        };
        let msg = d.get("message").and_then(Value::as_str).unwrap_or("");
        lines.push(format!("{file}:{line}:{ch}: {sev}: {msg}"));
    }
    lines.join("\n")
}

// Os testes vivem em `crates/btv-tools/tests/lsp_integration.rs`: um hermético
// (server fixture, sempre roda) e um contra o rust-analyzer REAL (ignored; roda
// no CI com a componente instalada).
