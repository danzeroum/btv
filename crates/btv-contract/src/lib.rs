//! Suíte de contrato dual-adapter (Trilha B do plano DDD, ADR 0026).
//!
//! Estas funções testam o CONTRATO das traits de `btv-domain::ports` —
//! comportamento observável pela assinatura — e são o artefato mais longevo
//! da trilha: o adapter SQLite (B2) e o adapter Postgres (B4) passam pelos
//! MESMOS testes; é isso que garante paridade entre o modo local e o SaaS.
//! Se um teste daqui só passa por idiossincrasia de um adapter, ele está
//! testando o adapter errado — regra da revisão do A4.
//!
//! Uso (no teste do adapter): `btv_contract::suite_run_repository(|| meu_adapter())`
//! — cada caso recebe um repositório FRESCO da factory (estado isolado).

use btv_domain::ports::{
    DomainEvent, DomainEventKind, LedgerRepository, PersonaRepository, RunRepository, RunStatus,
};
use btv_domain::tenant::{ActorId, TenantContext, TenantId};
use btv_domain::{Deliverable, Run, TaskId};

/// Tenant A dos cenários multi-tenant (o LOCAL — o modo local é um tenant).
pub fn ctx_a() -> TenantContext {
    TenantContext::local(ActorId::new("contract:a").unwrap())
}

/// Tenant B — um segundo tenant real, para provar isolamento.
pub fn ctx_b() -> TenantContext {
    TenantContext::new(
        TenantId::parse("00000000-0000-0000-0000-0000000000b2").unwrap(),
        ActorId::new("contract:b").unwrap(),
    )
}

pub fn run_novo(ctx: &TenantContext, seq: u64, nome: &str) -> Run {
    Run {
        id: 0, // atribuído pelo adapter no primeiro save
        task_id: TaskId::new(seq),
        template_id: "editorial".into(),
        template_versao: "v1.4".into(),
        nome: nome.into(),
        briefing_json: r#"[{"label":"Pauta","resposta":"contrato"}]"#.into(),
        papeis_json: r#"["Redator"]"#.into(),
        status: RunStatus::Ativa,
        gates_aprovados: 0,
        created_ts: "2026-07-08T10:00:00Z".into(),
        updated_ts: "2026-07-08T10:00:00Z".into(),
        tenant: ctx.tenant,
    }
}

pub fn entrega_nova(ctx: &TenantContext, run_id: i64, seq: u64, nome: &str) -> Deliverable {
    Deliverable {
        id: 0,
        run_id,
        task_id: TaskId::new(seq),
        template_id: "editorial".into(),
        nome: nome.into(),
        path: format!("/tmp/{nome}"),
        formato: "MD".into(),
        versao: "v1".into(),
        trilha: "Redator · 1 gate(s)".into(),
        created_ts: "2026-07-08T10:10:00Z".into(),
        tenant: ctx.tenant,
    }
}

/// Contrato do `RunRepository`. `make` produz um adapter FRESCO por caso.
pub fn suite_run_repository<R: RunRepository>(mut make: impl FnMut() -> R) {
    // save + get: round-trip dentro do tenant (id atribuído no primeiro save).
    {
        let mut repo = make();
        let ctx = ctx_a();
        let run = run_novo(&ctx, 1, "Round-trip");
        repo.save(&ctx, &run).expect("save");
        let lido = repo
            .get(&ctx, "sq1")
            .expect("get")
            .expect("run existe no tenant dono");
        assert_eq!(lido.task_id, TaskId::new(1));
        assert_eq!(lido.nome, "Round-trip");
        assert_eq!(lido.status, RunStatus::Ativa);
        assert_eq!(lido.tenant, ctx.tenant);
    }

    // ISOLAMENTO fail-closed na leitura: run de outro tenant é
    // indistinguível de inexistente (rustdoc aceito no G1).
    {
        let mut repo = make();
        repo.save(&ctx_a(), &run_novo(&ctx_a(), 1, "Do tenant A"))
            .expect("save A");
        assert!(
            repo.get(&ctx_b(), "sq1").expect("get B").is_none(),
            "tenant B não enxerga run do tenant A"
        );
        assert!(repo.list(&ctx_b()).expect("list B").is_empty());
        assert!(repo
            .list_deliverables(&ctx_b())
            .expect("entregas B")
            .is_empty());
    }

    // MESMO task_id em tenants diferentes coexiste (no SaaS cada tenant
    // gera sq1, sq2, … por processo — a unicidade é POR tenant).
    {
        let mut repo = make();
        repo.save(&ctx_a(), &run_novo(&ctx_a(), 1, "A"))
            .expect("save A");
        repo.save(&ctx_b(), &run_novo(&ctx_b(), 1, "B"))
            .expect("save B — sq1 coexiste entre tenants");
        assert_eq!(repo.get(&ctx_a(), "sq1").unwrap().unwrap().nome, "A");
        assert_eq!(repo.get(&ctx_b(), "sq1").unwrap().unwrap().nome, "B");
    }

    // save é upsert por (tenant, task_id): o estado do agregado persiste —
    // é o caminho repo.get → run.approve_gate → repo.save (Fase 2 do plano).
    {
        let mut repo = make();
        let ctx = ctx_a();
        repo.save(&ctx, &run_novo(&ctx, 1, "Antes")).expect("save");
        let mut run = repo.get(&ctx, "sq1").unwrap().unwrap();
        let _evento = run
            .approve_gate(&ctx, Some("Gate".into()), "2026-07-08T11:00:00Z".into())
            .expect("gate no run ativo");
        run.transition_to(&ctx, RunStatus::Concluida, "2026-07-08T12:00:00Z".into())
            .expect("transição válida");
        repo.save(&ctx, &run).expect("upsert");
        let relido = repo.get(&ctx, "sq1").unwrap().unwrap();
        assert_eq!(relido.gates_aprovados, 1);
        assert_eq!(relido.status, RunStatus::Concluida);
        assert_eq!(relido.updated_ts, "2026-07-08T12:00:00Z");
        assert_eq!(
            repo.list(&ctx).unwrap().len(),
            1,
            "upsert não duplica o run"
        );
    }

    // list: mais recente primeiro, só do tenant.
    {
        let mut repo = make();
        let ctx = ctx_a();
        repo.save(&ctx, &run_novo(&ctx, 1, "Primeiro")).unwrap();
        repo.save(&ctx, &run_novo(&ctx, 2, "Segundo")).unwrap();
        repo.save(&ctx_b(), &run_novo(&ctx_b(), 3, "De B")).unwrap();
        let lista = repo.list(&ctx).unwrap();
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].nome, "Segundo", "mais recente primeiro");
        assert_eq!(lista[1].nome, "Primeiro");
    }

    // save_with_deliverables: run + entregas numa TRANSAÇÃO (critério 4 do
    // G1) — sucesso grava tudo…
    {
        let mut repo = make();
        let ctx = ctx_a();
        let run = run_novo(&ctx, 1, "Com entregas");
        let entregas = [
            entrega_nova(&ctx, 0, 1, "artigo.md"),
            entrega_nova(&ctx, 0, 1, "resumo.md"),
        ];
        repo.save_with_deliverables(&ctx, &run, &entregas)
            .expect("transação completa");
        assert_eq!(repo.list_deliverables(&ctx).unwrap().len(), 2);
        assert!(repo.get(&ctx, "sq1").unwrap().is_some());
        let d = repo.get_deliverable(&ctx, 1).unwrap().expect("entrega 1");
        assert_eq!(d.nome, "artigo.md");
        assert!(
            repo.get_deliverable(&ctx_b(), 1).unwrap().is_none(),
            "entrega invisível para outro tenant"
        );
    }

    // …e falha no MEIO desfaz TUDO (rollback provado): uma entrega de outro
    // tenant no lote é recusa fail-closed do adapter — nem o run nem a
    // primeira entrega podem sobreviver.
    {
        let mut repo = make();
        let ctx = ctx_a();
        let run = run_novo(&ctx, 1, "Atômico");
        let boa = entrega_nova(&ctx, 0, 1, "boa.md");
        let intrusa = entrega_nova(&ctx_b(), 0, 1, "intrusa.md"); // tenant errado
        let err = repo
            .save_with_deliverables(&ctx, &run, &[boa, intrusa])
            .expect_err("lote com tenant alheio é recusado");
        let msg = err.to_string();
        assert!(!msg.is_empty());
        assert!(
            repo.get(&ctx, "sq1").expect("get").is_none(),
            "rollback: o run não sobrevive à transação falhada"
        );
        assert!(
            repo.list_deliverables(&ctx).unwrap().is_empty(),
            "rollback: nenhuma entrega sobrevive"
        );
    }

    // Fail-closed também no run: salvar run cujo tenant ≠ ctx é recusado.
    {
        let mut repo = make();
        let alheio = run_novo(&ctx_b(), 1, "Alheio");
        assert!(
            repo.save(&ctx_a(), &alheio).is_err(),
            "run de outro tenant não entra pelo contexto errado"
        );
    }

    // max_task_seq é POR tenant (semeia o contador do hub no arranque).
    {
        let mut repo = make();
        repo.save(&ctx_a(), &run_novo(&ctx_a(), 5, "A5")).unwrap();
        repo.save(&ctx_b(), &run_novo(&ctx_b(), 9, "B9")).unwrap();
        assert_eq!(repo.max_task_seq(&ctx_a()).unwrap(), 5);
        assert_eq!(repo.max_task_seq(&ctx_b()).unwrap(), 9);
    }
}

/// Contrato do `PersonaRepository`.
pub fn suite_persona_repository<P: PersonaRepository>(mut make: impl FnMut() -> P) {
    // overrides: set/list/delete/clear dentro do tenant.
    {
        let mut repo = make();
        let ctx = ctx_a();
        repo.set_override(&ctx, "editorial", "Redator", "voz A")
            .expect("set");
        repo.set_override(&ctx, "editorial", "Pauteiro", "voz P")
            .expect("set 2");
        // upsert: reescrever o mesmo papel troca o prompt, não duplica.
        repo.set_override(&ctx, "editorial", "Redator", "voz A2")
            .expect("upsert");
        let lista = repo.list_overrides(&ctx, "editorial").expect("list");
        assert_eq!(lista.len(), 2);
        assert_eq!(
            lista.iter().find(|o| o.papel == "Redator").unwrap().prompt,
            "voz A2"
        );
        repo.delete_override(&ctx, "editorial", "Redator")
            .expect("delete");
        assert_eq!(repo.list_overrides(&ctx, "editorial").unwrap().len(), 1);
        repo.clear_overrides(&ctx, "editorial").expect("clear");
        assert!(repo.list_overrides(&ctx, "editorial").unwrap().is_empty());
    }

    // ISOLAMENTO: o mesmo (template, papel) tem override independente por
    // tenant; B não vê nem apaga o de A.
    {
        let mut repo = make();
        repo.set_override(&ctx_a(), "editorial", "Redator", "de A")
            .unwrap();
        repo.set_override(&ctx_b(), "editorial", "Redator", "de B")
            .unwrap();
        assert_eq!(
            repo.list_overrides(&ctx_a(), "editorial").unwrap()[0].prompt,
            "de A"
        );
        assert_eq!(
            repo.list_overrides(&ctx_b(), "editorial").unwrap()[0].prompt,
            "de B"
        );
        repo.clear_overrides(&ctx_b(), "editorial").unwrap();
        assert_eq!(
            repo.list_overrides(&ctx_a(), "editorial").unwrap().len(),
            1,
            "clear de B não toca A"
        );
    }

    // personas próprias: CRUD por tenant, ids não vazam entre tenants.
    {
        let mut repo = make();
        let id_a = repo
            .insert_custom(&ctx_a(), "editorial", "Ghost", "escreva")
            .expect("insert A");
        let id_b = repo
            .insert_custom(&ctx_b(), "editorial", "Ghost B", "escreva B")
            .expect("insert B");
        assert_eq!(repo.list_custom(&ctx_a(), "editorial").unwrap().len(), 1);
        repo.update_custom(&ctx_a(), id_a, "Ghost Sr", "melhor")
            .expect("update");
        assert_eq!(
            repo.list_custom(&ctx_a(), "editorial").unwrap()[0].nome,
            "Ghost Sr"
        );
        // B não atualiza nem apaga a persona de A pelo id.
        assert!(repo.update_custom(&ctx_b(), id_a, "x", "y").is_err());
        assert!(repo.delete_custom(&ctx_b(), id_a).is_err());
        repo.delete_custom(&ctx_a(), id_a).expect("delete A");
        assert!(repo.list_custom(&ctx_a(), "editorial").unwrap().is_empty());
        assert_eq!(
            repo.list_custom(&ctx_b(), "editorial").unwrap()[0].id,
            id_b,
            "a persona de B segue intacta"
        );
    }
}

/// Evento de domínio para os cenários do ledger — `actor` explícito porque
/// o contrato diz que o adapter NÃO reescreve `actor`/`ts` do evento.
pub fn evento_gate(ctx: &TenantContext, actor: &str, ts: &str) -> DomainEvent {
    DomainEvent {
        tenant: ctx.tenant,
        actor: ActorId::new(actor).unwrap(),
        ts: ts.into(),
        kind: DomainEventKind::GateApproved {
            task_id: TaskId::new(1),
            stage: Some("Gate do contrato".into()),
            gates_approved: 1,
        },
    }
}

/// Contrato do `LedgerRepository` (B3, ADR 0027): cadeias independentes por
/// tenant, verificáveis e exportáveis ISOLADAMENTE — os appends são
/// deliberadamente INTERCALADOS entre os dois tenants (é o teste dos "2
/// tenants concorrentes" da Definição de Pronto do plano; a INDEPENDÊNCIA
/// das cadeias é observável pela assinatura: seq 1..N por tenant, verify e
/// export cegos para o vizinho).
pub fn suite_ledger_repository<L: LedgerRepository>(mut make: impl FnMut() -> L) {
    // Appends intercalados: cada tenant numera a PRÓPRIA cadeia do 1.
    {
        let mut repo = make();
        assert_eq!(
            repo.append(&ctx_a(), &evento_gate(&ctx_a(), "contract:a", "t1"))
                .expect("append A#1"),
            1
        );
        assert_eq!(
            repo.append(&ctx_b(), &evento_gate(&ctx_b(), "contract:b", "t2"))
                .expect("append B#1 — a cadeia de B ignora a de A"),
            1
        );
        assert_eq!(
            repo.append(&ctx_a(), &evento_gate(&ctx_a(), "contract:a", "t3"))
                .expect("append A#2"),
            2
        );
        assert_eq!(
            repo.append(&ctx_b(), &evento_gate(&ctx_b(), "contract:b", "t4"))
                .expect("append B#2"),
            2
        );
        assert_eq!(
            repo.append(&ctx_a(), &evento_gate(&ctx_a(), "contract:a", "t5"))
                .expect("append A#3"),
            3
        );

        // verify: cada cadeia fecha sozinha, com a contagem própria.
        assert_eq!(repo.verify_chain(&ctx_a()).expect("verify A"), 3);
        assert_eq!(repo.verify_chain(&ctx_b()).expect("verify B"), 2);

        // export: a trilha completa do tenant, nada do vizinho.
        assert_eq!(repo.export(&ctx_a()).expect("export A").len(), 3);
        assert_eq!(repo.export(&ctx_b()).expect("export B").len(), 2);

        // recent: escopo do tenant + limite.
        assert_eq!(repo.recent(&ctx_a(), 10, None).expect("recent A").len(), 3);
        assert_eq!(repo.recent(&ctx_a(), 2, None).expect("limit").len(), 2);
        assert_eq!(repo.recent(&ctx_b(), 10, None).expect("recent B").len(), 2);
    }

    // Tenant sem nenhuma entrada: cadeia vazia é VÁLIDA (0), export/recent
    // vazios — indistinguível de tenant inexistente (fail-closed na leitura).
    {
        let mut repo = make();
        repo.append(&ctx_a(), &evento_gate(&ctx_a(), "contract:a", "t1"))
            .unwrap();
        assert_eq!(repo.verify_chain(&ctx_b()).expect("cadeia vazia"), 0);
        assert!(repo.export(&ctx_b()).unwrap().is_empty());
        assert!(repo.recent(&ctx_b(), 10, None).unwrap().is_empty());
    }

    // Filtro de actor combinado com o limite, dentro do tenant.
    {
        let mut repo = make();
        repo.append(&ctx_a(), &evento_gate(&ctx_a(), "humano", "t1"))
            .unwrap();
        for i in 0..4 {
            repo.append(
                &ctx_a(),
                &evento_gate(&ctx_a(), "robo", &format!("t{}", i + 2)),
            )
            .unwrap();
        }
        let humano = ActorId::new("humano").unwrap();
        let so_humano = repo
            .recent(&ctx_a(), 2, Some(&humano))
            .expect("filtro de actor");
        assert_eq!(
            so_humano.len(),
            1,
            "o filtro encontra o actor raro mesmo com limite pequeno"
        );
        // O actor de A não vaza na consulta de B.
        assert!(repo.recent(&ctx_b(), 10, Some(&humano)).unwrap().is_empty());
    }

    // Fail-closed na escrita: evento cujo tenant ≠ contexto é recusado —
    // e a cadeia do contexto NÃO cresce.
    {
        let mut repo = make();
        let err = repo
            .append(&ctx_a(), &evento_gate(&ctx_b(), "contract:b", "t1"))
            .expect_err("evento de outro tenant não entra");
        assert!(!err.to_string().is_empty());
        assert_eq!(repo.verify_chain(&ctx_a()).unwrap(), 0, "nada foi gravado");
        assert_eq!(repo.verify_chain(&ctx_b()).unwrap(), 0);
    }
}

/// Determinismo CROSS-ADAPTER do ledger (coordenada 4 da revisão do B4): os
/// MESMOS appends pelos dois adapters produzem a MESMA sequência de
/// `(seq, prev_hash, entry_hash)` — o corpo canônico é independente de
/// driver/banco, então a paridade local↔SaaS é CRIPTOGRÁFICA, não só
/// comportamental. `projeta` extrai a tripla do `Entry` do adapter: a suíte
/// continua só-domínio (não conhece `btv-schemas`), e os dois adapters
/// precisam compartilhar o MESMO tipo de entrada — se um dia divergirem, o
/// tipo deixa de unificar e ESTE teste deixa de compilar, que é o aviso
/// certo.
pub fn suite_ledger_determinismo_cross_adapter<E, A, B>(
    make_a: impl FnOnce() -> A,
    make_b: impl FnOnce() -> B,
    projeta: impl Fn(&E) -> (u64, String, String),
) where
    A: LedgerRepository<Entry = E>,
    B: LedgerRepository<Entry = E>,
{
    let mut a = make_a();
    let mut b = make_b();
    // Appends fixos e INTERCALADOS entre dois tenants (ts determinístico —
    // o hash depende do corpo inteiro, então qualquer relógio fabricado
    // aqui quebraria a comparação).
    let roteiro = [
        (ctx_a(), "2026-07-10T00:00:01Z"),
        (ctx_b(), "2026-07-10T00:00:02Z"),
        (ctx_a(), "2026-07-10T00:00:03Z"),
        (ctx_b(), "2026-07-10T00:00:04Z"),
        (ctx_a(), "2026-07-10T00:00:05Z"),
    ];
    for (ctx, ts) in &roteiro {
        let ev = evento_gate(ctx, ctx.actor.as_str(), ts);
        a.append(ctx, &ev).expect("append no adapter A");
        b.append(ctx, &ev).expect("append no adapter B");
    }
    for ctx in [ctx_a(), ctx_b()] {
        let de_a: Vec<_> = a
            .export(&ctx)
            .expect("export A")
            .iter()
            .map(&projeta)
            .collect();
        let de_b: Vec<_> = b
            .export(&ctx)
            .expect("export B")
            .iter()
            .map(&projeta)
            .collect();
        assert!(!de_a.is_empty(), "o roteiro gravou nas duas cadeias");
        assert_eq!(
            de_a, de_b,
            "os dois adapters divergiram na cadeia do tenant {} — o corpo \
             canônico deixou de ser independente de driver",
            ctx.tenant
        );
    }
}
