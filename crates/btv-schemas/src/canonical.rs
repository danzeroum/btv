//! Canonicalização JSON e hash de cache de prompt (`prompt-cache-key.v1`).
//!
//! Contrato herdado do prompte (`api/src/hash.js`): serialização com chaves
//! ordenadas em todos os níveis, sem espaços, e sha256 em hex minúsculo.
//! O mesmo algoritmo existe em Python (`btv_promptforge.hashing`) e a
//! paridade é garantida pelas fixtures em `platform/schemas/fixtures/`.
//!
//! Restrição v1 (ADR 0032): números de ponto flutuante com parte fracionária
//! zero (ex.: `1.0`) são proibidos nas entradas — JS os serializa como `1`,
//! Rust/Python como `1.0`. Antes a regra era só prosa; agora `request_hash`
//! a ENFORÇA (rejeita), com validador espelhado no Python
//! (`btv_promptforge.hashing`) e os `reject_cases` das fixtures como juízes.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Entrada recusada pelo contrato `prompt-cache-key.v1`: contém um número que
/// divergiria entre produtores (JS × Rust/Python). Ver ADR 0032.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CacheKeyError {
    #[error("número proibido no prompt-cache-key.v1 em {path}: {motivo}")]
    NumeroProibido { path: String, motivo: String },
}

/// Rejeita números que não sobrevivem à fronteira de produtores no v1: floats
/// com parte fracionária zero (`1.0` → JS emite `1`, Rust/Python `1.0`) e
/// não-finitos (NaN/Inf — nem são JSON válido). Recursivo por todo o valor.
fn reject_forbidden_numbers(value: &Value, path: &str) -> Result<(), CacheKeyError> {
    match value {
        Value::Number(n) if n.is_f64() => {
            let f = n.as_f64().expect("is_f64 implica as_f64");
            if !f.is_finite() {
                return Err(CacheKeyError::NumeroProibido {
                    path: path.to_string(),
                    motivo: format!("número não-finito ({f})"),
                });
            }
            if f.fract() == 0.0 {
                return Err(CacheKeyError::NumeroProibido {
                    path: path.to_string(),
                    motivo: format!("float com fração zero ({f}); use o inteiro {}", f as i64),
                });
            }
            Ok(())
        }
        Value::Object(map) => map
            .iter()
            .try_for_each(|(k, v)| reject_forbidden_numbers(v, &format!("{path}.{k}"))),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .try_for_each(|(i, v)| reject_forbidden_numbers(v, &format!("{path}[{i}]"))),
        _ => Ok(()),
    }
}

/// Valida as entradas do `prompt-cache-key.v1` (o mesmo guard que `request_hash`
/// aplica). Público para validar antes de montar o request, se desejado.
pub fn validate_cache_key(messages: &Value, temperature: &Value) -> Result<(), CacheKeyError> {
    reject_forbidden_numbers(messages, "$.messages")?;
    reject_forbidden_numbers(temperature, "$.temperature")
}

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
pub fn request_hash(messages: &Value, temperature: &Value) -> Result<String, CacheKeyError> {
    validate_cache_key(messages, temperature)?;
    let envelope = serde_json::json!({
        "messages": messages,
        "temperature": temperature,
    });
    Ok(sha256_hex(&canonical_json(&envelope)))
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

    #[test]
    fn request_hash_aceita_entradas_validas() {
        // Inteiros, floats não inteiros, bool/null passam.
        assert!(request_hash(&json!([{"role": "user", "content": "oi"}]), &json!(0.7)).is_ok());
        assert!(request_hash(&json!([{"n": 42, "flag": true}]), &json!(1)).is_ok());
        assert!(request_hash(&json!([]), &Value::Null).is_ok());
    }

    #[test]
    fn request_hash_rejeita_float_com_fracao_zero() {
        // 1.0/0.0 divergem entre produtores (JS "1"/"0", Rust/Python "1.0"/"0.0").
        assert!(request_hash(&json!([{"role": "user", "content": "oi"}]), &json!(1.0)).is_err());
        assert!(request_hash(&json!([{"role": "user", "content": "oi"}]), &json!(0.0)).is_err());
        // Também aninhado dentro de messages.
        assert!(request_hash(&json!([{"n": 3.0}]), &json!(0.5)).is_err());
    }

    #[test]
    fn serde_json_nao_carrega_nao_finito() {
        // Justifica por que os reject_cases de NaN/Inf ficam só no lado Python:
        // serde_json recusa construir um Number não-finito (from_f64 → None),
        // então um Value nunca carrega Inf/NaN e o guard correspondente em Rust
        // é defensivo. O Python (onde float('inf') é um float real) o exercita.
        assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
        assert!(serde_json::Number::from_f64(f64::NAN).is_none());
    }
}
