# BuildToValue

**Squads de inteligência artificial para cada ofício — com o humano no comando.**

BuildToValue é uma plataforma local-first que monta **equipes de agentes de IA**
("squads") para profissionais que não escrevem código: editores, analistas,
professores, advogados, músicos, produtores. Você descreve o que precisa nas
palavras da sua área; uma equipe com papéis humanos — Pauteiro, Redator,
Revisor de estilo, Fact-checker — produz, revisa e valida o trabalho numa
esteira visível, para em pontos de aprovação que só você libera, e entrega
**artefatos reais da sua profissão** (documentos, planilhas, roteiros,
partituras), com trilha completa de quem produziu, quem revisou e quem aprovou.

## Propósito

Fechar a distância entre o poder dos agentes de IA e as pessoas que mais podem
se beneficiar dele. Hoje, orquestrar múltiplos agentes exige terminal, prompts
e jargão técnico. O BuildToValue transforma essa capacidade em uma linha de
produção compreensível: **a esteira é a interface**, os agentes têm nomes de
ofício, e a complexidade técnica (telemetria, custos, permissões, auditoria)
vive num perfil de administração separado — presente, mas nunca no caminho de
quem só quer a entrega.

## Missão

Colocar squads de IA confiáveis nas mãos de qualquer profissional, sob três
compromissos inegociáveis:

1. **O humano é membro e gate, nunca espectador.** A esteira para e espera a
   sua aprovação nos pontos que importam; pelo cockpit, sua orientação entra
   de verdade no contexto do agente ativo — você dirige o trabalho, não
   assiste a ele.
2. **Nada fake.** Cada número, status e artefato na tela vem de uma execução
   real. O que ainda não existe é dito com todas as letras ("em breve"), nunca
   simulado. Toda ação relevante fica num ledger imutável com hash encadeado —
   auditável, verificável, à prova de reescrita.
3. **Local-first e sob seu controle.** Tudo roda na sua máquina (`127.0.0.1`);
   chaves de API nunca saem do processo do motor; permissões de ferramentas
   são explícitas e valem na hora.

## O que a plataforma faz

| Você (Meu espaço) | Administração |
|---|---|
| **Galeria** com 12 modelos de squad prontos (Editorial/SEO, Pesquisa, BI, Operações, Sales, Imagem, Educação, Design/UX, Jurídico, Música, Podcast, Vídeo) | **Telemetria** de uso real por squad |
| **Wizard** de 3 passos: briefing na linguagem da sua área, equipe ajustável, entregas & gates | **Ledger** de auditoria com verificação de integridade |
| **Squad ao vivo**: esteira em tempo real, gates de aprovação, pedido de ajuste, **cockpit** (chat com a equipe) | **Providers** LLM e rate limits |
| **Biblioteca de entregas** com procedência completa e export dos artefatos reais | **Permissões** de ferramentas com efeito imediato |
| **Personas & prompts**: edite o prompt de qualquer papel — vale na próxima ativação | **Modelos**: publicar/versionar templates, inclusive os do Designer |
| **Squad Designer**: desenhe sua própria squad em notação BPMN, teste-a de verdade e salve como modelo | **Usuários**: perfis locais de acesso |

## Arquitetura (resumo)

- **Motor** (`crates/btv-*`, Rust): CLI/servidor `btv`, gateway LLM com
  fallback e rate limit, ferramentas com motor de permissões, sandbox Docker,
  pipeline de verificação (`btv verify`), ledger hash-encadeado e storage
  SQLite local (`.btv/`).
- **Orquestração** (`python/packages/btv-*`, Python): squad multi-agente
  (5 agentes, consenso ponderado, HITL, loop ReAct com execução real de
  ferramentas), falando com o motor por gRPC sobre Unix socket — chaves de
  API existem só no lado Rust.
- **Produto** (`btv-web/`, React): as 12 telas do BuildToValue, servidas por
  `btv dashboard` na raiz. O console técnico de desenvolvedor (`web/`)
  continua disponível em `/dev`.
- **Squad Designer** sobre a biblioteca agnóstica
  [`danzeroum/bpmn`](https://github.com/danzeroum/bpmn) (submodule
  `vendor/bpmn`): canvas BPMN, versionamento com lifecycle, registry e
  run-binding — cada entrega carrega a versão exata do fluxo que a produziu.
- **Contratos** com fonte única em `schemas/` (protos gRPC, JSON Schemas como
  `squad-template.v1`, fixtures golden).

## Como rodar

```sh
git clone --recurse-submodules <repo> && cd btv

# backend + produto
cargo build -p btv-cli
cd btv-web && pnpm install && pnpm build && cd ..
cd web && pnpm install && pnpm build && cd ..      # console dev (/dev), opcional
cd python && uv sync && cd ..                      # orquestrador dos squads

ANTHROPIC_API_KEY=... ./target/debug/btv dashboard # http://127.0.0.1:7878
```

Testes: `cargo test --workspace` · `cd python && uv run pytest` ·
`cd btv-web && pnpm test && pnpm test:e2e:integration` · atalhos no `justfile`.

> **Nota de migração (rename do motor):** o motor chamava-se *Forge*; código,
> binário (`forge` → `btv`), env vars (`FORGE_*` → `BTV_*`), protos
> (`forge.*.v1` → `btv.*.v1`) e o diretório de dados (`.forge/` → `.btv/`)
> foram renomeados. Instalações existentes devem renomear o diretório de dados
> (`mv .forge .btv`) e atualizar env vars. Os documentos históricos em `docs/`
> (ADRs, planos de fase, handoffs de design) preservam o nome antigo como
> registro; o componente interno **PromptForge** mantém o nome próprio.

## Documentação

- `CLAUDE.md` — mapa vivo do repositório e convenções.
- `docs/design_handoff_buildtovalue/` — handoff de design do produto (12 telas).
- `docs/adr/` — decisões de arquitetura (0001–0023).
- `pendencias.md` — decisões e descopes registrados entrega a entrega.

Código e comentários em português; identificadores em inglês. Heranças:
[opencode](https://github.com/danzeroum/opencode) ·
[prompte](https://github.com/danzeroum/prompte) ·
[BuildToValue_AI_Agent_Specialization](https://github.com/danzeroum/BuildToValue_AI_Agent_Specialization).
