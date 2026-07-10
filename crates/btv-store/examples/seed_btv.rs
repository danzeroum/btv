//! Semeia um run concluído + uma entrega no `BtvStore` REAL — usado pelo
//! harness de integração do btv-web (mirror de `seed_ledger.rs`): prova que
//! a Biblioteca (U4) e Minhas squads (U6) mostram o que foi gravado pelos
//! MESMOS caminhos de produção, sem SQL cru.
//!
//! A4: o `task_id` agora CONFORMA ao formato do produto (`sq{hex}`, via
//! `TaskId` + `max_run_task_seq()+1` — único por estado do DB). O seed
//! antigo gravava `seed-{nome}`, fora do formato que o gerador de produção
//! sempre emitiu — a leitura tipada fail-closed do A4 pegou a divergência
//! (achado real: 4 specs de integração com UI vazia). A leitura de
//! verificação no fim falha AQUI, no seed, se o caminho de leitura rejeitar
//! o que acabou de ser gravado — nunca mais UI vazia silenciosa.
//!
//! Uso: seed_btv <db> <template_id> <nome_run> <arquivo> <formato>

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [db, template_id, nome_run, arquivo, formato] = &args[..] else {
        eprintln!("uso: seed_btv <db> <template_id> <nome_run> <arquivo> <formato>");
        std::process::exit(2);
    };
    let store = btv_store::BtvStore::open(db).expect("abre btv.db");
    let task_id = btv_domain::TaskId::new(store.max_run_task_seq() + 1).to_string();
    let run_id = store
        .insert_run(
            &task_id,
            template_id,
            "v1.4",
            nome_run,
            r#"[{"label":"Pauta","resposta":"seed"}]"#,
            r#"["Redator","Revisor de estilo"]"#,
            "2026-07-08T00:00:00Z",
        )
        .expect("insere run");
    store
        .increment_gates(&task_id, "2026-07-08T00:05:00Z")
        .expect("gate");
    store
        .set_status(
            &task_id,
            btv_domain::ports::RunStatus::Concluida,
            "2026-07-08T00:10:00Z",
        )
        .expect("status");
    let nome = std::path::Path::new(arquivo)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| arquivo.clone());
    store
        .insert_deliverable(
            run_id,
            &task_id,
            template_id,
            &nome,
            arquivo,
            formato,
            "v1",
            "Redator → Revisor de estilo · 1 gate(s) aprovado(s) por você",
            "2026-07-08T00:10:00Z",
        )
        .expect("insere entrega");
    // Verificação de leitura: o que foi gravado VOLTA pelo caminho tipado.
    let runs = store.list_runs().expect("lê runs de volta (fail-closed)");
    assert!(
        runs.iter().any(|r| r.id == run_id),
        "run semeado não voltou na leitura"
    );
    let entregas = store
        .list_deliverables()
        .expect("lê entregas de volta (fail-closed)");
    assert!(
        entregas.iter().any(|d| d.run_id == run_id),
        "entrega semeada não voltou na leitura"
    );
    println!("run {run_id} (task {task_id}) + entrega semeados e relidos em {db}");
}
