# 15 — Referência: ADRs (Architecture Decision Records)

Resumo das 32 decisões de arquitetura (`docs/adr/`), na maioria com status "aceita". São a
justificativa das escolhas refletidas nos diagramas.

| ADR | Tema |
|---|---|
| **0001** | Núcleo Rust + sidecar Python via gRPC/UDS (**a regra de fronteira**) |
| 0002 | Port seletivo do branch `rust-migration` do opencode |
| 0003 | Primeira ativação gRPC: `PromptForgeService` |
| 0004 | Linhagem canônica do squad multi-agente |
| 0005 | Fronteira de injeção do agente: `GatewayClient` + review_system opcional |
| 0006 | Planejamento real + HITL como gate (não executor) |
| 0007 | `UnifiedOrchestrator`: coordenação determinística, evaluator honesto |
| 0008 | Evidência de verificação cruza Rust→Python como campo do `SquadTask` |
| 0009 | Manifesto mínimo de skill + skill-vetter Rust-puro |
| 0010 | Self-hosting do `/verify` no CI (job próprio + artefato de evidência) |
| 0011 | Skills como `dyn Tool`: carga em runtime, vetting, sandbox de terceiro |
| 0012 | Política de confiança MCP: servidor declarado, permissão por chamada |
| 0013 | Embedder RAG: léxico local (TF-IDF), zero-dep |
| 0014 | `experiment.v1`: relatório A/B, Rust-only, verdito honesto |
| 0015 | Modelo de ameaça do web agent: local-first fica, navegador é hostil |
| 0016 | DTO de evento próprio + `Serialize`, contrato SSE (snapshot-then-live) |
| 0017 | Timeout de permissão pendente: fail-closed (Deny após o prazo) |
| 0018 | Sessão como ator único; auditoria de mutação da matriz de permissão |
| 0019 | Sidecar como serviço de longa duração: singleton vs pool, restart-on-crash |
| 0020 | Topologia de processo do web agent: onde o código vive, opt-in, cap de sessão |
| 0021 | Escopo de autonomia progressiva na Fase 7 (`max_autonomy_level` **não wireado**) |
| 0022 | `MemoryService`: ponte Rust↔Python da memória do squad |
| 0023 | `RunTool` ativado: squad como executor sob o motor de permissões |
| 0024 | Context map DDD + classificação Core/Supporting/Generic |
| 0025 | Newtype `TenantId` + `TenantContext` fail-closed (local = tenant fixo) |
| 0026 | Persistência dual: SQLite e Postgres+RLS atrás dos MESMOS traits |
| 0027 | Hash-chain do ledger POR tenant, tenant dentro do hash |
| 0028 | Serialização de append por tenant no Postgres: UNIQUE + retry otimista |
| 0029 | Modelo de identidade + resolução de tenant na borda HTTP |
| 0030 | Evidência de verificação TIPADA no wire (o breaking assinado da janela G3) — *proposta* |
| 0031 | A fronteira real é console/dashboard vs engine (redefinição T4, fechamento C4) |
| 0032 | Restrição numérica do `prompt-cache-key.v1` ENFORÇADA (rejeita floats de fração zero) — *proposto* |

---

## Agrupamento por tema

- **Fronteira Rust/Python:** 0001, 0003, 0005, 0019, 0022.
- **Squad e agentes:** 0004, 0006, 0007, 0008, 0023.
- **Segurança e contenção:** 0009, 0011, 0012, 0015, 0016, 0017, 0018, 0020.
- **Verificação e review:** 0008, 0010, 0014.
- **DDD multitenant e storage:** 0024, 0025, 0026, 0027, 0028, 0029, 0031.
- **Contratos:** 0030, 0032 (o wire tipado e a restrição de hash).
- **Ecossistema:** 0011 (sandbox), 0012 (MCP), 0013 (RAG/LSP embutido no espírito zero-dep),
  0014 (A/B).
