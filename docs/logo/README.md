# Marca BuildToValue — guia de implementação

Símbolo: aro interrompido com gate terracota no topo. Leitura: o sistema gira (aro, raios),
a decisão humana governa (gate). Fonte da verdade visual: `Folha de Identidade BuildToValue` (projeto de design).

## Arquivos (docs/logo/)

| Arquivo | Função | Uso |
|---|---|---|
| `logo-principal.svg` | Versão principal (grafite #2B2B28 + terracota #A85B3F) | Padrão em fundo claro: site, UI, social, promo |
| `logo-fundo-escuro.svg` | Principal para fundo escuro (osso #F1EEE8 + terracota) | Headers escuros, slides escuros |
| `logo-monocromatico.svg` | Uma cor só | Apenas quando a reprodução impõe 1 cor: carimbo, marca d'água, impressão 1-cor |
| `logo-institucional.svg` | Marinho #1E2A44 + latão #B98A2F | Só materiais formais/executivos. Nunca no produto |
| `logo-reduzido.svg` / `logo-reduzido-fundo-escuro.svg` | Aro + centro + gate, sem raios | Obrigatório abaixo de 20 px |
| `favicon.svg` | Ícone de aba (fundo grafite arredondado) | Copiar para `btv-web/public/favicon.svg` |
| `avatar.svg` | Círculo grafite com símbolo | GitHub org, redes sociais |

## Hierarquia de uso

1. **Principal** — escolha padrão. Em caso de dúvida, use esta.
2. **Monocromática** — só por restrição técnica de reprodução, nunca por estética.
3. **Institucional** — só contexto corporativo formal (contratos, propostas, enterprise).

## Fundos permitidos

- Fundo claro → estrutura grafite (`logo-principal.svg`).
- Fundo escuro → estrutura osso (`logo-fundo-escuro.svg`).
- A terracota do gate é **constante** nos dois mundos.
- Proibido: tons médios, fotos sem tratamento. Nesses casos, aplicar sobre bloco de cor sólida ou usar a monocromática.

## Wordmark

"BuildToValue" em **Instrument Sans SemiBold (600)**, sempre em uma cor só.
O acento cromático vive exclusivamente no símbolo. Espaço entre símbolo e wordmark: a largura do gate.
Área de proteção do conjunto: a altura do gate em todos os lados. Tamanho mínimo do símbolo: 20 px.

## Usos incorretos (bloqueados por convenção)

- Rotacionar o símbolo (o gate vive no topo).
- Fechar o aro.
- Gradientes, sombras ou contornos extras.
- Acento no wordmark.
- Dourado/latão fora da variante institucional.
- Recolorir o gate por contexto (estado, tema, feature).

## Tokens (brand.css)

Importar `brand.css` (ou copiar as variáveis para `btv-web/src/styles/global.css`):

- `--brand-graphite: #2B2B28` — estrutura em fundo claro
- `--brand-terracotta: #A85B3F` — gate e acentos de decisão humana
- `--brand-bone: #F1EEE8` — estrutura em fundo escuro
- `--brand-navy: #1E2A44` / `--brand-brass: #B98A2F` — apenas variante institucional

## Checklist de implementação

- [ ] Copiar `favicon.svg` para `btv-web/public/favicon.svg` (substitui o "B" placeholder)
- [ ] Símbolo + wordmark no header do app (usar `logo-principal.svg`, 24–28 px)
- [ ] Splash/login e tela de carregamento com a versão adequada ao fundo
- [ ] Tokens de marca adicionados ao CSS global
- [ ] Avatar atualizado no GitHub org e redes
- [ ] Revisão das telas principais: contraste, área de proteção, tamanho mínimo
