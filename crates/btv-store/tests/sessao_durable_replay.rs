//! D1t: a composição sessão durável × driver REAL (SQLite via
//! `EventStorePort`) — o unit da sessão vive em `btv-core` com mock; aqui
//! provamos que o adapter (`impl EventStorePort for EventStore`) sustenta
//! replay, conflito otimista e o fail-closed de tenant do modo LOCAL.

use btv_core::{DurableSession, SessionError};
use btv_domain::chat::ChatMessage;
use btv_domain::ports::RepositoryError;
use btv_domain::{ActorId, TenantContext, TenantId};
use btv_store::EventStore;

fn ctx_local() -> TenantContext {
    TenantContext::local(ActorId::new("test:store").unwrap())
}

#[test]
fn sessao_sobre_sqlite_real_sobrevive_a_reabertura_e_detecta_conflito() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.db");
    let path = path.to_str().unwrap();

    {
        let mut s = DurableSession::open(
            EventStore::open(path).unwrap(),
            ctx_local(),
            "ses_1",
            "tarefa",
            "m",
        )
        .unwrap();
        s.messages.push(ChatMessage::user_text("primeira"));
        assert_eq!(s.persist_new().unwrap(), 1);
    }

    let mut a = DurableSession::open(
        EventStore::open(path).unwrap(),
        ctx_local(),
        "ses_1",
        "t",
        "m",
    )
    .unwrap();
    assert_eq!(a.resumed_messages(), 1, "replay do arquivo real");

    // Conflito otimista entre duas conexões reais ao mesmo arquivo.
    let mut b = DurableSession::open(
        EventStore::open(path).unwrap(),
        ctx_local(),
        "ses_1",
        "t",
        "m",
    )
    .unwrap();
    a.messages.push(ChatMessage::user_text("de A"));
    a.persist_new().unwrap();
    b.messages.push(ChatMessage::user_text("de B"));
    assert!(matches!(
        b.persist_new().unwrap_err(),
        SessionError::Store(RepositoryError::ConcurrencyConflict { .. })
    ));
}

#[test]
fn event_store_local_recusa_contexto_de_outro_tenant() {
    // O adapter é o do modo LOCAL (sem coluna de tenant): contexto de outro
    // tenant é recusado fail-closed — nunca aceito fingindo isolamento.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.db");
    let outro = TenantContext::new(
        TenantId::parse("00000000-0000-0000-0000-0000000000d1").unwrap(),
        ActorId::new("test:intruso").unwrap(),
    );
    let aberto = DurableSession::open(
        EventStore::open(path.to_str().unwrap()).unwrap(),
        outro,
        "ses_1",
        "t",
        "m",
    );
    let Err(err) = aberto else {
        panic!("contexto de outro tenant deveria ser recusado")
    };
    assert!(err.to_string().contains("fail-closed"));
}
