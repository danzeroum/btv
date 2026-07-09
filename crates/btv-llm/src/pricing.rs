//! Tabela de preços por modelo/provider (validação de pendencias.md — custo
//! monetário por squad, A1). Preços em **USD por 1 milhão de tokens** (input e
//! output separados), a convenção que os provedores publicam.
//!
//! **Honestidade:** esta é uma tabela ESTÁTICA embutida (não uma consulta de
//! preço ao vivo — provedores mudam preço e a tabela envelhece). O custo
//! derivado é uma **estimativa** a partir de tokens reais (`ModelUsage`) ×
//! preço tabelado; a UI diz isso e mostra a data de referência (`AS_OF`).
//! Modelo sem preço na tabela → `None` (a UI mostra "sem preço tabelado", não
//! um custo fabricado). Casa por prefixo/substring do id, na ordem da tabela.

/// Data de referência dos preços — mostrada na UI para o usuário saber quão
/// atual (ou não) é a estimativa.
pub const AS_OF: &str = "2026-01";

/// Preço de um modelo, em USD por 1M de tokens.
#[derive(Debug, Clone, Copy)]
pub struct ModelPrice {
    /// Provider dono do modelo (rótulo).
    pub provider: &'static str,
    /// USD por 1M de tokens de entrada.
    pub input_per_mtok: f64,
    /// USD por 1M de tokens de saída.
    pub output_per_mtok: f64,
}

/// Regras de casamento (substring do id → preço), na ordem de prioridade.
/// A primeira que casar vence — por isso os mais específicos (ex.: `-mini`,
/// `haiku`) vêm antes dos genéricos do mesmo provider.
const PRICES: &[(&str, ModelPrice)] = &[
    // Anthropic
    (
        "haiku",
        ModelPrice {
            provider: "anthropic",
            input_per_mtok: 0.80,
            output_per_mtok: 4.00,
        },
    ),
    (
        "opus",
        ModelPrice {
            provider: "anthropic",
            input_per_mtok: 15.00,
            output_per_mtok: 75.00,
        },
    ),
    (
        "sonnet",
        ModelPrice {
            provider: "anthropic",
            input_per_mtok: 3.00,
            output_per_mtok: 15.00,
        },
    ),
    // DeepSeek
    (
        "deepseek",
        ModelPrice {
            provider: "deepseek",
            input_per_mtok: 0.27,
            output_per_mtok: 1.10,
        },
    ),
    // OpenAI (mais específico antes)
    (
        "gpt-4o-mini",
        ModelPrice {
            provider: "openai",
            input_per_mtok: 0.15,
            output_per_mtok: 0.60,
        },
    ),
    (
        "gpt-4o",
        ModelPrice {
            provider: "openai",
            input_per_mtok: 2.50,
            output_per_mtok: 10.00,
        },
    ),
    (
        "gpt-4.1",
        ModelPrice {
            provider: "openai",
            input_per_mtok: 2.00,
            output_per_mtok: 8.00,
        },
    ),
];

/// Preço tabelado de um modelo pelo id (casa por substring, ordem da tabela).
/// `None` se não houver preço — a UI não fabrica custo nesse caso.
pub fn price_for(model: &str) -> Option<ModelPrice> {
    let m = model.to_ascii_lowercase();
    PRICES
        .iter()
        .find(|(key, _)| m.contains(key))
        .map(|(_, price)| *price)
}

/// Custo estimado (USD) de `input_tokens`/`output_tokens` de um modelo, pela
/// tabela. `None` se o modelo não tem preço tabelado.
pub fn estimate_cost_usd(model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    let p = price_for(model)?;
    let cost = (input_tokens as f64 / 1_000_000.0) * p.input_per_mtok
        + (output_tokens as f64 / 1_000_000.0) * p.output_per_mtok;
    Some(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casa_por_substring_e_prioriza_o_especifico() {
        assert_eq!(price_for("claude-sonnet-5").unwrap().provider, "anthropic");
        assert_eq!(price_for("claude-3-haiku").unwrap().input_per_mtok, 0.80);
        // gpt-4o-mini casa antes de gpt-4o (ordem da tabela).
        assert_eq!(price_for("gpt-4o-mini").unwrap().output_per_mtok, 0.60);
        assert_eq!(price_for("gpt-4o").unwrap().output_per_mtok, 10.00);
        assert_eq!(price_for("deepseek-chat").unwrap().provider, "deepseek");
    }

    #[test]
    fn modelo_desconhecido_nao_tem_preco() {
        assert!(price_for("modelo-fantasma-xyz").is_none());
        assert!(estimate_cost_usd("modelo-fantasma-xyz", 1000, 1000).is_none());
    }

    #[test]
    fn estimativa_de_custo_bate_a_conta() {
        // sonnet: 1M in × $3 + 1M out × $15 = $18.
        let c = estimate_cost_usd("claude-sonnet-5", 1_000_000, 1_000_000).unwrap();
        assert!((c - 18.0).abs() < 1e-9, "esperava $18, veio {c}");
        // Meio milhão in + zero out no haiku: 0.5 × $0.80 = $0.40.
        let c = estimate_cost_usd("claude-3-haiku", 500_000, 0).unwrap();
        assert!((c - 0.40).abs() < 1e-9, "esperava $0.40, veio {c}");
    }
}
