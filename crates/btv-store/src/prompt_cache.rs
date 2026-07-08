//! Cache de respostas de LLM por hash de request (`prompt-cache-key.v1`).
//!
//! Origem: prompte — requests idênticos não voltam à rede. A chave é o
//! sha256 do JSON canônico do request (`btv_schemas::request_hash`); o
//! valor é o turno serializado. Escrita é idempotente (INSERT OR REPLACE).

use rusqlite::{params, Connection};

pub struct PromptCache {
    conn: Connection,
}

impl PromptCache {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prompt_cache (
                hash       TEXT PRIMARY KEY,
                response   TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn get(&self, hash: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT response FROM prompt_cache WHERE hash = ?1",
                [hash],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
    }

    pub fn put(&self, hash: &str, response: &str, created_at: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO prompt_cache (hash, response, created_at) VALUES (?1, ?2, ?3)",
            params![hash, response, created_at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_e_miss() {
        let cache = PromptCache::open_in_memory().unwrap();
        assert_eq!(cache.get("h1").unwrap(), None);
        cache
            .put("h1", r#"{"x":1}"#, "2026-07-05T00:00:00Z")
            .unwrap();
        assert_eq!(cache.get("h1").unwrap().as_deref(), Some(r#"{"x":1}"#));
        // sobrescrita idempotente
        cache
            .put("h1", r#"{"x":2}"#, "2026-07-05T00:00:01Z")
            .unwrap();
        assert_eq!(cache.get("h1").unwrap().as_deref(), Some(r#"{"x":2}"#));
    }
}
