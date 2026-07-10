//! Tipos de conversa/tool-use — desde o D1t moram em `btv-domain::chat`
//! (o loop de agente os consome via `LlmPort` sem conhecer este crate);
//! este módulo re-exporta para os consumidores históricos. A conversão
//! provider-específica continua no gateway.

pub use btv_domain::chat::{
    AssistantTurn, ChatMessage, ContentBlock, GenerateRequest, Role, StopReason, ToolSpec, Usage,
};
