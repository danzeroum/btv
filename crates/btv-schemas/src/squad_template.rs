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
}
