//! Sessões duráveis (Fase 2): a conversa é um agregado de eventos.
//!
//! Cada `ChatMessage` vira um evento `message.1` no `EventStore` (portado
//! da branch `rust-migration` do opencode — ADR 0002); reabrir a sessão
//! reconstrói o histórico por replay. A concorrência otimista da head
//! detecta dois processos escrevendo na mesma sessão. Context Epochs e
//! compaction em fronteiras seguras entram na sequência da Fase 2.

use btv_domain::chat::ChatMessage;
use btv_domain::event::EventInput;
use btv_domain::ports::{EventStorePort, RepositoryError};
use btv_domain::TenantContext;
use serde_json::json;

/// Atalho: a sessão precisa nomear os tipos de dado do port (constrói
/// `EventInput`, replaya `StoredEvent` do domínio).
pub trait SessionStore:
    EventStorePort<NewEvent = EventInput, StoredEvent = btv_domain::event::StoredEvent>
{
}
impl<S> SessionStore for S where
    S: EventStorePort<NewEvent = EventInput, StoredEvent = btv_domain::event::StoredEvent>
{
}

pub const SESSION_STARTED: &str = "session.started.1";
pub const MESSAGE: &str = "message.1";
pub const EPOCH_STARTED: &str = "epoch.started.1";

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("event store: {0}")]
    Store(#[from] RepositoryError),
    #[error("evento malformado na sessão {session_id} (seq {seq}): {reason}")]
    Malformed {
        session_id: String,
        seq: i64,
        reason: String,
    },
}

/// Sessão durável: histórico reconstruído por replay + head para
/// concorrência otimista nos appends.
pub struct DurableSession<S: SessionStore> {
    store: S,
    /// Dono da sessão — repassado ao port em toda operação (D1t: a sessão
    /// declara em nome de quem opera; no CLI local é `TenantContext::local`).
    ctx: TenantContext,
    pub session_id: String,
    /// Histórico corrente (replay + turnos desta execução).
    pub messages: Vec<ChatMessage>,
    /// Head do agregado no store (nº do último evento persistido).
    head: i64,
    /// Quantas mensagens do histórico já estão persistidas.
    persisted: usize,
    /// Época atual (incrementada a cada compaction).
    epoch: usize,
}

impl<S: SessionStore> DurableSession<S> {
    /// Abre (ou cria) a sessão `session_id`, reconstruindo o histórico.
    pub fn open(
        store: S,
        ctx: TenantContext,
        session_id: &str,
        task_hint: &str,
        model: &str,
    ) -> Result<Self, SessionError> {
        let head = store.head_seq(&ctx, session_id)?;
        let mut messages = Vec::new();
        if head == 0 {
            let mut store = store;
            let head = store.append(
                &ctx,
                session_id,
                0,
                vec![EventInput::new(
                    SESSION_STARTED,
                    json!({"task": task_hint, "model": model}),
                )],
            )?;
            return Ok(Self {
                store,
                ctx,
                session_id: session_id.to_string(),
                messages,
                head,
                persisted: 0,
                epoch: 0,
            });
        }
        let mut epoch = 0usize;
        for event in store.read(&ctx, session_id, 0)? {
            match event.kind.as_str() {
                MESSAGE => {
                    let message: ChatMessage = serde_json::from_value(event.data).map_err(|e| {
                        SessionError::Malformed {
                            session_id: session_id.to_string(),
                            seq: event.seq,
                            reason: e.to_string(),
                        }
                    })?;
                    messages.push(message);
                }
                // Nova época: o que veio antes foi resumido — o replay
                // recomeça do resumo (baseline da época).
                EPOCH_STARTED => {
                    epoch += 1;
                    messages.clear();
                }
                _ => {}
            }
        }
        let persisted = messages.len();
        Ok(Self {
            store,
            ctx,
            session_id: session_id.to_string(),
            messages,
            head,
            persisted,
            epoch,
        })
    }

    /// Inicia uma nova época: grava `epoch.started.1` com o resumo e troca
    /// o histórico em memória pela baseline resumida — atomicamente (os
    /// dois eventos entram no mesmo append). Só chame em fronteira segura
    /// ([`crate::compaction::CompactionPolicy::is_safe_boundary`]).
    pub fn compact(&mut self, summary: &str) -> Result<(), SessionError> {
        let baseline = ChatMessage::user_text(format!(
            "[Contexto resumido da conversa anterior]\n{summary}"
        ));
        let baseline_event =
            serde_json::to_value(&baseline).map_err(|e| SessionError::Malformed {
                session_id: self.session_id.clone(),
                seq: self.head,
                reason: e.to_string(),
            })?;
        self.head = self.store.append(
            &self.ctx,
            &self.session_id,
            self.head,
            vec![
                EventInput::new(EPOCH_STARTED, json!({"summary": summary})),
                EventInput::new(MESSAGE, baseline_event),
            ],
        )?;
        self.epoch += 1;
        self.messages = vec![baseline];
        self.persisted = 1;
        Ok(())
    }

    /// Época atual (0 = nunca compactada).
    pub fn epoch(&self) -> usize {
        self.epoch
    }

    /// Persiste as mensagens novas do histórico (as além de `persisted`),
    /// com concorrência otimista sobre a head.
    pub fn persist_new(&mut self) -> Result<usize, SessionError> {
        let new: Vec<EventInput> = self.messages[self.persisted..]
            .iter()
            .map(|m| Ok(EventInput::new(MESSAGE, serde_json::to_value(m)?)))
            .collect::<Result<_, serde_json::Error>>()
            .map_err(|e| SessionError::Malformed {
                session_id: self.session_id.clone(),
                seq: self.head,
                reason: e.to_string(),
            })?;
        if new.is_empty() {
            return Ok(0);
        }
        let count = new.len();
        self.head = self
            .store
            .append(&self.ctx, &self.session_id, self.head, new)?;
        self.persisted = self.messages.len();
        Ok(count)
    }

    /// Quantas mensagens vieram do replay ao abrir.
    pub fn resumed_messages(&self) -> usize {
        self.persisted
    }
}

#[cfg(test)]
mod tests {
    //! Unit da sessão com um event store EM MEMÓRIA (mock puro do
    //! `EventStorePort` — DoD do D1t: sem SQLite). A composição com o
    //! driver real continua provada em
    //! `btv-store/tests/sessao_durable_replay.rs` (movida no D1t).

    use super::*;
    use btv_domain::chat::{ContentBlock, Role};
    use btv_domain::event::StoredEvent;
    use btv_domain::{ActorId, TenantId};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Mock em memória do port — estado COMPARTILHÁVEL entre instâncias
    /// (clones veem o mesmo mapa), para o teste de conflito reproduzir dois
    /// escritores na mesma sessão como o SQLite real faria.
    #[derive(Clone, Default)]
    struct MemStore {
        eventos: Arc<Mutex<HashMap<String, Vec<StoredEvent>>>>,
    }

    impl EventStorePort for MemStore {
        type NewEvent = EventInput;
        type StoredEvent = StoredEvent;

        fn append(
            &mut self,
            _ctx: &TenantContext,
            aggregate_id: &str,
            expected_head: i64,
            events: Vec<EventInput>,
        ) -> Result<i64, RepositoryError> {
            let mut mapa = self.eventos.lock().unwrap();
            let fila = mapa.entry(aggregate_id.to_string()).or_default();
            let found = fila.len() as i64;
            if found != expected_head {
                return Err(RepositoryError::ConcurrencyConflict {
                    expected: expected_head,
                    found,
                });
            }
            for e in events {
                let seq = fila.len() as i64 + 1;
                fila.push(StoredEvent {
                    id: format!("evt_{seq}"),
                    aggregate_id: aggregate_id.to_string(),
                    seq,
                    kind: e.kind,
                    data: e.data,
                });
            }
            Ok(fila.len() as i64)
        }

        fn read(
            &self,
            _ctx: &TenantContext,
            aggregate_id: &str,
            from_seq: i64,
        ) -> Result<Vec<StoredEvent>, RepositoryError> {
            Ok(self
                .eventos
                .lock()
                .unwrap()
                .get(aggregate_id)
                .map(|f| f.iter().filter(|e| e.seq > from_seq).cloned().collect())
                .unwrap_or_default())
        }

        fn head_seq(
            &self,
            _ctx: &TenantContext,
            aggregate_id: &str,
        ) -> Result<i64, RepositoryError> {
            Ok(self
                .eventos
                .lock()
                .unwrap()
                .get(aggregate_id)
                .map(|f| f.len() as i64)
                .unwrap_or(0))
        }
    }

    fn ctx() -> TenantContext {
        TenantContext::new(TenantId::LOCAL, ActorId::new("test:core").unwrap())
    }

    #[test]
    fn sessao_sobrevive_a_reabertura() {
        let store = MemStore::default();

        {
            let mut s = DurableSession::open(store.clone(), ctx(), "ses_1", "tarefa", "m").unwrap();
            assert_eq!(s.resumed_messages(), 0);
            s.messages.push(ChatMessage::user_text("primeira"));
            s.messages.push(ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "resposta".into(),
                }],
            });
            assert_eq!(s.persist_new().unwrap(), 2);
            assert_eq!(s.persist_new().unwrap(), 0); // idempotente
        }

        let s = DurableSession::open(store, ctx(), "ses_1", "tarefa", "m").unwrap();
        assert_eq!(s.resumed_messages(), 2);
        assert!(matches!(s.messages[0].role, Role::User));
        assert!(matches!(s.messages[1].role, Role::Assistant));
    }

    #[test]
    fn escritor_concorrente_gera_conflito() {
        let store = MemStore::default();
        let mut a = DurableSession::open(store.clone(), ctx(), "ses_1", "t", "m").unwrap();
        let mut b = DurableSession::open(store, ctx(), "ses_1", "t", "m").unwrap();

        a.messages.push(ChatMessage::user_text("de A"));
        a.persist_new().unwrap();

        b.messages.push(ChatMessage::user_text("de B"));
        let err = b.persist_new().unwrap_err();
        assert!(matches!(
            err,
            SessionError::Store(RepositoryError::ConcurrencyConflict { .. })
        ));
    }

    #[test]
    fn compaction_inicia_nova_epoca_e_replay_parte_do_resumo() {
        let store = MemStore::default();
        {
            let mut s = DurableSession::open(store.clone(), ctx(), "ses_1", "t", "m").unwrap();
            s.messages.push(ChatMessage::user_text("pergunta longa"));
            s.messages.push(ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "resposta longa".into(),
                }],
            });
            s.persist_new().unwrap();

            s.compact("objetivo X; arquivo f.rs editado; pendência Y")
                .unwrap();
            assert_eq!(s.epoch(), 1);
            assert_eq!(s.messages.len(), 1, "histórico vira só a baseline");

            s.messages.push(ChatMessage::user_text("continua"));
            s.persist_new().unwrap();
        }

        let s = DurableSession::open(store, ctx(), "ses_1", "t", "m").unwrap();
        assert_eq!(s.epoch(), 1);
        assert_eq!(s.resumed_messages(), 2);
        assert!(matches!(
            &s.messages[0].content[0],
            ContentBlock::Text { text } if text.contains("Contexto resumido")
        ));
    }

    #[test]
    fn tool_use_e_tool_result_sobrevivem_ao_replay() {
        let store = MemStore::default();
        {
            let mut s = DurableSession::open(store.clone(), ctx(), "ses_1", "t", "m").unwrap();
            s.messages.push(ChatMessage {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "read".into(),
                    input: json!({"path": "f.txt"}),
                }],
            });
            s.messages.push(ChatMessage {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "tu1".into(),
                    content: "1\tx".into(),
                    is_error: false,
                }],
            });
            s.persist_new().unwrap();
        }

        let s = DurableSession::open(store, ctx(), "ses_1", "t", "m").unwrap();
        assert!(matches!(
            &s.messages[0].content[0],
            ContentBlock::ToolUse { name, .. } if name == "read"
        ));
        assert!(matches!(
            &s.messages[1].content[0],
            ContentBlock::ToolResult {
                is_error: false,
                ..
            }
        ));
    }
}
