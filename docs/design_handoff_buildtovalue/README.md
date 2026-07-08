# Handoff: BuildToValue — Plataforma de Squads de IA

> Pacote de handoff para implementação por desenvolvedor (Claude Code).
> Cobre **tudo** que foi produzido nesta iniciativa: a mudança de plano (Forge → BuildToValue),
> as 12 telas do protótipo navegável, o Squad Designer com notação BPMN + auditoria,
> o Cockpit conversacional, e as recomendações de arquitetura sobre a biblioteca
> agnóstica `danzeroum/bpmn` (registry de versionamento + run-binding).

---

## 1. Overview

**BuildToValue** é a evolução do Forge (`github.com/danzeroum/mix_btv_code`) — de coding agent
CLI para desenvolvedores, para uma **plataforma de squads de agentes de IA para profissionais
não técnicos** (editor, analista, professor, músico, advogado…).

Tese central: o motor do Forge (orquestração multiagente, tools plugáveis, cliente MCP,
sandbox, ledger de auditoria, pipeline de verificação) é forte onde o trabalho é
**estruturável** — planejar → gerar → validar → revisar → exportar. Essa esteira vira a
espinha dorsal de toda a UI. O que muda é o público e a entrega: artefatos da profissão
do usuário (DOCX, XLSX, MusicXML, MIDI, PDF, SVG, SRT…), em vez de código.

### Cinco princípios de experiência (guiam toda decisão de UI)

1. **A esteira é a interface.** Toda squad aparece como linha de produção horizontal
   (briefing → produção → revisão → entrega). Nunca logs, grafos técnicos ou métricas
   de token na visão padrão.
2. **Papéis humanos, não agentes.** Cada agente tem nome de ofício: Pauteiro, Redator,
   Revisor de estilo, Fact-checker (editorial); Compositor, Arranjador, Copista,
   Revisor técnico (musical).
3. **O humano é um gate, não um espectador.** Pontos de aprovação param a esteira,
   mostram preview e pedem decisão: aprovar ou pedir ajuste em linguagem natural.
   Evolução (Cockpit): o humano também é **membro** da squad via chat.
4. **A entrega é o produto.** Artefatos exportáveis têm biblioteca própria com versões
   e trilha de procedência (quem produziu / revisou / aprovou).
5. **Complexidade tem endereço próprio.** Telemetria, custos, ledger, providers e
   permissões MCP vivem no perfil Administração, com menu próprio. Máximo 5–6 itens
   de navegação visíveis por contexto.

---

## 2. About the Design Files

Os arquivos deste pacote são **referências de design criadas em HTML** — protótipos que
mostram aparência e comportamento pretendidos. **Não são código de produção.**

A tarefa é **recriar estes designs no ambiente do codebase alvo** (o Forge é Python/gRPC;
o frontend novo pode ser React/Vue/Svelte conforme decisão do time), usando os padrões e
bibliotecas já estabelecidos. Se ainda não existe frontend, recomenda-se React + TypeScript,
para aproveitar a biblioteca `danzeroum/bpmn` (camada React) no Squad Designer — ver seção 9.

Arquivos de referência (na raiz do projeto de design, copiados para `design/` neste pacote):

| Arquivo | Conteúdo |
|---|---|
| `design/BuildToValue.dc.html` | Protótipo navegável completo: 12 telas, 2 perfis, wizard, cockpit, designer BPMN |
| `design/Análise BuildToValue.dc.html` | Documento de análise: diagnóstico, princípios, arquitetura de navegação, inventário de telas, sistema de squads, roadmap |
| `design/Análise bpmn-react.dc.html` | Análise do repositório `danzeroum/bpmn`: cobertura, lacunas, módulos propostos |

O protótipo é um Design Component (HTML + classe JS). Toda a lógica de simulação
(esteira animada, chat, gates) está na classe `Component` dentro do arquivo — útil como
especificação executável do comportamento.

## 3. Fidelity

**High-fidelity (hifi).** Cores, tipografia, espaçamentos, estados e copy são finais e
devem ser reproduzidos fielmente. As áreas de dados (telemetria, ledger, biblioteca)
usam dados de exemplo realistas — os valores são placeholder, o formato/layout é final.

---

## 4. Design Tokens

Definidos como CSS custom properties no elemento raiz do protótipo:

### Cores
| Token | Hex | Uso |
|---|---|---|
| `--paper` | `#efe9de` | Fundo geral da aplicação |
| `--card` | `#fbf8f1` | Superfícies: sidebar, topbar, cards secundários |
| `--white` | `#ffffff` | Cards primários, inputs |
| `--ink` | `#221d15` | Texto principal |
| `--muted` | `#6f675a` | Texto secundário |
| `--faint` | `#a89e8b` | Texto terciário, hints, metadados |
| `--line` | `#e0d7c4` | Bordas suaves |
| `--line2` | `#d2c7ae` | Bordas de controles |
| `--brand` | `#14614f` | Cor da marca (verde-petróleo). **Tweakável pelo usuário** — opções: `#14614f`, `#8a3b2a`, `#2b4a8c`, `#5b3f8c`, `#9a6b14` |
| `--brandink` | `#0c4237` | Hover da marca |
| `--gold` | `#f2b63c` | Gates humanos, destaque do logo |
| `--ok` | `#3d8b4f` | Sucesso / ativo |
| `--warn` | `#b8531f` | Alerta / ações destrutivas |

### Cores por squad (identidade de cada modelo)
Editorial `#b8531f` · Pesquisa `#345f9e` · BI `#1d6f63` · Operações `#57702b` ·
Sales `#b0742c` · Imagem `#8d3f6a` · Educação `#9a6b14` · Design/UX `#2b7a8c` ·
Jurídico `#6b5744` · Música `#6b4fae` · Podcast `#c04a4a` · Vídeo `#444d99`

### Cores das setas BPMN (finalidade)
sequência `#8b8171` · sim `#3d8b4f` · não `#b8531f` · fluxo de dados `#345f9e` (tracejada)

### Tipografia
| Família | Uso | Pesos |
|---|---|---|
| **Bricolage Grotesque** | Display: títulos, logo, números grandes | 700, 800 |
| **Instrument Sans** | UI e corpo | 400–700 |
| **Spline Sans Mono** | Metadados, kickers, timestamps, hashes, chips técnicos | 400–600 |

Escala: título de tela 30px/700 · título de card 15–17px/700 · corpo 13–13.5px ·
metadado mono 9.5–11px com `letter-spacing: .08–.18em` e uppercase nos kickers.

### Raios e sombras
Cards 12–16px · controles 8–10px · pills 999px · blocos BPMN: cards 11px,
losango 7px rotacionado 45°, eventos círculo, API borda dupla (`4px double`),
base de dados topo `5px double` (cilindro).
Sombra de card flutuante: `0 6px 16px -8px #22140a44`. Overlay wizard: `#221d1580`.

### Animações
`btvPulse` (opacity .35↔1, 1.6–2s) para etapa ativa e status ·
`btvBlink` (1s) para cursor "digitando" do papel ativo ·
transições de hover 0.15s (transform/box-shadow/border-color).

---

## 5. Arquitetura de navegação

**Sidebar contextual** (236px, fixa à esquerda) + **topbar** (56px):

- Topbar: logo "B" + wordmark, **toggle de perfil** (Meu espaço / Administração),
  chip da squad ativa (nome + status pulsante), status de conexão, **engrenagem ⚙**
  (abre painel de Ajustes rápidos — sempre visível, em qualquer tela).
- Sidebar perfil usuário: seção **SQUAD ATIVA** (só quando existe squad rodando:
  "Ao vivo" com badge de gate pendente, "Entregas") + seção **GERAL**
  (Início, Minhas squads, Personas, Biblioteca, Designer).
- Sidebar perfil admin: Telemetria, Ledger, Providers, Permissões, Modelos, Usuários.
- O wizard NÃO ocupa item de menu — é overlay de fluxo.
- Fluxo principal: Início → clique num modelo → Wizard (3 passos) → Ao vivo.

Cada tela tem header padrão: kicker mono uppercase (ex. "perfil usuário · U3"),
título display 30px, nota explicativa à direita (máx. 340px, texto muted).

---

## 6. Screens / Views — Perfil Usuário

### U1 · Início — Galeria de squads
- **Propósito**: marketplace de modelos; um clique abre o wizard.
- **Layout**: chips de filtro por categoria (Todas, Conteúdo, Análise, Criativas,
  Operações) + grid `repeat(auto-fit, minmax(250px, 1fr))`, gap 14px.
- **Card de modelo** (branco, borda `--line`, raio 14px, padding 20px; hover:
  translateY(-3px) + sombra): quadradinho 12px na cor da squad + nome (display 16px/700)
  + pill de onda (onda 1 verde `#e7efe9/#2d6a50`, onda 2 âmbar `#fdf3e3/#9a6b14`,
  onda 3 rosa `#f4e9ef/#8d3f6a`); descrição 12.5px muted (min-height 38px);
  chips dos papéis (pill paper); rodapé com chips de formato (mono, cor da squad)
  e link "montar →".
- **12 modelos** (dados completos na classe do protótipo — papéis, formatos,
  perguntas de briefing por squad):
  - Onda 1: Editorial/SEO, Pesquisa & Inteligência, BI/Dados, Operações/SOPs, Sales enablement
  - Onda 2: Imagem/Branding, Educação, Design/UX, Jurídico documental
  - Onda 3: Música simbólica, Podcast, Vídeo

### U2 · Wizard "Montar squad" (overlay modal, 640px)
3 passos com barra de progresso (segmentos coloridos na cor da squad):
1. **Briefing** — 3 perguntas **na linguagem da área** (ex. editorial: pauta, tom,
   público; música: forma, instrumentação, andamento — textos exatos no protótipo) +
   **briefing rico**: input de link ("adicionar") + dropzone de arquivos
   (borda dashed, "⇪ arraste arquivos…"); referências viram chips removíveis (× para excluir).
2. **Equipe** — papéis do modelo com avatar (inicial, cor da squad), descrição e
   **toggle** para desligar papéis ("Desligue papéis que você mesmo fará").
3. **Entregas & gates** — chips dos formatos de exportação + lista dos gates
   ("✋ Aprovar o rascunho antes da revisão", "✋ Aprovar a entrega final antes da
   exportação") + nota de que as ferramentas já foram liberadas pela administração.
Botões: "← Voltar" / "Continuar →" / "⚑ Ativar squad" (primário verde).

### U3 · Squad ao vivo (tela-coração)
- **Esteira** (card branco topo): etapas em linha horizontal — círculo 26px por etapa
  (✓ verde da squad = concluída; contorno pulsante = ativa; ✋ dourado pulsante = gate
  aberto; número cinza = futura), conectadas por linhas 2px (coloridas até onde o
  trabalho chegou), nome da etapa + papel responsável embaixo, barra de progresso
  fina na etapa ativa. Header com nome da squad, "etapa N de M" e botão
  "encerrar squad" (ghost; hover vermelho).
- **Etapas geradas por modelo**: Briefing(Você) → Planejamento(papel 1) →
  Produção(papel 2) → **Rascunho(gate)** → Revisão(papel 3) → Validação(papel 4) →
  **Entrega(gate)** → Exportação(BuildToValue).
- **Coluna esquerda (1.5fr)**:
  - **Card do papel ativo**: avatar quadrado 44px cor da squad, nome do papel,
    pill "trabalhando agora" (verde), frase do que está fazendo + cursor piscante ▮.
  - **Card de gate** (borda 2px `--gold`): título ("Rascunho/Entrega final pronta para
    sua aprovação"), preview do artefato (box paper com kicker "prévia · DOCX · v1"),
    checklist do que o revisor automático validou (✓ verdes), botões
    **"Aprovar e continuar"** (primário) e **"Pedir ajuste"** — este abre textarea
    ("Descreva o ajuste em uma frase — ela vira instrução para o papel certo") e
    vira "Enviar ajuste"; ao enviar, a esteira **volta 2 etapas** e o feed registra.
  - **Card de conclusão** (verde suave): "Entrega concluída" + link para a Biblioteca.
  - **Cockpit** (ver seção 8).
- **Coluna direita (1fr)**: feed de atividade (card `--card`, max-height 420px,
  scroll) — timestamp mono + evento, mais recente no topo.
- **Simulação no protótipo**: timer de 140ms avança progresso (1.1–2.5%/tick ×
  multiplicador de ritmo); ao completar etapa, registra no feed e avança; gates
  interrompem até decisão humana (ou auto-aprovação — ver Ajustes rápidos).

### U4 · Biblioteca de entregas
Grupos por squad (quadradinho de cor + nome + contagem). Cada artefato é uma linha
grid `86px 1.3fr 1.6fr auto`: chip de formato (mono, cor da squad, fundo `#f0ebdf`),
nome + versão/data, **trilha de procedência** ("Redator → Revisor de estilo →
Fact-checker · gate aprovado por Marina"), botão "exportar ↓" (ghost → preenchido no hover).

### U5 · Squad Designer (canvas BPMN) — ver seção 7.

### U6 · Minhas squads
Linhas grid `1.5fr 1fr 140px 1fr auto`: nome, modelo de origem, pill de status
(em produção / aguardando você / concluída / encerrada), barra de progresso na cor
da squad, ação contextual ("abrir ao vivo" / "ver entregas" / "reativar" — reativar
abre o wizard do modelo). A squad em execução aparece no topo.

### U7 · Personas & prompts
- Chips de seleção de modelo (todas as 12 squads, contorno na cor quando ativa).
- Barra: resumo ("N personas · Modelo") + "↺ restaurar todos ao padrão" +
  "+ Nova persona" (primário).
- Grid `minmax(330px, 1fr)` de cards de persona: avatar (inicial, cor da squad),
  nome (editável se persona própria), descrição do papel, **badge de estado**
  (padrão cinza / editado âmbar / própria verde), **textarea com o prompt**
  (mono 11.5px, fundo paper, editável), e ações: "↺ restaurar padrão" (só aparece
  se editado), "remover" (só personas próprias).
- **Prompts padrão** são gerados por papel (4 arquétipos: abre o trabalho / produz /
  revisa / valida — textos completos na classe do protótipo). Override é armazenado
  por modelo+papel; restaurar apaga o override.

### Ajustes rápidos (engrenagem ⚙ — drawer 320px à direita, overlay suave)
- **Marca**: 5 swatches de cor (muda `--brand` na hora).
- **Esteira**: slider "Ritmo da esteira" (0.3×–3×) + toggle "Aprovar rascunhos por
  mim" (gates passam sozinhos após ~1.7s, registrados no feed como aprovação automática).
- **Atalhos**: Personas & prompts, Minhas squads.
- Rodapé: "Estes ajustes valem só para você e são aplicados na hora."

---

## 7. Squad Designer — BPMN simplificado (U5)

### Toolbar
Nome da squad (input inline, borda dashed embaixo) · chips "começar de"
(Em branco / Editorial / Pesquisa / Música — carrega blocos + setas encadeadas do
modelo) · "⧉ duplicar fluxo" (clona todos os blocos E setas com remapeamento de ids)
· "limpar" · "salvar como modelo" (→ aparece como rascunho em Admin/Modelos) ·
"▶ Testar squad" (primário — converte o fluxo em etapas e roda na tela Ao vivo,
com chat/cockpit, marcado "execução de teste").

### Layout
Grid `230px 1fr`. Coluna esquerda: **inspetor acima da paleta** quando há seleção
(bloco ou seta selecionada faz o painel de edição aparecer no topo; desselecionar
devolve a paleta ao topo). Canvas 1200×480 com fundo pontilhado
(`radial-gradient(#d8cfba 1px, transparent 1px)`, 20px), scroll horizontal.

### Paleta (10 blocos, notação BPMN simplificada)
| Bloco | Forma | Cor | Semântica BPMN |
|---|---|---|---|
| Papel | card retangular, ícone ☺ | `#b8531f` | userTask/roleTask |
| Ferramenta | card, ⚒ | `#345f9e` | tool/MCP |
| Chamada de API | card com **borda dupla**, ⇌ + campo endpoint | `#2b7a8c` | serviceTask |
| Base de dados | card **cilindro** (topo double), ⛁ + campo fonte | `#345f9e` | dataStoreReference |
| Gate humano | card, ✋ | `#c79a1e` | manual approval |
| Decisão ◇ | **losango** 36px (rot. 45°) + legenda + campo condição | `#9a6b14` | exclusive gateway |
| Paralelo ⧓ | losango | `#6b4fae` | parallel gateway |
| Início ○ | **círculo** 34px borda fina, ▷ | `#3d8b4f` | startEvent |
| Fim ◉ | círculo borda 3px, ◼ | `#5b5344` | endEvent |
| Exportador | card, ↧ | `#14614f` | output |

Interações: clique na paleta adiciona (e já seleciona); arrastar reposiciona
(clamp ao canvas); clique no fundo desseleciona/cancela conexão.

### Setas = conexões reais (não derivadas de posição)
- Modelo de dados: `{ id, from, to, tipo: 'seq'|'sim'|'nao'|'dados', label }`.
- Criar: selecionar bloco → "⤳ conectar" → banner no canvas ("clique no bloco de
  destino…") → clique no destino. Duplicatas e self-loop são ignorados.
- Renderização: SVG com curvas Bézier entre centros dos blocos, marcador de flecha
  colorido por tipo, rótulo no meio (mono 10px com stroke branco; default por tipo:
  "sim ✓", "não ✕", "dados"), hit-path invisível de 16px para clique.
- **Inspetor da seta**: descrição ("A → B · finalidade"), 4 chips de finalidade
  (mudar tipo recolore e re-traceja — dados é dasharray), campo rótulo,
  "⇄ inverter direção", "apagar seta".
- **Inspetor do bloco**: tipo (pill), nome; campos específicos: prompt do papel
  (com "↺ prompt padrão" quando editado), condição (gateways), endpoint (API),
  fonte (dados); ações: ⤳ conectar, ⧉ duplicar, remover (remove também as setas ligadas).
- **Travessia do fluxo** (esteira resultante + teste): começa no Início sem seta de
  entrada (fallback: bloco sem entrada, senão o primeiro), segue setas de saída
  preferindo não-"nao", evita ciclos, anexa blocos órfãos por ordem de x.
  Exibida como chips encadeados "⌂ Briefing → … → Fim".

### Auditoria do fluxo (requisito: fluxo auditável)
Card "auditoria do fluxo · registro imutável" abaixo do canvas: cada mutação gera
entrada com timestamp HH:MM:SS — bloco criado/duplicado/removido (com nota "setas
ligadas apagadas"), seta criada/mudou de finalidade/invertida/apagada, base
carregada, fluxo duplicado/limpo/zerado, salvamento, teste iniciado.
Mostra as 8 mais recentes + contagem total. **Na produção, isto deve ser gravado
no ledger real (append-only), não apenas em memória.**

---

## 8. Cockpit — o usuário como membro da squad (dentro de U3)

Card abaixo do painel ativo/gate: header "Cockpit · você faz parte da squad" +
**roster** (chips mono de todos os papéis; o chip "Você" preenchido na cor da marca).
Corpo: chat com bolhas — usuário à direita (fundo `--brand`, branco, raio
12/12/4/12), squad à esquerda (fundo paper, borda, raio 12/12/12/4), com autor +
timestamp. Input "fale com a squad — direcione, pergunte, mude o rumo…" + botão
"enviar ↑" (Enter também envia).

Comportamento: mensagem do usuário → registra no feed ("💬 você orientou a squad
pelo cockpit") → o **papel ativo da etapa atual** responde (~1.2s) confirmando a
incorporação da direção. Na produção: a mensagem vira instrução real injetada no
contexto do agente ativo; a resposta vem do modelo. Ao ativar squad (wizard ou
teste do Designer), o primeiro papel envia mensagem de boas-vindas explicando que
o usuário faz parte da equipe. Encerrar squad limpa o chat.

---

## 9. Screens / Views — Perfil Administração

### A1 · Telemetria & custos
4 stat cards (kicker mono, número display 26px/800, delta colorido) +
card "custo por squad" com barras horizontais (grid `170px 1fr 74px`, barra 10px
na cor da squad, valor mono à direita).

### A2 · Ledger de auditoria
Tabela grid `120px 1.7fr 1fr 1fr 110px` (quando / evento / squad / ator / hash).
Eventos exemplo: gate aprovado, exportação gerada, chamada de tool, ajuste
solicitado, squad ativada de modelo vX, conversão na sandbox, **permissão negada
por política MCP**, template publicado. Hash truncado mono (`a91f…c204`).

### A3 · Providers & rate limits
Cards grid `1.2fr 1.4fr 1.4fr auto`: nome + tipo (API direta / gateway / on-device),
modelos e política de uso, barra "uso do limite" com %, pill de status
(operacional verde / ocioso âmbar).

### A4 · Permissões — skills, tools e MCP
Linhas grid `200px 1.7fr 1.5fr auto`: nome da tool (mono) + escopo (ex. "rede ·
somente leitura", "execução confinada", "MCP externo"), descrição, chips das squads
que usam, **toggle permitir/negar** (efeito imediato). Tools exemplo: busca_web,
sandbox_docker, conversor_musicxml, planilhas, gerador_imagem, mcp_figma.

### A5 · Modelos de squad
Tabela `1.6fr 90px 1fr 100px 120px 150px` (modelo / versão / categoria / em uso /
status / ação) com scroll horizontal interno (min-width 780px). Publicar/despublicar
por linha. Modelos salvos do Designer aparecem no topo como "(Designer)",
categoria "Personalizada", v0.1, rascunho.

### A6 · Usuários & acessos
Linhas com avatar circular colorido, nome + e-mail, pill de papel (admin/usuário),
contagem de squads ativas, label ativo/suspenso + toggle de acesso.

---

## 10. State Management

Estado global mínimo (no protótipo é um único estado de componente; na produção,
separar por domínio):

- **Shell**: `perfil` ('user'|'admin'), `tela`, `gearOpen`, `accent`, `ritmo`, `autoGates`.
- **Galeria/Wizard**: `cat` (filtro), `wizardOpen`, `wizardStep` (0–2), `wizardModelId`,
  `papeisOff` (map índice→bool), `wizardRefs` (links/arquivos do briefing).
- **Squad ativa**: `squad { modelId, etapas[{nome, papel, gate?}], idx, prog(0–100),
  gateOpen, done, feed[{ts,txt}] }`, `ajusteMode`, `chat[{from,ts,txt}]`.
- **Personas**: `personaModelId`, `promptOv` (map modelId→{roleKey→prompt}),
  `customPersonas` (map modelId→[{id,nome,prompt}]).
- **Designer**: `blocos[{id,tipo,nome,x,y,prompt?,cond?,endpoint?,fonte?}]`,
  `edges[{id,from,to,tipo,label}]`, `selBloco`, `selEdge`, `connectFrom`,
  `drag`, `designerNome`, `designerBase`, `designerSaved`, `dzLog[]`.
- **Admin**: `toolsOff`, `tplDraft`, `usersOff` (maps de toggles).

Transições-chave: ativar squad (wizard/teste) constrói `etapas` a partir do modelo ou
da travessia do fluxo; tick avança `prog`; gate abre em etapa com `gate:true`;
aprovar avança, ajuste retrocede 2 etapas com instrução; toda mutação do Designer
loga em `dzLog`.

## 11. Interactions & Behavior (resumo dos detalhes críticos)

- Hover em card de modelo: `translateY(-3px)` + sombra, 0.15s.
- Etapa ativa e chip de squad pulsam (`btvPulse`).
- Gate: borda dourada 2px; badge "1 gate" na sidebar; item "Ao vivo".
- Pedir ajuste: 1º clique abre textarea, 2º envia; esteira volta 2 etapas;
  texto citado no feed (truncado a 60 chars).
- Designer: mousedown em bloco = seleciona + inicia drag; em modo conexão,
  mousedown no destino cria a seta; Enter envia chat; clique no fundo limpa seleção.
- Toggles: 40×22px, knob 16px, transição 0.15s.
- Responsividade: grids `auto-fit/minmax`; tabela A5 com scroll interno;
  topbar não quebra (chips `white-space:nowrap`, tabs `flex:none`).

---

## 12. Recomendações de arquitetura — biblioteca `danzeroum/bpmn`

O Squad Designer **não deve ser implementado do zero**: a biblioteca agnóstica
`github.com/danzeroum/bpmn` (bpmn-react) foi criada com essa motivação. Regra de
fronteira — **a biblioteca nunca menciona BuildToValue/BTV**; todo vocabulário de
produto (papéis, gates, personas, prompts, entregas) entra por **plugin de domínio**.

### O que a biblioteca já cobre
- Core headless: modelo tipado, command stack com undo, lifecycle
  `draft→test→candidate→active→deprecated→retired`, versões imutáveis
  (`createDraftFrom` + bump SemVer), setas nunca apagadas
  (`removedInVersion`/`supersedesEdgeId`), aprovação multi-papel, changelog
  obrigatório, vigência (`effectiveFrom/Until`), diff estruturado, ledger SHA-256
  encadeado, import/export BPMN 2.0 + DI.
- React: canvas SVG, formas, gestos, paleta, inspetor, minimap, DiffView, StatusBadge.

### Lacunas a implementar na biblioteca (nomes agnósticos)
1. **`@bpmn-react/registry`** (prioridade 1): VersionRegistry headless com sink
   plugável — `activeAt(date)`, `history()`, `diffBetween(v1,v2)`, canais/ambientes
   (`publish(version, {channel: 'internal'|'pilot'|'general', environment})`),
   changelog dual (`changeSummary` negócio + `technicalNotes` técnico).
2. **`@bpmn-react/run-binding`** (prioridade 2): `bindRun(diagram)` → registro
   imutável `{versionId, semanticVersion, snapshotHash, channel}` gravado em cada
   execução; execução nunca muda de versão em andamento.
3. Melhorias: edição de rótulo inline no canvas, gestos de toque, pools/lanes,
   router com desvio, componente `VersionTimeline`, comandos `promote`/`history` no CLI.

### Mapeamento produto ↔ biblioteca
- Paleta/inspetor/setas/auditoria do Designer → `BpmnEditor` + plugin de domínio
  (nodeTypes: role, tool, service-call, data-store, approval, decisão, paralelo,
  eventos, output — mapeados a tags BPMN 2.0 interoperáveis).
- Auditoria do fluxo (seção 7) → `AuditLedger` da biblioteca.
- "Salvar como modelo" / publicação em A5 → lifecycle + registry (canais).
- Trilha de procedência da Biblioteca (U4) e ledger admin (A2) → run-binding +
  ledger: cada entrega grava a versão exata do fluxo que a produziu.
- Versionamento de templates (A5, coluna versão) → SemVer do lifecycle.

## 13. Backend (Forge) — pontos de integração

- Squad ativa = sessão de orquestração do Forge; etapas mapeiam ao pipeline
  planejar→gerar→validar→revisar; gates usam o mecanismo de verificação existente.
- Cockpit injeta mensagens do usuário no contexto do agente da etapa ativa.
- Permissões A4 = allow-list de tools/MCP por template, aplicada pelo cliente MCP.
- Ledger A2 = ledger existente do Forge, com eventos novos (gate, exportação,
  publicação de template, mudança de fluxo).
- Exportadores por formato rodam na sandbox (ex.: toolchain MusicXML↔MIDI↔PDF).

## 14. Assets

Nenhum asset binário. Ícones são caracteres Unicode (⚙ ✋ ⌂ ▤ ✎ ☺ ⚒ ⇌ ⛁ ◇ ⧓ ▷ ◼ ↧ ⤳ ⧉ ↺ ⇄).
Fontes via Google Fonts (Bricolage Grotesque, Instrument Sans, Spline Sans Mono).
Logo: quadrado arredondado na cor da marca com "B" em Bricolage 800 (dourado no header).

## 15. Files

- `design/BuildToValue.dc.html` — protótipo completo (template + classe `Component`
  com toda a lógica de simulação; a classe serve de spec de comportamento).
- `design/Análise BuildToValue.dc.html` — análise e planejamento (produto).
- `design/Análise bpmn-react.dc.html` — análise da biblioteca e módulos propostos.

## 16. Ordem de implementação sugerida

1. Shell (topbar + sidebar contextual + roteamento por perfil) e tokens.
2. Galeria (U1) + Wizard (U2) com os 12 modelos como dados/templates.
3. Squad ao vivo (U3): esteira + gates + feed, integrada ao orquestrador; depois Cockpit.
4. Biblioteca (U4), Minhas squads (U6), Personas & prompts (U7), Ajustes rápidos (⚙).
5. Squad Designer (U5) sobre `bpmn-react` + plugin de domínio; implementar
   `registry` e `run-binding` na biblioteca em paralelo.
6. Admin (A1–A6) integrando telemetria, ledger, providers, MCP e publicação de modelos.
