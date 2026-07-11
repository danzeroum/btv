//! Adapter Postgres (B4, ADR 0026): o modo SaaS atrás das MESMAS traits que
//! o SQLite serve no modo local — dois adapters permanentes, não migração.
//!
//! Fronteiras deste módulo:
//! - As traits do domínio são SÍNCRONAS (assinaturas aceitas no G1); o sqlx
//!   é async — o adapter carrega um runtime tokio próprio (current-thread)
//!   e faz `block_on` por operação. Custo aceito conscientemente: o
//!   chamador não muda de cor, e a troca de adapter é troca de construtor.
//! - Toda operação roda numa TRANSAÇÃO que primeiro fixa `app.tenant_id`
//!   (`set_config(..., true)` = local à transação): o RLS das migrations é
//!   a segunda linha de defesa; o `WHERE tenant_id = $n` explícito continua
//!   em TODA query (defesa em profundidade do ADR 0026 — nenhuma camada
//!   confia só na outra; o teste adversarial prova a camada RLS sozinha).
//! - Append do ledger: retry otimista sobre `UNIQUE (tenant_id, seq)`
//!   (ADR 0028) — o perdedor da corrida relê o topo da cadeia do SEU tenant
//!   e reencadeia. Nenhum lock de sessão: sobrevive a pooler em modo
//!   transação, e tenants nunca se serializam entre si.
//! - O DTO do ledger é o MESMO do adapter SQLite (`ledger::entry_de_dominio`
//!   / `payload_wire`) e a verificação de cadeia é a MESMA função
//!   (`ledger::verifica_cadeia_rows`) — paridade criptográfica por
//!   construção, cobrada pelo teste de determinismo cross-adapter da suíte.

use crate::btv::{exige_mesmo_tenant, DELIVERABLE_COLS, RUN_COLS};
use crate::ledger::{entry_de_dominio, verifica_cadeia_rows};
use btv_domain::ports::{DomainEvent, LedgerRepository, PersonaRepository, RunRepository};
use btv_domain::ports::{
    RepositoryError, RunStatus, TemplatePublicationRepository, UserRepository,
};
use btv_domain::{ActorId, CustomPersona, Deliverable, PersonaOverride, Run, TaskId};
use btv_domain::{PinCheck, TenantContext, TenantId, User};
use btv_schemas::ledger::LedgerEntry;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow};
use sqlx::{PgPool, Postgres, Row, Transaction};

/// Tentativas máximas do append otimista antes de desistir com erro — sob
/// contenção real cada retry relê o topo novo, então só um adversário
/// gravando sem parar na MESMA cadeia esgotaria isto.
const MAX_TENTATIVAS_APPEND: usize = 64;

/// `updated_ts` de escrituração do adapter (mesma regra do SQLite: a
/// assinatura aceita no G1 não carrega relógio, o banco fornece — RFC3339 UTC
/// no mesmo formato do resto do sistema).
const NOW_UTC_SQL: &str = "to_char(now() at time zone 'utc', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')";

pub struct PgStore {
    rt: tokio::runtime::Runtime,
    pool: PgPool,
}

fn storage(e: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Storage(e.to_string())
}

/// Tamanho do pool de conexões Postgres (modo SaaS). Overridável por ambiente
/// sem recompilar — `BTV_PG_MAX_CONNECTIONS` (default 4). Valor inválido ou
/// ausente cai no default.
fn pool_max_connections() -> u32 {
    std::env::var("BTV_PG_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4)
}

/// Fixa o tenant da TRANSAÇÃO para as policies de RLS (`is_local = true`:
/// evapora no COMMIT/ROLLBACK — nada vaza para a próxima transação da mesma
/// conexão do pool).
async fn fixa_tenant(tx: &mut Transaction<'_, Postgres>, tenant: &str) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Erro de violação do `UNIQUE (tenant_id, seq)` — a corrida do append
/// otimista (SQLSTATE 23505). Qualquer outro erro propaga.
fn eh_conflito_unique(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|d| d.code())
        .is_some_and(|c| c == "23505")
}

fn linha_para_run(row: &PgRow) -> Result<Run, RepositoryError> {
    let task_raw: String = row.try_get(1).map_err(storage)?;
    let status_raw: String = row.try_get(7).map_err(storage)?;
    let tenant_raw: String = row.try_get(11).map_err(storage)?;
    Ok(Run {
        id: row.try_get(0).map_err(storage)?,
        task_id: TaskId::parse(&task_raw).map_err(storage)?,
        template_id: row.try_get(2).map_err(storage)?,
        template_versao: row.try_get(3).map_err(storage)?,
        nome: row.try_get(4).map_err(storage)?,
        briefing_json: row.try_get(5).map_err(storage)?,
        papeis_json: row.try_get(6).map_err(storage)?,
        status: RunStatus::parse(&status_raw).map_err(storage)?,
        gates_aprovados: row.try_get(8).map_err(storage)?,
        created_ts: row.try_get(9).map_err(storage)?,
        updated_ts: row.try_get(10).map_err(storage)?,
        tenant: TenantId::parse(&tenant_raw).map_err(storage)?,
    })
}

fn linha_para_deliverable(row: &PgRow) -> Result<Deliverable, RepositoryError> {
    let task_raw: String = row.try_get(2).map_err(storage)?;
    let tenant_raw: String = row.try_get(10).map_err(storage)?;
    Ok(Deliverable {
        id: row.try_get(0).map_err(storage)?,
        run_id: row.try_get(1).map_err(storage)?,
        task_id: TaskId::parse(&task_raw).map_err(storage)?,
        template_id: row.try_get(3).map_err(storage)?,
        nome: row.try_get(4).map_err(storage)?,
        path: row.try_get(5).map_err(storage)?,
        formato: row.try_get(6).map_err(storage)?,
        versao: row.try_get(7).map_err(storage)?,
        trilha: row.try_get(8).map_err(storage)?,
        created_ts: row.try_get(9).map_err(storage)?,
        tenant: TenantId::parse(&tenant_raw).map_err(storage)?,
    })
}

impl PgStore {
    /// Conecta e roda as migrations embutidas (`migrations_pg/`) —
    /// idempotente: o sqlx registra as versões aplicadas em
    /// `_sqlx_migrations` no schema do `search_path`.
    pub fn connect(url: &str) -> Result<Self, RepositoryError> {
        let opts: PgConnectOptions = url.parse().map_err(storage)?;
        Self::connect_with(opts)
    }

    pub fn connect_with(opts: PgConnectOptions) -> Result<Self, RepositoryError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(storage)?;
        let pool = rt.block_on(async {
            let pool = PgPoolOptions::new()
                .max_connections(pool_max_connections())
                .connect_with(opts)
                .await
                .map_err(storage)?;
            sqlx::migrate!("./migrations_pg")
                .run(&pool)
                .await
                .map_err(storage)?;
            Ok::<_, RepositoryError>(pool)
        })?;
        Ok(Self { rt, pool })
    }
}

impl RunRepository for PgStore {
    fn get(&self, ctx: &TenantContext, task_id: &str) -> Result<Option<Run>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let row = self.rt.block_on(async {
            let mut tx = self.pool.begin().await?;
            fixa_tenant(&mut tx, &tenant).await?;
            let row = sqlx::query(&format!(
                "SELECT {RUN_COLS} FROM runs WHERE tenant_id = $1 AND task_id = $2"
            ))
            .bind(&tenant)
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok::<_, sqlx::Error>(row)
        });
        row.map_err(storage)?
            .map(|r| linha_para_run(&r))
            .transpose()
    }

    fn list(&self, ctx: &TenantContext) -> Result<Vec<Run>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(&format!(
                    "SELECT {RUN_COLS} FROM runs WHERE tenant_id = $1 ORDER BY id DESC"
                ))
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter().map(linha_para_run).collect()
    }

    fn save(&mut self, ctx: &TenantContext, run: &Run) -> Result<(), RepositoryError> {
        exige_mesmo_tenant(ctx, run.tenant, "o run")?;
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                upsert_run(&mut tx, run).await?;
                tx.commit().await
            })
            .map_err(storage)
    }

    fn save_with_deliverables(
        &mut self,
        ctx: &TenantContext,
        run: &Run,
        novas: &[Deliverable],
    ) -> Result<(), RepositoryError> {
        exige_mesmo_tenant(ctx, run.tenant, "o run")?;
        let tenant = ctx.tenant.to_string();
        self.rt.block_on(async {
            // A MESMA transação real do critério 4 do G1 que o SQLite serve:
            // run + entregas gravam juntos ou nada grava — o retorno
            // antecipado (recusa fail-closed no meio do lote) derruba `tx`
            // sem commit ⇒ rollback.
            let mut tx = self.pool.begin().await.map_err(storage)?;
            fixa_tenant(&mut tx, &tenant).await.map_err(storage)?;
            upsert_run(&mut tx, run).await.map_err(storage)?;
            let run_row_id: i64 =
                sqlx::query_scalar("SELECT id FROM runs WHERE tenant_id = $1 AND task_id = $2")
                    .bind(&tenant)
                    .bind(run.task_id.to_string())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            for entrega in novas {
                exige_mesmo_tenant(ctx, entrega.tenant, "a entrega")?;
                sqlx::query(
                    "INSERT INTO deliverables (run_id, task_id, template_id, nome, path,
                                               formato, versao, trilha, created_ts, tenant_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(run_row_id)
                .bind(entrega.task_id.to_string())
                .bind(&entrega.template_id)
                .bind(&entrega.nome)
                .bind(&entrega.path)
                .bind(&entrega.formato)
                .bind(&entrega.versao)
                .bind(&entrega.trilha)
                .bind(&entrega.created_ts)
                .bind(&tenant)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            }
            tx.commit().await.map_err(storage)
        })
    }

    fn list_deliverables(&self, ctx: &TenantContext) -> Result<Vec<Deliverable>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(&format!(
                    "SELECT {DELIVERABLE_COLS} FROM deliverables
                     WHERE tenant_id = $1 ORDER BY id DESC"
                ))
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter().map(linha_para_deliverable).collect()
    }

    fn get_deliverable(
        &self,
        ctx: &TenantContext,
        id: i64,
    ) -> Result<Option<Deliverable>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let row = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let row = sqlx::query(&format!(
                    "SELECT {DELIVERABLE_COLS} FROM deliverables
                     WHERE tenant_id = $1 AND id = $2"
                ))
                .bind(&tenant)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(row)
            })
            .map_err(storage)?;
        row.map(|r| linha_para_deliverable(&r)).transpose()
    }

    fn max_task_seq(&self, ctx: &TenantContext) -> Result<u64, RepositoryError> {
        // Mesma semântica do adapter SQLite: o parse do `task_id` é regra de
        // domínio (TaskId), então acontece em Rust — não num SUBSTRING SQL
        // que os dois bancos escreveriam diferente.
        let tenant = ctx.tenant.to_string();
        let raws: Vec<String> = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let raws = sqlx::query_scalar("SELECT task_id FROM runs WHERE tenant_id = $1")
                    .bind(&tenant)
                    .fetch_all(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(raws)
            })
            .map_err(storage)?;
        let mut max = 0u64;
        for raw in raws {
            if let Ok(task) = TaskId::parse(&raw) {
                max = max.max(task.seq());
            }
        }
        Ok(max)
    }
}

/// Upsert por `(tenant_id, task_id)` — `ON CONFLICT` sobre a unicidade que a
/// migration declara; o `id` da linha sobrevive ao update (identidade do
/// agregado é o `task_id` no tenant, como no SQLite).
async fn upsert_run(tx: &mut Transaction<'_, Postgres>, run: &Run) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO runs (tenant_id, task_id, template_id, template_versao, nome,
                           briefing_json, papeis_json, status, gates_aprovados,
                           created_ts, updated_ts)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (tenant_id, task_id) DO UPDATE SET
             template_id = EXCLUDED.template_id,
             template_versao = EXCLUDED.template_versao,
             nome = EXCLUDED.nome,
             briefing_json = EXCLUDED.briefing_json,
             papeis_json = EXCLUDED.papeis_json,
             status = EXCLUDED.status,
             gates_aprovados = EXCLUDED.gates_aprovados,
             created_ts = EXCLUDED.created_ts,
             updated_ts = EXCLUDED.updated_ts",
    )
    .bind(run.tenant.to_string())
    .bind(run.task_id.to_string())
    .bind(&run.template_id)
    .bind(&run.template_versao)
    .bind(&run.nome)
    .bind(&run.briefing_json)
    .bind(&run.papeis_json)
    .bind(run.status.as_str())
    .bind(run.gates_aprovados)
    .bind(&run.created_ts)
    .bind(&run.updated_ts)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl PersonaRepository for PgStore {
    fn list_overrides(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<PersonaOverride>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(
                    "SELECT template_id, papel, prompt, tenant_id FROM persona_overrides
                     WHERE tenant_id = $1 AND template_id = $2",
                )
                .bind(&tenant)
                .bind(template_id)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter()
            .map(|row| {
                let tenant_raw: String = row.try_get(3).map_err(storage)?;
                Ok(PersonaOverride {
                    template_id: row.try_get(0).map_err(storage)?,
                    papel: row.try_get(1).map_err(storage)?,
                    prompt: row.try_get(2).map_err(storage)?,
                    tenant: TenantId::parse(&tenant_raw).map_err(storage)?,
                })
            })
            .collect()
    }

    fn set_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
        prompt: &str,
    ) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                sqlx::query(&format!(
                    "INSERT INTO persona_overrides (tenant_id, template_id, papel, prompt, updated_ts)
                     VALUES ($1, $2, $3, $4, {NOW_UTC_SQL})
                     ON CONFLICT (tenant_id, template_id, papel)
                     DO UPDATE SET prompt = EXCLUDED.prompt, updated_ts = {NOW_UTC_SQL}"
                ))
                .bind(&tenant)
                .bind(template_id)
                .bind(papel)
                .bind(prompt)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            })
            .map_err(storage)
    }

    fn delete_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
    ) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                sqlx::query(
                    "DELETE FROM persona_overrides
                     WHERE tenant_id = $1 AND template_id = $2 AND papel = $3",
                )
                .bind(&tenant)
                .bind(template_id)
                .bind(papel)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            })
            .map_err(storage)
    }

    fn clear_overrides(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                sqlx::query(
                    "DELETE FROM persona_overrides WHERE tenant_id = $1 AND template_id = $2",
                )
                .bind(&tenant)
                .bind(template_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            })
            .map_err(storage)
    }

    fn list_custom(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<CustomPersona>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(
                    "SELECT id, template_id, nome, prompt, tenant_id FROM custom_personas
                     WHERE tenant_id = $1 AND template_id = $2 ORDER BY id",
                )
                .bind(&tenant)
                .bind(template_id)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter()
            .map(|row| {
                let tenant_raw: String = row.try_get(4).map_err(storage)?;
                Ok(CustomPersona {
                    id: row.try_get(0).map_err(storage)?,
                    template_id: row.try_get(1).map_err(storage)?,
                    nome: row.try_get(2).map_err(storage)?,
                    prompt: row.try_get(3).map_err(storage)?,
                    tenant: TenantId::parse(&tenant_raw).map_err(storage)?,
                })
            })
            .collect()
    }

    fn insert_custom(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        nome: &str,
        prompt: &str,
    ) -> Result<i64, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let id: i64 = sqlx::query_scalar(&format!(
                    "INSERT INTO custom_personas (tenant_id, template_id, nome, prompt, updated_ts)
                     VALUES ($1, $2, $3, $4, {NOW_UTC_SQL}) RETURNING id"
                ))
                .bind(&tenant)
                .bind(template_id)
                .bind(nome)
                .bind(prompt)
                .fetch_one(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(id)
            })
            .map_err(storage)
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
        let tenant = ctx.tenant.to_string();
        let n = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let done = sqlx::query(&format!(
                    "UPDATE custom_personas
                     SET nome = $3, prompt = $4, updated_ts = {NOW_UTC_SQL}
                     WHERE tenant_id = $1 AND id = $2"
                ))
                .bind(&tenant)
                .bind(id)
                .bind(nome)
                .bind(prompt)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(done.rows_affected())
            })
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn delete_custom(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let n = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let done =
                    sqlx::query("DELETE FROM custom_personas WHERE tenant_id = $1 AND id = $2")
                        .bind(&tenant)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(done.rows_affected())
            })
            .map_err(storage)?;
        if n == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

impl TemplatePublicationRepository for PgStore {
    fn set_published(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        published: bool,
    ) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                sqlx::query(&format!(
                    "INSERT INTO template_pub (tenant_id, template_id, publicado, updated_ts)
                     VALUES ($1, $2, $3, {NOW_UTC_SQL})
                     ON CONFLICT (tenant_id, template_id)
                     DO UPDATE SET publicado = EXCLUDED.publicado, updated_ts = {NOW_UTC_SQL}"
                ))
                .bind(&tenant)
                .bind(template_id)
                .bind(published)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            })
            .map_err(storage)
    }

    fn list_published(&self, ctx: &TenantContext) -> Result<Vec<(String, bool)>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(
                    "SELECT template_id, publicado FROM template_pub WHERE tenant_id = $1",
                )
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>(0).map_err(storage)?,
                    row.try_get::<bool, _>(1).map_err(storage)?,
                ))
            })
            .collect()
    }
}

impl UserRepository for PgStore {
    fn list(&self, ctx: &TenantContext) -> Result<Vec<User>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query(
                    "SELECT id, nome, email, papel, ativo, pin_hash, tenant_id FROM users
                     WHERE tenant_id = $1 ORDER BY id",
                )
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.iter()
            .map(|row| {
                let tenant_raw: String = row.try_get(6).map_err(storage)?;
                let stored: Option<String> = row.try_get(5).map_err(storage)?;
                Ok(User {
                    id: row.try_get(0).map_err(storage)?,
                    nome: row.try_get(1).map_err(storage)?,
                    email: row.try_get(2).map_err(storage)?,
                    papel: row.try_get(3).map_err(storage)?,
                    ativo: row.try_get(4).map_err(storage)?,
                    has_pin: stored.is_some(),
                    tenant: TenantId::parse(&tenant_raw).map_err(storage)?,
                })
            })
            .collect()
    }

    fn create(
        &mut self,
        ctx: &TenantContext,
        nome: &str,
        email: &str,
        papel: &str,
        pin: Option<&str>,
    ) -> Result<i64, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        self.rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                // O `created_ts` é o salt do pin_hash E a coluna — mesmo valor
                // nos dois; leio o relógio do banco (mesma fonte dos demais).
                let now: String = sqlx::query_scalar(&format!("SELECT {NOW_UTC_SQL}"))
                    .fetch_one(&mut *tx)
                    .await?;
                let hash = pin
                    .filter(|p| !p.is_empty())
                    .map(|p| crate::btv::pin_hash(&now, email, nome, p));
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO users (tenant_id, nome, email, papel, ativo, created_ts, pin_hash)
                     VALUES ($1, $2, $3, $4, true, $5, $6) RETURNING id",
                )
                .bind(&tenant)
                .bind(nome)
                .bind(email)
                .bind(papel)
                .bind(&now)
                .bind(&hash)
                .fetch_one(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(id)
            })
            .map_err(storage)
    }

    fn remove(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let n = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let r = sqlx::query("DELETE FROM users WHERE tenant_id = $1 AND id = $2")
                    .bind(&tenant)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(r.rows_affected())
            })
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
        let tenant = ctx.tenant.to_string();
        let n = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let r = sqlx::query("UPDATE users SET ativo = $3 WHERE tenant_id = $1 AND id = $2")
                    .bind(&tenant)
                    .bind(id)
                    .bind(ativo)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(r.rows_affected())
            })
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
        let tenant = ctx.tenant.to_string();
        let achou = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let row: Option<(String, String, String)> = sqlx::query_as(
                    "SELECT created_ts, email, nome FROM users WHERE tenant_id = $1 AND id = $2",
                )
                .bind(&tenant)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((created_ts, email, nome)) = row else {
                    return Ok::<_, sqlx::Error>(false);
                };
                let hash = pin
                    .filter(|p| !p.is_empty())
                    .map(|p| crate::btv::pin_hash(&created_ts, &email, &nome, p));
                sqlx::query("UPDATE users SET pin_hash = $3 WHERE tenant_id = $1 AND id = $2")
                    .bind(&tenant)
                    .bind(id)
                    .bind(&hash)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(true)
            })
            .map_err(storage)?;
        if !achou {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn verify_pin(
        &self,
        ctx: &TenantContext,
        id: i64,
        pin: &str,
    ) -> Result<PinCheck, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let row: Option<(String, String, String, Option<String>)> = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let row = sqlx::query_as(
                    "SELECT created_ts, email, nome, pin_hash FROM users
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(&tenant)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(row)
            })
            .map_err(storage)?;
        let Some((created_ts, email, nome, stored)) = row else {
            return Err(RepositoryError::NotFound);
        };
        match stored {
            None => Ok(PinCheck::NoPin),
            Some(stored) => {
                let candidate = crate::btv::pin_hash(&created_ts, &email, &nome, pin);
                if candidate == stored {
                    Ok(PinCheck::Ok)
                } else {
                    Ok(PinCheck::Wrong)
                }
            }
        }
    }
}

impl LedgerRepository for PgStore {
    type Entry = LedgerEntry;

    fn append(&mut self, ctx: &TenantContext, event: &DomainEvent) -> Result<u64, RepositoryError> {
        let modelo = entry_de_dominio(ctx, event)?;
        let tenant = ctx.tenant.to_string();
        self.rt.block_on(async {
            // Retry otimista (ADR 0028): quem perde a corrida no
            // UNIQUE (tenant_id, seq) relê o topo NOVO da cadeia e
            // reencadeia — correção igual à do lock, sem estado de sessão.
            for _ in 0..MAX_TENTATIVAS_APPEND {
                let mut tx = self.pool.begin().await.map_err(storage)?;
                fixa_tenant(&mut tx, &tenant).await.map_err(storage)?;
                let topo: Option<(i64, String)> = sqlx::query_as(
                    "SELECT seq, entry_hash FROM ledger
                     WHERE tenant_id = $1 ORDER BY seq DESC LIMIT 1",
                )
                .bind(&tenant)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
                let (prev_seq, prev_hash) = topo.unwrap_or((0, String::new()));
                let mut entry = modelo.clone();
                entry.prev_hash = prev_hash.clone();
                entry.entry_hash = entry.chain_hash(&prev_hash);
                // `body` serializado com `seq: 0`, como sempre (o seq de
                // verdade mora na coluna) — corpo canônico idêntico ao que o
                // adapter SQLite grava para os mesmos eventos.
                let body = serde_json::to_string(&entry).map_err(storage)?;
                let inserido = sqlx::query(
                    "INSERT INTO ledger (tenant_id, seq, prev_hash, entry_hash, body)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(&tenant)
                .bind(prev_seq + 1)
                .bind(&entry.prev_hash)
                .bind(&entry.entry_hash)
                .bind(&body)
                .execute(&mut *tx)
                .await;
                match inserido {
                    Ok(_) => {
                        tx.commit().await.map_err(storage)?;
                        return Ok((prev_seq + 1) as u64);
                    }
                    Err(e) if eh_conflito_unique(&e) => {
                        // Perdeu a corrida: outra conexão gravou este seq.
                        // O drop de `tx` faz rollback; a próxima volta lê o
                        // topo atualizado.
                        drop(tx);
                        continue;
                    }
                    Err(e) => return Err(storage(e)),
                }
            }
            Err(RepositoryError::Storage(format!(
                "append do ledger excedeu {MAX_TENTATIVAS_APPEND} tentativas sob contenção no tenant {tenant}"
            )))
        })
    }

    fn recent(
        &self,
        ctx: &TenantContext,
        limit: u32,
        actor: Option<&ActorId>,
    ) -> Result<Vec<LedgerEntry>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let actor = actor.map(|a| a.as_str().to_string());
        let rows: Vec<(i64, String)> = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                // Mesma regra do SQLite: o filtro de actor entra na MESMA
                // consulta que o LIMIT (o actor mora no body JSON).
                let rows = sqlx::query_as(
                    "SELECT seq, body FROM ledger
                     WHERE tenant_id = $1
                       AND ($2::text IS NULL OR body::jsonb ->> 'actor' = $2)
                     ORDER BY seq DESC LIMIT $3",
                )
                .bind(&tenant)
                .bind(&actor)
                .bind(limit as i64)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.into_iter()
            .map(|(seq, body)| {
                let mut entry: LedgerEntry = serde_json::from_str(&body).map_err(storage)?;
                entry.seq = seq as u64;
                Ok(entry)
            })
            .collect()
    }

    fn verify_chain(&self, ctx: &TenantContext) -> Result<u64, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows: Vec<(i64, String, String, String)> = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query_as(
                    "SELECT seq, prev_hash, entry_hash, body FROM ledger
                     WHERE tenant_id = $1 ORDER BY seq",
                )
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        // A MESMA verificação do adapter SQLite (hash encadeado +
        // anti-transplante) — extraída em `ledger::verifica_cadeia_rows`.
        verifica_cadeia_rows(
            &tenant,
            rows.into_iter()
                .map(|(seq, prev, hash, body)| (seq as u64, prev, hash, body)),
        )
        .map_err(|e| RepositoryError::Storage(e.to_string()))
    }

    fn export(&self, ctx: &TenantContext) -> Result<Vec<LedgerEntry>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows: Vec<(i64, String)> = self
            .rt
            .block_on(async {
                let mut tx = self.pool.begin().await?;
                fixa_tenant(&mut tx, &tenant).await?;
                let rows = sqlx::query_as(
                    "SELECT seq, body FROM ledger WHERE tenant_id = $1 ORDER BY seq",
                )
                .bind(&tenant)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(rows)
            })
            .map_err(storage)?;
        rows.into_iter()
            .map(|(seq, body)| {
                let mut entry: LedgerEntry = serde_json::from_str(&body).map_err(storage)?;
                entry.seq = seq as u64;
                Ok(entry)
            })
            .collect()
    }
}

// ── E1s.1: sessões do modo SaaS (ADR 0029 aceito) ─────────────────────────
//
// SAAS-ONLY POR DESIGN (fronteira declarada no aceite do revisor): sessões
// NÃO existem no modo local — `TenantId::LOCAL` implícito É a ausência delas.
// Por isso não há `SessionsPort` dual-adapter e nenhum análogo em SQLite: a
// regra da suíte de contrato do B2/B4 não se aplica aqui. Tudo vive sob a
// feature `pg`, como o resto do modo saas.

/// Uma sessão resolvida no caminho de auth — o par que o extractor (E1s.2)
/// vira `TenantContext` (`actor = user:{user_id}`, item 6 do ADR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoResolvida {
    pub tenant: TenantId,
    pub user_id: String,
}

/// Um token recém-emitido — o texto EM CLARO existe só aqui, uma vez. O
/// operador o vê no `btv session issue`; o banco guarda só o `token_hash`.
#[derive(Debug, Clone)]
pub struct TokenEmitido {
    pub token: String,
    pub token_hash: String,
}

/// Uma sessão ativa, na visão ADMINISTRATIVA (pós-auth, tenant-escopada) —
/// nunca carrega o token (que não existe no banco), só o hash e os prazos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoAdmin {
    pub token_hash: String,
    pub user_id: String,
    pub absolute_deadline: String,
    pub idle_deadline: String,
}

/// Gera um token opaco: 256 bits do CSPRNG do SO → base64url sem padding,
/// com prefixo `btvs_` (grepável em logs/vazamentos). O `token_hash` é o
/// SHA-256 do token INTEIRO (com prefixo) — o mesmo `sha256_hex` do resto
/// do sistema. Token forte de alta entropia não precisa de KDF caro: a
/// honestidade do `pin_hash` (sha256 simples) com a razão INVERTIDA —
/// KDF protege senha fraca, não segredo aleatório de 256 bits.
fn gerar_token() -> Result<TokenEmitido, RepositoryError> {
    use base64::Engine;
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| RepositoryError::Storage(format!("CSPRNG indisponível: {e}")))?;
    let token = format!(
        "btvs_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    );
    let token_hash = btv_schemas::sha256_hex(&token);
    Ok(TokenEmitido { token, token_hash })
}

impl PgStore {
    /// Emite uma sessão (o mecanismo de EMISSÃO que o esboço não tinha — a
    /// "chave da porta"): gera o token, grava SÓ o hash + os prazos do TTL
    /// (decisão (a): absoluta 30d, ociosidade 24h), e devolve o token em
    /// claro UMA vez. Sem `TenantContext`: emissão é ato de OPERADOR
    /// (`btv session issue`), anterior a qualquer sessão — não há contexto a
    /// exigir, o caller administrativo é confiável por construção (é quem
    /// tem a URL do banco).
    pub fn issue_session(
        &self,
        tenant: &TenantId,
        user_id: &str,
    ) -> Result<TokenEmitido, RepositoryError> {
        let emitido = gerar_token()?;
        let hash = emitido.token_hash.clone();
        let tenant = tenant.to_string();
        let user_id = user_id.to_string();
        self.rt
            .block_on(async {
                sqlx::query(
                    "INSERT INTO sessions
                        (token_hash, tenant_id, user_id, absolute_deadline, idle_deadline)
                     VALUES ($1, $2, $3, now() + interval '30 days',
                             now() + interval '24 hours')",
                )
                .bind(&hash)
                .bind(&tenant)
                .bind(&user_id)
                .execute(&self.pool)
                .await
            })
            .map_err(storage)?;
        Ok(emitido)
    }

    /// Resolve um token no caminho de AUTH — acesso SÓ por igualdade de
    /// `token_hash` (a política do item 5, letra (a)). Numa ÚNICA query:
    /// valida (não revogada, dentro da absoluta E da ociosidade) e RENOVA a
    /// ociosidade (`LEAST(now()+24h, absoluta)` — a renovação nunca ultrapassa
    /// o teto absoluto). Token inválido/expirado/revogado ⇒ zero linhas ⇒
    /// `None` (fail-closed). SEM `fixa_tenant`: esta tabela é a exceção sem
    /// RLS de tenant (o lookup é anterior ao `TenantContext`), e a segurança
    /// vem do token ser inforjável, não de `app.tenant_id`.
    pub fn resolve_session(&self, token: &str) -> Result<Option<SessaoResolvida>, RepositoryError> {
        let hash = btv_schemas::sha256_hex(token);
        let row: Option<(String, String)> = self
            .rt
            .block_on(async {
                sqlx::query_as(
                    "UPDATE sessions
                        SET idle_deadline = LEAST(now() + interval '24 hours', absolute_deadline)
                      WHERE token_hash = $1
                        AND revoked_at IS NULL
                        AND now() < absolute_deadline
                        AND now() < idle_deadline
                      RETURNING tenant_id, user_id",
                )
                .bind(&hash)
                .fetch_optional(&self.pool)
                .await
            })
            .map_err(storage)?;
        row.map(|(tenant, user_id)| {
            Ok(SessaoResolvida {
                tenant: TenantId::parse(&tenant).map_err(storage)?,
                user_id,
            })
        })
        .transpose()
    }

    /// Revoga uma sessão (logout real, item 4) — operação ADMINISTRATIVA
    /// pós-auth, tenant-escopada por WHERE explícito (item 5, letra (c)):
    /// revogar o hash de OUTRO tenant não faz nada (indistinguível de
    /// inexistente). Retorna `true` se uma sessão ativa foi revogada.
    pub fn revoke_session(
        &self,
        ctx: &TenantContext,
        token_hash: &str,
    ) -> Result<bool, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let n = self
            .rt
            .block_on(async {
                sqlx::query(
                    "UPDATE sessions SET revoked_at = now()
                     WHERE token_hash = $1 AND tenant_id = $2 AND revoked_at IS NULL",
                )
                .bind(token_hash)
                .bind(&tenant)
                .execute(&self.pool)
                .await
            })
            .map_err(storage)?
            .rows_affected();
        Ok(n > 0)
    }

    /// Lista as sessões ATIVAS do tenant do contexto — administrativa,
    /// tenant-escopada por WHERE explícito. Nunca devolve o token (que não
    /// existe no banco), só hash + prazos.
    pub fn list_sessions(&self, ctx: &TenantContext) -> Result<Vec<SessaoAdmin>, RepositoryError> {
        let tenant = ctx.tenant.to_string();
        let rows: Vec<(String, String, String, String)> = self
            .rt
            .block_on(async {
                sqlx::query_as(
                    "SELECT token_hash, user_id,
                            to_char(absolute_deadline at time zone 'utc',
                                    'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),
                            to_char(idle_deadline at time zone 'utc',
                                    'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
                       FROM sessions
                      WHERE tenant_id = $1 AND revoked_at IS NULL
                      ORDER BY created_ts DESC",
                )
                .bind(&tenant)
                .fetch_all(&self.pool)
                .await
            })
            .map_err(storage)?;
        Ok(rows
            .into_iter()
            .map(
                |(token_hash, user_id, absolute_deadline, idle_deadline)| SessaoAdmin {
                    token_hash,
                    user_id,
                    absolute_deadline,
                    idle_deadline,
                },
            )
            .collect())
    }
}

/// Harness de TESTE do adapter (usado por `tests/contract_pg.rs` e pelos
/// testes deste módulo; não há uso de produção — o análogo do
/// `open_in_memory` do SQLite, que aqui precisa de schema isolado + role
/// sem privilégio).
pub mod harness {
    use super::*;
    use sqlx::Connection;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// Um `PgStore` num schema isolado + as opções de conexão do MESMO
    /// schema/role, para o teste adversarial abrir conexões CRUAS por fora
    /// das traits.
    pub struct PgIsolado {
        pub store: PgStore,
        pub opts_app: PgConnectOptions,
    }

    /// `true` quando `BTV_PG_TEST_URL` aponta um Postgres — senão o teste
    /// PULA com aviso barulhento (nunca passa fingindo; no CI o job `pg`
    /// sempre define a URL, então lá a metade PG roda de verdade).
    pub fn disponivel() -> bool {
        if std::env::var("BTV_PG_TEST_URL").is_ok() {
            return true;
        }
        eprintln!(
            "AVISO(B4): BTV_PG_TEST_URL ausente — teste do adapter Postgres PULADO \
             (a metade PG da suíte de contrato não rodou nesta máquina; o job `pg` \
             do CI a exige com um Postgres real)"
        );
        false
    }

    /// Abre um `PgStore` num SCHEMA novo, conectado como o role de aplicação
    /// `btv_app_teste` — LOGIN, NOSUPERUSER, NOBYPASSRLS: o RLS das
    /// migrations SE APLICA a ele (superuser bypassaria RLS silenciosamente
    /// e o teste adversarial provaria nada — ele afirma isso).
    pub fn abrir_isolado() -> Option<PgIsolado> {
        let admin_url = std::env::var("BTV_PG_TEST_URL").ok()?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime do harness");
        // Nome único por processo+contador+nanos: schemas de execuções
        // anteriores no MESMO banco local não colidem.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio")
            .subsec_nanos();
        let schema = format!(
            "btv_b4_{}_{}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst),
            nanos
        );
        rt.block_on(async {
            let mut admin = sqlx::postgres::PgConnection::connect(&admin_url)
                .await
                .expect("conexão admin (BTV_PG_TEST_URL)");
            // O advisory lock transacional serializa a criação do ROLE entre
            // testes paralelos (o DO já engole duplicate_object; o lock evita
            // a corrida "tuple concurrently updated" do catálogo).
            let mut tx = admin.begin().await.expect("tx admin");
            sqlx::query("SELECT pg_advisory_xact_lock(48484)")
                .execute(&mut *tx)
                .await
                .expect("advisory do harness");
            sqlx::query(
                "DO $$ BEGIN
                     CREATE ROLE btv_app_teste LOGIN PASSWORD 'btv_app_teste'
                         NOSUPERUSER NOBYPASSRLS;
                 EXCEPTION WHEN duplicate_object THEN NULL; END $$",
            )
            .execute(&mut *tx)
            .await
            .expect("role de aplicação");
            tx.commit().await.expect("commit admin");
            sqlx::query(&format!(
                "CREATE SCHEMA {schema} AUTHORIZATION btv_app_teste"
            ))
            .execute(&mut admin)
            .await
            .expect("schema isolado");
        });
        let opts_app = admin_url
            .parse::<PgConnectOptions>()
            .expect("BTV_PG_TEST_URL válida")
            .username("btv_app_teste")
            .password("btv_app_teste")
            .options([("search_path", schema.as_str())]);
        let store = PgStore::connect_with(opts_app.clone()).expect("PgStore + migrations");
        Some(PgIsolado { store, opts_app })
    }
}

#[cfg(test)]
mod tests {
    use super::harness::{abrir_isolado, disponivel};
    use super::*;
    use btv_domain::ports::DomainEventKind;
    use sqlx::Connection;

    fn ctx_a() -> TenantContext {
        TenantContext::new(
            TenantId::parse("00000000-0000-0000-0000-00000000b4aa").unwrap(),
            ActorId::new("pg:a").unwrap(),
        )
    }

    fn ctx_b() -> TenantContext {
        TenantContext::new(
            TenantId::parse("00000000-0000-0000-0000-00000000b4bb").unwrap(),
            ActorId::new("pg:b").unwrap(),
        )
    }

    fn evento(ctx: &TenantContext, ts: &str) -> DomainEvent {
        DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts: ts.into(),
            kind: DomainEventKind::GateApproved {
                task_id: TaskId::new(1),
                stage: Some("Gate do PG".into()),
                gates_approved: 1,
            },
        }
    }

    fn run_b(ctx: &TenantContext) -> Run {
        Run {
            id: 0,
            task_id: TaskId::new(1),
            template_id: "editorial".into(),
            template_versao: "v1.4".into(),
            nome: "Do tenant B".into(),
            briefing_json: "[]".into(),
            papeis_json: r#"["Redator"]"#.into(),
            status: RunStatus::Ativa,
            gates_aprovados: 0,
            created_ts: "2026-07-10T00:00:00Z".into(),
            updated_ts: "2026-07-10T00:00:00Z".into(),
            tenant: ctx.tenant,
        }
    }

    /// A entrega de primeira classe do B4 (ADR 0026 item 3): uma conexão do
    /// role de aplicação com a sessão fixada no tenant A executa SQL
    /// ADULTERADO — `SELECT` direto nas tabelas, SEM `WHERE`, por fora das
    /// traits — e não lê UMA linha do tenant B. E a sessão que não fixou
    /// tenant NENHUM lê zero linhas (fail-closed), nunca todas.
    #[test]
    fn rls_impede_leitura_de_outro_tenant_mesmo_com_sql_adulterado() {
        let Some(iso) = abrir_isolado() else { return };
        let mut store = iso.store;

        // Popula TODAS as tabelas como tenant B, pelas traits.
        let b = ctx_b();
        store
            .save_with_deliverables(
                &b,
                &run_b(&b),
                &[Deliverable {
                    id: 0,
                    run_id: 0,
                    task_id: TaskId::new(1),
                    template_id: "editorial".into(),
                    nome: "e1".into(),
                    path: "/tmp/e1".into(),
                    formato: "MD".into(),
                    versao: "v1".into(),
                    trilha: "Redator".into(),
                    created_ts: "2026-07-10T00:10:00Z".into(),
                    tenant: b.tenant,
                }],
            )
            .unwrap();
        store
            .set_override(&b, "editorial", "Redator", "prompt do B")
            .unwrap();
        store
            .insert_custom(&b, "editorial", "Persona B", "prompt")
            .unwrap();
        LedgerRepository::append(&mut store, &b, &evento(&b, "t1")).unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            use sqlx::ConnectOptions;
            let mut conn = iso.opts_app.clone().connect().await.expect("conexão crua");

            // Honestidade do harness: se o role pudesse bypassar RLS, este
            // teste inteiro provaria NADA — então ele afirma que não pode.
            let (superuser, bypassa): (bool, bool) = sqlx::query_as(
                "SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user",
            )
            .fetch_one(&mut conn)
            .await
            .unwrap();
            assert!(
                !superuser && !bypassa,
                "harness mal configurado: o role de teste bypassaria RLS"
            );

            let tabelas = [
                "runs",
                "deliverables",
                "persona_overrides",
                "custom_personas",
                "ledger",
            ];

            // Sessão fixada no tenant A: SQL sem WHERE não vê NADA de B.
            let mut tx = conn.begin().await.unwrap();
            sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
                .bind(ctx_a().tenant.to_string())
                .execute(&mut *tx)
                .await
                .unwrap();
            for tabela in tabelas {
                let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
                assert_eq!(n, 0, "RLS vazou {tabela} do tenant B para a sessão de A");
            }
            tx.commit().await.unwrap();

            // Sessão SEM tenant fixado: fail-closed — zero linhas, nunca todas.
            for tabela in tabelas {
                let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
                assert_eq!(
                    n, 0,
                    "sessão sem tenant deveria ver zero linhas de {tabela}"
                );
            }

            // Sanidade (o teste morde nos dois sentidos): fixada em B, as
            // linhas EXISTEM — o zero de cima é o RLS, não um banco vazio.
            let mut tx = conn.begin().await.unwrap();
            sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
                .bind(b.tenant.to_string())
                .execute(&mut *tx)
                .await
                .unwrap();
            for tabela in tabelas {
                let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
                assert!(n >= 1, "{tabela} deveria ter a linha do próprio B");
            }
            tx.commit().await.unwrap();
        });
    }

    /// O juiz da decisão do ADR 0028 (mecanismo de serialização por tenant):
    /// threads com POOLS PRÓPRIOS (conexões separadas — corrida real) fazem
    /// appends concorrentes em DOIS tenants; cada cadeia fecha 1..N sem
    /// buraco e sem fork. É a versão PG do teste de conexões separadas que o
    /// SQLite tem desde a Onda 6 — aqui o retry otimista é quem segura.
    #[test]
    fn appends_concorrentes_de_pools_separados_mantem_as_cadeias_por_tenant() {
        if !disponivel() {
            return;
        }
        let iso = abrir_isolado().unwrap();
        let threads = 4usize;
        let por_thread = 8u64;

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let opts = iso.opts_app.clone();
                std::thread::spawn(move || {
                    let mut store = PgStore::connect_with(opts).expect("PgStore da thread");
                    // Metade das threads grava em A, metade em B — contenção
                    // dentro do tenant, independência entre tenants.
                    let ctx = if t % 2 == 0 { ctx_a() } else { ctx_b() };
                    for i in 0..por_thread {
                        LedgerRepository::append(
                            &mut store,
                            &ctx,
                            &evento(&ctx, &format!("t{t}-{i}")),
                        )
                        .expect("append concorrente");
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        let esperado_por_tenant = (threads as u64 / 2) * por_thread;
        let store = iso.store;
        assert_eq!(
            LedgerRepository::verify_chain(&store, &ctx_a()).unwrap(),
            esperado_por_tenant
        );
        assert_eq!(
            LedgerRepository::verify_chain(&store, &ctx_b()).unwrap(),
            esperado_por_tenant
        );
        // Sem buraco e sem fork: o export enumera exatamente 1..N.
        let export = LedgerRepository::export(&store, &ctx_a()).unwrap();
        let seqs: Vec<u64> = export.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, (1..=esperado_por_tenant).collect::<Vec<_>>());
    }

    /// Migrations são idempotentes: reconectar no MESMO schema não re-roda
    /// nada (o `_sqlx_migrations` do schema registra a versão aplicada) — e
    /// os dados sobrevivem à reconexão.
    #[test]
    fn reconectar_no_mesmo_schema_preserva_dados_e_nao_reroda_migrations() {
        let Some(iso) = abrir_isolado() else { return };
        let mut store = iso.store;
        let b = ctx_b();
        store.save(&b, &run_b(&b)).unwrap();
        drop(store);

        let reaberto = PgStore::connect_with(iso.opts_app.clone()).expect("reconexão");
        let lido = reaberto.get(&b, "sq1").unwrap().expect("run sobrevive");
        assert_eq!(lido.nome, "Do tenant B");
    }

    // ── E1s.1: sessões — o ciclo e o adversarial da tabela do bootstrap ──

    /// O ciclo completo do mecanismo de EMISSÃO (a "chave da porta" que o
    /// esboço não tinha): issue → resolve → revoke. Prova que uma sessão
    /// legítima passa a existir e vira identidade, e que o logout a mata.
    #[test]
    fn sessao_ciclo_emite_resolve_e_revoga() {
        let Some(iso) = abrir_isolado() else { return };
        let store = iso.store;
        let a = ctx_a();
        let emitido = store.issue_session(&a.tenant, "u-42").unwrap();
        assert!(emitido.token.starts_with("btvs_"), "prefixo grepável");
        assert_eq!(emitido.token_hash, btv_schemas::sha256_hex(&emitido.token));

        // Resolve: o token vira o par (tenant, user) que o extractor usa.
        let resolvida = store.resolve_session(&emitido.token).unwrap().unwrap();
        assert_eq!(resolvida.tenant, a.tenant);
        assert_eq!(resolvida.user_id, "u-42");

        // Revoga (logout real) → resolve falha fail-closed.
        assert!(store.revoke_session(&a, &emitido.token_hash).unwrap());
        assert!(store.resolve_session(&emitido.token).unwrap().is_none());
    }

    /// O ADVERSARIAL da tabela do bootstrap (item 5, a política que substitui
    /// o RLS de tenant): (1) um DUMP não autentica — só hashes são gravados,
    /// e apresentar o hash como token não resolve (SHA-256 é one-way);
    /// (2) o caminho de auth é SÓ por igualdade de hash — token aleatório
    /// não casa; (3) admin (listar/revogar) é tenant-escopado — a sessão de
    /// um tenant é invisível e intocável pelo outro.
    #[test]
    fn sessao_bootstrap_adversarial() {
        let Some(iso) = abrir_isolado() else { return };
        let store = iso.store;
        let (a, b) = (ctx_a(), ctx_b());
        let sa = store.issue_session(&a.tenant, "u-a").unwrap();
        let sb = store.issue_session(&b.tenant, "u-b").unwrap();

        // (1) Dump não autentica: o token NUNCA está no banco (nenhuma coluna
        // o guarda), e o hash apresentado como token não resolve. O bloco
        // async lê o dump por conexão CRUA; as chamadas ao `store` (que tem
        // runtime próprio) ficam FORA dele — block_on aninhado em
        // current-thread entra em pânico.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dump_hashes: Vec<String> = rt.block_on(async {
            use sqlx::ConnectOptions;
            let mut conn = iso.opts_app.clone().connect().await.unwrap();
            let colunas: Vec<String> = sqlx::query_scalar(
                "SELECT column_name FROM information_schema.columns
                 WHERE table_name = 'sessions'",
            )
            .fetch_all(&mut conn)
            .await
            .unwrap();
            assert!(
                colunas.iter().all(|c| c != "token" && c != "token_plain"),
                "nenhuma coluna guarda o token em claro — só o hash"
            );
            sqlx::query_scalar("SELECT token_hash FROM sessions")
                .fetch_all(&mut conn)
                .await
                .unwrap()
        });
        // Um dump entrega token_hash; usá-lo como token re-hasheia → valor
        // diferente → não resolve. Preimagem de SHA-256 mata isto.
        assert!(dump_hashes.contains(&sa.token_hash));
        for h in &dump_hashes {
            assert!(
                store.resolve_session(h).unwrap().is_none(),
                "o hash de um dump NÃO autentica"
            );
        }

        // (2) Auth só por igualdade de hash: token forjado aleatório = None.
        assert!(store
            .resolve_session("btvs_TOKEN_FORJADO_QUE_NAO_EXISTE")
            .unwrap()
            .is_none());

        // (3) Admin tenant-escopado: A lista só a sessão de A; revogar a de B
        // pelo contexto de A não faz nada; B se revoga.
        let lista_a = store.list_sessions(&a).unwrap();
        assert_eq!(lista_a.len(), 1);
        assert_eq!(lista_a[0].user_id, "u-a");
        assert!(lista_a.iter().all(|s| s.token_hash != sb.token_hash));
        assert!(
            !store.revoke_session(&a, &sb.token_hash).unwrap(),
            "A não revoga sessão de B (tenant-escopado)"
        );
        assert!(
            store.resolve_session(&sb.token).unwrap().is_some(),
            "B segue viva"
        );
        assert!(store.revoke_session(&b, &sb.token_hash).unwrap());
        assert!(store.resolve_session(&sb.token).unwrap().is_none());
    }

    /// Prova-que-morde da EXPIRAÇÃO (fail-closed do TTL): uma sessão com os
    /// prazos no passado (inserida crua, como o tempo faria) NÃO resolve —
    /// a validade mora no WHERE do resolve, não em confiança.
    #[test]
    fn sessao_expirada_nao_resolve() {
        let Some(iso) = abrir_isolado() else { return };
        let store = iso.store;
        let a = ctx_a();
        let emitido = super::gerar_token().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            // Absoluta e ociosidade no passado: sessão morta pelo tempo.
            sqlx::query(
                "INSERT INTO sessions
                    (token_hash, tenant_id, user_id, absolute_deadline, idle_deadline)
                 VALUES ($1, $2, 'u-velho', now() - interval '1 day',
                         now() - interval '1 hour')",
            )
            .bind(&emitido.token_hash)
            .bind(a.tenant.to_string())
            .execute(&store.pool)
            .await
            .unwrap();
        });
        assert!(
            store.resolve_session(&emitido.token).unwrap().is_none(),
            "sessão expirada é fail-closed"
        );
    }
}
