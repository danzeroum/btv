# ADR 0011 — Skills como `dyn Tool`: runtime de carregamento, vetting e sandbox de terceiros

- Status: aceita
- Data: 2026-07-06

## Contexto

A Fase 6 abre a plataforma para **código que não é dela**: skills de terceiros e
servidores MCP. O risco muda de natureza — deixa de ser "o LLM errar" e passa a
ser "código alheio rodar na máquina do usuário". O skill-vetter (Fase 5 Onda 5,
ADR 0009) já existe como mecanismo bloqueante; a Fase 6 é quem constrói o
**runtime** que carrega e executa skills vetadas por cima dele.

Três perguntas de contrato precisavam de resposta, resolvidas em ondas na ordem
ditada por segurança (runtime → sandbox → terceiros, nunca o inverso): (1) como um
manifest de skill vira uma ferramenta no registry; (2) como código de terceiro é
confinado; (3) qual a régua de confiança que separa built-in de terceiro.

## Decisão 1 — manifest/entrypoint vira `dyn Tool` no `ToolRegistry` (Onda 1)

Uma skill vetada com `Vet` é registrada como um `SkillTool`
(`crates/forge-tools/src/skill.rs`) que implementa o trait `Tool` — a mesma
costura de qualquer built-in. O `build_registry` (`crates/forge-cli/src/skills.rs`)
é o **ponto único** de montagem: enumera `<root>/skills/` (built-ins do repo),
veta cada uma (`forge_verify::vetter::vet_skill`) e registra as aprovadas. Uma
skill `Block` **nunca** é registrada — é o que impede o vetting de ser decorativo.
Built-ins são confiáveis (rodam direto, sem sandbox) mas **passam pelo vetter
mesmo assim** (dogfooding do mecanismo). O `entrypoint` do manifest é o comando
executado; colisão de nome com um tool já registrado é recusada (não sombreia).

## Decisão 2 — sandbox Docker real em Rust, via bollard (Onda 2)

O confinamento que os terceiros exigem vive em **Rust** (`forge-tools::sandbox`,
via `bollard`), não no stub Python (`DockerSandbox` vira interface a preservar). Um
comando roda em contêiner com limites: mount do workspace, rede desligada, timeout
com kill de grupo de processos, memória. A imagem é puxada se ausente
(`ensure_image`); o contêiner roda como o **uid dono do mount** (senão o processo
confinado não consegue escrever no próprio workspace montado). **Fail-closed para
terceiro:** sem daemon Docker, terceiro não roda; built-in continua. A ponte
sync→async (`Tool::run` é sync, bollard é async) é uma thread dedicada com runtime
próprio — não dá para aninhar `block_on` no worker do loop.

## Decisão 3 — a régua de confiança: built-in direto, terceiro confinado (Onda 3)

Duas fontes, duas réguas: `<root>/skills/` (built-ins, confiáveis, rodam direto) e
`<root>/.forge/skills/` (terceiros, **untrusted**, vetados e registrados como
`sandboxed` — o `run` roteia pro sandbox, fail-closed sem daemon). É o critério de
conclusão nº 1 da fase: skill de terceiro roda **após** vetting. A tela admin
`skills` virou **read-only** (badge do status real do vetter + "re-vetar") — deixar
o usuário "aprovar" uma skill bloqueada anularia a régua fail-closed. O veredito
vai ao ledger (`skill.vetting`).

## O que foi provado, não só declarado

- Skill de terceiro **maliciosa** (baixa script remoto e encana pro shell) é
  bloqueada pelo vetter e **não** entra no registry (teste `terceiro_malicioso_e_bloqueado`).
- Skill de terceiro vetada é registrada como **sandboxed** (fail-closa sem daemon,
  distinguindo-a de um built-in que rodaria direto).
- Os quatro vetores de contenção (escrita fora do mount, rede proibida, timeout,
  memória) são testes que **mordem** — `#[ignore]` no `cargo test` normal (aparecem
  como "ignored", nunca verdes por engano), rodados DE VERDADE no job `sandbox` do
  CI com Docker real; o helper `daemon_obrigatorio` FALHA se o daemon não responder.

## Consequências

- O `ToolRegistry` é a costura única de extensão — skills, MCP (ADR 0012) e LSP
  plugam nela, todos sob o mesmo motor de permissões (nada de caminho paralelo).
- A ordem das ondas (sandbox **antes** de terceiros) é a mitigação central do risco
  novo da fase; foi respeitada sem exceção.
- O stub Python `DockerSandbox` permanece como interface; a implementação real é
  Rust-side. Uma futura fiação Python→Rust do sandbox via gRPC fica em aberto.
