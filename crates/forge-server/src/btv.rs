//! Rotas do BuildToValue (produto sobre o motor Forge — ver
//! `docs/design_handoff_buildtovalue/`).
//!
//! Os 12 modelos de squad têm fonte única em `schemas/squad-templates/*.json`
//! (contrato `squad-template.v1`) e são **embutidos no binário** em tempo de
//! compilação — o dashboard funciona de qualquer CWD, instalado ou não, sem
//! depender do checkout do repo. O teste deste módulo prova que todos os 12
//! parseiam e validam; um JSON inválido quebra o build nos testes, nunca o
//! servidor em produção.

use axum::Json;
use forge_schemas::squad_template::SquadTemplate;
use std::sync::OnceLock;

/// Ordem de exibição da galeria (U1): ondas 1 → 2 → 3, mesma sequência do
/// protótipo.
const TEMPLATE_SOURCES: [&str; 12] = [
    include_str!("../../../schemas/squad-templates/editorial.json"),
    include_str!("../../../schemas/squad-templates/pesquisa.json"),
    include_str!("../../../schemas/squad-templates/bi.json"),
    include_str!("../../../schemas/squad-templates/operacoes.json"),
    include_str!("../../../schemas/squad-templates/sales.json"),
    include_str!("../../../schemas/squad-templates/imagem.json"),
    include_str!("../../../schemas/squad-templates/educacao.json"),
    include_str!("../../../schemas/squad-templates/design.json"),
    include_str!("../../../schemas/squad-templates/juridico.json"),
    include_str!("../../../schemas/squad-templates/musica.json"),
    include_str!("../../../schemas/squad-templates/podcast.json"),
    include_str!("../../../schemas/squad-templates/video.json"),
];

/// Parse único e cacheado. O `expect` é seguro por construção: as fontes são
/// literais de compile-time cobertas por `todos_os_templates_embutidos_parseiam_e_validam`.
pub(crate) fn builtin_templates() -> &'static [SquadTemplate] {
    static TEMPLATES: OnceLock<Vec<SquadTemplate>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        TEMPLATE_SOURCES
            .iter()
            .map(|src| {
                let t: SquadTemplate = serde_json::from_str(src)
                    .expect("template embutido é squad-template.v1 válido (provado por teste)");
                t
            })
            .collect()
    })
}

/// `GET /api/btv/templates` — os modelos da galeria (U1), do wizard (U2) e da
/// tabela de modelos do admin (A5).
pub(crate) async fn list_templates() -> Json<&'static [SquadTemplate]> {
    Json(builtin_templates())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_os_templates_embutidos_parseiam_e_validam() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 12);
        for t in templates {
            t.validate()
                .unwrap_or_else(|e| panic!("template '{}' inválido: {e}", t.id));
        }
        // Identidade de navegação (aprovação obs. 6): os 12 hex exatos da
        // seção 4 do handoff, na ordem da galeria.
        let cores: Vec<&str> = templates.iter().map(|t| t.cor.as_str()).collect();
        assert_eq!(
            cores,
            [
                "#b8531f", "#345f9e", "#1d6f63", "#57702b", "#b0742c", "#8d3f6a", "#9a6b14",
                "#2b7a8c", "#6b5744", "#6b4fae", "#c04a4a", "#444d99"
            ]
        );
    }

    #[test]
    fn formatos_binarios_sao_marcados_para_export_honesto() {
        let templates = builtin_templates();
        let juridico = templates.iter().find(|t| t.id == "juridico").unwrap();
        // DOCX/PDF ainda não têm conversor real — a UI desabilita o export.
        assert!(juridico
            .formatos
            .iter()
            .any(|f| f.nome == "DOCX" && f.binario));
        assert!(juridico
            .formatos
            .iter()
            .any(|f| f.nome == "Checklist" && !f.binario));
    }
}
