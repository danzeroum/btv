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

use btv_domain::ports::{
    PersonaRepository, RepositoryError, RunRepository, RunStatus, TemplatePublicationRepository,
    UserRepository,
};
use btv_domain::{TaskId, TenantContext, TenantId};
use rusqlite::{params, Connection, OptionalExtension};

// Tarefa A2 do plano DDD: os tipos do produto moram em `btv-domain` (com
// `tenant` desde já — D1/ADR 0025); este adapter os re-exporta sob os nomes
// legados para os call sites atuais. Enquanto a coluna `tenant_id` não chega
// (B2), este adapter SQLite preenche `TenantId::LOCAL` fixo — o modo local É
// um tenant (ADR 0026). Wire inalterado: `tenant` é `skip_serializing`,
// provado pelos goldens T1.
pub use btv_domain::run::{Deliverable as BtvDeliverable, Run as BtvRun};
pub use btv_domain::user::User as BtvUser;
pub use btv_domain::{CustomPersona, PersonaOverride, PinCheck};

#[derive(Debug, thiserror::Error)]
pub enum BtvStoreError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("registro não encontrado")]
    NotFound,
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

    /// UUID textual do tenant do modo local — o DEFAULT das colunas
    /// `tenant_id` (backfill determinístico do ADR 0026: legado E escritas
    /// legadas, sem ctx, caem no LOCAL automaticamente).
    const LOCAL_TENANT: &'static str = "00000000-0000-0000-0000-000000000001";

    fn init(conn: Connection) -> Result<Self, BtvStoreError> {
        // Fresh DBs nascem multi-tenant (B2): tenant_id em toda tabela com
        // DEFAULT LOCAL; unicidade/PK compostas COM o tenant — no SaaS cada
        // tenant gera sq1, sq2, … por processo, então task_id é único POR
        // tenant, nunca global.
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                template_id TEXT NOT NULL,
                template_versao TEXT NOT NULL,
                nome TEXT NOT NULL,
                briefing_json TEXT NOT NULL,
                papeis_json TEXT NOT NULL,
                status TEXT NOT NULL,
                gates_aprovados INTEGER NOT NULL DEFAULT 0,
                created_ts TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                UNIQUE (tenant_id, task_id)
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
                created_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}'
            );
            CREATE TABLE IF NOT EXISTS persona_overrides (
                template_id TEXT NOT NULL,
                papel TEXT NOT NULL,
                prompt TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                PRIMARY KEY (tenant_id, template_id, papel)
            );
            CREATE TABLE IF NOT EXISTS template_pub (
                template_id TEXT NOT NULL,
                publicado INTEGER NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                PRIMARY KEY (tenant_id, template_id)
            );
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nome TEXT NOT NULL,
                email TEXT NOT NULL,
                papel TEXT NOT NULL,
                ativo INTEGER NOT NULL DEFAULT 1,
                created_ts TEXT NOT NULL,
                pin_hash TEXT,
                tenant_id TEXT NOT NULL DEFAULT '{local}'
            );
            CREATE TABLE IF NOT EXISTS custom_personas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id TEXT NOT NULL,
                nome TEXT NOT NULL,
                prompt TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}'
            );",
            local = Self::LOCAL_TENANT
        ))?;
        Self::migrate_legacy(&conn)?;
        // Migração defensiva para bancos criados antes do PIN do A6: adiciona a
        // coluna se ausente (SQLite não tem "ADD COLUMN IF NOT EXISTS"; um erro
        // de coluna duplicada em banco já migrado é ignorado).
        let _ = conn.execute("ALTER TABLE users ADD COLUMN pin_hash TEXT", []);
        Ok(Self { conn })
    }

    /// Migra um banco PRÉ-tenant (B2, ADR 0026): backfill determinístico
    /// `TenantId::LOCAL` em toda linha existente, sem perda — provado por
    /// teste que abre um DB com o schema antigo populado. `deliverables`/
    /// `users`/`custom_personas` ganham a coluna por `ADD COLUMN … DEFAULT`
    /// (que preenche as linhas existentes); `runs`/`persona_overrides`/
    /// `template_pub` precisam de REBUILD porque a unicidade/PK muda para
    /// incluir o tenant (SQLite não altera constraint por ALTER).
    fn migrate_legacy(conn: &Connection) -> Result<(), BtvStoreError> {
        let tem_tenant: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('runs') WHERE name = 'tenant_id'")?
            .exists([])?;
        if tem_tenant {
            return Ok(());
        }
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE;
            ALTER TABLE deliverables ADD COLUMN tenant_id TEXT NOT NULL DEFAULT '{local}';
            ALTER TABLE users ADD COLUMN tenant_id TEXT NOT NULL DEFAULT '{local}';
            ALTER TABLE custom_personas ADD COLUMN tenant_id TEXT NOT NULL DEFAULT '{local}';

            CREATE TABLE runs_b2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                template_id TEXT NOT NULL,
                template_versao TEXT NOT NULL,
                nome TEXT NOT NULL,
                briefing_json TEXT NOT NULL,
                papeis_json TEXT NOT NULL,
                status TEXT NOT NULL,
                gates_aprovados INTEGER NOT NULL DEFAULT 0,
                created_ts TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                UNIQUE (tenant_id, task_id)
            );
            INSERT INTO runs_b2 (id, task_id, template_id, template_versao, nome,
                                 briefing_json, papeis_json, status, gates_aprovados,
                                 created_ts, updated_ts, tenant_id)
                SELECT id, task_id, template_id, template_versao, nome,
                       briefing_json, papeis_json, status, gates_aprovados,
                       created_ts, updated_ts, '{local}' FROM runs;
            DROP TABLE runs;
            ALTER TABLE runs_b2 RENAME TO runs;

            CREATE TABLE persona_overrides_b2 (
                template_id TEXT NOT NULL,
                papel TEXT NOT NULL,
                prompt TEXT NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                PRIMARY KEY (tenant_id, template_id, papel)
            );
            INSERT INTO persona_overrides_b2 (template_id, papel, prompt, updated_ts, tenant_id)
                SELECT template_id, papel, prompt, updated_ts, '{local}' FROM persona_overrides;
            DROP TABLE persona_overrides;
            ALTER TABLE persona_overrides_b2 RENAME TO persona_overrides;

            CREATE TABLE template_pub_b2 (
                template_id TEXT NOT NULL,
                publicado INTEGER NOT NULL,
                updated_ts TEXT NOT NULL,
                tenant_id TEXT NOT NULL DEFAULT '{local}',
                PRIMARY KEY (tenant_id, template_id)
            );
            INSERT INTO template_pub_b2 (template_id, publicado, updated_ts, tenant_id)
                SELECT template_id, publicado, updated_ts, '{local}' FROM template_pub;
            DROP TABLE template_pub;
            ALTER TABLE template_pub_b2 RENAME TO template_pub;
            COMMIT;",
            local = Self::LOCAL_TENANT
        ))?;
        Ok(())
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
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT task_id FROM runs WHERE tenant_id = ?1")
        else {
            return 0;
        };
        let Ok(rows) = stmt.query_map(params![Self::LOCAL_TENANT], |r| r.get::<_, String>(0))
        else {
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

    /// Reconcilia runs ZUMBIS no arranque: uma run `ativa` no volume cujo
    /// processo já morreu (crash, restart, container recriado) não tem como
    /// estar realmente rodando — o estado vivo da squad mora só na MEMÓRIA do
    /// processo, que se foi. Marca todas as `ativa` como `encerrada` para não
    /// ficarem "ativa" para sempre na tela. Deve rodar SÓ no arranque do
    /// dashboard (quando nada ainda está rodando neste processo). Devolve
    /// quantas reconciliou.
    pub fn reconcile_stale_runs(&self, now: &str) -> Result<usize, BtvStoreError> {
        let n = self.conn.execute(
            "UPDATE runs SET status = 'encerrada', updated_ts = ?1
             WHERE status = 'ativa' AND tenant_id = ?2",
            params![now, Self::LOCAL_TENANT],
        )?;
        Ok(n)
    }

    /// Transição de status ao fim da execução (`concluida`/`erro`) ou no
    /// kill-switch (`encerrada`). Silencioso para task_id desconhecido — o
    /// watcher pode sobreviver a um run apagado. A4: recebe `RunStatus`
    /// tipado — `status = "qualquer_string"` não compila mais; o SQL grava
    /// exatamente `as_str()` (mesmos bytes de sempre, T3 como juiz). Este
    /// método morre em C3, quando o handler migrar para o caminho
    /// `RunRepository::get → Run::transition_to → save` (o adapter das
    /// traits já existe neste arquivo desde B2).
    pub fn set_status(
        &self,
        task_id: &str,
        status: RunStatus,
        now: &str,
    ) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE runs SET status = ?2, updated_ts = ?3
             WHERE task_id = ?1 AND tenant_id = ?4",
            params![task_id, status.as_str(), now, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    /// Runs mais recentes primeiro (a squad em execução aparece no topo de
    /// U6 porque é a mais nova com status `ativa`). API legada = a porta do
    /// modo local: escopo fixo no tenant LOCAL (B2) — num banco multi-tenant,
    /// linhas de outros tenants não vazam por aqui.
    pub fn list_runs(&self) -> Result<Vec<BtvRun>, BtvStoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {RUN_COLS} FROM runs WHERE tenant_id = ?1 ORDER BY id DESC"
        ))?;
        let rows = stmt.query_map(params![Self::LOCAL_TENANT], row_to_run)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    /// Incrementa a contagem de gates aprovados do run (chamado pelo
    /// handler de gate do BTV — compõe a trilha de procedência de U4).
    pub fn increment_gates(&self, task_id: &str, now: &str) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "UPDATE runs SET gates_aprovados = gates_aprovados + 1, updated_ts = ?2
             WHERE task_id = ?1 AND tenant_id = ?3",
            params![task_id, now, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    pub fn get_run_by_task(&self, task_id: &str) -> Result<Option<BtvRun>, BtvStoreError> {
        Ok(self
            .list_runs()?
            .into_iter()
            .find(|r| r.task_id.to_string() == task_id))
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
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {DELIVERABLE_COLS} FROM deliverables WHERE tenant_id = ?1 ORDER BY id DESC"
        ))?;
        let rows = stmt.query_map(params![Self::LOCAL_TENANT], row_to_deliverable)?;
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
        // B2: a PK agora inclui o tenant — o alvo do ON CONFLICT acompanha
        // (o DEFAULT da coluna preenche o LOCAL na escrita legada).
        self.conn.execute(
            "INSERT INTO persona_overrides (template_id, papel, prompt, updated_ts)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(tenant_id, template_id, papel) DO UPDATE SET prompt = ?3, updated_ts = ?4",
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
            "DELETE FROM persona_overrides
             WHERE template_id = ?1 AND papel = ?2 AND tenant_id = ?3",
            params![template_id, papel, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    pub fn clear_persona_overrides(&self, template_id: &str) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "DELETE FROM persona_overrides WHERE template_id = ?1 AND tenant_id = ?2",
            params![template_id, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    pub fn list_persona_overrides(
        &self,
        template_id: &str,
    ) -> Result<Vec<PersonaOverride>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT template_id, papel, prompt, tenant_id FROM persona_overrides
             WHERE template_id = ?1 AND tenant_id = ?2",
        )?;
        let rows = stmt.query_map(params![template_id, Self::LOCAL_TENANT], |row| {
            Ok(PersonaOverride {
                template_id: row.get(0)?,
                papel: row.get(1)?,
                prompt: row.get(2)?,
                tenant: parse_tenant_col(row, 3)?,
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
            "UPDATE custom_personas SET nome = ?2, prompt = ?3, updated_ts = ?4
             WHERE id = ?1 AND tenant_id = ?5",
            params![id, nome, prompt, now, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    pub fn delete_custom_persona(&self, id: i64) -> Result<(), BtvStoreError> {
        self.conn.execute(
            "DELETE FROM custom_personas WHERE id = ?1 AND tenant_id = ?2",
            params![id, Self::LOCAL_TENANT],
        )?;
        Ok(())
    }

    pub fn list_custom_personas(
        &self,
        template_id: &str,
    ) -> Result<Vec<CustomPersona>, BtvStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, template_id, nome, prompt, tenant_id FROM custom_personas
             WHERE template_id = ?1 AND tenant_id = ?2 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![template_id, Self::LOCAL_TENANT], |row| {
            Ok(CustomPersona {
                id: row.get(0)?,
                template_id: row.get(1)?,
                nome: row.get(2)?,
                prompt: row.get(3)?,
                tenant: parse_tenant_col(row, 4)?,
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
        // B2: idem persona_overrides — alvo do ON CONFLICT acompanha a PK.
        self.conn.execute(
            "INSERT INTO template_pub (template_id, publicado, updated_ts) VALUES (?1, ?2, ?3)
             ON CONFLICT(tenant_id, template_id) DO UPDATE SET publicado = ?2, updated_ts = ?3",
            params![template_id, publicado as i64, now],
        )?;
        Ok(())
    }

    pub fn list_template_pub(&self) -> Result<Vec<(String, bool)>, BtvStoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT template_id, publicado FROM template_pub WHERE tenant_id = ?1")?;
        let rows = stmt.query_map(params![Self::LOCAL_TENANT], |row| {
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
        let mut stmt = self.conn.prepare(
            "SELECT id, nome, email, papel, ativo, pin_hash, tenant_id FROM users
             WHERE tenant_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![Self::LOCAL_TENANT], |row| {
            Ok(BtvUser {
                id: row.get(0)?,
                nome: row.get(1)?,
                email: row.get(2)?,
                papel: row.get(3)?,
                ativo: row.get::<_, i64>(4)? != 0,
                // Nunca vaza o hash — só se HÁ um PIN.
                has_pin: row.get::<_, Option<String>>(5)?.is_some(),
                tenant: parse_tenant_col(row, 6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

// ── B2: adapter SQLite das traits de `btv-domain::ports` (ADR 0026) ─────────
//
// O `BtvStore` É o adapter: os mesmos arquivos `.btv/btv.db` servem a API
// legada (acima, escopo fixo LOCAL) e as traits com `&TenantContext`. A
// suíte de contrato (`btv-contract`) julga ESTE bloco — e julgará o adapter
// Postgres em B4 com os mesmos testes.

/// Colunas do SELECT de runs na ordem que `row_to_run` espera.
pub(crate) const RUN_COLS: &str =
    "id, task_id, template_id, template_versao, nome, briefing_json, \
                        papeis_json, status, gates_aprovados, created_ts, updated_ts, tenant_id";

/// Colunas do SELECT de deliverables na ordem que `row_to_deliverable` espera.
pub(crate) const DELIVERABLE_COLS: &str =
    "id, run_id, task_id, template_id, nome, path, formato, versao, trilha, created_ts, tenant_id";

/// Fail-closed: coluna `tenant_id` fora do formato UUID é ERRO de leitura,
/// não tenant fabricado (mesma regra do `TaskId`/`RunStatus` do A4).
fn parse_tenant_col(row: &rusqlite::Row, idx: usize) -> Result<TenantId, rusqlite::Error> {
    let raw: String = row.get(idx)?;
    TenantId::parse(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Mapeamento tipado da linha de run — task_id/status/tenant com parse
/// fail-closed (linha fora do vocabulário é erro, não valor fabricado; todo
/// dado real passa — os formatos são os que o próprio produto sempre gravou).
fn row_to_run(row: &rusqlite::Row) -> Result<BtvRun, rusqlite::Error> {
    let task_id_raw: String = row.get(1)?;
    let status_raw: String = row.get(7)?;
    Ok(BtvRun {
        id: row.get(0)?,
        task_id: TaskId::parse(&task_id_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        template_id: row.get(2)?,
        template_versao: row.get(3)?,
        nome: row.get(4)?,
        briefing_json: row.get(5)?,
        papeis_json: row.get(6)?,
        status: RunStatus::parse(&status_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
        })?,
        gates_aprovados: row.get(8)?,
        created_ts: row.get(9)?,
        updated_ts: row.get(10)?,
        tenant: parse_tenant_col(row, 11)?,
    })
}

fn row_to_deliverable(row: &rusqlite::Row) -> Result<BtvDeliverable, rusqlite::Error> {
    let task_id_raw: String = row.get(2)?;
    Ok(BtvDeliverable {
        id: row.get(0)?,
        run_id: row.get(1)?,
        task_id: TaskId::parse(&task_id_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        template_id: row.get(3)?,
        nome: row.get(4)?,
        path: row.get(5)?,
        formato: row.get(6)?,
        versao: row.get(7)?,
        trilha: row.get(8)?,
        created_ts: row.get(9)?,
        tenant: parse_tenant_col(row, 10)?,
    })
}

/// Tradução na fronteira (critério 2 do G1): `rusqlite::Error` NUNCA
/// atravessa a assinatura das traits — vira `RepositoryError::Storage` com a
/// mensagem preservada para diagnóstico.
fn storage(e: rusqlite::Error) -> RepositoryError {
    RepositoryError::Storage(e.to_string())
}

/// Fail-closed na ESCRITA: um agregado/entrega cujo `tenant` difere do
/// contexto é recusado antes de tocar o banco — a suíte de contrato prova
/// que a recusa no meio de um lote desfaz a transação inteira.
pub(crate) fn exige_mesmo_tenant(
    ctx: &TenantContext,
    dono: TenantId,
    o_que: &str,
) -> Result<(), RepositoryError> {
    if dono != ctx.tenant {
        return Err(RepositoryError::Storage(format!(
            "recusado (fail-closed): {o_que} pertence ao tenant {dono}, não ao do contexto {}",
            ctx.tenant
        )));
    }
    Ok(())
}

/// Upsert do agregado por `(tenant_id, task_id)` — compartilhado por `save`
/// e `save_with_deliverables` (que o roda dentro da transação; `Transaction`
/// deref-a para `Connection`). `id` do struct é ignorado: quem numera a
/// linha é o banco, e a identidade do agregado é o `task_id` no tenant.
fn upsert_run(conn: &Connection, run: &btv_domain::Run) -> Result<(), rusqlite::Error> {
    let tenant = run.tenant.to_string();
    let task = run.task_id.to_string();
    let n = conn.execute(
        "UPDATE runs SET template_id = ?3, template_versao = ?4, nome = ?5,
                         briefing_json = ?6, papeis_json = ?7, status = ?8,
                         gates_aprovados = ?9, created_ts = ?10, updated_ts = ?11
         WHERE tenant_id = ?1 AND task_id = ?2",
        params![
            tenant,
            task,
            run.template_id,
            run.template_versao,
            run.nome,
            run.briefing_json,
            run.papeis_json,
            run.status.as_str(),
            run.gates_aprovados,
            run.created_ts,
            run.updated_ts
        ],
    )?;
    if n == 0 {
        conn.execute(
            "INSERT INTO runs (tenant_id, task_id, template_id, template_versao, nome,
                               briefing_json, papeis_json, status, gates_aprovados,
                               created_ts, updated_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tenant,
                task,
                run.template_id,
                run.template_versao,
                run.nome,
                run.briefing_json,
                run.papeis_json,
                run.status.as_str(),
                run.gates_aprovados,
                run.created_ts,
                run.updated_ts
            ],
        )?;
    }
    Ok(())
}

impl RunRepository for BtvStore {
    fn get(
        &self,
        ctx: &TenantContext,
        task_id: &str,
    ) -> Result<Option<btv_domain::Run>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {RUN_COLS} FROM runs WHERE tenant_id = ?1 AND task_id = ?2"
            ))
            .map_err(storage)?;
        stmt.query_row(params![ctx.tenant.to_string(), task_id], row_to_run)
            .optional()
            .map_err(storage)
    }

    fn list(&self, ctx: &TenantContext) -> Result<Vec<btv_domain::Run>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {RUN_COLS} FROM runs WHERE tenant_id = ?1 ORDER BY id DESC"
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string()], row_to_run)
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }

    fn save(&mut self, ctx: &TenantContext, run: &btv_domain::Run) -> Result<(), RepositoryError> {
        exige_mesmo_tenant(ctx, run.tenant, "o run")?;
        upsert_run(&self.conn, run).map_err(storage)
    }

    fn save_with_deliverables(
        &mut self,
        ctx: &TenantContext,
        run: &btv_domain::Run,
        novas: &[btv_domain::Deliverable],
    ) -> Result<(), RepositoryError> {
        exige_mesmo_tenant(ctx, run.tenant, "o run")?;
        // A transação REAL do critério 4 do G1: run + entregas gravam juntos
        // ou nada grava — retorno antecipado (recusa fail-closed no meio do
        // lote) derruba `tx` sem commit ⇒ rollback automático.
        let tx = self.conn.transaction().map_err(storage)?;
        upsert_run(&tx, run).map_err(storage)?;
        let run_row_id: i64 = tx
            .query_row(
                "SELECT id FROM runs WHERE tenant_id = ?1 AND task_id = ?2",
                params![ctx.tenant.to_string(), run.task_id.to_string()],
                |r| r.get(0),
            )
            .map_err(storage)?;
        for entrega in novas {
            exige_mesmo_tenant(ctx, entrega.tenant, "a entrega")?;
            tx.execute(
                "INSERT INTO deliverables (run_id, task_id, template_id, nome, path,
                                           formato, versao, trilha, created_ts, tenant_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run_row_id,
                    entrega.task_id.to_string(),
                    entrega.template_id,
                    entrega.nome,
                    entrega.path,
                    entrega.formato,
                    entrega.versao,
                    entrega.trilha,
                    entrega.created_ts,
                    ctx.tenant.to_string()
                ],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)
    }

    fn list_deliverables(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<btv_domain::Deliverable>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {DELIVERABLE_COLS} FROM deliverables WHERE tenant_id = ?1 ORDER BY id DESC"
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string()], row_to_deliverable)
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }

    fn get_deliverable(
        &self,
        ctx: &TenantContext,
        id: i64,
    ) -> Result<Option<btv_domain::Deliverable>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {DELIVERABLE_COLS} FROM deliverables WHERE tenant_id = ?1 AND id = ?2"
            ))
            .map_err(storage)?;
        stmt.query_row(params![ctx.tenant.to_string(), id], row_to_deliverable)
            .optional()
            .map_err(storage)
    }

    fn max_task_seq(&self, ctx: &TenantContext) -> Result<u64, RepositoryError> {
        // Mesma semântica do `max_run_task_seq` legado, mas POR tenant e com
        // erros de storage propagados (não engolidos como 0).
        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM runs WHERE tenant_id = ?1")
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string()], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        let mut max = 0u64;
        for raw in rows {
            let raw = raw.map_err(storage)?;
            if let Ok(task) = TaskId::parse(&raw) {
                max = max.max(task.seq());
            }
        }
        Ok(max)
    }
}

impl PersonaRepository for BtvStore {
    fn list_overrides(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<PersonaOverride>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT template_id, papel, prompt, tenant_id FROM persona_overrides
                 WHERE tenant_id = ?1 AND template_id = ?2",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string(), template_id], |row| {
                Ok(PersonaOverride {
                    template_id: row.get(0)?,
                    papel: row.get(1)?,
                    prompt: row.get(2)?,
                    tenant: parse_tenant_col(row, 3)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }

    fn set_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
        prompt: &str,
    ) -> Result<(), RepositoryError> {
        // `updated_ts` é escrituração do ADAPTER, não estado do domínio — a
        // assinatura aceita no G1 não carrega relógio, então o adapter usa o
        // do banco (mesmo formato RFC3339 do restante do sistema).
        self.conn
            .execute(
                "INSERT INTO persona_overrides (tenant_id, template_id, papel, prompt, updated_ts)
                 VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                 ON CONFLICT(tenant_id, template_id, papel)
                 DO UPDATE SET prompt = ?4, updated_ts = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                params![ctx.tenant.to_string(), template_id, papel, prompt],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn delete_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
    ) -> Result<(), RepositoryError> {
        self.conn
            .execute(
                "DELETE FROM persona_overrides
                 WHERE tenant_id = ?1 AND template_id = ?2 AND papel = ?3",
                params![ctx.tenant.to_string(), template_id, papel],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn clear_overrides(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<(), RepositoryError> {
        self.conn
            .execute(
                "DELETE FROM persona_overrides WHERE tenant_id = ?1 AND template_id = ?2",
                params![ctx.tenant.to_string(), template_id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn list_custom(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<CustomPersona>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, template_id, nome, prompt, tenant_id FROM custom_personas
                 WHERE tenant_id = ?1 AND template_id = ?2 ORDER BY id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string(), template_id], |row| {
                Ok(CustomPersona {
                    id: row.get(0)?,
                    template_id: row.get(1)?,
                    nome: row.get(2)?,
                    prompt: row.get(3)?,
                    tenant: parse_tenant_col(row, 4)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }

    fn insert_custom(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        nome: &str,
        prompt: &str,
    ) -> Result<i64, RepositoryError> {
        self.conn
            .execute(
                "INSERT INTO custom_personas (tenant_id, template_id, nome, prompt, updated_ts)
                 VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
                params![ctx.tenant.to_string(), template_id, nome, prompt],
            )
            .map_err(storage)?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_custom(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        nome: &str,
        prompt: &str,
    ) -> Result<(), RepositoryError> {
        // 0 linhas = não existe NESTE tenant (id de outro tenant é
        // indistinguível de inexistente — isolamento também na mutação).
        let n = self
            .conn
            .execute(
                "UPDATE custom_personas
                 SET nome = ?3, prompt = ?4, updated_ts = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id, nome, prompt],
            )
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn delete_custom(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError> {
        let n = self
            .conn
            .execute(
                "DELETE FROM custom_personas WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id],
            )
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

impl TemplatePublicationRepository for BtvStore {
    fn set_published(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        published: bool,
    ) -> Result<(), RepositoryError> {
        // `updated_ts` é escrituração do adapter (o port não carrega relógio,
        // decisão do G1) — o do banco, mesmo formato do restante.
        self.conn
            .execute(
                "INSERT INTO template_pub (tenant_id, template_id, publicado, updated_ts)
                 VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                 ON CONFLICT(tenant_id, template_id)
                 DO UPDATE SET publicado = ?3, updated_ts = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                params![ctx.tenant.to_string(), template_id, published as i64],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn list_published(&self, ctx: &TenantContext) -> Result<Vec<(String, bool)>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare("SELECT template_id, publicado FROM template_pub WHERE tenant_id = ?1")
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
            })
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }
}

impl UserRepository for BtvStore {
    fn list(&self, ctx: &TenantContext) -> Result<Vec<BtvUser>, RepositoryError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, nome, email, papel, ativo, pin_hash, tenant_id FROM users
                 WHERE tenant_id = ?1 ORDER BY id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![ctx.tenant.to_string()], |row| {
                Ok(BtvUser {
                    id: row.get(0)?,
                    nome: row.get(1)?,
                    email: row.get(2)?,
                    papel: row.get(3)?,
                    ativo: row.get::<_, i64>(4)? != 0,
                    has_pin: row.get::<_, Option<String>>(5)?.is_some(),
                    tenant: parse_tenant_col(row, 6)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)
    }

    fn create(
        &mut self,
        ctx: &TenantContext,
        nome: &str,
        email: &str,
        papel: &str,
        pin: Option<&str>,
    ) -> Result<i64, RepositoryError> {
        // O `created_ts` é o salt do `pin_hash` E a coluna — precisa do MESMO
        // valor nos dois; leio o relógio do banco uma vez (como os outros
        // adapters usam strftime, mas aqui o valor tem que existir em Rust).
        let now: String = self
            .conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |r| {
                r.get(0)
            })
            .map_err(storage)?;
        let hash = pin
            .filter(|p| !p.is_empty())
            .map(|p| pin_hash(&now, email, nome, p));
        self.conn
            .execute(
                "INSERT INTO users (tenant_id, nome, email, papel, ativo, created_ts, pin_hash)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                params![ctx.tenant.to_string(), nome, email, papel, now, hash],
            )
            .map_err(storage)?;
        Ok(self.conn.last_insert_rowid())
    }

    fn remove(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError> {
        let n = self
            .conn
            .execute(
                "DELETE FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id],
            )
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn set_active(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        ativo: bool,
    ) -> Result<(), RepositoryError> {
        let n = self
            .conn
            .execute(
                "UPDATE users SET ativo = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id, ativo as i64],
            )
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn set_pin(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        pin: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let row: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT created_ts, email, nome FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(storage)?;
        let Some((created_ts, email, nome)) = row else {
            return Err(RepositoryError::NotFound);
        };
        let hash = pin
            .filter(|p| !p.is_empty())
            .map(|p| pin_hash(&created_ts, &email, &nome, p));
        self.conn
            .execute(
                "UPDATE users SET pin_hash = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id, hash],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn verify_pin(
        &self,
        ctx: &TenantContext,
        id: i64,
        pin: &str,
    ) -> Result<PinCheck, RepositoryError> {
        let row: Option<(String, String, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT created_ts, email, nome, pin_hash FROM users
                 WHERE tenant_id = ?1 AND id = ?2",
                params![ctx.tenant.to_string(), id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(storage)?;
        let Some((created_ts, email, nome, stored)) = row else {
            return Err(RepositoryError::NotFound);
        };
        match stored {
            None => Ok(PinCheck::NoPin),
            Some(stored) => {
                let candidate = pin_hash(&created_ts, &email, &nome, pin);
                if candidate == stored {
                    Ok(PinCheck::Ok)
                } else {
                    Ok(PinCheck::Wrong)
                }
            }
        }
    }
}

/// Hash do PIN: `sha256(created_ts|email|nome|pin)`. O salt (created_ts+
/// email+nome) é único por perfil e recomputável dos campos persistidos, então
/// não precisa de coluna própria. **Honesto:** é sha256 simples, não um KDF
/// pesado — adequado para um PIN de perfil local num dashboard 127.0.0.1, não
/// para um cofre de senhas exposto à rede.
pub(crate) fn pin_hash(created_ts: &str, email: &str, nome: &str, pin: &str) -> String {
    btv_schemas::sha256_hex(&format!("{created_ts}|{email}|{nome}|{pin}"))
}

// (BtvUser/BtvDeliverable/PersonaOverride/CustomPersona: definições movidas
// para `btv-domain` na A2 — ver re-exports no topo do módulo.)

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
        assert_eq!(runs[0].status, RunStatus::Ativa);
        assert_eq!(runs[0].template_id, "editorial");

        store
            .set_status("sq1", RunStatus::Concluida, "2026-07-08T00:10:00Z")
            .unwrap();
        let runs = store.list_runs().unwrap();
        assert_eq!(runs[0].status, RunStatus::Concluida);
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
    fn reconcile_stale_runs_encerra_ativas_orfas() {
        let store = BtvStore::open_in_memory().unwrap();
        store
            .insert_run("sq1", "editorial", "v1", "R1", "[]", "[]", "t1")
            .unwrap(); // fica 'ativa'
        store
            .insert_run("sq2", "editorial", "v1", "R2", "[]", "[]", "t2")
            .unwrap();
        store.set_status("sq2", RunStatus::Concluida, "t2").unwrap(); // concluída NÃO deve mudar
        let n = store.reconcile_stale_runs("t3").unwrap();
        assert_eq!(n, 1, "só a 'ativa' órfã é reconciliada");
        let runs = store.list_runs().unwrap();
        let sq1 = runs.iter().find(|r| r.task_id == TaskId::new(1)).unwrap();
        let sq2 = runs.iter().find(|r| r.task_id == TaskId::new(2)).unwrap();
        assert_eq!(sq1.status, RunStatus::Encerrada);
        assert_eq!(sq2.status, RunStatus::Concluida, "concluída fica intacta");
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

/// Uma linha cujo valor está FORA do vocabulário fechado do domínio (A3) —
/// resultado da varredura do `btv doctor` (pendência da revisão do A4:
/// transformar o fail-closed que "grita como UI vazia" em diagnóstico que
/// aponta a linha).
#[derive(Debug, Clone, serde::Serialize)]
pub struct VocabViolation {
    pub tabela: &'static str,
    /// `id` físico da linha ofensora (ou `seq` no ledger).
    pub linha: i64,
    pub coluna: &'static str,
    pub valor: String,
    pub erro: String,
}

impl BtvStore {
    /// Varre `runs` e `deliverables` validando cada valor com os MESMOS
    /// parses fail-closed do domínio (`TaskId::parse`/`RunStatus::parse`) —
    /// a régua é uma só; o doctor só a aplica em lote e aponta a linha.
    pub fn linhas_fora_do_vocabulario(&self) -> Result<Vec<VocabViolation>, BtvStoreError> {
        let mut fora = Vec::new();
        let mut stmt = self.conn.prepare("SELECT id, task_id, status FROM runs")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, task_id, status) = row?;
            if let Err(e) = TaskId::parse(&task_id) {
                fora.push(VocabViolation {
                    tabela: "runs",
                    linha: id,
                    coluna: "task_id",
                    valor: task_id.clone(),
                    erro: e.to_string(),
                });
            }
            if let Err(e) = RunStatus::parse(&status) {
                fora.push(VocabViolation {
                    tabela: "runs",
                    linha: id,
                    coluna: "status",
                    valor: status,
                    erro: e.to_string(),
                });
            }
        }
        let mut stmt = self.conn.prepare("SELECT id, task_id FROM deliverables")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, task_id) = row?;
            if let Err(e) = TaskId::parse(&task_id) {
                fora.push(VocabViolation {
                    tabela: "deliverables",
                    linha: id,
                    coluna: "task_id",
                    valor: task_id,
                    erro: e.to_string(),
                });
            }
        }
        Ok(fora)
    }
}

#[cfg(test)]
mod vocab_tests {
    use super::*;

    /// A varredura do doctor (pendência da revisão do A4) aponta A LINHA:
    /// linhas corrompidas por fora do vocabulário (typo de status, task_id
    /// malformado) viram diagnóstico com tabela+linha+coluna+valor — o
    /// mesmo parse fail-closed do domínio, aplicado em lote.
    #[test]
    fn scan_aponta_a_linha_fora_do_vocabulario() {
        let store = BtvStore::open_in_memory().unwrap();
        store
            .insert_run("sq1", "editorial", "v1.4", "Ok", "[]", "[]", "t")
            .unwrap();
        assert!(store.linhas_fora_do_vocabulario().unwrap().is_empty());

        // Corrompe por fora da API (o que um UPDATE manual/bug faria).
        store
            .conn
            .execute(
                "UPDATE runs SET status = 'ativva' WHERE task_id = 'sq1'",
                [],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO deliverables (run_id, task_id, template_id, nome, path, formato,
                                           versao, trilha, created_ts)
                 VALUES (1, 'xx-9', 'editorial', 'e', '/tmp/e', 'MD', 'v1', 't', 't')",
                [],
            )
            .unwrap();

        let fora = store.linhas_fora_do_vocabulario().unwrap();
        assert_eq!(fora.len(), 2);
        let status = fora.iter().find(|v| v.coluna == "status").unwrap();
        assert_eq!(status.tabela, "runs");
        assert_eq!(status.valor, "ativva");
        assert!(status.linha >= 1, "aponta a linha física");
        let task = fora.iter().find(|v| v.tabela == "deliverables").unwrap();
        assert_eq!(task.coluna, "task_id");
        assert_eq!(task.valor, "xx-9");
    }
}
