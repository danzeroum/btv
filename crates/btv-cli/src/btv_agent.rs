//! BuildToValue — ativação de squad pela galeria/wizard (U1/U2 → U3) e
//! gates com auditoria (plano BTV, Onda 3).
//!
//! Uma squad ativada aqui roda o MESMO motor do `POST /api/squad/run`
//! (`squad_agent::start_squad_task` — orquestrador Python real, HITL real,
//! RunTool real): este módulo só monta a descrição da tarefa a partir do
//! briefing + personas do modelo, registra a ativação no ledger (com hash
//! dos prompts efetivos — procedência de U7↔U4) e persiste o run
//! (`btv_store::BtvStore`) para U6/U3.
//!
//! Semântica dos gates (mapeamento honesto sobre o HITL real):
//! - **Aprovar** = `resolve_hitl(allow=true)` + `btv.gate_approved` no ledger.
//! - **Pedir ajuste** = a instrução vira orientação REAL do cockpit
//!   (`push_user_message` → injetada no próximo `Generate` do agente ativo,
//!   ver `inject_cockpit_context`) e o gate é aprovado com ela em contexto +
//!   `btv.adjust_requested` no ledger. Negar o HITL não serve para "ajustar":
//!   o orquestrador aborta a tarefa com "Plan rejected" (`orchestrator.py`) —
//!   seria encerrar, não refazer.

use crate::tenant_extractor::{Tenant, TenantResolucao};
use axum::extract::{FromRef, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use btv_schemas::squad_template::SquadTemplate;
use btv_store::{BtvStore, LedgerStore};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::squad_agent::{start_squad_task, SquadAgentState, SquadHub};
use crate::web_agent::ErrorBody;

/// Prompts padrão por papel — os 4 arquétipos do handoff (§6 U7): abre o
/// trabalho / produz / revisa / valida. Papéis além do 4º herdam o último
/// arquétipo (mesma regra do protótipo). O prompt EFETIVO de uma ativação é
/// o override persistido (U7) quando existir, senão este padrão — e é o
/// efetivo que entra na descrição da tarefa e no hash de procedência.
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
    /// E1s.3: o resolver de sessões que o extractor de tenant puxa via
    /// `FromRef`. None no modo local (o dashboard local-first); Some com o
    /// `PgStore` no modo saas (a borda resolve o token). Clonável (Arc dentro).
    tenant_resolver: TenantResolucao,
}

impl FromRef<BtvAgentState> for TenantResolucao {
    fn from_ref(state: &BtvAgentState) -> Self {
        state.tenant_resolver.clone()
    }
}

/// Monta a descrição REAL da tarefa que o orquestrador recebe: briefing na
/// linguagem da área + referências + equipe com os prompts efetivos de cada
/// papel + entregas/gates esperados.
/// Função da persona no pipeline por posição do papel na esteira (Fase 1):
/// Planejamento[0]→plan, Produção[1]→produce, Revisão[2]→review,
/// Validação[3]→validate; papéis extras caem em "produce". O "ops/deploy" fica
/// FORA do roster do produto — só faz sentido em software (Fase 2 torna isso
/// declarado por domínio em vez de posicional).
fn funcao_por_indice(i: usize) -> &'static str {
    match i {
        0 => "plan",
        1 => "produce",
        2 => "review",
        3 => "validate",
        _ => "produce",
    }
}

fn montar_descricao(
    template: &SquadTemplate,
    body: &AtivarSquadBody,
    papeis_ativos: &[(usize, &str)],
    prompt_efetivo: &dyn Fn(usize, &str) -> String,
    proprias: &[btv_store::CustomPersona],
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
        out.push_str(&format!("- {}: {}\n", papel, prompt_efetivo(*i, papel)));
    }
    // Personas próprias (U7, Fase 3): criadas do zero pelo usuário no frontend,
    // entram na equipe como contribuintes de PRODUÇÃO com o prompt delas — não
    // são mais só linhas mortas no banco.
    for cp in proprias {
        out.push_str(&format!("- {} (persona própria): {}\n", cp.nome, cp.prompt));
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
    let entry = btv_schemas::ledger::LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: kind.into(),
        actor: "web:btv".into(),
        payload,
        r#override: None,
        fake_marker: None,
        ts: crate::session::now_rfc3339(),
        // Emissor legado dos kinds `btv.*` (sem contexto): cadeia LOCAL,
        // corpo idêntico — C3 troca isto pela porta `LedgerRepository`.
        tenant: None,
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
    Tenant(ctx): Tenant,
    Json(body): Json<AtivarSquadBody>,
) -> Response {
    let Some(template) = btv_schemas::squad_template::builtin_templates()
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

    // Overrides de persona (U7) em vigor NESTA ativação — o prompt efetivo
    // (override ?? padrão) entra na descrição real e no hash de procedência.
    // Personas próprias (Fase 3): criadas do zero pelo usuário, entram na squad
    // como agentes de produção de primeira classe (com o prompt delas), não só
    // como texto — antes eram ignoradas na ativação.
    let (overrides, proprias): (
        std::collections::HashMap<String, String>,
        Vec<btv_store::CustomPersona>,
    ) = {
        // C3.2: a ativação também LÊ personas — pela porta, com o contexto da
        // borda (o handler já o tem desde a E1s.3). Fecha o último toque cru de
        // persona no arquivo; wire imóvel (a descrição/procedência não muda —
        // golden de ativação é o juiz).
        use btv_domain::ports::PersonaRepository;
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        let overrides = store
            .list_overrides(&ctx, &template.id)
            .unwrap_or_default()
            .into_iter()
            .map(|o| (o.papel, o.prompt))
            .collect();
        let proprias = store.list_custom(&ctx, &template.id).unwrap_or_default();
        (overrides, proprias)
    };
    let template_nome = template.nome.clone();
    let prompt_efetivo = move |i: usize, papel: &str| -> String {
        overrides
            .get(papel)
            .cloned()
            .unwrap_or_else(|| prompt_padrao(i, papel, &template_nome))
    };

    let descricao = montar_descricao(template, &body, &papeis_ativos, &prompt_efetivo, &proprias);

    // Procedência dos prompts (aprovação obs. 5): hash de cada prompt
    // efetivo em vigor na ativação — fecha U7 (personas) ↔ U4 (trilha).
    // Inclui as personas próprias (Fase 3): o prompt delas de fato roda, então
    // o hash entra na trilha como o dos papéis de template.
    let mut prompt_hashes: Vec<btv_domain::ports::PromptHash> = papeis_ativos
        .iter()
        .map(|(i, papel)| btv_domain::ports::PromptHash {
            role: papel.to_string(),
            prompt_sha256: btv_schemas::sha256_hex(&prompt_efetivo(*i, papel)),
            custom: false,
        })
        .collect();
    for cp in &proprias {
        prompt_hashes.push(btv_domain::ports::PromptHash {
            role: cp.nome.clone(),
            prompt_sha256: btv_schemas::sha256_hex(&cp.prompt),
            custom: true,
        });
    }

    // Roster de personas (Fase 1): cada papel ativo vira uma PersonaSpec cujo
    // `prompt` (override de U7 ?? padrão do arquétipo) o Python usa como system
    // prompt do agente do estágio. Assim editar a persona no frontend muda quem
    // trabalha e como — não mais só texto no briefing. `funcao`/`ordem` guiam o
    // encaixe no pipeline (personas próprias e roster por domínio vêm nas
    // fases 2/3).
    let mut roster: Vec<btv_proto::squad::PersonaSpec> = papeis_ativos
        .iter()
        .map(|(i, papel)| btv_proto::squad::PersonaSpec {
            papel: papel.to_string(),
            prompt: prompt_efetivo(*i, papel),
            funcao: funcao_por_indice(*i).to_string(),
            ordem: *i as u32,
            custom: false,
        })
        .collect();
    // Personas próprias (Fase 3): entram como contribuintes de PRODUÇÃO
    // (`funcao="produce"`) — o motor tem um elenco fixo, então o lado Python
    // combina os prompts das personas que caem no mesmo agente (não descarta
    // silenciosamente). `ordem` continua após os papéis do template.
    let base_ordem = template.papeis.len() as u32;
    for (j, cp) in proprias.iter().enumerate() {
        roster.push(btv_proto::squad::PersonaSpec {
            papel: cp.nome.clone(),
            prompt: cp.prompt.clone(),
            funcao: "produce".to_string(),
            ordem: base_ordem + j as u32,
            custom: true,
        });
    }

    // A galeria BuildToValue não tem seletor de modelo por ativação (o produto
    // usa o default do deploy, `BTV_SQUAD_MODEL`); `None` = herda esse default.
    let task_id = match start_squad_task(&state.squad, descricao, None, roster) {
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
    // C3.1 endpoint 2: factory do agregado + portas no lugar de insert_run
    // cru + JSON solto. `task_id`/`ts` são INSUMOS (o hub sequenciou acima; o
    // relógio é do chamador); o evento DERIVA do run PERSISTIDO — o id é
    // numerado pelo storage no save, então salvamos, relemos e só então o
    // agregado emite (fail-closed em id=0). Contexto de fonte LOCAL fixa,
    // como no gate — a E1s troca a fonte. Hub/SSE/pool ficaram acima: o
    // serviço orquestra domínio+portas, não streaming.
    let roles: Vec<String> = papeis_ativos.iter().map(|(_, p)| p.to_string()).collect();
    let ts = crate::session::now_rfc3339();
    let run = match btv_domain::Run::activate(
        &ctx,
        match btv_domain::TaskId::parse(&task_id) {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("task_id_error", e.to_string())),
                )
                    .into_response()
            }
        },
        template.id.clone(),
        template.versao.clone(),
        nome.clone(),
        briefing_json.clone(),
        &roles,
        ts.clone(),
    ) {
        Ok(run) => run,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorBody::new("activate_error", e.to_string())),
            )
                .into_response()
        }
    };
    let (run_id, evento) = {
        use btv_domain::ports::RunRepository;
        let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = store.save(&ctx, &run) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody::new("store_error", e.to_string())),
            )
                .into_response();
        }
        // Relê para obter o id que o storage numerou — o evento deriva do
        // agregado persistido, nunca de um placeholder.
        let salvo = match store.get(&ctx, &task_id) {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("store_error", "run sumiu após o save")),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("store_error", e.to_string())),
                )
                    .into_response()
            }
        };
        let facts = btv_domain::ports::ActivationFacts {
            custom_personas: proprias.iter().map(|c| c.nome.clone()).collect(),
            prompt_hashes,
            refs: body.refs.clone(),
        };
        match salvo.activation_event(&ctx, facts, ts) {
            Ok(evento) => (salvo.id, evento),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("event_error", e.to_string())),
                )
                    .into_response()
            }
        }
    };

    {
        use btv_domain::ports::LedgerRepository;
        let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
            // Ledger indisponível é falha de auditoria — reporta, não esconde
            // (a squad já está rodando; o cliente decide o que fazer).
            eprintln!("btv: falha ao registrar ativação no ledger: {e}");
        }
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
            btv_domain::ports::RunStatus::Encerrada
        } else {
            let (log, _) = state.squad.hub.subscribe(&task_id);
            let teve_erro = log.iter().any(|e| {
                matches!(
                    &e.payload,
                    Some(btv_proto::squad::squad_event::Payload::Error(_))
                )
            });
            if teve_erro {
                btv_domain::ports::RunStatus::Erro
            } else {
                btv_domain::ports::RunStatus::Concluida
            }
        };
        let now = crate::session::now_rfc3339();
        {
            let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            let _ = store.set_status(&task_id, status, &now);
        }
        // Entregas (U4): arquivos REAIS gravados pelas ferramentas da tarefa
        // (trilha `tool_runs` do hub) viram itens da Biblioteca, com trilha
        // de procedência (papéis do run + gates aprovados) e registro no
        // ledger. Só em conclusão limpa — run com erro/encerrado não
        // "entrega".
        if status == btv_domain::ports::RunStatus::Concluida {
            let escritas = arquivos_escritos(&state.squad.hub.tool_runs(&task_id));
            if !escritas.is_empty() {
                let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
                if let Ok(Some(run)) = store.get_run_by_task(&task_id) {
                    let papeis: Vec<String> =
                        serde_json::from_str(&run.papeis_json).unwrap_or_default();
                    let trilha = format!(
                        "{} · {} gate(s) aprovado(s) por você",
                        papeis.join(" → "),
                        run.gates_aprovados
                    );
                    for path in escritas {
                        let nome = std::path::Path::new(&path)
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.clone());
                        let formato = std::path::Path::new(&path)
                            .extension()
                            .map(|e| e.to_string_lossy().to_uppercase())
                            .unwrap_or_else(|| "TXT".into());
                        match store.insert_deliverable(
                            run.id,
                            &task_id,
                            &run.template_id,
                            &nome,
                            &path,
                            &formato,
                            "v1",
                            &trilha,
                            &now,
                        ) {
                            Ok(deliverable_id) => {
                                if let Err(e) = append_ledger(
                                    &state.ledger,
                                    "btv.export_generated",
                                    serde_json::json!({
                                        "task_id": task_id,
                                        "deliverable_id": deliverable_id,
                                        "nome": nome,
                                        "formato": formato,
                                        "trilha": trilha,
                                    }),
                                ) {
                                    eprintln!("btv: falha ao registrar entrega no ledger: {e}");
                                }
                            }
                            Err(e) => eprintln!("btv: falha ao registrar entrega: {e}"),
                        }
                    }
                }
            }
        }
    });
}

/// Caminhos de arquivo ESCRITOS pela tarefa — só ferramentas que gravam
/// (`edit`) com exit 0, deduplicados preservando a ordem (o mesmo arquivo
/// editado N vezes é UMA entrega). Função pura, testada isolada.
fn arquivos_escritos(runs: &[crate::squad_agent::ToolRunNote]) -> Vec<String> {
    let mut vistos = std::collections::HashSet::new();
    runs.iter()
        .filter(|r| r.tool == "edit" && r.exit_code == 0 && !r.scope.is_empty())
        .filter(|r| vistos.insert(r.scope.clone()))
        .map(|r| r.scope.clone())
        .collect()
}

/// `GET /api/btv/squads` — runs persistidos, mais recente primeiro (U6).
async fn list_runs_handler(State(state): State<BtvAgentState>, Tenant(ctx): Tenant) -> Response {
    // C3.1 endpoint 4: leitura pela PORTA (RunRepository::list) com contexto
    // de fonte LOCAL fixa — mesmo tipo de retorno (Run re-exportado como
    // BtvRun desde A2), mesma serialização, wire intacto (golden T1 é o
    // juiz). Nenhum evento: leitura não é fato auditável.
    use btv_domain::ports::RunRepository;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.list(&ctx) {
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
    Tenant(ctx): Tenant,
    body: Option<Json<GateBody>>,
) -> Response {
    match state.squad.hub.resolve_hitl(&task_id, true) {
        Ok(()) => {
            // C3.1 (primeiro estrangulado): agregado + portas no lugar de
            // increment_gates cru + JSON solto — `Run::approve_gate` valida a
            // transição, incrementa e RETORNA o evento; as portas gravam. O
            // contexto vem de fonte LOCAL FIXA (mesma decisão dos adapters
            // desde o B2) — a E1s troca a FONTE do contexto, não este
            // consumidor. Semântica preservada do legado: o gate do hub já
            // foi liberado acima, então falha de escrituração é reportada e
            // não desfaz a liberação (como o `let _`/eprintln de antes).
            let etapa = body.and_then(|b| b.0.etapa);
            let evento = {
                use btv_domain::ports::RunRepository;
                let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
                match store.get(&ctx, &task_id) {
                    Ok(Some(mut run)) => {
                        match run.approve_gate(&ctx, etapa, crate::session::now_rfc3339()) {
                            Ok(evento) => match store.save(&ctx, &run) {
                                Ok(()) => Some(evento),
                                Err(e) => {
                                    eprintln!(
                                        "btv: gate liberado, mas o run não pôde ser salvo: {e}"
                                    );
                                    None
                                }
                            },
                            Err(e) => {
                                eprintln!("btv: gate liberado, mas o agregado recusou: {e}");
                                None
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("btv: gate liberado sem run correspondente ({task_id})");
                        None
                    }
                    Err(e) => {
                        eprintln!("btv: gate liberado, mas o run não pôde ser lido: {e}");
                        None
                    }
                }
            };
            if let Some(evento) = evento {
                use btv_domain::ports::LedgerRepository;
                let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
                    eprintln!("btv: falha ao registrar gate no ledger: {e}");
                }
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
    Tenant(ctx): Tenant,
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
    // C3.1 endpoint 3 (o último emissor de escrita da onda): o fato vai pela
    // porta de domínio. O ajuste NÃO muta o Run (a liberação do gate é
    // infraestrutura do hub; o estado segue Ativa) — por isso não há método
    // de agregado aqui: o evento é fato da tarefa, construído no serviço e
    // gravado pela porta. Contexto de fonte LOCAL fixa, como nos outros dois.
    match btv_domain::TaskId::parse(&task_id) {
        Ok(task) => {
            use btv_domain::ports::LedgerRepository;
            let evento = btv_domain::ports::DomainEvent {
                tenant: ctx.tenant,
                actor: ctx.actor.clone(),
                ts: crate::session::now_rfc3339(),
                kind: btv_domain::ports::DomainEventKind::AdjustRequested {
                    task_id: task,
                    stage: body.etapa,
                    instruction: instrucao,
                    gate_released: havia_gate,
                },
            };
            let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
                eprintln!("btv: falha ao registrar ajuste no ledger: {e}");
            }
        }
        Err(e) => eprintln!("btv: ajuste sem registro no ledger — task_id inválido: {e}"),
    }
    StatusCode::OK.into_response()
}

/// `GET /api/btv/deliverables` — Biblioteca de entregas (U4).
async fn list_deliverables_handler(
    State(state): State<BtvAgentState>,
    Tenant(ctx): Tenant,
) -> Response {
    // C3.1 endpoint 5: leitura pela porta (RunRepository::list_deliverables),
    // ctx LOCAL fixo — mesmo tipo (Deliverable re-exportado desde A2), wire
    // intacto. Nenhum evento: leitura não é fato auditável.
    use btv_domain::ports::RunRepository;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match RunRepository::list_deliverables(&*store, &ctx) {
        Ok(list) => Json(list).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

/// `GET /api/btv/deliverables/{id}/download` — baixa a entrega. Formato de
/// texto: serve o conteúdo como está. Formato binário: converte o texto por
/// serialização determinística (DOCX/XLSX/PDF via `crate::convert`, sem
/// sandbox) ou serve markup como texto (SVG/MusicXML); formatos que exigem
/// renderização/mídia real (PNG, MIDI) respondem 422 honesto. Arquivo sumido
/// do disco responde 404.
async fn download_deliverable_handler(
    State(state): State<BtvAgentState>,
    Path(id): Path<i64>,
    Tenant(ctx): Tenant,
) -> Response {
    let deliverable = {
        // C3.1 endpoint 6 (fecha a onda): leitura pela porta, ctx LOCAL
        // fixo — id de outro tenant é indistinguível de inexistente
        // (isolamento na leitura, contrato da suíte desde o B2). A leitura
        // do ARQUIVO em disco continua abaixo, fora da porta (é filesystem,
        // não estado do agregado).
        use btv_domain::ports::RunRepository;
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        match RunRepository::get_deliverable(&*store, &ctx, id) {
            Ok(Some(d)) => d,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorBody::new("not_found", "entrega inexistente")),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody::new("store_error", e.to_string())),
                )
                    .into_response()
            }
        }
    };
    let binario = btv_schemas::squad_template::builtin_templates()
        .iter()
        .find(|t| t.id == deliverable.template_id)
        .and_then(|t| t.formatos.iter().find(|f| f.nome == deliverable.formato))
        .map(|f| f.binario)
        // Formato fora do catálogo do template (extensão de arquivo livre):
        // texto por padrão — o conteúdo é o que a ferramenta gravou.
        .unwrap_or(false);
    // Lê o conteúdo (texto) gravado pela ferramenta do squad.
    let content = match std::fs::read(&deliverable.path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorBody::new(
                    "file_missing",
                    format!("arquivo da entrega não encontrado: {e}"),
                )),
            )
                .into_response()
        }
    };
    if binario {
        // Converte o texto para o formato binário por serialização determinística
        // (DOCX/XLSX/PDF) ou serve markup como texto (SVG/MusicXML). Formatos que
        // exigem renderização/mídia real (PNG, MIDI) seguem sem conversor → 422.
        let text = String::from_utf8_lossy(&content);
        return match crate::convert::convert(&deliverable.formato, &text) {
            Some(conv) => (
                StatusCode::OK,
                [
                    ("content-type", conv.content_type.to_string()),
                    (
                        "content-disposition",
                        format!(
                            "attachment; filename=\"{}.{}\"",
                            deliverable.nome, conv.extension
                        ),
                    ),
                ],
                conv.bytes,
            )
                .into_response(),
            None => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorBody::new(
                    "format_not_exportable",
                    format!(
                        "exportação de {} exige renderização/conversão de mídia — sem conversor honesto disponível",
                        deliverable.formato
                    ),
                )),
            )
                .into_response(),
        };
    }
    // Formato de texto: serve o conteúdo como está.
    (
        StatusCode::OK,
        [
            ("content-type", "text/plain; charset=utf-8".to_string()),
            (
                "content-disposition",
                format!("attachment; filename=\"{}\"", deliverable.nome),
            ),
        ],
        content,
    )
        .into_response()
}

// ── personas (U7) ──

#[derive(Serialize)]
struct PersonaView {
    papel: String,
    prompt: String,
    padrao: String,
    editado: bool,
}

#[derive(Serialize)]
struct PersonasResponse {
    template_id: String,
    personas: Vec<PersonaView>,
    proprias: Vec<btv_store::btv::CustomPersona>,
}

/// `GET /api/btv/personas/{template_id}` — papéis do modelo com o prompt
/// EFETIVO (override ?? padrão) + personas próprias.
async fn list_personas_handler(
    State(state): State<BtvAgentState>,
    Path(template_id): Path<String>,
    Tenant(ctx): Tenant,
) -> Response {
    // C3.2 (leitura pela porta): mesmo tipo (`PersonaOverride`/`CustomPersona`
    // re-exportados do domínio desde A2), wire byte-idêntico — o golden
    // `personas` é o juiz. Contexto da borda; leitura não é fato auditável.
    use btv_domain::ports::PersonaRepository;
    let Some(template) = btv_schemas::squad_template::builtin_templates()
        .iter()
        .find(|t| t.id == template_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("unknown_template", "modelo desconhecido")),
        )
            .into_response();
    };
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    let overrides: std::collections::HashMap<String, String> = store
        .list_overrides(&ctx, &template_id)
        .unwrap_or_default()
        .into_iter()
        .map(|o| (o.papel, o.prompt))
        .collect();
    let personas = template
        .papeis
        .iter()
        .enumerate()
        .map(|(i, papel)| {
            let padrao = prompt_padrao(i, papel, &template.nome);
            let (prompt, editado) = match overrides.get(papel) {
                Some(ov) => (ov.clone(), ov != &padrao),
                None => (padrao.clone(), false),
            };
            PersonaView {
                papel: papel.clone(),
                prompt,
                padrao,
                editado,
            }
        })
        .collect();
    let proprias = store.list_custom(&ctx, &template_id).unwrap_or_default();
    Json(PersonasResponse {
        template_id,
        personas,
        proprias,
    })
    .into_response()
}

#[derive(Deserialize)]
struct PromptBody {
    prompt: String,
}

/// `PUT /api/btv/personas/{template_id}/{papel}` — override do prompt do
/// papel (efetivo já na PRÓXIMA ativação; auditado no ledger).
async fn set_override_handler(
    State(state): State<BtvAgentState>,
    Path((template_id, papel)): Path<(String, String)>,
    Tenant(ctx): Tenant,
    Json(body): Json<PromptBody>,
) -> Response {
    // C3.2 endpoint 1 (o emissor da onda de personas): a escrita vai pela PORTA
    // (`PersonaRepository::set_override`) com o contexto da borda — o
    // `updated_ts` é escrituração do adapter (o port não carrega relógio,
    // decisão do G1). Wire imóvel: o golden `personas` é o juiz.
    {
        use btv_domain::ports::PersonaRepository;
        let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = store.set_override(&ctx, &template_id, &papel, &body.prompt) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody::new("store_error", e.to_string())),
            )
                .into_response();
        }
    }
    // O fato `btv.persona_updated` vai pela porta do ledger — o corpo hasheado
    // ganha `tenant` (ADR 0027; regravação justificada do golden no ato 3). O
    // mapeamento `role → "papel"` já vive no adapter desde o B3 (`ledger.rs`);
    // o `append_ledger` legado morre NESTE caminho.
    {
        use btv_domain::ports::LedgerRepository;
        let evento = btv_domain::ports::DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts: crate::session::now_rfc3339(),
            kind: btv_domain::ports::DomainEventKind::PersonaUpdated {
                template_id,
                role: papel,
                prompt_sha256: btv_schemas::sha256_hex(&body.prompt),
            },
        };
        let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
            eprintln!("btv: falha ao registrar persona no ledger: {e}");
        }
    }
    StatusCode::OK.into_response()
}

/// `DELETE /api/btv/personas/{template_id}/{papel}` — restaurar ao padrão.
async fn delete_override_handler(
    State(state): State<BtvAgentState>,
    Path((template_id, papel)): Path<(String, String)>,
    Tenant(ctx): Tenant,
) -> Response {
    use btv_domain::ports::PersonaRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.delete_override(&ctx, &template_id, &papel) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

/// `DELETE /api/btv/personas/{template_id}` — restaurar TODOS ao padrão.
async fn clear_overrides_handler(
    State(state): State<BtvAgentState>,
    Path(template_id): Path<String>,
    Tenant(ctx): Tenant,
) -> Response {
    use btv_domain::ports::PersonaRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.clear_overrides(&ctx, &template_id) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CustomPersonaBody {
    nome: String,
    prompt: String,
}

async fn create_custom_handler(
    State(state): State<BtvAgentState>,
    Path(template_id): Path<String>,
    Tenant(ctx): Tenant,
    Json(body): Json<CustomPersonaBody>,
) -> Response {
    use btv_domain::ports::PersonaRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.insert_custom(&ctx, &template_id, &body.nome, &body.prompt) {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

async fn update_custom_handler(
    State(state): State<BtvAgentState>,
    Path((_template_id, id)): Path<(String, i64)>,
    Tenant(ctx): Tenant,
    Json(body): Json<CustomPersonaBody>,
) -> Response {
    use btv_domain::ports::PersonaRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.update_custom(&ctx, id, &body.nome, &body.prompt) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

async fn delete_custom_handler(
    State(state): State<BtvAgentState>,
    Path((_template_id, id)): Path<(String, i64)>,
    Tenant(ctx): Tenant,
) -> Response {
    use btv_domain::ports::PersonaRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.delete_custom(&ctx, id) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SalvarFluxoBody {
    nome: String,
    /// Diagrama completo da lib bpmn (BpmnDiagram serializado) — opaco para
    /// o servidor, hasheado para a trilha (run-binding do Designer).
    diagram: serde_json::Value,
    /// Metadados de versão do registry da lib (`VersionRegistry.register`).
    #[serde(default)]
    versao_semantica: Option<String>,
    #[serde(default)]
    snapshot_hash: Option<String>,
    /// Cabeça da cadeia do AuditLedger da lib (auditoria do fluxo, §7).
    #[serde(default)]
    audit_head: Option<String>,
    #[serde(default)]
    audit_len: Option<u64>,
}

/// `POST /api/btv/designer/flows` — "salvar como modelo" (U5→A5): o fluxo
/// desenhado é validado minimamente, hasheado e gravado no ledger REAL
/// (`btv.flow_saved`) com os metadados de versão do registry da lib.
/// "Salvo e auditado" — a aplicação do fluxo ao orquestrador real continua
/// sendo trabalho futuro (mesma honestidade do Designer do console BuildToValue).
async fn salvar_fluxo_handler(
    State(state): State<BtvAgentState>,
    Tenant(ctx): Tenant,
    Json(body): Json<SalvarFluxoBody>,
) -> Response {
    if body.nome.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new("empty_name", "o fluxo precisa de um nome")),
        )
            .into_response();
    }
    let Some(nodes) = body.diagram.get("nodes").and_then(|n| n.as_object()) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new(
                "invalid_diagram",
                "diagrama sem 'nodes' — não é um BpmnDiagram serializado",
            )),
        )
            .into_response();
    };
    if nodes.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new(
                "empty_diagram",
                "fluxo vazio não vira modelo",
            )),
        )
            .into_response();
    }
    let blocks = nodes.len() as u64;
    let canonico = serde_json::to_string(&body.diagram).unwrap_or_default();
    let diagram_sha256 = btv_schemas::sha256_hex(&canonico);
    // C3.3b: o fato `btv.flow_saved` vai pela porta do ledger com o `Tenant` da
    // borda — o corpo ganha `tenant` (ADR 0027). Emissor PURO (o fluxo em si
    // vive no workflow do Designer; o handler só registra o fato); os 7 campos
    // do `FlowSaved` já são byte-idênticos no adapter desde o B3. Sem porta
    // nova. `append_ledger` legado morto neste caminho.
    use btv_domain::ports::LedgerRepository;
    let evento = btv_domain::ports::DomainEvent {
        tenant: ctx.tenant,
        actor: ctx.actor.clone(),
        ts: crate::session::now_rfc3339(),
        kind: btv_domain::ports::DomainEventKind::FlowSaved {
            name: body.nome,
            blocks,
            diagram_sha256: diagram_sha256.clone(),
            semantic_version: body.versao_semantica,
            snapshot_hash: body.snapshot_hash,
            audit_head: body.audit_head,
            audit_len: body.audit_len,
        },
    };
    let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
    match LedgerRepository::append(&mut *ledger, &ctx, &evento) {
        Ok(seq) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "seq": seq, "diagram_sha256": diagram_sha256 })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("ledger_error", e.to_string())),
        )
            .into_response(),
    }
}

// ── admin (A5 publicação de templates · A6 perfis locais) ──

#[derive(Deserialize)]
struct PublicacaoBody {
    publicado: bool,
}

/// `POST /api/btv/templates/{id}/publicacao` — publicar/despublicar um
/// modelo (A5). Override persistido sobre o `publicado` embutido; auditado.
async fn set_publicacao_handler(
    State(state): State<BtvAgentState>,
    Path(template_id): Path<String>,
    Tenant(ctx): Tenant,
    Json(body): Json<PublicacaoBody>,
) -> Response {
    if !btv_schemas::squad_template::builtin_templates()
        .iter()
        .any(|t| t.id == template_id)
    {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("unknown_template", "modelo desconhecido")),
        )
            .into_response();
    }
    // C3.3a: a publicação vai pela PORTA (`TemplatePublicationRepository`) com o
    // contexto da borda; o `updated_ts` é escrituração do adapter.
    {
        use btv_domain::ports::TemplatePublicationRepository;
        let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = store.set_published(&ctx, &template_id, body.publicado) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody::new("store_error", e.to_string())),
            )
                .into_response();
        }
    }
    // O fato `btv.template_published` pela porta do ledger — o corpo ganha
    // `tenant` (ADR 0027); o mapeamento `published → "publicado"` já vive no
    // adapter desde o B3. `append_ledger` legado morto neste caminho.
    {
        use btv_domain::ports::LedgerRepository;
        let evento = btv_domain::ports::DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts: crate::session::now_rfc3339(),
            kind: btv_domain::ports::DomainEventKind::TemplatePublished {
                template_id,
                published: body.publicado,
            },
        };
        let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
            eprintln!("btv: falha ao registrar publicação no ledger: {e}");
        }
    }
    StatusCode::OK.into_response()
}

/// `GET /api/btv/templates/publicacao` — overrides persistidos (a tela A5
/// mescla com o `publicado` embutido dos templates).
async fn list_publicacao_handler(
    State(state): State<BtvAgentState>,
    Tenant(ctx): Tenant,
) -> Response {
    use btv_domain::ports::TemplatePublicationRepository;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.list_published(&ctx) {
        Ok(list) => Json(
            list.into_iter()
                .map(|(id, publicado)| serde_json::json!({ "template_id": id, "publicado": publicado }))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct NovoUsuarioBody {
    nome: String,
    email: String,
    #[serde(default)]
    papel: Option<String>,
    /// PIN opcional para proteger o perfil (verificado pelo backend).
    #[serde(default)]
    pin: Option<String>,
}

async fn list_users_handler(State(state): State<BtvAgentState>, Tenant(ctx): Tenant) -> Response {
    // C3.4a: leitura pela porta `UserRepository`, contexto da borda. Mesmo tipo
    // (`User` re-exportado desde A2), wire byte-idêntico.
    use btv_domain::ports::UserRepository;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.list(&ctx) {
        Ok(users) => Json(users).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

async fn create_user_handler(
    State(state): State<BtvAgentState>,
    Tenant(ctx): Tenant,
    Json(body): Json<NovoUsuarioBody>,
) -> Response {
    if body.nome.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorBody::new("empty_name", "nome obrigatório")),
        )
            .into_response();
    }
    let papel = body.papel.unwrap_or_else(|| "usuario".into());
    // C3.4a: escrita pela porta; o `created_ts` (salt do pin_hash) é escrituração
    // do adapter.
    use btv_domain::ports::UserRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.create(
        &ctx,
        body.nome.trim(),
        body.email.trim(),
        &papel,
        body.pin.as_deref(),
    ) {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

/// `DELETE /api/btv/users/:id` — remove um perfil de vez (o toggle "ativo" só
/// suspende). Registra `btv.user_removed` no ledger para trilha auditável.
async fn delete_user_handler(
    State(state): State<BtvAgentState>,
    Path(id): Path<i64>,
    Tenant(ctx): Tenant,
) -> Response {
    use btv_domain::ports::{LedgerRepository, UserRepository};
    let resultado = {
        let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        store.remove(&ctx, id)
    };
    match resultado {
        Ok(()) => {
            // C3.4a: o fato `btv.user_removed` pela porta do ledger — o corpo
            // ganha `tenant` (ADR 0027); `UserRemoved{user_id}→{id}` já era
            // byte-idêntico no adapter (B3). O `let _` que engolia o erro morre
            // com o `append_ledger` legado (dívida do levantamento fechada).
            let evento = btv_domain::ports::DomainEvent {
                tenant: ctx.tenant,
                actor: ctx.actor.clone(),
                ts: crate::session::now_rfc3339(),
                kind: btv_domain::ports::DomainEventKind::UserRemoved { user_id: id },
            };
            let mut ledger = state.ledger.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
                eprintln!("btv: falha ao registrar remoção de perfil no ledger: {e}");
            }
            StatusCode::OK.into_response()
        }
        Err(btv_domain::ports::RepositoryError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("user_not_found", "perfil não encontrado")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct AtivoBody {
    ativo: bool,
}

async fn set_user_ativo_handler(
    State(state): State<BtvAgentState>,
    Path(id): Path<i64>,
    Tenant(ctx): Tenant,
    Json(body): Json<AtivoBody>,
) -> Response {
    use btv_domain::ports::UserRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.set_active(&ctx, id, body.ativo) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(btv_domain::ports::RepositoryError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("user_not_found", "perfil não encontrado")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SetPinBody {
    /// PIN novo; vazio/ausente limpa o PIN (perfil volta a aberto).
    #[serde(default)]
    pin: Option<String>,
}

/// `POST /api/btv/users/:id/pin` — define ou limpa o PIN de um perfil.
async fn set_user_pin_handler(
    State(state): State<BtvAgentState>,
    Path(id): Path<i64>,
    Tenant(ctx): Tenant,
    Json(body): Json<SetPinBody>,
) -> Response {
    use btv_domain::ports::UserRepository;
    let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.set_pin(&ctx, id, body.pin.as_deref()) {
        Ok(()) => StatusCode::OK.into_response(),
        Err(btv_domain::ports::RepositoryError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("user_not_found", "perfil não encontrado")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct VerifyPinBody {
    pin: String,
}

/// `POST /api/btv/users/:id/verify-pin` — verifica o PIN de um perfil no
/// backend (o hash nunca sai). `{ok, reason}`: `no_pin` (perfil aberto),
/// `ok` (correto) ou `wrong` (incorreto). Sempre HTTP 200 salvo perfil
/// inexistente (404) — a distinção "aberto/certo/errado" é do corpo.
async fn verify_user_pin_handler(
    State(state): State<BtvAgentState>,
    Path(id): Path<i64>,
    Tenant(ctx): Tenant,
    Json(body): Json<VerifyPinBody>,
) -> Response {
    use btv_domain::ports::UserRepository;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.verify_pin(&ctx, id, &body.pin) {
        Ok(check) => {
            let (ok, reason) = match check {
                btv_domain::PinCheck::NoPin => (true, "no_pin"),
                btv_domain::PinCheck::Ok => (true, "ok"),
                btv_domain::PinCheck::Wrong => (false, "wrong"),
            };
            Json(serde_json::json!({ "ok": ok, "reason": reason })).into_response()
        }
        Err(btv_domain::ports::RepositoryError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody::new("user_not_found", "perfil não encontrado")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody::new("store_error", e.to_string())),
        )
            .into_response(),
    }
}

/// Router aditivo do BuildToValue — `.merge()`ado ao router do agente web
/// (mesma guarda de `Origin`/`Host` do `merged_router`).
pub fn router(
    hub: SquadHub,
    pool: Arc<btv_sidecar::SquadPool>,
    ledger: Arc<Mutex<LedgerStore>>,
    store: Arc<Mutex<BtvStore>>,
    tenant_resolver: TenantResolucao,
) -> Router {
    use axum::routing::{get, put};
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
        .route("/api/btv/designer/flows", post(salvar_fluxo_handler))
        .route(
            "/api/btv/templates/publicacao",
            get(list_publicacao_handler),
        )
        .route(
            "/api/btv/templates/{id}/publicacao",
            post(set_publicacao_handler),
        )
        .route(
            "/api/btv/users",
            get(list_users_handler).post(create_user_handler),
        )
        .route(
            "/api/btv/users/{id}",
            axum::routing::delete(delete_user_handler),
        )
        .route("/api/btv/users/{id}/ativo", post(set_user_ativo_handler))
        .route("/api/btv/users/{id}/pin", post(set_user_pin_handler))
        .route(
            "/api/btv/users/{id}/verify-pin",
            post(verify_user_pin_handler),
        )
        .route("/api/btv/deliverables", get(list_deliverables_handler))
        .route(
            "/api/btv/deliverables/{id}/download",
            get(download_deliverable_handler),
        )
        .route(
            "/api/btv/personas/{template_id}",
            get(list_personas_handler).delete(clear_overrides_handler),
        )
        .route(
            "/api/btv/personas/{template_id}/custom",
            post(create_custom_handler),
        )
        .route(
            "/api/btv/personas/{template_id}/custom/{id}",
            put(update_custom_handler).delete(delete_custom_handler),
        )
        .route(
            "/api/btv/personas/{template_id}/{papel}",
            put(set_override_handler).delete(delete_override_handler),
        )
        .with_state(BtvAgentState {
            squad: SquadAgentState { hub, pool },
            ledger,
            store,
            // Injetado por quem monta o router (main: `from_env`; goldens:
            // `local()`) — mesma fonte do resolver do layer da borda, um único
            // `current_mode()` no arranque. O extractor produz o contexto dos
            // seis a partir daqui; a onda saas injeta Some(PgStore).
            tenant_resolver,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_editorial() -> &'static SquadTemplate {
        btv_schemas::squad_template::builtin_templates()
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
        let d = montar_descricao(
            template,
            &body,
            &papeis,
            &|i, papel| prompt_padrao(i, papel, &template.nome),
            &[],
        );
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

    #[test]
    fn arquivos_escritos_filtra_edits_ok_e_deduplica() {
        use crate::squad_agent::ToolRunNote;
        let runs = vec![
            ToolRunNote {
                tool: "read".into(),
                scope: "a.md".into(),
                exit_code: 0,
            },
            ToolRunNote {
                tool: "edit".into(),
                scope: "artigo.md".into(),
                exit_code: 0,
            },
            ToolRunNote {
                tool: "edit".into(),
                scope: "artigo.md".into(),
                exit_code: 0,
            },
            ToolRunNote {
                tool: "edit".into(),
                scope: "falhou.md".into(),
                exit_code: 1,
            },
            ToolRunNote {
                tool: "bash".into(),
                scope: "echo oi".into(),
                exit_code: 0,
            },
            ToolRunNote {
                tool: "edit".into(),
                scope: "notas.txt".into(),
                exit_code: 0,
            },
        ];
        assert_eq!(arquivos_escritos(&runs), vec!["artigo.md", "notas.txt"]);
    }

    #[test]
    fn descricao_usa_prompt_efetivo_com_override() {
        let body = AtivarSquadBody {
            template_id: "editorial".into(),
            nome: None,
            briefing: vec![],
            refs: vec![],
            papeis_off: vec![],
        };
        let template = template_editorial();
        let papeis: Vec<(usize, &str)> = template
            .papeis
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.as_str()))
            .collect();
        let d = montar_descricao(
            template,
            &body,
            &papeis,
            &|i, papel| {
                if papel == "Redator" {
                    "PROMPT CUSTOMIZADO DO REDATOR".into()
                } else {
                    prompt_padrao(i, papel, &template.nome)
                }
            },
            &[],
        );
        assert!(d.contains("PROMPT CUSTOMIZADO DO REDATOR"));
        assert!(!d.contains("Redator: Você é Redator da squad"));
    }

    #[test]
    fn descricao_inclui_personas_proprias_com_o_prompt_delas() {
        // Fase 3: uma persona criada do zero pelo usuário entra na equipe da
        // descrição com o prompt dela (antes era ignorada na ativação).
        let body = AtivarSquadBody {
            template_id: "editorial".into(),
            nome: None,
            briefing: vec![],
            refs: vec![],
            papeis_off: vec![],
        };
        let template = template_editorial();
        let papeis: Vec<(usize, &str)> = template
            .papeis
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.as_str()))
            .collect();
        let proprias = vec![btv_store::CustomPersona {
            id: 1,
            template_id: "editorial".into(),
            nome: "Curador de fontes".into(),
            prompt: "VOZ DA PERSONA PRÓPRIA: seleciono fontes confiáveis".into(),
            tenant: btv_domain::TenantId::LOCAL,
        }];
        let d = montar_descricao(
            template,
            &body,
            &papeis,
            &|i, papel| prompt_padrao(i, papel, &template.nome),
            &proprias,
        );
        assert!(d.contains("Curador de fontes (persona própria)"));
        assert!(d.contains("VOZ DA PERSONA PRÓPRIA"));
    }
}
