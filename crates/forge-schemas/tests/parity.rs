//! Teste de contrato cross-language do `prompt-cache-key.v1`.
//!
//! As mesmas fixtures são validadas pelo Python em
//! `python/packages/forge-promptforge/tests/test_hashing.py`. Divergência
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
        let got = forge_schemas::request_hash(&case["messages"], &case["temperature"]);
        assert_eq!(got, expected, "fixture {name}");
    }
}
