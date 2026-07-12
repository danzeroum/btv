# Parte IV — Mapeamentos operacionais

Oito mapas voltados a **operação, evolução e risco** do sistema: para decidir o que muda
quando você toca em algo, onde cada milissegundo importa, o que acontece quando algo
falha, o que não está coberto por testes, quem controla o quê, o que quebra num schema,
onde está o custo e onde está o dado sensível.

> **Nota de honestidade (regra "Nada Fake" do projeto).** Vários destes mapas pedem
> **dados de runtime** — p99 de latência, cache hit ratio, custo em dólar, % de cobertura.
> Esses números **não são estáveis por análise estática** e **não foram medidos** aqui, então
> **não estão inventados**. Onde falta medição, o mapa entrega (a) a **estrutura estática**
> real (o que é síncrono/bloqueante, os enums de erro, o modelo de custo, a presença/tipo
> de teste) e (b) **o comando/telemetria** para obter o número real. Campos assim aparecem
> marcados como `⟨medir⟩`.

---

## Os oito mapas

| # | Mapa | Pergunta que responde | Base |
|---|---|---|---|
| 01 | [Dependências de build e features](01-dependencias-build-features.md) | Se eu mexer/remover X, o que quebra? | 100% estático (Cargo/pyproject/package/proto) |
| 02 | [Caminho crítico de performance](02-caminho-critico-performance.md) | Onde cada ms importa? | Estrutura estática + benches/k6 reais; p99 do pipeline `⟨medir⟩` |
| 03 | [Failure modes e propagação de erro](03-failure-modes.md) | Se X falha, o sistema para ou continua? | 100% estático (enums de erro + handlers) |
| 04 | [Cobertura de testes por caminho](04-cobertura-de-testes.md) | O que NÃO está coberto? | Presença/tipo de teste (estático); % real `⟨medir⟩` |
| 05 | [Feature flags e variáveis de ambiente](05-feature-flags-e-env.md) | Quem controla o quê? | 100% estático (grep de env/clap/cfg) |
| 06 | [Migração de schema (DB + protobuf)](06-migracao-de-schema.md) | O que quebra se eu alterar o schema? | Estático (proto/JSON schema/migrations/ADRs) |
| 07 | [Custos por operação](07-custos-por-operacao.md) | Quanto custa cada ação? | Modelo de custo estático; $/fluxo e hit ratio `⟨medir⟩` |
| 08 | [Dados sensíveis e fluxo de segurança](08-dados-sensiveis-e-seguranca.md) | Onde entram/saem/repousam PII e keys? | 100% estático (grep de keys/persistência/endpoints) |

---

## Como usar (por cenário)

| Cenário | Mapas mais úteis |
|---|---|
| **Adicionar nova feature** | 01 (dependências), 02 (hot path), 05 (flags) |
| **Refatorar um crate** | 01 (dependências), 04 (cobertura), 06 (schema) |
| **Otimizar performance** | 02 (hot path), 07 (custos), 03 (failure modes) |
| **Migrar para nova versão** | 06 (schema), 01 (dependências), 04 (cobertura) |
| **Auditoria de segurança** | 08 (dados sensíveis), 03 (failure), 05 (flags) |

> **Dica de processo.** Ao criar uma funcionalidade, preencha o mapa de dependências (01)
> e o de failure modes (03) **antes** de escrever código. Isso revela impactos sistêmicos
> cedo e evita retrabalho.

---

## Como obter os números `⟨medir⟩`

| Número | Comando / fonte |
|---|---|
| Cobertura Rust (%) | `cargo tarpaulin --workspace` (não está no CI hoje) |
| Cobertura Python (%) | `cd python && uv run pytest --cov` |
| P95/P99 do gateway | job `k6` do CI (`infra/k6/gateway_load.js`, `ScriptedGenerator`) — P95≈3.5ms documentado |
| Benches de hot path | `cargo bench -p btv-schemas` / `-p btv-core` / `-p btv-llm` (job `bench` do CI) |
| Custo por modelo / uso | `GET /api/models/usage` (telemetria real: `estimate_cost_usd` × tokens) |
| Cache hit ratio | `GET /api/summary` (`cache_hit_rate` da telemetria) |
| Latência real do provedor LLM | dominada pela rede/provedor — fora do controle do repo |
