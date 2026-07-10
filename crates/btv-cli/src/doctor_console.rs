//! Fase 7 Onda 13 (Modelo & Onboarding): `GET /api/doctor` agrega checagens
//! já existentes mas espalhadas (env vars do gateway, `uv --version`, ping ao
//! Docker via bollard, git) numa única resposta — mesmo padrão de agregação
//! de `GET /api/sandbox`/`GET /api/mcp` (Ondas 7/10). Mora aqui (não em
//! `btv-server`) porque a checagem de Docker precisa de
//! `btv_tools::sandbox::Sandbox` — regra de posicionamento de rota da fase.

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use btv_store::btv::VocabViolation;
use btv_store::{BtvStore, LedgerStore};
use btv_tools::sandbox::Sandbox;
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
struct DoctorCheck {
    id: &'static str,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
struct DoctorView {
    checks: Vec<DoctorCheck>,
}

/// Mesma ordem/conjunto de `btv-server`'s `KNOWN_PROVIDERS` (Onda 12) —
/// duplicada, não importada: a direção de dependência entre os dois crates
/// só permite `btv-cli` depender de `btv-server`, nunca o contrário, e
/// isto mora em `btv-cli`. Mesma convenção já usada por `git_sha`/
/// `now_rfc3339`.
const KNOWN_PROVIDERS: [&str; 3] = ["anthropic", "deepseek", "openai"];

fn providers_check() -> DoctorCheck {
    let gateway = btv_llm::gateway::Gateway::from_env();
    let available: std::collections::HashSet<String> = gateway.available().into_iter().collect();
    let configured = KNOWN_PROVIDERS
        .iter()
        .filter(|id| available.contains(**id))
        .count();
    DoctorCheck {
        id: "providers",
        ok: configured > 0,
        detail: format!(
            "{configured}/{} provider(s) configurado(s)",
            KNOWN_PROVIDERS.len()
        ),
    }
}

/// `uv --version` com PATH injetável — permite ao teste simular "uv ausente"
/// apontando pra um PATH garantidamente vazio, sem depender do PATH real do
/// processo de teste (mesmo espírito de `Sandbox::ping_with` receber um
/// client já configurado em vez de só `ping()`). Confere `status.success()`,
/// não só se o processo conseguiu ser criado — diferente do guard de teste
/// `uv_missing()` (duplicado em vários `#[cfg(test)]` deste workspace, que só
/// quer saber "existe pra pular o teste"), este é o doctor real mostrado ao
/// usuário, então um `uv` presente mas quebrado deve aparecer como ausente.
fn uv_check_with_path(path_override: Option<&str>) -> DoctorCheck {
    let mut cmd = std::process::Command::new("uv");
    cmd.arg("--version");
    if let Some(path) = path_override {
        cmd.env("PATH", path);
    }
    let ok = cmd.output().is_ok_and(|o| o.status.success());
    DoctorCheck {
        id: "uv",
        ok,
        detail: if ok {
            "uv encontrado — sidecar Python (squad/PromptForge) disponível".into()
        } else {
            "uv ausente do PATH — squad/PromptForge ficam indisponíveis".into()
        },
    }
}

/// Reusa `crate::git_sha()` (`main.rs`) — mesma checagem que já formata o sha
/// no cabeçalho do `btv verify`, aqui só reinterpretada como um check
/// ok/detail. `pub(crate)` não é necessário: `git_sha` já é visível a
/// qualquer módulo deste crate (item privado do módulo raiz).
fn git_check() -> DoctorCheck {
    match crate::git_sha() {
        Some(sha) => DoctorCheck {
            id: "git",
            ok: true,
            detail: format!(
                "repositório git detectado — HEAD {}",
                &sha[..sha.len().min(8)]
            ),
        },
        None => DoctorCheck {
            id: "git",
            ok: false,
            detail: "git ausente do PATH ou fora de um repositório".into(),
        },
    }
}

async fn docker_check() -> DoctorCheck {
    let ok = Sandbox::ping().await;
    DoctorCheck {
        id: "docker",
        ok,
        detail: if ok {
            "daemon Docker alcançável — sandbox de skills de terceiro disponível".into()
        } else {
            "daemon Docker inalcançável — skills de terceiro não rodam confinadas".into()
        },
    }
}

/// Varredura de VOCABULÁRIO (pendência da revisão do A4): valida cada
/// `task_id`/`status` de runs+deliverables e cada `kind` do ledger com os
/// MESMOS parses fail-closed do domínio (A3) e aponta A LINHA ofensora —
/// transforma o fail-closed que "grita como UI vazia" em diagnóstico
/// acionável. O SQL mora em `btv-store` (métodos de scan); aqui só a
/// agregação, como manda o espírito do T4-B.
fn vocabulario_check(stores: &DoctorStores) -> DoctorCheck {
    let mut fora: Vec<VocabViolation> = Vec::new();
    match stores.btv.lock().unwrap().linhas_fora_do_vocabulario() {
        Ok(mut v) => fora.append(&mut v),
        Err(e) => {
            return DoctorCheck {
                id: "vocabulario",
                ok: false,
                detail: format!("varredura de runs/deliverables falhou: {e}"),
            }
        }
    }
    match stores.ledger.lock().unwrap().kinds_fora_do_vocabulario() {
        Ok(mut v) => fora.append(&mut v),
        Err(e) => {
            return DoctorCheck {
                id: "vocabulario",
                ok: false,
                detail: format!("varredura do ledger falhou: {e}"),
            }
        }
    }
    if fora.is_empty() {
        return DoctorCheck {
            id: "vocabulario",
            ok: true,
            detail: "runs, deliverables e ledger dentro do vocabulário fechado do domínio".into(),
        };
    }
    // A linha ofensora, uma a uma — é o diagnóstico, não um contador.
    let linhas: Vec<String> = fora
        .iter()
        .map(|v| {
            format!(
                "{}[{}].{} = {:?} ({})",
                v.tabela, v.linha, v.coluna, v.valor, v.erro
            )
        })
        .collect();
    DoctorCheck {
        id: "vocabulario",
        ok: false,
        detail: linhas.join("; "),
    }
}

/// Stores do produto injetados pelo `btv dashboard` (as MESMAS instâncias
/// dos handlers do BuildToValue — nenhuma conexão paralela).
#[derive(Clone)]
pub struct DoctorStores {
    pub ledger: Arc<Mutex<LedgerStore>>,
    pub btv: Arc<Mutex<BtvStore>>,
}

/// `GET /api/doctor` — as 4 checagens de `btv init`, hoje espalhadas,
/// agregadas numa resposta só + a varredura de vocabulário (A4). Reexecuta
/// tudo a cada request (mesmo estilo síncrono-por-request de
/// `/api/sandbox`/`/api/mcp`) sem cache — o custo é baixo (1 spawn de
/// processo cada + 1 ping Docker com timeout curto + 1 scan SQLite local).
async fn get_doctor(State(stores): State<DoctorStores>) -> impl IntoResponse {
    let checks = vec![
        providers_check(),
        uv_check_with_path(None),
        docker_check().await,
        git_check(),
        vocabulario_check(&stores),
    ];
    Json(DoctorView { checks })
}

/// Router aditivo do doctor — `.merge()`ado ao router do agente web, mesma
/// composição de `sandbox_console::router`/`lsp_console::router`.
pub fn router(stores: DoctorStores) -> Router {
    Router::new()
        .route("/api/doctor", get(get_doctor))
        .with_state(stores)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Determinístico, sem depender do PATH real do processo de teste: aponta
    /// pra um PATH vazio, garantidamente sem `uv`.
    #[test]
    fn uv_ausente_do_path_da_check_false() {
        let check = uv_check_with_path(Some(""));
        assert!(!check.ok);
        assert!(check.detail.contains("ausente"));
    }

    /// Fronteira: `providers` bate por igualdade com os mesmos env vars reais
    /// que uma sessão real leria — mesma técnica de isolamento de
    /// `providers_reflete_env_vars_reais` (`btv-server`, Onda 12): nenhum
    /// outro teste deste crate lê essas 3 chaves, então não há disputa com
    /// outro teste rodando em paralelo no mesmo binário. `uv`/`docker`/`git`
    /// não têm valor fixo afirmado aqui (variam por ambiente — mesmo espírito
    /// do teste de `/api/sandbox`); só que vieram bem formados.
    fn stores_vazios() -> DoctorStores {
        DoctorStores {
            ledger: Arc::new(Mutex::new(
                LedgerStore::open_in_memory().expect("ledger em memória"),
            )),
            btv: Arc::new(Mutex::new(
                BtvStore::open_in_memory().expect("btv em memória"),
            )),
        }
    }

    #[tokio::test]
    async fn doctor_agrega_as_5_checagens_com_providers_real() {
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-onda-13");

        let app = router(stores_vazios());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/doctor")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let checks = json["checks"].as_array().unwrap();
        assert_eq!(checks.len(), 5);

        let ids: Vec<&str> = checks.iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["providers", "uv", "docker", "git", "vocabulario"]);

        // Bancos frescos: vocabulário fechado, ok=true — o caminho VERMELHO
        // (linha ofensora apontada) é provado nos testes de scan de
        // btv-store, onde a conexão é acessível para corromper uma linha.
        let vocab = checks.iter().find(|c| c["id"] == "vocabulario").unwrap();
        assert_eq!(vocab["ok"], true);

        let providers = checks.iter().find(|c| c["id"] == "providers").unwrap();
        assert_eq!(providers["ok"], true);
        assert_eq!(providers["detail"], "1/3 provider(s) configurado(s)");

        for id in ["uv", "docker", "git"] {
            let c = checks.iter().find(|c| c["id"] == id).unwrap();
            assert!(
                c["ok"].is_boolean(),
                "{id} deveria devolver um bool bem formado"
            );
            assert!(c["detail"].is_string());
        }
    }
}
