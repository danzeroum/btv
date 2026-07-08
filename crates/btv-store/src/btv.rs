//! Persistência do produto BuildToValue (`.btv/btv.db`): **runs** (squads
//! ativadas, U3/U6), **entregas** (artefatos reais gravados pelas
//! ferramentas do squad, U4), **personas** (overrides de prompt por
//! modelo+papel e personas próprias, U7). Perfis locais (A6) entram na
//! onda do admin.
//!
//! Puramente armazenamento (regra do crate): a orquestração vive em
//! `btv-cli::btv_agent`/`squad_agent`; o ledger de auditoria continua sendo
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
    /// Quantos gates humanos já foram aprovados neste run (trilha de U4).
    pub gates_aprovados: i64,
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
                gates_aprovados INTEGER NOT NULL DEFAULT 0,
                created_ts TEXT NOT NULL,
                updated_ts TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS deliverables (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                template_id TEXT NOT NULL,
                nome TEXT NOT NULL,
                path TEXT NOT NULL,
                formato TEXT NOT NULL,
                versao TEXT NOT NULL,
                trilha TEXT NOT NULL,
                created_ts TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS persona_overrides (
                template_id TEXT NOT NULL,
                papel TEXT NOT NULL,
                prompt TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                PRIMARY KEY (template_id, papel)
            );
            CREATE TABLE IF NOT EXISTS template_pub (
                template_id TEXT PRIMARY KEY,
                publicado INTEGER NOT NULL,
                updated_ts TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nome TEXT NOT NULL,
                email TEXT NOT NULL,
                papel TEXT NOT NULL,
                ativo INTEGER NOT NULL DEFAULT 1,
                created_ts TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS custom_personas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id TEXT NOT NULL,
                nome TEXT NOT NULL,
                prompt TEXT NOT NULL,
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
                    papeis_json, status, gates_aprovados, created_ts, updated_ts
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
                gates_aprovados: row.get(8)?,
                created_ts: row.get(9)?,
                updated_ts: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    /// Incrementa a contagem de gates aprovados do run (chamado pelo
    /// handler de gate do BTV — compõe a trilha de procedência de U4).
    pub fn increment_gates(&self, task_id: &str, now: &str) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE runs SET gates_aprovados = gates_aprovados + 1, updated_ts = ?2 WHERE task_id = ?1",
            params![task_id, now],
        )?;
        Ok(())
    }

    pub fn get_run_by_task(&self, task_id: &str) -> Result<Option<BtvRun>, BtvStoreError> {
        Ok(self.list_runs()?.into_iter().find(|r| r.task_id == task_id))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_deliverable(
        &self,
        run_id: i64,
        task_id: &str,
        template_id: &str,
        nome: &str,
        path: &str,
        formato: &str,
        versao: &str,
        trilha: &str,
        now: &str,
    ) -> Result<i64, BtvStoreError> {
        self.conn.execute(
            "INSERT INTO deliverables (run_id, task_id, template_id, nome, path, formato, versao, trilha, created_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![run_id, task_id, template_id, nome, path, formato, versao, trilha, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_deliverables(&self) -> Result<Vec<BtvDeliverable>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, task_id, template_id, nome, path, formato, versao, trilha, created_ts
             FROM deliverables ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BtvDeliverable {
                id: row.get(0)?,
                run_id: row.get(1)?,
                task_id: row.get(2)?,
                template_id: row.get(3)?,
                nome: row.get(4)?,
                path: row.get(5)?,
                formato: row.get(6)?,
                versao: row.get(7)?,
                trilha: row.get(8)?,
                created_ts: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_deliverable(&self, id: i64) -> Result<Option<BtvDeliverable>, BtvStoreError> {
        Ok(self.list_deliverables()?.into_iter().find(|d| d.id == id))
    }

    // ── personas (U7): override de prompt por modelo+papel + personas próprias ──

    pub fn set_persona_override(
        &self,
        template_id: &str,
        papel: &str,
        prompt: &str,
        now: &str,
    ) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "INSERT INTO persona_overrides (template_id, papel, prompt, updated_ts)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(template_id, papel) DO UPDATE SET prompt = ?3, updated_ts = ?4",
            params![template_id, papel, prompt, now],
        )?;
        Ok(())
    }

    pub fn delete_persona_override(
        &self,
        template_id: &str,
        papel: &str,
    ) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "DELETE FROM persona_overrides WHERE template_id = ?1 AND papel = ?2",
            params![template_id, papel],
        )?;
        Ok(())
    }

    pub fn clear_persona_overrides(&self, template_id: &str) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "DELETE FROM persona_overrides WHERE template_id = ?1",
            params![template_id],
        )?;
        Ok(())
    }

    pub fn list_persona_overrides(
        &self,
        template_id: &str,
    ) -> Result<Vec<PersonaOverride>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT template_id, papel, prompt FROM persona_overrides WHERE template_id = ?1",
        )?;
        let rows = stmt.query_map(params![template_id], |row| {
            Ok(PersonaOverride {
                template_id: row.get(0)?,
                papel: row.get(1)?,
                prompt: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn insert_custom_persona(
        &self,
        template_id: &str,
        nome: &str,
        prompt: &str,
        now: &str,
    ) -> Result<i64, BtvStoreError> {
        self.conn.execute(
            "INSERT INTO custom_personas (template_id, nome, prompt, updated_ts) VALUES (?1, ?2, ?3, ?4)",
            params![template_id, nome, prompt, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_custom_persona(
        &self,
        id: i64,
        nome: &str,
        prompt: &str,
        now: &str,
    ) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE custom_personas SET nome = ?2, prompt = ?3, updated_ts = ?4 WHERE id = ?1",
            params![id, nome, prompt, now],
        )?;
        Ok(())
    }

    pub fn delete_custom_persona(&self, id: i64) -> Result<(), BtvStoreError> {
        self.conn
            .execute("DELETE FROM custom_personas WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_custom_personas(
        &self,
        template_id: &str,
    ) -> Result<Vec<CustomPersona>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, template_id, nome, prompt FROM custom_personas WHERE template_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![template_id], |row| {
            Ok(CustomPersona {
                id: row.get(0)?,
                template_id: row.get(1)?,
                nome: row.get(2)?,
                prompt: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    // ── A5: publicação de templates (override sobre o `publicado` embutido) ──

    pub fn set_template_publicado(
        &self,
        template_id: &str,
        publicado: bool,
        now: &str,
    ) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "INSERT INTO template_pub (template_id, publicado, updated_ts) VALUES (?1, ?2, ?3)
             ON CONFLICT(template_id) DO UPDATE SET publicado = ?2, updated_ts = ?3",
            params![template_id, publicado as i64, now],
        )?;
        Ok(())
    }

    pub fn list_template_pub(&self) -> Result<Vec<(String, bool)>, BtvStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT template_id, publicado FROM template_pub")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ── A6: perfis locais (sem senha/auth — atribuição de ator, local-first) ──

    pub fn insert_user(
        &self,
        nome: &str,
        email: &str,
        papel: &str,
        now: &str,
    ) -> Result<i64, BtvStoreError> {
        self.conn.execute(
            "INSERT INTO users (nome, email, papel, ativo, created_ts) VALUES (?1, ?2, ?3, 1, ?4)",
            params![nome, email, papel, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_user_ativo(&self, id: i64, ativo: bool) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE users SET ativo = ?2 WHERE id = ?1",
            params![id, ativo as i64],
        )?;
        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<BtvUser>, BtvStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, nome, email, papel, ativo FROM users ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(BtvUser {
                id: row.get(0)?,
                nome: row.get(1)?,
                email: row.get(2)?,
                papel: row.get(3)?,
                ativo: row.get::<_, i64>(4)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// Perfil local (A6): identidade nomeada para atribuição — SEM autenticação
/// (local-first, 127.0.0.1). Auth real é trabalho futuro explícito.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BtvUser {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub papel: String,
    pub ativo: bool,
}

/// Artefato exportado — linha da Biblioteca de entregas (U4), com trilha de
/// procedência real (papéis do run + gates aprovados) e o caminho do arquivo
/// REAL gravado pelas ferramentas do squad.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BtvDeliverable {
    pub id: i64,
    pub run_id: i64,
    pub task_id: String,
    pub template_id: String,
    pub nome: String,
    pub path: String,
    pub formato: String,
    pub versao: String,
    pub trilha: String,
    pub created_ts: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PersonaOverride {
    pub template_id: String,
    pub papel: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CustomPersona {
    pub id: i64,
    pub template_id: String,
    pub nome: String,
    pub prompt: String,
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
    fn entregas_personas_e_gates_roundtrip() {
        let store = BtvStore::open_in_memory().unwrap();
        let run_id = store
            .insert_run("sq1", "editorial", "v1.4", "Run", "[]", "[]", "t0")
            .unwrap();
        store.increment_gates("sq1", "t1").unwrap();
        store.increment_gates("sq1", "t2").unwrap();
        assert_eq!(
            store
                .get_run_by_task("sq1")
                .unwrap()
                .unwrap()
                .gates_aprovados,
            2
        );

        let d = store
            .insert_deliverable(
                run_id,
                "sq1",
                "editorial",
                "artigo.md",
                "/w/artigo.md",
                "MD",
                "v1",
                "Redator → Revisor · 2 gates",
                "t3",
            )
            .unwrap();
        let list = store.list_deliverables().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].formato, "MD");
        assert!(store.get_deliverable(d).unwrap().is_some());

        store
            .set_persona_override("editorial", "Redator", "prompt novo", "t4")
            .unwrap();
        store
            .set_persona_override("editorial", "Redator", "prompt novo 2", "t5")
            .unwrap();
        let ov = store.list_persona_overrides("editorial").unwrap();
        assert_eq!(ov.len(), 1);
        assert_eq!(ov[0].prompt, "prompt novo 2");
        let id = store
            .insert_custom_persona("editorial", "Nova persona", "p", "t6")
            .unwrap();
        store
            .update_custom_persona(id, "Persona X", "p2", "t7")
            .unwrap();
        assert_eq!(
            store.list_custom_personas("editorial").unwrap()[0].nome,
            "Persona X"
        );
        store.delete_custom_persona(id).unwrap();
        store.clear_persona_overrides("editorial").unwrap();
        assert!(store
            .list_persona_overrides("editorial")
            .unwrap()
            .is_empty());
        assert!(store.list_custom_personas("editorial").unwrap().is_empty());
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
