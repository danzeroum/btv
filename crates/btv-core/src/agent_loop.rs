//! Loop de agente da Fase 1: mensagens → gateway → tool_use → permissão →
//! execução → tool_result → repete, até `end_turn` ou o limite de passos.
//!
//! O loop é genérico sobre `LlmPort` (gateway real ou roteirizado em
//! teste) e executa ferramentas via `ToolsPort` — desde o D1t ele conhece
//! SÓ as portas do domínio (a violação 4 do levantamento fecha aqui:
//! nenhum concreto de btv-llm/btv-tools/btv-store entra neste crate).
//! Emite `LoopEvent`s via callback — o CLI os transforma em streaming no
//! terminal e entradas no ledger.

use crate::permission::{Decision, PermissionEngine};
use btv_domain::chat::{ChatMessage, ContentBlock, GenerateRequest, Role, StopReason};
use btv_domain::ports::{LlmError, LlmPort, ToolsPort};
use btv_domain::tool::DiffLine;
use serde_json::Value;

/// Eventos observáveis do loop (streaming, ledger, telemetria).
#[derive(Debug)]
pub enum LoopEvent<'a> {
    /// Delta de texto do modelo (streaming).
    TextDelta(&'a str),
    /// Turno do assistente concluído (texto completo, provider, tokens).
    TurnCompleted {
        provider: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    /// Ferramenta prestes a rodar (já autorizada).
    ToolStarted { name: String, scope: String },
    /// Resultado de ferramenta (truncado ao devolver ao modelo se preciso).
    ToolFinished {
        name: String,
        ok: bool,
        summary: String,
        /// Diff de linhas, quando a ferramenta alterou um arquivo texto (edit).
        diff: Option<Vec<DiffLine>>,
    },
    /// Ferramenta negada pela política ou pelo usuário.
    ToolDenied { name: String, scope: String },
}

/// Decide pedidos `Ask` — no CLI pergunta ao usuário; nos testes é roteado.
pub trait PermissionResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool;
}

/// Política fixa para contextos não interativos: nega todo `Ask`.
pub struct DenyAll;
impl PermissionResolver for DenyAll {
    fn resolve(&mut self, _tool: &str, _scope: &str) -> bool {
        false
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("gateway: {0}")]
    Gateway(#[from] LlmError),
    #[error("limite de {0} passos atingido sem end_turn")]
    MaxSteps(usize),
}

pub struct AgentLoop<'a, G: LlmPort> {
    pub generator: &'a G,
    pub tools: &'a dyn ToolsPort,
    pub permissions: PermissionEngine,
    pub model: String,
    pub system: String,
    pub max_steps: usize,
    pub max_tokens: u32,
}

/// Resultado final do loop.
#[derive(Debug)]
pub struct LoopOutcome {
    pub final_text: String,
    pub steps: usize,
    pub messages: Vec<ChatMessage>,
}

/// Resumo de uma rodada (uma entrada do usuário até `end_turn`) quando o
/// histórico é gerenciado pelo chamador (`continue_run`, usado pelo REPL).
#[derive(Debug)]
pub struct TurnSummary {
    pub final_text: String,
    pub steps: usize,
}

impl<'a, G: LlmPort> AgentLoop<'a, G> {
    /// Executa uma tarefa única até o modelo encerrar o turno.
    pub async fn run(
        &self,
        task: &str,
        resolver: &mut (dyn PermissionResolver + Send),
        on_event: &mut (dyn FnMut(LoopEvent) + Send),
    ) -> Result<LoopOutcome, LoopError> {
        let mut messages = vec![ChatMessage::user_text(task)];
        let summary = self.continue_run(&mut messages, resolver, on_event).await?;
        Ok(LoopOutcome {
            final_text: summary.final_text,
            steps: summary.steps,
            messages,
        })
    }

    /// Continua uma conversa existente: assume que a última mensagem do
    /// histórico é a entrada pendente do usuário e itera até `end_turn`,
    /// anexando os turnos ao histórico (base do REPL `btv chat`).
    pub async fn continue_run(
        &self,
        messages: &mut Vec<ChatMessage>,
        resolver: &mut (dyn PermissionResolver + Send),
        on_event: &mut (dyn FnMut(LoopEvent) + Send),
    ) -> Result<TurnSummary, LoopError> {
        let specs = self.tools.specs();

        for step in 1..=self.max_steps {
            let req = GenerateRequest {
                model: self.model.clone(),
                system: self.system.clone(),
                messages: messages.clone(),
                tools: specs.clone(),
                max_tokens: self.max_tokens,
                temperature: None,
            };

            // Encaminha deltas de texto ao observador em tempo real.
            let turn = {
                let mut sink = |d: &str| on_event(LoopEvent::TextDelta(d));
                self.generator.generate(req, &mut sink).await?
            };
            on_event(LoopEvent::TurnCompleted {
                provider: turn.provider.clone(),
                input_tokens: turn.usage.input_tokens,
                output_tokens: turn.usage.output_tokens,
            });

            let tool_uses: Vec<(String, String, Value)> = turn
                .tool_uses()
                .into_iter()
                .map(|(id, name, input)| (id.to_string(), name.to_string(), input.clone()))
                .collect();

            messages.push(ChatMessage {
                role: Role::Assistant,
                content: turn.content.clone(),
            });

            if turn.stop_reason != StopReason::ToolUse || tool_uses.is_empty() {
                return Ok(TurnSummary {
                    final_text: turn.text(),
                    steps: step,
                });
            }

            // Executa cada ferramenta pedida, sob o motor de permissões.
            let mut results = Vec::new();
            for (id, name, input) in tool_uses {
                let result = self.run_tool(&name, &input, resolver, on_event);
                results.push(ContentBlock::ToolResult {
                    tool_use_id: id,
                    content: result.0,
                    is_error: result.1,
                });
            }
            messages.push(ChatMessage {
                role: Role::User,
                content: results,
            });
        }
        Err(LoopError::MaxSteps(self.max_steps))
    }

    /// Retorna (conteúdo, is_error).
    fn run_tool(
        &self,
        name: &str,
        input: &Value,
        resolver: &mut (dyn PermissionResolver + Send),
        on_event: &mut (dyn FnMut(LoopEvent) + Send),
    ) -> (String, bool) {
        let Some(tool) = self.tools.get(name) else {
            return (format!("ferramenta desconhecida: {name}"), true);
        };
        let scope = tool.scope(input);
        let allowed = match self.permissions.evaluate(name, &scope) {
            Decision::Allow => true,
            Decision::Deny => false,
            Decision::Ask => resolver.resolve(name, &scope),
        };
        if !allowed {
            on_event(LoopEvent::ToolDenied {
                name: name.to_string(),
                scope: scope.clone(),
            });
            return (format!("permissão negada para {name} em {scope:?}"), true);
        }
        on_event(LoopEvent::ToolStarted {
            name: name.to_string(),
            scope,
        });
        match tool.run(input) {
            Ok(out) => {
                let summary = out
                    .content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect();
                on_event(LoopEvent::ToolFinished {
                    name: name.to_string(),
                    ok: true,
                    summary,
                    diff: out.diff.clone(),
                });
                let mut content = out.content;
                if out.truncated {
                    match &out.overflow_path {
                        Some(path) => {
                            content.push_str(&format!(
                                "\n[output truncado; completo em {path} — use read para consultar]"
                            ));
                        }
                        None => content.push_str("\n[output truncado]"),
                    }
                }
                (content, false)
            }
            Err(e) => {
                on_event(LoopEvent::ToolFinished {
                    name: name.to_string(),
                    ok: false,
                    summary: e.to_string(),
                    diff: None,
                });
                (e.to_string(), true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit do loop com MOCKS PUROS (Definição de Pronto do D1t, critério da
    //! Fase 2 do plano-mestre): nenhum SQLite, nenhum gRPC, nenhuma rede,
    //! nenhum subprocesso — e o teste do fluxo completo AFIRMA <100ms.
    //! O fio com ferramentas REAIS (edit/bash/skills) continua provado em
    //! `btv-tools/tests/loop_com_ferramentas_reais.rs` (movido no D1t).

    use super::*;
    use btv_domain::chat::{AssistantTurn, ToolSpec, Usage};
    use btv_domain::tool::{Tool, ToolError, ToolOutput};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    /// Gerador roteirizado: devolve turnos pré-definidos em sequência.
    struct Scripted {
        turns: Mutex<Vec<AssistantTurn>>,
    }

    impl LlmPort for Scripted {
        async fn generate(
            &self,
            _req: GenerateRequest,
            on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, LlmError> {
            let turn = self.turns.lock().unwrap().remove(0);
            on_delta(&turn.text());
            Ok(turn)
        }
    }

    /// Ferramenta mock: devolve um output fixo e marca que rodou — o
    /// suficiente para provar autorização, execução e retorno ao modelo.
    struct MockTool {
        name: &'static str,
        output: ToolOutput,
        ran: AtomicBool,
    }

    impl MockTool {
        fn ok(name: &'static str, content: &str) -> Self {
            Self {
                name,
                output: ToolOutput {
                    content: content.into(),
                    truncated: false,
                    overflow_path: None,
                    diff: None,
                },
                ran: AtomicBool::new(false),
            }
        }
    }

    impl Tool for MockTool {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "mock"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        fn scope(&self, _args: &Value) -> String {
            "mock-scope".into()
        }
        fn run(&self, _args: &Value) -> Result<ToolOutput, ToolError> {
            self.ran.store(true, Ordering::SeqCst);
            Ok(self.output.clone())
        }
    }

    /// Porta de ferramentas mock — a implementação mínima de `ToolsPort`.
    struct MockTools {
        tools: Vec<MockTool>,
    }

    impl ToolsPort for MockTools {
        fn specs(&self) -> Vec<ToolSpec> {
            self.tools
                .iter()
                .map(|t| ToolSpec {
                    name: t.name.into(),
                    description: "mock".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                })
                .collect()
        }
        fn get(&self, name: &str) -> Option<&dyn Tool> {
            self.tools
                .iter()
                .find(|t| t.name == name)
                .map(|t| t as &dyn Tool)
        }
    }

    fn turn(content: Vec<ContentBlock>, stop: StopReason) -> AssistantTurn {
        AssistantTurn {
            content,
            stop_reason: stop,
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            provider: "scripted".into(),
        }
    }

    fn tool_use(id: &str, name: &str) -> ContentBlock {
        ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input: serde_json::json!({}),
        }
    }

    struct AllowAll;
    impl PermissionResolver for AllowAll {
        fn resolve(&mut self, _t: &str, _s: &str) -> bool {
            true
        }
    }

    fn agent_loop<'a>(gen: &'a Scripted, tools: &'a MockTools) -> AgentLoop<'a, Scripted> {
        AgentLoop {
            generator: gen,
            tools,
            permissions: PermissionEngine::default(), // tudo Ask → resolver decide
            model: "test-model".into(),
            system: "teste".into(),
            max_steps: 5,
            max_tokens: 512,
        }
    }

    /// O critério LITERAL da DoD do D1t: o fluxo completo (tool_use →
    /// permissão → execução → tool_result → end_turn) roda com mocks em
    /// menos de 100ms — sem SQLite, sem gRPC, sem rede, sem subprocesso.
    #[tokio::test]
    async fn fluxo_completo_com_mocks_roda_em_menos_de_100ms() {
        let inicio = std::time::Instant::now();

        let tools = MockTools {
            tools: vec![MockTool::ok("edit", "arquivo editado")],
        };
        let gen = Scripted {
            turns: Mutex::new(vec![
                turn(
                    vec![
                        ContentBlock::Text {
                            text: "Vou corrigir.".into(),
                        },
                        tool_use("tu1", "edit"),
                    ],
                    StopReason::ToolUse,
                ),
                turn(
                    vec![ContentBlock::Text {
                        text: "Pronto.".into(),
                    }],
                    StopReason::EndTurn,
                ),
            ]),
        };

        let mut events = Vec::new();
        let al = agent_loop(&gen, &tools);
        let outcome = al
            .run("corrija", &mut AllowAll, &mut |e| {
                events.push(format!("{e:?}"))
            })
            .await
            .unwrap();

        assert_eq!(outcome.final_text, "Pronto.");
        assert_eq!(outcome.steps, 2);
        assert!(
            tools.tools[0].ran.load(Ordering::SeqCst),
            "a ferramenta rodou"
        );
        // O tool_result do mock voltou ao "modelo" intacto.
        assert!(matches!(
            &outcome.messages[2].content[0],
            ContentBlock::ToolResult { content, is_error: false, .. } if content == "arquivo editado"
        ));
        assert!(events.iter().any(|e| e.contains("ToolStarted")));
        assert!(events.iter().any(|e| e.contains("TurnCompleted")));

        let gasto = inicio.elapsed();
        assert!(
            gasto < std::time::Duration::from_millis(100),
            "loop com mocks deveria fechar em <100ms, levou {gasto:?}"
        );
    }

    /// Negação: `ToolDenied` é emitido, o tool_result volta como erro, a
    /// ferramenta NÃO executa (o flag do mock prova) e o modelo continua.
    #[tokio::test]
    async fn negacao_vira_tool_result_de_erro_e_a_ferramenta_nao_roda() {
        let tools = MockTools {
            tools: vec![MockTool::ok("bash", "não deveria aparecer")],
        };
        let gen = Scripted {
            turns: Mutex::new(vec![
                turn(vec![tool_use("tu1", "bash")], StopReason::ToolUse),
                turn(
                    vec![ContentBlock::Text {
                        text: "Entendido.".into(),
                    }],
                    StopReason::EndTurn,
                ),
            ]),
        };

        let al = agent_loop(&gen, &tools);
        let mut denied = false;
        let outcome = al
            .run("tarefa", &mut DenyAll, &mut |e| {
                if matches!(e, LoopEvent::ToolDenied { .. }) {
                    denied = true;
                }
            })
            .await
            .unwrap();

        assert!(denied);
        assert!(
            !tools.tools[0].ran.load(Ordering::SeqCst),
            "negada não roda"
        );
        assert!(matches!(
            outcome.messages[2].content[0],
            ContentBlock::ToolResult { is_error: true, .. }
        ));
    }

    /// Ferramenta desconhecida: erro devolvido ao modelo, loop segue.
    #[tokio::test]
    async fn ferramenta_desconhecida_e_tool_result_de_erro() {
        let tools = MockTools { tools: vec![] };
        let gen = Scripted {
            turns: Mutex::new(vec![
                turn(vec![tool_use("tu1", "inexistente")], StopReason::ToolUse),
                turn(
                    vec![ContentBlock::Text { text: "ok".into() }],
                    StopReason::EndTurn,
                ),
            ]),
        };
        let al = agent_loop(&gen, &tools);
        let outcome = al.run("t", &mut AllowAll, &mut |_| {}).await.unwrap();
        assert!(matches!(
            &outcome.messages[2].content[0],
            ContentBlock::ToolResult { content, is_error: true, .. }
                if content.contains("desconhecida")
        ));
    }

    /// Output truncado com arquivo gerenciado: o loop anexa a nota com o
    /// caminho ao devolver ao modelo (a persistência é da ferramenta; o
    /// CONTRATO da nota é do loop — e é isso que o mock prova).
    #[tokio::test]
    async fn output_truncado_ganha_nota_com_o_caminho_gerenciado() {
        let mut t = MockTool::ok("bash", "inicio do output");
        t.output.truncated = true;
        t.output.overflow_path = Some(".btv/tool-outputs/x.txt".into());
        let tools = MockTools { tools: vec![t] };
        let gen = Scripted {
            turns: Mutex::new(vec![
                turn(vec![tool_use("tu1", "bash")], StopReason::ToolUse),
                turn(
                    vec![ContentBlock::Text { text: "ok".into() }],
                    StopReason::EndTurn,
                ),
            ]),
        };
        let al = agent_loop(&gen, &tools);
        let outcome = al.run("t", &mut AllowAll, &mut |_| {}).await.unwrap();
        assert!(matches!(
            &outcome.messages[2].content[0],
            ContentBlock::ToolResult { content, .. }
                if content.contains("output truncado")
                    && content.contains(".btv/tool-outputs/x.txt")
        ));
    }

    /// Gerador que registra quantas mensagens recebeu em cada chamada.
    struct Counting {
        turns: Mutex<Vec<AssistantTurn>>,
        seen: Mutex<Vec<usize>>,
    }

    impl LlmPort for Counting {
        async fn generate(
            &self,
            req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, LlmError> {
            self.seen.lock().unwrap().push(req.messages.len());
            Ok(self.turns.lock().unwrap().remove(0))
        }
    }

    #[tokio::test]
    async fn continue_run_carrega_o_historico_entre_turnos() {
        let tools = MockTools { tools: vec![] };
        let gen = Counting {
            turns: Mutex::new(vec![
                turn(
                    vec![ContentBlock::Text {
                        text: "olá".into()
                    }],
                    StopReason::EndTurn,
                ),
                turn(
                    vec![ContentBlock::Text {
                        text: "de novo".into(),
                    }],
                    StopReason::EndTurn,
                ),
            ]),
            seen: Mutex::new(vec![]),
        };
        let al = AgentLoop {
            generator: &gen,
            tools: &tools,
            permissions: PermissionEngine::default(),
            model: "test".into(),
            system: "t".into(),
            max_steps: 3,
            max_tokens: 64,
        };

        let mut history = vec![ChatMessage::user_text("primeira")];
        let s1 = al
            .continue_run(&mut history, &mut AllowAll, &mut |_| {})
            .await
            .unwrap();
        history.push(ChatMessage::user_text("segunda"));
        let s2 = al
            .continue_run(&mut history, &mut AllowAll, &mut |_| {})
            .await
            .unwrap();

        assert_eq!(s1.final_text, "olá");
        assert_eq!(s2.final_text, "de novo");
        // 1ª chamada viu 1 mensagem; a 2ª viu 3 (user, assistant, user).
        assert_eq!(*gen.seen.lock().unwrap(), vec![1, 3]);
        assert_eq!(history.len(), 4);
    }

    #[tokio::test]
    async fn limite_de_passos_e_um_erro() {
        let tools = MockTools {
            tools: vec![MockTool::ok("read", "conteudo")],
        };
        // Sempre pede ferramenta — nunca termina.
        let loops: Vec<AssistantTurn> = (0..6)
            .map(|i| {
                turn(
                    vec![tool_use(&format!("tu{i}"), "read")],
                    StopReason::ToolUse,
                )
            })
            .collect();
        let gen = Scripted {
            turns: Mutex::new(loops),
        };
        let al = agent_loop(&gen, &tools);
        let err = al
            .run("tarefa", &mut AllowAll, &mut |_| {})
            .await
            .unwrap_err();
        assert!(matches!(err, LoopError::MaxSteps(5)));
    }
}
