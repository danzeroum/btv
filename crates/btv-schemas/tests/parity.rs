//! Teste de contrato cross-language do `prompt-cache-key.v1`.
//!
//! As mesmas fixtures são validadas pelo Python em
//! `python/packages/btv-promptforge/tests/test_hashing.py`. Divergência
//! aqui = quebra do contrato de cache entre gateway (Rust) e sidecar
//! (Python).

use serde_json::Value;

#[test]
fn paridade_com_fixtures_compartilhadas() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/prompt-cache-key.v1.json"
    );
    let raw = std::fs::read_to_string(path).expect("fixtures existem");
    let doc: Value = serde_json::from_str(&raw).expect("fixtures são JSON válido");
    let cases = doc["cases"].as_array().expect("campo cases");
    assert!(cases.len() >= 5, "esperava pelo menos 5 fixtures");

    for case in cases {
        let name = case["name"].as_str().unwrap();
        let expected = case["sha256"].as_str().unwrap();
        let got = btv_schemas::request_hash(&case["messages"], &case["temperature"])
            .expect("caso válido não deve ser rejeitado");
        assert_eq!(got, expected, "fixture {name}");
    }

    // Casos PROIBIDOS (ADR 0032): ambos os lados devem RECUSAR — o Python
    // (`test_hashing.py`) valida os mesmos `reject_cases` levantando CacheKeyError.
    let reject = doc["reject_cases"].as_array().expect("campo reject_cases");
    assert!(!reject.is_empty(), "esperava reject_cases");
    for case in reject {
        let name = case["name"].as_str().unwrap();
        assert!(
            btv_schemas::request_hash(&case["messages"], &case["temperature"]).is_err(),
            "reject_case {name} deveria ser rejeitado pelo guard do v1"
        );
    }
}
