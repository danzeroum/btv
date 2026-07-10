//! Cliente MCP (Fase 6 Onda 4): conecta a servidores MCP externos (via `rmcp`,
//! transporte child-process/stdio), lista suas tools e as expõe como `dyn Tool`
//! no `ToolRegistry` — **sob o mesmo motor de permissões** (tool MCP = tool como
//! qualquer outra: pede permissão, entra no ledger). Nomes são namespaced
//! (`mcp__<server>__<tool>`) para não colidir com built-ins/skills.
//!
//! **Sessão persistente (Fase 6 pendência fechada):** a chamada do rmcp é async
//! e o `Tool::run` é sync. Em vez de conectar-chamar-encerrar a cada invocação,
//! cada servidor tem UMA `McpSession`: uma thread dedicada com runtime próprio
//! conecta uma vez (preguiçosamente, no primeiro uso) e reusa a conexão por
//! todas as chamadas seguintes, recebendo comandos por um canal. O processo é
//! encerrado quando o último `McpTool` que segura a `Arc<McpSession>` é dropado.
//!
//! **Sem thread pendurada (o "thread leak" do probe):** toda operação (conectar,
//! listar, chamar) é envolta em `tokio::time::timeout`. Um servidor que conecta
//! mas nunca responde vira um erro em vez de bloquear a thread para sempre — o
//! que fechava o vazamento em que `list_tools_blocking` podia travar
//! indefinidamente no probe do console MCP.

use crate::{bound_output, Tool, ToolError, ToolOutput, ToolRegistry, DEFAULT_OUTPUT_LIMIT};
use serde::Serialize;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Prazo para conectar (spawn + handshake) ao servidor MCP.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Prazo de UMA operação (listar/chamar) num servidor já conectado.
const OP_TIMEOUT: Duration = Duration::from_secs(30);

/// Cliente MCP conectado (alias do tipo verboso do rmcp).
type McpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

/// Um servidor MCP declarado pelo usuário: o comando que o sobe via stdio.
#[derive(Debug, Clone, Serialize)]
pub struct McpServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Lê `<root>/.btv/mcp.toml` e devolve os servidores declarados, SEM conectar a
/// nenhum (só parsing). C4-3: helper multi-consumidor extraído do `skills.rs`
/// da btv-cli para o dono do tipo — compartilhado entre o registry-builder do
/// agente (`skills::load_mcp_servers`, que registra as tools para uso real) e o
/// console MCP (`mcp_console`, que só enumera/probe para exibição). Ausente ou
/// inválido → vazio (fail-soft).
pub fn read_server_configs(root: &std::path::Path) -> Vec<McpServerConfig> {
    let config_path = root.join(".btv").join("mcp.toml");
    let Ok(raw) = std::fs::read_to_string(&config_path) else {
        return Vec::new();
    };
    #[derive(serde::Deserialize)]
    struct McpConfigFile {
        #[serde(default)]
        server: Vec<ServerEntry>,
    }
    #[derive(serde::Deserialize)]
    struct ServerEntry {
        id: String,
        command: String,
        #[serde(default)]
        args: Vec<String>,
    }
    let cfg: McpConfigFile = match toml::from_str(&raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  mcp: .btv/mcp.toml inválido ({e}) — ignorado");
            return Vec::new();
        }
    };
    cfg.server
        .into_iter()
        .map(|s| McpServerConfig {
            id: s.id,
            command: s.command,
            args: s.args,
        })
        .collect()
}

/// Metadados de uma tool anunciada por um servidor MCP.
#[derive(Serialize)]
pub struct McpToolMeta {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Comandos que a thread da sessão processa sobre a conexão persistente.
enum McpCommand {
    ListTools {
        reply: mpsc::Sender<Result<Vec<McpToolMeta>, String>>,
    },
    Call {
        tool: String,
        args: Value,
        reply: mpsc::Sender<Result<String, String>>,
    },
}

/// Uma conexão MCP persistente por servidor: uma thread dedicada conecta uma
/// vez e serve os comandos. Compartilhada (`Arc`) por todas as `McpTool` de um
/// mesmo servidor; a thread encerra (e mata o processo MCP) quando a última
/// `Arc` cai — o `tx` fecha, o `recv` do loop retorna `Err`.
pub struct McpSession {
    tx: mpsc::Sender<McpCommand>,
    _thread: JoinHandle<()>,
}

impl McpSession {
    /// Sobe a thread da sessão (a conexão em si é preguiçosa: só no 1º comando).
    pub fn connect(config: McpServerConfig) -> Arc<Self> {
        let (tx, rx) = mpsc::channel::<McpCommand>();
        let thread = std::thread::spawn(move || session_loop(config, rx));
        Arc::new(Self {
            tx,
            _thread: thread,
        })
    }

    /// Lista as tools do servidor (bloqueia até a resposta da thread da sessão).
    pub fn list_tools(&self) -> Result<Vec<McpToolMeta>, String> {
        let (reply, wait) = mpsc::channel();
        self.tx
            .send(McpCommand::ListTools { reply })
            .map_err(|_| "sessão MCP encerrada".to_string())?;
        wait.recv()
            .map_err(|_| "sessão MCP sem resposta".to_string())?
    }

    /// Chama uma tool e devolve o texto do resultado.
    fn call(&self, tool: &str, args: Value) -> Result<String, String> {
        let (reply, wait) = mpsc::channel();
        self.tx
            .send(McpCommand::Call {
                tool: tool.to_string(),
                args,
                reply,
            })
            .map_err(|_| "sessão MCP encerrada".to_string())?;
        wait.recv()
            .map_err(|_| "sessão MCP sem resposta".to_string())?
    }
}

/// Loop da thread da sessão: um runtime `current_thread` próprio (não dá para
/// aninhar `block_on` no worker do loop), conexão preguiçosa reusada entre
/// comandos, cada operação com timeout. Uma operação que estoura o prazo
/// derruba a conexão (`client = None`) para o próximo comando reconectar, em
/// vez de herdar um socket travado.
fn session_loop(config: McpServerConfig, rx: mpsc::Receiver<McpCommand>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            // Sem runtime, responde erro a cada comando até o canal fechar.
            for cmd in rx {
                reply_error(cmd, format!("runtime MCP indisponível: {e}"));
            }
            return;
        }
    };
    let mut client: Option<McpClient> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            McpCommand::ListTools { reply } => {
                let res = ensure_client(&rt, &mut client, &config).and_then(|c| {
                    rt.block_on(async {
                        match tokio::time::timeout(OP_TIMEOUT, c.list_all_tools()).await {
                            Ok(Ok(tools)) => Ok(tools
                                .into_iter()
                                .map(|t| McpToolMeta {
                                    name: t.name.to_string(),
                                    description: t
                                        .description
                                        .map(|d| d.to_string())
                                        .unwrap_or_default(),
                                    input_schema: Value::Object((*t.input_schema).clone()),
                                })
                                .collect()),
                            Ok(Err(e)) => Err(e.to_string()),
                            Err(_) => Err("__timeout__".to_string()),
                        }
                    })
                });
                if matches!(&res, Err(e) if e == "__timeout__") {
                    client = None;
                }
                let res = res.map_err(|e| timeout_msg(e, "listar tools"));
                let _ = reply.send(res);
            }
            McpCommand::Call { tool, args, reply } => {
                let res = ensure_client(&rt, &mut client, &config).and_then(|c| {
                    let mut params = rmcp::model::CallToolRequestParams::new(tool);
                    if let Value::Object(m) = args {
                        params = params.with_arguments(m);
                    }
                    rt.block_on(async {
                        match tokio::time::timeout(OP_TIMEOUT, c.call_tool(params)).await {
                            Ok(Ok(result)) => Ok(render_content(&result)),
                            Ok(Err(e)) => Err(e.to_string()),
                            Err(_) => Err("__timeout__".to_string()),
                        }
                    })
                });
                if matches!(&res, Err(e) if e == "__timeout__") {
                    client = None;
                }
                let res = res.map_err(|e| timeout_msg(e, "chamada MCP"));
                let _ = reply.send(res);
            }
        }
    }
    // Canal fechado (última `Arc<McpSession>` caiu) → encerra a conexão.
    if let Some(c) = client {
        let _ = rt.block_on(c.cancel());
    }
}

fn timeout_msg(e: String, what: &str) -> String {
    if e == "__timeout__" {
        format!("timeout ({}s) em {what} MCP", OP_TIMEOUT.as_secs())
    } else {
        e
    }
}

/// Conecta (bounded) se ainda não houver conexão; devolve uma referência ao
/// cliente vivo. Uma falha de conexão deixa `client` em `None`.
fn ensure_client<'a>(
    rt: &tokio::runtime::Runtime,
    client: &'a mut Option<McpClient>,
    config: &McpServerConfig,
) -> Result<&'a McpClient, String> {
    if client.is_none() {
        let c = rt.block_on(async {
            match tokio::time::timeout(CONNECT_TIMEOUT, connect(config)).await {
                Ok(Ok(c)) => Ok(c),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(format!(
                    "timeout ({}s) conectando ao servidor MCP",
                    CONNECT_TIMEOUT.as_secs()
                )),
            }
        })?;
        *client = Some(c);
    }
    Ok(client.as_ref().expect("cliente recém-conectado"))
}

fn reply_error(cmd: McpCommand, msg: String) {
    match cmd {
        McpCommand::ListTools { reply } => {
            let _ = reply.send(Err(msg));
        }
        McpCommand::Call { reply, .. } => {
            let _ = reply.send(Err(msg));
        }
    }
}

/// Uma tool MCP exposta como `dyn Tool`. Compartilha a `Arc<McpSession>` do seu
/// servidor com as demais tools do mesmo servidor; `run` manda um comando para a
/// thread da sessão e espera a resposta.
pub struct McpTool {
    full_name: String,
    description: String,
    input_schema: Value,
    session: Arc<McpSession>,
    server_id: String,
    tool: String,
}

impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.full_name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }
    fn scope(&self, args: &Value) -> String {
        // Informativo para o permission-engine; permite regras por servidor/tool.
        let preview: String = args.to_string().chars().take(60).collect();
        format!("mcp:{}/{} {}", self.server_id, self.tool, preview)
    }
    fn run(&self, args: &Value) -> Result<ToolOutput, ToolError> {
        let out = self
            .session
            .call(&self.tool, args.clone())
            .map_err(ToolError::Execution)?;
        Ok(bound_output(out, DEFAULT_OUTPUT_LIMIT))
    }
}

/// Enumera as tools de um servidor MCP (sync, via uma sessão efêmera que sobe
/// só para listar e é encerrada em seguida). Usado pelo console MCP para exibir
/// status — a operação é bounded (timeout interno), então a thread SEMPRE
/// termina, mesmo contra um servidor que trava (fim do "thread leak" do probe).
pub fn list_tools_blocking(config: &McpServerConfig) -> Result<Vec<McpToolMeta>, String> {
    let session = McpSession::connect(config.clone());
    session.list_tools()
    // `session` (única `Arc`) cai aqui → a thread encerra e mata o processo.
}

/// Conecta a um servidor MCP via transporte child-process (stdio).
async fn connect(config: &McpServerConfig) -> Result<McpClient, String> {
    use rmcp::transport::TokioChildProcess;
    use rmcp::ServiceExt;
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);
    let transport = TokioChildProcess::new(cmd).map_err(|e| format!("spawn MCP: {e}"))?;
    ().serve(transport)
        .await
        .map_err(|e| format!("handshake MCP: {e}"))
}

/// Extrai o texto dos blocos de conteúdo do resultado (robusto ao shape exato:
/// serializa cada bloco e puxa o campo `text`, ignorando não-texto).
fn render_content(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| {
            serde_json::to_value(c)
                .ok()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(String::from))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Conecta ao servidor (sessão persistente), lista as tools e registra cada uma
/// como `McpTool` namespaced no registry, todas compartilhando a MESMA
/// `Arc<McpSession>` (uma conexão por servidor, reusada). Guarda de colisão como
/// o loader de skills. Devolve quantas foram registradas — se `0`, a `Arc` cai
/// aqui e a conexão é encerrada (nenhuma tool a segurando).
pub fn register_mcp_server(
    registry: &mut ToolRegistry,
    config: &McpServerConfig,
) -> Result<usize, String> {
    let session = McpSession::connect(config.clone());
    let metas = session.list_tools()?;
    let mut n = 0;
    for m in metas {
        let full_name = format!("mcp__{}__{}", config.id, m.name);
        if registry.get(&full_name).is_some() {
            eprintln!("  mcp tool '{full_name}' colide com um tool já registrado — pulada");
            continue;
        }
        registry.register(Box::new(McpTool {
            full_name,
            description: m.description,
            input_schema: m.input_schema,
            session: Arc::clone(&session),
            server_id: config.id.clone(),
            tool: m.name,
        }));
        n += 1;
    }
    Ok(n)
}

// Os testes de integração cross-process (precisam de
// `CARGO_BIN_EXE_btv_mcp_fixture`, exposto pelo cargo só a integration tests)
// vivem em `crates/btv-tools/tests/mcp_integration.rs`.
