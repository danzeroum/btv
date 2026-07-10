//! Golden tests dos fluxos HTTP servidos pelo dashboard (`btv-server`):
//! `GET /api/btv/templates` e `GET /api/ledger` (Trilha T1 do plano DDD
//! multitenant). O contrato completo — status, content-type, corpo — é
//! comparado byte-a-byte (via JSON) com a fixture gravada da resposta REAL;
//! ver `btv-golden` para a mecânica e a regravação.
//!
//! Os dois fluxos são 100% determinísticos (templates embutidos no binário;
//! ledger semeado com timestamps fixos ⇒ hashes da cadeia determinísticos),
//! então NENHUM campo é volátil: até `entry_hash`/`prev_hash` são exatos —
//! mudança no algoritmo da cadeia também é mudança de contrato.

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use btv_schemas::ledger::LedgerEntry;
use btv_store::{LedgerStore, PromptLibrary, Telemetry};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Fixture mínima da SPA — o router exige um `web_dir` com `index.html`.
fn fixture_web_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>btv</html>").unwrap();
    dir
}

fn app_com_ledger(ledger: LedgerStore) -> (Router, tempfile::TempDir) {
    let web_dir = fixture_web_dir();
    let app = btv_server::router(
        Telemetry::open_in_memory().unwrap(),
        Arc::new(Mutex::new(PromptLibrary::open_in_memory().unwrap())),
        Arc::new(Mutex::new(ledger)),
        web_dir.path(),
        web_dir.path(),
    );
    (app, web_dir)
}

/// Dispara a requisição in-process e captura o contrato observável.
async fn drive(
    app: &Router,
    method: &str,
    uri: &str,
) -> (u16, Option<String>, Option<String>, serde_json::Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .map(|v| v.to_str().unwrap_or_default().to_string())
    };
    let (content_type, content_disposition) =
        (header("content-type"), header("content-disposition"));
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::json!({ "text": String::from_utf8_lossy(&bytes) }))
    };
    (status, content_type, content_disposition, body)
}

#[tokio::test]
async fn golden_templates() {
    let (app, _web_dir) = app_com_ledger(LedgerStore::open_in_memory().unwrap());
    let (status, ct, cd, body) = drive(&app, "GET", "/api/btv/templates").await;
    btv_golden::check(
        "templates",
        vec![btv_golden::step(
            "lista os 12 modelos embutidos",
            "GET",
            "/api/btv/templates",
            None,
            status,
            ct,
            cd,
            body,
            &[],
        )],
    );
}

/// Entrada com timestamp FIXO — torna `prev_hash`/`entry_hash` determinísticos
/// (o hash da cadeia só depende do corpo canônico + hash anterior).
fn entrada(kind: &str, actor: &str, payload: serde_json::Value, ts: &str) -> LedgerEntry {
    LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: kind.into(),
        actor: actor.into(),
        payload,
        r#override: None,
        fake_marker: None,
        ts: ts.into(),
        tenant: None,
    }
}

#[tokio::test]
async fn golden_ledger() {
    let mut ledger = LedgerStore::open_in_memory().unwrap();
    ledger
        .append(entrada(
            "session.start",
            "humano",
            serde_json::json!({"task": "revisar contrato"}),
            "2026-07-08T10:00:00Z",
        ))
        .unwrap();
    ledger
        .append(entrada(
            "tool.run",
            "web:btv",
            serde_json::json!({"tool": "edit", "exit_code": 0}),
            "2026-07-08T10:01:00Z",
        ))
        .unwrap();
    ledger
        .append(entrada(
            "session.end",
            "humano",
            serde_json::json!({"ok": true}),
            "2026-07-08T10:02:00Z",
        ))
        .unwrap();
    let (app, _web_dir) = app_com_ledger(ledger);

    let (s1, ct1, cd1, b1) = drive(&app, "GET", "/api/ledger").await;
    let (s2, ct2, cd2, b2) = drive(&app, "GET", "/api/ledger?actor=humano&limit=2").await;

    btv_golden::check(
        "ledger",
        vec![
            btv_golden::step(
                "tudo, mais recente primeiro",
                "GET",
                "/api/ledger",
                None,
                s1,
                ct1,
                cd1,
                b1,
                &[],
            ),
            btv_golden::step(
                "filtro por actor combinado com limit (dentro do SQL)",
                "GET",
                "/api/ledger?actor=humano&limit=2",
                None,
                s2,
                ct2,
                cd2,
                b2,
                &[],
            ),
        ],
    );
}
