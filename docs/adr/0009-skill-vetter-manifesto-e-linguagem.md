# ADR 0009 — Manifesto mínimo de skill e o skill-vetter em Rust puro

- Status: aceita
- Data: 2026-07-06

## Contexto

A Fase 5 Onda 5 constrói o skill-vetter descrito no PLANO-PLATAFORMA-FORGE
(`forge-verify`: "pipeline /verify + skill-vetter + evidência JSON"). Não
havia, antes desta onda, nenhum conceito de "skill" no backend (Rust,
Python, proto ou `schemas/`) — só um mock no frontend
(`web/src/types/domain.ts::SkillEntry`, `web/src/api/skills.ts::vetSkill`
marcado `// TODO: backend Fase 5`). Duas perguntas de contrato precisavam
de resposta antes de escrever código: o que é uma skill, minimamente, e em
que linguagem/crate o vetter mora.

## Decisão 1 — `skill.toml` como manifesto mínimo

Uma skill é um diretório com um `skill.toml` na raiz (`name`,
`description`, `entrypoint` opcional, `permissions: Vec<String>`
declaradas, `[[verify]]` opcional — passos de verificação próprios da
skill, no mesmo formato `StepConfig` do `[[step]]` do `forge.toml` da
Onda 2) mais o código da skill.

Razão: consistência com o `forge.toml` já existente (mesmo parser TOML,
mesma forma de declarar passos de verificação) em vez de inventar um
segundo formato de configuração (ex. `SKILL.md` com frontmatter). Não
ganhou um JSON Schema próprio em `schemas/json/` pela mesma razão que
`forge.toml` não tem um: é configuração local lida por um binário Rust,
não um documento persistido/trocado entre processos como
`verification-evidence.v1` ou `ledger-entry.v1` — a regra do PLANO
("documentos persistidos = JSON Schema + fixtures") não se aplica aqui.

## Decisão 2 — vetter em Rust (`forge-verify::vetter`), não em `forge_review` Python

A Fase 5 Onda 4 já tem uma regra dura equivalente
(`forge_review.gates.evaluate`: finding crítico/veredito fail/piso de
segurança sobrepõem a média). Reusar aquele código exigiria uma
dependência Python dentro de `forge-verify` — um crate que hoje é um
motor determinístico puro (sem I/O de rede, sem sidecar, sem gRPC).
Decisão: **reimplementar** a regra "finding crítico ou veredito fail
bloqueia" em Rust (`vetter::vet_skill`), documentada no módulo como uma
duplicação deliberada, não uma dependência cruzada.

Razões:
- `forge-verify` é chamado tanto pelo `forge verify` do CLI quanto,
  potencialmente, por um endpoint do `forge-server` — nenhum dos dois
  caminhos já tem (ou deveria precisar) do sidecar Python de pé.
- A regra em si é pequena (duas condições) — o custo de duplicá-la é
  bem menor que o custo de uma travessia de processo só para avaliar um
  booleano.
- Mantém a fronteira ADR 0001 limpa: decisão de agente/reasoning fica em
  Python: aqui não há reasoning, é checagem determinística de padrão e
  coerência de permissão declarada — o mesmo tipo de trabalho que o
  `/verify` genérico já faz.

## Consequências

- `crates/forge-verify/src/vetter.rs`: `SkillManifest`, `vet_skill(skill_dir,
  run_id, git_sha, produced_at) -> VettingResult{decision: Vet|Block,
  evidence}`. Reusa `run_pipeline`/`StepSpec` para os `[[verify]]`
  declarados; acrescenta duas checagens próprias (padrão perigoso no
  código; permissão declarada incoerente com sinais de uso) como passo
  sintético `checks` na mesma `VerificationEvidence`.
- Fail-closed: manifesto ausente ou TOML inválido → `Block` sempre,
  nunca "vet por default" — testado explicitamente.
- `decision_to_skill_status` mapeia `Vet/Block` para exatamente as
  strings que o frontend já espera (`"aprovado"`/`"bloqueado"`) —
  `"em_analise"` é o estado anterior a rodar o vetter, não uma saída
  dele.
- Fora desta onda (por decisão, não regressão): o endpoint HTTP que liga
  a tela admin `skills` ao vetter real. O mecanismo existe e está
  testado isoladamente em Rust; ligar a tela é trabalho de wiring que
  fica para quando a Fase 5 (ou a 6) exigir a tela real — o mock
  (`web/src/api/skills.ts`) permanece honestamente mock, com o `// TODO`
  atualizado.
