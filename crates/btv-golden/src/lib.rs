//! Golden tests dos fluxos HTTP críticos (Trilha T1 do plano DDD multitenant).
//!
//! Este crate existe SÓ como dev-dependency: `btv-server` e `btv-cli` precisam
//! do mesmo comparador de contrato e a direção de dependência entre eles
//! (`btv-cli → btv-server`) impede reuso de um módulo de teste interno.
//!
//! O contrato de um fluxo é gravado como fixture em
//! `schemas/fixtures/http/<fluxo>.golden.json` a partir da resposta REAL do
//! servidor (nunca escrito à mão) e comparado por igualdade profunda de JSON:
//! campo novo, campo removido, renomeação, mudança de tipo, status ou
//! content-type diferente ⇒ o teste falha. Campos voláteis (ids de tarefa,
//! paths de tempdir) são declarados explicitamente: presença + tipo + não-vazio
//! continuam checados, só o VALOR é substituído por placeholder.
//!
//! Regravação (mudança de contrato consciente, revisada no diff da fixture):
//!
//! ```sh
//! BTV_UPDATE_GOLDEN=1 cargo test -p btv-server -p btv-cli golden
//! ```
//!
//! A regravação é proibida no CI (`CI` setada junto com `BTV_UPDATE_GOLDEN`
//! aborta) — impossível "regravar para passar" no gate.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

/// Tipo esperado de um campo volátil — o valor é livre, o tipo não.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Str,
    Num,
}

/// Campo volátil do corpo de resposta: caminho estilo pointer com curinga
/// (`/task_id`, `/*/ts`, `/steps/*/entry_hash`). `*` percorre todos os
/// elementos de array (ou valores de objeto) naquele nível.
#[derive(Debug, Clone, Copy)]
pub struct Volatile {
    pub path: &'static str,
    pub kind: Kind,
}

/// Volátil string (não-vazia).
pub fn vstr(path: &'static str) -> Volatile {
    Volatile {
        path,
        kind: Kind::Str,
    }
}

/// Volátil numérico.
pub fn vnum(path: &'static str) -> Volatile {
    Volatile {
        path,
        kind: Kind::Num,
    }
}

#[derive(Debug, Serialize)]
pub struct GoldenRequest {
    pub method: String,
    /// Caminho como deve ficar GRAVADO (o chamador substitui ids reais por
    /// placeholder quando o caminho contém um id volátil).
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct GoldenResponse {
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_disposition: Option<String>,
    pub body: Value,
}

#[derive(Debug, Serialize)]
pub struct GoldenStep {
    pub name: String,
    pub request: GoldenRequest,
    pub response: GoldenResponse,
}

/// Monta um passo do fluxo normalizando os campos voláteis do corpo capturado.
/// Falha (panic, é código de teste) se um campo volátil estiver ausente, com
/// tipo errado ou string vazia — volátil não é campo opcional.
#[allow(clippy::too_many_arguments)]
pub fn step(
    name: &str,
    method: &str,
    path: &str,
    request_body: Option<Value>,
    status: u16,
    content_type: Option<String>,
    content_disposition: Option<String>,
    mut body: Value,
    volatiles: &[Volatile],
) -> GoldenStep {
    for v in volatiles {
        if let Err(e) = apply_volatile(&mut body, v) {
            panic!("passo '{name}': campo volátil inválido — {e}");
        }
    }
    GoldenStep {
        name: name.to_string(),
        request: GoldenRequest {
            method: method.to_string(),
            path: path.to_string(),
            body: request_body,
        },
        response: GoldenResponse {
            status,
            content_type,
            content_disposition,
            body,
        },
    }
}

/// Compara o fluxo capturado com a fixture gravada (ou regrava sob
/// `BTV_UPDATE_GOLDEN=1`, nunca no CI). Qualquer diferença estrutural ou de
/// valor não-volátil é mudança de contrato ⇒ panic com o primeiro ponto de
/// divergência.
pub fn check(flow: &str, steps: Vec<GoldenStep>) {
    let captured = serde_json::json!({ "flow": flow, "steps": steps });
    let path = fixture_path(flow);

    if std::env::var_os("BTV_UPDATE_GOLDEN").is_some() {
        assert!(
            std::env::var_os("CI").is_none(),
            "BTV_UPDATE_GOLDEN é proibido no CI — regravar a fixture no gate \
             anularia o golden test (regrave localmente e revise o diff)"
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("criar schemas/fixtures/http");
        }
        let pretty = serde_json::to_string_pretty(&captured).expect("serializar fixture");
        std::fs::write(&path, format!("{pretty}\n")).expect("gravar fixture");
        eprintln!("golden: fixture regravada em {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "golden: fixture ausente/ilegível em {} ({e}) — grave-a com \
             BTV_UPDATE_GOLDEN=1 cargo test e commite o arquivo",
            path.display()
        )
    });
    let expected: Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("fixture corrompida: {e}"));

    if expected != captured {
        let at =
            first_diff(&expected, &captured, String::new()).unwrap_or_else(|| "(raiz)".to_string());
        panic!(
            "golden '{flow}': contrato divergiu da fixture em `{at}`.\n\
             Se a mudança é intencional, regrave com BTV_UPDATE_GOLDEN=1 e \
             revise o diff de {}.\n\n--- esperado (fixture) ---\n{}\n\n--- capturado ---\n{}",
            path.display(),
            serde_json::to_string_pretty(&expected).unwrap_or_default(),
            serde_json::to_string_pretty(&captured).unwrap_or_default(),
        );
    }
}

/// `schemas/fixtures/http/<fluxo>.golden.json`, resolvido a partir deste
/// crate (`crates/btv-golden` → raiz do repo).
fn fixture_path(flow: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/http")
        .join(format!("{flow}.golden.json"))
}

/// Substitui o valor volátil por placeholder, exigindo presença, tipo certo e
/// (para strings) não-vazio em TODOS os pontos alcançados pelo caminho.
fn apply_volatile(value: &mut Value, v: &Volatile) -> Result<(), String> {
    let segs: Vec<&str> = v.path.trim_start_matches('/').split('/').collect();
    if segs.is_empty() || segs[0].is_empty() {
        return Err(format!("caminho volátil vazio: {:?}", v.path));
    }
    walk(value, &segs, v)
}

fn walk(value: &mut Value, segs: &[&str], v: &Volatile) -> Result<(), String> {
    let (seg, rest) = (segs[0], &segs[1..]);
    if rest.is_empty() {
        return replace_leaf(value, seg, v);
    }
    match (seg, value) {
        ("*", Value::Array(items)) => {
            for item in items {
                walk(item, rest, v)?;
            }
            Ok(())
        }
        ("*", Value::Object(map)) => {
            for (_, item) in map.iter_mut() {
                walk(item, rest, v)?;
            }
            Ok(())
        }
        ("*", other) => Err(format!(
            "`{}`: `*` exige array/objeto, encontrou {other}",
            v.path
        )),
        (key, Value::Object(map)) => match map.get_mut(key) {
            Some(inner) => walk(inner, rest, v),
            None => Err(format!("`{}`: chave `{key}` ausente", v.path)),
        },
        (key, other) => Err(format!(
            "`{}`: esperava objeto com `{key}`, encontrou {other}",
            v.path
        )),
    }
}

fn replace_leaf(value: &mut Value, seg: &str, v: &Volatile) -> Result<(), String> {
    match (seg, value) {
        ("*", Value::Array(items)) => {
            for item in items {
                check_and_replace(item, v)?;
            }
            Ok(())
        }
        ("*", Value::Object(map)) => {
            for (_, item) in map.iter_mut() {
                check_and_replace(item, v)?;
            }
            Ok(())
        }
        ("*", other) => Err(format!(
            "`{}`: `*` final exige array/objeto, encontrou {other}",
            v.path
        )),
        (key, Value::Object(map)) => match map.get_mut(key) {
            Some(leaf) => check_and_replace(leaf, v),
            None => Err(format!("`{}`: campo `{key}` ausente", v.path)),
        },
        (key, other) => Err(format!(
            "`{}`: esperava objeto com `{key}`, encontrou {other}",
            v.path
        )),
    }
}

fn check_and_replace(leaf: &mut Value, v: &Volatile) -> Result<(), String> {
    match v.kind {
        Kind::Str => match leaf {
            Value::String(s) if !s.is_empty() => {
                *leaf = Value::String("<volatil>".into());
                Ok(())
            }
            Value::String(_) => Err(format!("`{}`: string vazia", v.path)),
            other => Err(format!("`{}`: esperava string, encontrou {other}", v.path)),
        },
        Kind::Num => match leaf {
            Value::Number(_) => {
                *leaf = Value::Number((-1).into());
                Ok(())
            }
            other => Err(format!("`{}`: esperava número, encontrou {other}", v.path)),
        },
    }
}

/// Primeiro caminho onde `a` e `b` divergem — só para a mensagem de erro.
fn first_diff(a: &Value, b: &Value, at: String) -> Option<String> {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                match mb.get(k) {
                    Some(vb) => {
                        if let Some(p) = first_diff(va, vb, format!("{at}/{k}")) {
                            return Some(p);
                        }
                    }
                    None => return Some(format!("{at}/{k} (removido)")),
                }
            }
            for k in mb.keys() {
                if !ma.contains_key(k) {
                    return Some(format!("{at}/{k} (novo)"));
                }
            }
            None
        }
        (Value::Array(xa), Value::Array(xb)) => {
            for (i, (va, vb)) in xa.iter().zip(xb.iter()).enumerate() {
                if let Some(p) = first_diff(va, vb, format!("{at}/{i}")) {
                    return Some(p);
                }
            }
            (xa.len() != xb.len()).then(|| format!("{at} (tamanhos {} vs {})", xa.len(), xb.len()))
        }
        _ => (a != b).then_some(at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn volatil_substitui_mas_exige_presenca_tipo_e_nao_vazio() {
        let mut v = json!({"task_id": "sq7", "run_id": 3});
        apply_volatile(&mut v, &vstr("/task_id")).unwrap();
        apply_volatile(&mut v, &vnum("/run_id")).unwrap();
        assert_eq!(v, json!({"task_id": "<volatil>", "run_id": -1}));

        // Ausente ⇒ erro (volátil não é opcional).
        let mut sem = json!({"outro": 1});
        assert!(apply_volatile(&mut sem, &vstr("/task_id")).is_err());
        // Tipo errado ⇒ erro.
        let mut errado = json!({"task_id": 42});
        assert!(apply_volatile(&mut errado, &vstr("/task_id")).is_err());
        // String vazia ⇒ erro.
        let mut vazia = json!({"task_id": ""});
        assert!(apply_volatile(&mut vazia, &vstr("/task_id")).is_err());
    }

    #[test]
    fn curinga_percorre_arrays_e_falha_se_um_elemento_nao_tem_o_campo() {
        let mut ok = json!([{"ts": "t1", "x": 1}, {"ts": "t2", "x": 2}]);
        apply_volatile(&mut ok, &vstr("/*/ts")).unwrap();
        assert_eq!(ok[0]["ts"], "<volatil>");
        assert_eq!(ok[1]["ts"], "<volatil>");
        assert_eq!(ok[1]["x"], 2, "campo não-volátil fica intacto");

        let mut faltando = json!([{"ts": "t1"}, {"sem_ts": true}]);
        assert!(apply_volatile(&mut faltando, &vstr("/*/ts")).is_err());
    }

    #[test]
    fn first_diff_aponta_campo_novo_removido_e_valor_diferente() {
        let a = json!({"x": 1, "lista": [{"k": "a"}]});
        assert_eq!(
            first_diff(&a, &json!({"x": 2, "lista": [{"k": "a"}]}), String::new()),
            Some("/x".into())
        );
        assert_eq!(
            first_diff(&a, &json!({"lista": [{"k": "a"}]}), String::new()),
            Some("/x (removido)".into())
        );
        assert_eq!(
            first_diff(
                &a,
                &json!({"x": 1, "lista": [{"k": "a"}], "novo": true}),
                String::new()
            ),
            Some("/novo (novo)".into())
        );
        assert_eq!(
            first_diff(&a, &json!({"x": 1, "lista": [{"k": "b"}]}), String::new()),
            Some("/lista/0/k".into())
        );
        assert_eq!(first_diff(&a, &a.clone(), String::new()), None);
    }
}
