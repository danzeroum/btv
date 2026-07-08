# Handoff: Identidade Visual BuildToValue → btv-web

**Repositório alvo:** `danzeroum/btv` · **App:** `btv-web/` (React + Vite + TS, estilos via CSS custom properties em `#btv-root`)

## Overview

Implementar a nova identidade visual do BuildToValue no produto: novo logo ("aro interrompido com gate"), novos tokens de cor com **semântica fixa** (verde = execução da máquina; terracota = decisão humana; grafite = estrutura), tipografia em três vozes e ajustes pontuais nas 12 telas de `btv-web/src/components/screens/`.

Princípio que governa tudo: **"a inteligência gira, a consciência governa."** A cor ensina o modelo mental: quando algo está terracota, o sistema parou e espera o humano; quando está verde, a máquina opera.

## About the Design Files

Os arquivos em `design/` são **referências de design criadas em HTML** (protótipos mostrando aparência e intenção), **não código de produção**. A tarefa é aplicar essas decisões no código real do `btv-web`, usando os padrões já existentes do repositório (tokens CSS em `#btv-root`, `screenMeta.ts`, componentes de shell). Abra os `.dc.html` no navegador para inspecionar (precisam do `support.js` ao lado, já incluído).

- `design/Folha de Identidade BuildToValue.dc.html` — manual da marca (logo, versões, redução, contraste)
- `design/Plano Geral de Identidade BuildToValue.dc.html` — sistema (cor, tipografia, arquitetura de tela, componentes, 10 regras)
- `design/Referência de Telas BuildToValue.dc.html` — as 12 telas com a identidade aplicada, uma a uma

## Fidelity

**Hi-fi para tokens, cores, tipografia e componentes-base** (valores exatos abaixo — usar verbatim). **Lo-fi para os mocks das 12 telas**: os wireframes na Referência de Telas mostram ONDE cada cor/regra se aplica, não um redesign de layout. **Não reestruturar as telas** — os layouts atuais do btv-web permanecem; muda a pele, não o esqueleto.

## Plano de implementação (ordem recomendada)

### 1. Tokens — `btv-web/src/styles/global.css`

Substituir pelo arquivo `code/global.css` deste pacote (é a versão atualizada do arquivo existente; preserva todas as classes atuais). Resumo das mudanças:

| Token | Antes | Depois | Nota |
|---|---|---|---|
| `--ink` | `#221d15` | `#2B2B28` | grafite da marca |
| `--faint` | `#a89e8b` | `#A89E8B` | inalterado |
| `--brand` / `--brandink` | `#14614f` / `#0c4237` | inalterados | agora significam **execução** |
| `--gold` | `#f2b63c` | **removido da UI** | vira `--inst-navy #1E2A44` / `--inst-brass #B98A2F` (só materiais formais, nunca UI) |
| `--decision` / `--decisionink` | — | `#A85B3F` / `#8A4832` | **novo** — decisão humana |
| `--bone` | — | `#F1EEE8` | texto/logo sobre escuro |
| `--ok` | `#3d8b4f` | `#3D8B4F` | inalterado |
| `--warn` | `#b8531f` | `#B3893B` | **âmbar** — o antigo era próximo demais da terracota |
| `--err` | — | `#C0392B` | novo |
| `--dark-*` | — | ver arquivo | console /dev: `#201E1B` bg, `#2B2B28` surf, `#3C3934` line, `#3E8C74` execução, `#C0765A` decisão |

Novas classes utilitárias incluídas: `.btn-decision` (botão terracota) e `.status-gate` (chip com **ponto quadrado** `border-radius: 2px` — eco do bloco do logo).

Depois, **grep por `--gold`** em todo `btv-web/src/` e migrar cada uso: se marca ponto de decisão/gate → `--decision`; se é alerta → `--warn`.

### 2. Logo e favicon

- Copiar `assets/logo/favicon.svg` → `btv-web/public/favicon.svg` (substitui o "B" placeholder).
- Os SVGs mestres já estão no repo em `docs/logo/` (idênticos aos de `assets/logo/` aqui).
- Header/Topbar (`components/shell/Topbar.tsx`): símbolo `logo-principal.svg` a 24–28px + wordmark "BuildToValue" em Instrument Sans 600, cor `var(--ink)` (uma cor só, nunca acento no texto). Gap entre símbolo e wordmark ≈ largura do gate (~0.5em).
- Regras invioláveis do logo: sem `transform`, sem `filter`, gate sempre no topo; abaixo de 20px usar `logo-reduzido.svg`.

### 3. `screenMeta.ts` — accent do admin

Em `btv-web/src/lib/screenMeta.ts`, trocar o accent das 6 telas de administração de `#345f9e` (azul, fora do sistema) para `var(--muted)` (`#6F675A`). Kickers de usuário permanecem `var(--brand)`.

### 4. Componentes-base

- **Botões** (3 níveis):
  - Decisão humana (terracota cheio): `background: var(--decision)`, hover `--decisionink`, radius 9px, padding 10px 16px, 600/13.5px. **Máximo 1 por tela.** Usos: "Aprovar e liberar" (cockpit do Vivo), "Revisar" (linha com gate em Minhas).
  - Operacional (verde cheio): `background: var(--brand)`, hover `--brandink`. Usos: "Ativar squad", "Testar squad", "Publicar", "Salvar prompt".
  - Secundário (contorno): `border: 1px solid var(--line2)`, texto `--ink`. Usos: "Pedir ajuste", "Exportar", "Ver equipe".
- **Chips de status** (sempre `--mono`, 11px, caixa alta):
  - `EXECUTANDO` — ponto redondo `--brand`
  - `AGUARDANDO GATE` — ponto **quadrado** (radius 2px) `--decision`
  - `CONCLUÍDO/VERIFICADO ✓` — ponto redondo `--ok`
  - `NA FILA` — ponto vazado `--faint`, cartão com borda dashed
  - `EM BREVE` — texto `--faint`, sem interação, nunca simular
- **Cartão de procedência** (Biblioteca): nome do artefato em sans 600; linhas de procedência em mono 10px — "produzido/revisado" em `--muted`, linha "aprovado por você · ledger <hash>" em `--decision`.

### 5. Ajustes por tela (ver mocks na Referência de Telas)

**Meu espaço** (kicker `var(--brand)`):
- `user/Inicio.tsx` — CTA verde só no cartão em foco; demais em contorno. Zero terracota.
- `user/Vivo.tsx` — a tela-mãe: cartão do papel executando com borda `--brand`; cartão do gate com borda `--decision`; papéis na fila com borda dashed + fundo levemente rebaixado. Cockpit à direita contém o único botão terracota.
- `user/Biblioteca.tsx` — cartão de procedência (acima); Exportar em contorno.
- `user/Designer.tsx` — no canvas BPMN, nó de gate: fundo `#F6EBE4`, borda 1.5px `--decision`, prefixo "■"; nó selecionado: borda `--brand`. Conectores em `--muted`.
- `user/Minhas.tsx` — linha com gate ganha botão terracota "Revisar"; demais linhas navegam em contorno.
- `user/Personas.tsx` — prompt em mono sobre `--paper`; papel selecionado com borda `--brand`; "vale na próxima ativação" como kicker `--faint`. Sem terracota.

**Administração** (kicker `var(--muted)`):
- `admin/Telemetria.tsx` — números em `--disp` 700; rótulos mono; barras/dados de execução em `--brand`; métricas no limite em `--warn`.
- `admin/Ledger.tsx` — 100% mono; coluna de ator: humano em `--decision`, agente/sistema em `--brand`; badge "INTEGRIDADE ✓" em `--ok`; hashes truncados `7fe2…91aa` em `--muted`.
- `admin/Providers.tsx` — barra de consumo `--brand` até ~80%, acima `--warn`; falha de provider `--err`; nota local-first em `--faint` mono.
- `admin/Permissoes.tsx` — toggle ligado `--brand`, desligado `--line2` (nunca vermelho); nomes de tools em mono.
- `admin/Modelos.tsx` — chip de versão mono sobre `--paper`; "Publicar" verde; "Nova versão" contorno.
- `admin/Usuarios.tsx` — avatar de iniciais grafite; chip de papel mono (ADMIN invertido grafite/osso); sem cores funcionais.

**Shell/Wizard/GearDrawer**: passo ativo do Wizard em `--brand`; o passo de gates (entregas & gates) marca gates em `--decision`; GearDrawer (Administração) inteiro na família `--muted`/`--faint`.

### 6. Auditoria final

- "Meu espaço" 100% claro (papel); `/dev` (`web/`) 100% escuro quente com `--dark-*`. Nunca misturar na mesma tela.
- Grep de sanidade: nenhum `#f2b63c`, nenhum `#345f9e`, nenhum uso de `--decision` fora de contexto de gate/aprovação.

## Interactions & Behavior

- Hover botões cheios: trocar para o token `*ink` (sem sombra, sem scale). Hover ghost: fundo `color-mix(in srgb, var(--ink) 5%, transparent)`.
- Focus: `outline: 2px solid color-mix(in srgb, var(--brand) 20%, transparent)`; em elementos de decisão, misturar com `--decision`.
- Disabled: `opacity: .45` — **nunca trocar a matiz** (a semântica da cor não pode mudar).
- Executando: animação `btvPulse` existente; cor permanece `--brand`.

## Design Tokens (resumo completo)

- **Fundos:** `--paper #EFE9DE` · `--card #FBF8F1` · `--white #FFFFFF` · linhas `#E0D7C4`/`#D2C7AE`
- **Estrutura:** `--ink #2B2B28` · `--muted #6F675A` · `--faint #A89E8B` · `--bone #F1EEE8`
- **Funcionais:** execução `#14614F`/`#0C4237` · decisão `#A85B3F`/`#8A4832` · ok `#3D8B4F` · warn `#B3893B` · err `#C0392B`
- **Institucional (fora da UI):** navy `#1E2A44` · brass `#B98A2F`
- **Dark /dev:** bg `#201E1B` · surf `#2B2B28` · line `#3C3934` · execução `#3E8C74` · decisão `#C0765A`
- **Tipografia:** Bricolage Grotesque 600–800 (títulos, ls −0.02em) · Instrument Sans 400–700 (corpo 14px/1.6, UI 13px, botões 600) · Spline Sans Mono 400–600 (evidência; kicker 10.5px uppercase ls 0.15em)
- **Layout:** max-width 1060px · espaçamento 8/12/16/22/30 · radius 8–16px · sem sombras (profundidade por camada de fundo)

## Assets

`assets/logo/` (idêntico a `docs/logo/` no repo): `logo-principal.svg`, `logo-monocromatico.svg`, `logo-institucional.svg`, `logo-fundo-escuro.svg`, `logo-reduzido.svg`, `logo-reduzido-fundo-escuro.svg`, `favicon.svg`, `avatar.svg`. Fontes via Google Fonts (já usadas no produto).

## Files

- `docs/identity-system.md` — fonte oficial das regras (colocar em `docs/design/identity-system.md` no repo)
- `docs/brand-README.md` — guia da marca (colocar em `docs/brand/README.md`)
- `code/global.css` — substituto direto de `btv-web/src/styles/global.css`
- `design/*.dc.html` + `support.js` — referências visuais (abrir no navegador)

## Checklist (para PRs)

- [ ] `global.css` substituído; app compila sem regressão visual grosseira
- [ ] Migração de `--gold` concluída (grep limpo)
- [ ] Favicon + logo no Topbar
- [ ] `screenMeta.ts`: admin accent → `var(--muted)`
- [ ] `.btn-decision` e `.status-gate` usados em Vivo/Minhas/Biblioteca
- [ ] 12 telas revisadas conforme seção 5
- [ ] Docs de marca commitadas em `docs/design/` e `docs/brand/`
- [ ] Revisão final: 1 terracota cheio por tela, no máximo
