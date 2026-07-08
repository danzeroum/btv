//! Teste de integração cross-process real: sobe o servidor Python de
//! verdade (`uv run python -m btv_promptforge.server`) via
//! `SidecarSupervisor` e fala com ele por gRPC — valida a Fase 3
//! fim-a-fim (não um mock, ao contrário de `client_over_uds.rs`).
//! Pulado (sem falhar) se `uv` ou o workspace Python não estiverem
//! disponíveis no ambiente.

use btv_sidecar::SidecarSupervisor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

fn python_workspace_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../python"))
}

#[tokio::test]
async fn sidecar_python_real_responde_por_grpc() {
    let dir = python_workspace_dir();
    if !dir.join("pyproject.toml").exists() {
        eprintln!("workspace Python ausente em {dir:?} — pulando teste de integração real");
        return;
    }

    let socket = std::env::temp_dir().join(format!("btv-sidecar-test-{}.sock", std::process::id()));
    let mut supervisor = match SidecarSupervisor::spawn(&dir, socket) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "não foi possível spawnar o sidecar ({e}) — pulando teste de integração real"
            );
            return;
        }
    };

    let mut client = supervisor
        .wait_ready(Duration::from_secs(30))
        .await
        .expect("sidecar Python real deveria ficar pronto");

    let (ready, version) = client.health().await.unwrap();
    assert!(ready);
    assert!(!version.is_empty());

    let generators = client.list_generators().await.unwrap();
    let names: Vec<&str> = generators.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"code-review"), "geradores: {names:?}");
    assert!(names.contains(&"bug-fix"), "geradores: {names:?}");

    let mut fields = HashMap::new();
    fields.insert("language".to_string(), "rust".to_string());
    fields.insert("context".to_string(), "gateway LLM".to_string());
    fields.insert("code".to_string(), "fn main() {}".to_string());
    let rendered = client.render("code-review", fields).await.unwrap();
    assert!(rendered.contains("rust"));
    assert!(rendered.contains("fn main() {}"));

    let vago = client.lint("faça o melhor código").await.unwrap();
    assert!(
        vago.score < 0.7,
        "score inesperado para prompt vago: {}",
        vago.score
    );

    let bom = client
        .lint("Revise a função de pagamento buscando arredondamento. Entrada:\n```python\ndef f(): pass\n```")
        .await
        .unwrap();
    assert!(
        bom.score >= 0.7,
        "score inesperado para prompt concreto: {}",
        bom.score
    );
}
