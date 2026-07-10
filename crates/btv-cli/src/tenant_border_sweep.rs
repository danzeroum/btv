//! Varredura adversarial da borda de tenant (E1s.4): prova, no router REAL de
//! produção (`web_agent::merged_router` + `btv_agent::router`), que o layer de
//! cobertura universal (E1s.3) gateia TODA a superfície em modo saas — não uma
//! amostra. O refino da E1s.4 (o `Mode` resolvido UMA vez no arranque e
//! injetado, não lido da env por-request) torna isto determinístico no MESMO
//! processo dos goldens, sem `set_var` da env de modo — que, além de racear
//! com os goldens (que agora passam pelo extractor), virou `unsafe` na edição
//! 2024.
//!
//! Duas mordidas (prova-que-morde):
//!  - **cobertura**: uma rota NÃO estrangulada real (`GET /api/btv/users`,
//!    sem extractor) vaza (200 sem sessão) quando o layer é removido — o layer
//!    é a única coisa que fecha a superfície não estrangulada no saas.
//!  - **auth de verdade**: token forjado/expirado → 401 (o resolver devolve
//!    None; a expiração no TEMPO é provada contra PG real no E1s.1
//!    `sessao_expirada_nao_resolve`).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

use crate::squad_agent::{default_hub, default_squad_pool};
use crate::tenant_extractor::{guarda_tenant, Mode, SessionResolver, TenantResolucao};
use btv_domain::TenantId;
use btv_store::{BtvStore, LedgerStore};

const TENANT_OK: &str = "00000000-0000-0000-0000-00000000e154";
const TOKEN_OK: &str = "btvs_valido";

/// Resolver que conhece UM token válido — o resto (forjado, expirado) resolve
/// para None, exatamente como o `PgStore::resolve_session` fail-closed.
struct MockResolver;
impl SessionResolver for MockResolver {
    fn resolve(&self, token: &str) -> Option<(TenantId, String)> {
        (token == TOKEN_OK).then(|| (TenantId::parse(TENANT_OK).unwrap(), "u1".to_string()))
    }
}

fn saas() -> TenantResolucao {
    TenantResolucao::new(Mode::Saas, Some(Arc::new(MockResolver)))
}

/// Monta o `btv_agent::router` REAL (os seis estrangulados + rotas NÃO
/// estranguladas reais: `users`, `personas`, `templates/publicacao`) no modo
/// dado. Stores em memória — nenhuma varredura ativa squad (o layer recusa
/// antes de tocar o pool), então nada aqui precisa de uv/Python.
fn btv_router(dir: &std::path::Path, tenant: TenantResolucao) -> Router {
    let store = Arc::new(Mutex::new(BtvStore::open_in_memory().unwrap()));
    let ledger = Arc::new(Mutex::new(LedgerStore::open_in_memory().unwrap()));
    crate::btv_agent::router(
        default_hub(),
        default_squad_pool(dir),
        ledger,
        store,
        tenant,
    )
}

/// Request com `Origin` válido (passa a guarda de `Origin`/CSRF) e SEM sessão.
fn sem_sessao(method: &str, uri: &str) -> Request<Body> {
    let corpo = if method == "GET" {
        Body::empty()
    } else {
        Body::from("{}")
    };
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::ORIGIN, "http://127.0.0.1")
        .header("content-type", "application/json")
        .body(corpo)
        .unwrap()
}

/// A varredura: no `merged_router` REAL (dashboard + web_agent + btv_agent, sob
/// o layer universal e a guarda de Origin), TODA rota em modo saas SEM sessão
/// devolve 401 — inclusive uma rota INEXISTENTE, porque o layer roda ANTES do
/// roteamento: a cobertura é total por CONSTRUÇÃO, não por curadoria de amostra.
#[tokio::test]
async fn saas_sem_sessao_recusa_a_superficie_inteira() {
    let dir = tempfile::tempdir().unwrap();
    let dashboard = Router::new().route("/", axum::routing::get(|| async { "spa" }));
    let extra = btv_router(dir.path(), saas());
    let app =
        crate::web_agent::merged_router(crate::web_agent::default_hub(), dashboard, extra, saas());

    let rotas = [
        ("GET", "/"),                             // SPA (raiz)
        ("GET", "/api/btv/squads"),               // estrangulada
        ("GET", "/api/btv/deliverables"),         // estrangulada
        ("POST", "/api/btv/squads"),              // estrangulada (mutação)
        ("POST", "/api/btv/squads/t1/gate"),      // estrangulada (mutação)
        ("GET", "/api/btv/users"),                // NÃO estrangulada
        ("GET", "/api/btv/personas/tpl"),         // NÃO estrangulada
        ("GET", "/api/btv/templates/publicacao"), // NÃO estrangulada
        ("GET", "/api/session/s1/events"),        // web_agent SSE (GET+stream)
        ("GET", "/rota/que/nao/existe"),          // INEXISTENTE → prova pré-roteamento
    ];
    for (m, u) in rotas {
        let resp = app.clone().oneshot(sem_sessao(m, u)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{m} {u} deveria ser 401 na borda saas (cobertura universal)"
        );
    }
}

/// Sessão VÁLIDA atravessa a borda: a mesma rota não estrangulada que 401-a sem
/// sessão devolve 200 com um Bearer válido — a borda RESOLVE, não faz 401 cego.
#[tokio::test]
async fn saas_com_sessao_valida_passa_a_borda() {
    let dir = tempfile::tempdir().unwrap();
    let dashboard = Router::new().route("/", axum::routing::get(|| async { "spa" }));
    let extra = btv_router(dir.path(), saas());
    let app =
        crate::web_agent::merged_router(crate::web_agent::default_hub(), dashboard, extra, saas());

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/btv/users")
                .header(header::ORIGIN, "http://127.0.0.1")
                .header(header::AUTHORIZATION, format!("Bearer {TOKEN_OK}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "sessão válida não pode ser recusada — a borda resolve e o handler roda"
    );
}

/// Prova-que-morde da COBERTURA: a mesma rota NÃO estrangulada real
/// (`GET /api/btv/users`, sem extractor) 401-a COM o layer e VAZA (200) sem
/// ele — o layer universal é a única coisa que fecha a superfície não
/// estrangulada no saas. Remove o layer e o buraco reabre.
#[tokio::test]
async fn prova_que_morde_sem_o_layer_a_rota_nao_estrangulada_vaza() {
    // COM o layer: 401.
    let dir = tempfile::tempdir().unwrap();
    let app_com = btv_router(dir.path(), saas())
        .layer(axum::middleware::from_fn_with_state(saas(), guarda_tenant));
    let r_com = app_com
        .oneshot(sem_sessao("GET", "/api/btv/users"))
        .await
        .unwrap();
    assert_eq!(r_com.status(), StatusCode::UNAUTHORIZED);

    // SEM o layer (mesmo BtvAgentState saas): a rota não estrangulada VAZA.
    let dir2 = tempfile::tempdir().unwrap();
    let app_sem = btv_router(dir2.path(), saas());
    let r_sem = app_sem
        .oneshot(sem_sessao("GET", "/api/btv/users"))
        .await
        .unwrap();
    assert_eq!(
        r_sem.status(),
        StatusCode::OK,
        "sem o layer universal a rota não estrangulada vaza sem sessão — a mordida"
    );
}

/// Token forjado/expirado → 401: a auth é REALMENTE checada (o resolver devolve
/// None). A expiração no TEMPO é provada contra PG real no E1s.1
/// (`sessao_expirada_nao_resolve`); aqui a cadeia None→401 na borda fecha o elo.
#[tokio::test]
async fn saas_token_forjado_ou_expirado_e_401() {
    let dir = tempfile::tempdir().unwrap();
    let app = btv_router(dir.path(), saas())
        .layer(axum::middleware::from_fn_with_state(saas(), guarda_tenant));
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/btv/users")
                .header(header::ORIGIN, "http://127.0.0.1")
                .header(header::AUTHORIZATION, "Bearer btvs_FORJADO")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Modo LOCAL não gateia nada (a borda universal é no-op): a mesma rota não
/// estrangulada passa sem sessão — o outro lado da invariante que os goldens
/// já congelam byte-a-byte.
#[tokio::test]
async fn local_nao_gateia_nada() {
    let dir = tempfile::tempdir().unwrap();
    let app = btv_router(dir.path(), TenantResolucao::local()).layer(
        axum::middleware::from_fn_with_state(TenantResolucao::local(), guarda_tenant),
    );
    let resp = app
        .oneshot(sem_sessao("GET", "/api/btv/users"))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "modo local nunca recusa — a borda universal é no-op"
    );
    assert_eq!(resp.status(), StatusCode::OK);
}
