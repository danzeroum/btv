//! Sobe o sidecar Python (`forge_promptforge`) com degradação graciosa:
//! se não subir a tempo, `try_start` devolve `None` e o CLI segue sem
//! lint/geradores — o sidecar nunca é obrigatório para `run`/`chat`/`tui`.
//!
//! Localização do workspace Python: `FORGE_PYTHON_DIR` se definida, senão
//! o caminho relativo ao código-fonte deste crate (`../../python`) — só
//! funciona em desenvolvimento (`cargo run`); empacotar o sidecar para
//! distribuição é uma preocupação da Fase 6.

use forge_sidecar::{SidecarClient, SidecarSupervisor};
use std::path::PathBuf;
use std::time::Duration;

const START_TIMEOUT: Duration = Duration::from_secs(8);

fn python_workspace_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FORGE_PYTHON_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../python"))
}

/// Tenta subir o sidecar; devolve `None` silenciosamente se o workspace
/// Python não existir, o processo não subir, ou o health check não
/// responder dentro do prazo. O `SidecarSupervisor` devolvido precisa ficar
/// vivo enquanto o cliente for usado (o processo morre quando ele é dropado).
pub async fn try_start() -> Option<(SidecarSupervisor, SidecarClient)> {
    let dir = python_workspace_dir();
    if !dir.join("pyproject.toml").exists() {
        return None;
    }
    let socket = std::env::temp_dir().join(format!("forge-sidecar-{}.sock", std::process::id()));
    let mut supervisor = SidecarSupervisor::spawn(&dir, socket).ok()?;
    let client = supervisor.wait_ready(START_TIMEOUT).await.ok()?;
    Some((supervisor, client))
}

/// Formata um aviso de lint para o terminal, ou `None` se o prompt já
/// está bom o bastante para não incomodar o usuário.
pub fn advisory(report: &forge_proto::promptforge::LintReport) -> Option<String> {
    if report.score >= 0.9 {
        return None;
    }
    let issues: Vec<String> = report.issues.iter().map(|i| i.message.clone()).collect();
    Some(format!(
        "  ✎ aviso de prompt (score {:.2}, nota {}): {}",
        report.score,
        report.grade,
        issues.join("; ")
    ))
}
