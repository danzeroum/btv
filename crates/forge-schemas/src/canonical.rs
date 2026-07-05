//! Canonicalização JSON e hash de cache de prompt (`prompt-cache-key.v1`).
//!
//! Contrato herdado do prompte (`api/src/hash.js`): serialização com chaves
//! ordenadas em todos os níveis, sem espaços, e sha256 em hex minúsculo.
//! O mesmo algoritmo existe em Python (`forge_promptforge.hashing`) e a
//! paridade é garantida pelas fixtures em `platform/schemas/fixtures/`.
//!
//! Restrição v1: números de ponto flutuante com parte fracionária zero
//! (ex.: `1.0`) são proibidos nas entradas — JS os serializa como `1`,
//! Rust/Python como `1.0`. Use inteiros ou decimais não inteiros.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Serializa um `Value` em JSON canônico: chaves ordenadas, sem espaços.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("string serializa"));
                out.push(':');
                write_canonical(&map[*key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&serde_json::to_string(scalar).expect("escalar serializa")),
    }
}

/// sha256 do texto, em hex minúsculo.
pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash de request de LLM para cache: sha256 do JSON canônico de
/// `{"messages": ..., "temperature": ...}` — idêntico nos dois lados da
/// fronteira Rust×Python.
pub fn request_hash(messages: &Value, temperature: &Value) -> String {
    let envelope = serde_json::json!({
        "messages": messages,
        "temperature": temperature,
    });
    sha256_hex(&canonical_json(&envelope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordena_chaves_em_todos_os_niveis() {
        let value = json!({"b": {"z": 1, "a": [true, null]}, "a": "x"});
        assert_eq!(
            canonical_json(&value),
            r#"{"a":"x","b":{"a":[true,null],"z":1}}"#
        );
    }

    #[test]
    fn escalares_seguem_json_compacto() {
        assert_eq!(canonical_json(&json!(null)), "null");
        assert_eq!(canonical_json(&json!(0.7)), "0.7");
        assert_eq!(canonical_json(&json!("olá")), "\"olá\"");
    }

    #[test]
    fn sha256_conhecido() {
        // echo -n 'abc' | sha256sum
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
