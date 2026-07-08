//! Testa `SidecarClient` contra um servidor tonic real (mock, não Python)
//! servido sobre um Unix Domain Socket efêmero — cobre a fiação
//! Rust↔Rust do transporte; o teste cross-process com o servidor Python
//! de verdade fica em `crates/btv-sidecar/tests/python_sidecar.rs`.

use btv_proto::promptforge::prompt_forge_service_server::{
    PromptForgeService, PromptForgeServiceServer,
};
use btv_proto::promptforge::{
    GeneratorInfo, HealthRequest, HealthResponse, LintReport, ListGeneratorsRequest,
    ListGeneratorsResponse, RenderRequest, RenderResponse,
};
use btv_sidecar::SidecarClient;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Request, Response, Status};

struct MockPromptForge;

#[tonic::async_trait]
impl PromptForgeService for MockPromptForge {
    async fn health(
        &self,
        _req: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse {
            ready: true,
            version: "mock-1".into(),
        }))
    }

    async fn lint(
        &self,
        req: Request<btv_proto::promptforge::LintRequest>,
    ) -> Result<Response<LintReport>, Status> {
        let prompt = req.into_inner().prompt;
        Ok(Response::new(LintReport {
            score: if prompt.len() > 10 { 1.0 } else { 0.2 },
            grade: "A".into(),
            issues: vec![],
        }))
    }

    async fn render(
        &self,
        req: Request<RenderRequest>,
    ) -> Result<Response<RenderResponse>, Status> {
        let req = req.into_inner();
        let name = req.fields.get("nome").cloned().unwrap_or_default();
        Ok(Response::new(RenderResponse {
            prompt: format!("gerador={} nome={name}", req.generator),
        }))
    }

    async fn list_generators(
        &self,
        _req: Request<ListGeneratorsRequest>,
    ) -> Result<Response<ListGeneratorsResponse>, Status> {
        Ok(Response::new(ListGeneratorsResponse {
            generators: vec![GeneratorInfo {
                name: "code-review".into(),
                category: "codigo".into(),
                fields: vec![],
            }],
        }))
    }
}

#[tokio::test]
async fn cliente_fala_com_servidor_mock_sobre_uds() {
    let dir = tempfile::tempdir().unwrap();
    let socket_path = dir.path().join("mock.sock");

    let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
    let incoming = UnixListenerStream::new(listener);
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(PromptForgeServiceServer::new(MockPromptForge))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    // dá tempo do servidor começar a aceitar conexões
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut client = SidecarClient::connect(socket_path).await.unwrap();

    let (ready, version) = client.health().await.unwrap();
    assert!(ready);
    assert_eq!(version, "mock-1");

    let report = client.lint("um prompt razoavelmente longo").await.unwrap();
    assert_eq!(report.score, 1.0);

    let mut fields = std::collections::HashMap::new();
    fields.insert("nome".to_string(), "Ana".to_string());
    let rendered = client.render("code-review", fields).await.unwrap();
    assert_eq!(rendered, "gerador=code-review nome=Ana");

    let generators = client.list_generators().await.unwrap();
    assert_eq!(generators.len(), 1);
    assert_eq!(generators[0].name, "code-review");
}
