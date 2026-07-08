//! Teste de integração cross-process real do comando `btv verify`: sobe o
//! binário compilado de verdade (`Command::new(env!("CARGO_BIN_EXE_btv"))`,
//! não uma chamada de função in-process) contra um workspace fixture com
//! `current_dir` explícito — nunca o diretório do próprio repo, senão o
//! teste mediria o BuildToValue real por acidente.
//!
//! O requisito crítico da Onda 2: exit code ≠ 0 quando o veredito é Fail —
//! sem isso o gate de CI da Onda 6 não teria como reprovar PR.

use std::path::Path;
use std::process::Command;

fn btv_bin() -> &'static str {
    env!("CARGO_BIN_EXE_btv")
}

fn write_btv_toml(dir: &Path, contents: &str) {
    std::fs::write(dir.join("btv.toml"), contents).unwrap();
}

fn only_evidence_file(dir: &Path) -> std::path::PathBuf {
    let evidence_dir = dir.join(".btv").join("evidence");
    let mut entries: Vec<_> = std::fs::read_dir(&evidence_dir)
        .unwrap_or_else(|e| panic!("esperava {evidence_dir:?} existir: {e}"))
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "deveria ter gravado exatamente um artefato de evidência"
    );
    entries.remove(0)
}

#[test]
fn passo_que_falha_sai_com_codigo_diferente_de_zero_e_grava_veredito_fail() {
    let dir = tempfile::tempdir().unwrap();
    write_btv_toml(
        dir.path(),
        r#"
[[step]]
name = "quebra"
program = "false"
args = []
"#,
    );

    let output = Command::new(btv_bin())
        .arg("verify")
        .current_dir(dir.path())
        .output()
        .expect("btv verify deveria rodar");

    assert_ne!(
        output.status.code(),
        Some(0),
        "veredito Fail deve sair com código != 0 — stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(only_evidence_file(dir.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["verdict"], "fail");
}

#[test]
fn passo_que_passa_sai_com_codigo_zero_e_grava_veredito_pass() {
    let dir = tempfile::tempdir().unwrap();
    write_btv_toml(
        dir.path(),
        r#"
[[step]]
name = "ok"
program = "true"
args = []
"#,
    );

    let output = Command::new(btv_bin())
        .arg("verify")
        .current_dir(dir.path())
        .output()
        .expect("btv verify deveria rodar");

    assert_eq!(output.status.code(), Some(0));

    let raw = std::fs::read_to_string(only_evidence_file(dir.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["verdict"], "pass");
}

#[test]
fn artefato_gravado_valida_contra_o_schema() {
    let dir = tempfile::tempdir().unwrap();
    write_btv_toml(
        dir.path(),
        r#"
[[step]]
name = "ok"
program = "true"
args = []
"#,
    );

    Command::new(btv_bin())
        .arg("verify")
        .current_dir(dir.path())
        .output()
        .unwrap();

    let raw = std::fs::read_to_string(only_evidence_file(dir.path())).unwrap();
    let instance: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let schema_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/json/verification-evidence.v1.schema.json"
    );
    let schema_raw = std::fs::read_to_string(schema_path).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&schema_raw).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator.iter_errors(&instance).collect();
    assert!(
        errors.is_empty(),
        "artefato real não bateu o schema: {errors:?}"
    );
}

#[test]
fn respeita_flags_out_e_config_custom() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("custom.toml");
    std::fs::write(
        &config_path,
        r#"
[[step]]
name = "ok"
program = "true"
args = []
"#,
    )
    .unwrap();
    let out_path = dir.path().join("saida").join("evidencia.json");

    let output = Command::new(btv_bin())
        .args([
            "verify",
            "--config",
            config_path.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(out_path.exists(), "deveria ter gravado no --out custom");
    // sem btv.toml na raiz do fixture, o default só seria usado se
    // --config não tivesse sido respeitado — confirma que o custom.toml
    // (que tem só o passo "ok") foi realmente o usado, não o default de 3 passos.
    let raw = std::fs::read_to_string(&out_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["steps"].as_array().unwrap().len(), 1);
}

#[test]
fn format_json_imprime_evidencia_parseavel_no_stdout() {
    let dir = tempfile::tempdir().unwrap();
    write_btv_toml(
        dir.path(),
        r#"
[[step]]
name = "ok"
program = "true"
args = []
"#,
    );

    let output = Command::new(btv_bin())
        .args(["verify", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("stdout deveria ser JSON parseável em --format json");
    assert_eq!(json["verdict"], "pass");
}

#[test]
fn sem_btv_toml_cai_no_default_que_espelha_o_ci() {
    // Fixture sem btv.toml algum — deve cair no default_steps() (test+lint+fmt
    // do workspace), que aqui rodaria contra o próprio fixture vazio (sem
    // Cargo.toml) e por isso falha — o que já basta pra provar que o default
    // foi de fato usado (3 passos gravados), sem depender do resultado exato.
    let dir = tempfile::tempdir().unwrap();

    Command::new(btv_bin())
        .arg("verify")
        .current_dir(dir.path())
        .output()
        .expect("btv verify deveria rodar mesmo sem btv.toml");

    let raw = std::fs::read_to_string(only_evidence_file(dir.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        json["steps"].as_array().unwrap().len(),
        3,
        "default_steps() tem 3 passos"
    );
}
