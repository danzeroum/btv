//! Teste de integração do cliente LSP (Fase 6 Onda 5), em duas camadas:
//!
//! 1. **Hermético** (`lsp_fixture_definicao_via_registry`): sobe o servidor
//!    fixture (`forge_lsp_fixture`) como processo separado, registra as tools no
//!    `ToolRegistry` e chama `lsp__fixture__definition` — o cliente atravessa o
//!    processo, extrai a Location conhecida (linha 3, char 7) e a devolve por
//!    igualdade. Prova o framing/handshake/ida-e-volta do cliente **em qualquer
//!    lugar**, sem depender do rust-analyzer instalado. SEMPRE roda.
//!
//! 2. **Real** (`lsp_rust_analyzer_real_*`, `#[ignore]`): dirige o
//!    rust-analyzer DE VERDADE contra um fixture cargo e assere a definição
//!    **semântica** de um símbolo por igualdade com a posição conhecida — a
//!    fronteira da onda. Só roda com `--include-ignored` (o job de CI instala a
//!    componente); se pedido para rodar e o rust-analyzer não estiver lá, FALHA
//!    (guarda de honestidade — nunca verde sem ter rodado).

use forge_tools::lsp::{register_lsp_server, LspServerConfig};
use forge_tools::ToolRegistry;

fn fixture_server_config(root: &std::path::Path) -> LspServerConfig {
    LspServerConfig {
        id: "fixture".to_string(),
        command: env!("CARGO_BIN_EXE_forge_lsp_fixture").to_string(),
        args: vec![],
        root: root.to_path_buf(),
    }
}

#[test]
fn lsp_fixture_definicao_via_registry() {
    let dir = tempfile::tempdir().unwrap();
    // O cliente lê o arquivo do disco para o didOpen — precisa existir.
    let file = dir.path().join("alvo.txt");
    std::fs::write(&file, "linha0\nlinha1\nlinha2\nabc def ghi\n").unwrap();

    let mut registry = ToolRegistry::default_set(dir.path());
    let n = register_lsp_server(&mut registry, &fixture_server_config(dir.path()));
    assert_eq!(
        n, 3,
        "esperava as 3 tools LSP (definition/references/diagnostics)"
    );

    // Namespaced — não sombreia built-ins.
    assert!(registry.get("bash").is_some());
    let tool = registry
        .get("lsp__fixture__definition")
        .expect("a tool de definição do fixture deve estar registrada");

    let out = tool
        .run(&serde_json::json!({ "file": "alvo.txt", "line": 3, "character": 4 }))
        .expect("a consulta LSP deve retornar");
    // O fixture devolve a Location conhecida: linha 3, char 7.
    assert!(
        out.content.contains(":3:7"),
        "esperava a definição em :3:7 (a Location do fixture); veio: {}",
        out.content
    );
    assert!(
        out.content.contains("alvo.txt"),
        "esperava o caminho do arquivo; veio: {}",
        out.content
    );
}

#[test]
fn lsp_registro_duplicado_nao_duplica() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = ToolRegistry::default_set(dir.path());
    register_lsp_server(&mut registry, &fixture_server_config(dir.path()));
    // Registrar o mesmo server de novo: colisão de nomes → nada registrado.
    let n2 = register_lsp_server(&mut registry, &fixture_server_config(dir.path()));
    assert_eq!(n2, 0, "o segundo registro do mesmo server não duplica");
}

// --- Camada 2: rust-analyzer REAL ---

fn rust_analyzer_disponivel() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).starts_with("rust-analyzer")
        })
        .unwrap_or(false)
}

/// Cria um fixture cargo real e devolve sua raiz (num tempdir que o chamador
/// mantém vivo).
fn escreve_fixture_cargo(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    // `alvo` é definido na linha 0, char 7 (após "pub fn "); usado na linha 5.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn alvo(x: i32) -> i32 {\n    x + 1\n}\n\npub fn usa() -> i32 {\n    alvo(41)\n}\n\npub mod ruim;\n",
    )
    .unwrap();
    // Arquivo com erro de sintaxe (reportado nativamente pelo rust-analyzer,
    // sem depender de cargo check) — para a consulta de diagnósticos.
    std::fs::write(
        root.join("src/ruim.rs"),
        "pub fn ruim() -> i32 {\n    let x = ;\n    0\n}\n",
    )
    .unwrap();
}

fn config_rust(root: &std::path::Path) -> LspServerConfig {
    LspServerConfig {
        id: "rust".to_string(),
        command: "rust-analyzer".to_string(),
        args: vec![],
        root: root.to_path_buf(),
    }
}

#[test]
#[ignore = "requer rust-analyzer (roda no job de CI que instala a componente)"]
fn lsp_rust_analyzer_real_definicao_por_igualdade() {
    assert!(
        rust_analyzer_disponivel(),
        "GUARDA: este teste foi pedido (--include-ignored) mas rust-analyzer não \
         está disponível — instale a componente. Nunca marcar verde sem rodar."
    );
    let dir = tempfile::tempdir().unwrap();
    escreve_fixture_cargo(dir.path());
    let mut registry = ToolRegistry::default_set(dir.path());
    let n = register_lsp_server(&mut registry, &config_rust(dir.path()));
    assert_eq!(n, 3);

    let def = registry.get("lsp__rust__definition").unwrap();
    // Posição do uso de `alvo` no call-site (linha 5, char 4, 0-indexed).
    let out = def
        .run(&serde_json::json!({ "file": "src/lib.rs", "line": 5, "character": 4 }))
        .expect("definition deve retornar");
    // A definição semântica de `alvo`: linha 0, char 7 — derivada pelo
    // rust-analyzer do código real, não fabricada. Igualdade com a conhecida.
    assert!(
        out.content.contains("lib.rs:0:7"),
        "esperava a definição real de `alvo` em lib.rs:0:7; veio: {}",
        out.content
    );
}

#[test]
#[ignore = "requer rust-analyzer (roda no job de CI que instala a componente)"]
fn lsp_rust_analyzer_real_referencias_e_diagnosticos() {
    assert!(
        rust_analyzer_disponivel(),
        "GUARDA: --include-ignored pedido mas rust-analyzer ausente."
    );
    let dir = tempfile::tempdir().unwrap();
    escreve_fixture_cargo(dir.path());
    let mut registry = ToolRegistry::default_set(dir.path());
    register_lsp_server(&mut registry, &config_rust(dir.path()));

    // Referências de `alvo` (posição da declaração, linha 0 char 7): inclui o
    // uso no call-site (linha 5).
    let refs = registry.get("lsp__rust__references").unwrap();
    let out = refs
        .run(&serde_json::json!({ "file": "src/lib.rs", "line": 0, "character": 7 }))
        .expect("references deve retornar");
    assert!(
        out.content.contains("lib.rs:5:"),
        "esperava o uso de `alvo` na linha 5 entre as referências; veio: {}",
        out.content
    );

    // Diagnósticos do arquivo com erro de sintaxe: pelo menos um erro.
    let diag = registry.get("lsp__rust__diagnostics").unwrap();
    let out = diag
        .run(&serde_json::json!({ "file": "src/ruim.rs" }))
        .expect("diagnostics deve retornar");
    assert!(
        out.content.contains("error"),
        "esperava um erro de sintaxe em ruim.rs; veio: {}",
        out.content
    );
}
