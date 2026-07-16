//! Servidor `CoreService` (`schemas/proto/core.proto`) — o lado Rust do
//! laço bidirecional (Onda 4d). O sidecar Python do squad chama de volta
//! `Generate` (LLM), `RequestPermission` (HITL) e `RunTool` (execução real
//! de ferramenta, ativada na "tool execution architecture" — squad como
//! executor) enquanto executa.
//!
//! `Generate`/`RequestPermission`/`RunTool` são atendidos por um
//! [`CoreBackend`] injetável: em produção, o `Gateway` real (btv-cli) +
//! um `ToolRegistry`/`PermissionEngine` + um resolver de permissão; em
//! teste, um backend roteirizado. Expõe só `Generate`/`RunTool`/
//! `RequestPermission`. Os antigos `AppendLedger`/`Recall`/`Remember` foram
//! REMOVIDOS (ADR 0034): eram a direção errada para memória (`CoreService` é
//! servido pelo Rust; memória mora no Python) — superados pelo `MemoryService`
//! (servido pelo Python, ADR 0022).

use btv_proto::core::core_service_server::{CoreService, CoreServiceServer};
use btv_proto::core::{
    permission_decision, PermissionDecision, PermissionRequest, ToolCall, ToolResult,
};
use btv_proto::llm::{llm_chunk, LlmChunk, LlmRequest, Usage};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::{Request, Response, Status};

/// Backend que atende os RPCs realmente usados pelo squad. Recebe os tipos
/// **proto** (com `requester`, para telemetria/roteamento em teste).
#[tonic::async_trait]
pub trait CoreBackend: Send + Sync + 'static {
    /// Gera texto para um request; devolve `(texto_agregado, usage)` ou uma
    /// mensagem de erro (que vira um `LlmChunk::error` no stream).
    async fn generate(&self, req: &LlmRequest) -> Result<(String, Usage), String>;
    /// Decide uma permissão (true = ALLOW). Fail-closed é responsabilidade
    /// de quem implementa.
    async fn request_permission(&self, req: &PermissionRequest) -> bool;
    /// Executa uma ferramenta. Nunca falha a RPC — negação do motor de
    /// permissões ou erro de execução viram um `ToolResult` com o
    /// `exit_code` apropriado, não um `Status` de transporte (mesmo
    /// espírito de `generate`: erro de domínio vira payload).
    async fn run_tool(&self, call: &ToolCall) -> ToolResult;
}

pub struct CoreServer<B: CoreBackend> {
    backend: Arc<B>,
}

impl<B: CoreBackend> CoreServer<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    pub fn into_service(self) -> CoreServiceServer<Self> {
        CoreServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl<B: CoreBackend> CoreService for CoreServer<B> {
    type GenerateStream = ReceiverStream<Result<LlmChunk, Status>>;

    async fn generate(
        &self,
        request: Request<LlmRequest>,
    ) -> Result<Response<Self::GenerateStream>, Status> {
        let req = request.into_inner();
        let backend = self.backend.clone();
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            // Item do stream é `Result<LlmChunk, Status>` (exigido pelo
            // tonic); construímos os `Ok(..)` inline em vez de num helper
            // para não disparar `result_large_err` sobre o `Status`.
            let send = |payload| LlmChunk {
                payload: Some(payload),
            };
            match backend.generate(&req).await {
                Ok((text, usage)) => {
                    let _ = tx.send(Ok(send(llm_chunk::Payload::TextDelta(text)))).await;
                    let _ = tx.send(Ok(send(llm_chunk::Payload::Usage(usage)))).await;
                }
                Err(e) => {
                    let _ = tx.send(Ok(send(llm_chunk::Payload::Error(e)))).await;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn request_permission(
        &self,
        request: Request<PermissionRequest>,
    ) -> Result<Response<PermissionDecision>, Status> {
        let approved = self.backend.request_permission(&request.into_inner()).await;
        let decision = if approved {
            permission_decision::Decision::Allow
        } else {
            permission_decision::Decision::Deny
        };
        Ok(Response::new(PermissionDecision {
            decision: decision as i32,
            operator_note: None,
        }))
    }

    async fn run_tool(&self, request: Request<ToolCall>) -> Result<Response<ToolResult>, Status> {
        Ok(Response::new(
            self.backend.run_tool(&request.into_inner()).await,
        ))
    }
}

/// Sobe o `CoreService` num Unix Domain Socket. Bloqueia até o servidor
/// terminar — normalmente rodado numa task e abortado quando o squad
/// encerra.
pub async fn serve_core<B: CoreBackend>(
    backend: B,
    socket_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = std::fs::remove_file(&socket_path);
    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    let incoming = UnixListenerStream::new(listener);
    tonic::transport::Server::builder()
        .add_service(CoreServer::new(backend).into_service())
        .serve_with_incoming(incoming)
        .await?;
    Ok(())
}
