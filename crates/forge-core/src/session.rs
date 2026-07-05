//! Sessões duráveis (Fase 2): a conversa é um agregado de eventos.
//!
//! Cada `ChatMessage` vira um evento `message.1` no `EventStore` (portado
//! da branch `rust-migration` do opencode — ADR 0002); reabrir a sessão
//! reconstrói o histórico por replay. A concorrência otimista da head
//! detecta dois processos escrevendo na mesma sessão. Context Epochs e
//! compaction em fronteiras seguras entram na sequência da Fase 2.

use forge_llm::chat::ChatMessage;
use forge_store::{EventError, EventInput, EventStore};
use serde_json::json;

pub const SESSION_STARTED: &str = "session.started.1";
pub const MESSAGE: &str = "message.1";

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("event store: {0}")]
    Store(#[from] EventError),
    #[error("evento malformado na sessão {session_id} (seq {seq}): {reason}")]
    Malformed {
        session_id: String,
        seq: i64,
        reason: String,
    },
}

/// Sessão durável: histórico reconstruído por replay + head para
/// concorrência otimista nos appends.
pub struct DurableSession {
    store: EventStore,
    pub session_id: String,
    /// Histórico corrente (replay + turnos desta execução).
    pub messages: Vec<ChatMessage>,
    /// Head do agregado no store (nº do último evento persistido).
    head: i64,
    /// Quantas mensagens do histórico já estão persistidas.
    persisted: usize,
}

impl DurableSession {
    /// Abre (ou cria) a sessão `session_id`, reconstruindo o histórico.
    pub fn open(
        store: EventStore,
        session_id: &str,
        task_hint: &str,
        model: &str,
    ) -> Result<Self, SessionError> {
        let head = store.head_seq(session_id)?;
        let mut messages = Vec::new();
        if head == 0 {
            let mut store = store;
            let head = store.append(
                session_id,
                0,
                vec![EventInput::new(
                    SESSION_STARTED,
                    json!({"task": task_hint, "model": model}),
                )],
            )?;
            return Ok(Self {
                store,
                session_id: session_id.to_string(),
                messages,
                head,
                persisted: 0,
            });
        }
        for event in store.read(session_id, 0)? {
            if event.kind == MESSAGE {
                let message: ChatMessage =
                    serde_json::from_value(event.data).map_err(|e| SessionError::Malformed {
                        session_id: session_id.to_string(),
                        seq: event.seq,
                        reason: e.to_string(),
                    })?;
                messages.push(message);
            }
        }
        let persisted = messages.len();
        Ok(Self {
            store,
            session_id: session_id.to_string(),
            messages,
            head,
            persisted,
        })
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
        self.head = self.store.append(&self.session_id, self.head, new)?;
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
    use super::*;
    use forge_llm::chat::{ContentBlock, Role};

    fn store_at(path: &str) -> EventStore {
        EventStore::open(path).unwrap()
    }

    #[test]
    fn sessao_sobrevive_a_reabertura() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.db");
        let path = path.to_str().unwrap();

        {
            let mut s = DurableSession::open(store_at(path), "ses_1", "tarefa", "m").unwrap();
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

        let s = DurableSession::open(store_at(path), "ses_1", "tarefa", "m").unwrap();
        assert_eq!(s.resumed_messages(), 2);
        assert!(matches!(s.messages[0].role, Role::User));
        assert!(matches!(s.messages[1].role, Role::Assistant));
        assert_eq!(
            s.messages[1].content.len(),
            1,
            "blocos de conteúdo preservados no replay"
        );
    }

    #[test]
    fn escritor_concorrente_gera_conflito() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.db");
        let path = path.to_str().unwrap();

        let mut a = DurableSession::open(store_at(path), "ses_1", "t", "m").unwrap();
        let mut b = DurableSession::open(store_at(path), "ses_1", "t", "m").unwrap();

        a.messages.push(ChatMessage::user_text("de A"));
        a.persist_new().unwrap();

        b.messages.push(ChatMessage::user_text("de B"));
        let err = b.persist_new().unwrap_err();
        assert!(matches!(
            err,
            SessionError::Store(EventError::Conflict { .. })
        ));
    }

    #[test]
    fn tool_use_e_tool_result_sobrevivem_ao_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.db");
        let path = path.to_str().unwrap();

        {
            let mut s = DurableSession::open(store_at(path), "ses_1", "t", "m").unwrap();
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

        let s = DurableSession::open(store_at(path), "ses_1", "t", "m").unwrap();
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
