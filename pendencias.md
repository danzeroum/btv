# Pendências e decisões da execução autônoma (Fase 6, Ondas 3-tail → 9)

> Log das decisões que tomei sozinho e das dúvidas que quero que você revise.
> Cada item diz se é **decisão** (segui em frente) ou **dúvida** (precisa do seu
> olhar). Ordenado por onda.

## Onda 3 — cauda (`/api/skills` + tela + ledger)

- **[decisão] Tela `skills` vira read-only + "re-vetar".** O status do vetter é
  determinístico e **não sobreponível** pelo usuário (é a régua fail-closed da
  fase — deixar o usuário "aprovar" uma skill bloqueada anularia a segurança).
  Então troquei os botões `aprovar`/`bloquear` (que o mock permitia) por: badge
  read-only do status real + um botão `re-vetar` que re-busca `/api/skills`
  (re-roda o vetter no servidor). O `vetSkill` mock virou `fetchSkills` real.
- **[decisão] `/api/skills` é GET read-only.** Enumera `skills/` (builtin) +
  `.forge/skills/` (third-party), veta cada uma via
  `forge-verify::vetter::list_skill_statuses`, devolve `[{id,status,detail}]`.
  Sem endpoint de ação (vet/block) — não há o que "acionar", o vetter decide.
- **[dúvida] Ledger `skill.vetting` re-veta (double-vet).** Registro o veredito
  no ledger em `run_once` reusando `list_skill_statuses` — mas isso re-veta as
  skills (o `build_registry` já vetou ao carregar). Para built-ins (sem
  `[[verify]]`) o custo é nulo; para uma skill de terceiro com passos
  `[[verify]]` que rodam subprocessos, roda-os 2×. Aceitei por simplicidade e
  zero-ripple. **Futuro:** `load_skills` devolver as decisões e registrar sem
  re-vetar. Além disso, só `run_once` registra hoje; `chat`/`tui` não (fácil de
  estender com o mesmo helper, deixei fora para não alargar o diff).

## Onda 4 — MCP (rmcp)

- **[decisão] Conexão por chamada (connect-per-call).** `McpTool::run` reconecta
  ao servidor (spawn do processo), chama, encerra — a cada invocação. Simples e
  sem estado compartilhado, espelha o sandbox. **Futuro (otimização):** sessão
  persistente (conecta uma vez, reusa a conexão) via um handle numa thread de
  runtime dedicada. Vale para servidores MCP caros de subir.
- **[decisão] Política de confiança MCP (o ADR planejado da onda).** O servidor
  é declarado pelo usuário (em `.forge/mcp.toml`) = confiança explícita; **cada
  chamada** passa pelo permission-engine (nomes `mcp__<server>__<tool>` não
  batem em nenhuma regra → default `Ask` → pergunta ao usuário). Não há vetting
  estilo-skill do servidor. Isto é o conteúdo do ADR 0011 (MCP) — **falta
  formalizar o arquivo em `docs/adr/`** (item da Onda 9).
- **[decisão] Namespacing `mcp__<server>__<tool>` + guarda de colisão.** Uma
  tool MCP não sombreia built-in/skill; registro do mesmo servidor 2× não
  duplica. Fail-soft: `.forge/mcp.toml` ausente/inválido ou servidor que não
  sobe → loga e segue (não derruba o CLI).
- **[decisão] `render_content` extrai só texto.** O resultado MCP pode ter
  blocos não-texto (imagem, resource_link); hoje concateno só os `text`. Refinar
  quando uma tool MCP real devolver conteúdo rico.
- **[dúvida/defer] Frontend MCP não ligado.** `MCP_SERVERS`/`reconnectMcp`
  seguem mock. O wiring real (`/api/mcp` + `fetchMcpServers`) espelha o que fiz
  no `/api/skills` da cauda da Onda 3 — deixei para depois para não inflar a PR.
- **[nota] `rmcp` v2.1.0** entrou como dep direta de `forge-tools` (features
  `client,server,transport-child-process,transport-io`), não em
  `[workspace.dependencies]`. Dep pesada, mas é a lib nomeada pelo PLANO. Passou
  no `cargo deny` local? — verificar no CI (job `deny`). **Resolvido:** passou no
  job `deny` da PR #14 (merge 83a61c4).

## Onda 5 — LSP (rust-analyzer/pyright)

- **[decisão] Zero dependência nova — framing LSP hand-rolled.** O protocolo LSP
  é JSON-RPC com framing `Content-Length` sobre stdio, simples o bastante para
  escrever à mão (só `serde_json`, que já é dep). **Não** puxei `lsp-types`/
  `lsp-server`/`async-lsp` — mantém o `cargo deny` leve e nos dá controle total.
  Provado por um probe contra o rust-analyzer REAL antes de escrever o módulo (o
  framing bate exatamente; a definição de um símbolo volta na posição certa).
- **[decisão] Sessão persistente preguiçosa (≠ connect-per-call do MCP).** O
  language server é caro de subir (rust-analyzer indexa o workspace, ~1-3s). Ao
  contrário do MCP (conecta por chamada), a sessão LSP sobe **uma vez** no
  primeiro uso e as consultas seguintes reusam o processo já indexado
  (`Arc<LspSession>` compartilhada pelas 3 tools do server). Processo morto no
  `Drop` (lição do process-group da Fase 4 — nada de órfão).
- **[decisão] Registro é lazy — não sobe o server no load.** `register_lsp_server`
  só registra as 3 tools (`lsp__<id>__{definition,references,diagnostics}`); o
  processo sobe no primeiro `run`. Então um comando LSP inválido em `.forge/
  lsp.toml` **não** derruba nem trava o `build_registry` (fail-soft): só falha na
  primeira invocação daquela tool. As posições são **0-indexed** (convenção LSP),
  documentado no schema/descrição das tools.
- **[decisão] Prova em duas camadas.** (1) Teste **hermético** com server fixture
  (`forge_lsp_fixture`, sempre roda, sem depender do rust-analyzer instalado) —
  prova framing/handshake/ida-e-volta do cliente. (2) Teste contra o
  **rust-analyzer REAL** (`#[ignore]`, roda no job `sandbox` do CI que instala a
  componente; guarda que FALHA se ela faltar) — prova a semântica: a definição de
  `alvo` volta em `lib.rs:0:7` por igualdade, referências incluem o call-site,
  diagnósticos pegam um erro de sintaxe. Mesma postura anti-falso-positivo do
  sandbox (Onda 2).
- **[dúvida/limitação] Leitura síncrona sob o lock (sem reader de fundo).** Entre
  consultas, notificações do server (`$/progress`, `publishDiagnostics`) ficam no
  buffer do pipe do SO até a próxima consulta drená-las. Para o fixture e uso
  típico é seguro (buffer de 64KB); um projeto gigante com enxurrada de
  notificações poderia, em teoria, encher o buffer entre consultas. **Futuro
  (endurecimento):** thread de fundo drenando stdout num canal. Aceitei a versão
  simples porque a consulta drena tudo ao ler até o próprio id.
- **[dúvida/limitação] Diagnósticos são best-effort/assíncronos.** O LSP empurra
  `publishDiagnostics` após o `didOpen`, sem sinal claro de "assentou". Bombeio
  round-trips baratos (`documentSymbol`) até aparecer um diagnóstico ou estourar
  o orçamento (`DIAG_BUDGET` 12s; sai em ~3s após a 1ª notificação se vier
  vazio). Arquivo limpo → devolve "sem diagnósticos" (honesto). Testei com erro
  de **sintaxe** (reportado nativamente, rápido) e não de tipo (que dependeria de
  `cargo check`/flycheck, mais lento e flaky).
- **[dúvida/defer] Frontend LSP não ligado.** Não há mock de LSP no frontend a
  ligar (diferente do MCP/skills); as consultas LSP são tools que o agente usa no
  loop, não um painel. Sem trabalho de UI nesta onda.
- **[nota] rust-analyzer é uma componente do rustup**, não vem por padrão. O job
  `sandbox` do CI roda `rustup component add rust-analyzer` antes do
  `--include-ignored`. Local: idem para exercitar o caminho real.
