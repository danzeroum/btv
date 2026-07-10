# ADR 0031 — a fronteira real é console/dashboard vs motor (redefinição do T4, fecho do C4)

- Status: proposta (aguardando o aceite do dono no PR de fecho do C4 — corrige
  o plano-mestre aceito no G0, então é decisão de dono, não de executor)
- Data: 2026-07-10

## Contexto

O levantamento de julho diagnosticou a `btv-cli` como "God Crate que é
simultaneamente CLI e servidor HTTP" e prescreveu, na Trilha T4, "remover axum
do CLI" — com um guarda armado desde a semana 1 (`arch-lint.sh`, checagem
"btv-cli não importa axum") que só ativaria depois do C4 mover "os 9
módulos-roteadores" para `btv-server`.

O C4 moveu os quatro consoles-folha que eram, de fato, dashboard perdido no
crate errado: `sandbox_console`, `doctor_console`, `lsp_console` (e o
`mcp_console` reordenado — ver abaixo), com os leitores de config extraídos para
o dono do tipo (`btv-tools`, C4-3) e o `git_sha` para `btv-verify` (C4-2). Mas o
recon dos três "grandes" (`squad_agent`, `btv_agent`, `web_agent`) revelou que a
premissa de julho estava errada sobre eles: **não são roteadores, são o
MOTOR do produto** — o agent-loop do navegador (`web_agent`: `build_loop`/
`open_durable`/`prepare`/`session`), o motor de squad (`squad_agent`: `SquadHub`
+ `start_squad_task`), o router de produto (`btv_agent`) — que servem HTTP
porque o produto local **é** um servidor. O axum deles não é vazamento de
camada; é a interface do motor.

Mover esses três "inteiros" para `btv-server` arrastaria o motor (agent-loop,
sidecar, sessões, permissões) para dentro do crate de dashboard — `btv-server`
passaria a depender de `btv-core`/`btv-sidecar`, e a inversão é impossível de
qualquer forma porque **`btv-server` não pode chamar `btv-cli` de volta** (a
direção de dependência é a oposta). Cumprir o T4 literal ("zero axum na
btv-cli") só seria possível traindo o objetivo que ele media.

## Decisão

**A fronteira real não é "axum vs CLI"; é "console/dashboard vs MOTOR".** Os
consoles-folha consolidam em `btv-server`; o axum que **serve o motor** fica com
o motor, na `btv-cli`. O T4 redefine-se em conformidade:

- O guarda `arch-lint.sh` **ativa hoje**, com escopo verdadeiro, como **T4-E**:
  a superfície axum da `btv-cli` está CONGELADA numa **allowlist** explícita dos
  módulos-motor + consoles que servem o motor + borda + juízes
  (`btv_agent`, `squad_agent`, `web_agent`, `mcp_console`, `memory_console`,
  `prompt_render`, `tenant_extractor`, `btv_agent_golden`,
  `tenant_border_sweep`). Um módulo axum **novo** fora da allowlist → vermelho —
  o console que devia nascer em `btv-server` nascendo no lar errado, o bug que a
  onda inteira combateu. Provado que morde (módulo-canário fora da allowlist
  reprova; removido, verde).
- Os consoles que FICAM o fazem por **acoplamento de grafo ao motor, não por
  preguiça:** `mcp_console` lê o overlay de permissão real (`load_rule_overrides`
  → `RuleStore` + `btv_core::Rule` + helpers locais do `web_agent`);
  `memory_console`/`prompt_render` **dirigem os sidecars Python**
  (`default_memory_service`/`default_sidecar_service` via `locate_python_dir`).
  São consoles que orquestram o motor, não dashboard puro.

## Não-escopo explícito

- **A decomposição dos três grandes** (extrair o motor para crate(s) abaixo que
  `btv-server` possa depender, depois routers finos em cima) NÃO entra nesta
  campanha — é redesenho de lógica, não movimento de endereço, e fazê-lo no
  último quilômetro inverteria a relação risco/portão que trouxe a campanha a
  zero regressão. Fica registrado em `pendencias.md` como projeto próprio
  pós-campanha, com gatilho: quando o motor precisar de um SEGUNDO consumidor
  (ex.: modo saas num processo separado do CLI), a extração paga a si mesma;
  até lá, é custo sem comprador. Se o dono quiser, é uma campanha com seu
  próprio G0.
- Nenhuma mudança de wire, schema ou comportamento — o T4-E é guarda estático;
  os goldens seguem byte-idênticos.

## Consequências

- O guarda da semana 1 recebe seu dia com escopo **verdadeiro** em vez de nunca
  ativar com escopo falso. "Nada Fake" vale para metas também: cumprir a métrica
  arrastando o motor para o dashboard seria atingir o número traindo o objetivo;
  redefini-la à luz do terreno real, com ADR e guarda ativo, é o oposto de
  rebaixar a régua. A regra "quando o critério morde, conserta-se o código"
  ganha a cláusula que faltava: **salvo quando a investigação prova que o
  critério media a doença errada — aí conserta-se o critério, por escrito, com
  aceite.**
- A campanha DDD multitenant fecha com a fronteira VERDADEIRA guardada
  mecanicamente — a de valor real (console vs motor), que sempre valeu mais que
  a fronteira prometida em julho (axum vs CLI).
