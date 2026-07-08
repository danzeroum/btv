//! Stubs gRPC gerados de `schemas/proto/` (tonic).
//!
//! Fonte única do contrato: os `.proto` em `schemas/proto/`. Mudança
//! breaking = novo arquivo `.proto` (ex.: `promptforge_v2.proto`) + ADR.
//!
//! A estrutura de módulos espelha a hierarquia de pacotes protobuf
//! (`btv.core.v1` → `btv::core::v1`), porque o prost gera referências
//! entre pacotes por caminho relativo (`super::super::llm::v1::LlmRequest`
//! em `core` referenciando `llm`) — só funciona com o aninhamento correto.

pub mod btv {
    pub mod llm {
        pub mod v1 {
            tonic::include_proto!("btv.llm.v1");
        }
    }
    pub mod core {
        pub mod v1 {
            tonic::include_proto!("btv.core.v1");
        }
    }
    pub mod squad {
        pub mod v1 {
            tonic::include_proto!("btv.squad.v1");
        }
    }
    pub mod promptforge {
        pub mod v1 {
            tonic::include_proto!("btv.promptforge.v1");
        }
    }
    pub mod memory {
        pub mod v1 {
            tonic::include_proto!("btv.memory.v1");
        }
    }
}

// Aliases curtos e estáveis para os consumidores. `promptforge` já era
// exposto assim (btv-sidecar depende disso) — mantido; os demais seguem
// o mesmo formato.
pub mod promptforge {
    pub use crate::btv::promptforge::v1::*;
}
pub mod llm {
    pub use crate::btv::llm::v1::*;
}
pub mod core {
    pub use crate::btv::core::v1::*;
}
pub mod squad {
    pub use crate::btv::squad::v1::*;
}
pub mod memory {
    pub use crate::btv::memory::v1::*;
}
