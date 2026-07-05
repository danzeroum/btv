//! `forge` — CLI da plataforma unificada (BuildToValue + opencode + prompte).
//!
//! Fase 1 (scaffold): esqueleto de comandos. `run`/`chat` ganham o loop de
//! agente real ainda na Fase 1; `squad` ativa o sidecar Python na Fase 4.

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge_llm::{tier_from_id, ModelTier};

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
    },
    /// Abre o REPL de conversa.
    Chat,
    /// Roda o pipeline de verificação determinística.
    Verify,
    /// Delega a tarefa ao squad multi-agente (requer sidecar Python).
    Squad { task: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { task, model } => {
            let tier = tier_from_id(&model);
            let tier_name = match tier {
                ModelTier::Small => "small",
                ModelTier::Medium => "medium",
                ModelTier::Large => "large",
            };
            println!("forge run — tarefa: {task:?} | modelo: {model} (tier: {tier_name})");
            println!("(loop de agente em implementação — Fase 1 do roadmap)");
        }
        Commands::Chat => {
            println!("forge chat — REPL em implementação (Fase 1 do roadmap)");
        }
        Commands::Verify => {
            println!("forge verify — pipeline em implementação (Fase 5 do roadmap)");
        }
        Commands::Squad { task } => {
            println!("forge squad — tarefa: {task:?}");
            println!("(sidecar forge-squadd em implementação — Fase 4 do roadmap)");
        }
    }
    Ok(())
}
