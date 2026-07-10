//! Ledger append-only com hash-chain POR TENANT (governança BuildToValue,
//! ADR 0027 desde B3).
//!
//! Cada entrada referencia o hash da anterior DENTRO da cadeia do seu
//! tenant; `verify_chain` detecta qualquer adulteração retroativa. Não há
//! UPDATE nem DELETE — apenas INSERT (overrides são novas entradas
//! marcadas). A cadeia local legada É a cadeia do tenant LOCAL: mesmos
//! `seq`, mesmos hashes (backfill de coluna sem re-hash, provado por teste
//! de migração).

use btv_domain::ports::{DomainEvent, DomainEventKind, LedgerRepository, RepositoryError};
use btv_domain::{ActorId, TenantContext};
use btv_schemas::ledger::LedgerEntry;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::time::Duration;

/// UUID textual do tenant do modo local — mesma decisão registrada do B2
/// (`pendencias.md`): a porta legada, sem contexto, é a porta do LOCAL.
const LOCAL_TENANT: &str = "00000000-0000-0000-0000-000000000001";

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("erro de storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("erro de serialização: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("cadeia corrompida na seq {seq}: esperado {expected}, encontrado {found}")]
    BrokenChain {
        seq: u64,
        expected: String,
        found: String,
    },
    /// ADR 0027 item 2 (anti-transplante): o corpo hasheado diz um tenant e
    /// a entrada está na cadeia de outro — reatribuição detectada.
    #[error("entrada transplantada na seq {seq}: o corpo pertence ao tenant {body_tenant}, mas está na cadeia de {chain_tenant}")]
    ForeignEntry {
        seq: u64,
        body_tenant: String,
        chain_tenant: String,
    },
}

pub struct LedgerStore {
    conn: Connection,
}

impl LedgerStore {
    pub fn open(path: &str) -> Result<Self, LedgerError> {
        let conn = Connection::open(path)?;
        // CLI (`btv run`/`chat`/`squad`) e o dashboard web (rotas de
        // permissão/squad) tocam `.btv/btv.db` ao mesmo tempo — sem WAL,
        // isso é "database is locked" esperando pra acontecer (bug de
        // concorrência latente, fechado só agora, Onda 6; mesmo padrão já
        // usado por `EventStore`/`RuleStore::open`).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Escritores concorrentes (CLI/squad e dashboard têm CONEXÕES separadas
        // ao mesmo arquivo) esperam o lock em vez de estourar "database is
        // locked" — casado com o `BEGIN IMMEDIATE` do `append`, que serializa o
        // read-modify-write que encadeia o hash. Sem isso, dois `append` liam o
        // MESMO último hash e o segundo gravava `prev_hash` obsoleto → cadeia
        // violada (o modo WAL, ao deixar leitor e escritor coexistirem, expunha
        // exatamente essa corrida).
        conn.busy_timeout(Duration::from_secs(10))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, LedgerError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, LedgerError> {
        // B3 (ADR 0027): a chave da CADEIA é (tenant_id, seq) — `seq`
        // monotônico POR tenant, `prev_hash` encadeando dentro do tenant.
        // `id` é só identidade física/ordem de inserção global (admin);
        // a verdade auditável é a cadeia.
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS ledger (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id  TEXT NOT NULL DEFAULT '{LOCAL_TENANT}',
                seq        INTEGER NOT NULL,
                prev_hash  TEXT NOT NULL,
                entry_hash TEXT NOT NULL,
                body       TEXT NOT NULL,
                UNIQUE (tenant_id, seq)
            );"
        ))?;
        Self::migrate_legacy(&conn)?;
        Ok(Self { conn })
    }

    /// Migra um ledger PRÉ-tenant (B3, ADR 0027 item 5): a cadeia global
    /// existente VIRA a cadeia do tenant LOCAL — `seq`, `prev_hash` e
    /// `entry_hash` intactos (nenhum corpo é tocado, nenhum hash é
    /// recomputado; as entradas antigas não têm `tenant` no corpo e o corpo
    /// canônico permanece byte-idêntico). REBUILD porque o `seq` deixa de
    /// ser a PK física (dois tenants têm ambos seq=1) — `id` herda o seq
    /// antigo, preservando a ordem física.
    fn migrate_legacy(conn: &Connection) -> Result<(), LedgerError> {
        let tem_tenant: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('ledger') WHERE name = 'tenant_id'")?
            .exists([])?;
        if tem_tenant {
            return Ok(());
        }
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE;
            CREATE TABLE ledger_b3 (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id  TEXT NOT NULL DEFAULT '{LOCAL_TENANT}',
                seq        INTEGER NOT NULL,
                prev_hash  TEXT NOT NULL,
                entry_hash TEXT NOT NULL,
                body       TEXT NOT NULL,
                UNIQUE (tenant_id, seq)
            );
            INSERT INTO ledger_b3 (id, tenant_id, seq, prev_hash, entry_hash, body)
                SELECT seq, '{LOCAL_TENANT}', seq, prev_hash, entry_hash, body FROM ledger;
            DROP TABLE ledger;
            ALTER TABLE ledger_b3 RENAME TO ledger;
            COMMIT;"
        ))?;
        Ok(())
    }

    /// Anexa uma entrada, calculando `seq`, `prev_hash` e `entry_hash` NA
    /// CADEIA DO TENANT da entrada. Porta legada (sem contexto): `tenant`
    /// ausente cai na cadeia LOCAL — mesma decisão registrada do B2
    /// (`pendencias.md`, porta legada = porta do modo local), e o corpo
    /// serializado fica byte-idêntico ao de sempre (hash inalterado, goldens
    /// T1 como juízes). A porta com contexto (`LedgerRepository::append`)
    /// grava `tenant` no corpo hasheado (anti-transplante, ADR 0027 item 2).
    pub fn append(&mut self, mut entry: LedgerEntry) -> Result<LedgerEntry, LedgerError> {
        let chain_tenant = entry
            .tenant
            .map(|t| t.to_string())
            .unwrap_or_else(|| LOCAL_TENANT.to_string());
        // `Immediate`: pega o lock de escrita ANTES do `SELECT` do topo da
        // cadeia, então o read-modify-write é atômico ENTRE conexões. Sem
        // isso (o default `Deferred`), duas conexões concorrentes liam o
        // mesmo `prev_hash` e a segunda encadeava no hash errado → cadeia
        // violada. Global-mas-barato no SQLite local (ADR 0027 item 1); o
        // mecanismo POR tenant do Postgres é decisão diferida da B4.
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let topo: Option<(u64, String)> = tx
            .query_row(
                "SELECT seq, entry_hash FROM ledger
                 WHERE tenant_id = ?1 ORDER BY seq DESC LIMIT 1",
                params![chain_tenant],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (prev_seq, prev_hash) = topo.unwrap_or((0, String::new()));
        entry.prev_hash = prev_hash.clone();
        entry.entry_hash = entry.chain_hash(&prev_hash);
        // O `body` é serializado com `seq: 0`, como sempre foi (o seq de
        // verdade mora na coluna; `recent`/`export` a fazem mandar) — mexer
        // nisso mudaria o corpo canônico das entradas novas sem necessidade.
        let body = serde_json::to_string(&entry)?;
        tx.execute(
            "INSERT INTO ledger (tenant_id, seq, prev_hash, entry_hash, body)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                chain_tenant,
                prev_seq + 1,
                entry.prev_hash,
                entry.entry_hash,
                body
            ],
        )?;
        entry.seq = prev_seq + 1;
        tx.commit()?;
        Ok(entry)
    }

    /// Lista as entradas mais recentes primeiro, opcionalmente filtradas por
    /// um ator exato — mesmo padrão de paginação de `TelemetryStore::recent`.
    /// `actor` não é coluna própria (mora dentro do `body` JSON), então o
    /// filtro entra via `json_extract` na MESMA consulta que o `LIMIT`, não
    /// depois em Rust — senão `?actor=X` devolveria menos que deveria toda
    /// vez que outros atores aparecerem entre as N mais recentes.
    ///
    /// Porta legada (B3): escopo fixo na cadeia LOCAL — num banco
    /// multi-tenant, `GET /api/ledger` (que chama isto) não vaza a trilha de
    /// outros tenants. No modo local, resposta byte-idêntica (golden T1).
    pub fn recent(&self, limit: u32, actor: Option<&str>) -> Result<Vec<LedgerEntry>, LedgerError> {
        self.recent_in_chain(LOCAL_TENANT, limit, actor)
    }

    fn recent_in_chain(
        &self,
        tenant: &str,
        limit: u32,
        actor: Option<&str>,
    ) -> Result<Vec<LedgerEntry>, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, body FROM ledger
             WHERE tenant_id = ?3 AND (?2 IS NULL OR json_extract(body, '$.actor') = ?2)
             ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit, actor, tenant], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, body) = row?;
            // `body` carrega `seq: 0` (só é conhecido depois do INSERT em
            // `append`, tarde demais pra entrar no JSON serializado) — a
            // coluna é sempre quem manda no `seq` de verdade.
            let mut entry: LedgerEntry = serde_json::from_str(&body)?;
            entry.seq = seq;
            out.push(entry);
        }
        Ok(out)
    }

    /// Verifica TODAS as cadeias do arquivo (uma por tenant), devolvendo o
    /// total de entradas — no modo local (um tenant) é exatamente a
    /// varredura única de sempre, mesmo resultado. Custo do loop por tenant
    /// declarado no ADR 0027 (consequências).
    pub fn verify_chain(&self) -> Result<u64, LedgerError> {
        let tenants: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT DISTINCT tenant_id FROM ledger ORDER BY tenant_id")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut total = 0u64;
        for tenant in tenants {
            total += self.verify_tenant_chain(&tenant)?;
        }
        Ok(total)
    }

    /// Percorre UMA cadeia (a do tenant) do primeiro `prev_hash = ""` ao
    /// topo, validando os hashes encadeados E que o `tenant` do corpo —
    /// quando presente — é o dono da cadeia (anti-transplante, ADR 0027
    /// item 2: entradas legadas sem `tenant` no corpo passam; uma entrada
    /// com corpo de OUTRO tenant nesta cadeia é reatribuição detectada).
    fn verify_tenant_chain(&self, tenant: &str) -> Result<u64, LedgerError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, prev_hash, entry_hash, body FROM ledger
             WHERE tenant_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![tenant], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let rows = rows.collect::<Result<Vec<_>, _>>()?;
        verifica_cadeia_rows(tenant, rows)
    }

    /// A cadeia COMPLETA de um tenant, do `seq` 1 ao topo (ordem
    /// ascendente) — o export portátil do ADR 0027 item 4: verificável
    /// isoladamente, sem um único hash de outro tenant.
    fn export_chain(&self, tenant: &str) -> Result<Vec<LedgerEntry>, LedgerError> {
        let mut stmt = self
            .conn
            .prepare("SELECT seq, body FROM ledger WHERE tenant_id = ?1 ORDER BY seq")?;
        let rows = stmt.query_map(params![tenant], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, body) = row?;
            let mut entry: LedgerEntry = serde_json::from_str(&body)?;
            entry.seq = seq;
            out.push(entry);
        }
        Ok(out)
    }
}

// ── B3: porta tipada dos FATOS DE DOMÍNIO (LedgerRepository, ADR 0026/0027) ─
//
// Escopo declarado no rustdoc do port (lacuna 1 da revisão do G1): esta
// porta registra os kinds `btv.*` de `DomainEventKind`; as entradas
// operacionais continuam entrando por `LedgerStore::append`/`Session::note`
// — as DUAS portas alimentam a MESMA cadeia por tenant deste arquivo.

/// O payload EXATO do wire para cada variante — chaves em pt, como os
/// emissores de `btv_agent.rs` sempre gravaram (decisão da revisão do G1: o
/// DTO do adapter é o ÚNICO dono das chaves pt; o domínio fica em inglês).
/// `pub(crate)` desde B4: o adapter Postgres usa ESTA função — dois adapters,
/// um só DTO (paridade por construção, cobrada pelo teste de determinismo
/// cross-adapter da suíte).
///
/// Lacunas DECLARADAS vs. os emissores atuais (registradas em
/// `pendencias.md`, decisão de C3): `btv.squad_activated` hoje grava também
/// `template_versao`/`nome`/`papeis`/`personas_proprias`/`prompt_hashes`/
/// `refs` e `btv.export_generated` grava `trilha` — campos que as variantes
/// aceitas no G1 não carregam; `btv.gate_approved` aqui ganha
/// `gates_aprovados` (enriquecimento deliberado da variante). Igualar os
/// dois lados é decisão do dono quando C3 trocar os emissores — com os
/// goldens como juízes e regravação consciente.
pub(crate) fn payload_wire(kind: &DomainEventKind) -> serde_json::Value {
    match kind {
        DomainEventKind::SquadActivated {
            task_id,
            run_id,
            template_id,
        } => serde_json::json!({
            "task_id": task_id,
            "run_id": run_id,
            "template_id": template_id,
        }),
        DomainEventKind::GateApproved {
            task_id,
            stage,
            gates_approved,
        } => serde_json::json!({
            "task_id": task_id,
            "etapa": stage.clone().unwrap_or_default(),
            "gates_aprovados": gates_approved,
        }),
        DomainEventKind::AdjustRequested {
            task_id,
            stage,
            instruction,
            gate_released,
        } => serde_json::json!({
            "task_id": task_id,
            "etapa": stage.clone().unwrap_or_default(),
            "instrucao": instruction,
            "gate_liberado": gate_released,
        }),
        DomainEventKind::DeliverableProduced {
            task_id,
            deliverable_id,
            name,
            format,
        } => serde_json::json!({
            "task_id": task_id,
            "deliverable_id": deliverable_id,
            "nome": name,
            "formato": format,
        }),
        DomainEventKind::PersonaUpdated {
            template_id,
            role,
            prompt_sha256,
        } => serde_json::json!({
            "template_id": template_id,
            "papel": role,
            "prompt_sha256": prompt_sha256,
        }),
        DomainEventKind::TemplatePublished {
            template_id,
            published,
        } => serde_json::json!({
            "template_id": template_id,
            "publicado": published,
        }),
        DomainEventKind::FlowSaved {
            name,
            blocks,
            diagram_sha256,
            semantic_version,
            snapshot_hash,
            audit_head,
            audit_len,
        } => serde_json::json!({
            "nome": name,
            "blocos": blocks,
            "diagram_sha256": diagram_sha256,
            "versao_semantica": semantic_version,
            "snapshot_hash": snapshot_hash,
            "audit_head": audit_head,
            "audit_len": audit_len,
        }),
        DomainEventKind::UserRemoved { user_id } => serde_json::json!({ "id": user_id }),
    }
}

/// Constrói o `LedgerEntry` de um `DomainEvent` — o DTO ÚNICO da porta de
/// domínio, compartilhado pelos DOIS adapters (SQLite aqui, Postgres em
/// `pg.rs` desde B4): mesma serialização canônica ⇒ mesmos hashes, provado
/// pelo teste de determinismo cross-adapter. Fail-closed embutido (mesma
/// regra do B2): evento de outro tenant não entra pela cadeia do contexto.
pub(crate) fn entry_de_dominio(
    ctx: &TenantContext,
    event: &DomainEvent,
) -> Result<LedgerEntry, RepositoryError> {
    if event.tenant != ctx.tenant {
        return Err(RepositoryError::Storage(format!(
            "recusado (fail-closed): evento pertence ao tenant {}, não ao do contexto {}",
            event.tenant, ctx.tenant
        )));
    }
    Ok(LedgerEntry {
        seq: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        kind: event.kind.wire_kind().into(),
        // O `ts`/`actor` do evento são a verdade — o adapter não os
        // reescreve (contrato do port, aceito no G1).
        actor: event.actor.as_str().into(),
        payload: payload_wire(&event.kind),
        r#override: None,
        fake_marker: None,
        ts: event.ts.clone(),
        // Entradas novas nascem com o tenant NO CORPO HASHEADO
        // (ADR 0027 item 2) — transplante entre cadeias quebra o hash.
        tenant: Some(ctx.tenant),
    })
}

/// Percorre as linhas de UMA cadeia (ordenadas por `seq` ascendente),
/// validando hashes encadeados + anti-transplante — a MESMA verificação para
/// os dois adapters (extraída em B4; o SQLite a usava inline desde B3).
/// Linha = `(seq, prev_hash, entry_hash, body)`.
pub(crate) fn verifica_cadeia_rows(
    tenant: &str,
    rows: impl IntoIterator<Item = (u64, String, String, String)>,
) -> Result<u64, LedgerError> {
    let mut expected_prev = String::new();
    let mut count = 0u64;
    for (seq, prev_hash, entry_hash, body) in rows {
        let entry: LedgerEntry = serde_json::from_str(&body)?;
        if let Some(body_tenant) = entry.tenant {
            if body_tenant.to_string() != tenant {
                return Err(LedgerError::ForeignEntry {
                    seq,
                    body_tenant: body_tenant.to_string(),
                    chain_tenant: tenant.to_string(),
                });
            }
        }
        let recomputed = entry.chain_hash(&expected_prev);
        if prev_hash != expected_prev || entry_hash != recomputed {
            return Err(LedgerError::BrokenChain {
                seq,
                expected: recomputed,
                found: entry_hash,
            });
        }
        expected_prev = entry_hash;
        count += 1;
    }
    Ok(count)
}

impl LedgerRepository for LedgerStore {
    /// Gatilho do `AuditEntry` próprio do domínio registrado na revisão do
    /// G1 (pendencias.md): até o domínio precisar INTERPRETAR entradas, o
    /// contrato é o `ledger-entry.v1` de `btv-schemas`.
    type Entry = LedgerEntry;

    fn append(&mut self, ctx: &TenantContext, event: &DomainEvent) -> Result<u64, RepositoryError> {
        let entry = entry_de_dominio(ctx, event)?;
        LedgerStore::append(self, entry)
            .map(|e| e.seq)
            .map_err(|e| RepositoryError::Storage(e.to_string()))
    }

    fn recent(
        &self,
        ctx: &TenantContext,
        limit: u32,
        actor: Option<&ActorId>,
    ) -> Result<Vec<LedgerEntry>, RepositoryError> {
        self.recent_in_chain(&ctx.tenant.to_string(), limit, actor.map(|a| a.as_str()))
            .map_err(|e| RepositoryError::Storage(e.to_string()))
    }

    fn verify_chain(&self, ctx: &TenantContext) -> Result<u64, RepositoryError> {
        self.verify_tenant_chain(&ctx.tenant.to_string())
            .map_err(|e| RepositoryError::Storage(e.to_string()))
    }

    fn export(&self, ctx: &TenantContext) -> Result<Vec<LedgerEntry>, RepositoryError> {
        self.export_chain(&ctx.tenant.to_string())
            .map_err(|e| RepositoryError::Storage(e.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(kind: &str) -> LedgerEntry {
        entry_with_actor(kind, "test")
    }

    fn entry_with_actor(kind: &str, actor: &str) -> LedgerEntry {
        LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: kind.into(),
            actor: actor.into(),
            payload: json!({"n": 1}),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:00Z".into(),
            tenant: None,
        }
    }

    fn ctx_local() -> TenantContext {
        TenantContext::local(btv_domain::ActorId::new("test:b3").unwrap())
    }

    fn ctx_b() -> TenantContext {
        TenantContext::new(
            btv_domain::TenantId::parse("00000000-0000-0000-0000-0000000000b3").unwrap(),
            btv_domain::ActorId::new("test:b3-outro").unwrap(),
        )
    }

    fn evento(ctx: &TenantContext, ts: &str) -> DomainEvent {
        DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts: ts.into(),
            kind: DomainEventKind::GateApproved {
                task_id: btv_domain::TaskId::new(1),
                stage: Some("Aprovar o rascunho".into()),
                gates_approved: 1,
            },
        }
    }

    #[test]
    fn append_encadeia_e_verifica() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let first = store.append(entry("session.start")).unwrap();
        let second = store.append(entry("tool.run")).unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(second.prev_hash, first.entry_hash);
        assert_eq!(store.verify_chain().unwrap(), 2);
    }

    #[test]
    fn appends_concorrentes_de_conexoes_separadas_mantem_a_cadeia() {
        // Reproduz a corrida real de produção: CLI/squad e dashboard têm
        // CONEXÕES SEPARADAS ao mesmo `.btv/btv.db`. Antes do `BEGIN IMMEDIATE`,
        // dois `append` simultâneos liam o mesmo último hash e a segunda
        // entrada encadeava no hash errado → `verify_chain` acusava violação.
        // Com o conserto, N threads × M entradas mantêm a cadeia íntegra.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.db");
        let p = path.to_str().unwrap().to_string();
        // Semeia o schema uma vez (evita corrida no CREATE TABLE inicial).
        LedgerStore::open(&p).unwrap();

        let threads = 6u64;
        let por_thread = 20u64;
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let p = p.clone();
                std::thread::spawn(move || {
                    let mut store = LedgerStore::open(&p).unwrap();
                    for _ in 0..por_thread {
                        store
                            .append(entry_with_actor("tool.run", &format!("t{t}")))
                            .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        // Conexão fresca valida a cadeia inteira: nenhuma quebra.
        let store = LedgerStore::open(&p).unwrap();
        assert_eq!(store.verify_chain().unwrap(), threads * por_thread);
    }

    #[test]
    fn adulteracao_e_detectada() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store.append(entry("session.start")).unwrap();
        store.append(entry("tool.run")).unwrap();
        store
            .conn
            .execute(
                "UPDATE ledger SET body = replace(body, '\"n\":1', '\"n\":2') WHERE seq = 1",
                [],
            )
            .unwrap();
        assert!(matches!(
            store.verify_chain(),
            Err(LedgerError::BrokenChain { seq: 1, .. })
        ));
    }

    /// Fase 5 Onda 4: a certificação é PRODUZIDA em Python
    /// (`btv_review.certification.certify`) e REGISTRADA aqui — o ledger
    /// já suporta qualquer `kind`/`payload` livre, então nenhuma mudança de
    /// produção é necessária; este teste prova a capacidade com um payload
    /// no formato real que `Certification.model_dump()` produziria.
    #[test]
    fn certificacao_registra_no_ledger_com_cadeia_integra() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store.append(entry("session.start")).unwrap();

        let certification_payload = json!({
            "run_id": "run-1",
            "git_sha": "deadbeef",
            "verdict": {
                "approved": true,
                "value_score": 0.86,
                "reason": "aprovado por média ponderada",
                "gate_triggered": null,
            },
            "evidence_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "steps_summary": ["test: ok", "lint: ok"],
            "produced_at": "2026-07-05T00:00:00Z",
        });
        let cert_entry = LedgerEntry {
            seq: 0,
            prev_hash: String::new(),
            entry_hash: String::new(),
            kind: "certification".into(),
            actor: "btv_review".into(),
            payload: certification_payload.clone(),
            r#override: None,
            fake_marker: None,
            ts: "2026-07-05T00:00:01Z".into(),
            tenant: None,
        };

        let registered = store.append(cert_entry).unwrap();
        assert_eq!(registered.seq, 2);
        assert_eq!(registered.kind, "certification");
        assert_eq!(
            registered.payload["evidence_hash"],
            certification_payload["evidence_hash"]
        );
        assert_eq!(store.verify_chain().unwrap(), 2);
    }

    /// Fronteira da Onda 6: `recent` devolve exatamente as N mais recentes,
    /// mais nova primeiro, com `seq`/hashes batendo por igualdade com o que
    /// `append` de fato gravou (não um dump reformatado).
    #[test]
    fn recent_lista_mais_recentes_primeiro_e_respeita_limit() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let a = store.append(entry("session.start")).unwrap();
        let b = store.append(entry("tool.run")).unwrap();
        let c = store.append(entry("tool.result")).unwrap();

        let last_two = store.recent(2, None).unwrap();
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].seq, c.seq);
        assert_eq!(last_two[0].entry_hash, c.entry_hash);
        assert_eq!(last_two[0].prev_hash, c.prev_hash);
        assert_eq!(last_two[1].seq, b.seq);
        // `a` não aparece — respeitou o limite de 2.
        assert!(last_two.iter().all(|e| e.seq != a.seq));
    }

    /// O filtro por ator precisa combinar com o LIMIT na MESMA consulta —
    /// se filtrasse só depois de truncar para as N mais recentes, um ator
    /// raro nas últimas N aparições sumiria mesmo tendo entradas de verdade.
    #[test]
    fn recent_filtra_por_actor_combinado_com_o_limit() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        let raro = store
            .append(entry_with_actor("user.turn", "humano"))
            .unwrap();
        for _ in 0..5 {
            store.append(entry_with_actor("llm.turn", "build")).unwrap();
        }

        // Sem filtro, um LIMIT 3 não veria a entrada de "humano" (é a mais antiga).
        let sem_filtro = store.recent(3, None).unwrap();
        assert!(sem_filtro.iter().all(|e| e.actor != "humano"));

        // Com filtro, mesmo um LIMIT pequeno encontra a entrada certa.
        let filtrado = store.recent(3, Some("humano")).unwrap();
        assert_eq!(filtrado.len(), 1);
        assert_eq!(filtrado[0].seq, raro.seq);
        assert_eq!(filtrado[0].actor, "humano");

        let inexistente = store.recent(50, Some("ninguem")).unwrap();
        assert!(inexistente.is_empty());
    }

    /// B3, o teste do TRANSPLANTE (ADR 0027 item 2 — o ataque exato que o
    /// tenant-no-corpo-hasheado existe para fechar): mover uma entrada da
    /// cadeia do tenant B para a cadeia do tenant A (só a COLUNA muda — é o
    /// que um UPDATE malicioso consegue sem recomputar hash nenhum) é
    /// detectado pelo `verify_chain` de A. Sem o corpo hasheado carregar o
    /// tenant, uma cadeia A vazia aceitaria a entrada estrangeira com
    /// `prev_hash = ""` estruturalmente válido.
    #[test]
    fn transplante_de_entrada_entre_cadeias_e_detectado() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        // B constrói a própria cadeia pela porta com contexto (tenant no corpo).
        LedgerRepository::append(&mut store, &ctx_b(), &evento(&ctx_b(), "t1")).unwrap();
        assert_eq!(LedgerRepository::verify_chain(&store, &ctx_b()).unwrap(), 1);
        assert_eq!(
            LedgerRepository::verify_chain(&store, &ctx_local()).unwrap(),
            0
        );

        // Ataque: reatribui a entrada de B para a cadeia LOCAL mudando SÓ a
        // coluna (nenhum hash recomputado — exatamente o que o item 2 fecha).
        store
            .conn
            .execute(
                "UPDATE ledger SET tenant_id = ?1 WHERE tenant_id = ?2",
                params![super::LOCAL_TENANT, ctx_b().tenant.to_string()],
            )
            .unwrap();

        // A cadeia LOCAL agora contém um corpo que jura pertencer a B:
        // ForeignEntry, na seq certa.
        let err = LedgerRepository::verify_chain(&store, &ctx_local()).unwrap_err();
        assert!(
            err.to_string().contains("transplantada"),
            "esperava detecção de transplante, veio: {err}"
        );
        // E a verificação legada (todas as cadeias) também acusa.
        assert!(matches!(
            store.verify_chain(),
            Err(LedgerError::ForeignEntry { seq: 1, .. })
        ));
    }

    /// B3: cadeias INDEPENDENTES por tenant — appends intercalados de dois
    /// tenants numeram 1..N cada um, `prev_hash` encadeia dentro do tenant
    /// (a primeira entrada de CADA cadeia tem `prev_hash = ""`), e as duas
    /// portas (legada sem tenant + `LedgerRepository` com contexto)
    /// alimentam a MESMA cadeia LOCAL.
    #[test]
    fn cadeias_por_tenant_sao_independentes_e_as_duas_portas_compoem() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        // Porta legada (sem tenant) → cadeia LOCAL, seq 1.
        let legada = store.append(entry("session.start")).unwrap();
        assert_eq!(legada.seq, 1);
        assert_eq!(legada.prev_hash, "");
        // Tenant B intercala: cadeia própria começa do "" com seq 1.
        let b1 = LedgerRepository::append(&mut store, &ctx_b(), &evento(&ctx_b(), "t1")).unwrap();
        assert_eq!(b1, 1, "cadeia de B numera do 1, ignorando o LOCAL");
        // Porta com contexto no LOCAL → continua a MESMA cadeia legada.
        let l2 = LedgerRepository::append(&mut store, &ctx_local(), &evento(&ctx_local(), "t2"))
            .unwrap();
        assert_eq!(l2, 2, "as duas portas compõem a mesma cadeia LOCAL");
        let b2 = LedgerRepository::append(&mut store, &ctx_b(), &evento(&ctx_b(), "t3")).unwrap();
        assert_eq!(b2, 2);

        // Cada cadeia verifica isolada; recent/export não vazam entre tenants.
        assert_eq!(
            LedgerRepository::verify_chain(&store, &ctx_local()).unwrap(),
            2
        );
        assert_eq!(LedgerRepository::verify_chain(&store, &ctx_b()).unwrap(), 2);
        assert_eq!(store.verify_chain().unwrap(), 4, "legado soma as cadeias");
        let export_b = LedgerRepository::export(&store, &ctx_b()).unwrap();
        assert_eq!(export_b.len(), 2);
        assert_eq!(export_b[0].prev_hash, "", "cadeia de B fecha isolada");
        assert_eq!(export_b[1].prev_hash, export_b[0].entry_hash);
        assert!(export_b.iter().all(|e| e.tenant == Some(ctx_b().tenant)));
        // A porta legada de leitura (GET /api/ledger) segue no LOCAL.
        let local = store.recent(10, None).unwrap();
        assert_eq!(local.len(), 2);
        assert!(local.iter().all(|e| e.actor != "test:b3-outro"));
    }

    /// B3: o payload da porta de domínio usa as chaves pt do wire (o DTO é
    /// o único dono delas — decisão da revisão do G1) e o evento chega com
    /// kind/actor/ts do evento, não reescritos pelo adapter.
    #[test]
    fn append_de_domain_event_grava_kind_e_payload_do_wire() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        LedgerRepository::append(
            &mut store,
            &ctx_local(),
            &evento(&ctx_local(), "2026-07-10T00:00:00Z"),
        )
        .unwrap();
        let entradas = LedgerRepository::recent(&store, &ctx_local(), 10, None).unwrap();
        assert_eq!(entradas.len(), 1);
        let e = &entradas[0];
        assert_eq!(e.kind, "btv.gate_approved");
        assert_eq!(e.actor, "test:b3");
        assert_eq!(e.ts, "2026-07-10T00:00:00Z");
        assert_eq!(e.payload["task_id"], "sq1");
        assert_eq!(e.payload["etapa"], "Aprovar o rascunho");
        assert_eq!(e.payload["gates_aprovados"], 1);
        assert_eq!(e.tenant, Some(btv_domain::TenantId::LOCAL));

        // Fail-closed: evento de outro tenant não entra pelo contexto errado.
        assert!(
            LedgerRepository::append(&mut store, &ctx_local(), &evento(&ctx_b(), "t")).is_err()
        );
    }

    /// Onda 6: `LedgerStore::open` liga WAL (bug de concorrência latente,
    /// exposto quando CLI e dashboard web tocam `.btv/btv.db` juntos).
    /// `open_in_memory` (usado pelo resto destes testes) não suporta WAL —
    /// por isso este teste, especificamente, abre um arquivo real.
    #[test]
    fn open_liga_wal_no_arquivo_real() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("btv.db");
        let store = LedgerStore::open(path.to_str().unwrap()).unwrap();
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
