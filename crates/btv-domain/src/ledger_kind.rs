//! `LedgerKind` — o vocabulário FECHADO da trilha auditável (tarefa A3).
//!
//! O `kind` do ledger era string livre ("o ledger já suporta qualquer kind",
//! dívida registrada no ADR 0027); este enum fecha o vocabulário nos 21
//! kinds REAIS de produção (inventário canônico `wire-strings.v1.json`,
//! extraído dos emissores e provado por T3) — kind com typo deixa de compilar
//! no caminho tipado e falha o parse no caminho dinâmico.
//!
//! Este tipo é VOCABULÁRIO, não porta: a decisão diferida das duas portas do
//! ledger (fatos de domínio via `LedgerRepository` × instrumentação via API
//! existente — `pendencias.md` §G1) não é prejulgada aqui; as duas categorias
//! compartilham o mesmo vocabulário e a mesma cadeia.
//!
//! `certification` NÃO tem variante de propósito: é exclusão consciente da
//! fixture (`excluded.ledger_kinds` — só existe em módulo de teste). Se
//! ganhar emissor real, o teste de exclusão do T3 manda promovê-lo, e o
//! round-trip exaustivo daqui obriga a variante nova.

/// Os 21 kinds de produção, por área (a ordem espelha a fixture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LedgerKind {
    // fatos de domínio do produto (as 8 variantes de `DomainEventKind`)
    BtvAdjustRequested,
    BtvExportGenerated,
    BtvFlowSaved,
    BtvGateApproved,
    BtvPersonaUpdated,
    BtvSquadActivated,
    BtvTemplatePublished,
    BtvUserRemoved,
    // instrumentação operacional (a segunda porta — pendencias.md §G1)
    DesignerWorkflowSaved,
    LlmTurn,
    PermissionRuleRevoke,
    PermissionRuleSet,
    SessionEnd,
    SessionStart,
    SkillVetting,
    SquadConsensus,
    SquadToolRun,
    ToolDenied,
    ToolResult,
    ToolRun,
    UserTurn,
}

impl LedgerKind {
    /// Todas as variantes — base do round-trip exaustivo e da cobertura
    /// contra a fixture (variante nova sem entrada aqui quebra os testes).
    pub const ALL: [LedgerKind; 21] = [
        LedgerKind::BtvAdjustRequested,
        LedgerKind::BtvExportGenerated,
        LedgerKind::BtvFlowSaved,
        LedgerKind::BtvGateApproved,
        LedgerKind::BtvPersonaUpdated,
        LedgerKind::BtvSquadActivated,
        LedgerKind::BtvTemplatePublished,
        LedgerKind::BtvUserRemoved,
        LedgerKind::DesignerWorkflowSaved,
        LedgerKind::LlmTurn,
        LedgerKind::PermissionRuleRevoke,
        LedgerKind::PermissionRuleSet,
        LedgerKind::SessionEnd,
        LedgerKind::SessionStart,
        LedgerKind::SkillVetting,
        LedgerKind::SquadConsensus,
        LedgerKind::SquadToolRun,
        LedgerKind::ToolDenied,
        LedgerKind::ToolResult,
        LedgerKind::ToolRun,
        LedgerKind::UserTurn,
    ];

    /// A string EXATA do banco (contrato T3) — match exaustivo: variante
    /// nova sem string não compila.
    pub fn as_str(self) -> &'static str {
        match self {
            LedgerKind::BtvAdjustRequested => "btv.adjust_requested",
            LedgerKind::BtvExportGenerated => "btv.export_generated",
            LedgerKind::BtvFlowSaved => "btv.flow_saved",
            LedgerKind::BtvGateApproved => "btv.gate_approved",
            LedgerKind::BtvPersonaUpdated => "btv.persona_updated",
            LedgerKind::BtvSquadActivated => "btv.squad_activated",
            LedgerKind::BtvTemplatePublished => "btv.template_published",
            LedgerKind::BtvUserRemoved => "btv.user_removed",
            LedgerKind::DesignerWorkflowSaved => "designer.workflow_saved",
            LedgerKind::LlmTurn => "llm.turn",
            LedgerKind::PermissionRuleRevoke => "permission_rule.revoke",
            LedgerKind::PermissionRuleSet => "permission_rule.set",
            LedgerKind::SessionEnd => "session.end",
            LedgerKind::SessionStart => "session.start",
            LedgerKind::SkillVetting => "skill.vetting",
            LedgerKind::SquadConsensus => "squad.consensus",
            LedgerKind::SquadToolRun => "squad.tool_run",
            LedgerKind::ToolDenied => "tool.denied",
            LedgerKind::ToolResult => "tool.result",
            LedgerKind::ToolRun => "tool.run",
            LedgerKind::UserTurn => "user.turn",
        }
    }

    /// Parse fail-closed: kind fora do vocabulário é `UnknownKind` — typo em
    /// call site deixa de virar entrada silenciosa na cadeia.
    pub fn parse(s: &str) -> Result<Self, UnknownKind> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| UnknownKind(s.to_string()))
    }
}

impl std::fmt::Display for LedgerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Kind fora do vocabulário fechado.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("kind de ledger fora do vocabulário: {0}")]
pub struct UnknownKind(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn kinds_da_fixture() -> BTreeSet<String> {
        let fixture: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/fixtures/wire-strings.v1.json"),
            )
            .expect("fixture wire-strings.v1"),
        )
        .expect("fixture é JSON");
        fixture["ledger_kinds"]
            .as_array()
            .expect("lista ledger_kinds")
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_string)
            .collect()
    }

    /// O vocabulário do enum é EXATAMENTE o inventário canônico do T3 — kind
    /// novo na fixture sem variante (ou variante órfã) quebra aqui.
    #[test]
    fn vocabulario_igual_a_fixture() {
        let do_enum: BTreeSet<String> = LedgerKind::ALL.iter().map(|k| k.as_str().into()).collect();
        assert_eq!(do_enum, kinds_da_fixture());
    }

    /// Round-trip exaustivo sobre TODAS as variantes: parse(as_str(k)) == k.
    #[test]
    fn roundtrip_exaustivo() {
        for kind in LedgerKind::ALL {
            assert_eq!(LedgerKind::parse(kind.as_str()), Ok(kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
    }

    /// Fail-closed: typo e a exclusão consciente (`certification`) não parseiam.
    #[test]
    fn typo_e_exclusao_consciente_nao_parseiam() {
        assert_eq!(
            LedgerKind::parse("btv.gate_aproved"),
            Err(UnknownKind("btv.gate_aproved".into()))
        );
        assert!(LedgerKind::parse("certification").is_err());
    }

    /// Coerência entre as duas portas: todo kind que `DomainEventKind` emite
    /// pertence ao vocabulário (e é da família `btv.*`).
    #[test]
    fn kinds_do_domain_event_pertencem_ao_vocabulario() {
        use crate::ports::DomainEventKind;
        let dummy = [
            DomainEventKind::UserRemoved { user_id: 1 },
            DomainEventKind::TemplatePublished {
                template_id: "editorial".into(),
                published: true,
            },
        ];
        for kind in dummy {
            let parsed = LedgerKind::parse(kind.wire_kind()).expect("kind emitido pertence");
            assert!(parsed.as_str().starts_with("btv."));
        }
    }
}
