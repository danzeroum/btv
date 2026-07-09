//! Tenant como tipo, fail-closed por construção (D1 do plano, ADR 0025).
//!
//! O modo local-first É um tenant (`TenantId::LOCAL`), não a ausência de um:
//! o mesmo caminho de código serve o modo local e o SaaS, zero fork. Toda
//! operação de repositório (Trilha B) recebe `&TenantContext` — esquecer o
//! filtro de tenant é erro de compilação, não bug de runtime.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TenantError {
    #[error("id de tenant inválido: {0}")]
    InvalidTenantId(String),
    #[error("actor vazio — toda operação precisa de um autor para o ledger")]
    EmptyActor,
}

/// Identidade de tenant — newtype OPACO sobre UUID. Sem `From<String>`/
/// `From<Uuid>`: construção só por `parse` validado ou pela constante
/// `LOCAL` — um id solto não "vira" tenant por acidente (ADR 0025).
///
/// Deliberadamente SEM `Default`: um tenant defaultado em silêncio é o bug
/// clássico de multitenancy. O único default legítimo é o backfill
/// determinístico do modo local, e ele tem nome: `TenantId::LOCAL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(Uuid);

impl TenantId {
    /// Tenant fixo do modo local-first / self-hosted single-tenant
    /// (`00000000-0000-0000-0000-000000000001`). O UUID fixo torna a
    /// migração dos bancos locais existentes um backfill determinístico
    /// (ADRs 0026/0027).
    pub const LOCAL: TenantId = TenantId(Uuid::from_u128(1));

    /// Constrói de um UUID textual validado (borda de auth do modo SaaS,
    /// Trilha E). Única porta de entrada além de `LOCAL`.
    pub fn parse(s: &str) -> Result<Self, TenantError> {
        Uuid::parse_str(s)
            .map(TenantId)
            .map_err(|_| TenantError::InvalidTenantId(s.to_string()))
    }

    /// Função de default EXPLÍCITA para `#[serde(default = ...)]` nos tipos
    /// Serialize-only do domínio — o wire atual não carrega tenant (goldens
    /// T1 congelam isso); os adapters preenchem do contexto (ADR 0026).
    /// Não é `impl Default`: o nome diz o que o valor é.
    pub fn local() -> Self {
        Self::LOCAL
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Quem agiu — alimenta o `actor` do ledger (hoje strings com prefixo de
/// borda, ex.: `web:btv`, `btv-cli:sessao`; o newtype absorve a convenção
/// sem mudar o wire). Não-vazio por construção: entrada de auditoria sem
/// autor não existe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(String);

impl ActorId {
    pub fn new(actor: impl Into<String>) -> Result<Self, TenantError> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(TenantError::EmptyActor);
        }
        Ok(Self(actor))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ActorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Contexto obrigatório de TODA operação de repositório (Trilha B).
///
/// NÃO implementa `Default` — impossível esquecer o tenant por omissão;
/// construir um contexto é sempre decisão explícita do chamador. O `actor`
/// viaja junto porque o ledger precisa dele: uma assinatura que aceitasse só
/// o tenant perderia a autoria da auditoria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantContext {
    pub tenant: TenantId,
    pub actor: ActorId,
}

impl TenantContext {
    pub fn new(tenant: TenantId, actor: ActorId) -> Self {
        Self { tenant, actor }
    }

    /// Contexto do modo local-first: tenant fixo `LOCAL`, actor explícito
    /// (as bordas atuais — CLI, dashboard — sabem quem está agindo).
    pub fn local(actor: ActorId) -> Self {
        Self::new(TenantId::LOCAL, actor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_e_o_uuid_fixo_do_adr_0025() {
        assert_eq!(
            TenantId::LOCAL.to_string(),
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(TenantId::local(), TenantId::LOCAL);
    }

    #[test]
    fn parse_valida_e_roundtrip_serde_preserva_o_uuid() {
        let id = TenantId::parse("00000000-0000-0000-0000-000000000001").unwrap();
        assert_eq!(id, TenantId::LOCAL);
        assert!(TenantId::parse("não-é-uuid").is_err());

        // No wire (quando exposto, Trilha E), TenantId é o UUID textual.
        let json = serde_json::to_string(&TenantId::LOCAL).unwrap();
        assert_eq!(json, "\"00000000-0000-0000-0000-000000000001\"");
        let de: TenantId = serde_json::from_str(&json).unwrap();
        assert_eq!(de, TenantId::LOCAL);
    }

    #[test]
    fn actor_vazio_nao_constroi() {
        assert_eq!(ActorId::new("  "), Err(TenantError::EmptyActor));
        assert_eq!(ActorId::new("web:btv").unwrap().as_str(), "web:btv");
    }

    #[test]
    fn contexto_local_carrega_tenant_fixo_e_actor_explicito() {
        let ctx = TenantContext::local(ActorId::new("btv-cli:sessao").unwrap());
        assert_eq!(ctx.tenant, TenantId::LOCAL);
        assert_eq!(ctx.actor.as_str(), "btv-cli:sessao");
    }
}
