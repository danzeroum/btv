//! Comando `btv squad`: delega a tarefa ao squad multi-agente Python
//! (Onda 4d). Fecha o laço bidirecional com o `Gateway` real como
//! `CoreBackend` — as API keys ficam só aqui (ADR 0001), o Python só
//! conhece o UDS. Fallback progressivo de 3 níveis: squad → agente-único
//! → safe-mode read-only.

use crate::session::{now_rfc3339, Session};
use crate::{run_once, RunOpts};
use anyhow::Result;
use btv_core::{Decision, PermissionEngine};
use btv_llm::chat::{ChatMessage, ContentBlock, GenerateRequest, Role};
use btv_llm::Generator;
use btv_proto::core::{PermissionRequest, ToolCall, ToolResult};
use btv_proto::llm::{LlmRequest, Usage};
use btv_proto::squad::{squad_event, SquadTask};
use btv_sidecar::{serve_core, CoreBackend, SquadRun, SquadSupervisor};
use btv_tools::ToolRegistry;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// `ToolResult.exit_code`: convenção compartilhada pelos três `CoreBackend`
/// de produção (ver `core_run_tool`). `0` = sucesso; `1` = erro de
/// execução/args inválidos/ferramenta desconhecida (nunca rodou, ou rodou e
/// falhou — vale tentar de novo com outra entrada); `-1` = negado pelo
/// motor de permissões ou por um humano (nunca chegou a executar — não
/// adianta repetir a mesma ação).
pub(crate) const TOOL_EXIT_OK: i32 = 0;
pub(crate) const TOOL_EXIT_ERROR: i32 = 1;
pub(crate) const TOOL_EXIT_DENIED: i32 = -1;

/// `CoreBackend::run_tool` de verdade, compartilhado pelos três backends de
/// produção (CLI, web, scripted). Recalcula o escopo a partir de
/// `args_json` via `Tool::scope` — o `ToolCall.scope` vindo da rede NUNCA é
/// usado na decisão de permissão (só o Rust decide escopo; um Python
/// bugado/comprometido não pode declarar um escopo mais permissivo que o
/// real). Avalia via `PermissionEngine`; no caso `Ask`, delega a decisão a
/// `ask` (o mesmo bridge de HITL que o backend já usa para
/// `request_permission` — stdin no CLI, `SquadHub::request_hitl` na web).
/// A execução síncrona (`tool.run`) roda dentro de `spawn_blocking`; a
/// checagem de permissão (incluindo o `Ask` assíncrono) fica fora — não
/// bloqueia uma worker-thread do reactor esperando um clique humano.
/// Registra cada chamada no ledger de `root` (best-effort — falha de
/// ledger nunca derruba a execução da ferramenta).
pub(crate) async fn core_run_tool<F, Fut>(
    tools: &Arc<ToolRegistry>,
    permissions: &PermissionEngine,
    call: &ToolCall,
    root: &Path,
    ask: F,
) -> ToolResult
where
    F: FnOnce(PermissionRequest) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let args: serde_json::Value = match serde_json::from_str(&call.args_json) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("args_json inválido: {e}"),
                truncated: false,
                exit_code: TOOL_EXIT_ERROR,
            }
        }
    };
    if tools.get(&call.tool).is_none() {
        return ToolResult {
            content: format!("ferramenta desconhecida: {}", call.tool),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        };
    }
    let scope = tools.get(&call.tool).expect("validado acima").scope(&args);

    let allowed = match permissions.evaluate(&call.tool, &scope) {
        Decision::Allow => true,
        Decision::Deny => false,
        Decision::Ask => {
            ask(PermissionRequest {
                tool: call.tool.clone(),
                scope: scope.clone(),
                reason: format!("squad pede '{}' em {scope:?}", call.tool),
                confidence: 0.0,
            })
            .await
        }
    };
    if !allowed {
        let result = ToolResult {
            content: format!("permissão negada para {} em {scope:?}", call.tool),
            truncated: false,
            exit_code: TOOL_EXIT_DENIED,
        };
        log_tool_run(root, call, &scope, &result);
        return result;
    }

    let tools_for_blocking = Arc::clone(tools);
    let tool_name = call.tool.clone();
    let run_result = tokio::task::spawn_blocking(move || {
        let tool = tools_for_blocking
            .get(&tool_name)
            .expect("validado antes do spawn_blocking");
        tool.run(&args)
    })
    .await;

    let result = match run_result {
        Ok(Ok(out)) => {
            let mut content = out.content;
            if out.truncated {
                match &out.overflow_path {
                    Some(path) => content.push_str(&format!(
                        "\n[output truncado; completo em {path} — use read para consultar]"
                    )),
                    None => content.push_str("\n[output truncado]"),
                }
            }
            ToolResult {
                content,
                truncated: out.truncated,
                exit_code: TOOL_EXIT_OK,
            }
        }
        Ok(Err(e)) => ToolResult {
            content: e.to_string(),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        },
        Err(e) => ToolResult {
            content: format!("falha interna ao rodar ferramenta: {e}"),
            truncated: false,
            exit_code: TOOL_EXIT_ERROR,
        },
    };
    log_tool_run(root, call, &scope, &result);
    result
}

/// Scope de um `ToolCall` (mesma resolução de dentro de `core_run_tool`) —
/// exposto para correlacionar execuções à tarefa (trilha de entregas do BTV).
/// Converte a evidência de verificação canônica (`btv_schemas::verification`)
/// na mensagem proto TIPADA (D3t). Espelho 1:1 do schema
/// `verification-evidence.v1` — o mesmo conteúdo que antes viajava como string
/// JSON no campo `verification_evidence_json`.
pub(crate) fn evidence_to_proto(
    e: &btv_schemas::verification::VerificationEvidence,
) -> btv_proto::squad::VerificationEvidence {
    use btv_proto::squad as pb;
    use btv_schemas::verification::Verdict as V;
    pb::VerificationEvidence {
        run_id: e.run_id.clone(),
        git_sha: e.git_sha.clone(),
        steps: e
            .steps
            .iter()
            .map(|s| pb::VerificationStep {
                name: s.name.clone(),
                tool: s.tool.clone(),
                exit_code: s.exit_code,
                duration_ms: s.duration_ms,
                findings: s
                    .findings
                    .iter()
                    .map(|f| pb::VerificationFinding {
                        tool: f.tool.clone(),
                        severity: f.severity.clone(),
                        message: f.message.clone(),
                        file: f.file.clone(),
                        line: f.line,
                    })
                    .collect(),
            })
            .collect(),
        verdict: match e.verdict {
            V::Pass => pb::Verdict::Pass,
            V::Fail => pb::Verdict::Fail,
            V::Skipped => pb::Verdict::Skipped,
        } as i32,
        produced_at: e.produced_at.clone(),
    }
}

pub(crate) fn tool_scope(tools: &ToolRegistry, call: &ToolCall) -> String {
    serde_json::from_str::<serde_json::Value>(&call.args_json)
        .ok()
        .and_then(|args| tools.get(&call.tool).map(|t| t.scope(&args)))
        .unwrap_or_default()
}

/// Best-effort — nunca deixa uma falha de ledger derrubar a resposta do
/// `RunTool` (mesma postura de `Session::note`).
fn log_tool_run(root: &Path, call: &ToolCall, scope: &str, result: &ToolResult) {
    if let Err(e) = crate::session::append_entry(
        root,
        "btv-cli:squad-tool",
        "squad.tool_run",
        json!({
            "tool": call.tool,
            "scope": scope,
            "exit_code": result.exit_code,
            "truncated": result.truncated,
        }),
    ) {
        eprintln!("  [ledger] falha ao registrar squad.tool_run: {e}");
    }
}

/// `CoreBackend` real: `Generate` passa pelo `Gateway` (streaming agregado),
/// `RequestPermission` resolve HITL no terminal (ou auto-aprova com `--yes`),
/// `RunTool` executa de verdade sob `ToolRegistry`/`PermissionEngine`
/// ("tool execution architecture" — squad como executor).
struct GatewayCoreBackend<G: Generator> {
    generator: Arc<G>,
    auto_yes: bool,
    root: PathBuf,
    tools: Arc<ToolRegistry>,
    tool_permissions: PermissionEngine,
}

#[derive(serde::Deserialize)]
pub(crate) struct WireMsg {
    role: String,
    content: String,
}

/// `CoreBackend::generate` de verdade: desempacota `messages_json`, chama o
/// `Generator` real (mesmo Gateway/rate-limit/cache do resto da CLI) e
/// agrega a resposta. Compartilhado entre o backend do `btv squad` (CLI,
/// HITL via stdin) e o do agente web (Onda 4, HITL via HTTP) — a única
/// diferença entre os dois é `request_permission`.
pub(crate) async fn core_generate<G: Generator>(
    generator: &G,
    req: &LlmRequest,
) -> Result<(String, Usage), String> {
    let msgs: Vec<WireMsg> = serde_json::from_str(&req.messages_json)
        .map_err(|e| format!("messages_json inválido: {e}"))?;
    let mut system = String::new();
    let mut chat = Vec::new();
    for m in msgs {
        match m.role.as_str() {
            "system" => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&m.content);
            }
            // Loop ReAct do squad (Onda 2) manda histórico multi-turno de
            // verdade — sem isto, um "assistant" cairia em `Role::User` e a
            // API da Anthropic (que exige alternância estrita user/
            // assistant) recusaria/malformaria a conversa. Todo caller
            // anterior mandava só 1 system + 1 user, então este ramo nunca
            // foi exercitado antes do loop ReAct existir.
            "assistant" => chat.push(ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::Text { text: m.content }],
            }),
            _ => chat.push(ChatMessage::user_text(&m.content)),
        }
    }
    let gen_req = GenerateRequest {
        model: req.model.clone(),
        system,
        messages: chat,
        tools: vec![],
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature,
    };
    let mut sink = |_: &str| {};
    let turn = generator
        .generate(gen_req, &mut sink)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        turn.text(),
        Usage {
            input_tokens: turn.usage.input_tokens,
            output_tokens: turn.usage.output_tokens,
            cache_hit: turn.provider.contains("+cache"),
            provider: turn.provider,
        },
    ))
}

#[tonic::async_trait]
impl<G: Generator + Send + Sync + 'static> CoreBackend for GatewayCoreBackend<G> {
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String> {
        core_generate(self.generator.as_ref(), req).await
    }

    async fn request_permission(&self, req: &PermissionRequest) -> bool {
        if self.auto_yes {
            return true;
        }
        let prompt = format!(
            "\n  [HITL] o squad pede aprovação para '{}' (confiança {:.2}) — {}? [s/N] ",
            req.tool, req.confidence, req.reason
        );
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            eprint!("{prompt}");
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            if std::io::stdin().read_line(&mut answer).is_err() {
                return false;
            }
            matches!(
                answer.trim().to_lowercase().as_str(),
                "s" | "sim" | "y" | "yes"
            )
        })
        .await
        .unwrap_or(false)
    }

    async fn run_tool(&self, call: &ToolCall) -> ToolResult {
        core_run_tool(
            &self.tools,
            &self.tool_permissions,
            call,
            &self.root,
            |req| async move { self.request_permission(&req).await },
        )
        .await
    }
}

/// Localiza o workspace Python do sidecar: `BTV_PYTHON_DIR`, senão um
/// `python/pyproject.toml` subindo a partir do binário ou do cwd.
/// `pub(crate)` — reusado por `squad_agent.rs` (Onda 4) e `prompt_render.rs`
/// (Onda 5) para achar o mesmo workspace Python dos dois sidecares.
pub(crate) fn locate_python_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BTV_PYTHON_DIR") {
        let p = PathBuf::from(dir);
        if p.join("pyproject.toml").exists() {
            return Some(p);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            candidates.push(ancestor.join("python"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("python"));
    }
    candidates
        .into_iter()
        .find(|p| p.join("pyproject.toml").exists())
}

/// Ponto de entrada do `btv squad`. Constrói o `CoreService` real e
/// tenta o squad; degrada em 3 níveis se necessário.
pub async fn run_squad<G: Generator + Send + Sync + 'static>(
    generator: G,
    opts: &RunOpts,
    root: &Path,
    task: String,
) -> Result<()> {
    let generator = Arc::new(generator);
    let btv_dir = root.join(".btv");
    std::fs::create_dir_all(&btv_dir)?;
    let pid = std::process::id();
    let core_sock = btv_dir.join(format!("squad-core-{pid}.sock"));
    let squad_sock = btv_dir.join(format!("squad-{pid}.sock"));

    let backend = GatewayCoreBackend {
        generator: generator.clone(),
        auto_yes: opts.yes,
        root: root.to_path_buf(),
        tools: Arc::new(ToolRegistry::default_set(root)),
        tool_permissions: (btv_core::BUILD.permissions)(),
    };
    let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
    for _ in 0..100 {
        if core_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Nível 1: squad multi-agente.
    let squad_result = try_squad(&core_sock, &squad_sock, opts, root, &task).await;
    core_task.abort();

    match squad_result {
        Ok(()) => Ok(()),
        Err(reason) => {
            eprintln!("\n  ⚠ squad indisponível ({reason}) — fallback nível 2: agente-único");
            // Nível 2: agente-único (Rust puro, sem Python).
            match run_once(generator.as_ref(), opts, root, task.clone()).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Nível 3: safe-mode read-only.
                    eprintln!(
                        "\n  ⚠ agente-único falhou ({e}) — fallback nível 3: safe-mode read-only"
                    );
                    safe_mode(&task);
                    Ok(())
                }
            }
        }
    }
}

/// Sobe o squad e drena o stream, registrando o consenso no ledger.
/// Devolve `Err(motivo)` se qualquer etapa falhar (dispara o fallback).
async fn try_squad(
    core_sock: &Path,
    squad_sock: &Path,
    opts: &RunOpts,
    root: &Path,
    task: &str,
) -> std::result::Result<(), String> {
    let py_dir = locate_python_dir()
        .ok_or_else(|| "workspace Python não encontrado (defina BTV_PYTHON_DIR)".to_string())?;
    let mut supervisor =
        SquadSupervisor::spawn(&py_dir, squad_sock.to_path_buf(), core_sock, &opts.model)
            .map_err(|e| e.to_string())?;
    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .map_err(|e| e.to_string())?;

    let mut session = Session::open(root, task, &opts.model).map_err(|e| e.to_string())?;

    // Fase 5 Onda 3: roda o /verify sobre o workspace atual ANTES do squad e
    // anexa a evidência ao SquadTask — é isso que tira o auditor do vácuo
    // (julgar código no ar) e o coloca julgando sobre fatos determinísticos.
    // Evidência ausente/inválida do outro lado (server.py) é fail-closed, não
    // "sem evidência = ok" — por isso preferimos propagar um erro aqui a
    // enviar uma string vazia silenciosamente se o pipeline falhar ao rodar.
    eprintln!("  ⏱ rodando /verify sobre o workspace antes do squad (evidência para o auditor)…");
    let evidence = crate::run_verify_pipeline(root, None)
        .map_err(|e| format!("falha ao rodar /verify antes do squad: {e}"))?;
    eprintln!(
        "  ✓ /verify concluído — veredito {:?} ({} passo(s))",
        evidence.verdict,
        evidence.steps.len()
    );
    // D3t: evidência TIPADA no wire (antes `serde_json::to_string`).
    let verification_evidence = Some(evidence_to_proto(&evidence));

    // `max_autonomy_level` hardcoded (não uma flag de CLI, nem lido do
    // request web): confirmado nesta onda que o campo é ignorado
    // ponta-a-ponta hoje — `btv_squad/server.py::ExecuteTask` nunca lê
    // `request.max_autonomy_level`; a autonomia real vem de
    // `ProgressiveAutonomyManager`/`agent_trust_scores` (`hitl.py`),
    // desconectado deste campo do proto. Wire-lo até a UI seria só "o campo
    // viajou" sem efeito nenhum — descope explícito (ADR 0021), não
    // esquecimento. Ver `pendencias.md` (Onda 13).
    let stream = client
        .execute_task(SquadTask {
            task_id: format!("s{pid:x}", pid = std::process::id()),
            description: task.to_string(),
            decision_type: "architecture".into(),
            max_autonomy_level: 3,
            verification_evidence,
            // `--model` da CLI também vale por-tarefa (além de ir no `--model`
            // do sidecar): o Python o sobrepõe ao default do orquestrador.
            model: opts.model.clone(),
            // A CLI `btv squad` não tem personas de template — elenco fixo.
            roster: Vec::new(),
            // D2t: a CLI é o modo local por definição; o Python só PROPAGA
            // (ecoado em todo evento — o tenant real chega com a E1s).
            tenant_id: btv_domain::TenantId::LOCAL.to_string(),
            actor: "cli:squad".into(),
        })
        .await
        .map_err(|e| e.to_string())?;

    eprintln!("btv squad — {task:?}\n");
    // Drena manualmente para renderizar ao vivo e registrar o consenso.
    let outcome = render_and_record(stream, &mut session).await;
    match outcome {
        SquadRun::Completed(_) => {
            let _ = session.finish(true, 1);
            Ok(())
        }
        SquadRun::Failed { reason, .. } => {
            let _ = session.finish(false, 0);
            Err(reason)
        }
    }
}

async fn render_and_record(
    stream: tonic::Streaming<btv_proto::squad::SquadEvent>,
    session: &mut Session,
) -> SquadRun {
    // Reutiliza drain_stream mas com efeito colateral de render+ledger: como
    // drain_stream consome o stream, aqui replicamos o laço para poder
    // imprimir e registrar cada evento conforme chega.
    let mut inner = stream;
    let mut events = Vec::new();
    loop {
        match inner.message().await {
            Ok(Some(ev)) => {
                render_event(&ev, session);
                if let Some(squad_event::Payload::Error(reason)) = &ev.payload {
                    let reason = reason.clone();
                    events.push(ev);
                    return SquadRun::Failed { events, reason };
                }
                events.push(ev);
            }
            Ok(None) => return SquadRun::Completed(events),
            Err(status) => {
                return SquadRun::Failed {
                    events,
                    reason: status.to_string(),
                }
            }
        }
    }
}

fn render_event(ev: &btv_proto::squad::SquadEvent, session: &mut Session) {
    match &ev.payload {
        Some(squad_event::Payload::Proposal(p)) => {
            eprintln!("  · proposta {} (conf {:.2})", p.agent, p.confidence);
        }
        Some(squad_event::Payload::Consensus(c)) => {
            eprintln!(
                "  ⚖ consenso: {} (força {:.2}){}",
                c.decision_maker,
                c.strength,
                if c.requires_human {
                    " — pede HITL"
                } else {
                    ""
                }
            );
            // Critério da Fase 4: consenso registrado no ledger.
            session.note(
                "squad.consensus",
                json!({
                    "decision_maker": c.decision_maker,
                    "strength": c.strength,
                    "requires_human": c.requires_human,
                    "ts": now_rfc3339(),
                }),
            );
        }
        Some(squad_event::Payload::Handoff(h)) => {
            eprintln!(
                "  → handoff {}→{} (fase {})",
                h.from_agent, h.to_agent, h.phase
            );
        }
        Some(squad_event::Payload::Hitl(h)) => {
            eprintln!(
                "  ⏸ escalonamento HITL: {} (conf {:.2})",
                h.reason, h.confidence
            );
        }
        Some(squad_event::Payload::Step(s)) => {
            eprintln!(
                "  {} step {}: {}",
                if s.success { "✓" } else { "✗" },
                s.step_id,
                s.summary
            );
        }
        Some(squad_event::Payload::Error(e)) => eprintln!("  ✗ erro do squad: {e}"),
        Some(squad_event::Payload::Chat(c)) => eprintln!("  💬 {}: {}", c.author, c.text),
        None => {}
    }
}

fn safe_mode(task: &str) {
    eprintln!(
        "  safe-mode read-only: nenhum motor de agente disponível para {task:?}.\n  \
         nenhuma ação de escrita foi tomada. Configure um provider (ANTHROPIC_API_KEY etc.) \
         ou o sidecar Python para reativar squad/agente-único."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use btv_llm::chat::{AssistantTurn, StopReason, Usage as ChatUsage};
    use btv_llm::gateway::GatewayError;
    use std::sync::Mutex;

    /// D3t — paridade do mapeamento evidência→proto contra a fixture canônica
    /// `verification-evidence.v1`: o mesmo conteúdo que o caminho pré-D3t
    /// carregava como string JSON, agora tipado no wire. Prova que
    /// `evidence_to_proto` é fiel, inclusive a opcionalidade de `file`/`line`
    /// (espelho do `Option`/`skip_serializing_if` do struct canônico).
    #[test]
    fn evidence_to_proto_espelha_a_fixture_v1() {
        let fixture =
            include_str!("../../../schemas/fixtures/verification-evidence.v1.example.json");
        let ev: btv_schemas::verification::VerificationEvidence =
            serde_json::from_str(fixture).expect("fixture desserializa no struct canônico");
        let proto = evidence_to_proto(&ev);
        assert_eq!(proto.run_id, "d3t-parity");
        assert_eq!(proto.git_sha, "abc123def456");
        assert_eq!(proto.verdict, btv_proto::squad::Verdict::Fail as i32);
        assert_eq!(proto.produced_at, "2026-07-10T00:00:00Z");
        assert_eq!(proto.steps.len(), 2);
        assert!(proto.steps[0].findings.is_empty());
        let lint = &proto.steps[1];
        assert_eq!(lint.exit_code, 1);
        assert_eq!(lint.duration_ms, 340);
        assert_eq!(lint.findings.len(), 2);
        // Finding com localização preenchida.
        assert_eq!(lint.findings[0].file.as_deref(), Some("src/x.rs"));
        assert_eq!(lint.findings[0].line, Some(12));
        // Finding SEM localização: `None`, nunca string vazia — paridade com o
        // `skip_serializing_if = Option::is_none` do serde.
        assert_eq!(lint.findings[1].file, None);
        assert_eq!(lint.findings[1].line, None);
    }

    /// Gerador de teste que só registra as `messages` recebidas — usado
    /// para provar o mapeamento de papel de `core_generate` (Onda 2), sem
    /// precisar de um provider real.
    struct RecordingGenerator {
        received: Mutex<Vec<Vec<ChatMessage>>>,
    }

    impl Generator for RecordingGenerator {
        async fn generate(
            &self,
            req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            self.received.lock().unwrap().push(req.messages);
            Ok(AssistantTurn {
                content: vec![ContentBlock::Text { text: "ok".into() }],
                stop_reason: StopReason::EndTurn,
                usage: ChatUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
                provider: "recording".into(),
            })
        }
    }

    #[tokio::test]
    async fn core_generate_mapeia_papel_assistant_para_role_assistant() {
        let generator = RecordingGenerator {
            received: Mutex::new(Vec::new()),
        };
        let messages_json = serde_json::to_string(&serde_json::json!([
            {"role": "system", "content": "prompt de sistema"},
            {"role": "user", "content": "tarefa"},
            {"role": "assistant", "content": "{\"action\":\"tool_call\"}"},
            {"role": "user", "content": "observação"},
        ]))
        .unwrap();
        let req = LlmRequest {
            model: "m".into(),
            messages_json,
            temperature: None,
            max_tokens: None,
            requester: "developer".into(),
        };

        core_generate(&generator, &req).await.expect("generate ok");

        let received = generator.received.lock().unwrap();
        let messages = &received[0];
        // system não entra em `messages` (vira `GenerateRequest.system`) —
        // só as duas mensagens de chat + a de assistant sobram, na ordem.
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[2].role, Role::User);
    }

    // ── consenso→ledger (pendência de exercício da Fase 4, re-declarada na
    // Fase 6 Onda 9 e fechada na validação de pendencias.md) ─────────────────

    use btv_proto::squad::squad_service_server::{
        SquadService as SquadServiceTrait, SquadServiceServer,
    };
    use btv_proto::squad::{
        Consensus, HealthRequest, HealthResponse, Proposal, SquadEvent, StepResult,
    };
    use tokio_stream::wrappers::UnixListenerStream;
    use tonic::{Response, Status};

    /// SquadService roteirizado (Rust, sem key, sem Python): emite o roteiro
    /// mínimo de uma tarefa real — proposta → CONSENSO → passo — e encerra o
    /// stream. Mesmo idioma do mock de `client_over_uds.rs` do btv-sidecar.
    struct ScriptedSquadService;

    #[tonic::async_trait]
    impl SquadServiceTrait for ScriptedSquadService {
        type ExecuteTaskStream = tokio_stream::wrappers::ReceiverStream<Result<SquadEvent, Status>>;

        // `tonic::Status` é grande por natureza — mock de teste, sem valor em
        // boxear aqui.
        #[allow(clippy::result_large_err)]
        async fn execute_task(
            &self,
            req: tonic::Request<SquadTask>,
        ) -> Result<Response<Self::ExecuteTaskStream>, Status> {
            let task = req.into_inner();
            let task_id = task.task_id;
            let (tenant_id, actor) = (task.tenant_id, task.actor);
            let (tx, rx) = tokio::sync::mpsc::channel(8);
            let ev = move |payload| {
                Ok(SquadEvent {
                    task_id: task_id.clone(),
                    ts: "2026-01-01T00:00:00Z".into(),
                    payload: Some(payload),
                    // D2t: o mock respeita o contrato do servidor real —
                    // tenant/actor do task ecoados em todo evento.
                    tenant_id: tenant_id.clone(),
                    actor: actor.clone(),
                })
            };
            tx.try_send(ev(squad_event::Payload::Proposal(Proposal {
                agent: "architect".into(),
                confidence: 0.9,
                content_json: "{}".into(),
            })))
            .unwrap();
            tx.try_send(ev(squad_event::Payload::Consensus(Consensus {
                decision_maker: "architect".into(),
                strength: 0.87,
                decision_json: "{}".into(),
                requires_human: false,
            })))
            .unwrap();
            tx.try_send(ev(squad_event::Payload::Step(StepResult {
                step_id: "s1".into(),
                success: true,
                summary: "ok".into(),
            })))
            .unwrap();
            Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
                rx,
            )))
        }

        async fn health(
            &self,
            _req: tonic::Request<HealthRequest>,
        ) -> Result<Response<HealthResponse>, Status> {
            Ok(Response::new(HealthResponse {
                ready: true,
                version: "scripted-0".into(),
            }))
        }
    }

    /// Dirige o MESMO laço de produção do `btv squad` (`render_and_record`,
    /// quem grava `squad.consensus` via `Session::note`) com um stream gRPC
    /// REAL sobre UDS — roteirizado, sem key e sem Python — até o evento
    /// `Consensus`, e assere a entrada `squad.consensus` no ledger com a
    /// cadeia íntegra. É o teste de regressão permanente que a pendência nº 1
    /// da Fase 6 Onda 9 pedia; o gêmeo cross-process (Python real emitindo
    /// `Consensus` no stream) já vive em `btv-sidecar/tests/squad_e2e.rs`.
    #[tokio::test]
    async fn consenso_do_stream_e_registrado_no_ledger_com_cadeia_integra() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("scripted-squad.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(SquadServiceServer::new(ScriptedSquadService))
                .serve_with_incoming(UnixListenerStream::new(listener)),
        );

        let mut client = btv_sidecar::SquadClient::connect(socket.clone())
            .await
            .expect("conecta no scripted squad");
        let stream = client
            .execute_task(SquadTask {
                task_id: "t-consenso".into(),
                description: "tarefa roteirizada".into(),
                decision_type: "architecture".into(),
                max_autonomy_level: 3,
                verification_evidence: None,
                model: String::new(),
                roster: Vec::new(),
                tenant_id: btv_domain::TenantId::LOCAL.to_string(),
                actor: "test:squad".into(),
            })
            .await
            .expect("stream aberto");

        let mut session =
            Session::open(dir.path(), "tarefa roteirizada", "scripted").expect("sessão");
        let outcome = render_and_record(stream, &mut session).await;
        let events = match outcome {
            SquadRun::Completed(events) => events,
            SquadRun::Failed { reason, .. } => panic!("stream deveria completar: {reason}"),
        };
        assert!(
            events
                .iter()
                .any(|e| matches!(e.payload, Some(squad_event::Payload::Consensus(_)))),
            "o roteiro emite um Consensus"
        );
        session.finish(true, 1).expect("finish");
        server.abort();

        // Fronteira: a entrada `squad.consensus` existe no ledger REAL
        // (`.btv/btv.db`), com os campos do evento e a cadeia íntegra.
        let store =
            btv_store::LedgerStore::open(dir.path().join(".btv").join("btv.db").to_str().unwrap())
                .expect("abre o ledger");
        let total = store.verify_chain().expect("cadeia íntegra");
        // session.start + skill... (nenhuma) + consensus + session.end = 3.
        assert_eq!(total, 3, "start + consensus + end");
        let consensus: Vec<_> = store
            .recent(10, None)
            .expect("recent")
            .into_iter()
            .filter(|e| e.kind == "squad.consensus")
            .collect();
        assert_eq!(consensus.len(), 1, "exatamente um squad.consensus");
        assert_eq!(consensus[0].payload["decision_maker"], "architect");
        assert_eq!(consensus[0].payload["requires_human"], false);
    }
}
