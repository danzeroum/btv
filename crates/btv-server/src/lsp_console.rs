//! Fase 7 Onda 10 (A7): enumera `.btv/lsp.toml` para exibição via
//! `btv_tools::lsp::read_server_configs`. C4-4: migrou para `btv-server` (a
//! casa da borda axum) — o leitor de config já vivia em `btv-tools` desde o
//! C4-3, então o movimento é puro.
//!
//! **Zero probe sob demanda**: esta rota NUNCA sobe o processo do language
//! server para "ver se está rodando" — isso quebraria exatamente a
//! propriedade que `skills.rs`'s `lsp_server_declarado_registra_tres_consultas_lazy`
//! já prova segura (um comando LSP inexistente registra as 3 tools sem nada
//! subir). Cada servidor declarado é sempre "declarado, não iniciado": não há
//! como saber se algum OUTRO processo (`btv run`/`chat`/`tui`) já subiu
//! aquele language server sem introspectar estado entre processos, o que esta
//! onda não constrói — mostrar um "rodando" fabricado seria pior que não
//! mostrar nada.

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone)]
struct LspConsoleState {
    root: PathBuf,
}

#[derive(Serialize)]
struct LspServerView {
    id: String,
    command: String,
    args: Vec<String>,
}

/// `GET /api/lsp` — servidores declarados em `.btv/lsp.toml`, sem subir
/// nenhum processo.
async fn list_lsp(State(state): State<LspConsoleState>) -> impl IntoResponse {
    let servers: Vec<LspServerView> = btv_tools::lsp::read_server_configs(&state.root)
        .into_iter()
        .map(|c| LspServerView {
            id: c.id,
            command: c.command,
            args: c.args,
        })
        .collect();
    Json(servers)
}

/// Router aditivo do console de LSP — `.merge()`ado ao router do agente web,
/// mesma composição de `mcp_console::router`/`sandbox_console::router`.
pub fn router(root: PathBuf) -> Router {
    Router::new()
        .route("/api/lsp", get(list_lsp))
        .with_state(LspConsoleState { root })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Fronteira da Onda 10 (A7): um comando LSP INEXISTENTE aparece
    /// declarado — nenhum processo sobe (mesma prova que
    /// `skills.rs`'s teste de registro lazy já faz, agora pela rota HTTP).
    #[tokio::test]
    async fn lsp_declarado_aparece_sem_subir_processo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".btv")).unwrap();
        std::fs::write(
            dir.path().join(".btv").join("lsp.toml"),
            "[[server]]\nid = \"rust\"\ncommand = \"comando-lsp-inexistente-xyz\"\nargs = [\"--stdio\"]\n",
        )
        .unwrap();

        let app = router(dir.path().to_path_buf());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/lsp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let servers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["id"], "rust");
        assert_eq!(servers[0]["command"], "comando-lsp-inexistente-xyz");
        assert_eq!(servers[0]["args"][0], "--stdio");
    }

    /// Sem `.btv/lsp.toml`, devolve lista vazia (fail-soft, mesmo padrão do
    /// console MCP sem config).
    #[tokio::test]
    async fn sem_config_devolve_lista_vazia() {
        let dir = tempfile::tempdir().unwrap();
        let app = router(dir.path().to_path_buf());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/lsp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let servers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(servers.is_empty());
    }
}
