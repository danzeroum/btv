# 04 — Mapa de cobertura de testes por caminho de execução

**Pergunta:** o que NÃO está coberto — e onde mudanças são perigosas?
**Entrada:** arquivos de teste presentes (unit `#[cfg(test)]`, `tests/`, pytest, vitest,
Playwright) + análise de código.

> **Honestidade sobre "%":** **nenhum relatório de cobertura foi executado** (não há
> `tarpaulin`/`pytest-cov`/`lcov` no CI). Portanto **não há percentuais inventados** aqui.
> Este mapa cobre **presença e tipo de teste** por módulo (fato estático) e sinaliza risco
> **qualitativo** nos ramos de erro. Para o % real: `cargo tarpaulin --workspace` e
> `cd python && uv run pytest --cov`.

---

## 4.1 Legenda de tipos de teste

`U` unit (`#[cfg(test)]`/pytest/vitest) · `I` integração (`tests/`) · `E2E` Playwright ·
`C` contrato (dual-adapter) · `G` golden (HTTP/schema) · `P` paridade (Rust↔Python) ·
`B` bench criterion · `X` cross-process (UDS/subprocesso real).

## 4.2 Matriz Rust (crate × tipo × risco no ramo de erro)

| Crate | Tipos presentes | Happy path | Ramos de erro | Risco |
|---|---|---|---|---|
| btv-domain | U | ✅ (agregado, máquina de status, round-trip) | ✅ transições inválidas, tenant vazio | baixo |
| btv-schemas | U, P, G, B | ✅ | ✅ `NumeroProibido`, paridade de hash, fixtures | baixo |
| btv-core | U | ✅ loop completo <100ms, negação, desconhecida, truncado | ✅ `MaxSteps`, deny | baixo |
| btv-llm | U, B | ✅ agregadores, tier, rate-limit | ✅ falha REAL de transporte (conexão recusada) e timeout (servidor que pendura, corta rápido) → `AllFailed` | baixo |
| btv-tools | U, I (`loop_com_ferramentas_reais`, `mcp_integration`, `lsp_integration`) | ✅ tools reais, MCP/LSP contra servidor real | ⚠️ sandbox Docker: testes `#[ignore]` (só no job `sandbox`) | médio |
| btv-store | U, C (`contract_sqlite`, `contract_pg`), I (migrações, replay) | ✅ | ✅ `BrokenChain`/`ForeignEntry`/`Conflict`, RLS adversarial, retry concorrente | baixo |
| btv-verify | U, G (`schema_golden`) | ✅ pipeline, vetter, kill de grupo | ✅ timeout, block fail-closed | baixo |
| btv-sidecar | I, X (`client_over_uds`, `core_server_inprocess`, `python_sidecar`, `squad_e2e`) | ✅ | ✅ **UDS disconnect + `kill -9` → fallback** provado no `squad_e2e` | baixo |
| btv-server | U, G (`golden_http`) | ✅ rotas, guard | ⚠️ ramos de erro dos handlers cobertos parcialmente pelos goldens | médio |
| btv-cli | U, I (`verify_cli`, `wire_strings`), G | ✅ ativação (golden), verify, wire | ⚠️ **web_agent/squad_agent/btv_agent** são grandes; ramos de erro dependem dos e2e | **alto** (superfície grande) |
| btv-tui | U (`TestBackend`) | ✅ render | — (view pura) | baixo |
| btv-golden / btv-contract | (são harness de teste) | — | — | — |
| btv-proto | (gerado) | — | — | — |

## 4.3 Python (pacote × teste)

| Pacote | Testes | Observação |
|---|---|---|
| btv-squad | pytest (`tests/`) | orquestrador/consenso/recall/agentes com `Scripted*` clients |
| btv-promptforge | pytest | hashing (paridade), lint, generators |
| btv-review | pytest | gates duros, score, certification |
| btv-proto-py | — | gerado; exercitado indiretamente pelos cross-process do sidecar |
| btv-eval | — | placeholder vazio |

## 4.4 Frontend (SPA × teste)

| SPA | vitest | Playwright | Observação |
|---|---|---|---|
| btv-web | sim (`*.test.ts`) | e2e + **e2e-integration** (dashboard real + sidecar uv, porta 7998) | CLAUDE.md: 28 vitest + 16 specs de integração; `brand-lint.test.ts` garante lib bpmn sem "BTV" |
| web | sim | e2e-integration (dashboard real) | cobre sessão/permissão/telas |

## 4.5 Caminhos de risco (onde mudar é perigoso)

| Caminho | Cobertura | Risco | Fixture mínima sugerida | Real ou teórico? |
|---|---|---|---|---|
| `btv-cli::web_agent` ramos de erro (permissão timeout, 409 ator único, sessão morta) | e2e parcial | **alto** | unit do `SessionHub` p/ timeout→Deny e single-actor | real |
| `btv-cli::squad_agent` (cockpit inject, HITL expirado, emergency-stop) | e2e | **médio-alto** | teste do `inject_cockpit_context` e do gate obsoleto | real |
| `Gateway` falha real de rede/timeout do provedor | ✅ unit (conexão recusada + servidor que pendura c/ timeout curto) → `AllFailed` | baixo | coberto (`gateway.rs`) | real |
| `Sandbox` contenção (escape, rede, mem) | `#[ignore]` (job `sandbox` com Docker) | médio | já existe — exige daemon | real |
| `PgStore` sob contenção extrema (>64 retries) | contract_pg | baixo-médio | teste que force >64 adversários | teórico (defesa) |
| `BrokenChain` real em produção | unit | baixo | já coberto | teórico (não deve ocorrer) |

## 4.6 Nota sobre o gate de qualidade

O CI **não** roda cobertura, mas roda o job **`verify`** (self-hosting: `btv verify` sobre o
próprio workspace, falha em veredito `Fail`) e o job **`sandbox`** (contenção Docker real +
rust-analyzer LSP real). Isso dá garantia de **comportamento** onde a % de linha não é
medida. Adicionar `tarpaulin`/`pytest-cov` ao CI é a forma de trocar os `⟨medir⟩` desta
página por números reais.
