//! Perfil local (A6) — movido de `btv-store::btv` (`BtvUser`) na tarefa A2
//! com `tenant` desde já. É o embrião do Contexto de Identidade (ADR 0024):
//! no modo SaaS a Trilha E liga auth real por cima; aqui continua o perfil
//! local sem barreira de rede de sempre.

use serde::Serialize;

use crate::tenant::TenantId;

/// Resultado de `verify_pin` — perfil aberto (sem PIN), PIN correto ou
/// incorreto. Promovido de `btv-store` à porta (C3.4): a verificação é
/// operação da PORTA (o hash nunca sai do adapter), então o veredito é tipo do
/// domínio. `btv-store` re-exporta para não quebrar chamador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCheck {
    NoPin,
    Ok,
    Wrong,
}

/// Perfil local: identidade nomeada para atribuição, com PIN OPCIONAL
/// verificado pelo backend (o hash nunca é exposto — por isso `has_pin`, não
/// o hash). O PIN gate o "assumir perfil" na UI; não é barreira de rede.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub papel: String,
    pub ativo: bool,
    /// Se o perfil exige PIN para ser assumido.
    pub has_pin: bool,
    /// Tenant do perfil. Fora do wire nesta fase (mesma regra de `run.rs`).
    #[serde(skip_serializing)]
    pub tenant: TenantId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_fica_fora_do_wire() {
        let u = User {
            id: 1,
            nome: "Dani".into(),
            email: "dani@exemplo.com".into(),
            papel: "usuario".into(),
            ativo: true,
            has_pin: false,
            tenant: TenantId::LOCAL,
        };
        let json = serde_json::to_value(&u).unwrap();
        assert!(json.get("tenant").is_none());
        assert_eq!(json.as_object().unwrap().len(), 6);
    }
}
