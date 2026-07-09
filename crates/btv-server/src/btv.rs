//! Rotas do BuildToValue (produto sobre o motor BuildToValue — ver
//! `docs/design_handoff_buildtovalue/`).
//!
//! O catálogo embutido dos 12 modelos mudou de casa na C1 da Trilha C
//! (plano DDD): a fonte única agora é `btv_schemas::squad_template::
//! builtin_templates()` — contrato mora no crate de contratos, e a
//! ativação (`btv-cli::btv_agent`) o consome de lá sem atravessar este
//! crate (inversão CLI→Server morta; levantamento E9). Este módulo só
//! SERVE o catálogo.

use axum::Json;
use btv_schemas::squad_template::SquadTemplate;

/// Re-export de compatibilidade (mesmo caminho público de antes da C1).
pub use btv_schemas::squad_template::builtin_templates;

/// `GET /api/btv/templates` — os modelos da galeria (U1), do wizard (U2) e da
/// tabela de modelos do admin (A5).
pub(crate) async fn list_templates() -> Json<&'static [SquadTemplate]> {
    Json(builtin_templates())
}
