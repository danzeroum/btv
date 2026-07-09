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

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum BtvStoreError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("registro não encontrado")]
    NotFound,
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
                created_ts TEXT NOT NULL,
                pin_hash TEXT
            );
            CREATE TABLE IF NOT EXISTS custom_personas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id TEXT NOT NULL,
                nome TEXT NOT NULL,
                prompt TEXT NOT NULL,
                updated_ts TEXT NOT NULL
            );",
        )?;
        // Migração defensiva para bancos criados antes do PIN do A6: adiciona a
        // coluna se ausente (SQLite não tem "ADD COLUMN IF NOT EXISTS"; um erro
        // de coluna duplicada em banco já migrado é ignorado).
        let _ = conn.execute("ALTER TABLE users ADD COLUMN pin_hash TEXT", []);
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

    /// Maior número de `task_id` no formato `sq{hex}` já persistido (0 se não
    /// houver). O contador de `task_id` da squad é POR-PROCESSO e reinicia a
    /// cada restart; o volume sobrevive. Sem semear o contador a partir daqui,
    /// a primeira ativação após um redeploy geraria `sq1` de novo e bateria em
    /// `UNIQUE constraint failed: runs.task_id`. Usado no arranque do dashboard.
    pub fn max_run_task_seq(&self) -> u64 {
        let Ok(mut stmt) = self.conn.prepare("SELECT task_id FROM runs") else {
            return 0;
        };
        let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) else {
            return 0;
        };
        rows.flatten()
            .filter_map(|id| {
                id.strip_prefix("sq")
                    .and_then(|h| u64::from_str_radix(h, 16).ok())
            })
            .max()
            .unwrap_or(0)
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

    // ── A6: perfis locais com PIN opcional (verificado pelo backend) ──

    /// Cria um perfil, com PIN OPCIONAL. Um `pin` presente é guardado como
    /// hash (nunca em claro); ausente = perfil aberto (comportamento anterior).
    pub fn insert_user(
        &self,
        nome: &str,
        email: &str,
        papel: &str,
        pin: Option<&str>,
        now: &str,
    ) -> Result<i64, BtvStoreError> {
        let pin_hash = pin
            .filter(|p| !p.is_empty())
            .map(|p| pin_hash(now, email, nome, p));
        self.conn.execute(
            "INSERT INTO users (nome, email, papel, ativo, created_ts, pin_hash)
             VALUES (?1, ?2, ?3, 1, ?4, ?5)",
            params![nome, email, papel, now, pin_hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_user_ativo(&self, id: i64, ativo: bool) -> Result<(), BtvStoreError> {
        let afetadas = self.conn.execute(
            "UPDATE users SET ativo = ?2 WHERE id = ?1",
            params![id, ativo as i64],
        )?;
        if afetadas == 0 {
            return Err(BtvStoreError::NotFound);
        }
        Ok(())
    }

    /// Remove um perfil de vez (o "suspender" só desativa; isto apaga). Id
    /// inexistente → `NotFound`, para o handler devolver 404 em vez de fingir
    /// sucesso silencioso.
    pub fn delete_user(&self, id: i64) -> Result<(), BtvStoreError> {
        let afetadas = self
            .conn
            .execute("DELETE FROM users WHERE id = ?1", params![id])?;
        if afetadas == 0 {
            return Err(BtvStoreError::NotFound);
        }
        Ok(())
    }

    /// Define (ou limpa, com `None`/vazio) o PIN de um perfil. O salt é
    /// recomputado dos campos já persistidos (`created_ts|email|nome`).
    pub fn set_user_pin(&self, id: i64, pin: Option<&str>) -> Result<(), BtvStoreError> {
        let row: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT created_ts, email, nome FROM users WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((created_ts, email, nome)) = row else {
            return Err(BtvStoreError::NotFound);
        };
        let hash = pin
            .filter(|p| !p.is_empty())
            .map(|p| pin_hash(&created_ts, &email, &nome, p));
        self.conn.execute(
            "UPDATE users SET pin_hash = ?2 WHERE id = ?1",
            params![id, hash],
        )?;
        Ok(())
    }

    /// Verifica o PIN de um perfil contra o hash guardado. Perfil aberto (sem
    /// PIN) → [`PinCheck::NoPin`] (nada a verificar); id inexistente → `NotFound`.
    pub fn verify_user_pin(&self, id: i64, pin: &str) -> Result<PinCheck, BtvStoreError> {
        let row: Option<(String, String, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT created_ts, email, nome, pin_hash FROM users WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((created_ts, email, nome, stored)) = row else {
            return Err(BtvStoreError::NotFound);
        };
        match stored {
            None => Ok(PinCheck::NoPin),
            Some(stored) => {
                let candidate = pin_hash(&created_ts, &email, &nome, pin);
                // Comparação de tamanho fixo (hex de sha256) — diferença de
                // tempo desprezível para um dashboard local.
                if candidate == stored {
                    Ok(PinCheck::Ok)
                } else {
                    Ok(PinCheck::Wrong)
                }
            }
        }
    }

    pub fn list_users(&self) -> Result<Vec<BtvUser>, BtvStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, nome, email, papel, ativo, pin_hash FROM users ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(BtvUser {
                id: row.get(0)?,
                nome: row.get(1)?,
                email: row.get(2)?,
                papel: row.get(3)?,
                ativo: row.get::<_, i64>(4)? != 0,
                // Nunca vaza o hash — só se HÁ um PIN.
                has_pin: row.get::<_, Option<String>>(5)?.is_some(),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// Resultado de `verify_user_pin` — perfil aberto, PIN correto ou incorreto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCheck {
    NoPin,
    Ok,
    Wrong,
}

/// Hash do PIN: `sha256(created_ts|email|nome|pin)`. O salt (created_ts+
/// email+nome) é único por perfil e recomputável dos campos persistidos, então
/// não precisa de coluna própria. **Honesto:** é sha256 simples, não um KDF
/// pesado — adequado para um PIN de perfil local num dashboard 127.0.0.1, não
/// para um cofre de senhas exposto à rede.
fn pin_hash(created_ts: &str, email: &str, nome: &str, pin: &str) -> String {
    btv_schemas::sha256_hex(&format!("{created_ts}|{email}|{nome}|{pin}"))
}

/// Perfil local (A6): identidade nomeada para atribuição, com PIN OPCIONAL
/// verificado pelo backend (hash sha256, nunca em claro). O PIN gate o "assumir
/// perfil" na UI; não é uma barreira de rede (o dashboard é 127.0.0.1 e
/// guardado por `Origin`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BtvUser {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub papel: String,
    pub ativo: bool,
    /// Se o perfil exige PIN para ser assumido (o hash em si nunca é exposto).
    pub has_pin: bool,
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

    #[test]
    fn perfil_sem_pin_e_aberto_com_pin_verifica_backend() {
        let store = BtvStore::open_in_memory().unwrap();
        // Perfil aberto (sem PIN): has_pin=false; verify devolve NoPin.
        let aberto = store
            .insert_user("Ana", "ana@x", "usuario", None, "t1")
            .unwrap();
        // Perfil com PIN: has_pin=true, hash guardado (nunca exposto).
        let travado = store
            .insert_user("Bia", "bia@x", "admin", Some("1234"), "t2")
            .unwrap();

        let users = store.list_users().unwrap();
        assert!(!users.iter().find(|u| u.id == aberto).unwrap().has_pin);
        assert!(users.iter().find(|u| u.id == travado).unwrap().has_pin);

        assert_eq!(
            store.verify_user_pin(aberto, "qualquer").unwrap(),
            PinCheck::NoPin
        );
        assert_eq!(
            store.verify_user_pin(travado, "1234").unwrap(),
            PinCheck::Ok
        );
        assert_eq!(
            store.verify_user_pin(travado, "0000").unwrap(),
            PinCheck::Wrong
        );
        assert!(matches!(
            store.verify_user_pin(999, "x"),
            Err(BtvStoreError::NotFound)
        ));
    }

    #[test]
    fn set_pin_define_e_limpa() {
        let store = BtvStore::open_in_memory().unwrap();
        let id = store
            .insert_user("Ana", "ana@x", "usuario", None, "t1")
            .unwrap();
        assert_eq!(store.verify_user_pin(id, "x").unwrap(), PinCheck::NoPin);
        // Define um PIN → passa a exigir.
        store.set_user_pin(id, Some("42")).unwrap();
        assert!(store.list_users().unwrap()[0].has_pin);
        assert_eq!(store.verify_user_pin(id, "42").unwrap(), PinCheck::Ok);
        assert_eq!(store.verify_user_pin(id, "99").unwrap(), PinCheck::Wrong);
        // Limpa (None) → volta a aberto.
        store.set_user_pin(id, None).unwrap();
        assert!(!store.list_users().unwrap()[0].has_pin);
        assert_eq!(store.verify_user_pin(id, "42").unwrap(), PinCheck::NoPin);
    }

    #[test]
    fn max_run_task_seq_le_o_maior_sq_hex() {
        let store = BtvStore::open_in_memory().unwrap();
        assert_eq!(store.max_run_task_seq(), 0, "vazio → 0");
        store
            .insert_run("sq1", "editorial", "v1", "R1", "[]", "[]", "t1")
            .unwrap();
        store
            .insert_run("sqa", "editorial", "v1", "R2", "[]", "[]", "t2")
            .unwrap(); // 0xa = 10
        store
            .insert_run("sq3", "editorial", "v1", "R3", "[]", "[]", "t3")
            .unwrap();
        // Pega o MAIOR valor hex (10), não o último inserido nem a ordem textual.
        assert_eq!(store.max_run_task_seq(), 10);
    }

    #[test]
    fn set_ativo_em_id_inexistente_e_not_found() {
        let store = BtvStore::open_in_memory().unwrap();
        let id = store
            .insert_user("Ana", "ana@x", "usuario", None, "t1")
            .unwrap();
        // Perfil real: alterna sem erro.
        store.set_user_ativo(id, false).unwrap();
        // Id inexistente: NotFound (não mais no-op silencioso com 200).
        assert!(matches!(
            store.set_user_ativo(999_999, false),
            Err(BtvStoreError::NotFound)
        ));
    }

    #[test]
    fn delete_user_remove_de_vez_e_404_em_id_inexistente() {
        let store = BtvStore::open_in_memory().unwrap();
        let id = store
            .insert_user("Ana", "ana@x", "usuario", None, "t1")
            .unwrap();
        assert_eq!(store.list_users().unwrap().len(), 1);
        // Remove o perfil real: some da lista.
        store.delete_user(id).unwrap();
        assert!(store.list_users().unwrap().is_empty());
        // Remover de novo (ou id inexistente) → NotFound.
        assert!(matches!(
            store.delete_user(id),
            Err(BtvStoreError::NotFound)
        ));
    }
}
