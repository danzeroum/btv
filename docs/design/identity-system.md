# BuildToValue — Identity System (v1 · jul/2026)

Fonte oficial das regras visuais do BuildToValue. Deriva de dois documentos de design:
a **Folha de Identidade** (marca) e o **Plano Geral de Identidade** (sistema).
Assets da marca em `docs/logo/`; tokens de código em `btv-web/src/styles/global.css`.

Princípio-mãe: **a inteligência gira, a consciência governa.**
A máquina trabalha em verde; a decisão humana é terracota; a estrutura é grafite sobre papel.

---

## 1. Tokens de cor

### Fundos e estrutura (modo claro — produto)

| Token | Hex | Uso |
|---|---|---|
| `--paper` | `#EFE9DE` | Fundo base da aplicação |
| `--card` | `#FBF8F1` | Superfícies, cartões, header |
| `--line` / `--line2` | `#E0D7C4` / `#D2C7AE` | Bordas 1px / bordas de ênfase |
| `--ink` | `#2B2B28` | Texto principal, estrutura (grafite da marca) |
| `--muted` | `#6F675A` | Texto secundário |
| `--faint` | `#A89E8B` | Texto terciário, "em breve", Administração |
| `--bone` | `#F1EEE8` | Texto/estrutura sobre fundo escuro |

### Funcionais — semântica FIXA

| Token | Hex | Significado | Nunca usar para |
|---|---|---|---|
| `--brand` | `#14614F` | **Execução**: agentes trabalhando, links, ações operacionais | Decisão humana, sucesso |
| `--brandink` | `#0C4237` | Execução :hover | — |
| `--decision` | `#A85B3F` | **Decisão humana**: gates, aprovações, "aguarda você" | Alerta, erro, destaque genérico, decoração |
| `--decisionink` | `#8A4832` | Decisão :hover | — |
| `--ok` | `#3D8B4F` | Sucesso, verificado, íntegro | — |
| `--warn` | `#B3893B` | Atenção: limites, pendências | Gates |
| `--err` | `#C0392B` | Erro, falha, bloqueio | Gates |

> **Regra crítica:** terracota (`--decision`) não é alerta — é decisão. Um elemento terracota
> significa "o sistema parou e espera o humano". Máximo **1 botão cheio terracota por tela**.
> Alertas usam `--warn`/`--err`. Se alguém pedir terracota "para dar destaque", a resposta é não.

### Institucional (marca, fora do produto)

| Token | Hex | Uso |
|---|---|---|
| `--inst-navy` | `#1E2A44` | Variante institucional do logo — só materiais formais |
| `--inst-brass` | `#B98A2F` | Idem. Só aparecem juntos. Nunca na UI |

### Console técnico (`/dev` — escuro quente)

| Token | Hex |
|---|---|
| `--dark-bg` | `#201E1B` |
| `--dark-surf` | `#2B2B28` |
| `--dark-line` | `#3C3934` |
| texto | `--bone #F1EEE8` |
| `--dark-brand` (execução) | `#3E8C74` |
| `--dark-decision` (decisão) | `#C0765A` |

O escuro é **quente** (base grafite, nunca azul) e restrito ao console. Produto claro e
console escuro nunca se misturam na mesma tela.

---

## 2. Tipografia

| Família | Papel | Pesos | Regras |
|---|---|---|---|
| Bricolage Grotesque (`--disp`) | Display | 600–800 | Títulos de tela (30px), seções (20px), números grandes. Letter-spacing −0.02em. Nunca em corpo |
| Instrument Sans (`--sans`) | Texto & UI | 400–700 | Corpo 14px/1.6, UI 13px, botões 14px/600, wordmark 600. Voz do dia a dia |
| Spline Sans Mono (`--mono`) | Evidência | 400–600 | Hashes, IDs, timestamps, custos, status, kickers (10.5px, caixa alta, tracking 0.15em). **Se é auditável, é mono** |

Padrão de cabeçalho de toda tela: kicker mono → título display → corpo sans (classe `.kicker`).

---

## 3. Estados interativos

| Estado | Especificação |
|---|---|
| Hover (botões cheios) | Trocar para o token `*ink` correspondente (`--brandink`, `--decisionink`). Sem sombra, sem scale |
| Hover (contorno/ghost) | Fundo `color-mix(in srgb, var(--ink) 5%, transparent)` |
| Focus | `outline: 2px solid color-mix(in srgb, var(--brand) 20%, transparent)`; em elementos de decisão, usar `--decision` na mistura |
| Active | Escurecer 1 passo além do hover; sem deslocamento |
| Disabled | `opacity: .45; cursor: not-allowed`. Nunca trocar a matiz — a semântica da cor não pode mudar |
| Loading/executando | Animação `btvPulse`; cor permanece `--brand` |
| "Em breve" | Texto em `--faint`, sem interação. Nunca simular funcionalidade |

---

## 4. Layout

- Conteúdo centrado: `max-width: 1060px` (`.btv-stage-inner`)
- Espaçamento em escala de 4: `8 · 12 · 16 · 22 · 30`
- Raio: `8–16px` em cartões e botões; nunca pílulas em cartões
- Bordas `1px var(--line)`; profundidade por camadas de fundo (papel → cartão), **sem sombras**
- Shell: header fino (logo + nav do ofício à esquerda; Administração à direita, em `--faint`),
  esteira como palco central (fluxo horizontal, um cartão por papel), cockpit fixo à direita
  (o único botão terracota da tela vive nele)

---

## 5. Contraste — aprovado / reprovado

| Situação | Veredito |
|---|---|
| Logo principal sobre `--paper`, `--card`, branco | ✔ aprovado |
| Logo fundo-escuro sobre `--dark-bg`, `--dark-surf`, `--ink` | ✔ aprovado |
| Terracota do gate sobre claro E escuro (é constante) | ✔ aprovado |
| Logo sobre tons médios (`--faint`, verdes médios) | ✘ reprovado — usar bloco de cor sólida por trás |
| Logo sobre foto sem tratamento | ✘ reprovado — aplicar overlay sólido ou usar monocromático |
| Texto `--muted` sobre `--paper` em tamanhos <12px | ✘ reprovado — subir para `--ink` |
| Wordmark com acento de cor | ✘ reprovado — wordmark é sempre uma cor só |

---

## 6. As dez regras

1. Terracota = decisão humana. Nunca alerta, nunca decoração; máx. 1 botão cheio por tela.
2. Verde = a máquina trabalhando (links, execução, operações).
3. Se é evidência (hash, custo, timestamp, status), é Spline Sans Mono.
4. Bricolage só em títulos; Instrument Sans em todo o resto.
5. Produto no papel claro; console técnico no escuro quente. Nunca misturar.
6. Profundidade por camadas de fundo, não por sombras.
7. Nada simulado: o que não existe se escreve "em breve", em `--faint`.
8. Administração sempre presente, sempre discreta — nunca no caminho da entrega.
9. Sem gradientes, sem emoji na interface, sem ícones decorativos.
10. O logo segue a folha de identidade: gate no topo, estrutura conforme o fundo, terracota constante.

---

## 7. Checklist de implementação

- [ ] Substituir `btv-web/src/styles/global.css` pela versão com os novos tokens (`--decision`, `--bone`, `--warn`, `--err`, `--dark-*`)
- [ ] Buscar usos de `--gold` na UI e migrar: gates → `--decision`; alertas → `--warn`
- [ ] Componentes-base: `.btn-decision`, `.status-gate` (ponto quadrado), cartão de procedência
- [ ] Copiar `docs/logo/favicon.svg` → `btv-web/public/favicon.svg`
- [ ] Header do app com símbolo + wordmark (logo-principal, 24–28px)
- [ ] Auditoria: "Meu espaço" 100% claro; `/dev` 100% escuro quente
- [ ] Página de referência interna com 6 telas-chave: header, esteira, cockpit, gate de aprovação, cartão de procedência, modo /dev
- [ ] Lint de marca: proibir `transform`/`filter` no logo e terracota fora de contexto de decisão (revisão de PR)
