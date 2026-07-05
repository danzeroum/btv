//! Runtime de sessão da plataforma Forge.
//!
//! Fase 1 (scaffold): motor de permissões e perfis de agente. O System
//! Context completo (Context Sources tipados, Epochs, compaction em Safe
//! Provider-Turn Boundaries — spec: `opencode/CONTEXT.md`) chega na Fase 2.

pub mod agent;
pub mod permission;

pub use agent::{AgentProfile, BUILD, GENERAL, PLAN};
pub use permission::{Decision, PermissionEngine, Rule};
