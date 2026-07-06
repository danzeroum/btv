# Plano-mestre: Fase 6 — Ecossistema e escala

> Documento de execução, no formato do plano da Fase 5: fatos ancorados no código
> real (verificados no main `bc134e2`, Fases 1–5 concluídas), ondas com fronteira
> verificável, decisões de contrato viram ADR. Estimativa do PLANO-mestre: ~8–10
> semanas.

## 0. Contexto e critérios de conclusão

A Fase 6 abre a plataforma para **código que não é dela**: skills de terceiros,
servidores MCP, e as capacidades de escala (RAG, A/B, bench). O PLANO-mestre define
três critérios literais de conclusão:

1. **Uma skill de terceiro roda após vetting.**
2. **O A/B testing gera relatório.**
3. **O k6 valida o P95 do gateway.**

A tese de segurança herdada das fases anteriores continua mandando: permissões no
core Rust não-contornáveis, vetter bloqueante, e agora **sandbox obrigatório para
código de terceiro** — a Fase 6 é onde o risco muda de natureza (deixa de ser "o
LLM errar" e passa a ser "código alheio rodar na máquina do usuário").

## 1. Estado real na entrada (verificado no main `bc134e2`)

O inventário — o que existe e o que é zero:

**Existe (fundações prontas):**
- **`ToolRegistry`** (`forge-tools/src/registry.rs`): `default_set(root)`, `get`,
  `iter` — o ponto de extensão limpo onde skills e tools MCP plugam.
- **skill-vetter completo** (Fase 5 Onda 5, `forge-verify/src/vetter.rs`):
  `SkillManifest`, `Decision{Vet,Block}`, `vet_skill(dir)`, `VettingResult` com
  evidência, e `decision_to_skill_status` mapeando pro frontend. **A Fase 6 não cria
  vetting — ela o coloca a serviço de código externo.**
- **`DockerSandbox` stub** (Python, `forge_squad/sandbox.py`) com o comentário
  literal: "contêineres reais são escopo da Fase 6". A árvore do PLANO (linha 71)
  situa o sandbox real em **Rust** (`forge-tools`, via **bollard**) — o stub Python
  é interface a preservar, não o lugar da implementação.
- **Telemetria** (`forge-store/src/telemetry.rs`): `record/recent/summary` com
  `by_name`/`cache_hit_rate` — a base de dados do A/B.
- **Frontend com os mocks marcados**: `MCP_SERVERS` em `skills.ts`, e TODOs "Fase 6"
  em `models.ts`/`squad.ts` — os contratos que as ondas ligam.
- **Referência de porte** (PLANO §reuso, linha 120): opencode → "LSP, MCP,
  plugins/skills de terceiros" → `forge-tools` + `skills/`. Portar **ideias**, não
  código TS (a regra de sempre).

**Zero código (tudo a construir):**
- `skills/` (diretório de built-ins + padrão de autoria) — **não existe**.
- LSP, MCP, RAG, A/B, criterion benches, `infra/` (terraform/ansible), k6 — **nenhuma
  linha**; todos os hits de grep foram substring/doc (o "k6" é nome de fixture nos
  testes do vetter).

**Pendência herdada (não-bloqueante):** a pendência de exercício do consenso→ledger —
registrar `squad.consensus` no ledger numa rodada de `forge squad` com API key válida
(`PLANO-PLATAFORMA-FORGE.md` §"Pendência de exercício da Fase 4") — segue aberta; não
interfere na Fase 6, mas deve constar em qualquer reconciliação de docs até ser
exercitada.

Baseline a preservar: contagens do main pós-Fase 5 (medir na entrada de cada onda),
zero falhas, clippy/fmt limpos, e o job `verify` do CI verde (o self-hosting agora
vigia junto).

## 2. Arquitetura das ondas (a lógica da ordem)

A ordem é ditada por **segurança e dependência**, não por tamanho:

- Skills built-in (confiáveis) podem rodar sem sandbox → vêm primeiro e fundam o
  runtime.
- Código de **terceiro** só roda depois do sandbox existir → sandbox antes de
  terceiros, sem exceção.
- MCP e LSP são fontes de ferramentas/contexto — plugam no registry já estendido.
- RAG, A/B e bench/k6 são as ondas de "escala" — dependem pouco entre si e fecham
  os critérios 2 e 3.

### Onda 1 — `skills/` built-in + runtime de skill
**Objetivo:** o conceito de skill vira executável. Criar `skills/` com 1–2 skills
built-in de exemplo (padrão de autoria documentado), e o runtime que carrega uma
skill vetada e a expõe como tool no `ToolRegistry`. Built-ins são confiáveis: rodam
sem Docker, mas **passam pelo vetter mesmo assim** (dogfooding do mecanismo).
**Decisões:** formato de execução da skill (o manifest da Onda 5 já tem
`entrypoint`); como uma skill vira `dyn Tool` no registry. Candidato a ADR.
**Fronteira verificável:** uma skill built-in é vetada, registrada e **invocada
de verdade** por um `forge run` — o resultado da skill aparece na sessão; skill
com vetting Block **não** é registrada (teste que prova a recusa).

### Onda 2 — Sandbox Docker real (bollard, em Rust)
**Objetivo:** o confinamento que os terceiros vão exigir. `forge-tools::sandbox`
via bollard: rodar um comando em contêiner com limites (fs, rede, tempo, memória).
Substituir/ligar o stub Python (a interface `DockerSandbox.run` existente vira
chamada ao lado Rust via o caminho gRPC já existente, ou fica Python-side apenas
como client). Graceful quando Docker não está disponível (**fail-closed para
terceiros**: sem Docker, terceiro não roda; built-in continua).
**Fronteira verificável:** teste de contenção que **morde** — um comando no sandbox
tenta escapar (escrever fora do mount, acessar rede proibida, estourar timeout) e é
bloqueado; o mesmo comando fora do sandbox teria sucesso. No CI, o job precisa de
Docker (runner ubuntu tem) — e pula graciosamente onde não há, **sem** marcar verde
o que não rodou (a lição do skip silencioso).

### Onda 3 — Skill de terceiro de ponta a ponta (critério nº 1)
**Objetivo:** o marco: uma skill externa (fixture "de terceiro": fora do repo,
instalada num diretório de skills do usuário) passa pelo vetter **bloqueante** e
roda **dentro do sandbox**. Fluxo completo: instalar → vet → (Block? nunca roda |
Vet? registra) → executar confinada → resultado na sessão + entrada no ledger.
Ligar a tela admin `skills` de verdade (o `vetSkill` mock vira endpoint real —
a Fase 5 Onda 5 deixou o mapeamento pronto).
**Fronteira verificável:** o critério literal — skill de terceiro roda após
vetting; e o gêmeo negativo: skill maliciosa é bloqueada **e** uma skill vetada que
tenta algo fora das permissões declaradas é contida pelo sandbox. Ambos rodados,
não lidos.

### Onda 4 — MCP (rmcp): servidores externos como fonte de tools
**Objetivo:** cliente MCP no `forge-tools` (lib rmcp, como o PLANO prevê):
conectar a um servidor MCP, listar tools, expô-las no `ToolRegistry` **sob o motor
de permissões existente** (tool MCP = tool como qualquer outra: pede permissão,
registra no ledger). Ligar `MCP_SERVERS` da tela skills.
**Decisões:** tools MCP passam por algum vetting? (Recomendação: o servidor é
declarado pelo usuário = confiança explícita, mas cada chamada passa pelo
permission-engine; registrar em ADR.)
**Fronteira verificável:** teste de integração no padrão cross-process das fases
anteriores — sobe um servidor MCP fixture real (processo separado), o registry
lista suas tools, uma chamada real atravessa e volta, a permissão é pedida, o
ledger registra.

### Onda 5 — LSP: contexto de código para os agentes
**Objetivo:** cliente LSP no `forge-tools`: subir o language server do projeto
(rust-analyzer/pyright conforme o workspace), e expor consultas (definição,
referências, diagnósticos) como tool para os agentes — o squad passa a "enxergar"
o código semanticamente, não só por grep.
**Fronteira verificável:** num workspace fixture, a tool LSP devolve a definição
real de um símbolo (comparada por igualdade com a posição conhecida); o agente
consegue usá-la num fluxo `forge run` real.

### Onda 6 — RAG sobre a memória/telemetria
**Objetivo:** recuperação semântica para o `recall` do squad (hoje o
`AgentMemorySystem.recall_similar` é um no-op na prática). Embeddings + índice local
(offline-first: nada sai da máquina — coerente com o princípio do produto).
**Decisões:** onde vive o índice (`.forge/`), qual embedder (local vs API — se API,
keys só no Rust, como sempre). ADR.
**Fronteira verificável:** hoje o `recall_similar` sem chromadb é um no-op na prática
(o `_FallbackCollection.query` retorna listas vazias sempre; chromadb não é dep
declarada), então um teste comparativo contra ele passaria por vacuidade. A fronteira
certa: com fixture de ground truth (N memórias gravadas, k relevantes conhecidas para
a consulta), o recall recupera **exatamente as k relevantes** (igualdade contra o
conjunto esperado, não "retornou algo"); e um teste prova que o caminho vazio foi
substituído (a mesma consulta que hoje retorna vazio passa a retornar as memórias
certas).

### Onda 7 — A/B testing via telemetria (critério nº 2)
**Objetivo:** o conceito de experimento (variante A/B de prompt/modelo/tier),
atribuição registrada na telemetria existente (`record` já aceita props), e o
**relatório** comparando métricas por variante.
**Fronteira verificável:** o critério literal — um experimento fixture com eventos
das duas variantes gera um relatório com a comparação; e o gêmeo honesto: variantes
sem diferença real não produzem conclusão fabricada (o relatório diz "sem
significância", não inventa vencedor — a régua Nada Fake aplicada a estatística).

### Onda 8 — bench criterion + k6 + `infra/` (critério nº 3)
**Objetivo:** benchmarks criterion nos caminhos quentes (hash canônico, context
epochs, gateway), o cenário k6 contra o gateway local validando o **P95**, e a
`infra/` (terraform/ansible) do PLANO.
**Fronteira verificável:** o critério literal — `k6` roda contra o gateway (com
generator scripted/cassette, sem key real) e o P95 fica sob o limiar definido;
os benches criterion rodam e produzem baseline comparável.
**Nota honesta:** infra/ (terraform/ansible) depende de alvo de deploy que o
produto local-first talvez não tenha ainda — se não houver alvo real, entregar o
esqueleto honestamente marcado é melhor que terraform decorativo. Decidir na onda.

### Onda 9 — Fecho: reconciliação final do roadmap
**Objetivo:** o ato de fecho de sempre — README × PLANO × CLAUDE.md declarando as
6 fases concluídas (ou o estado honesto do que ficou), ADRs citados, contagens
atualizadas, **e o destino da pendência do consenso→ledger** (fechada até lá — uma
rodada real de `forge squad` — ou re-declarada como pendência explícita). É o fim do
roadmap original: o que vier depois é produto novo, não plano antigo.
**Fronteira verificável:** os três documentos contam a mesma história (grep, como
nos fechos anteriores), e nenhuma pendência vive fora dos documentos.

## 3. Decisões de contrato previstas (ADRs)

- **ADR: skill → `dyn Tool`** (Onda 1) — como o manifest/entrypoint vira tool no
  registry.
- **ADR: política de confiança MCP** (Onda 4) — servidor declarado vs vetting por
  chamada.
- **ADR: embedder do RAG** (Onda 6) — local vs API, onde vive o índice.
- Schemas novos prováveis: `experiment.v1` (A/B) — segue a regra (JSON Schema +
  fixture golden + os dois lados).

## 4. Riscos da fase

| Risco | Mitigação |
|---|---|
| Código de terceiro escapar do confinamento (o risco novo da fase) | Sandbox antes de terceiros (ordem das ondas); teste de contenção que morde (Onda 2); permissões Rust não-contornáveis |
| Skip silencioso de testes que exigem Docker/servidor externo | A lição da 4d: skip imprime e é auditável; nunca verde sem ter rodado |
| Vetting de MCP tratado como o de skills (categorias diferentes) | ADR próprio da Onda 4; chamada MCP sempre sob permission-engine |
| RAG/A-B decorativos (existem mas não provam ganho) | Fronteiras comparativas: o RAG acerta onde a heurística erra; o A/B admite "sem significância" |
| infra/ sem alvo real virar terraform de fachada | Decidir na Onda 8; esqueleto honesto > decoração |
| Escopo (a fase é a maior do roadmap) | Cada onda termina em software usável (a regra do PLANO); critérios literais como bússola |
| Pendência do consenso→ledger esquecida no fecho | Item explícito da Onda 9 |

## 5. Sequência e paralelismo

Fio principal (dependência dura): **1 → 2 → 3** (runtime → sandbox → terceiros).
Depois do 3, as ondas 4–8 são majoritariamente independentes entre si e podem ser
reordenadas por conveniência; a recomendação 4 → 5 → 6 → 7 → 8 vai do mais próximo
do produto (tools) ao mais periférico (infra). A 9 é sempre a última.

Contrato de sempre: cada onda ganha seu plano detalhado ancorado no código na hora
de começar (como as ondas da Fase 5); implementa, commita, o próximo puxa e roda —
com o `forge verify` do CI agora cobrando junto desde o primeiro PR da fase.
