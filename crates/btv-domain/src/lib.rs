//! Domínio BuildToValue — Trilha A do plano DDD multitenant (ADRs 0024–0027).
//!
//! Este crate é o núcleo SEM infraestrutura do produto: tenant (ADR 0025) e
//! os tipos dos contextos Core do mapa (ADR 0024 — produto BTV: runs,
//! entregas, personas, perfis). A regra de fronteira é verificada por
//! máquina: `scripts/arch-lint.sh` (job `arch-lint` do CI) falha o build se
//! rusqlite/axum/tonic/reqwest entrarem aqui, direta ou transitivamente.
//!
//! Convenção de nomes (ADR 0024, decisão 2): identificadores NOVOS em inglês
//! (`Run`, `Deliverable`, `TenantId`); os campos de contrato serializado que
//! JÁ são português (`nome`, `papeis`, `formato`, `trilha`…) permanecem — o
//! wire está congelado pelos goldens T1 e property tests T3.

pub mod persona;
pub mod run;
pub mod tenant;
pub mod user;

pub use persona::{CustomPersona, PersonaOverride};
pub use run::{Deliverable, Run};
pub use tenant::{ActorId, TenantContext, TenantError, TenantId};
pub use user::User;
