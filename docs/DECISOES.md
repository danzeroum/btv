# Registro de decisões da junção (sessão de 2026-07-05)

Histórico do que foi discutido e decidido ao unificar os três repositórios na
plataforma Forge. Complementa o plano (`PLANO-PLATAFORMA-FORGE.md`) e o ADR 0001.

## As origens e o que cada uma contribui

1. **danzeroum/opencode** (fork TypeScript do coding agent OpenCode) — runtime de
   sessão durável (System Context, Context Epochs, compaction em fronteiras seguras —
   spec em `CONTEXT.md` do repo), agentes selecionáveis (build/plan/general),
   permissões por ferramenta/escopo, ferramentas (grep/edit/bash/webfetch/LSP/MCP),
   TUI. Contribuições próprias do fork: **ModelTier** (classificação small/medium/large
   por id de modelo, comportamento tier-gated: prompt enxuto, menos ferramentas,
   compaction ~75%, step-discipline) e o **pipeline de verificação determinística**
   (`/verify`: typecheck→test→lint→SAST com evidência JSON; filosofia "o LLM orquestra;
   ferramentas determinísticas verificam"), skill-vetter e CI de segurança.
2. **danzeroum/prompte** (ferramenta web de engenharia de prompts, JS/Node) —
   geradores declarativos `{name, fields, build}`, base de conhecimento aditiva
   (3 níveis), quality linter ("ESLint de prompts"), **cache por hash** (JSON canônico
   de chaves ordenadas + sha256; contrato hash cliente == servidor, `api/src/hash.js`),
   rate limiting auth-aware, proxy LLM seguro com fallback (keys só no servidor),
   biblioteca de prompts, telemetria offline-first com dashboard.
3. **danzeroum/BuildToValue_AI_Agent_Specialization** (metodologia BuildToFlip v6 +
   protótipo Python) — squad de agentes especializados
   (Architect/Developer/Auditor/Designer/Ops + Supervisor/Exploration/Recovery),
   UnifiedOrchestrator (recall→plano→propostas→consenso→execução→auditoria→ledger→
   aprendizado), **consenso ponderado por expertise**, planejamento hierárquico,
   LearningRouter, memória com esquecimento inteligente, HITL/autonomia progressiva,
   **fallback progressivo 3 níveis**, ledger append-only, "Nada Fake", review por
   valor (4 reviewers, value_score > 0.7), quality gates e certificação.

## Decisões de produto (do usuário)

- **Produto final**: CLI/TUI de coding agent (`forge`) cujo motor é o squad
  multi-agente, com camada de prompts/qualidade do prompte.
- **Escopo**: 100% das ideias dos 3 repos, roadmap completo em 6 fases longas
  (~44–56 semanas), cada fase terminando em software usável.
- **Linguagens por design**: Rust + Python (pedido original da junção).
- **Sede**: inicialmente workspace `platform/` no BuildToValue; em seguida o usuário
  criou o repositório dedicado **mix_btv_code** — o trabalho passa a viver aqui, com o
  workspace promovido à raiz e commits direto na `main`.

## Decisões de arquitetura (ADR 0001)

- **Regra de fronteira**: Rust = tudo que toca disco/rede/processo/segredo ou roda a
  cada keystroke; Python = tudo que decide o próximo passo por raciocínio de agente.
- **Integração**: gRPC bidirecional sobre Unix Domain Socket (`tonic`/`prost` ×
  `betterproto`/`grpclib`). PyO3 rejeitado no caminho principal (conflito
  tokio×asyncio, isolamento de falhas). Crash do sidecar aciona o fallback
  progressivo do BuildToValue: squad → agente-único → safe-mode read-only.
- **Segurança**: API keys só no processo Rust (princípio do proxy do prompte);
  permissões não contornáveis pelo Python; skill-vetter determinístico; gitleaks
  bloqueante no CI.
- **Contratos**: fonte única em `schemas/` — protobuf no wire, JSON Schema
  (`*.v1.schema.json`) para documentos auditáveis, golden fixtures de paridade
  cross-language; breaking → `.v2` + ADR.

## O que já foi entregue (scaffold da Fase 1)

- Workspace cargo (10 crates) + uv (5 pacotes), compilando com **26 testes Rust +
  13 Python verdes**, clippy/fmt limpos.
- Contratos: 3 protos gRPC (`core`, `squad`, `llm`), 6 JSON Schemas, fixtures de
  paridade do hash de cache validadas pelos dois lados.
- Portes reais: ModelTier (de `model-tier.ts`, com exclusões substituindo lookaheads),
  motor de permissões com perfis build/plan/general, ledger hash-chain com detecção de
  adulteração testada, `/verify` mínimo com evidência JSON, contrato de ferramenta com
  truncamento UTF-8 seguro, consenso ponderado migrado e tipado (pydantic, gatilho
  HITL < 0.7), primeiros geradores declarativos, quality linter, value_score do review.
- Operação: justfile, CI, ADR 0001, `scripts/gen_fixtures.py`.

## Estado dos repositórios de origem (referência histórica)

Branch `claude/multi-repo-implementation-plan-brp6w4` em cada um:

- **opencode**: documento do plano mergeado na `dev` via **PR #196** (squash
  `9b478e5`), CI verde (typecheck, unit, gitleaks, semgrep, compliance, standards,
  nix-eval).
- **prompte**: documento do plano commitado (`ed7419d`), sem PR.
- **BuildToValue**: plano + scaffold `platform/` (`a18282e`) + roadmap visual
  (`41efdb6`), sem PR. O conteúdo foi migrado para este repositório.

## Nota técnica: o roadmap visual

`docs/roadmap-forge.html` é a versão autocontida (React 18.3.1, ReactDOM e o runtime
DC embutidos) do roadmap interativo. Durante o merge foi encontrado e corrigido um bug
real: o runtime DC re-parseia o texto da própria página e corta o template a partir do
primeiro `<x-dc>` literal — que passou a existir dentro do próprio runtime embutido
(string de erro `"has no <x-dc> block"`). A correção quebra o literal em concatenação
(`"<x-dc" + ">"`). Verificado no Chromium headless via `file://` e HTTP: render,
expansão de fases, filtros da matriz (21 ideias) e acordeões funcionando.

## Próximos marcos (Fase 1)

1. Loop de agente real no `forge run`: providers HTTP (Anthropic/OpenAI/DeepSeek) com
   streaming SSE no `forge-llm`, retry e fallback de provedor.
2. Ferramentas reais em `forge-tools` (read/grep nativo via crates `grep`/`ignore`,
   edit/patch, bash com PTY, webfetch) sob o motor de permissões.
3. Sessão persistida no `forge-store` + registro no ledger a cada turno.
4. Critério de conclusão da fase: `forge run "corrija o teste X"` completa uma edição
   com permissão interativa num repo real; ledger registra; `just test` verde.
