//! BuildToValue — ativação de squad pela galeria/wizard (U1/U2 → U3) e
//! gates com auditoria (plano BTV, Onda 3).
//!
//! Uma squad ativada aqui roda o MESMO motor do `POST /api/squad/run`
//! (`squad_agent::start_squad_task` — orquestrador Python real, HITL real,
//! RunTool real): este módulo só monta a descrição da tarefa a partir do
//! briefing + personas do modelo, registra a ativação no ledger (com hash
//! dos prompts efetivos — procedência de U7↔U4) e persiste o run
//! (`forge_store::BtvStore`) para U6/U3.
//!
//! Semântica dos gates (mapeamento honesto sobre o HITL real):
//! - **Aprovar** = `resolve_hitl(allow=true)` + `btv.gate_approved` no ledger.
//! - **Pedir ajuste** = a instrução vira orientação REAL do cockpit
//!   (`push_user_message` → injetada no próximo `Generate` do agente ativo,
//!   ver `inject_cockpit_context`) e o gate é aprovado com ela em contexto +
//!   `btv.adjust_requested` no ledger. Negar o HITL não serve para "ajustar":
//!   o orquestrador aborta a tarefa com "Plan rejected" (`orchestrator.py`) —
//!   seria encerrar, não refazer.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use forge_schemas::squad_template::SquadTemplate;
use forge_store::{BtvStore, LedgerStore};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::squad_agent::{start_squad_task, SquadAgentState, SquadHub};
use crate::web_agent::ErrorBody;

/// Prompts padrão por papel — os 4 arquétipos do handoff (§6 U7): abre o
/// trabalho / produz / revisa / valida. Papéis além do 4º herdam o último
/// arquétipo (mesma regra do protótipo). Overrides por modelo+papel entram
/// na Onda 4 (U7) por cima destes defaults.
pub(crate) fn prompt_padrao(indice: usize, papel: &str, template_nome: &str) -> String {
    let bases = [
        format!("Você é {papel} da squad {template_nome}. Abra o trabalho: interprete o briefing, estruture o plano e defina critérios de pronto. Fale em português claro, sem jargão. Entregue um plano curto e verificável antes de passar adiante."),
        format!("Você é {papel} da squad {template_nome}. Produza a primeira versão completa a partir do plano aprovado, respeitando tom, público e formato do briefing. Sinalize incertezas em vez de inventar."),
        format!("Você é {papel} da squad {template_nome}. Revise qualidade, consistência e clareza. Não reescreva do zero: melhore o que existe e registre o que mudou e por quê."),
        format!("Você é {papel} da squad {template_nome}. Valide fatos, requisitos e conformidade antes da entrega. Se algo não puder ser verificado, bloqueie e explique — nunca aprove com pendência."),
    ];
    bases[indice.min(3)].clone()
}

#[derive(Deserialize)]
pub(crate) struct RespostaBriefing {
    label: String,
    resposta: String,
}

#[derive(Deserialize)]
pub(crate) struct AtivarSquadBody {
    template_id: String,
    #[serde(default)]
    nome: Option<String>,
    #[serde(default)]
    briefing: Vec<RespostaBriefing>,
    /// Links/nomes de arquivos anexados no passo 1 do wizard.
    #[serde(default)]
    refs: Vec<String>,
    /// Índices dos papéis DESLIGADOS no passo 2 ("você mesmo fará").
    #[serde(default)]
    papeis_off: Vec<usize>,
}

#[derive(Serialize)]
struct AtivarSquadResponse {
    task_id: String,
    run_id: i64,
}

#[derive(Clone)]
pub struct BtvAgentState {
    squad: SquadAgentState,
    ledger: Arc<Mutex<LedgerStore>>,
    store: Arc<Mutex<BtvStore>>,
}

/// Monta a descrição REAL da tarefa que o orquestrador recebe: briefing na
/// linguagem da área + referências + equipe com os prompts efetivos de cada
/// papel + entregas/gates esperados.
fn montar_descricao(
    template: &SquadTemplate,
    body: &AtivarSquadBody,
    papeis_ativos: &[(usize, &str)],
) -> String {
    let mut out = format!(
        "Squad \"{}\" (modelo {} {}) ativada pelo BuildToValue.\n\n## Briefing\n",
        body.nome.clone().unwrap_or_else(|| template.nome.clone()),
        template.nome,
        template.versao,
    );
    for r in &body.briefing {
        if !r.resposta.trim().is_empty() {
            out.push_str(&format!("- {}: {}\n", r.label, r.resposta.trim()));
        }
    }
    if !body.refs.is_empty() {
        out.push_str("\n## Referências e materiais\n");
        for r in &body.refs {
            out.push_str(&format!("- {r}\n"));
        }
    }
    out.push_str("\n## Equipe (papéis e responsabilidades)\n");
    for (i, papel) in papeis_ativos {
        out.push_str(&format!(
            "- {}: {}\n",
            papel,
            prompt_padrao(*i, papel, &template.nome)
        ));
    }
    out.push_str("\n## Entrega esperada\n");
    let formatos: Vec<&str> = template.formatos.iter().map(|f| f.nome.as_str()).collect();
    out.push_str(&format!(
        "Artefato final da área nos formatos: {}. Grave o resultado como arquivo real no workspace.\n",
        formatos.join(", ")
    ));
    if !template.gates.is_empty() {
        out.push_str("\n## Gates humanos\n");
        for g in &template.gates {
            out.push_str(&format!("- ✋ {g}\n"));
        }
    }
    out
}

fn append_ledger(
    ledger: &Arc<Mutex<LedgerStore>>,
    kind: &str,
    payload: serde_json::Value,
) -> Result<u64, String> {
    let entry = forge_schemas::ledger::LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: kind.into(),
        actor: "web:btv".into(),
        payload,
        r#override: None,
        fake_marker: None,
        ts: crate::session::now_rfc3339(),
    };
    let mut guard = ledger.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .append(entry)
        .map(|e| e.seq)
        .map_err(|e| e.to_string())
}

/// `POST /api/btv/squads` — ativa uma squad de verdade a partir do wizard.
async fn ativar_squad_handler(
    State(state): State<BtvAgentState>,
    Json(body): Json<AtivarSquadBody>,
) -> Response {
    let Some(template) = forge_server::btv::builtin_templates()
        .iter()
        .find(|t| t.id == body.template_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "unknown_template",
                format!("modelo de squad desconhecido: {}", body.template_id),
            )),
        )
            .into_response();
    };

    let papeis_ativos: Vec<(usize, &str)> = template
        .papeis
        .iter()
        .enumerate()
        .filter(|(i, _)| !body.papeis_off.contains(i))
        .map(|(i, p)| (i, p.as_str()))
        .collect();
    if papeis_ativos.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new(
                "no_roles",
                "todos os papéis foram desligados — a squad precisa de ao menos um",
            )),
        )
            .into_response();
    }

    let descricao = montar_descricao(template, &body, &papeis_ativos);

    // Procedência dos prompts (aprovação obs. 5): hash de cada prompt
    // efetivo em vigor na ativação — fecha U7 (personas) ↔ U4 (trilha).
    let prompt_hashes: Vec<serde_json::Value> = papeis_ativos
        .iter()
        .map(|(i, papel)| {
            serde_json::json!({
                "papel": papel,
                "prompt_sha256": forge_schemas::sha256_hex(
                    &prompt_padrao(*i, papel, &template.nome)
                ),
            })
        })
        .collect();

    let task_id = match start_squad_task(&state.squad, descricao) {
        Ok(id) => id,
        Err(resp) => return *resp,
    };

    let nome = body.nome.clone().unwrap_or_else(|| template.nome.clone());
    let briefing_json = serde_json::to_string(
        &body
            .briefing
            .iter()
            .map(|r| serde_json::json!({"label": r.label, "resposta": r.resposta}))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    let papeis_json =
        serde_json::to_string(&papeis_ativos.iter().map(|(_, p)| *p).collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".into());

    let run_id = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        match store.insert_run(
            &task_id,
            &template.id,
            &template.versao,
            &nome,
            &briefing_json,
            &papeis_json,
            &crate::session::now_rfc3339(),
        ) {
            Ok(id) => id,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("store_error", e.to_string())),
                )
                    .into_response()
            }
        }
    };

    if let Err(e) = append_ledger(
        &state.ledger,
        "btv.squad_activated",
        serde_json::json!({
            "task_id": task_id,
            "run_id": run_id,
            "template_id": template.id,
            "template_versao": template.versao,
            "nome": nome,
            "papeis": papeis_ativos.iter().map(|(_, p)| *p).collect::<Vec<_>>(),
            "prompt_hashes": prompt_hashes,
            "refs": body.refs,
        }),
    ) {
        // Ledger indisponível é falha de auditoria — reporta, não esconde
        // (a squad já está rodando; o cliente decide o que fazer).
        eprintln!("btv: falha ao registrar ativação no ledger: {e}");
    }

    // Watcher: quando o stream da tarefa terminar, transiciona o status do
    // run (concluida/erro/encerrada) — U6 mostra estado real, não congelado.
    spawn_status_watcher(state.clone(), task_id.clone());

    (
        StatusCode::ACCEPTED,
        Json(AtivarSquadResponse { task_id, run_id }),
    )
        .into_response()
}

/// Acompanha a tarefa até o fim do stream e grava o status final no store.
fn spawn_status_watcher(state: BtvAgentState, task_id: String) {
    tokio::spawn(async move {
        let (_snapshot, rx) = state.squad.hub.subscribe(&task_id);
        if let Some(mut rx) = rx {
            // Drena até o canal fechar (fim da tarefa — `finish_task` dropa o
            // Sender). Lagged é ok: só nos importa o fechamento.
            loop {
                match rx.recv().await {
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
        // Estado final honesto: kill-switch > erro no log > concluída.
        let status = if state.squad.hub.is_stopped(&task_id) {
            "encerrada"
        } else {
            let (log, _) = state.squad.hub.subscribe(&task_id);
            let teve_erro = log.iter().any(|e| {
                matches!(
                    &e.payload,
                    Some(forge_proto::squad::squad_event::Payload::Error(_))
                )
            });
            if teve_erro {
                "erro"
            } else {
                "concluida"
            }
        };
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        let _ = store.set_status(&task_id, status, &crate::session::now_rfc3339());
    });
}

/// `GET /api/btv/squads` — runs persistidos, mais recente primeiro (U6).
async fn list_runs_handler(State(state): State<BtvAgentState>) -> Response {
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.list_runs() {
        Ok(runs) => Json(runs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct GateBody {
    /// Nome da etapa/gate como exibido (vai para o ledger).
    #[serde(default)]
    etapa: Option<String>,
}

/// `POST /api/btv/squads/{task_id}/gate` — aprova o gate HITL pendente e
/// registra a aprovação no ledger (`btv.gate_approved`).
async fn aprovar_gate_handler(
    State(state): State<BtvAgentState>,
    Path(task_id): Path<String>,
    body: Option<Json<GateBody>>,
) -> Response {
    match state.squad.hub.resolve_hitl(&task_id, true) {
        Ok(()) => {
            let etapa = body.and_then(|b| b.0.etapa).unwrap_or_default();
            if let Err(e) = append_ledger(
                &state.ledger,
                "btv.gate_approved",
                serde_json::json!({ "task_id": task_id, "etapa": etapa }),
            ) {
                eprintln!("btv: falha ao registrar gate no ledger: {e}");
            }
            StatusCode::OK.into_response()
        }
        Err(()) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "hitl_not_found",
                "nenhum gate pendente para esta tarefa",
            )),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct AjusteBody {
    instrucao: String,
    #[serde(default)]
    etapa: Option<String>,
}

/// `POST /api/btv/squads/{task_id}/ajuste` — "Pedir ajuste" no gate: a
/// instrução vira orientação real do cockpit (injetada no próximo `Generate`
/// do agente ativo) e o gate é liberado com ela em contexto. Registrado no
/// ledger como `btv.adjust_requested`. Ver o comentário de módulo para o
/// porquê de não negar o HITL.
async fn pedir_ajuste_handler(
    State(state): State<BtvAgentState>,
    Path(task_id): Path<String>,
    Json(body): Json<AjusteBody>,
) -> Response {
    let instrucao = body.instrucao.trim().to_string();
    if instrucao.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody::new(
                "empty_instruction",
                "instrução de ajuste vazia",
            )),
        )
            .into_response();
    }
    // 1. Instrução entra na inbox (echo `ChatMessage(HUMAN)` + injeção real
    // no próximo Generate) ANTES de liberar o gate — o agente retoma já com
    // a orientação em contexto.
    if state
        .squad
        .hub
        .push_user_message(&task_id, format!("Ajuste solicitado no gate: {instrucao}"))
        .is_err()
    {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new(
                "task_not_found",
                "tarefa de squad inexistente ou já encerrada",
            )),
        )
            .into_response();
    }
    // 2. Libera o gate pendente, se houver (o ajuste também pode ser pedido
    // fora de um gate — aí é só orientação de cockpit).
    let havia_gate = state.squad.hub.resolve_hitl(&task_id, true).is_ok();
    if let Err(e) = append_ledger(
        &state.ledger,
        "btv.adjust_requested",
        serde_json::json!({
            "task_id": task_id,
            "etapa": body.etapa.unwrap_or_default(),
            "instrucao": instrucao,
            "gate_liberado": havia_gate,
        }),
    ) {
        eprintln!("btv: falha ao registrar ajuste no ledger: {e}");
    }
    StatusCode::OK.into_response()
}

/// Router aditivo do BuildToValue — `.merge()`ado ao router do agente web
/// (mesma guarda de `Origin`/`Host` do `merged_router`).
pub fn router(
    hub: SquadHub,
    pool: Arc<forge_sidecar::SquadPool>,
    ledger: Arc<Mutex<LedgerStore>>,
    store: Arc<Mutex<BtvStore>>,
) -> Router {
    Router::new()
        .route(
            "/api/btv/squads",
            post(ativar_squad_handler).get(list_runs_handler),
        )
        .route("/api/btv/squads/{task_id}/gate", post(aprovar_gate_handler))
        .route(
            "/api/btv/squads/{task_id}/ajuste",
            post(pedir_ajuste_handler),
        )
        .with_state(BtvAgentState {
            squad: SquadAgentState { hub, pool },
            ledger,
            store,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_editorial() -> &'static SquadTemplate {
        forge_server::btv::builtin_templates()
            .iter()
            .find(|t| t.id == "editorial")
            .unwrap()
    }

    #[test]
    fn descricao_inclui_briefing_equipe_e_gates() {
        let body = AtivarSquadBody {
            template_id: "editorial".into(),
            nome: Some("Newsletter de julho".into()),
            briefing: vec![RespostaBriefing {
                label: "Qual é a pauta ou tema?".into(),
                resposta: "logística verde no Brasil".into(),
            }],
            refs: vec!["https://exemplo.com/estudo".into()],
            papeis_off: vec![3],
        };
        let template = template_editorial();
        let papeis: Vec<(usize, &str)> = template
            .papeis
            .iter()
            .enumerate()
            .filter(|(i, _)| !body.papeis_off.contains(i))
            .map(|(i, p)| (i, p.as_str()))
            .collect();
        let d = montar_descricao(template, &body, &papeis);
        assert!(d.contains("Newsletter de julho"));
        assert!(d.contains("logística verde no Brasil"));
        assert!(d.contains("https://exemplo.com/estudo"));
        // Papel desligado (índice 3, Fact-checker) fica fora da equipe.
        assert!(d.contains("Revisor de estilo:"));
        assert!(!d.contains("Fact-checker:"));
        assert!(d.contains("✋ Aprovar o rascunho antes da revisão"));
        // Prompt do arquétipo certo por índice.
        assert!(d.contains("Abra o trabalho"));
    }

    #[test]
    fn prompt_padrao_herda_ultimo_arquetipo_alem_do_quarto() {
        let p = prompt_padrao(7, "Papel Extra", "Squad X");
        assert!(p.contains("Valide fatos"));
    }
}
