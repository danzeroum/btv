//! Tipos compartilhados e contratos serializados da plataforma BuildToValue.
//!
//! Os documentos persistidos/auditáveis têm schema canônico em
//! `platform/schemas/json/*.v1.schema.json`. Os tipos deste crate devem
//! permanecer compatíveis com esses arquivos (testes de contrato garantem).

pub mod canonical;
pub mod experiment;
pub mod handoff;
pub mod ledger;
pub mod persona;
pub mod plan;
pub mod squad_template;
pub mod telemetry;
pub mod verification;
pub mod workflow;

pub use canonical::{canonical_json, request_hash, sha256_hex};
