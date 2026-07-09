//! Personas (U7): override de prompt por modelo+papel e personas próprias —
//! movidos de `btv-store::btv` na tarefa A2 com `tenant` desde já (D1).
//! Área declarada "além da tipagem, não tocar" (ADR 0024 não-escopo): este
//! movimento é SÓ tipagem — nenhuma semântica nova.

use serde::Serialize;

use crate::tenant::TenantId;

/// Override do prompt de um papel de template (efetivo na próxima ativação;
/// auditado no ledger como `btv.persona_updated`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonaOverride {
    pub template_id: String,
    pub papel: String,
    pub prompt: String,
    /// Dono do override. Fora do wire nesta fase (mesma regra de `run.rs`).
    #[serde(skip_serializing)]
    pub tenant: TenantId,
}

/// Persona própria criada pelo usuário para um template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CustomPersona {
    pub id: i64,
    pub template_id: String,
    pub nome: String,
    pub prompt: String,
    /// Dono da persona. Fora do wire nesta fase (mesma regra de `run.rs`).
    #[serde(skip_serializing)]
    pub tenant: TenantId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_fica_fora_do_wire() {
        let p = CustomPersona {
            id: 1,
            template_id: "editorial".into(),
            nome: "Ghostwriter".into(),
            prompt: "Escreva como ghostwriter.".into(),
            tenant: TenantId::LOCAL,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert!(json.get("tenant").is_none());
        assert_eq!(json.as_object().unwrap().len(), 4);
    }
}
