//! Persistência do produto BuildToValue (`.forge/btv.db`) — Onda 3 do plano
//! BTV: **runs** (squads ativadas pela galeria/wizard, U3/U6). Ondas
//! seguintes somam entregas (U4), overrides de persona (U7) e perfis locais
//! (A6) no mesmo arquivo.
//!
//! Puramente armazenamento (regra do crate): a orquestração vive em
//! `forge-cli::btv_agent`/`squad_agent`; o ledger de auditoria continua sendo
//! o `LedgerStore` — este store guarda estado consultável, não trilha
//! imutável.

use rusqlite::{params, Connection};

#[derive(Debug, thiserror::Error)]
pub enum BtvStoreError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
}

/// Uma squad ativada (execução) — linha de "Minhas squads" (U6) e âncora da
/// tela Ao vivo (U3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BtvRun {
    pub id: i64,
    pub task_id: String,
    pub template_id: String,
    pub template_versao: String,
    pub nome: String,
    /// Respostas do briefing (JSON: `[{label, resposta}]`).
    pub briefing_json: String,
    /// Papéis ativos (JSON: `["Pauteiro", ...]` — já sem os desligados).
    pub papeis_json: String,
    /// `ativa` | `concluida` | `encerrada` | `erro`.
    pub status: String,
    pub created_ts: String,
    pub updated_ts: String,
}

pub struct BtvStore {
    conn: Connection,
}

impl BtvStore {
    pub fn open(path: &str) -> Result<Self, BtvStoreError> {
        let conn = Connection::open(path)?;
        // WAL: CLI e dashboard podem tocar o mesmo arquivo (mesma lição do
        // LedgerStore, fechada na Fase 7 Onda 6).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, BtvStoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, BtvStoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL UNIQUE,
                template_id TEXT NOT NULL,
                template_versao TEXT NOT NULL,
                nome TEXT NOT NULL,
                briefing_json TEXT NOT NULL,
                papeis_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_ts TEXT NOT NULL,
                updated_ts TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_run(
        &self,
        task_id: &str,
        template_id: &str,
        template_versao: &str,
        nome: &str,
        briefing_json: &str,
        papeis_json: &str,
        now: &str,
    ) -> Result<i64, BtvStoreError> {
        self.conn.execute(
            "INSERT INTO runs (task_id, template_id, template_versao, nome, briefing_json,
                               papeis_json, status, created_ts, updated_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ativa', ?7, ?7)",
            params![
                task_id,
                template_id,
                template_versao,
                nome,
                briefing_json,
                papeis_json,
                now
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Transição de status ao fim da execução (`concluida`/`erro`) ou no
    /// kill-switch (`encerrada`). Silencioso para task_id desconhecido — o
    /// watcher pode sobreviver a um run apagado.
    pub fn set_status(&self, task_id: &str, status: &str, now: &str) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE runs SET status = ?2, updated_ts = ?3 WHERE task_id = ?1",
            params![task_id, status, now],
        )?;
        Ok(())
    }

    /// Runs mais recentes primeiro (a squad em execução aparece no topo de
    /// U6 porque é a mais nova com status `ativa`).
    pub fn list_runs(&self) -> Result<Vec<BtvRun>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, template_id, template_versao, nome, briefing_json,
                    papeis_json, status, created_ts, updated_ts
             FROM runs ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BtvRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                template_id: row.get(2)?,
                template_versao: row.get(3)?,
                nome: row.get(4)?,
                briefing_json: row.get(5)?,
                papeis_json: row.get(6)?,
                status: row.get(7)?,
                created_ts: row.get(8)?,
                updated_ts: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_lista_e_transiciona_status() {
        let store = BtvStore::open_in_memory().unwrap();
        let id = store
            .insert_run(
                "sq1",
                "editorial",
                "v1.4",
                "Newsletter de julho",
                r#"[{"label":"Pauta","resposta":"logística verde"}]"#,
                r#"["Pauteiro","Redator"]"#,
                "2026-07-08T00:00:00Z",
            )
            .unwrap();
        assert!(id > 0);
        let runs = store.list_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "ativa");
        assert_eq!(runs[0].template_id, "editorial");

        store
            .set_status("sq1", "concluida", "2026-07-08T00:10:00Z")
            .unwrap();
        let runs = store.list_runs().unwrap();
        assert_eq!(runs[0].status, "concluida");
        assert_eq!(runs[0].updated_ts, "2026-07-08T00:10:00Z");
    }

    #[test]
    fn lista_mais_recente_primeiro() {
        let store = BtvStore::open_in_memory().unwrap();
        for (task, nome) in [("sq1", "antiga"), ("sq2", "nova")] {
            store
                .insert_run(task, "bi", "v2.1", nome, "[]", "[]", "2026-07-08T00:00:00Z")
                .unwrap();
        }
        let runs = store.list_runs().unwrap();
        assert_eq!(runs[0].nome, "nova");
        assert_eq!(runs[1].nome, "antiga");
    }

    #[test]
    fn wal_ligado_em_arquivo_real() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("btv.db");
        let store = BtvStore::open(path.to_str().unwrap()).unwrap();
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
