# 12 — Referência: os 2 frontends TypeScript

Duas SPAs Vite + React 19 + TypeScript independentes. **Nenhuma usa URL-router** —
navegação por estado (`screen` num reducer `AppContext`, ação `SET_SCREEN`). **Sem axios**
— todo I/O é `fetch` nativo + `EventSource` (SSE). `/api` proxy para `http://127.0.0.1:7878`
(`btv dashboard`) em dev.

Diagrama de classes: ver
[05-classes §5.7](../diagramas/05-classes.md#57-frontend-typescript--contexts-api-clients-e-dtos).
Tabela completa de endpoints: ver [14-endpoints-http](14-endpoints-http.md).

---

## Frontend 1 — `web/` (console dev/admin, montado em `/dev`)

**Papel.** Console do desenvolvedor/admin do motor BuildToValue. Servido por `btv dashboard`
em `/dev` (`base: './'`). Personas: `user` (8 telas) e `admin` (12 telas).

**package.json.** `react`/`react-dom` `^19`, sem router, sem lib de estado (Context +
`useReducer`), **sem bpmn** (Designer hand-rolled), `vite`, `vitest`, `@playwright/test`,
`oxlint`.

**Estrutura `src/`.**
- `main.tsx` → `App.tsx` (providers: `AppProvider` → `ToastProvider` → `SessionProvider` → `Shell`)
- `api/` — 22 módulos client (ver §endpoints)
- `components/primitives/` — `AsyncStatus/Badge/Button/Card/Gauge/Modal/ProgressBar/StatTile/Table/Toast`
- `components/screens/user/` — `Onboarding/Sessao/Permissao/Modelo/Prompts/Squad/Sugestoes` +
  `Designer/` (hand-rolled: `Board/EdgesOverlay/NodeView/Palette/PropertiesPanel/geometry.ts/reducer.ts`)
- `components/screens/admin/` — `Telemetria/Mcp/Modelos/Memoria/Experimentos/RateLimits/Sandbox/Lsp/Ledger/Verify/Providers/Skills`
- `components/shell/` — `Shell/Sidebar/Topbar/WindowChrome/AccentSwitcher/PersonaToggle/ThemeSwitcher`
- `hooks/` — `useAsyncAction.ts`, `usePolling.ts`
- `state/` — `AppContext.tsx`, `SessionContext.tsx`, `useTheme.ts`
- `types/domain.ts` — tipos + DTOs de backend

**Contexts.** `AppProvider` (state+dispatch), `SessionProvider` (sessão de código ao vivo,
montado acima da troca de tela para que uma permissão pendente sobreviva à navegação),
`ToastProvider`. `SessionContext.tsx` é o hook-chave: `sessionId`, `transcript`,
`streamingText`, `pending`, `busy`, `ledgerVerified`; abre `connectSessionEvents`, expõe
`sendMessage`/`resolvePermission`.

**Designer aqui é hand-rolled** (`reducer.ts` + `geometry.ts`), **não** a lib bpmn.

---

## Frontend 2 — `btv-web/` (produto BuildToValue, raiz `/`)

**Papel.** Produto para profissionais **não técnicos** (handoff em
`docs/design_handoff_buildtovalue/`). Servido como SPA **raiz `/`**. Ativação roda o motor
REAL de squad (compartilhado com `/api/squad/run`). Personas: `user` (6 telas nav + `vivo`)
e `admin` (6 telas). Ondas U1–U7 / A1–A6.

**package.json.** `react@^19`; sem router/lib de estado (Context+reducer). **bpmn** consumido
**NÃO por npm deps** mas por **aliases vite** ao submódulo `vendor/bpmn` (dist ESM). Scripts
`predev/prebuild/pretest/ensure:bpmn` rodam `scripts/ensure-bpmn.mjs`. `brand-lint.test.ts`
garante que a lib nunca menciona "BTV".

**Integração bpmn (`vite.config.ts`).** `resolve.dedupe: ['react','react-dom']` (força a
única cópia React 19 do app). `resolve.alias` mapeia `@bpmn-react/{core,react,registry}` →
`vendor/bpmn/.../dist/esm/index.js` (+ `styles.css`).

**Estrutura `src/`.**
- `main.tsx` → `App.tsx` (providers: `AppProvider` → `TemplatesProvider` → `SquadRunProvider` → `Shell`)
- `api/` — `client.ts`, `btv.ts`, `squad.ts`, `templates.ts`, `admin.ts`
- `components/screens/user/` — `Inicio`(U1)/`Vivo`(U3)/`Biblioteca`(U4)/`Designer`(U5)/`Minhas`(U6)/`Personas`(U7)
- `components/screens/admin/` — `Telemetria/Ledger/Providers/Permissoes/Modelos/Usuarios`
- `components/wizard/Wizard.tsx` — "Montar squad" (U2)
- `designer/` — `btvPlugin.tsx` (plugin de domínio para bpmn), `bases.ts`, `flow.ts`
- `lib/` — `esteira.ts` (**`esteiraFromEvents`** + feed), `entregas.ts`, `nav.ts`, `time.ts`
- `state/` — `AppContext.tsx`, `SquadRunContext.tsx`, `TemplatesContext.tsx`

**Contexts.**
- `TemplatesContext` — carrega os 12 templates de `GET /api/btv/templates`, compartilha
  como `{status, templates, byId}`.
- `SquadRunContext` — o hook central da tela ao vivo. Possui `RunState` (`template, nome,
  etapas, taskId, events[], acoes[], streamEnded`). Métodos: `ativar` (`POST /api/btv/squads`
  + SSE), `abrirRun` (reconecta a run persistido), `ativarTeste` (`POST /api/squad/run` para
  "▶ Testar" do Designer), `aprovar`/`ajustar`/`enviarChat`/`encerrar`. Deriva `view` via
  `esteiraFromEvents`, `feed` via `feedFromEvents`.

**Squad Designer plugin (`designer/btvPlugin.tsx`).** Exporta `btvDesignerPlugin: BpmnPlugin`
(vocabulário de domínio injetado na lib agnóstica). `BLOCO_META` (10 tipos), `cardShape`,
`NODE_TYPES` (6 nós `squad:*` → tags BPMN 2.0), `EDGE_STYLES`, `paletteGroups`. Consumido por
`Designer.tsx` que importa `BpmnEditor`/`AuditLedger`/`VersionRegistry` da lib aliased. Ao
salvar: registra versão, anexa ao `AuditLedger` hash-encadeado da lib, e espelha no ledger
BuildToValue via `POST /api/btv/designer/flows`. "▶ Testar" roda o squad real via
`ativarTeste`.

**`esteiraFromEvents` (`lib/esteira.ts`).** Função **pura** `(etapas, events, acoes,
streamEnded) → EsteiraView`. Mapeia `SquadEvent`s reais + ações locais para posição de
esteira `{idx, gateOpen, done, erro, inferida}`. **Sinais diretos:** `Hitl` abre gate,
`Consensus` avança, `Error` congela, `Step` avança, stream-end → done. **Sinais inferidos**
(rotulados `inferida:true` na UI): avanço pós-aprovação e a regressão visual de 2 passos do
"pedir ajuste" (único caso em que `idx` decresce). `makeEtapas` (esteira fixa de 8 estágios),
`feedFromEvents` (mostra os agentes reais architect/developer/auditor/ops).

---

## Cross-cutting

- **Client HTTP compartilhado:** ambas têm um `api/client.ts` idêntico — `fetchJson<T>`
  embrulha `fetch`, lança `ApiError{message, code}`, e guarda corpos vazios (202/204) contra
  `JSON.parse`. `fetch` cru é usado deliberadamente onde há 202/200 sem corpo
  (`postSquadMessage`, `emergencyStopSquad`, `startVerifyRun`).
- **SSE é o único mecanismo de streaming** — dois consumidores: eventos de sessão (só `web`)
  e eventos de squad (ambos). Sem WebSockets; polling é um hook próprio (`usePolling.ts`).
- **Convenção de espelhamento de DTO:** tipos que espelham contratos Rust/proto são
  comentados com a origem (`btv_proto::squad::SquadEvent`, `btv_core::LoopEvent`,
  `btv_schemas::verification::VerificationEvidence`, `btv_store::BtvRun`,
  `btv_schemas::experiment::ExperimentReport`) e consumidos como **serde output direto, sem
  wrapper**.
- **Distinção-chave:** o Designer de `web/` é board bespoke (reducer/geometria); o de
  `btv-web/` é construído sobre `@bpmn-react/*`. Só `btv-web` depende de `vendor/bpmn`.
