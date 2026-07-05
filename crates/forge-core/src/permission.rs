//! Motor de permissões por ferramenta e escopo (origem: opencode).
//!
//! Superfície de segurança: as decisões vivem no processo Rust e não são
//! contornáveis pelo sidecar Python — o squad pede permissão via
//! `CoreService.RequestPermission` e recebe a decisão pronta.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

/// Regra: decisão para uma ferramenta, opcionalmente restrita a um prefixo
/// de escopo (caminho de arquivo, comando, URL...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_prefix: Option<String>,
    pub decision: Decision,
}

/// Avalia regras na ordem: a primeira compatível vence; sem regra → `Ask`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionEngine {
    pub rules: Vec<Rule>,
}

impl PermissionEngine {
    pub fn evaluate(&self, tool: &str, scope: &str) -> Decision {
        for rule in &self.rules {
            if rule.tool != tool {
                continue;
            }
            match &rule.scope_prefix {
                Some(prefix) if !scope.starts_with(prefix.as_str()) => continue,
                _ => return rule.decision,
            }
        }
        Decision::Ask
    }

    /// Perfil somente leitura (safe mode / agente `plan`): edits e bash
    /// negados ou sob pergunta, leitura liberada.
    pub fn read_only() -> Self {
        Self {
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
                    decision: Decision::Deny,
                },
                Rule {
                    tool: "bash".into(),
                    scope_prefix: None,
                    decision: Decision::Ask,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_regra_pergunta() {
        assert_eq!(
            PermissionEngine::default().evaluate("edit", "src/main.rs"),
            Decision::Ask
        );
    }

    #[test]
    fn escopo_por_prefixo() {
        let engine = PermissionEngine {
            rules: vec![
                Rule {
                    tool: "edit".into(),
                    scope_prefix: Some("src/".into()),
                    decision: Decision::Allow,
                },
                Rule {
                    tool: "edit".into(),
                    scope_prefix: None,
                    decision: Decision::Deny,
                },
            ],
        };
        assert_eq!(engine.evaluate("edit", "src/lib.rs"), Decision::Allow);
        assert_eq!(engine.evaluate("edit", "/etc/passwd"), Decision::Deny);
    }

    #[test]
    fn read_only_nega_edits() {
        let engine = PermissionEngine::read_only();
        assert_eq!(engine.evaluate("edit", "src/lib.rs"), Decision::Deny);
        assert_eq!(engine.evaluate("read", "src/lib.rs"), Decision::Allow);
        assert_eq!(engine.evaluate("bash", "cargo test"), Decision::Ask);
    }
}
