//! Trilha T3 do plano DDD multitenant — compatibilidade wire provada por
//! propriedade, não por exemplo.
//!
//! Cada tipo que a Trilha A (A3) vai transformar em enum/newtype tem aqui a
//! prova de qual é a string EXATA que circula hoje no banco e na API, gerada
//! a partir do conjunto REAL de valores do código (fixture
//! `schemas/fixtures/wire-strings.v1.json`), nunca de strings arbitrárias:
//!
//! - `RunStatus`  — os 4 status produzidos pelos caminhos de produção de
//!   `BtvStore` (`insert_run`/`set_status`/`reconcile_stale_runs`);
//! - `TaskId`     — propriedade gerador↔parser do formato `sq{hex}`
//!   (proptest sobre u64 arbitrários, via SQLite real);
//! - `LedgerKind` — round-trip de cada kind real por `LedgerStore` +
//!   presença de cada kind da fixture no código-fonte de produção;
//! - kinds do EventStore — as 3 constantes de `btv-core` + o rename serde
//!   para `type` no wire;
//! - `TelemetryName` — round-trip dos 4 nomes reais por `Telemetry`.
//!
//! Quando A3 introduzir os tipos, o round-trip serde deles deve reproduzir
//! byte-a-byte estas strings — estes testes são a rede que falha se não
//! reproduzir. Limitação declarada (não escondida): a direção
//! "kind novo no código ⇒ fixture desatualizada" só fecha de vez quando o
//! `LedgerKind` exaustivo de A3 existir; até lá, kind novo exige adicionar à
//! fixture (revisado como mudança de contrato).

use proptest::prelude::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/wire-strings.v1.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture wire-strings.v1"))
        .expect("fixture wire-strings.v1 é JSON válido")
}

fn fixture_set(key: &str) -> BTreeSet<String> {
    fixture()[key]
        .as_array()
        .unwrap_or_else(|| panic!("fixture sem lista `{key}`"))
        .iter()
        .map(|v| v.as_str().expect("string").to_string())
        .collect()
}

const TS: &str = "2026-07-08T10:00:00Z";

// ── RunStatus ───────────────────────────────────────────────────────────────

/// Os status são produzidos pelos MESMOS caminhos de produção (nunca UPDATE
/// de teste) e lidos de volta do SQLite real: o conjunto observado deve ser
/// EXATAMENTE o da fixture, e a serialização serde (o wire de
/// `GET /api/btv/squads`) deve repetir a string do banco byte-a-byte.
#[test]
fn run_status_dos_caminhos_de_producao_reproduz_exatamente_a_fixture() {
    let store = btv_store::BtvStore::open_in_memory().unwrap();
    // 1 run por transição terminal + 1 que fica "ativa" + 1 zumbi reconciliado.
    for (i, _) in ["ativa", "concluida", "erro", "encerrada", "zumbi"]
        .iter()
        .enumerate()
    {
        store
            .insert_run(&format!("sq{i:x}"), "editorial", "v1", "t", "[]", "[]", TS)
            .unwrap();
    }
    store
        .set_status("sq1", btv_domain::ports::RunStatus::Concluida, TS)
        .unwrap();
    store
        .set_status("sq2", btv_domain::ports::RunStatus::Erro, TS)
        .unwrap();
    store
        .set_status("sq3", btv_domain::ports::RunStatus::Encerrada, TS)
        .unwrap();
    // O zumbi (sq4) e o sq0 continuam "ativa"; a reconciliação de startup
    // transiciona TODOS os ativos — então preservamos um "ativa" observado
    // ANTES de reconciliar.
    let antes = store.get_run_by_task("sq0").unwrap().unwrap();
    assert_eq!(
        antes.status.as_str(),
        "ativa",
        "status inicial do caminho de produção"
    );
    store.reconcile_stale_runs(TS).unwrap();

    // A4: o status agora é RunStatus — o conjunto observado vem de as_str(),
    // que é EXATAMENTE o byte gravado no banco (é o que este teste prova).
    let mut observados: BTreeSet<String> = store
        .list_runs()
        .unwrap()
        .iter()
        .map(|r| r.status.as_str().to_string())
        .collect();
    observados.insert(antes.status.as_str().to_string());

    assert_eq!(
        observados,
        fixture_set("run_status"),
        "conjunto de status observado no banco divergiu da fixture"
    );

    // Wire HTTP: serde repete a string do banco byte-a-byte.
    for run in store.list_runs().unwrap() {
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["status"].as_str().unwrap(), run.status.as_str());
    }
}

// ── TaskId (`sq{hex}`) ──────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Propriedade gerador↔parser: para QUALQUER conjunto de u64, o formato
    /// de produção `format!("sq{n:x}")` (o mesmo de `SquadHub::new_task`,
    /// cujo formato já é cravado pelo teste `seed_task_seq_continua_...` em
    /// `squad_agent.rs`) é parseado de volta por
    /// `BtvStore::max_run_task_seq` (strip `sq` + hex) — via SQLite REAL,
    /// não string em memória. O `TaskId` de A3 ("valida sq{hex}") tem que
    /// preservar exatamente este par.
    #[test]
    fn task_id_sq_hex_roundtrip_gerador_parser(
        ns in proptest::collection::btree_set(any::<u64>(), 1..8)
    ) {
        let store = btv_store::BtvStore::open_in_memory().unwrap();
        for n in &ns {
            store
                .insert_run(&format!("sq{n:x}"), "editorial", "v1", "t", "[]", "[]", TS)
                .unwrap();
        }
        prop_assert_eq!(store.max_run_task_seq(), *ns.iter().max().unwrap());
    }

    // ── LedgerKind ──────────────────────────────────────────────────────────

    /// Round-trip por kind REAL (amostrado da fixture, não arbitrário):
    /// append em `LedgerStore` de verdade, leitura de volta, kind byte-a-byte
    /// no struct E no JSON servido por `GET /api/ledger` — com a cadeia
    /// íntegra depois.
    #[test]
    fn ledger_kind_real_roundtrip_byte_a_byte(
        kinds in proptest::sample::subsequence(
            fixture_set("ledger_kinds").into_iter().collect::<Vec<_>>(),
            1..8,
        )
    ) {
        let mut ledger = btv_store::LedgerStore::open_in_memory().unwrap();
        for kind in &kinds {
            let registrado = ledger
                .append(btv_schemas::ledger::LedgerEntry {
                    seq: 0,
                    prev_hash: String::new(),
                    entry_hash: String::new(),
                    kind: kind.clone(),
                    actor: "web:btv".into(),
                    payload: serde_json::json!({"t3": true}),
                    r#override: None,
                    fake_marker: None,
                    ts: TS.into(),
                    tenant: None,
                })
                .unwrap();
            prop_assert_eq!(&registrado.kind, kind);
        }
        let lidas = ledger.recent(kinds.len() as u32, None).unwrap();
        let lidos: BTreeSet<&str> = lidas.iter().map(|e| e.kind.as_str()).collect();
        let esperados: BTreeSet<&str> = kinds.iter().map(|k| k.as_str()).collect();
        prop_assert_eq!(lidos, esperados);
        for e in &lidas {
            let json = serde_json::to_value(e).unwrap();
            prop_assert_eq!(json["kind"].as_str().unwrap(), e.kind.as_str());
        }
        prop_assert_eq!(ledger.verify_chain().unwrap(), kinds.len() as u64);
    }

    // ── TelemetryName ───────────────────────────────────────────────────────

    /// Round-trip dos nomes reais por `Telemetry` de verdade: gravado e lido
    /// idêntico, no struct e no JSON (o wire de `GET /api/events`).
    #[test]
    fn telemetry_name_real_roundtrip_byte_a_byte(
        nomes in proptest::sample::subsequence(
            fixture_set("telemetry_names").into_iter().collect::<Vec<_>>(),
            1..5,
        )
    ) {
        let telemetry = btv_store::Telemetry::open_in_memory().unwrap();
        for nome in &nomes {
            telemetry.record(nome, "s1", serde_json::json!({}), TS);
        }
        let lidos: BTreeSet<String> = telemetry
            .recent(nomes.len() as u32)
            .iter()
            .map(|r| r.name.clone())
            .collect();
        let esperados: BTreeSet<String> = nomes.iter().cloned().collect();
        prop_assert_eq!(&lidos, &esperados);
        for r in telemetry.recent(nomes.len() as u32) {
            let json = serde_json::to_value(&r).unwrap();
            prop_assert_eq!(json["name"].as_str().unwrap(), r.name.as_str());
        }
    }
}

/// Cada kind da fixture existe como literal no código-fonte de PRODUÇÃO
/// (módulos de teste `#[cfg(test)]`, diretórios `tests/` e linhas de
/// comentário excluídos): a fixture não pode inventar vocabulário que o
/// código não emite. Kind removido do código sem atualizar a fixture ⇒ este
/// teste falha.
#[test]
fn todo_ledger_kind_da_fixture_existe_no_codigo_de_producao() {
    let fontes = fontes_de_producao();
    let ausentes: Vec<String> = fixture_set("ledger_kinds")
        .into_iter()
        .filter(|kind| !fontes.contains(&format!("\"{kind}\"")))
        .collect();
    assert!(
        ausentes.is_empty(),
        "kinds na fixture sem emissor no código de produção: {ausentes:?} — \
         remova da fixture (mudança de contrato) ou aponte o emissor real"
    );
}

/// As exclusões CONSCIENTES registradas na fixture (`excluded`) são
/// load-bearing, não documentais: cada item excluído (a) não pode estar na
/// lista principal correspondente, e (b) não pode ter emissor no código de
/// produção — `certification` vive só em módulo de teste e `rate.limited`
/// só em doc-comment, ambos fora da varredura. Se um deles ganhar um
/// emissor real, este teste falha mandando movê-lo (com a A3 decidindo o
/// lugar dele no enum exaustivo).
#[test]
fn exclusoes_conscientes_nao_tem_emissor_de_producao() {
    let fontes = fontes_de_producao();
    for (lista, chave) in [
        ("ledger_kinds", "ledger_kinds"),
        ("telemetry_names", "telemetry_names"),
    ] {
        let incluidos = fixture_set(lista);
        let excluidos = fixture()["excluded"][chave]
            .as_object()
            .unwrap_or_else(|| panic!("fixture sem excluded.{chave}"))
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert!(!excluidos.is_empty(), "excluded.{chave} vazio");
        for nome in excluidos {
            assert!(
                !incluidos.contains(&nome),
                "`{nome}` está ao mesmo tempo em {lista} e em excluded.{chave}"
            );
            assert!(
                !fontes.contains(&format!("\"{nome}\"")),
                "`{nome}` está em excluded.{chave} mas ganhou emissor de \
                 produção — mova para a lista principal (e a A3 o inclui no enum)"
            );
        }
    }
}

fn fontes_de_producao() -> String {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut fontes = String::new();
    coleta_fontes_de_producao(&raiz.join("crates"), &mut fontes);
    fontes
}

/// Concatena os .rs de produção: pula diretórios `tests/`/`target/`, corta
/// cada arquivo no início do módulo de teste INLINE (`#[cfg(test)]` seguido
/// de `mod tests`, convenção do repo — no fim do arquivo) e descarta linhas
/// de comentário (`//`/`///`/`//!`) — literal citado em doc-comment não é
/// emissor (caso real: `rate.limited`, só exemplo em telemetry.rs). Cortar
/// em QUALQUER `#[cfg(test)]` seria errado: declarações como
/// `#[cfg(test)] mod test_support;` aparecem no TOPO de main.rs e o corte
/// descartaria o arquivo inteiro (bug real pego por este próprio teste ao
/// nascer: `skill.vetting`, emitido em main.rs, sumia da varredura).
fn coleta_fontes_de_producao(dir: &Path, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let nome = entry.file_name();
        if path.is_dir() {
            if nome != "tests" && nome != "target" {
                coleta_fontes_de_producao(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(conteudo) = std::fs::read_to_string(&path) {
                let producao = match conteudo.find("#[cfg(test)]\nmod tests") {
                    Some(idx) => &conteudo[..idx],
                    None => &conteudo,
                };
                for linha in producao.lines() {
                    if !linha.trim_start().starts_with("//") {
                        out.push_str(linha);
                        out.push('\n');
                    }
                }
            }
        }
    }
}

// ── kinds do EventStore ─────────────────────────────────────────────────────

/// As 3 constantes de produção de `btv-core::session` batem com a fixture, e
/// o wire do `StoredEvent` usa o rename serde `type` (não `kind`) — fato de
/// contrato que A2/A3 têm que preservar. Round-trip pelo `EventStore` real.
#[test]
fn event_store_kinds_e_rename_type_batem_com_a_fixture() {
    let consts: BTreeSet<String> = [
        btv_core::session::SESSION_STARTED,
        btv_core::session::MESSAGE,
        btv_core::session::EPOCH_STARTED,
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(consts, fixture_set("event_store_kinds"));

    let mut store = btv_store::EventStore::open_in_memory().unwrap();
    for kind in &consts {
        let mut agg = String::from("agg-");
        agg.push_str(kind);
        store
            .append(
                &agg,
                0,
                vec![btv_store::EventInput::new(
                    kind.clone(),
                    serde_json::json!({"t3": true}),
                )],
            )
            .unwrap();
        let eventos = store.read(&agg, 0).unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(&eventos[0].kind, kind);
        let json = serde_json::to_value(&eventos[0]).unwrap();
        assert_eq!(json["type"].as_str().unwrap(), kind, "wire usa `type`");
        assert!(
            json.get("kind").is_none(),
            "o campo interno `kind` NÃO aparece no wire (rename serde)"
        );
    }
}
