//! Modelo de squad do BuildToValue (`squad-template.v1`).
//!
//! Fonte única dos 12 modelos da galeria (U1) em
//! `schemas/squad-templates/*.json` — embutidos no `btv-server`
//! (`include_str!`) e servidos em `GET /api/btv/templates`. O wizard (U2)
//! deriva as perguntas de briefing e a equipe daqui; a ativação (Onda 3)
//! deriva as etapas da esteira e os prompts padrão por papel.
//!
//! `FormatoEntrega::binario` marca formatos cuja exportação direta ainda não
//! é suportada (DOCX/PDF/XLSX/... exigem conversor na sandbox — onda futura):
//! a UI mostra o formato desabilitado ("em breve"), nunca um botão de export
//! que entrega outra coisa.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CategoriaSquad {
    Conteudo,
    Analise,
    Criativa,
    Operacoes,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FormatoEntrega {
    pub nome: String,
    /// Formato binário → exportação direta indisponível até existir o
    /// conversor real na sandbox. Nunca fingir.
    pub binario: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerguntaBriefing {
    pub label: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SquadTemplate {
    pub id: String,
    pub nome: String,
    pub categoria: CategoriaSquad,
    /// Cor de identidade do modelo (handoff §4) — hex minúsculo `#rrggbb`.
    pub cor: String,
    /// Onda de maturidade (1–3) exibida no card da galeria.
    pub onda: u8,
    /// Versão de exibição/publicação (A5) — `vMAJOR.MINOR`.
    pub versao: String,
    pub publicado: bool,
    pub descricao: String,
    pub papeis: Vec<String>,
    pub formatos: Vec<FormatoEntrega>,
    pub perguntas: Vec<PerguntaBriefing>,
    /// Pontos onde a esteira para e espera o humano (U2 passo 3 / U3).
    pub gates: Vec<String>,
}

impl SquadTemplate {
    /// Checagens semânticas além do JSON Schema (que já cobre campos, enum,
    /// regex de cor/versão e limites de onda): invariantes que o schema puro
    /// não expressa de forma legível.
    pub fn validate(&self) -> Result<(), String> {
        if self.papeis.is_empty() {
            return Err(format!("template '{}' sem papéis", self.id));
        }
        if !(1..=3).contains(&self.onda) {
            return Err(format!("template '{}' com onda fora de 1–3", self.id));
        }
        if self.formatos.is_empty() {
            return Err(format!("template '{}' sem formatos de entrega", self.id));
        }
        Ok(())
    }
}

// ── catálogo embutido (C1 da Trilha C do plano DDD) ─────────────────────────
//
// Movido de `btv-server::btv` para matar a inversão de dependência
// CLI→Server (levantamento E9, violação 3): o catálogo é CONTRATO, e o crate
// de contratos é a fonte única — o server serve, a CLI ativa, ninguém mais
// atravessa camada para ler template.

/// Ordem de exibição da galeria (U1): ondas 1 → 2 → 3, mesma sequência do
/// protótipo. Embutidos no binário em tempo de compilação — o dashboard
/// funciona de qualquer CWD, instalado ou não, sem depender do checkout.
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

/// Parse único e cacheado dos 12 modelos. O `expect` é seguro por
/// construção: as fontes são literais de compile-time cobertas por
/// `todos_os_templates_embutidos_parseiam_e_validam`.
pub fn builtin_templates() -> &'static [SquadTemplate] {
    static TEMPLATES: std::sync::OnceLock<Vec<SquadTemplate>> = std::sync::OnceLock::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn template_minimo() -> SquadTemplate {
        SquadTemplate {
            id: "editorial".into(),
            nome: "Editorial / SEO".into(),
            categoria: CategoriaSquad::Conteudo,
            cor: "#b8531f".into(),
            onda: 1,
            versao: "v1.4".into(),
            publicado: true,
            descricao: "Artigos com revisão embutida.".into(),
            papeis: vec!["Pauteiro".into(), "Redator".into()],
            formatos: vec![FormatoEntrega {
                nome: "MD".into(),
                binario: false,
            }],
            perguntas: vec![PerguntaBriefing {
                label: "Qual é a pauta?".into(),
                placeholder: "ex.: logística verde".into(),
            }],
            gates: vec!["Aprovar o rascunho antes da revisão".into()],
        }
    }

    #[test]
    fn template_valido_passa() {
        assert!(template_minimo().validate().is_ok());
    }

    #[test]
    fn template_sem_papeis_reprova_com_erro_claro() {
        let mut t = template_minimo();
        t.papeis.clear();
        let err = t.validate().unwrap_err();
        assert!(err.contains("sem papéis"), "erro: {err}");
    }

    #[test]
    fn onda_fora_do_intervalo_reprova() {
        let mut t = template_minimo();
        t.onda = 4;
        assert!(t.validate().is_err());
    }

    #[test]
    fn roundtrip_serde_preserva_categoria_snake_case() {
        let json = serde_json::to_value(template_minimo()).unwrap();
        assert_eq!(json["categoria"], "conteudo");
        let back: SquadTemplate = serde_json::from_value(json).unwrap();
        assert_eq!(back.id, "editorial");
    }

    // Movidos de btv-server::btv junto com o catálogo (C1): um JSON inválido
    // quebra o build nos testes, nunca o servidor em produção.

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
