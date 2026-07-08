//! Semeia um run concluído + uma entrega no `BtvStore` REAL — usado pelo
//! harness de integração do btv-web (mirror de `seed_ledger.rs`): prova que
//! a Biblioteca (U4) e Minhas squads (U6) mostram o que foi gravado pelos
//! MESMOS caminhos de produção, sem SQL cru.
//!
//! Uso: seed_btv <db> <template_id> <nome_run> <arquivo> <formato>

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [db, template_id, nome_run, arquivo, formato] = &args[..] else {
        eprintln!("uso: seed_btv <db> <template_id> <nome_run> <arquivo> <formato>");
        std::process::exit(2);
    };
    let store = btv_store::BtvStore::open(db).expect("abre btv.db");
    let task_id = format!("seed-{nome_run}");
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
        .set_status(&task_id, "concluida", "2026-07-08T00:10:00Z")
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
    println!("run {run_id} + entrega semeados em {db}");
}
