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
use std::time::Duration;
use tower::ServiceExt;

use crate::squad_agent::{default_hub, default_squad_pool, SquadHub};

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
        .set_status(
            "sq1",
            btv_domain::ports::RunStatus::Concluida,
            "2026-07-08T10:10:00Z",
        )
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

// ── ativação real (orquestrador Python + backend roteirizado, sem API key) ──

fn uv_missing() -> bool {
    std::process::Command::new("uv")
        .arg("--version")
        .output()
        .is_err()
}

fn python_workspace_present() -> bool {
    crate::squad::locate_python_dir().is_some()
}

/// Espera o gate HITL da tarefa ficar pendente (evento `Hitl` no stream).
async fn wait_hitl(hub: &SquadHub, task_id: &str) {
    for _ in 0..900 {
        let (snapshot, _rx) = hub.subscribe(task_id);
        let pendente = snapshot.iter().any(|e| {
            matches!(
                &e.payload,
                Some(btv_proto::squad::squad_event::Payload::Hitl(_))
            )
        });
        if pendente {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("timeout esperando o gate HITL pendente de {task_id}");
}

/// Drena o stream da tarefa até o fim (canal fechado = tarefa concluída).
async fn wait_finish(hub: &SquadHub, task_id: &str) {
    let (_snapshot, rx) = hub.subscribe(task_id);
    let Some(mut rx) = rx else { return };
    let drenar = async {
        loop {
            match rx.recv().await {
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(300), drenar)
        .await
        .unwrap_or_else(|_| panic!("timeout esperando a tarefa {task_id} terminar"));
}

/// Passo capturado via HTTP real (reqwest) — a ativação sobe um servidor de
/// porta efêmera porque o fluxo atravessa processos (Rust ⇄ Python via UDS).
async fn reqwest_step(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
    volatiles: &[btv_golden::Volatile],
) -> btv_golden::GoldenStep {
    let url = format!("{base}{path}");
    let builder = match method {
        "POST" => client.post(&url),
        "GET" => client.get(&url),
        other => panic!("método inesperado: {other}"),
    };
    let builder = match &body {
        Some(json) => builder.json(json),
        None => builder,
    };
    let resp = builder.send().await.unwrap();
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap_or_default().to_string());
    let bytes = resp.bytes().await.unwrap();
    let parsed = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::json!({ "text": String::from_utf8_lossy(&bytes) }))
    };
    btv_golden::step(
        name,
        method,
        path,
        body,
        status,
        content_type,
        None,
        parsed,
        volatiles,
    )
}

/// T1 — o fluxo crítico inteiro, REAL: ativação pela galeria roda o motor do
/// squad de verdade (orquestrador Python via `SquadPool`, backend roteirizado
/// `BTV_SCRIPTED=1` — determinístico e sem API key), o consenso fraco pede
/// gate, a aprovação e o "pedir ajuste" passam pelos handlers do produto.
/// Contratos congelados na fixture; efeitos colaterais (ledger, contador de
/// gates) checados FORA do golden — prova que o fluxo executou, não só que o
/// HTTP respondeu. Pula (com aviso) sem `uv`/workspace Python — o job `rust`
/// do CI tem ambos, o caminho real roda lá.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn golden_squad_activation() {
    if uv_missing() || !python_workspace_present() {
        eprintln!("uv/workspace Python ausente — pulando golden de ativação real");
        return;
    }
    let _guard = crate::test_support::lock_cwd().await;
    std::env::set_var("BTV_SCRIPTED", "1");
    let dir = tempfile::tempdir().unwrap();
    let orig_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let hub = default_hub();
    let store = Arc::new(Mutex::new(BtvStore::open_in_memory().unwrap()));
    let ledger = Arc::new(Mutex::new(LedgerStore::open_in_memory().unwrap()));
    let app = crate::btv_agent::router(
        hub.clone(),
        default_squad_pool(dir.path()),
        ledger.clone(),
        store.clone(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::new();

    let ativacao = serde_json::json!({
        "template_id": "editorial",
        "nome": "Newsletter de julho",
        "briefing": [{"label": "Qual é a pauta ou tema?", "resposta": "logística verde no Brasil"}],
        "refs": [],
        "papeis_off": [3],
    });

    // Run A: ativa → espera o gate REAL ficar pendente → aprova.
    let passo_ativa_a = reqwest_step(
        &client,
        &base,
        "ativação roda o motor real e responde 202",
        "POST",
        "/api/btv/squads",
        Some(ativacao.clone()),
        &[],
    )
    .await;
    wait_hitl(&hub, "sq1").await;
    let passo_gate = reqwest_step(
        &client,
        &base,
        "aprovar o gate HITL pendente",
        "POST",
        "/api/btv/squads/sq1/gate",
        Some(serde_json::json!({"etapa": "Aprovar o rascunho antes da revisão"})),
        &[],
    )
    .await;
    wait_finish(&hub, "sq1").await;

    // Run B: ativa de novo → no gate, pede AJUSTE (instrução vira orientação
    // real de cockpit e o gate é liberado com ela em contexto).
    let passo_ativa_b = reqwest_step(
        &client,
        &base,
        "segunda ativação (run B) para o fluxo de ajuste",
        "POST",
        "/api/btv/squads",
        Some(ativacao),
        &[],
    )
    .await;
    wait_hitl(&hub, "sq2").await;
    let passo_ajuste = reqwest_step(
        &client,
        &base,
        "pedir ajuste no gate injeta a instrução e libera",
        "POST",
        "/api/btv/squads/sq2/ajuste",
        Some(serde_json::json!({
            "instrucao": "use tom mais formal na abertura",
            "etapa": "Aprovar o rascunho antes da revisão",
        })),
        &[],
    )
    .await;
    wait_finish(&hub, "sq2").await;

    btv_golden::check(
        "squad_activation",
        vec![passo_ativa_a, passo_gate, passo_ativa_b, passo_ajuste],
    );

    // Anti-fake, fora do golden: o fluxo EXECUTOU — auditoria no ledger e
    // contador de gates persistido, não só respostas HTTP bem formadas.
    {
        let guard = ledger.lock().unwrap_or_else(|e| e.into_inner());
        let entradas = guard.recent(50, None).unwrap();
        let conta = |kind: &str| entradas.iter().filter(|e| e.kind == kind).count();
        assert_eq!(conta("btv.squad_activated"), 2);
        assert_eq!(conta("btv.gate_approved"), 1);
        assert_eq!(conta("btv.adjust_requested"), 1);
        guard.verify_chain().unwrap();

        // C3.1 (primeiro estrangulado): a entrada de gate agora nasce da
        // PORTA de domínio — o payload carrega o contador pós-incremento e
        // o corpo hasheado carrega o tenant (ADR 0027; a mudança de wire
        // consciente anunciada desde o B3). As entradas dos emissores ainda
        // não estrangulados (ativação/ajuste) seguem SEM tenant — os dois
        // regimes convivem na mesma cadeia até as ondas seguintes.
        let gate = entradas
            .iter()
            .find(|e| e.kind == "btv.gate_approved")
            .expect("entrada de gate existe");
        assert_eq!(gate.actor, "web:btv", "mesmo actor do emissor legado");
        assert_eq!(gate.payload["etapa"], "Aprovar o rascunho antes da revisão");
        assert_eq!(
            gate.payload["gates_aprovados"], 1,
            "o payload carrega o contador pós-incremento (variante do G1)"
        );
        assert_eq!(
            gate.tenant,
            Some(btv_domain::TenantId::LOCAL),
            "entrada nova carrega o tenant no corpo hasheado"
        );
        let ativacao = entradas
            .iter()
            .find(|e| e.kind == "btv.squad_activated")
            .expect("entrada de ativação existe");
        assert_eq!(
            ativacao.tenant, None,
            "emissor não estrangulado: sem tenant"
        );
    }
    {
        let guard = store.lock().unwrap_or_else(|e| e.into_inner());
        let run_a = guard.get_run_by_task("sq1").unwrap().unwrap();
        assert_eq!(run_a.gates_aprovados, 1, "gate aprovado ficou no run A");
        assert!(guard.get_run_by_task("sq2").unwrap().is_some());
    }

    std::env::set_current_dir(orig_cwd).unwrap();
}
