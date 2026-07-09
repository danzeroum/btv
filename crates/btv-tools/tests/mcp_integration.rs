//! Teste de integração cross-process do cliente MCP (Fase 6 Onda 4): sobe o
//! servidor fixture (`btv_mcp_fixture`, compilado pelo cargo) como **processo
//! separado**, lista suas tools no `ToolRegistry` (namespaced) e faz uma chamada
//! real ida-e-volta — a fronteira literal da onda. Auto-contido, roda em
//! qualquer lugar (não depende de node/npx nem de servidor externo).

use btv_tools::mcp::{register_mcp_server, McpServerConfig};
use btv_tools::ToolRegistry;

fn fixture_config() -> McpServerConfig {
    McpServerConfig {
        id: "fixture".to_string(),
        command: env!("CARGO_BIN_EXE_btv_mcp_fixture").to_string(),
        args: vec![],
    }
}

#[test]
fn mcp_server_fixture_lista_e_chama_via_registry() {
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    let n = register_mcp_server(&mut registry, &fixture_config()).expect("registra o fixture");
    assert!(n >= 1, "esperava >=1 tool do fixture, veio {n}");

    let tool = registry
        .get("mcp__fixture__echo")
        .expect("a tool echo do fixture deve estar registrada, namespaced");

    // A chamada real atravessa o processo MCP separado e volta.
    let out = tool
        .run(&serde_json::json!({"input": "mundo"}))
        .expect("a chamada MCP deve retornar");
    assert!(
        out.content.contains("ECHO:mundo"),
        "o echo do fixture deveria voltar; veio: {}",
        out.content
    );
}

#[test]
fn mcp_nomes_namespaced_nao_colidem() {
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    register_mcp_server(&mut registry, &fixture_config()).unwrap();
    // o nome namespaced (mcp__fixture__echo) não sombreia um built-in
    assert!(registry.get("bash").is_some());
    // registrar o mesmo servidor de novo: a colisão é pulada (não duplica)
    let n2 = register_mcp_server(&mut registry, &fixture_config()).unwrap();
    assert_eq!(n2, 0, "segundo registro do mesmo servidor não duplica");
}

/// Sessão persistente: a MESMA tool registrada é chamada várias vezes e a
/// conexão é reusada (não reconecta a cada chamada). Se a sessão não fosse
/// persistente, o processo teria sido encerrado após a 1ª chamada e a 2ª
/// falharia; aqui as três voltam corretas pela conexão viva.
#[test]
fn mcp_sessao_persistente_reusa_conexao_entre_chamadas() {
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    register_mcp_server(&mut registry, &fixture_config()).unwrap();
    let tool = registry.get("mcp__fixture__echo").unwrap();
    for palavra in ["um", "dois", "tres"] {
        let out = tool.run(&serde_json::json!({ "input": palavra })).unwrap();
        assert!(
            out.content.contains(&format!("ECHO:{palavra}")),
            "chamada reusando a conexão persistente deveria voltar; veio: {}",
            out.content
        );
    }
}

/// Um servidor cujo comando não existe falha ao conectar (a thread da sessão
/// NÃO fica pendurada) — a operação é bounded e devolve erro. Prova o fim do
/// "thread leak": mesmo o caminho de falha termina.
#[test]
fn mcp_servidor_inexistente_falha_sem_pendurar() {
    let config = McpServerConfig {
        id: "morto".to_string(),
        command: "/caminho/inexistente/btv-mcp-xyz".to_string(),
        args: vec![],
    };
    let mut registry = ToolRegistry::default_set(std::path::Path::new("."));
    let res = register_mcp_server(&mut registry, &config);
    assert!(
        res.is_err(),
        "servidor inexistente deve falhar, não pendurar"
    );
}
