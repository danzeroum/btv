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
//! `status`/`task_id` seguem `String` AQUI de propósito: virar `RunStatus`/
//! `TaskId` tipados é a tarefa A3, com o round-trip provado pelos property
//! tests T3 (`wire-strings.v1.json`) — um passo por PR.

use serde::Serialize;

use crate::tenant::TenantId;

/// Uma squad ativada (execução) — linha de "Minhas squads" (U6) e âncora da
/// tela Ao vivo (U3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    pub id: i64,
    pub task_id: String,
    pub template_id: String,
    pub template_versao: String,
    pub nome: String,
    /// Respostas do briefing (JSON: `[{label, resposta}]`).
    pub briefing_json: String,
    /// Papéis ativos (JSON: `["Pauteiro", ...]` — já sem os desligados).
    pub papeis_json: String,
    /// `ativa` | `concluida` | `encerrada` | `erro` (enum tipado em A3).
    pub status: String,
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
    pub task_id: String,
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
            task_id: "sq1".into(),
            template_id: "editorial".into(),
            template_versao: "v1.4".into(),
            nome: "Newsletter".into(),
            briefing_json: "[]".into(),
            papeis_json: "[]".into(),
            status: "ativa".into(),
            gates_aprovados: 0,
            created_ts: "2026-07-08T10:00:00Z".into(),
            updated_ts: "2026-07-08T10:00:00Z".into(),
            tenant: TenantId::LOCAL,
        };
        let json = serde_json::to_value(&run).unwrap();
        assert!(json.get("tenant").is_none(), "tenant não vaza no wire");
        assert_eq!(json["status"], "ativa");
        assert_eq!(json.as_object().unwrap().len(), 11, "11 campos de wire");

        let entrega = Deliverable {
            id: 1,
            run_id: 1,
            task_id: "sq1".into(),
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
