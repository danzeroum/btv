//! Event store append-only com concorrência otimista.
//!
//! Porte do `opencode-db`/`opencode-events` da branch `rust-migration` do
//! opencode (ADR 0002), adaptado ao Forge: mesmo schema
//! (`event_sequence` + `event`, índice único `(aggregate_id, seq)` como
//! base da concorrência otimista, WAL + pragmas por conexão), sem a
//! maquinaria de coexistência com o servidor TS — aqui o Rust é dono do
//! schema. A versão do evento vai embutida no `type` (convenção `nome.N`).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    /// Conflito de concorrência otimista: a head esperada não bate com a
    /// armazenada (equivale ao índice único `(aggregate_id, seq)` rejeitar).
    #[error(
        "conflito de concorrência em {aggregate_id}: esperava head {expected}, encontrou {found}"
    )]
    Conflict {
        aggregate_id: String,
        expected: i64,
        found: i64,
    },
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("erro de serialização: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Evento novo a anexar; `id`/`seq` são atribuídos pelo store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventInput {
    /// Tipo do evento, com a versão embutida (ex.: `message.1`).
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

impl EventInput {
    pub fn new(kind: impl Into<String>, data: Value) -> Self {
        Self {
            kind: kind.into(),
            data,
        }
    }
}

/// Evento persistido; `(aggregate_id, seq)` é único.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: String,
    pub aggregate_id: String,
    pub seq: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

/// Mesmo DDL do opencode (menos a preocupação de compat com o TS):
/// `IF NOT EXISTS` torna a criação idempotente.
const SCHEMA_DDL: &str = "\
CREATE TABLE IF NOT EXISTS event_sequence (
  aggregate_id TEXT PRIMARY KEY NOT NULL,
  seq INTEGER NOT NULL,
  owner_id TEXT
);
CREATE TABLE IF NOT EXISTS event (
  id TEXT PRIMARY KEY NOT NULL,
  aggregate_id TEXT NOT NULL REFERENCES event_sequence(aggregate_id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,
  type TEXT NOT NULL,
  data TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS event_aggregate_seq_idx ON event (aggregate_id, seq);
CREATE INDEX IF NOT EXISTS event_aggregate_type_seq_idx ON event (aggregate_id, type, seq);";

pub struct EventStore {
    conn: Connection,
}

impl EventStore {
    /// Abre (criando se preciso) o store em `path`, com os mesmos pragmas
    /// por conexão do opencode: WAL, synchronous NORMAL, busy timeout e
    /// foreign keys.
    pub fn open(path: &str) -> Result<Self, EventError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    /// Store efêmero em memória (sem WAL — não suportado em `:memory:`).
    pub fn open_in_memory() -> Result<Self, EventError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, EventError> {
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA_DDL)?;
        Ok(Self { conn })
    }

    /// Anexa `events` ao agregado exigindo que a head atual seja
    /// `expected_head` (0 para agregado novo). Retorna a nova head.
    pub fn append(
        &mut self,
        aggregate_id: &str,
        expected_head: i64,
        events: Vec<EventInput>,
    ) -> Result<i64, EventError> {
        let tx = self.conn.transaction()?;
        let found: i64 = tx
            .query_row(
                "SELECT seq FROM event_sequence WHERE aggregate_id = ?1",
                [aggregate_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if found != expected_head {
            return Err(EventError::Conflict {
                aggregate_id: aggregate_id.to_string(),
                expected: expected_head,
                found,
            });
        }

        // A linha de sequência entra primeiro: `event.aggregate_id` tem
        // FOREIGN KEY para `event_sequence`.
        let new_head = found + events.len() as i64;
        tx.execute(
            "INSERT INTO event_sequence (aggregate_id, seq) VALUES (?1, ?2)
             ON CONFLICT(aggregate_id) DO UPDATE SET seq = excluded.seq",
            params![aggregate_id, new_head],
        )?;
        let mut seq = found;
        for event in events {
            seq += 1;
            tx.execute(
                "INSERT INTO event (id, aggregate_id, seq, type, data) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event_id(aggregate_id, seq),
                    aggregate_id,
                    seq,
                    event.kind,
                    serde_json::to_string(&event.data)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(new_head)
    }

    /// Lê os eventos do agregado com `seq > from_seq`, em ordem.
    pub fn read(&self, aggregate_id: &str, from_seq: i64) -> Result<Vec<StoredEvent>, EventError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, seq, type, data FROM event
             WHERE aggregate_id = ?1 AND seq > ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![aggregate_id, from_seq], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (id, seq, kind, data) = row?;
            events.push(StoredEvent {
                id,
                aggregate_id: aggregate_id.to_string(),
                seq,
                kind,
                data: serde_json::from_str(&data)?,
            });
        }
        Ok(events)
    }

    /// Head atual do agregado (0 se não há eventos).
    pub fn head_seq(&self, aggregate_id: &str) -> Result<i64, EventError> {
        Ok(self
            .conn
            .query_row(
                "SELECT seq FROM event_sequence WHERE aggregate_id = ?1",
                [aggregate_id],
                |row| row.get(0),
            )
            .unwrap_or(0))
    }

    /// Lista os ids de agregados existentes (mais recentes primeiro).
    pub fn aggregates(&self) -> Result<Vec<String>, EventError> {
        let mut stmt = self
            .conn
            .prepare("SELECT aggregate_id FROM event_sequence ORDER BY rowid DESC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// Id único do evento: agregado tem seq única, então `aggregate+seq` é
/// globalmente único; o prefixo temporal preserva ordenação lexicográfica.
fn event_id(aggregate_id: &str, seq: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("evt_{nanos:024x}_{aggregate_id}_{seq}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn append_read_e_head() {
        let mut store = EventStore::open_in_memory().unwrap();
        let head = store
            .append(
                "ses_1",
                0,
                vec![
                    EventInput::new("session.started.1", json!({"task": "t"})),
                    EventInput::new("message.1", json!({"role": "user"})),
                ],
            )
            .unwrap();
        assert_eq!(head, 2);
        assert_eq!(store.head_seq("ses_1").unwrap(), 2);

        let events = store.read("ses_1", 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "session.started.1");
        assert_eq!(events[1].seq, 2);

        // leitura incremental
        assert_eq!(store.read("ses_1", 1).unwrap().len(), 1);
    }

    #[test]
    fn head_divergente_e_conflito() {
        let mut store = EventStore::open_in_memory().unwrap();
        store
            .append("ses_1", 0, vec![EventInput::new("a.1", json!({}))])
            .unwrap();
        let err = store
            .append("ses_1", 0, vec![EventInput::new("b.1", json!({}))])
            .unwrap_err();
        assert!(matches!(
            err,
            EventError::Conflict {
                expected: 0,
                found: 1,
                ..
            }
        ));
    }

    #[test]
    fn sobrevive_a_reabertura_do_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.db");
        let path = path.to_str().unwrap();
        {
            let mut store = EventStore::open(path).unwrap();
            store
                .append("ses_1", 0, vec![EventInput::new("a.1", json!({"n": 1}))])
                .unwrap();
        }
        let store = EventStore::open(path).unwrap();
        assert_eq!(store.head_seq("ses_1").unwrap(), 1);
        assert_eq!(store.read("ses_1", 0).unwrap()[0].data["n"], 1);
        assert_eq!(store.aggregates().unwrap(), vec!["ses_1".to_string()]);
    }

    #[test]
    fn agregados_sao_independentes() {
        let mut store = EventStore::open_in_memory().unwrap();
        store
            .append("a", 0, vec![EventInput::new("x.1", json!({}))])
            .unwrap();
        store
            .append("b", 0, vec![EventInput::new("y.1", json!({}))])
            .unwrap();
        assert_eq!(store.head_seq("a").unwrap(), 1);
        assert_eq!(store.read("b", 0).unwrap().len(), 1);
    }
}
