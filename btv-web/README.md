# btv-web — frontend do BuildToValue

SPA primária do `btv dashboard` (servida na raiz; o console BuildToValue de
desenvolvedor, `web/`, fica em `/dev`). Recria em React o design hifi de
`docs/design_handoff_buildtovalue/` — 12 telas em 2 perfis (Meu espaço /
Administração) sobre o motor real do BuildToValue (squad, ledger, telemetria,
permissões).

Stack idêntica ao console (`web/`): React 19 + TypeScript + Vite, zero
dependência de UI, roteamento e estado próprios (Context + reducer), tokens
como CSS custom properties (`src/styles/global.css`, valores do handoff §4).

```sh
pnpm install
pnpm dev            # vite em :5173 (proxy /api → btv dashboard :7878)
pnpm test           # vitest
pnpm build          # tsc -b + vite build → dist/
pnpm test:e2e       # Playwright de UI (shell/navegação), porta 5174
```

Em ambientes com Chromium pré-instalado fora do cache do Playwright:
`PW_EXECUTABLE_PATH=/opt/pw-browsers/chromium pnpm test:e2e`.
