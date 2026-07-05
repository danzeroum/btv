//! Stubs gRPC — placeholder até a Fase 3 (primeira ativação do canal
//! Rust↔Python).
//!
//! Quando ativado: `build.rs` com `tonic-build` sobre
//! `platform/schemas/proto/{core,squad,llm}.proto`. Os protos são a fonte
//! única de contrato; `buf breaking` no CI impede mudanças incompatíveis.
