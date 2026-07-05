# ADR 0001 — Núcleo Rust + Sidecar Python via gRPC/UDS

- Status: aceita
- Data: 2026-07-05

## Contexto

A plataforma Forge unifica três projetos: o coding agent de terminal
(opencode, TypeScript), a camada de prompts (prompte, JS/Node) e o squad
multi-agente (BuildToValue, Python). A decisão de produto é um CLI/TUI
construído por design em Rust e Python.

## Decisão

1. **Divisão de linguagens pela regra de fronteira**: código que toca
   disco/rede/processo/segredo ou roda a cada keystroke fica em **Rust**
   (CLI/TUI, runtime de sessão, gateway LLM, ferramentas, permissões,
   verificação, storage). Código que decide "o que fazer a seguir" por
   raciocínio de agente fica em **Python** (squad, PromptForge, review,
   avaliação).

2. **Integração por gRPC bidirecional sobre Unix Domain Socket**, nunca
   embedding (PyO3) no caminho principal:
   - evita a convivência de dois runtimes async (tokio × asyncio) e o GIL
     no mesmo processo;
   - isola falhas: crash do sidecar aciona o fallback progressivo
     (agente-único → safe-mode read-only) em vez de derrubar o CLI;
   - streaming nativo para eventos do squad (propostas, votos, handoffs);
   - contratos explícitos em `.proto` com `buf breaking` no CI.

3. **API keys só no processo Rust** (princípio do proxy do prompte): o
   sidecar Python nunca fala com provedores LLM — usa
   `CoreService.Generate`.

4. **Contratos versionados com fonte única em `platform/schemas/`**:
   protobuf para o wire, JSON Schema (`*.v1.schema.json`) para documentos
   persistidos/auditáveis, golden fixtures para paridade cross-language
   (ex.: hash de cache de prompt). Mudança breaking = novo `.v2` + ADR.

## Consequências

- PyO3/maturin fica documentado como otimização futura apenas para funções
  puras, se medição justificar.
- O CLI precisa supervisionar o ciclo de vida do sidecar (spawn,
  health-check, restart) — implementado na Fase 4.
- Todo dado que cruza a fronteira precisa de schema; nada de dicts ad hoc.
