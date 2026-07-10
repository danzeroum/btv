//! O fio do loop com ferramentas REAIS (edit/bash/read/skills executando
//! de verdade em tempdir) — movido de `btv-core` no D1t, quando o core
//! passou a depender só das portas do domínio: quem conhece as
//! implementações concretas é ESTE crate, então é aqui que o fio completo
//! (registry → permissão → subprocesso → output real) é provado. Os
//! cenários são os MESMOS de sempre, sem afrouxar um assert.

use btv_core::{AgentLoop, DenyAll, LoopError, LoopEvent, PermissionEngine, PermissionResolver};
use btv_domain::chat::{
    AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, Role, StopReason, Usage,
};
use btv_domain::ports::{LlmError, LlmPort};
use btv_tools::ToolRegistry;
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

struct AllowAll;
impl PermissionResolver for AllowAll {
    fn resolve(&mut self, _t: &str, _s: &str) -> bool {
        true
    }
}

fn agent_loop<'a>(gen: &'a Scripted, tools: &'a ToolRegistry) -> AgentLoop<'a, Scripted> {
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

#[tokio::test]
async fn fluxo_completo_com_edicao_de_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "valor = 1\n").unwrap();
    let tools = ToolRegistry::default_set(dir.path());

    let gen = Scripted {
        turns: Mutex::new(vec![
            turn(
                vec![
                    ContentBlock::Text {
                        text: "Vou corrigir.".into(),
                    },
                    ContentBlock::ToolUse {
                        id: "tu1".into(),
                        name: "edit".into(),
                        input: serde_json::json!({"path": "f.txt", "old_string": "valor = 1", "new_string": "valor = 2"}),
                    },
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

    let al = agent_loop(&gen, &tools);
    let mut events = Vec::new();
    let outcome = al
        .run("corrija f.txt", &mut AllowAll, &mut |e| {
            events.push(format!("{e:?}"))
        })
        .await
        .unwrap();

    assert_eq!(outcome.final_text, "Pronto.");
    assert_eq!(outcome.steps, 2);
    let content = std::fs::read_to_string(dir.path().join("f.txt")).unwrap();
    assert_eq!(content, "valor = 2\n");
    assert!(events.iter().any(|e| e.contains("ToolStarted")));
}

#[tokio::test]
async fn output_grande_vira_managed_tool_output_file() {
    let dir = tempfile::tempdir().unwrap();
    let tools = ToolRegistry::default_set(dir.path());

    let gen = Scripted {
        turns: Mutex::new(vec![
            turn(
                vec![ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "bash".into(),
                    // gera >32 KiB de saída para ultrapassar DEFAULT_OUTPUT_LIMIT
                    input: serde_json::json!({"command": "yes MARCADOR_FINAL | head -c 40000"}),
                }],
                StopReason::ToolUse,
            ),
            turn(
                vec![ContentBlock::Text { text: "ok".into() }],
                StopReason::EndTurn,
            ),
        ]),
    };

    let al = agent_loop(&gen, &tools);
    let outcome = al
        .run("rode o comando", &mut AllowAll, &mut |_| {})
        .await
        .unwrap();

    let tool_result = match &outcome.messages[2].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(!is_error);
            content.clone()
        }
        other => panic!("esperava ToolResult, achei {other:?}"),
    };
    assert!(tool_result.contains("output truncado"));
    assert!(tool_result.contains(".btv/tool-outputs/"));

    let marker = tool_result
        .rsplit("completo em ")
        .next()
        .unwrap()
        .split(" —")
        .next()
        .unwrap();
    let persisted = std::fs::read_to_string(dir.path().join(marker)).unwrap();
    assert!(
        persisted.len() >= 40_000 && persisted.contains("MARCADOR_FINAL"),
        "arquivo gerenciado tem o conteúdo completo"
    );
}

#[tokio::test]
async fn negacao_vira_tool_result_de_erro_e_o_modelo_continua() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x\n").unwrap();
    let tools = ToolRegistry::default_set(dir.path());

    let gen = Scripted {
        turns: Mutex::new(vec![
            turn(
                vec![ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "rm -rf /"}),
                }],
                StopReason::ToolUse,
            ),
            turn(
                vec![ContentBlock::Text {
                    text: "Entendido, não vou executar.".into(),
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
    assert_eq!(outcome.steps, 2);
    // O tool_result de erro foi entregue ao "modelo" na 2ª chamada.
    let last_user = outcome
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::User))
        .unwrap();
    // mensagens: [user task, assistant tool_use, user tool_result(erro), assistant final]
    assert!(matches!(
        outcome.messages[2].content[0],
        ContentBlock::ToolResult { is_error: true, .. }
    ));
    let _ = last_user;
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
    let dir = tempfile::tempdir().unwrap();
    let tools = ToolRegistry::default_set(dir.path());
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
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x\n").unwrap();
    let tools = ToolRegistry::default_set(dir.path());
    // Sempre pede ferramenta — nunca termina.
    let loops: Vec<AssistantTurn> = (0..6)
        .map(|i| {
            turn(
                vec![ContentBlock::ToolUse {
                    id: format!("tu{i}"),
                    name: "read".into(),
                    input: serde_json::json!({"path": "f.txt"}),
                }],
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

/// Fronteira nº 1 da Onda 1 (Fase 6): o fio completo — registry → permissão
/// → run → output — com uma skill registrada que **executa de verdade** como
/// subprocesso e cujo stdout real volta ao loop. Não é unit do wrapper.
#[tokio::test]
async fn skill_registrada_e_invocada_de_verdade_pelo_loop() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("greet");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill =
        btv_tools::SkillTool::new("greet", "cumprimenta", r#"printf 'OLA:%s' "$1""#, skill_dir);
    let mut tools = ToolRegistry::default_set(dir.path());
    tools.register(Box::new(skill));

    let gen = Scripted {
        turns: Mutex::new(vec![
            turn(
                vec![ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "greet".into(),
                    input: serde_json::json!({"input": "mundo"}),
                }],
                StopReason::ToolUse,
            ),
            turn(
                vec![ContentBlock::Text {
                    text: "feito".into(),
                }],
                StopReason::EndTurn,
            ),
        ]),
    };

    let al = agent_loop(&gen, &tools);
    let outcome = al
        .run("use a skill", &mut AllowAll, &mut |_| {})
        .await
        .unwrap();

    // A skill executou de verdade: seu stdout real voltou como tool_result.
    let tool_result = match &outcome.messages[2].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(!is_error);
            content.clone()
        }
        other => panic!("esperava ToolResult, achei {other:?}"),
    };
    assert_eq!(tool_result, "OLA:mundo");
}

/// Fronteira nº 4 da Onda 1: a invocação da skill passa pelo permission-
/// engine; um resolver que nega emite `ToolDenied` e a skill **não** roda —
/// provado por um entrypoint que criaria um arquivo se tivesse executado.
#[tokio::test]
async fn skill_negada_pela_permissao_nao_executa() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("toca");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let marca = dir.path().join("EXECUTOU");
    let entry = format!(r#"touch "{}""#, marca.display());
    let skill = btv_tools::SkillTool::new("toca", "cria arquivo", entry, skill_dir);
    let mut tools = ToolRegistry::default_set(dir.path());
    tools.register(Box::new(skill));

    let gen = Scripted {
        turns: Mutex::new(vec![
            turn(
                vec![ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "toca".into(),
                    input: serde_json::json!({"input": ""}),
                }],
                StopReason::ToolUse,
            ),
            turn(
                vec![ContentBlock::Text { text: "ok".into() }],
                StopReason::EndTurn,
            ),
        ]),
    };

    let al = agent_loop(&gen, &tools);
    let mut denied = false;
    let _ = al
        .run("tarefa", &mut DenyAll, &mut |e| {
            if matches!(e, LoopEvent::ToolDenied { .. }) {
                denied = true;
            }
        })
        .await
        .unwrap();

    assert!(denied, "a permissão deveria ter negado a skill");
    assert!(
        !marca.exists(),
        "a skill negada não pode ter executado o entrypoint"
    );
}
