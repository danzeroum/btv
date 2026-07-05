//! `forge` — CLI da plataforma unificada (BuildToValue + opencode + prompte).
//!
//! Fase 1: `run` executa o loop de agente real — gateway LLM com streaming,
//! ferramentas sob permissão interativa e ledger em `.forge/forge.db`.
//! `squad` ativa o sidecar Python na Fase 4; `verify` completa na Fase 5.

mod session;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use forge_core::{AgentLoop, LoopEvent, PermissionResolver, BUILD, PLAN};
use forge_llm::{tier_from_id, Gateway, ModelTier};
use forge_tools::ToolRegistry;
use std::io::Write;

#[derive(Parser)]
#[command(
    name = "forge",
    version,
    about = "Coding agent unificado (Rust + Python)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Executa uma tarefa única com o agente ativo.
    Run {
        /// Descrição da tarefa.
        task: String,
        /// Modelo a usar (define o ModelTier).
        #[arg(long, default_value = "claude-sonnet-5")]
        model: String,
        /// Perfil de agente: build (edita) ou plan (somente leitura).
        #[arg(long, default_value = "build")]
        agent: String,
        /// Aprova automaticamente pedidos de permissão (use com cautela).
        #[arg(long)]
        yes: bool,
    },
    /// Abre o REPL de conversa.
    Chat,
    /// Roda o pipeline de verificação determinística.
    Verify,
    /// Delega a tarefa ao squad multi-agente (requer sidecar Python).
    Squad { task: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            task,
            model,
            agent,
            yes,
        } => run(task, model, agent, yes).await,
        Commands::Chat => {
            println!("forge chat — REPL em implementação (Fase 1 do roadmap)");
            Ok(())
        }
        Commands::Verify => {
            println!("forge verify — pipeline em implementação (Fase 5 do roadmap)");
            Ok(())
        }
        Commands::Squad { task } => {
            println!("forge squad — tarefa: {task:?}");
            println!("(sidecar forge-squadd em implementação — Fase 4 do roadmap)");
            Ok(())
        }
    }
}

/// Pergunta ao usuário no terminal quando a política devolve `Ask`.
struct CliResolver {
    auto_yes: bool,
}

impl PermissionResolver for CliResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool {
        if self.auto_yes {
            return true;
        }
        eprint!("\n  permitir {tool} em {scope:?}? [s/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(
            answer.trim().to_lowercase().as_str(),
            "s" | "sim" | "y" | "yes"
        )
    }
}

async fn run(task: String, model: String, agent: String, yes: bool) -> Result<()> {
    let gateway = Gateway::from_env();
    let available = gateway.available();
    if available.is_empty() {
        bail!(
            "nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY"
        );
    }

    let profile = match agent.as_str() {
        "build" => &BUILD,
        "plan" => &PLAN,
        other => bail!("agente desconhecido: {other} (use build ou plan)"),
    };

    let root = std::env::current_dir().context("diretório atual")?;
    let tools = ToolRegistry::default_set(&root);
    let tier = tier_from_id(&model);

    let mut session = session::Session::open(&root, &task, &model)?;
    eprintln!(
        "forge run — modelo {model} ({}) · agente {} · providers: {}",
        tier_name(tier),
        profile.name,
        available.join(", ")
    );

    let agent_loop = AgentLoop {
        generator: &gateway,
        tools: &tools,
        permissions: (profile.permissions)(),
        model: model.clone(),
        system: system_prompt(tier),
        max_steps: 20,
        max_tokens: 4096,
    };

    let mut resolver = CliResolver { auto_yes: yes };
    let mut on_event = |event: LoopEvent| {
        match &event {
            LoopEvent::TextDelta(d) => {
                print!("{d}");
                let _ = std::io::stdout().flush();
            }
            LoopEvent::TurnCompleted { .. } => println!(),
            LoopEvent::ToolStarted { name, scope } => eprintln!("  ⚒ {name} {scope:?}"),
            LoopEvent::ToolFinished { name, ok, summary } => {
                eprintln!("  {} {name}: {summary}", if *ok { "✓" } else { "✗" })
            }
            LoopEvent::ToolDenied { name, scope } => eprintln!("  ⛔ {name} {scope:?} negado"),
        }
        session.record(&event);
    };

    let result = agent_loop.run(&task, &mut resolver, &mut on_event).await;
    match result {
        Ok(outcome) => {
            session.finish(true, outcome.steps)?;
            eprintln!(
                "\nconcluído em {} passo(s) · ledger íntegro: {} entrada(s)",
                outcome.steps,
                session.verify()?
            );
            Ok(())
        }
        Err(e) => {
            session.finish(false, 0)?;
            bail!("{e}");
        }
    }
}

fn system_prompt(tier: ModelTier) -> String {
    let base = "Você é o forge, um coding agent de terminal. Trabalhe no diretório atual \
usando as ferramentas disponíveis (read, grep, edit, bash). Leia antes de editar; edits \
exigem old_string única. Verifique seu trabalho com as ferramentas (testes, build) antes \
de concluir. Seja direto e objetivo nas respostas.";
    match tier {
        // Disciplina de passos para modelos small (fork do opencode).
        ModelTier::Small => format!("{base} Faça UMA ação por vez e reavalie após cada resultado."),
        _ => base.to_string(),
    }
}

fn tier_name(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Small => "small",
        ModelTier::Medium => "medium",
        ModelTier::Large => "large",
    }
}
