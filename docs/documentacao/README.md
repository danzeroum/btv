# Documentação de Arquitetura — BuildToValue (`mix_btv_code`)

Documentação completa gerada por **análise estática de todo o código-fonte** do
repositório — Rust (14 crates), Python (5 pacotes), TypeScript (2 SPAs React) — e dos
contratos compartilhados em `schemas/`. O objetivo é mapear **como cada
script/módulo/crate/pacote se conecta ao todo**: dependências, fronteiras de linguagem,
fluxos de dados e responsabilidades — base para estudo de arquitetura e refatoramentos.

**Cobertura:** 137 arquivos `.rs` · 89 arquivos `.py` · 160 arquivos `.ts/.tsx`.

---

## Como navegar

A documentação está dividida em **diagramas UML** (a espinha visual) e **referência**
(o inventário textual detalhado, ancorado por arquivo, que sustenta os diagramas).

### Parte I — Diagramas UML (`diagramas/`)

| # | Documento | O que esclarece |
|---|---|---|
| 01 | [Visão geral e fronteiras de sistema](diagramas/01-visao-geral-e-fronteiras.md) | Mapa de diretórios, a regra de fronteira (ADR 0001), mecanismos de comunicação entre linguagens, dependências externas |
| 02 | [Casos de uso](diagramas/02-casos-de-uso.md) | Atores e funcionalidades principais, `«include»`/`«extend»` |
| 03 | [Pacotes](diagramas/03-pacotes.md) | Grafo de dependências: crates Rust, pacotes Python, módulos TS |
| 04 | [Componentes](diagramas/04-componentes.md) | Processos de runtime e interfaces (HTTP, gRPC, subprocessos, arquivos) |
| 05 | [Classes](diagramas/05-classes.md) | 7 subdiagramas por camada (domínio, runtime, gateway, tools, storage, squad Python, frontend) |
| 06 | [Sequência](diagramas/06-sequencia.md) | 5 fluxos de negócio (ativação de squad, sessão web, decorators, verify, ledger) |
| 07 | [Atividades](diagramas/07-atividades.md) | Orquestração do squad (consenso/HITL/fallback) e ciclo de ferramenta |
| 08 | [Implantação](diagramas/08-implantacao.md) | Local-first + SaaS opcional + esqueleto de infra e CI |
| 09 | [Análise crítica](diagramas/09-analise-critica.md) | Coesão, acoplamento, oportunidades de refatoramento |
| 10 | [Modelo C4](diagramas/10-c4-modelo.md) | Contexto → Contêiner → Componente (Rust, Python, SPA) |
| 11 | [Modelo de dados (ER)](diagramas/11-modelo-de-dados.md) | Esquema de persistência: produto, ledger, event store, telemetria, sessões PG |
| 12 | [Máquinas de estado](diagramas/12-maquinas-de-estado.md) | RunStatus, permissão, handoff, verdict, degradação, sidecar, sessão web |
| 13 | [Módulos por crate (Rust)](diagramas/13-modulos-rust.md) | Estrutura interna dos 14 crates |
| 14 | [Módulos Python e frontend](diagramas/14-modulos-python-e-frontend.md) | Estrutura interna dos 5 pacotes Python e das 2 SPAs |

### Parte II — Referência detalhada (`referencia/`)

| # | Documento | Conteúdo |
|---|---|---|
| 10 | [Inventário Rust](referencia/10-rust-crates.md) | Os 14 crates: propósito, deps, tipos-chave, trait impls, concorrência — com caminhos de arquivo |
| 11 | [Inventário Python](referencia/11-python-pacotes.md) | Os 5 pacotes: classes, ABCs/Protocols, servers gRPC, os 5 agentes |
| 12 | [Inventário TypeScript](referencia/12-typescript-frontend.md) | As 2 SPAs: estrutura, contexts, camada `api/`, DTOs, Designer bpmn |
| 13 | [Contratos gRPC e JSON Schemas](referencia/13-contratos-grpc-e-schemas.md) | 4 serviços gRPC (direções), 11 schemas `*.v1`, os 12 templates, hash dual |
| 14 | [Endpoints HTTP](referencia/14-endpoints-http.md) | Tabela completa REST + SSE, com o handler Rust e o consumidor TS |
| 15 | [ADRs](referencia/15-adrs.md) | Resumo das 32 decisões de arquitetura |
| 16 | [Glossário](referencia/16-glossario.md) | Termos do domínio e da plataforma |

---

## Metodologia da análise

1. **Mapeamento estrutural (top-down):** diretórios e linguagens, fronteiras de sistema,
   dependências externas dos arquivos de gerenciamento (`Cargo.toml`, `pyproject.toml`,
   `package.json`, `*.proto`, `*.v1.schema.json`).
2. **Análise estática de código:** extração de todas as classes/structs/enums/traits/
   interfaces/types e seus relacionamentos (herança, implementação de trait/Protocol/
   interface, composição, agregação, associação, dependência), com ênfase nas **chamadas
   entre módulos e fronteiras de linguagem**.
3. **Identificação de padrões arquiteturais:** camadas (domínio/aplicação/infraestrutura),
   Ports & Adapters, Decorator, Repository, DDD multitenant, ownership Rust
   (`Arc`/`Mutex`/`RwLock`/canais).
4. **Construção dos diagramas UML** em Mermaid (renderizável no GitHub), quebrados por
   camada/fluxo quando a complexidade exigiu.

## Convenções dos diagramas

Estereótipos: `«service»`, `«entity»`, `«value-object»`, `«port»` (trait/Protocol/
interface), `«adapter»`, `«decorator»`, `«dto»`, `«controller»`, `«enum»`.

Relações de classe (Mermaid): `..|>` realização (implementa port), `--|>` herança,
`*--` composição, `o--` agregação, `-->` associação, `..>` dependência.

Nomes preservados **case-sensitive** como no código; identificadores em inglês e
comentários/campos de contrato em português seguem a convenção do projeto (ADR 0024).

---

## Resumo em uma frase

> Um **núcleo Rust** (dono de keys, disco, rede, processos) e um **sidecar Python** (só
> raciocínio) conversam por **gRPC sobre Unix Domain Socket carregando 4 serviços em duas
> direções**; o **navegador** fala só HTTP/SSE com a borda Rust em `127.0.0.1`; os
> **contratos** são single-source em `schemas/`, com o único algoritmo duplicado
> (`prompt-cache-key.v1`) verificado por fixtures de paridade.
