//! Ports e agregado do domínio — assinaturas aceitas no portão G1 e, desde
//! A3/A4, implementadas: `RunStatus` (máquina de transições), o agregado
//! `Run` (`approve_gate`/`transition_to`) e o `DomainEvent`. As TRAITS de
//! repositório seguem sem implementação até B2 (adapter SQLite sob a suíte
//! de contrato). Evoluções de assinatura vs. o G1 aceito estão declaradas
//! no rustdoc do método correspondente (forma exigida pela revisão).
//!
//! Os quatro critérios de julgamento (revisão do G0) e onde cada um mora:
//! 1. **Tenant fail-closed sem exceção** — todo método de toda trait recebe
//!    `&TenantContext` (tenant + actor: o actor alimenta o ledger e não se
//!    perde na assinatura). Não há método de conveniência sem contexto.
//! 2. **Assinaturas limpas de infraestrutura** — nas traits de REPOSITÓRIO,
//!    nenhum `rusqlite::Error`/`serde_json::Value`/tipo de wire; erros são
//!    enums de domínio
//!    (`RepositoryError`/`RunError`), tipos de entrada/saída são deste crate
//!    (o que ainda é do adapter fica em associated type, decisão visível).
//! 3. **O agregado é a única porta** — `Run::approve_gate` valida a
//!    transição, incrementa o contador e RETORNA o `DomainEvent`; as traits
//!    NÃO têm `update_status`/`increment_gates` — mutação por fora do
//!    agregado não existe na API.
//! 4. **Atomicidade no tipo** — `RunRepository::save_with_deliverables` é a
//!    unidade transacional run+entregas; não há como usar a API
//!    corretamente e quebrar essa consistência.

use crate::run::{Deliverable, Run, TaskId};
use crate::tenant::{ActorId, TenantContext, TenantId};

// ── status do run (A3 — implementado após o aceite do G1) ──────────────────

/// Os 4 estados reais do banco (`wire-strings.v1.json`, provado por T3).
/// `as_str`/`parse` reproduzem exatamente `ativa`/`concluida`/`encerrada`/
/// `erro` — byte-a-byte, é contrato de banco (round-trip provado nos testes
/// deste módulo contra a fixture canônica).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunStatus {
    Ativa,
    Concluida,
    Encerrada,
    Erro,
}

impl RunStatus {
    /// Todas as variantes — base dos testes de cobertura (variante nova sem
    /// entrada aqui não passa no round-trip exaustivo).
    pub const ALL: [RunStatus; 4] = [
        RunStatus::Ativa,
        RunStatus::Concluida,
        RunStatus::Encerrada,
        RunStatus::Erro,
    ];

    /// A string EXATA persistida hoje (contrato T3).
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Ativa => "ativa",
            RunStatus::Concluida => "concluida",
            RunStatus::Encerrada => "encerrada",
            RunStatus::Erro => "erro",
        }
    }

    /// Parse fail-closed: string fora do vocabulário é `RunError::InvalidStatus`
    /// — `status = "qualquer_string"` deixa de existir como estado possível.
    pub fn parse(s: &str) -> Result<Self, RunError> {
        match s {
            "ativa" => Ok(RunStatus::Ativa),
            "concluida" => Ok(RunStatus::Concluida),
            "encerrada" => Ok(RunStatus::Encerrada),
            "erro" => Ok(RunStatus::Erro),
            outro => Err(RunError::InvalidStatus(outro.to_string())),
        }
    }

    /// Máquina de transições (A3): `Ativa` → qualquer terminal; terminal →
    /// nada. `Concluida → Ativa` retorna `false` — e `Run::transition_to` a
    /// transforma em `RunError::InvalidTransition` em vez de UPDATE
    /// silencioso. A reconciliação de zumbis (`Ativa → Encerrada` no
    /// arranque) está coberta; auto-transição não é transição.
    pub fn can_transition_to(self, target: RunStatus) -> bool {
        matches!(
            (self, target),
            (
                RunStatus::Ativa,
                RunStatus::Concluida | RunStatus::Encerrada | RunStatus::Erro
            )
        )
    }
}

/// O wire (JSON das rotas e coluna do banco) usa exatamente `as_str` — a
/// troca `String`→`RunStatus` no `Run` não move um byte (goldens T1 e T3
/// como juízes, sem regravação).
impl serde::Serialize for RunStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for RunStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        RunStatus::parse(&s).map_err(serde::de::Error::custom)
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

/// Variantes E CAMPOS em inglês (ADR 0024, decisão 2; confirmado na revisão
/// do G1) mapeando os nomes do plano A5: `SquadAtivada`→`SquadActivated`,
/// `GateAprovado`→`GateApproved`, `AjusteSolicitado`→`AdjustRequested`,
/// `EntregaProduzida`→`DeliverableProduced`, `PersonaAtualizada`→
/// `PersonaUpdated`, `TemplatePublicado`→`TemplatePublished`.
///
/// O rename dos campos é grátis no código: `DomainEvent` NÃO deriva
/// `Serialize` — as chaves pt do payload atual do ledger (`etapa`,
/// `instrucao`, `papel`, …) viram responsabilidade EXCLUSIVA do DTO de
/// serialização do adapter (Trilha B), com os goldens T1 de guarda de que
/// nada se move no banco. Um lugar só para o mapeamento, em vez de o domínio
/// carregar nomes de wire para sempre.
///
/// Cobertura do vocabulário: `wire_kind()` liga cada variante ao kind
/// `btv.*` do inventário (`wire-strings.v1.json`) e o teste deste módulo
/// prova que os DOIS conjuntos são idênticos — variante órfã ou kind sem
/// variante quebram o build (pedido da revisão do G1).
/// Hash de procedência do prompt efetivo de UM membro da ativação —
/// shape FECHADO (C3.0): `custom` distingue persona própria de papel de
/// template, e o DTO do adapter omite a chave quando `false` (o wire
/// sempre foi assim: a chave só aparece nas personas próprias).
/// Fatos da ativação que o `Run` NÃO persiste mas o evento carrega
/// (procedência e briefing) — insumos de [`Run::activation_event`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivationFacts {
    pub custom_personas: Vec<String>,
    pub prompt_hashes: Vec<PromptHash>,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHash {
    pub role: String,
    pub prompt_sha256: String,
    pub custom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventKind {
    /// `btv.squad_activated`
    SquadActivated {
        task_id: TaskId,
        run_id: i64,
        template_id: String,
        // C3.0 (decisão (a) do dono): a variante alcança o payload REAL do
        // emissor — o wire congelado pelos goldens é a verdade.
        template_version: String,
        name: String,
        /// Papéis ativos (nomes) — já sem os desligados.
        roles: Vec<String>,
        /// Nomes das personas próprias incluídas na ativação (U7).
        custom_personas: Vec<String>,
        /// Procedência: hash do prompt EFETIVO de cada membro (shape
        /// fechado — a cerca do `Value` não se aplica: o dono do schema é
        /// este tipo).
        prompt_hashes: Vec<PromptHash>,
        /// Referências fornecidas no briefing (URLs/caminhos livres do
        /// usuário — conteúdo opaco, mas a FORMA é lista de strings).
        refs: Vec<String>,
    },
    /// `btv.gate_approved` — carrega o contador APÓS o incremento: o evento
    /// é o fato consumado que o ledger registra.
    GateApproved {
        task_id: TaskId,
        stage: Option<String>,
        gates_approved: i64,
    },
    /// `btv.adjust_requested`
    AdjustRequested {
        task_id: TaskId,
        stage: Option<String>,
        instruction: String,
        gate_released: bool,
    },
    /// `btv.export_generated`
    DeliverableProduced {
        task_id: TaskId,
        deliverable_id: i64,
        name: String,
        format: String,
        /// Trilha de procedência exibida na Biblioteca (U4) — ex.:
        /// "Redator · 1 gate(s)" (C3.0: o emissor sempre a gravou).
        trail: String,
    },
    /// `btv.persona_updated` — hash, nunca o prompt em claro (procedência).
    PersonaUpdated {
        template_id: String,
        role: String,
        prompt_sha256: String,
    },
    /// `btv.template_published`
    TemplatePublished {
        template_id: String,
        published: bool,
    },
    /// `btv.flow_saved` — fluxo do Squad Designer salvo como modelo
    /// ("salvo e auditado" — a aplicação ao orquestrador real segue sendo
    /// trabalho futuro, mesma honestidade do handler atual). Estava AUSENTE
    /// no rascunho do G1 (7 variantes para 8 kinds `btv.*`) — lacuna 2 da
    /// revisão; o teste de cobertura abaixo impede a reincidência.
    FlowSaved {
        name: String,
        blocks: u64,
        diagram_sha256: String,
        semantic_version: Option<String>,
        snapshot_hash: Option<String>,
        audit_head: Option<String>,
        audit_len: Option<u64>,
    },
    /// `btv.user_removed`
    UserRemoved { user_id: i64 },
}

impl DomainEventKind {
    /// O kind EXATO do wire para cada variante — mapeamento declarativo
    /// (dado, não comportamento), a única exceção consciente ao "sem
    /// implementação" deste arquivo: é o que o teste de cobertura
    /// variantes↔fixture exige para não ser fake. Apontado para re-auditoria
    /// do G1.
    pub fn wire_kind(&self) -> &'static str {
        match self {
            Self::SquadActivated { .. } => "btv.squad_activated",
            Self::GateApproved { .. } => "btv.gate_approved",
            Self::AdjustRequested { .. } => "btv.adjust_requested",
            Self::DeliverableProduced { .. } => "btv.export_generated",
            Self::PersonaUpdated { .. } => "btv.persona_updated",
            Self::TemplatePublished { .. } => "btv.template_published",
            Self::FlowSaved { .. } => "btv.flow_saved",
            Self::UserRemoved { .. } => "btv.user_removed",
        }
    }
}

// ── erros de domínio (critério 2: semânticos, zero infraestrutura) ──────────

/// Regras do agregado violadas — erro de NEGÓCIO, não de storage.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RunError {
    #[error("transição de status inválida: {from:?} → {to:?}")]
    InvalidTransition { from: RunStatus, to: RunStatus },
    #[error("status fora do vocabulário do banco: {0}")]
    InvalidStatus(String),
    /// C3.1 endpoint 2: o evento de ativação DERIVA do agregado persistido —
    /// pedi-lo antes do save (id = 0) ou sobre estado corrompido
    /// (`papeis_json` não-parseável) é erro, nunca evento fabricado.
    #[error("estado do run inválido para derivar evento: {0}")]
    InvalidState(String),
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

impl Run {
    /// Aprovar o gate HITL pendente — ÚNICA porta para incrementar
    /// `gates_aprovados` (critério 3). Valida que o run está `Ativa` (a
    /// máquina de `RunStatus` decide), incrementa o contador E retorna o
    /// `DomainEvent::GateApproved` com tenant/actor do contexto — o chamador
    /// (serviço de aplicação, C3) persiste via `RunRepository::save` e
    /// registra via `LedgerRepository::append`, mas NÃO decide: o contador
    /// deixa de ser um i64 solto mutado por UPDATE.
    ///
    /// Evolução de assinatura vs. G1 (pré-aprovada na revisão, na forma
    /// exigida — o porquê de cada insumo): `stage` é o nome do gate exibido
    /// (vai no evento, como o payload atual do ledger carrega); `ts` é o
    /// relógio do CHAMADOR (o domínio não lê relógio — determinismo). O
    /// agregado segue decidindo; o chamador só fornece insumos.
    /// Factory da ativação (C3.1 endpoint 2, decisão exposta no rito do
    /// G1): VALIDA E CONSTRÓI o run nascente — `task_id` e `ts` são INSUMOS
    /// de infraestrutura (quem sequencia é o hub via `max_task_seq`; quem
    /// carimba hora é o chamador — o domínio não lê contador nem relógio,
    /// mesma decisão do `ts` no A4). `papeis_json` é DERIVADO de `roles`
    /// aqui dentro (fonte única: o estado e o evento futuro não podem
    /// divergir porque nascem do mesmo vetor). `roles` vazio é recusado —
    /// a mesma regra que o handler sempre aplicou, agora no agregado.
    ///
    /// EVOLUÇÃO vs. o prior da revisão (`activate -> (Run, DomainEvent)`),
    /// declarada: o `run_id` do evento é numerado pelo STORAGE no save —
    /// retornar o par aqui exigiria placeholder ou porta nova. Em vez
    /// disso, o evento DERIVA do agregado persistido
    /// ([`Run::activation_event`], fail-closed em `id == 0`): coerência
    /// run↔evento por construção — mais forte que o par, porque o evento é
    /// FUNÇÃO do estado salvo, não um irmão que se espera igual.
    #[allow(clippy::too_many_arguments)]
    pub fn activate(
        ctx: &TenantContext,
        task_id: TaskId,
        template_id: String,
        template_versao: String,
        nome: String,
        briefing_json: String,
        roles: &[String],
        ts: String,
    ) -> Result<Run, RunError> {
        if roles.is_empty() {
            return Err(RunError::InvalidState(
                "ativação exige ao menos um papel ativo".into(),
            ));
        }
        let papeis_json = serde_json::to_string(roles)
            .map_err(|e| RunError::InvalidState(format!("papéis não serializáveis: {e}")))?;
        Ok(Run {
            id: 0, // numerado pelo storage no primeiro save (contrato do B2)
            task_id,
            template_id,
            template_versao,
            nome,
            briefing_json,
            papeis_json,
            status: RunStatus::Ativa,
            gates_aprovados: 0,
            created_ts: ts.clone(),
            updated_ts: ts,
            tenant: ctx.tenant,
        })
    }

    /// O evento `btv.squad_activated` DERIVADO do agregado PERSISTIDO —
    /// `id == 0` é recusado fail-closed (o wire carrega o `run_id` que o
    /// storage numerou; sem save não há fato a auditar). `roles` volta de
    /// `papeis_json` (a fonte única que a factory gravou); os fatos da
    /// ativação que o run NÃO persiste (procedência de prompts, personas
    /// próprias, refs do briefing) entram como insumos.
    ///
    /// Este é o evento de NASCIMENTO, emitido EXATAMENTE UMA VEZ pelo
    /// serviço de ativação, logo após o primeiro save — o método é
    /// publicamente chamável em qualquer run persistido, e a UNICIDADE é
    /// contrato do CHAMADOR: chamá-lo de novo (num replay, numa releitura)
    /// fabricaria uma segunda ativação que nunca aconteceu.
    pub fn activation_event(
        &self,
        ctx: &TenantContext,
        facts: ActivationFacts,
        ts: String,
    ) -> Result<DomainEvent, RunError> {
        if self.id == 0 {
            return Err(RunError::InvalidState(
                "evento de ativação exige run persistido (id numerado pelo storage)".into(),
            ));
        }
        let roles: Vec<String> = serde_json::from_str(&self.papeis_json)
            .map_err(|e| RunError::InvalidState(format!("papeis_json corrompido: {e}")))?;
        Ok(DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts,
            kind: DomainEventKind::SquadActivated {
                task_id: self.task_id,
                run_id: self.id,
                template_id: self.template_id.clone(),
                template_version: self.template_versao.clone(),
                name: self.nome.clone(),
                roles,
                custom_personas: facts.custom_personas,
                prompt_hashes: facts.prompt_hashes,
                refs: facts.refs,
            },
        })
    }

    pub fn approve_gate(
        &mut self,
        ctx: &TenantContext,
        stage: Option<String>,
        ts: String,
    ) -> Result<DomainEvent, RunError> {
        if self.status != RunStatus::Ativa {
            // Aprovar gate é operação de run Ativa: num terminal, o erro
            // nomeia a transição implícita que seria necessária (voltar a
            // Ativa) — que a máquina proíbe. Estado NÃO muda.
            return Err(RunError::InvalidTransition {
                from: self.status,
                to: RunStatus::Ativa,
            });
        }
        self.gates_aprovados += 1;
        self.updated_ts = ts.clone();
        Ok(DomainEvent {
            tenant: ctx.tenant,
            actor: ctx.actor.clone(),
            ts,
            kind: DomainEventKind::GateApproved {
                task_id: self.task_id,
                stage,
                gates_approved: self.gates_aprovados,
            },
        })
    }

    /// Transição de status pela máquina de `RunStatus` — substitui o
    /// `set_status(task_id, "qualquer_string")` atual. `Concluida → Ativa`
    /// não compila no chamador tipado e retorna `InvalidTransition` no
    /// caminho dinâmico; em erro, o estado NÃO muda.
    ///
    /// Evolução de assinatura vs. G1 (declarada): retorna `Result<(), _>`,
    /// não `DomainEvent` — transição de status NUNCA foi fato auditado no
    /// wire (nenhum kind de ledger existe para ela; o teste de cobertura
    /// variantes↔fixture proíbe variante sem kind real). Retornar um evento
    /// aqui fabricaria auditoria que o produto não tem — Nada Fake vence a
    /// simetria da assinatura. `ts` idem `approve_gate`.
    pub fn transition_to(
        &mut self,
        _ctx: &TenantContext,
        target: RunStatus,
        ts: String,
    ) -> Result<(), RunError> {
        if !self.status.can_transition_to(target) {
            return Err(RunError::InvalidTransition {
                from: self.status,
                to: target,
            });
        }
        self.status = target;
        self.updated_ts = ts;
        Ok(())
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

/// Publicação de templates (A5) — área "só tipagem" (ADR 0024). Nome que
/// afirma só o que faz: o CATÁLOGO em si é estático (`include_str!` dos modelos
/// embutidos), esta porta governa APENAS o override de `publicado` por
/// template, com tenant — daí `TemplatePublication`, não `TemplateCatalog` (um
/// port que possuísse o catálogo prometeria posse do que é de outro dono).
/// O emissor `btv.template_published` já é fato de domínio no `LedgerRepository`
/// desde o G1; esta porta é o acesso de ESTADO que faltava (C3.3).
pub trait TemplatePublicationRepository {
    /// Publica/despublica um template no tenant do contexto (upsert).
    fn set_published(
        &mut self,
        ctx: &TenantContext,
        template_id: &str,
        published: bool,
    ) -> Result<(), RepositoryError>;

    /// Overrides de publicação persistidos no tenant. Leitura fail-closed: a
    /// publicação de outro tenant é indistinguível de inexistente.
    fn list_published(&self, ctx: &TenantContext) -> Result<Vec<(String, bool)>, RepositoryError>;
}

/// Perfis locais (A6) — a porta do Contexto de Identidade local. Formaliza o
/// que o store já praticava: `verify_pin` compara DENTRO do adapter (o
/// `pin_hash` nunca sai; o veredito é o `PinCheck` do domínio), CRUD ao lado.
/// Tudo com tenant, leitura fail-closed (perfil de outro tenant = inexistente).
pub trait UserRepository {
    fn list(&self, ctx: &TenantContext) -> Result<Vec<crate::User>, RepositoryError>;

    /// Cria um perfil (PIN opcional). O `created_ts` é escrituração do adapter.
    fn create(
        &mut self,
        ctx: &TenantContext,
        nome: &str,
        email: &str,
        papel: &str,
        pin: Option<&str>,
    ) -> Result<i64, RepositoryError>;

    fn remove(&mut self, ctx: &TenantContext, id: i64) -> Result<(), RepositoryError>;

    fn set_active(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        ativo: bool,
    ) -> Result<(), RepositoryError>;

    fn set_pin(
        &mut self,
        ctx: &TenantContext,
        id: i64,
        pin: Option<&str>,
    ) -> Result<(), RepositoryError>;

    /// Verifica o PIN CONTRA o hash guardado — a comparação vive no adapter, o
    /// hash nunca atravessa a fronteira. Perfil sem PIN → `NoPin`.
    fn verify_pin(
        &self,
        ctx: &TenantContext,
        id: i64,
        pin: &str,
    ) -> Result<crate::PinCheck, RepositoryError>;
}

/// Trilha auditável POR TENANT (ADR 0027): append consome `DomainEvent`
/// (não string — A5), a cadeia encadeia dentro do tenant e o export é
/// verificável isoladamente.
///
/// **Escopo DECLARADO deste port (lacuna 1 da revisão do G1):** ele registra
/// FATOS DE DOMÍNIO — os kinds `btv.*` que `DomainEventKind` enumera. As
/// entradas OPERACIONAIS da mesma cadeia (13 dos 21 kinds do inventário:
/// `session.*`, `tool.*`, `llm.turn`, `user.turn`, `squad.*`,
/// `permission_rule.*`, `designer.workflow_saved`, `skill.vetting`) são
/// instrumentação, não fato de negócio, e continuam entrando pela API
/// existente (`LedgerStore::append`/`Session::note`) — que a **B3 também
/// tenantiza**: as DUAS portas alimentam a MESMA cadeia por tenant, com o
/// mesmo hash-chain. Este port portanto NÃO substitui o `LedgerStore` — ele
/// é a porta tipada dos fatos de domínio sobre a mesma trilha. Unificar as
/// duas portas (ou mantê-las como categorias distintas em definitivo) é
/// decisão diferida, registrada em `pendencias.md` (§ migração DDD, G1).
///
/// `type Entry` — decisão exposta, ACEITA na revisão do G1 com gatilho
/// registrado: no dia em que o domínio precisar INTERPRETAR entradas
/// (export verificável da Trilha E, billing lendo a trilha), nasce o
/// `AuditEntry` próprio do domínio e o associated type morre. Até lá, evita
/// dois donos para o contrato que hoje é de `btv_schemas::ledger::LedgerEntry`.
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
        actor: Option<&ActorId>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Uma instância de CADA variante (valores dummy — o teste é sobre o
    /// VOCABULÁRIO, não sobre payload). Variante nova sem entrada aqui não
    /// compila o `match` de `wire_kind`; kind novo na fixture sem variante
    /// quebra a igualdade de conjuntos abaixo.
    fn uma_de_cada_variante() -> Vec<DomainEventKind> {
        vec![
            DomainEventKind::SquadActivated {
                task_id: TaskId::new(1),
                run_id: 1,
                template_id: "editorial".into(),
                template_version: "v1.4".into(),
                name: "Ativação".into(),
                roles: vec!["Redator".into()],
                custom_personas: vec![],
                prompt_hashes: vec![PromptHash {
                    role: "Redator".into(),
                    prompt_sha256: "abc".into(),
                    custom: false,
                }],
                refs: vec![],
            },
            DomainEventKind::GateApproved {
                task_id: TaskId::new(1),
                stage: None,
                gates_approved: 1,
            },
            DomainEventKind::AdjustRequested {
                task_id: TaskId::new(1),
                stage: None,
                instruction: "tom formal".into(),
                gate_released: true,
            },
            DomainEventKind::DeliverableProduced {
                task_id: TaskId::new(1),
                deliverable_id: 1,
                name: "artigo.md".into(),
                format: "MD".into(),
                trail: "Redator · 1 gate(s)".into(),
            },
            DomainEventKind::PersonaUpdated {
                template_id: "editorial".into(),
                role: "Redator".into(),
                prompt_sha256: "abc".into(),
            },
            DomainEventKind::TemplatePublished {
                template_id: "editorial".into(),
                published: true,
            },
            DomainEventKind::FlowSaved {
                name: "fluxo".into(),
                blocks: 3,
                diagram_sha256: "abc".into(),
                semantic_version: None,
                snapshot_hash: None,
                audit_head: None,
                audit_len: None,
            },
            DomainEventKind::UserRemoved { user_id: 1 },
        ]
    }

    /// A3: o vocabulário de `RunStatus` é EXATAMENTE o `run_status` da
    /// fixture canônica (T3), com round-trip exaustivo — `as_str`/`parse`
    /// reproduzem as strings do banco byte-a-byte.
    #[test]
    fn run_status_roundtrip_exaustivo_contra_a_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/fixtures/wire-strings.v1.json"),
            )
            .expect("fixture wire-strings.v1"),
        )
        .expect("fixture é JSON");
        let da_fixture: BTreeSet<String> = fixture["run_status"]
            .as_array()
            .expect("lista run_status")
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_string)
            .collect();
        let do_enum: BTreeSet<String> = RunStatus::ALL.iter().map(|s| s.as_str().into()).collect();
        assert_eq!(do_enum, da_fixture);
        for status in RunStatus::ALL {
            assert_eq!(RunStatus::parse(status.as_str()), Ok(status));
        }
        assert_eq!(
            RunStatus::parse("qualquer_string"),
            Err(RunError::InvalidStatus("qualquer_string".into()))
        );
    }

    /// A3: a máquina de transições — `Ativa` → terminais; terminal → nada;
    /// `Concluida → Ativa` (o caso nomeado no plano) é `false`.
    #[test]
    fn maquina_de_transicoes_fail_closed() {
        use RunStatus::*;
        for terminal in [Concluida, Encerrada, Erro] {
            assert!(Ativa.can_transition_to(terminal));
            assert!(!terminal.can_transition_to(Ativa), "{terminal:?}→Ativa");
            assert!(!terminal.can_transition_to(terminal));
        }
        assert!(
            !Ativa.can_transition_to(Ativa),
            "auto-transição não é transição"
        );
        assert!(!Concluida.can_transition_to(Erro));
    }

    fn run_ativo() -> Run {
        Run {
            id: 1,
            task_id: TaskId::new(1),
            template_id: "editorial".into(),
            template_versao: "v1.4".into(),
            nome: "Newsletter".into(),
            briefing_json: "[]".into(),
            papeis_json: "[]".into(),
            status: RunStatus::Ativa,
            gates_aprovados: 0,
            created_ts: "2026-07-08T10:00:00Z".into(),
            updated_ts: "2026-07-08T10:00:00Z".into(),
            tenant: TenantId::LOCAL,
        }
    }

    fn ctx() -> TenantContext {
        TenantContext::local(ActorId::new("web:btv").unwrap())
    }

    /// A4 (DoD: invariante testada SEM banco): aprovar o gate incrementa,
    /// atualiza o relógio e retorna o evento COMPLETO — tenant/actor do
    /// contexto, contador pós-incremento, kind certo.
    #[test]
    fn approve_gate_incrementa_e_retorna_o_evento() {
        let mut run = run_ativo();
        let evento = run
            .approve_gate(
                &ctx(),
                Some("Aprovar o rascunho".into()),
                "2026-07-08T10:05:00Z".into(),
            )
            .unwrap();
        assert_eq!(run.gates_aprovados, 1);
        assert_eq!(run.updated_ts, "2026-07-08T10:05:00Z");
        assert_eq!(evento.tenant, TenantId::LOCAL);
        assert_eq!(evento.actor.as_str(), "web:btv");
        assert_eq!(evento.ts, "2026-07-08T10:05:00Z");
        match evento.kind {
            DomainEventKind::GateApproved {
                task_id,
                stage,
                gates_approved,
            } => {
                assert_eq!(task_id, TaskId::new(1));
                assert_eq!(stage.as_deref(), Some("Aprovar o rascunho"));
                assert_eq!(
                    gates_approved, 1,
                    "o evento carrega o contador PÓS-incremento"
                );
            }
            outro => panic!("kind errado: {outro:?}"),
        }
        // segundo gate: contador segue pelo agregado, não por UPDATE solto
        let evento2 = run
            .approve_gate(&ctx(), None, "2026-07-08T10:06:00Z".into())
            .unwrap();
        assert!(matches!(
            evento2.kind,
            DomainEventKind::GateApproved {
                gates_approved: 2,
                ..
            }
        ));
    }

    /// A4, teste NEGATIVO de negócio (pedido explícito da revisão): aprovar
    /// gate em run terminal retorna `InvalidTransition` e o ESTADO NÃO MUDA
    /// — asserção no estado, não só no erro. É o invariante que justifica o
    /// agregado existir.
    #[test]
    fn approve_gate_em_run_terminal_falha_sem_mutar_o_estado() {
        for terminal in [RunStatus::Concluida, RunStatus::Encerrada, RunStatus::Erro] {
            let mut run = run_ativo();
            run.status = terminal;
            run.gates_aprovados = 3;
            let antes = run.clone();
            let err = run
                .approve_gate(&ctx(), None, "2026-07-08T11:00:00Z".into())
                .unwrap_err();
            assert_eq!(
                err,
                RunError::InvalidTransition {
                    from: terminal,
                    to: RunStatus::Ativa
                }
            );
            assert_eq!(run, antes, "estado intacto após a recusa ({terminal:?})");
        }
    }

    /// A4: transição válida muda status+relógio e NÃO fabrica evento
    /// (transição nunca foi fato auditado no wire — desvio declarado);
    /// inválida falha com o estado intacto.
    #[test]
    fn transition_to_respeita_a_maquina_sem_fabricar_evento() {
        let mut run = run_ativo();
        run.transition_to(&ctx(), RunStatus::Concluida, "2026-07-08T12:00:00Z".into())
            .unwrap();
        assert_eq!(run.status, RunStatus::Concluida);
        assert_eq!(run.updated_ts, "2026-07-08T12:00:00Z");

        let antes = run.clone();
        let err = run
            .transition_to(&ctx(), RunStatus::Ativa, "2026-07-08T13:00:00Z".into())
            .unwrap_err();
        assert_eq!(
            err,
            RunError::InvalidTransition {
                from: RunStatus::Concluida,
                to: RunStatus::Ativa
            }
        );
        assert_eq!(run, antes, "estado intacto após transição inválida");
    }

    /// Pedido da revisão do G1 (lacuna 2): o vocabulário `btv.*` da fixture
    /// canônica (T3) e as variantes de `DomainEventKind` são o MESMO
    /// conjunto — a ausência de `btv.flow_saved` no rascunho teria quebrado
    /// aqui. Kind órfão por omissão deixa de ser possível.
    #[test]
    fn variantes_cobrem_exatamente_os_kinds_btv_da_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/fixtures/wire-strings.v1.json"),
            )
            .expect("fixture wire-strings.v1"),
        )
        .expect("fixture é JSON");
        let da_fixture: BTreeSet<String> = fixture["ledger_kinds"]
            .as_array()
            .expect("lista ledger_kinds")
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|k| k.starts_with("btv."))
            .map(str::to_string)
            .collect();

        let das_variantes: BTreeSet<String> = uma_de_cada_variante()
            .iter()
            .map(|k| k.wire_kind().to_string())
            .collect();

        assert_eq!(
            das_variantes, da_fixture,
            "vocabulário de DomainEventKind divergiu do inventário btv.* da fixture"
        );
    }
}

// ── D1t: portas do runtime de agente (LlmPort / ToolsPort) ─────────────────
//
// A violação 4 do levantamento fecha aqui: o loop de agente (`btv-core`)
// passa a depender SÓ destas traits — `Gateway` (btv-llm) e `ToolRegistry`
// (btv-tools) viram implementações que o binário injeta. As assinaturas
// preservam o shape que o loop sempre consumiu (o `Generator` histórico e a
// dupla iter/get do registry) — nenhuma idiossincrasia de HTTP entra aqui:
// streaming é um callback de texto (`on_delta`), não um tipo de provider.

/// Erro da porta de LLM — as MESMAS três variantes do `GatewayError`
/// histórico (nenhuma carrega tipo de driver; `btv-llm` re-exporta este
/// tipo sob o nome antigo).
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY")]
    NoProvider,
    #[error("todos os providers falharam: {0}")]
    AllFailed(String),
    #[error("limite de requisições excedido: {0}")]
    RateLimited(String),
}

/// Porta de geração de texto/tool-use. Streaming entra como callback de
/// deltas — o transporte (SSE, terminal, buffer de teste) é problema do
/// chamador; o provider é problema do adapter.
pub trait LlmPort {
    fn generate(
        &self,
        req: crate::chat::GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> impl std::future::Future<Output = Result<crate::chat::AssistantTurn, LlmError>> + Send;
}

/// Porta do conjunto de ferramentas disponível ao loop: o anúncio ao
/// modelo (`specs`) e a resolução por nome (`get`) — exatamente a
/// superfície que o loop sempre consumiu do `ToolRegistry`. `Send + Sync`
/// como supertrait porque o loop cruza `.await` dentro de `tokio::spawn`
/// segurando a referência (o registry sempre foi ambos; mocks idem).
pub trait ToolsPort: Send + Sync {
    fn specs(&self) -> Vec<crate::chat::ToolSpec>;
    fn get(&self, name: &str) -> Option<&dyn crate::tool::Tool>;
}
