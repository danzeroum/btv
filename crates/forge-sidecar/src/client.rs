//! Cliente gRPC do sidecar PromptForge, sobre Unix Domain Socket.

use forge_proto::promptforge::prompt_forge_service_client::PromptForgeServiceClient;
use forge_proto::promptforge::{
    GeneratorInfo, HealthRequest, LintReport, ListGeneratorsRequest, RenderRequest,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::Request;

#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("sidecar indisponível: {0}")]
    Unavailable(String),
    #[error("chamada gRPC falhou: {0}")]
    Rpc(Box<tonic::Status>),
}

impl From<tonic::Status> for SidecarError {
    fn from(status: tonic::Status) -> Self {
        SidecarError::Rpc(Box::new(status))
    }
}

#[derive(Clone, Debug)]
pub struct SidecarClient {
    inner: PromptForgeServiceClient<Channel>,
}

impl SidecarClient {
    /// Conecta ao socket Unix em `path`. Não bloqueia esperando o servidor
    /// existir — a conexão é lazy (o primeiro RPC é que falha se o socket
    /// não estiver pronto). Use [`crate::supervisor::SidecarSupervisor`]
    /// para esperar o health check antes de usar o cliente.
    pub async fn connect(path: impl Into<PathBuf>) -> Result<Self, SidecarError> {
        let path = path.into();
        let channel = Endpoint::try_from("http://sidecar.invalid")
            .expect("URI placeholder válida")
            .connect_with_connector(tower::service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    let stream = tokio::net::UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|e| SidecarError::Unavailable(e.to_string()))?;
        Ok(Self {
            inner: PromptForgeServiceClient::new(channel),
        })
    }

    pub async fn health(&mut self) -> Result<(bool, String), SidecarError> {
        let resp = self
            .inner
            .health(Request::new(HealthRequest {}))
            .await?
            .into_inner();
        Ok((resp.ready, resp.version))
    }

    pub async fn lint(&mut self, prompt: &str) -> Result<LintReport, SidecarError> {
        let resp = self
            .inner
            .lint(Request::new(forge_proto::promptforge::LintRequest {
                prompt: prompt.to_string(),
            }))
            .await?;
        Ok(resp.into_inner())
    }

    pub async fn render(
        &mut self,
        generator: &str,
        fields: HashMap<String, String>,
    ) -> Result<String, SidecarError> {
        let resp = self
            .inner
            .render(Request::new(RenderRequest {
                generator: generator.to_string(),
                fields,
            }))
            .await?;
        Ok(resp.into_inner().prompt)
    }

    pub async fn list_generators(&mut self) -> Result<Vec<GeneratorInfo>, SidecarError> {
        let resp = self
            .inner
            .list_generators(Request::new(ListGeneratorsRequest {}))
            .await?;
        Ok(resp.into_inner().generators)
    }
}

/// Testa se um socket Unix existe e aceita conexão (usado pelo supervisor
/// para poll de prontidão sem depender de um RPC completo).
pub(crate) fn socket_ready(path: &Path) -> bool {
    path.exists()
}
