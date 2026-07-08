//! Perfis de agente selecionáveis (origem: opencode — build/plan/general).

use crate::permission::PermissionEngine;

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub name: &'static str,
    pub description: &'static str,
    /// Política de permissões aplicada quando o perfil está ativo.
    pub permissions: fn() -> PermissionEngine,
}

fn full_access() -> PermissionEngine {
    use crate::permission::{Decision, Rule};
    PermissionEngine {
        rules: vec![
            Rule {
                tool: "read".into(),
                scope_prefix: None,
                decision: Decision::Allow,
            },
            Rule {
                tool: "grep".into(),
                scope_prefix: None,
                decision: Decision::Allow,
            },
            Rule {
                tool: "edit".into(),
                scope_prefix: None,
                decision: Decision::Ask,
            },
            Rule {
                tool: "bash".into(),
                scope_prefix: None,
                decision: Decision::Ask,
            },
        ],
    }
}

/// Agente padrão com acesso total (edits/bash sob confirmação).
pub const BUILD: AgentProfile = AgentProfile {
    name: "build",
    description: "acesso total; edits e bash pedem confirmação",
    permissions: full_access,
};

/// Agente de planejamento: somente leitura.
pub const PLAN: AgentProfile = AgentProfile {
    name: "plan",
    description: "somente leitura; edits negados",
    permissions: PermissionEngine::read_only,
};

/// Subagente para buscas e tarefas multi-etapa.
pub const GENERAL: AgentProfile = AgentProfile {
    name: "general",
    description: "subagente de exploração multi-etapa",
    permissions: PermissionEngine::read_only,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::Decision;

    #[test]
    fn plan_e_somente_leitura() {
        let perms = (PLAN.permissions)();
        assert_eq!(perms.evaluate("edit", "x"), Decision::Deny);
    }

    #[test]
    fn build_pede_confirmacao_para_edit() {
        let perms = (BUILD.permissions)();
        assert_eq!(perms.evaluate("edit", "x"), Decision::Ask);
    }
}
