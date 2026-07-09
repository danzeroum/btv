//! G1 — ASSINATURAS para revisão humana (B1 + esqueleto do agregado A4).
//!
//! **Nada aqui tem implementação** (`todo!` onde o Rust exige corpo): errar
//! assinatura multiplica retrabalho por todas as trilhas, então este arquivo
//! para no portão G1 e espera o aceite antes de qualquer adapter (Trilha B)
//! ou serviço de aplicação (C3). Cada decisão carrega o porquê no rustdoc —
//! citável pelo ADR que fechar o G1.
//!
//! Os quatro critérios de julgamento (revisão do G0) e onde cada um mora:
//! 1. **Tenant fail-closed sem exceção** — todo método de toda trait recebe
//!    `&TenantContext` (tenant + actor: o actor alimenta o ledger e não se
//!    perde na assinatura). Não há método de conveniência sem contexto.
//! 2. **Assinaturas limpas de infraestrutura** — nenhum `rusqlite::Error`/
//!    `serde_json::Value`/tipo de wire; erros são enums de domínio
//!    (`RepositoryError`/`RunError`), tipos de entrada/saída são deste crate
//!    (o que ainda é do adapter fica em associated type, decisão visível).
//! 3. **O agregado é a única porta** — `Run::aprovar_gate` valida a
//!    transição, incrementa o contador e RETORNA o `DomainEvent`; as traits
//!    NÃO têm `update_status`/`increment_gates` — mutação por fora do
//!    agregado não existe na API.
//! 4. **Atomicidade no tipo** — `RunRepository::save_with_deliverables` é a
//!    unidade transacional run+entregas; não há como usar a API
//!    corretamente e quebrar essa consistência.

use crate::run::{Deliverable, Run};
use crate::tenant::{ActorId, TenantContext, TenantId};

// ── status do run (A3, aqui só a FORMA — a máquina de transições) ──────────

/// Os 4 estados reais do banco (`wire-strings.v1.json`, provado por T3).
/// `as_str`/`parse` devem reproduzir exatamente `ativa`/`concluida`/
/// `encerrada`/`erro` — byte-a-byte, é contrato de banco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Ativa,
    Concluida,
    Encerrada,
    Erro,
}

#[allow(unused_variables)] // corpos todo!() até o aceite do G1
impl RunStatus {
    /// A string EXATA persistida hoje (contrato T3).
    pub fn as_str(self) -> &'static str {
        todo!("A3, após aceite do G1")
    }

    /// Parse fail-closed: string fora do vocabulário é `RunError::InvalidStatus`.
    pub fn parse(s: &str) -> Result<Self, RunError> {
        todo!("A3, após aceite do G1")
    }

    /// Máquina de transições (A3): `Concluida → Ativa` retorna `false` — e
    /// `Run::transicionar` a transforma em `RunError::InvalidTransition` em
    /// vez de UPDATE silencioso. Transições válidas hoje: `Ativa` → qualquer
    /// terminal; terminal → nada (a reconciliação de zumbis é `Ativa →
    /// Encerrada`, coberta).
    pub fn can_transition_to(self, target: RunStatus) -> bool {
        todo!("A3, após aceite do G1")
    }
}

// ── eventos de domínio (A5) ─────────────────────────────────────────────────

/// Evento de domínio: `tenant` e `actor` são obrigatórios ESTRUTURALMENTE —
/// ficam no envelope, não em cada variante, então não existe evento sem
/// autoria nem dono (critério A5; billing/metering da Trilha E consome isto
/// sem tocar no fluxo). O ledger passa a consumir `DomainEvent`, não string:
/// cada variante mapeia 1:1 para um kind já existente no wire
/// (`btv.squad_activated`, `btv.gate_approved`, … — inventário em
/// `wire-strings.v1.json`), então a serialização NÃO muda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    pub tenant: TenantId,
    pub actor: ActorId,
    /// RFC3339 — vem do chamador (o domínio não lê relógio: determinismo).
    pub ts: String,
    pub kind: DomainEventKind,
}

/// Variantes em inglês (ADR 0024, decisão 2) mapeando os nomes do plano A5:
/// `SquadAtivada`→`SquadActivated`, `GateAprovado`→`GateApproved`,
/// `AjusteSolicitado`→`AdjustRequested`, `EntregaProduzida`→
/// `DeliverableProduced`, `PersonaAtualizada`→`PersonaUpdated`,
/// `TemplatePublicado`→`TemplatePublished`. Os kinds do wire já eram inglês
/// snake_case — o enum alinha código e wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventKind {
    /// `btv.squad_activated`
    SquadActivated {
        task_id: String,
        run_id: i64,
        template_id: String,
    },
    /// `btv.gate_approved` — carrega o contador APÓS o incremento: o evento
    /// é o fato consumado que o ledger registra.
    GateApproved {
        task_id: String,
        etapa: Option<String>,
        gates_aprovados: i64,
    },
    /// `btv.adjust_requested`
    AdjustRequested {
        task_id: String,
        etapa: Option<String>,
        instrucao: String,
        gate_liberado: bool,
    },
    /// `btv.export_generated`
    DeliverableProduced {
        task_id: String,
        deliverable_id: i64,
        nome: String,
        formato: String,
    },
    /// `btv.persona_updated` — hash, nunca o prompt em claro (procedência).
    PersonaUpdated {
        template_id: String,
        papel: String,
        prompt_sha256: String,
    },
    /// `btv.template_published`
    TemplatePublished {
        template_id: String,
        publicado: bool,
    },
    /// `btv.user_removed`
    UserRemoved { user_id: i64 },
}

// ── erros de domínio (critério 2: semânticos, zero infraestrutura) ──────────

/// Regras do agregado violadas — erro de NEGÓCIO, não de storage.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RunError {
    #[error("transição de status inválida: {from:?} → {to:?}")]
    InvalidTransition { from: RunStatus, to: RunStatus },
    #[error("status fora do vocabulário do banco: {0}")]
    InvalidStatus(String),
}

/// Semântica de persistência SEM tipo de driver: o adapter (SQLite/Postgres,
/// ADR 0026) traduz o erro dele para ISTO — `rusqlite::Error`/`sqlx::Error`
/// nunca atravessam a fronteira. `Storage(String)` carrega a mensagem para
/// diagnóstico sem acoplar o chamador ao driver.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("registro não encontrado")]
    NotFound,
    /// Corrida perdida no read-modify-write (ex.: append otimista do
    /// EventStore; candidato de B4 para o ledger PG — ADR 0027 item 1).
    #[error("conflito de concorrência: esperava head {expected}, encontrou {found}")]
    ConcurrencyConflict { expected: i64, found: i64 },
    #[error("erro de storage: {0}")]
    Storage(String),
}

// ── agregado Run (A4, esqueleto) ────────────────────────────────────────────

#[allow(unused_variables)] // corpos todo!() até o aceite do G1
impl Run {
    /// Aprovar o gate HITL pendente — ÚNICA porta para incrementar
    /// `gates_aprovados` (critério 3). Valida que o run está `Ativa` (a
    /// máquina de `RunStatus` decide), incrementa o contador E retorna o
    /// `DomainEvent::GateApproved` com tenant/actor do contexto — o chamador
    /// (serviço de aplicação, C3) persiste via `RunRepository::save` e
    /// registra via `LedgerRepository::append`, mas NÃO decide: o contador
    /// deixa de ser um i64 solto mutado por UPDATE.
    pub fn aprovar_gate(&mut self, ctx: &TenantContext) -> Result<DomainEvent, RunError> {
        todo!("A4, após aceite do G1")
    }

    /// Transição de status pela máquina de `RunStatus` — substitui o
    /// `set_status(task_id, "qualquer_string")` atual. `Concluida → Ativa`
    /// não compila no chamador tipado e retorna `InvalidTransition` no
    /// caminho dinâmico.
    pub fn transicionar(
        &mut self,
        ctx: &TenantContext,
        para: RunStatus,
    ) -> Result<DomainEvent, RunError> {
        todo!("A4, após aceite do G1")
    }
}

// ── traits de repositório (B1) — TODO método com `&TenantContext` ───────────

/// Repositório do agregado Run (+ entregas, que só existem sob um run).
///
/// Deliberadamente AUSENTES (critério 3): `update_status`,
/// `increment_gates`, qualquer setter de campo — mutação entra pelo
/// agregado e sai por `save`/`save_with_deliverables`. `task_id` fica
/// `&str` até o `TaskId` de A3 (valida `sq{hex}`, propriedade T3).
pub trait RunRepository {
    /// Carrega o agregado do tenant do contexto. `Ok(None)` = não existe
    /// NESTE tenant — um run de outro tenant é indistinguível de inexistente
    /// (isolamento fail-closed também na leitura).
    fn get(&self, ctx: &TenantContext, task_id: &str) -> Result<Option<Run>, RepositoryError>;

    /// Runs do tenant, mais recente primeiro (ordem atual de `list_runs`).
    fn list(&self, ctx: &TenantContext) -> Result<Vec<Run>, RepositoryError>;

    /// Persiste o estado do agregado (upsert por `task_id` dentro do tenant).
    fn save(&mut self, ctx: &TenantContext, run: &Run) -> Result<(), RepositoryError>;

    /// Persiste run + entregas novas NUMA transação (critério 4): a ausência
    /// de transação run+deliverable apontada no levantamento §4.1 morre AQUI,
    /// na forma da API — não há caminho para gravar entrega órfã de run.
    /// (A atomicidade com o LEDGER continua fora — restrição declarada no
    /// ADR 0026, item 5: outbox/idempotência é decisão da Trilha B.)
    fn save_with_deliverables(
        &mut self,
        ctx: &TenantContext,
        run: &Run,
        novas: &[Deliverable],
    ) -> Result<(), RepositoryError>;

    /// Entregas do tenant, mais recente primeiro (Biblioteca U4).
    fn list_deliverables(&self, ctx: &TenantContext) -> Result<Vec<Deliverable>, RepositoryError>;

    fn get_deliverable(
        &self,
        ctx: &TenantContext,
        id: i64,
    ) -> Result<Option<Deliverable>, RepositoryError>;

    /// Maior seq de `task_id` já persistido no tenant (semeia o contador do
    /// hub no arranque — comportamento atual de `max_run_task_seq`).
    fn max_task_seq(&self, ctx: &TenantContext) -> Result<u64, RepositoryError>;
}

/// Personas (U7) — área "só tipagem" (ADR 0024): a trait espelha as
/// operações atuais do store, com tenant, sem semântica nova.
pub trait PersonaRepository {
    fn list_overrides(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<crate::PersonaOverride>, RepositoryError>;

    fn set_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
        prompt: &str,
    ) -> Result<(), RepositoryError>;

    fn delete_override(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        papel: &str,
    ) -> Result<(), RepositoryError>;

    fn clear_overrides(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<(), RepositoryError>;

    fn list_custom(
        &self,
        ctx: &TenantContext,
        template_id: &str,
    ) -> Result<Vec<crate::CustomPersona>, RepositoryError>;

    fn insert_custom(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        nome: &str,
        prompt: &str,
    ) -> Result<i64, RepositoryError>;

    fn update_custom(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        nome: &str,
        prompt: &str,
    ) -> Result<(), RepositoryError>;

    fn delete_custom(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError>;
}

/// Trilha auditável POR TENANT (ADR 0027): append consome `DomainEvent`
/// (não string — A5), a cadeia encadeia dentro do tenant e o export é
/// verificável isoladamente.
///
/// `type Entry` é decisão EXPOSTA ao G1: o tipo de leitura da trilha hoje é
/// `btv_schemas::ledger::LedgerEntry`, que este crate não pode importar
/// (fronteira do lint T4-A não cobre btv-schemas, mas duplicar o tipo aqui
/// criaria dois donos do mesmo contrato). Associated type deixa o adapter
/// devolver o tipo canônico existente sem o domínio depender dele — se a
/// revisão preferir um `AuditEntry` próprio do domínio, é UMA mudança aqui.
pub trait LedgerRepository {
    type Entry;

    /// Registra o evento na cadeia DO TENANT do contexto; devolve o `seq`
    /// dentro dela. O `ts`/`actor` do evento são a verdade — o adapter não
    /// os reescreve.
    fn append(&mut self, ctx: &TenantContext, event: &DomainEvent) -> Result<u64, RepositoryError>;

    /// Entradas recentes da cadeia do tenant (contrato atual de `recent`,
    /// com o filtro de actor resolvido no adapter).
    fn recent(
        &self,
        ctx: &TenantContext,
        limit: u32,
        actor: Option<&str>,
    ) -> Result<Vec<Self::Entry>, RepositoryError>;

    /// Verifica UMA cadeia — a do tenant do contexto (ADR 0027 item 3).
    fn verify_chain(&self, ctx: &TenantContext) -> Result<u64, RepositoryError>;

    /// A cadeia completa do tenant, verificável isoladamente (export/
    /// auditoria portátil — ADR 0027 item 4).
    fn export(&self, ctx: &TenantContext) -> Result<Vec<Self::Entry>, RepositoryError>;
}

/// Event store de sessão com concorrência otimista (o `EventStore` atual de
/// `btv-store::events`, atrás de port). Associated types pela mesma razão
/// documentada em `LedgerRepository::Entry`.
pub trait EventStorePort {
    type NewEvent;
    type StoredEvent;

    /// Append otimista: `expected_head` errado ⇒
    /// `RepositoryError::ConcurrencyConflict` (a semântica atual de
    /// `EventError::Conflict`, sem o tipo do driver).
    fn append(
        &mut self,
        ctx: &TenantContext,
        aggregate_id: &str,
        expected_head: i64,
        events: Vec<Self::NewEvent>,
    ) -> Result<i64, RepositoryError>;

    fn read(
        &self,
        ctx: &TenantContext,
        aggregate_id: &str,
        from_seq: i64,
    ) -> Result<Vec<Self::StoredEvent>, RepositoryError>;

    fn head_seq(&self, ctx: &TenantContext, aggregate_id: &str) -> Result<i64, RepositoryError>;
}
