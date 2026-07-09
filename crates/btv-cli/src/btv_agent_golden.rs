//! Golden tests dos fluxos HTTP do produto BuildToValue (`btv_agent::router`)
//! — Trilha T1 do plano DDD multitenant. Ver `btv-golden` para a mecânica
//! (fixtures gravadas da resposta real, igualdade profunda, regravação via
//! `BTV_UPDATE_GOLDEN=1`).
//!
//! Os fluxos deste módulo dirigem o router do produto DIRETO (pré-guarda de
//! `Origin` — e requests sem o header passariam de qualquer forma, ver ADR
//! 0015), com `BtvStore`/`LedgerStore` em memória e semeadura pelo MESMO
//! caminho de produção (`insert_run`/`insert_deliverable`), não por SQL de
//! teste. Erros de ativação retornam ANTES de tocar o `SquadPool` — nada
//! aqui precisa de `uv`/Python (a ativação real, com orquestrador, está em
//! `golden_ativacao_gate_e_ajuste_reais`).

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use btv_store::{BtvStore, LedgerStore};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

use crate::squad_agent::{default_hub, default_squad_pool};

fn app_produto(
    dir: &std::path::Path,
    store: Arc<Mutex<BtvStore>>,
    ledger: Arc<Mutex<LedgerStore>>,
) -> Router {
    crate::btv_agent::router(default_hub(), default_squad_pool(dir), ledger, store)
}

/// Dispara a requisição in-process e captura o contrato observável
/// (status + content-type + content-disposition + corpo).
async fn drive(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (u16, Option<String>, Option<String>, serde_json::Value) {
    let builder = Request::builder().method(method).uri(uri);
    let req = match &body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(json).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
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
    let parsed = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::json!({ "text": String::from_utf8_lossy(&bytes) }))
    };
    (status, content_type, content_disposition, parsed)
}

/// Passo de fluxo com corpo JSON — encurta a assinatura verbosa.
async fn golden_step(
    app: &Router,
    name: &str,
    method: &str,
    uri: &str,
    recorded_path: &str,
    body: Option<serde_json::Value>,
    volatiles: &[btv_golden::Volatile],
) -> btv_golden::GoldenStep {
    let (status, ct, cd, resp) = drive(app, method, uri, body.clone()).await;
    btv_golden::step(
        name,
        method,
        recorded_path,
        body,
        status,
        ct,
        cd,
        resp,
        volatiles,
    )
}

#[tokio::test]
async fn golden_deliverables() {
    let dir = tempfile::tempdir().unwrap();
    // Semeadura pelo caminho de produção: run concluído com 1 gate aprovado
    // e uma entrega apontando para um arquivo REAL no tempdir.
    let artigo = dir.path().join("artigo.md");
    std::fs::write(&artigo, "# Artigo\n\nLogística verde no Brasil.\n").unwrap();
    let store = BtvStore::open_in_memory().unwrap();
    let run_id = store
        .insert_run(
            "sq1",
            "editorial",
            "v1.4",
            "Newsletter de julho",
            r#"[{"label":"Qual é a pauta ou tema?","resposta":"logística verde"}]"#,
            r#"["Pauteiro","Redator","Revisor de estilo"]"#,
            "2026-07-08T10:00:00Z",
        )
        .unwrap();
    store
        .increment_gates("sq1", "2026-07-08T10:05:00Z")
        .unwrap();
    store
        .set_status("sq1", "concluida", "2026-07-08T10:10:00Z")
        .unwrap();
    store
        .insert_deliverable(
            run_id,
            "sq1",
            "editorial",
            "artigo.md",
            artigo.to_str().unwrap(),
            "MD",
            "v1",
            "Pauteiro → Redator → Revisor de estilo · 1 gate(s) aprovado(s) por você",
            "2026-07-08T10:10:00Z",
        )
        .unwrap();

    let app = app_produto(
        dir.path(),
        Arc::new(Mutex::new(store)),
        Arc::new(Mutex::new(LedgerStore::open_in_memory().unwrap())),
    );

    // Único volátil do fluxo: o `path` da entrega aponta para o tempdir.
    let steps = vec![
        golden_step(
            &app,
            "biblioteca lista a entrega com trilha de procedência",
            "GET",
            "/api/btv/deliverables",
            "/api/btv/deliverables",
            None,
            &[btv_golden::vstr("/*/path")],
        )
        .await,
        golden_step(
            &app,
            "download de formato texto serve o conteúdo como está",
            "GET",
            "/api/btv/deliverables/1/download",
            "/api/btv/deliverables/1/download",
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "entrega inexistente responde 404 honesto",
            "GET",
            "/api/btv/deliverables/999/download",
            "/api/btv/deliverables/999/download",
            None,
            &[],
        )
        .await,
    ];
    btv_golden::check("deliverables", steps);
}

#[tokio::test]
async fn golden_personas() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_produto(
        dir.path(),
        Arc::new(Mutex::new(BtvStore::open_in_memory().unwrap())),
        Arc::new(Mutex::new(LedgerStore::open_in_memory().unwrap())),
    );

    let base = "/api/btv/personas/editorial";
    let steps = vec![
        golden_step(
            &app,
            "estado inicial: 4 papéis com prompt padrão, sem próprias",
            "GET",
            base,
            base,
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "override do prompt do Redator",
            "PUT",
            &format!("{base}/Redator"),
            "/api/btv/personas/editorial/Redator",
            Some(serde_json::json!({"prompt": "Você escreve com humor seco e frases curtas."})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "override visível como editado, padrão preservado",
            "GET",
            base,
            base,
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "cria persona própria",
            "POST",
            &format!("{base}/custom"),
            "/api/btv/personas/editorial/custom",
            Some(serde_json::json!({"nome": "Ghostwriter", "prompt": "Escreva como ghostwriter da marca."})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "atualiza persona própria",
            "PUT",
            &format!("{base}/custom/1"),
            "/api/btv/personas/editorial/custom/1",
            Some(serde_json::json!({"nome": "Ghostwriter Sênior", "prompt": "Escreva como ghostwriter sênior."})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "própria atualizada aparece na listagem",
            "GET",
            base,
            base,
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "remove persona própria",
            "DELETE",
            &format!("{base}/custom/1"),
            "/api/btv/personas/editorial/custom/1",
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "restaura o Redator ao padrão",
            "DELETE",
            &format!("{base}/Redator"),
            "/api/btv/personas/editorial/Redator",
            None,
            &[],
        )
        .await,
        golden_step(
            &app,
            "clear restaura todos e a listagem volta ao estado inicial",
            "DELETE",
            base,
            base,
            None,
            &[],
        )
        .await,
        golden_step(&app, "estado final igual ao inicial", "GET", base, base, None, &[]).await,
    ];
    btv_golden::check("personas", steps);
}

/// Contratos de ERRO da ativação/gates — todos retornam antes de tocar o
/// `SquadPool`, então rodam sem `uv`/Python. As mensagens em português são
/// parte do contrato (consumidas pelo frontend): mudá-las exige regravação
/// consciente da fixture.
#[tokio::test]
async fn golden_squad_activation_errors() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_produto(
        dir.path(),
        Arc::new(Mutex::new(BtvStore::open_in_memory().unwrap())),
        Arc::new(Mutex::new(LedgerStore::open_in_memory().unwrap())),
    );

    let steps = vec![
        golden_step(
            &app,
            "template desconhecido",
            "POST",
            "/api/btv/squads",
            "/api/btv/squads",
            Some(serde_json::json!({"template_id": "inexistente"})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "todos os papéis desligados",
            "POST",
            "/api/btv/squads",
            "/api/btv/squads",
            Some(serde_json::json!({"template_id": "editorial", "papeis_off": [0, 1, 2, 3]})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "gate sem HITL pendente",
            "POST",
            "/api/btv/squads/sq999/gate",
            "/api/btv/squads/sq999/gate",
            Some(serde_json::json!({})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "ajuste com instrução vazia",
            "POST",
            "/api/btv/squads/sq999/ajuste",
            "/api/btv/squads/sq999/ajuste",
            Some(serde_json::json!({"instrucao": "   "})),
            &[],
        )
        .await,
        golden_step(
            &app,
            "ajuste em tarefa inexistente",
            "POST",
            "/api/btv/squads/sq999/ajuste",
            "/api/btv/squads/sq999/ajuste",
            Some(serde_json::json!({"instrucao": "melhorar o tom"})),
            &[],
        )
        .await,
    ];
    btv_golden::check("squad_activation_errors", steps);
}
