# 16 — Glossário

Termos do domínio e da plataforma, para leitura dos diagramas e do código.

| Termo | Significado |
|---|---|
| **BuildToValue / BTV** | A plataforma de squads de IA para profissionais não técnicos (produto novo sobre o motor `mix_btv_code`). |
| **Sidecar** | O processo Python supervisionado que roda o raciocínio (squad, PromptForge, memória). Só conhece o socket; nunca vê keys. |
| **UDS** | Unix Domain Socket — o transporte gRPC entre Rust e Python. |
| **Squad** | Um time de agentes de IA que executa uma tarefa. No motor real são 5 agentes fixos (architect/developer/auditor/designer/ops). |
| **Esteira** | A visualização de progresso (conveyor belt) da tela "Ao vivo" (U3), derivada por `esteiraFromEvents`. |
| **Gate (HITL)** | Ponto de aprovação humana (Human-In-The-Loop). Aprovar libera; "pedir ajuste" aprova *com* uma instrução injetada no cockpit; negar abortaria. |
| **Cockpit** | O canal pelo qual a orientação do usuário vira um turno `user` no próximo `Generate` do agente ativo. |
| **Ledger** | Trilha append-only com hash-chain (`entry_hash = sha256(prev_hash + corpo canônico)`), uma cadeia por tenant, sem UPDATE/DELETE. |
| **Kind (`btv.*`)** | O tipo de uma entrada de ledger (`btv.squad_activated`, `btv.gate_approved`, `btv.flow_saved`, …). |
| **`fake_marker`** | Marca no ledger sinalizando que o payload contém dado simulado ("Nada Fake"). |
| **Persona (U7)** | Override REAL de prompt de um papel do squad, ou uma persona própria do usuário. O prompt efetivo entra na descrição da ativação e no hash de procedência. |
| **Roster** | A lista de `PersonaSpec` de uma ativação — quem trabalha e como (configurável, em vez de agentes fixos). |
| **Template (`squad-template.v1`)** | Um dos 12 modelos de galeria embutidos no binário (`bi`, `editorial`, `juridico`, …). |
| **Deliverable / Entrega** | Artefato exportado pela squad, com trilha de procedência real (papéis + gates). |
| **`/verify`** | Pipeline determinístico (typecheck/test/lint/SAST) que produz `verification-evidence.v1`. O auditor julga sobre ela, não sobre opinião de LLM. |
| **Vetter** | A máquina que decide `Vet`/`Block` para uma skill (reusa o pipeline do `/verify`). |
| **Gateway** | O adapter Rust que fala com os provedores LLM (Anthropic/DeepSeek/OpenAI). API keys só aqui. |
| **Decorator stack** | `CachedGenerator<RateLimitedGenerator<Gateway>>` — o cache (externo) evita consumir vaga/token em hits. |
| **`ModelTier`** | `Small`/`Medium`/`Large` — decide política de compaction e rate-limit. |
| **Compaction** | Resumo do histórico em fronteira segura quando se aproxima da janela de contexto (tier-gated). |
| **Port / Adapter** | Trait (Rust) ou `Protocol` (Python) que define a fronteira; o adapter é a implementação concreta injetada. |
| **`TenantId` / `TenantContext`** | Identidade multitenant. `LOCAL` = `...0001` (modo local-first é um tenant, não a ausência de um). Fail-closed por construção. |
| **`ScriptedGenerator`** | Gerador keyless/determinístico para benches, k6 e testes. |
| **MCP** | Model Context Protocol — servidores externos cujas tools entram no `ToolRegistry` sob o mesmo motor de permissões. |
| **LSP** | Language Server Protocol — cliente hand-rolled zero-dep (definição/referências/diagnósticos/símbolo). |
| **Sandbox** | Cela Docker (bollard) que confina skills de terceiro (rootfs RO, cap_drop, rede off), fail-closed. |
| **`onda` / U#/A#** | As "ondas" de entrega do produto: U1–U7 (usuário), A1–A6 (admin). |
| **`bpmn` / `@bpmn-react/*`** | A lib agnóstica (submódulo `vendor/bpmn`) sobre a qual o Squad Designer de `btv-web` é construído; nunca menciona "BTV". |
| **RLS** | Row-Level Security do Postgres, no modo SaaS (`feature pg`), isolando tenants. |
| **Golden test** | Teste que compara a resposta HTTP com uma fixture congelada (`schemas/fixtures/http/*.golden.json`). |
