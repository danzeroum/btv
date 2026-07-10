//! Run (squad ativada) e Deliverable (entrega da Biblioteca) — os tipos do
//! contexto Core "produto BTV" (ADR 0024), movidos de `btv-store::btv` na
//! tarefa A2 com `tenant` desde já (D1/ADR 0025).
//!
//! Nome `Deliverable` (não `Entrega`, como a tabela do plano sugeria):
//! identificador NOVO segue a decisão 2 do ADR 0024 — inglês no código, wire
//! em português inalterado (os goldens T1 congelam `nome`/`formato`/
//! `trilha`/… byte-a-byte).
//!
//! `tenant` NÃO entra no wire nesta fase (`skip_serializing` — provado pelos
//! goldens): quem preenche é o adapter de persistência a partir do
//! `TenantContext` (hoje `TenantId::LOCAL` fixo; a coluna real chega em B2).
//! Expor tenant por rota é decisão da Trilha E, não um vazamento acidental.
//!
//! A4: `status: RunStatus` e `task_id: TaskId` — o compilador impede
//! `status = "qualquer_string"` e id fora de `sq{hex}`; o wire não move um
//! byte (serde dos dois tipos usa exatamente a representação antiga —
//! goldens T1 e T3 como juízes, sem regravação).

use serde::Serialize;

use crate::ports::RunStatus;
use crate::tenant::TenantId;

/// Uma squad ativada (execução) — linha de "Minhas squads" (U6) e âncora da
/// tela Ao vivo (U3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    pub id: i64,
    pub task_id: TaskId,
    pub template_id: String,
    pub template_versao: String,
    pub nome: String,
    /// Respostas do briefing (JSON: `[{label, resposta}]`).
    pub briefing_json: String,
    /// Papéis ativos (JSON: `["Pauteiro", ...]` — já sem os desligados).
    pub papeis_json: String,
    /// Máquina de transições em `RunStatus` — mutação só pelo agregado
    /// (`approve_gate`/`transition_to`, ports.rs).
    pub status: RunStatus,
    /// Quantos gates humanos já foram aprovados neste run (trilha de U4).
    pub gates_aprovados: i64,
    pub created_ts: String,
    pub updated_ts: String,
    /// Dono do run. Fora do wire nesta fase (ver doc do módulo).
    #[serde(skip_serializing)]
    pub tenant: TenantId,
}

/// Artefato exportado — linha da Biblioteca de entregas (U4), com trilha de
/// procedência real (papéis do run + gates aprovados) e o caminho do arquivo
/// REAL gravado pelas ferramentas do squad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deliverable {
    pub id: i64,
    pub run_id: i64,
    pub task_id: TaskId,
    pub template_id: String,
    pub nome: String,
    pub path: String,
    pub formato: String,
    pub versao: String,
    pub trilha: String,
    pub created_ts: String,
    /// Dono da entrega. Fora do wire nesta fase (ver doc do módulo).
    #[serde(skip_serializing)]
    pub tenant: TenantId,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trava de wire: o campo `tenant` existe no domínio mas NÃO aparece na
    /// serialização — o contrato HTTP congelado pelos goldens T1 permanece
    /// byte-idêntico após a A2.
    #[test]
    fn tenant_fica_fora_do_wire() {
        let run = Run {
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
        };
        let json = serde_json::to_value(&run).unwrap();
        assert!(json.get("tenant").is_none(), "tenant não vaza no wire");
        assert_eq!(json["status"], "ativa", "enum serializa a string antiga");
        assert_eq!(json["task_id"], "sq1", "TaskId serializa sq{{hex}}");
        assert_eq!(json.as_object().unwrap().len(), 11, "11 campos de wire");

        let entrega = Deliverable {
            id: 1,
            run_id: 1,
            task_id: TaskId::new(1),
            template_id: "editorial".into(),
            nome: "artigo.md".into(),
            path: "/tmp/artigo.md".into(),
            formato: "MD".into(),
            versao: "v1".into(),
            trilha: "Pauteiro → Redator".into(),
            created_ts: "2026-07-08T10:00:00Z".into(),
            tenant: TenantId::LOCAL,
        };
        let json = serde_json::to_value(&entrega).unwrap();
        assert!(json.get("tenant").is_none());
        assert_eq!(json.as_object().unwrap().len(), 10, "10 campos de wire");
    }
}

// ── TaskId (A3) ─────────────────────────────────────────────────────────────

/// Id de tarefa de squad — newtype sobre o seq, com o formato `sq{hex}`
/// como ÚNICA representação textual (a que `SquadHub::new_task` gera e
/// `BtvStore::max_run_task_seq` parseia — o par gerador↔parser é propriedade
/// provada por T3 em `wire_strings.rs`, e o round-trip deste tipo é provado
/// aqui por proptest). `parse` espelha a leniência do parser de produção
/// (`from_str_radix(16)` aceita hex maiúsculo) — endurecer seria mudança de
/// comportamento, não tipagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(u64);

/// String que não é `sq{hex}`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("task_id fora do formato sq{{hex}}: {0}")]
pub struct InvalidTaskId(pub String);

impl TaskId {
    pub fn new(seq: u64) -> Self {
        Self(seq)
    }

    pub fn seq(self) -> u64 {
        self.0
    }

    /// Valida `sq{hex}` — fail-closed: sem prefixo, hex inválido ou overflow
    /// de u64 não viram id.
    pub fn parse(s: &str) -> Result<Self, InvalidTaskId> {
        s.strip_prefix("sq")
            .and_then(|h| u64::from_str_radix(h, 16).ok())
            .map(TaskId)
            .ok_or_else(|| InvalidTaskId(s.to_string()))
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sq{:x}", self.0)
    }
}

impl serde::Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for TaskId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        TaskId::parse(&s).map_err(serde::de::Error::custom)
    }
}

// ── Briefing tipado (A3) ────────────────────────────────────────────────────

/// Uma resposta do briefing do wizard — o item de `briefing_json`
/// (`[{label, resposta}]`, mesmo shape do corpo de `POST /api/btv/squads`).
/// Campos com os NOMES DO WIRE (`label`/`resposta` — ADR 0024: campo de
/// contrato serializado que já é pt permanece); round-trip byte-a-byte
/// provado no teste contra o JSON real das fixtures. O PARSE do
/// `briefing_json` persistido é do adapter (serde_json fica fora do lib do
/// domínio — regra de dependências de A1): o domínio dá o tipo, o adapter
/// faz `serde_json::from_str::<Vec<BriefingResposta>>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct BriefingResposta {
    pub label: String,
    pub resposta: String,
}

#[cfg(test)]
mod tests_a3 {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Round-trip do formato canônico para QUALQUER seq de u64 — o mesmo
        /// par que T3 prova via SQLite real; aqui a propriedade é do tipo.
        #[test]
        fn task_id_roundtrip_display_parse(seq in any::<u64>()) {
            let id = TaskId::new(seq);
            prop_assert_eq!(TaskId::parse(&id.to_string()), Ok(id));
            // serde usa a MESMA representação textual
            let json = serde_json::to_string(&id).unwrap();
            prop_assert_eq!(json, format!("\"sq{seq:x}\""));
        }
    }

    #[test]
    fn task_id_rejeita_fora_do_formato() {
        for ruim in ["", "sq", "1a", "sqzz", "sq-1", "SQ1f", "sq 1"] {
            assert!(TaskId::parse(ruim).is_err(), "aceitou `{ruim}`");
        }
        // Leniência do parser de produção preservada: hex maiúsculo passa.
        assert_eq!(TaskId::parse("sq1F"), Ok(TaskId::new(0x1f)));
    }

    /// O briefing REAL das fixtures (golden de ativação) parseia e
    /// re-serializa byte-a-byte — wire congelado.
    #[test]
    fn briefing_roundtrip_byte_a_byte_do_json_real() {
        let real =
            r#"[{"label":"Qual é a pauta ou tema?","resposta":"logística verde no Brasil"}]"#;
        let itens: Vec<BriefingResposta> = serde_json::from_str(real).unwrap();
        assert_eq!(itens.len(), 1);
        assert_eq!(itens[0].label, "Qual é a pauta ou tema?");
        assert_eq!(serde_json::to_string(&itens).unwrap(), real);

        let ruim: Result<Vec<BriefingResposta>, _> = serde_json::from_str(r#"[{"so_label":"x"}]"#);
        assert!(ruim.is_err());
        let nao_json: Result<Vec<BriefingResposta>, _> = serde_json::from_str("não é json");
        assert!(nao_json.is_err());
    }
}
