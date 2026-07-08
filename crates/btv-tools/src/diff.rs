//! Diff de linhas entre o conteúdo antes/depois de um `edit` — usado pelo
//! modelo (contexto da mudança) e pela TUI (bloco colorido).
//!
//! Como o `edit` já sabe exatamente qual trecho mudou (substituição de
//! `old_string`), o diff usa a simplificação de maior-prefixo-comum /
//! maior-sufixo-comum: tudo antes da primeira linha divergente e depois da
//! última é contexto; o meio é removido/adicionado. Isso é exato para
//! edições localizadas (o caso comum); com `replace_all` em ocorrências
//! muito distantes entre si, o "meio" pode incluir trechos inalterados —
//! aceitável para um diff informativo, não para um patch aplicável.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLine {
    Context(String),
    Removed(String),
    Added(String),
}

/// Linhas de contexto mantidas ao redor da região alterada.
const CONTEXT_LINES: usize = 2;

/// Calcula o diff de linhas entre `before` e `after`, com contexto limitado.
pub fn line_diff(before: &str, after: &str) -> Vec<DiffLine> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let max_common = before_lines.len().min(after_lines.len());
    let mut prefix = 0;
    while prefix < max_common && before_lines[prefix] == after_lines[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < max_common - prefix
        && before_lines[before_lines.len() - 1 - suffix]
            == after_lines[after_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let removed = &before_lines[prefix..before_lines.len() - suffix];
    let added = &after_lines[prefix..after_lines.len() - suffix];
    if removed.is_empty() && added.is_empty() {
        return Vec::new();
    }

    // Janela de contexto ao redor da região alterada.
    let ctx_before_start = prefix.saturating_sub(CONTEXT_LINES);
    let after_region_end = before_lines.len() - suffix;
    let ctx_after_end = (after_region_end + CONTEXT_LINES).min(before_lines.len());

    let mut out = Vec::new();
    if ctx_before_start > 0 {
        out.push(DiffLine::Context(format!(
            "… ({ctx_before_start} linhas antes)"
        )));
    }
    for line in &before_lines[ctx_before_start..prefix] {
        out.push(DiffLine::Context((*line).to_string()));
    }
    for line in removed {
        out.push(DiffLine::Removed((*line).to_string()));
    }
    for line in added {
        out.push(DiffLine::Added((*line).to_string()));
    }
    for line in &before_lines[after_region_end..ctx_after_end] {
        out.push(DiffLine::Context((*line).to_string()));
    }
    if ctx_after_end < before_lines.len() {
        out.push(DiffLine::Context(format!(
            "… ({} linhas depois)",
            before_lines.len() - ctx_after_end
        )));
    }
    out
}

/// Formata o diff no estilo unificado compacto (para o texto devolvido ao modelo).
pub fn format_diff(lines: &[DiffLine]) -> String {
    lines
        .iter()
        .map(|l| match l {
            DiffLine::Context(s) => format!("  {s}"),
            DiffLine::Removed(s) => format!("- {s}"),
            DiffLine::Added(s) => format!("+ {s}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edicao_localizada_produz_diff_minimo() {
        let before = "let x = 1;\nlet y = 2;\nlet z = 3;\n";
        let after = "let x = 10;\nlet y = 2;\nlet z = 3;\n";
        let diff = line_diff(before, after);
        assert_eq!(
            diff,
            vec![
                DiffLine::Removed("let x = 1;".into()),
                DiffLine::Added("let x = 10;".into()),
                DiffLine::Context("let y = 2;".into()),
                DiffLine::Context("let z = 3;".into()),
            ]
        );
    }

    #[test]
    fn nenhuma_mudanca_produz_diff_vazio() {
        assert!(line_diff("a\nb\n", "a\nb\n").is_empty());
    }

    #[test]
    fn contexto_e_limitado_em_arquivos_grandes() {
        let mut before_lines: Vec<String> = (0..20).map(|i| format!("linha {i}")).collect();
        let after_lines = before_lines.clone();
        before_lines[10] = "alterada".into();
        let mut after = after_lines.clone();
        after[10] = "nova".into();

        let diff = line_diff(&before_lines.join("\n"), &after.join("\n"));
        // 2 linhas de contexto antes + marcador + removida + adicionada + 2 depois
        assert!(diff.contains(&DiffLine::Removed("alterada".into())));
        assert!(diff.contains(&DiffLine::Added("nova".into())));
        assert!(
            diff.len() < 10,
            "contexto não deve incluir o arquivo inteiro"
        );
    }

    #[test]
    fn format_diff_usa_prefixos_unificados() {
        let text = format_diff(&[
            DiffLine::Context("igual".into()),
            DiffLine::Removed("velho".into()),
            DiffLine::Added("novo".into()),
        ]);
        assert_eq!(text, "  igual\n- velho\n+ novo");
    }
}
