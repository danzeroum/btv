//! Stubs gRPC gerados de `schemas/proto/` (tonic).
//!
//! Fonte única do contrato: os `.proto` em `schemas/proto/`. Mudança
//! breaking = novo arquivo `.proto` (ex.: `promptforge_v2.proto`) + ADR.

pub mod promptforge {
    tonic::include_proto!("forge.promptforge.v1");
}
