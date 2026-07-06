# ADR 0012 — Política de confiança MCP: servidor declarado, permissão por chamada

- Status: aceita
- Data: 2026-07-06

## Contexto

A Onda 4 adiciona um cliente MCP (`crates/forge-tools/src/mcp.rs`, via `rmcp`,
transporte child-process/stdio): conecta a servidores MCP externos, lista as tools
que eles anunciam e as expõe no `ToolRegistry`. A pergunta de contrato: tools MCP
passam por algum **vetting**, como as skills de terceiro (ADR 0011)? São categorias
diferentes de risco — uma skill é código que a plataforma executa; um servidor MCP
é um processo separado que o usuário já escolheu rodar.

## Decisão — confiança no servidor declarado; cada chamada sob o permission-engine

O servidor MCP é declarado pelo usuário em `.forge/mcp.toml` — isso **é** a
confiança explícita. Não há vetting estilo-skill do servidor (não executamos o
código dele; falamos um protocolo com um processo que o usuário mandou subir). Em
vez disso, **cada chamada** de uma tool MCP passa pelo motor de permissões: os
nomes são namespaced (`mcp__<server>__<tool>`) e não batem em nenhuma regra padrão
→ caem no default `Ask` → o usuário aprova ou nega **por chamada**, e o ledger
registra. Uma tool MCP é uma tool como qualquer outra: pede permissão, entra no
ledger, é auto-gated.

Escolhas de implementação que decorrem disso:

- **Namespacing + guarda de colisão:** uma tool MCP não sombreia built-in/skill;
  registrar o mesmo servidor 2× não duplica.
- **Fail-soft:** `.forge/mcp.toml` ausente/inválido, ou um servidor que não sobe →
  loga e segue (um MCP quebrado não derruba o CLI). Contraste deliberado com o
  fail-**closed** das skills de terceiro (ADR 0011): lá a plataforma executaria
  código alheio, então a ausência de sandbox tem que barrar; aqui é um processo
  externo que o usuário controla, então degradar graciosamente é o correto.
- **Conexão por chamada** (connect→call→encerra): simples, sem estado
  compartilhado. Sessão persistente é otimização futura (registrada em pendencias).

## O que foi provado, não só declarado

- Teste de integração cross-process real: sobe um servidor MCP fixture como
  processo separado, o registry lista suas tools (namespaced), uma chamada real
  atravessa o processo e volta (`ECHO:mundo`). Não há mock do protocolo.
- Registro do mesmo servidor 2× não duplica; a tool namespaced não sombreia `bash`.

## Consequências

- O motor de permissões (core Rust, não-contornável) é a única linha de defesa das
  chamadas MCP — coerente com a tese de segurança da plataforma. Não há uma segunda
  máquina de vetting a manter para uma categoria que não a pede.
- Regras de permissão por servidor/tool são possíveis (o `scope` devolve
  `mcp:<server>/<tool> <preview>`), mas não vêm ligadas por padrão.
- O frontend (`MCP_SERVERS`) e o wiring `/api/mcp` ficam para depois; a decisão de
  contrato (confiança) está fechada independentemente da UI.
