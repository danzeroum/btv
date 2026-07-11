# Auditoria exaustiva arquivo-a-arquivo — BuildToValue (`mix_btv_code`)

> **Somente relatório — nada foi alterado, commitado ou pushado.** Este documento é o resultado
> de uma varredura orquestrada (43 subagentes) que **leu integralmente os 337 arquivos-fonte**
> (Rust + Python + TypeScript) e submeteu **cada achado de severidade alta a verificação
> adversarial** — os falsos-positivos foram descartados antes de entrar aqui.

> Gerado a partir de 337 revisões de arquivo. Convenções do projeto (comentários
> em PT, testes inline, `thiserror`, DI por `Protocol`, `TenantId` sem `Default`) foram tratadas
> como padrão — não como defeito.

## 1. Cobertura e distribuição

- **Arquivos-fonte revisados integralmente:** 337 / 337 (100%).
- **Arquivos com pelo menos 1 achado:** 137.
- **Arquivos limpos (0 achados):** 200.
- **Total de achados retidos:** 239 (alta pós-verificação adversarial; rejeitados excluídos).

**Por severidade:** alta = 2 · media = 65 · baixa = 172

**Por linguagem:** rust = 119 · python = 40 · ts = 80

**Por princípio/eixo:**

| Princípio | Achados |
|---|---|
| correcao | 97 |
| DRY | 51 |
| Clean Code | 20 |
| naming | 15 |
| SRP | 13 |
| clean-code | 12 |
| Big-O | 11 |
| 12-Factor | 5 |
| seguranca | 4 |
| magic-number | 4 |
| efficiency | 2 |
| manutenibilidade | 2 |
| dead-code | 1 |
| performance | 1 |
| OCP | 1 |

## 2. Destaques — correção e segurança (alta + média)

34 achados de correção/segurança de maior impacto. Estes NÃO são estilo — são
comportamento. Recomenda-se PR próprio por item, com juiz declarado.

- **`crates/btv-sidecar/src/service.rs:256`** _(sev alta, verificado: confirmado)_ — Em SquadPool::acquire, se SquadSupervisor::spawn ou wait_ready falham (os `?` nas linhas 256-262), o `permit` do semaforo e liberado no Drop mas o `slot` ja retirado de `self.free` NUNCA volta para a lista de livres — free-list e semaforo saem de sincronia.
  - **Correção:** Nos caminhos de erro de spawn/wait_ready empurrar `slot` de volta para `self.free` antes de retornar (ex.: um guard com Drop que faz push, desarmado so ao construir o SquadLease com sucesso). Caso contrario cada falha de subida do sidecar (Python ausente/timeout) vaza um slot permanentemente e, apos capacity falhas, free.pop() na linha 239 atinge o .expect e derruba o processo.  · _diff auto-contido viável_
- **`crates/btv-tools/src/bash.rs:80`** _(sev alta, verificado: confirmado)_ — stdout/stderr são pipes lidos só APÓS o processo sair; um comando que produz mais que a capacidade do pipe (~64KB) bloqueia na escrita, nunca sai, e o loop try_wait só termina no timeout — todo comando com saída grande falha por timeout.
  - **Correção:** Ler stdout e stderr concorrentemente (threads dedicadas ou async) enquanto se aguarda o processo, ou usar Command::output com controle de timeout, em vez de ler depois do wait.  · _diff auto-contido viável_
- **`/home/user/btv/python/packages/btv-squad/tests/test_designer.py:67`** _(sev media)_ — O teste usa try/except com `assert False` para verificar que RuntimeError e levantado, padrao fragil (se execute nao levantar, a mensagem de falha vem do assert dentro do try, nao do fluxo esperado).
  - **Correção:** Substituir por `with pytest.raises(RuntimeError, match="attach_gateway"): asyncio.run(agent.execute(...))`, eliminando o assert-False manual.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/admin/Ledger.tsx:74`** _(sev media)_ — O regex que classifica ator humano inclui o nome hardcoded 'marina', um dado de demo vazando na logica de producao — qualquer ator chamado assim e marcado como humano indevidamente.
  - **Correção:** Remover 'marina' do regex; derivar 'humano' apenas dos kinds de gate e de marcadores genericos (human/voce/usuario), sem nomes proprios embutidos.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/admin/Usuarios.tsx:39`** _(sev media)_ — createUser(...).then() em 'adicionar' não tem .catch: falha de criação some silenciosamente (unhandled rejection), diferente de 'remover' que trata erro.
  - **Correção:** Encadear `.catch((e: Error) => setErro(e.message))` (ou similar) em createUser para dar feedback consistente ao usuário.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/admin/Usuarios.tsx:59`** _(sev media)_ — verifyUserPin(...).then() em 'confirmarPin' não tem .catch: erro de rede/backend na verificação de PIN não vira feedback nem pinErro.
  - **Correção:** Adicionar `.catch(() => setPinErro('Não consegui verificar o PIN.'))` para diferenciar falha de verificação de PIN incorreto.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/user/Personas.tsx:33`** _(sev media)_ — `recarregar` seta `erro` no catch mas nunca o limpa em sucesso, então um erro antigo persiste no banner mesmo após recarga bem-sucedida ou troca de template.
  - **Correção:** Chamar `setErro(null)` no `.then(setData)` (ou antes do fetch) para que um recarregamento bem-sucedido limpe o banner de erro anterior.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/user/Personas.tsx:93`** _(sev media)_ — As mutações (`restoreAllPersonas`, `createCustomPersona`, `setPersonaOverride`, `restorePersona`, `updateCustomPersona`, `deleteCustomPersona`) usam `void ...then(recarregar)` sem `.catch`, engolindo falhas de escrita sem feedback ao usuário.
  - **Correção:** Encadear `.catch((e) => setErro(e.message))` (ou toast) nessas promessas para que uma falha de gravação seja visível, em vez de rejeição não tratada silenciosa.  · _diff auto-contido viável_
- **`btv-web/src/components/screens/user/Vivo.tsx:66`** _(sev media)_ — Em contar(), uma falha de listDeliverables é engolida com catch → return 0, tornando erro de rede indistinguível de zero entregas e renderizando o alarmante "Concluída, mas sem artefato real".
  - **Correção:** Retornar null (ou lançar) no catch e só cravar artefatosDaTask=0 quando a contagem for confirmada; enquanto houver erro, não exibir a mensagem de "sem artefato" (manter null / estado neutro).  · _diff auto-contido viável_
- **`btv-web/src/components/wizard/Wizard.tsx:447`** _(sev media)_ — WizardOverlay renderiza <WizardInner> sem `key` por template; o estado (answers, step, papeisOff, refs) é inicializado só na montagem e não reseta se outro template for aberto sem desmontar o wizard.
  - **Correção:** Passar `key={wizardTemplateId}` (ou template.id) em <WizardInner> para forçar remontagem e reinicializar o estado quando o template muda.  · _diff auto-contido viável_
- **`btv-web/src/state/SquadRunContext.tsx:116`** _(sev media)_ — JSON.parse(btvRun.papeis_json || '[]') em abrirRun nao tem try/catch; papeis_json malformado lanca excecao nao tratada e quebra a reabertura do run.
  - **Correção:** Envolver o parse em try/catch (ou funcao helper safeParseArray) que devolve [] em caso de JSON invalido, degradando para nenhum papel ativo em vez de estourar.  · _diff auto-contido viável_
- **`crates/btv-cli/src/btv_agent.rs:285`** _(sev media)_ — Apos start_squad_task ter iniciado a squad, as falhas seguintes (save, get None, activation_event) retornam erro sem parar a task iniciada nem spawnar o watcher, deixando um run orfao rodando sem persistencia nem transicao de status.
  - **Correção:** Em cada caminho de erro apos o start (linhas 339, 348, 370), sinalizar o hub para encerrar a task (kill-switch) antes de retornar, ou spawnar o watcher/persistir de forma que o run nao fique orfao.  · _estrutural / requer contexto_
- **`crates/btv-cli/src/btv_agent.rs:918`** _(sev media)_ — list_overrides(...).unwrap_or_default() engole erro de banco: uma falha de leitura vira 'sem overrides', e o handler responde HTTP 200 com editado:false, representando incorretamente o estado das personas.
  - **Correção:** Tratar o Err (retornar 500 store_error como nos demais handlers de leitura, ou ao menos logar via eprintln) em vez de mascarar como HashMap vazio; o mesmo vale para list_custom na linha 942.  · _diff auto-contido viável_
- **`crates/btv-cli/src/squad_agent.rs:598`** _(sev media)_ — A task `core_task` (serve_core) so e abortada no caminho de sucesso (linha 704); todos os `?` de early-return depois do spawn a vazam.
  - **Correção:** O `serve_core` roda indefinidamente. Nas saidas antecipadas via `?` (verification_evidence L622-623, ledger open L636, acquire L641, execute_task L665) o `JoinHandle` e apenas dropado, e tasks tokio nao sao canceladas no drop — o servidor continua vivo detached segurando o UDS. Envolver o corpo apos o spawn de forma que `core_task.abort()` rode em todos os caminhos (ex.: extrair o miolo para uma fn interna e abortar no retorno, ou usar um guard que aborta no Drop).  · _diff auto-contido viável_
- **`crates/btv-cli/src/tui_app.rs:336`** _(sev media)_ — Combinações Ctrl+<tecla> não tratadas (qualquer coisa além de c/m/g) caem no braço `_ => {}` sem `continue` e escorregam para o segundo match, inserindo a letra no input (ex.: Ctrl+B digita 'b'; Ctrl+Backspace apaga).
  - **Correção:** Adicionar `continue;` após o bloco de CONTROL (ou no braço `_ => {}`) para consumir a tecla e não deixá-la cair no tratamento de Char/Backspace normal.  · _diff auto-contido viável_
- **`crates/btv-cli/src/web_agent.rs:989`** _(sev media)_ — Erros `Lagged` do broadcast sao descartados via `.filter_map(|r| r.ok())`, entao um cliente SSE lento perde eventos silenciosamente sem qualquer sinal de gap.
  - **Correção:** Tratar `BroadcastStreamRecvError::Lagged(n)` explicitamente emitindo um evento de sinalizacao (ex.: um `SessionEvent` de aviso ou forcando reconexao/snapshot) em vez de silenciosamente dropar; ou aumentar/monitorar a capacidade do canal (256) conforme a taxa de eventos.  · _estrutural / requer contexto_
- **`crates/btv-cli/src/web_agent.rs:194`** _(sev media)_ — Sessoes nunca sao removidas do HashMap e o `log` por sessao cresce sem limite; `max_sessions` vira um teto permanente do processo e a memoria nunca e liberada.
  - **Correção:** Adicionar remocao/expiracao de sessoes concluidas (ou LRU) e limitar/truncar `SessionState::log` (ring buffer ou cap), para que o teto reflita sessoes vivas e nao acumulado historico do processo.  · _estrutural / requer contexto_
- **`crates/btv-cli/src/web_agent.rs:492`** _(sev media)_ — `durable.persist_new().unwrap_or(0)` engole falha de persistencia da sessao duravel; o usuario recebe `done` mesmo se o historico da conversa nao foi salvo.
  - **Correção:** Propagar ou ao menos logar/publicar a falha de `persist_new()` (ex.: converter em `SessionEvent::Error` ou `?`), em vez de descartar o `Result` com `unwrap_or(0)`.  · _diff auto-contido viável_
- **`crates/btv-llm/src/gateway.rs:54`** _(sev media)_ — Se o builder do cliente com timeouts falhar, o fallback `reqwest::Client::new()` reintroduz exatamente o cliente SEM timeouts que o comentário do módulo diz causar travamento eterno.
  - **Correção:** Em vez de cair para um cliente sem timeouts, propagar o erro ou construir o fallback ainda com os timeouts default (connect/read) para nunca reabrir a janela de hang que a função existe para fechar.  · _diff auto-contido viável_
- **`crates/btv-llm/src/rate_limit.rs:73`** _(sev media)_ — Com max_requests=0 (construtor publico `new` aceita), `ts.len() < 0` e falso e o codigo cai no `ts.front().expect(...)` sobre um deque vazio, causando panic em vez de rejeitar a chamada.
  - **Correção:** Guardar max_requests==0 explicitamente (retornar RateLimitError ou tratar como 'nunca ha vaga') antes do acesso a front(), ou documentar/validar max_requests>=1 no construtor.  · _diff auto-contido viável_
- **`crates/btv-llm/src/sse.rs:27`** _(sev media)_ — O comentario afirma tolerar terminador '\r\n\r\n', mas find("\n\n") nao casa CRLF puro, entao um evento SSE com quebras CRLF nao e emitido ate chegar bytes extras.
  - **Correção:** Normalizar CRLF antes de procurar o terminador (ex.: substituir \r\n por \n no buffer) ou procurar tambem por "\r\n\r\n" ao delimitar blocos, para servidores que usam CRLF funcionarem sem depender de dados adicionais.  · _diff auto-contido viável_
- **`crates/btv-server/src/doctor_console.rs:125`** _(sev media)_ — vocabulario_check faz stores.btv.lock().unwrap() (e .ledger.lock().unwrap() na 135) no caminho do handler; um mutex envenenado por panic em outra thread derruba a request.
  - **Correção:** Tratar o PoisonError devolvendo um DoctorCheck{ok:false, detail: ...} em vez de unwrap(), mantendo o handler resiliente a lock envenenado.  · _diff auto-contido viável_
- **`crates/btv-store/src/btv.rs:243`** _(sev media)_ — max_run_task_seq engole qualquer erro de storage retornando 0, o que pode fazer a proxima ativacao gerar sq1 e colidir em UNIQUE(runs.task_id).
  - **Correção:** Propagar o erro (retornar Result<u64,_>) ou, se a assinatura legada nao pode mudar, ao menos logar a falha de prepare/query_map antes de cair no 0, para nao mascarar uma colisao de task_id como banco vazio.  · _estrutural / requer contexto_
- **`crates/btv-store/src/events.rs:96`** _(sev media)_ — query_row do seq usa unwrap_or(0), engolindo qualquer erro real do SQLite (db locked, corrupcao) e tratando como head=0.
  - **Correção:** Trocar por rusqlite OptionalExtension: `.optional()?` distingue QueryReturnedNoRows (→ 0) de erro genuino (→ propaga EventError::Storage). Mesmo padrao em head_seq (linha 168).  · _diff auto-contido viável_
- **`crates/btv-store/src/prompt_library.rs:131`** _(sev media)_ — `row_to_prompt` engole erros de parse de `fields`/`tags` com `unwrap_or(Value::Null)`/`unwrap_or_default()`, transformando dado corrompido em silêncio (perda invisível).
  - **Correção:** Propagar o erro convertendo o parse em `rusqlite::Error` (ex.: `FromSqlConversionFailure`) ou ao menos logar/telemetrar a linha corrompida, em vez de degradar silenciosamente para Null/vazio.  · _estrutural / requer contexto_
- **`crates/btv-store/src/telemetry.rs:214`** _(sev media)_ — O handle Telemetry usa `.lock().expect("...poisoned")` embora o contrato documentado (linhas 208-209) prometa que falhas de telemetria nunca quebram o caminho principal.
  - **Correção:** Um mutex envenenado (panic de outra thread segurando o lock) fara `record`/`recent`/`summary`/etc. entrar em panic, violando a promessa. Recupere o guard com `.lock().unwrap_or_else(|e| e.into_inner())` para nunca propagar o poison ao caminho principal.  · _diff auto-contido viável_
- **`crates/btv-tools/src/lsp.rs:321`** _(sev media)_ — ensure_open envia didOpen apenas uma vez por URI (set opened) e nunca emite didChange; se o arquivo mudar em disco entre consultas, o servidor mantem o texto da versao 1 e definition/references/diagnostics retornam contra conteudo obsoleto.
  - **Correção:** Detectar mudanca (mtime ou hash do texto lido em read_file) e emitir textDocument/didChange com nova versao antes da consulta, ou didClose+didOpen quando o conteudo divergir do ja aberto.  · _estrutural / requer contexto_
- **`crates/btv-tools/src/skill.rs:166`** _(sev media)_ — stdout/stderr so sao lidos DEPOIS que o processo termina; uma skill que emite mais que a capacidade do pipe (~64KB) bloqueia no write, nunca sai, e o loop sempre estoura no timeout.
  - **Correção:** Drenar stdout e stderr concorrentemente enquanto se espera o processo (threads de leitura, ou wait_with_output com timeout em thread separada) em vez de ler apenas apos o exit.  · _diff auto-contido viável_
- **`crates/btv-verify/src/vetter.rs:325`** _(sev media)_ — list_skill_statuses chama vet_skill para cada subdir, e vet_skill executa os `[[verify]]` do manifesto (run_pipeline lança programas externos) — ou seja, apenas LISTAR status na tela admin pode executar comandos declarados pela skill.
  - **Correção:** Para o caminho de listagem/status, rodar só as checagens estáticas (dangerous patterns + permission mismatch + presença de manifesto), pulando run_pipeline; ou expor um modo `vet_skill_static` sem execução de verify_steps.  · _diff auto-contido viável_
- **`python/packages/btv-squad/src/btv_squad/agents/developer.py:200`** _(sev media)_ — float(action.get('confidence', 0.0)) sobre saida nao confiavel do LLM lanca ValueError nao capturado se o modelo devolver confidence nao-numerico (ex.: 'high'), quebrando o loop ReAct.
  - **Correção:** Encapsular a conversao num helper defensivo (try/except ValueError/TypeError → 0.0) e usa-lo aqui e em _parse_result (linha 290), preservando o contrato de status='incomplete' em vez de propagar exceção.  · _diff auto-contido viável_
- **`python/packages/btv-squad/src/btv_squad/planning.py:190`** _(sev media)_ — replan_from_point indexa step["step"] e failed_step["step"] diretamente, mas os steps vêm do parse do LLM (_parse_decomposition retorna parsed.get("steps", []) sem validar a chave "step").
  - **Correção:** Usar step.get("step") com fallback (ou normalizar os steps em _parse_decomposition atribuindo "step" sequencial) antes de comparar, evitando KeyError num replan sobre um plano cujo LLM omitiu a chave.  · _diff auto-contido viável_
- **`python/packages/btv-squad/src/btv_squad/security.py:48`** _(sev media)_ — A checagem de padrões proibidos roda regex sobre str(params), o repr do dict, o que é impreciso (aspas/chaves afetam o match) e trivialmente contornável.
  - **Correção:** Percorrer os valores string de params recursivamente e aplicar os padrões a cada valor, em vez de casar contra o repr do dict; mantém a natureza de defesa-em-profundidade mas reduz falsos negativos/positivos.  · _estrutural / requer contexto_
- **`python/packages/btv-squad/src/btv_squad/server.py:197`** _(sev media)_ — O channel gRPC é criado na linha 197, mas o try/finally que o fecha só começa na 225; se AgentMemorySystem() ou UnifiedOrchestrator(...) (linhas 203-213) levantarem, o channel vaza sem close().
  - **Correção:** Mover a criação do channel para dentro do bloco try (ou envolver a construção do orquestrador no try já existente) de modo que o finally com channel.close() cubra toda a vida do channel.  · _diff auto-contido viável_
- **`web/src/components/screens/user/Sessao.tsx:181`** _(sev media)_ — O painel CONTEXTO exibe valores estáticos fabricados ("época 2 · compaction 1×", "janela 14k/200k · 7%", barra fixa em 7%) que não vêm de estado real da sessão — viola o princípio "Nada Fake" do projeto.
  - **Correção:** Ligar esses números ao estado real da sessão/contexto ou, se ainda não houver fonte, rotular explicitamente como placeholder/em breve como as demais telas fazem, em vez de apresentar métricas fixas como reais.  · _estrutural / requer contexto_

## 3. Todos os arquivos que precisam de alteração (por módulo)

Lista completa — cada arquivo com achado, seus achados e recomendações. `[A]` = diff auto-contido
viável; `[E]` = estrutural/precisa de contexto.

### `` — 3 arquivo(s), 3 achado(s)

**`/home/user/btv/crates/btv-tools/src/read.rs`**
- L43 · _baixa_ · Big-O [E] — Apesar de suportar offset/limit para 'arquivos grandes', o arquivo inteiro e carregado em memoria via read_to_string antes de pular/limitar linhas, negando o beneficio de memoria do offset/limit.
  → Ler com BufReader linha a linha, aplicando skip(offset-1).take(limit) sobre o iterador de linhas, evitando materializar todo o conteudo para leituras parciais.

**`/home/user/btv/python/packages/btv-squad/tests/test_designer.py`**
- L67 · _media_ · correcao [A] — O teste usa try/except com `assert False` para verificar que RuntimeError e levantado, padrao fragil (se execute nao levantar, a mensagem de falha vem do assert dentro do try, nao do fluxo esperado).
  → Substituir por `with pytest.raises(RuntimeError, match="attach_gateway"): asyncio.run(agent.execute(...))`, eliminando o assert-False manual.

**`/home/user/btv/web/src/components/screens/user/Modelo.tsx`**
- L22 · _baixa_ · DRY [A] — Os dois botoes transparentes (tier e agente) repetem quase o mesmo objeto de estilo inline (background transparent, border none, textAlign left, width 100%, color var(--ink)).
  → Extrair uma constante de estilo (ex.: const cardButtonStyle) ou um pequeno componente CardButton e reusar nas linhas 22 e 45.


### `btv-cli` — 15 arquivo(s), 44 achado(s)

**`crates/btv-cli/src/btv_agent.rs`**
- L161 · _media_ · SRP [E] — ativar_squad_handler tem ~243 linhas fazendo lookup, filtragem de papeis, montagem de roster/hashes, start da task, persistencia, releitura, evento e ledger num unico corpo.
  → Extrair sub-funcoes puras/testaveis (ex.: montar_roster, montar_prompt_hashes, persistir_run_e_evento) para reduzir o handler a orquestracao, seguindo o padrao ja usado com montar_descricao/registrar_entregas.
- L285 · _media_ · correcao [E] — Apos start_squad_task ter iniciado a squad, as falhas seguintes (save, get None, activation_event) retornam erro sem parar a task iniciada nem spawnar o watcher, deixando um run orfao rodando sem persistencia nem transicao de status.
  → Em cada caminho de erro apos o start (linhas 339, 348, 370), sinalizar o hub para encerrar a task (kill-switch) antes de retornar, ou spawnar o watcher/persistir de forma que o run nao fique orfao.
- L918 · _media_ · correcao [A] — list_overrides(...).unwrap_or_default() engole erro de banco: uma falha de leitura vira 'sem overrides', e o handler responde HTTP 200 com editado:false, representando incorretamente o estado das personas.
  → Tratar o Err (retornar 500 store_error como nos demais handlers de leitura, ou ao menos logar via eprintln) em vez de mascarar como HashMap vazio; o mesmo vale para list_custom na linha 942.
- L214 · _baixa_ · correcao [A] — Na ativacao, list_overrides/list_custom com unwrap_or_default silenciam erro de storage: uma leitura de personas que falha faz a squad ativar com prompts padrao e procedencia (hashes) sem os overrides, sem qualquer sinal.
  → Logar a falha (eprintln como nos caminhos de ledger) antes de degradar para default, para que a perda de override na procedencia nao seja silenciosa.
- L505 · _baixa_ · correcao [A] — serde_json::from_str(&run.papeis_json).unwrap_or_default() em papeis_json malformado produz Vec vazio, gerando trilha de procedencia '· N gate(s)' sem papeis e uma entrega com trilha degradada silenciosamente.
  → Logar quando o parse falhar (ou usar um fallback explicito) para nao registrar entrega com procedencia vazia sem sinal.
- L588 · _baixa_ · DRY [A] — O bloco (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorBody::new("store_error", e.to_string()))).into_response() se repete ~15 vezes ao longo do arquivo.
  → Extrair um helper fn store_error(e: impl Display) -> Response (e talvez not_found/unprocessable) e reutilizar nos handlers, reduzindo ruido e risco de divergencia de mensagem.

**`crates/btv-cli/src/btv_agent_golden.rs`**
- L438 · _media_ · DRY [A] — A logica de parsear o corpo da resposta (bytes vazios -> Null, senao from_slice com fallback para {text: lossy}) esta duplicada byte-a-byte em drive (68-73) e reqwest_step (438-443).
  → Extrair um helper `fn parse_body(bytes: &[u8]) -> serde_json::Value` e chamar dos dois pontos, removendo a duplicacao.
- L466 · _media_ · SRP [A] — golden_squad_activation tem ~320 linhas e mistura orquestracao HTTP, setup de servidor, disparo de multiplos emissores legados (C3.2/C3.3/C3.4) e cinco blocos de assercoes de ledger/store, dificultando manutencao.
  → Extrair helpers para os grupos de assercao (ex.: assert_ledger_bodies(&entradas), assert_run_state(&store)) e para a bateria de 'emissores legados', deixando o corpo do teste como sequencia legivel de passos.
- L375 · _baixa_ · clean-code [A] — wait_hitl faz polling com numeros magicos (0..900 x 200ms = 180s) e recria uma subscription do hub a cada iteracao dentro do loop.
  → Nomear timeout/intervalo em consts e subscrever uma vez fora do loop (ou usar o rx retornado) em vez de chamar hub.subscribe a cada iteracao.

**`crates/btv-cli/src/cache.rs`**
- L53 · _baixa_ · correcao [A] — `self.cache.lock().unwrap()` (linhas 53 e 84) faz panic se o Mutex estiver envenenado, num caminho de producao onde a filosofia declarada e que falhas de cache nunca derrubam o fluxo principal.
  → Tratar o `PoisonError` (ex.: `lock().map(|g| ...).ok()` ou `into_inner` no poison) e degradar para cache-miss em vez de panicar, coerente com o `let _ =` do put logo abaixo.

**`crates/btv-cli/src/convert.rs`**
- L155 · _baixa_ · correcao [E] — to_pdf gera sempre 1 pagina com Td fixo a partir de 760pt; textos com muitas linhas transbordam a MediaBox sem paginacao nem nota de honestidade.
  → Documentar a limitacao no doc do modulo (que ja se propoe honesto) ou quebrar em multiplas paginas quando o cursor Y ultrapassa a margem inferior.
- L208 · _baixa_ · naming [A] — Comentario diz 'Tabela ... calculada uma vez' mas nao ha tabela precomputada; o CRC e recalculado bit-a-bit por byte.
  → Corrigir o comentario para descrever o algoritmo bitwise real (sem lookup table), ou introduzir de fato uma tabela estatica de 256 entradas se performance importar.

**`crates/btv-cli/src/main.rs`**
- L342 · _media_ · SRP [E] — run_dashboard mistura, em ~120 linhas, criacao de stores, seed do task_seq, reconciliacao de runs orfas, montagem de ~8 routers e o serve — varias responsabilidades num so corpo.
  → Extrair a montagem do extra_router (blocos 403-437) e o bloco de seed/reconcile (389-402) para funcoes auxiliares, deixando run_dashboard como orquestrador enxuto.
- L876 · _media_ · SRP [E] — handle_prompt_command concentra o dispatch de 7 subcomandos (list/library/use/fav/rm/save/render) em ~170 linhas com aninhamento profundo e retornos precoces.
  → Extrair uma funcao por subcomando (cmd_library, cmd_use, cmd_fav, cmd_rm, cmd_save, cmd_render) e reduzir handle_prompt_command a um roteador que faz match no first_token.
- L247 · _baixa_ · correcao [E] — db_path.to_str().unwrap_or(".btv/telemetry.db") faz um path --db nao-UTF8 cair silenciosamente num banco diferente do que o usuario pediu, mascarando o erro em vez de reporta-lo.
  → Usar to_str().context("caminho de db nao e UTF-8 valido")? para falhar explicitamente quando o path fornecido nao converte, em vez do fallback silencioso (mesmo padrao repetido nas linhas 349-361, 366, 383, 591, 596, 636, 798).
- L623 · _baixa_ · Clean Code [A] — max_steps: 20 e max_tokens: 4096 sao magic numbers embutidos direto na construcao do AgentLoop.
  → Extrair para constantes nomeadas (ex.: const DEFAULT_MAX_STEPS/DEFAULT_MAX_TOKENS) ou expor via RunOpts, documentando a escolha.
- L641 · _baixa_ · DRY [A] — open_durable reimplementa inline o calculo de nanos desde UNIX_EPOCH que ja existe na funcao nanos_now (linha 554).
  → Trocar o bloco SystemTime inline por 'format!("s{:x}", nanos_now() & 0xffff_ffff_ffff)', reutilizando nanos_now.
- L934 · _baixa_ · DRY [A] — Os ramos use/fav/rm repetem o mesmo bloco 'command_arg.parse::<i64>()' + mensagem de uso quase identica.
  → Extrair um helper 'parse_id(command_arg, uso: &str) -> Option<i64>' que imprime o erro de uso e retorna None, reutilizado pelos tres ramos.

**`crates/btv-cli/src/mcp_console.rs`**
- L72 · _media_ · Big-O [A] — Os servidores MCP são sondados sequencialmente no for-loop com await por iteração, então N servidores lentos custam até N*5s de latência total.
  → Disparar todos os probes (`spawn_blocking` + `timeout`) e coletar com `futures::future::join_all`/`try_join_all` para que o pior caso seja ~5s independente do número de servidores; a ordem final pode ser reordenada por id se necessário.

**`crates/btv-cli/src/session.rs`**
- L27 · _baixa_ · correcao [E] — to_str().unwrap_or(".btv/btv.db") cai num caminho RELATIVO se o path do workspace não for UTF-8, abrindo o ledger em local diferente do pretendido.
  → Usar a API que aceita Path/OsStr (ou `dir.join("btv.db")` diretamente) em vez de converter para &str com fallback textual, evitando escrever num db no cwd errado. Mesma questão em append_entry_impl (linha 115).

**`crates/btv-cli/src/sidecar.rs`**
- L14 · _baixa_ · 12-Factor [A] — O timeout de subida do sidecar e fixo em 8s no codigo, sem override por ambiente.
  → Ler START_TIMEOUT de uma env opcional (ex.: BTV_SIDECAR_TIMEOUT_SECS) com fallback para 8s, permitindo ajuste em maquinas lentas/CI sem recompilar.

**`crates/btv-cli/src/skills.rs`**
- L102 · _media_ · SRP [A] — load_skills tem ~86 linhas concentrando leitura de diretório, vetting, montagem de status, checagem de colisão, logging e registro, com aninhamento profundo (loop + match + múltiplos continue).
  → Extrair o corpo do loop (do vet_skill até o register) para um helper `register_one_skill(...) -> Option<SkillStatus registrado>`, mantendo load_skills como orquestrador curto.
- L129 · _baixa_ · efficiency [E] — O manifesto é lido/parseado duas vezes por skill: uma dentro de vet_skill e outra em read_manifest logo em seguida.
  → Fazer vet_skill devolver o SkillManifest já parseado no VettingResult (campo opcional) para evitar o reparse; o comentário já reconhece o custo como aceito, mas a interface pode eliminá-lo.

**`crates/btv-cli/src/squad.rs`**
- L74 · _baixa_ · DRY [A] — core_run_tool faz tres lookups separados de tools.get(&call.tool) (linhas 67, 74 e 102) com um expect('validado acima') para reconciliar o guard anterior.
  → Resolver o tool uma unica vez com `let Some(tool) = tools.get(&call.tool) else { return ToolResult{...} };` e reusar a referencia (clonando o que for necessario para o spawn_blocking), eliminando o expect e as buscas repetidas na HashMap.
- L141 · _baixa_ · naming [A] — O doc-comment de evidence_to_proto comeca descrevendo 'Scope de um ToolCall ... trilha de entregas do BTV', texto que pertence a tool_scope (linha 185) e nao a conversao de evidencia.
  → Mover as duas primeiras frases (sobre scope/correlacao de execucoes) para o doc de tool_scope e deixar em evidence_to_proto apenas a descricao da conversao evidencia->proto.
- L267 · _baixa_ · magic-number [A] — max_tokens default 4096 aparece hardcoded inline no GenerateRequest.
  → Extrair para uma const nomeada (ex.: `const DEFAULT_MAX_TOKENS: u32 = 4096;`) documentando a origem do valor.
- L377 · _baixa_ · clean-code [A] — Espera do core_sock usa numeros magicos (0..100 iteracoes x 20ms) e ignora silenciosamente o caso do socket nunca aparecer (segue para try_squad, que so falha depois no connect).
  → Nomear os limites em consts (tentativas/intervalo) e, apos o loop, logar/registrar explicitamente que o socket do CoreService nao subiu no prazo antes de prosseguir para o fallback.

**`crates/btv-cli/src/squad_agent.rs`**
- L345 · _media_ · DRY [A] — A construcao de um SquadEvent de erro (task_id/ts/Error/tenant_id/actor) esta duplicada em tres lugares.
  → Existe `chat_event` como helper mas nenhum equivalente para erro. Extrair `fn error_event(task_id: &str, reason: String) -> SquadEvent` e usa-lo em emergency_stop (L345-356) e run_squad_task (L549-558), eliminando os literais repetidos de tenant_id/actor.
- L463 · _media_ · DRY [A] — `request_permission` e `run_tool` sao byte-a-byte identicos entre WebSquadCoreBackend e ScriptedSquadCoreBackend.
  → Extrair uma fn livre compartilhada, ex.: `async fn run_tool_and_note(hub, task_id, tools, perms, root, call)` e `fn request_hitl(hub, task_id)`, chamada pelos dois impls. Reduz ~30 linhas duplicadas (L390-410 vs L459-479) e garante que mudancas de logica de anotacao nao divirjam entre os backends.
- L598 · _media_ · correcao [A] — A task `core_task` (serve_core) so e abortada no caminho de sucesso (linha 704); todos os `?` de early-return depois do spawn a vazam.
  → O `serve_core` roda indefinidamente. Nas saidas antecipadas via `?` (verification_evidence L622-623, ledger open L636, acquire L641, execute_task L665) o `JoinHandle` e apenas dropado, e tasks tokio nao sao canceladas no drop — o servidor continua vivo detached segurando o UDS. Envolver o corpo apos o spawn de forma que `core_task.abort()` rode em todos os caminhos (ex.: extrair o miolo para uma fn interna e abortar no retorno, ou usar um guard que aborta no Drop).
- L279 · _baixa_ · Big-O [A] — `drain_user_messages` adquire e solta o mutex uma vez por mensagem em vez de drenar tudo sob um unico lock.
  → Em vez do loop sobre `take_user_message` (que faz lock/pop/unlock a cada iteracao), pegar o lock uma vez e fazer `std::mem::take(&mut state.inbox).into_iter().collect()` ou `drain(..)`, retornando o Vec.
- L521 · _baixa_ · SRP [A] — run_squad_task e run_squad_task_inner carregam 8 parametros posicionais silenciados por #[allow(too_many_arguments)].
  → Agrupar os campos correlatos (hub, pool, root, task_id, description, model, roster) em um struct `SquadRunParams` passado por valor, removendo o allow e tornando os call sites mais legiveis.
- L599 · _baixa_ · correcao [A] — O loop de espera do socket (100 iteracoes x 20ms) segue em frente silenciosamente se o socket nunca aparecer, com numeros magicos.
  → Se apos as 100 tentativas `core_sock.exists()` ainda for false, retornar um `Err` explicito (ex.: "CoreService local nao subiu em 2s") em vez de prosseguir para o acquire/execute que falhariam de forma menos diagnosticavel. Extrair os literais 100/20ms para constantes nomeadas.

**`crates/btv-cli/src/tenant_extractor.rs`**
- L165 · _baixa_ · correcao [E] — `expect` no caminho de producao (arm Local do resolver_contexto) ao construir o ActorId fixo.
  → O valor e uma constante infalivel, mas para eliminar qualquer panic no caminho de borda, considere validar ACTOR_LOCAL uma unica vez (ex.: LazyLock<ActorId>) no arranque, ou documentar a invariante como teste de const. Baixo impacto pois o input e fixo.

**`crates/btv-cli/src/tui_app.rs`**
- L336 · _media_ · correcao [A] — Combinações Ctrl+<tecla> não tratadas (qualquer coisa além de c/m/g) caem no braço `_ => {}` sem `continue` e escorregam para o segundo match, inserindo a letra no input (ex.: Ctrl+B digita 'b'; Ctrl+Backspace apaga).
  → Adicionar `continue;` após o bloco de CONTROL (ou no braço `_ => {}`) para consumir a tecla e não deixá-la cair no tratamento de Char/Backspace normal.
- L153 · _baixa_ · SRP [E] — run_tui mistura setup do sidecar, a task inteira do loop de agente (closure de ~110 linhas) e o loop de UI/eventos num só corpo de ~250 linhas.
  → Extrair a closure da task do agente para uma função `spawn_agent_task(...)` e o laço de teclado para um handler dedicado, reduzindo o corpo e as responsabilidades cruzadas.

**`crates/btv-cli/src/web_agent.rs`**
- L194 · _media_ · correcao [E] — Sessoes nunca sao removidas do HashMap e o `log` por sessao cresce sem limite; `max_sessions` vira um teto permanente do processo e a memoria nunca e liberada.
  → Adicionar remocao/expiracao de sessoes concluidas (ou LRU) e limitar/truncar `SessionState::log` (ring buffer ou cap), para que o teto reflita sessoes vivas e nao acumulado historico do processo.
- L492 · _media_ · correcao [A] — `durable.persist_new().unwrap_or(0)` engole falha de persistencia da sessao duravel; o usuario recebe `done` mesmo se o historico da conversa nao foi salvo.
  → Propagar ou ao menos logar/publicar a falha de `persist_new()` (ex.: converter em `SessionEvent::Error` ou `?`), em vez de descartar o `Result` com `unwrap_or(0)`.
- L556 · _media_ · DRY [A] — `std::env::current_dir()` e repetido em 5 handlers (send_message/set_rule/list_rules/revoke_rule/get_matrix), acoplando as rotas ao cwd global do processo e ignorando o `root` ja configurado em `serve_with_agent`.
  → Guardar o `root` do workspace em `WebAgentState` (fonte unica, passada por `serve_with_agent`) e usa-lo nos handlers, eliminando a dependencia de cwd global e o boilerplate de erro `cwd_error` duplicado.
- L989 · _media_ · correcao [E] — Erros `Lagged` do broadcast sao descartados via `.filter_map(|r| r.ok())`, entao um cliente SSE lento perde eventos silenciosamente sem qualquer sinal de gap.
  → Tratar `BroadcastStreamRecvError::Lagged(n)` explicitamente emitindo um evento de sinalizacao (ex.: um `SessionEvent` de aviso ou forcando reconexao/snapshot) em vez de silenciosamente dropar; ou aumentar/monitorar a capacidade do canal (256) conforme a taxa de eventos.
- L187 · _baixa_ · correcao [E] — O padrao `.lock().expect("session hub mutex poisoned")` repete-se em ~7 metodos; um panic em qualquer secao critica derruba todas as sessoes futuras via poison em cascata.
  → Centralizar o acesso ao lock num helper que recupere de poison (`lock().unwrap_or_else(|e| e.into_inner())`) ou trate o poison de forma nao-fatal, ja que o estado protegido e recuperavel.
- L549 · _baixa_ · 12-Factor [A] — Modelo default `"claude-sonnet-5"` e limites (`max_steps: 30` L400, `context_window: 200_000` L554, `max_tokens: 4096` L401) sao hardcoded, ao contrario de `default_hub` que ja le env vars.
  → Extrair para constantes nomeadas e/ou ler de env var (ex.: `BTV_DEFAULT_MODEL`, `BTV_MAX_STEPS`) no mesmo padrao de `default_hub`.
- L632 · _baixa_ · correcao [A] — `path.to_str().unwrap_or(".btv/rules.db")` faz um path nao-UTF8 cair silenciosamente num caminho relativo diferente do pretendido, abrindo/gravando regras no lugar errado.
  → Usar a API que aceita `&Path` diretamente, ou retornar erro quando `to_str()` for `None`, em vez de um fallback relativo silencioso.

**`crates/btv-cli/tests/verify_cli.rs`**
- L38 · _media_ · DRY [A] — O bloco tempdir + write_btv_toml com o passo `name="ok" program="true"` está repetido literalmente em 4 testes (linhas 38, 69, 95, 169).
  → Extrair um helper como `fixture_ok_step() -> TempDir` (ou `write_ok_toml(dir)`) que encapsule o TOML de passo único, reduzindo a duplicação grosseira entre os testes.


### `btv-contract` — 1 arquivo(s), 1 achado(s)

**`crates/btv-contract/src/lib.rs`**
- L76 · _baixa_ · DRY [A] — A string mágica "sq1" (chave derivada de task_id seq=1) aparece dezenas de vezes acoplando os testes à convenção interna de derivação de chave do adapter.
  → Extrair um helper como `key_for(seq: u64) -> String` (ou constante `SQ1`) para centralizar a convenção sq{seq} e evitar quebra em massa se a regra mudar.


### `btv-core` — 1 arquivo(s), 2 achado(s)

**`crates/btv-core/src/agent_loop.rs`**
- L122 · _baixa_ · Big-O [E] — A cada passo do loop messages.clone() copia o historico inteiro para montar o GenerateRequest; em conversas longas com muitos passos isso e O(passos x tamanho_historico).
  → Se/quando o LlmPort permitir, aceitar messages por referencia/Cow no GenerateRequest para evitar a copia total por iteracao; enquanto a interface exigir owned, documentar o custo. Baixa prioridade por ser inerente ao contrato atual.
- L175 · _baixa_ · naming [A] — run_tool devolve um tuplo cru (String, bool) documentado so por comentario, e o chamador acessa result.0/result.1 (linhas 160-164) com indices magicos que escondem o significado (conteudo/is_error).
  → Trocar o retorno por uma pequena struct ToolExecResult { content: String, is_error: bool } (ou um alias nomeado), tornando o call-site auto-documentado sem comentario.


### `btv-domain` — 1 arquivo(s), 2 achado(s)

**`crates/btv-domain/src/ports.rs`**
- L321 · _baixa_ · Clean Code [E] — Run::activate recebe 8 parametros posicionais (marcado com allow(too_many_arguments)), varios do mesmo tipo String, o que torna a chamada propensa a troca de argumentos.
  → Agrupar os insumos da ativacao (template_id/template_versao/nome/briefing_json/roles/ts) num struct ActivationInput e passar por referencia, eliminando o allow e tornando a chamada nomeada/segura.
- L767 · _baixa_ · DRY [A] — O bloco de carregar+parsear a fixture wire-strings.v1.json (env!(CARGO_MANIFEST_DIR).join(...) + from_str/expect) esta duplicado quase identico em dois testes (linhas 767-774 e 933-940).
  → Extrair um helper de teste fn load_wire_fixture() -> serde_json::Value e reutilizar nos dois testes, evitando divergencia de caminho/parse.


### `btv-golden` — 1 arquivo(s), 2 achado(s)

**`crates/btv-golden/src/lib.rs`**
- L200 · _media_ · DRY [E] — `walk` (linhas 200-231) e `replace_leaf` (233-260) duplicam quase integralmente o mesmo match de 4 casos (`*`+Array, `*`+Object, key+Object, key+outro), diferindo so na acao no no folha.
  → Unifique numa unica travessia parametrizada (ex.: uma funcao recursiva que recebe se e folha, ou um closure de acao aplicado no fim do caminho), eliminando a duplicacao dos ramos de navegacao/erro.
- L103 · _baixa_ · Clean Code [E] — `step` tem 9 parametros posicionais (`#[allow(clippy::too_many_arguments)]`), varios do mesmo tipo Option<String>/u16, propensos a troca acidental de argumentos.
  → Agrupe em structs coesos (ex.: um `GoldenReqSpec { method, path, body }` e um `GoldenRespSpec { status, content_type, content_disposition, body }`) para tornar as chamadas auto-documentadas. Muda a assinatura publica, entao coordene com os call sites.


### `btv-llm` — 6 arquivo(s), 9 achado(s)

**`crates/btv-llm/src/anthropic.rs`**
- L164 · _baixa_ · correcao [E] — JSON de input de tool malformado no stream vira objeto vazio silenciosamente via unwrap_or.
  → Ao falhar o parse de `json` do tool_use, preservar o texto bruto ou anexar um marcador de erro no bloco em vez de descartar para `{}` — hoje um input truncado/inválido some sem sinal, e o agente executa a tool com argumentos vazios.

**`crates/btv-llm/src/gateway.rs`**
- L54 · _media_ · correcao [A] — Se o builder do cliente com timeouts falhar, o fallback `reqwest::Client::new()` reintroduz exatamente o cliente SEM timeouts que o comentário do módulo diz causar travamento eterno.
  → Em vez de cair para um cliente sem timeouts, propagar o erro ou construir o fallback ainda com os timeouts default (connect/read) para nunca reabrir a janela de hang que a função existe para fechar.
- L112 · _baixa_ · DRY [A] — A URL do provider (`{base}/v1/messages` e `{base}/v1/chat/completions`) é formatada duas vezes em cada braço do match.
  → Computar `url` uma vez por braço e reutilizá-la no `.post(url.clone())`, evitando duas construções idênticas da mesma string.

**`crates/btv-llm/src/openai.rs`**
- L201 · _baixa_ · correcao [E] — Arguments de tool_call mal-formados sao silenciosamente convertidos em objeto vazio via unwrap_or(Value::Object(default)), engolindo o erro de parse sem sinal.
  → Manter o fallback mas registrar o descarte (log/trace) ou propagar um StopReason::Other/erro, para que argumentos corrompidos nao virem chamada de ferramenta silenciosamente vazia.

**`crates/btv-llm/src/rate_limit.rs`**
- L73 · _media_ · correcao [A] — Com max_requests=0 (construtor publico `new` aceita), `ts.len() < 0` e falso e o codigo cai no `ts.front().expect(...)` sobre um deque vazio, causando panic em vez de rejeitar a chamada.
  → Guardar max_requests==0 explicitamente (retornar RateLimitError ou tratar como 'nunca ha vaga') antes do acesso a front(), ou documentar/validar max_requests>=1 no construtor.
- L49 · _baixa_ · 12-Factor [E] — Os tetos por tier (60/30/15) e a janela de 600s estao hardcoded, sem override por env/config.
  → Permitir configuracao dos limites por env var (com esses valores como default) para ajustar a salvaguarda de custo sem recompilar.

**`crates/btv-llm/src/scripted.rs`**
- L84 · _media_ · Big-O [A] — O guard do Mutex é mantido durante a chamada de on_delta, serializando todas as chamadas concorrentes de generate — justamente no gerador feito para load-test/bench concorrente.
  → Clonar o turno e liberar o lock (drop do guard ou escopo) ANTES de invocar on_delta, para não segurar o mutex enquanto o callback roda.
- L78 · _baixa_ · correcao [A] — expect em mutex envenenado num tipo público usado fora de teste (endpoint/load-test) causa panic em vez de recuperar.
  → Recuperar com unwrap_or_else(|e| e.into_inner()) já que os turnos são somente-leitura, evitando propagar o envenenamento.

**`crates/btv-llm/src/sse.rs`**
- L27 · _media_ · correcao [A] — O comentario afirma tolerar terminador '\r\n\r\n', mas find("\n\n") nao casa CRLF puro, entao um evento SSE com quebras CRLF nao e emitido ate chegar bytes extras.
  → Normalizar CRLF antes de procurar o terminador (ex.: substituir \r\n por \n no buffer) ou procurar tambem por "\r\n\r\n" ao delimitar blocos, para servidores que usam CRLF funcionarem sem depender de dados adicionais.


### `btv-promptforge` — 1 arquivo(s), 1 achado(s)

**`python/packages/btv-promptforge/src/btv_promptforge/lint.py`**
- L48 · _baixa_ · correcao [A] — A deteccao de termo vago casa apenas com termo precedido por espaco (`f" {term}"`), perdendo ocorrencias apos pontuacao ou inicio pos-quebra de linha (ex.: '(otimizado').
  → Usar limite de palavra via regex (`re.search(rf"\\b{re.escape(term)}\\b", lowered)`) em vez de concatenar espaco, para casar em qualquer fronteira de palavra.


### `btv-review` — 3 arquivo(s), 3 achado(s)

**`python/packages/btv-review/src/btv_review/reviewers.py`**
- L27 · _baixa_ · Clean Code [A] — Penalidades de severidade e o piso conservador (0.05) são magic numbers dispersos entre a constante e o max(0.0, ...).
  → Documentar/constante nomeada para o piso padrão de finding desconhecido (ex.: _DEFAULT_PENALTY = 0.05) usado em security_score, em vez de literal inline.

**`python/packages/btv-review/src/btv_review/score.py`**
- L30 · _baixa_ · correcao [A] — is_approved usa comparacao estrita '>' contra APPROVAL_THRESHOLD, rejeitando o score exatamente no limiar (0.7).
  → Se o limiar deve ser inclusivo (aprovar em 0.7), trocar por '>='. Se a exclusao no limite for intencional, documentar explicitamente no docstring.

**`python/packages/btv-review/tests/test_certification.py`**
- L49 · _baixa_ · DRY [A] — O literal ReviewScores(technical=0.9, performance=0.9, security=0.9, value=0.9) e repetido identicamente em dois testes.
  → Extrair um pequeno helper ou constante de modulo (ex.: _SCORES_ALTOS) e reutilizar nos dois testes.


### `btv-schemas` — 5 arquivo(s), 7 achado(s)

**`crates/btv-schemas/src/review.rs`**
- L50 · _baixa_ · clean-code [A] — As penalidades por severidade (0.4/0.1/0.05) são magic numbers embutidos no match.
  → Extrair para constantes nomeadas (ex.: `PENALTY_CRITICAL`, `PENALTY_WARNING`, `PENALTY_OTHER`) espelhando `SECURITY_FLOOR` logo acima.
- L63 · _baixa_ · clean-code [A] — O valor neutro `0.5` usado para `technical`/`security` quando não há passos é um magic number repetido sem nome.
  → Extrair para uma constante como `NEUTRAL_SCORE: f64 = 0.5` documentando o significado de "sem sinal".

**`crates/btv-schemas/src/squad_template.rs`**
- L68 · _baixa_ · DRY [E] — A checagem de `onda` fora de 1..=3 duplica a validação que o doc afirma já estar no JSON Schema (regex/limites de onda).
  → Confirmar se o schema já garante 1..=3 no parse; se sim, remover a checagem duplicada de `validate` (ou documentar por que a redundância é intencional).
- L111 · _baixa_ · clean-code [A] — O closure faz `let t: SquadTemplate = ...; t` com binding intermediário desnecessário só para anotar o tipo.
  → Substituir por `.map(|src| serde_json::from_str::<SquadTemplate>(src).expect(...))` eliminando o binding `t` redundante.

**`crates/btv-schemas/src/verification.rs`**
- L21 · _baixa_ · naming [E] — `Finding.severity` é `String` livre, embora o domínio tenha um conjunto fechado de severidades (info/warn/error etc.), perdendo type-safety.
  → Considerar um enum `Severity` serializado com serde (aditivo ao contrato) ou, se a flexibilidade é intencional pelos parsers externos, documentar os valores válidos no doc-comment.

**`crates/btv-schemas/src/workflow.rs`**
- L60 · _baixa_ · correcao [E] — validate_edges retorna Result<(), String> em vez de um erro tipado, destoando do padrão thiserror do projeto para erros de lib.
  → Considerar um enum de erro (thiserror) com variantes FromInexistente/ToInexistente carregando o id, permitindo ao chamador mapear para 422 sem inspecionar a string.

**`crates/btv-schemas/tests/schema_fixtures.rs`**
- L50 · _media_ · DRY [A] — O bloco schema()/fixture()/validator_for()/is_valid(valid)/is_valid(invalid) repete-se quase identico em 8+ testes, com a mesma mensagem de erro.
  → Extrair um helper (ex.: `fn assert_schema(name: &str, invalid_keys: &[&str]) -> Value` que compila o validador, afirma o caso `valid` e cada `invalid_*`, devolvendo o doc para o parse especifico) e deixar em cada teste so a parte de desserializacao/asserts de negocio.


### `btv-server` — 4 arquivo(s), 4 achado(s)

**`crates/btv-server/src/doctor_console.rs`**
- L125 · _media_ · correcao [A] — vocabulario_check faz stores.btv.lock().unwrap() (e .ledger.lock().unwrap() na 135) no caminho do handler; um mutex envenenado por panic em outra thread derruba a request.
  → Tratar o PoisonError devolvendo um DoctorCheck{ok:false, detail: ...} em vez de unwrap(), mantendo o handler resiliente a lock envenenado.

**`crates/btv-server/src/handlers/mod.rs`**
- L59 · _media_ · naming [E] — O helper generico db_error, usado por todas as areas (telemetria, ledger, admin, etc.), fixa o code de erro como 'prompt_library_error', rotulando erros de DB nao relacionados a biblioteca de prompts.
  → Aceitar o code como parametro (ex.: db_error(code, message)) ou usar um code neutro como 'db_error'/'internal_error' para o helper compartilhado.

**`crates/btv-server/src/handlers/telemetria.rs`**
- L23 · _baixa_ · Clean Code [A] — Limite default de eventos (`unwrap_or(50)`) é um magic number no handler.
  → Extrair para uma const nomeada (ex.: `const DEFAULT_EVENTS_LIMIT: u32 = 50;`).

**`crates/btv-server/src/lib.rs`**
- L263 · _media_ · DRY [A] — Quase todos os ~25 testes repetem literalmente o bloco `fixture_web_dir()` + `router(Telemetry::open_in_memory().unwrap(), prompt_library_vazia(), ledger_vazio(), web_dir.path(), web_dir.path())`, com pequenas variacoes de qual argumento e customizado.
  → Extrair um helper de teste, ex. `fn app_padrao() -> Router` e/ou `fn app_com(telemetry, ledger, root)` que encapsule o `fixture_web_dir()` e a chamada de `router(...)`, mantendo o TempDir vivo; os testes passam a construir o app numa linha e so sobrescrevem o eixo relevante.


### `btv-sidecar` — 7 arquivo(s), 9 achado(s)

**`crates/btv-sidecar/src/client.rs`**
- L97 · _baixa_ · naming [A] — Doc de socket_ready afirma que verifica se o socket 'aceita conexao', mas a implementacao so testa path.exists().
  → Alinhar doc e implementacao: ou ajustar o comentario para 'verifica se o arquivo de socket existe', ou de fato tentar um UnixStream::connect de sondagem.

**`crates/btv-sidecar/src/memory_client.rs`**
- L176 · _baixa_ · DRY [A] — O bloco `unsafe { libc::kill(-(pid as i32), SIGKILL) }` está duplicado idêntico entre `kill()` (linha 131) e `Drop::drop` (linha 178).
  → Extrair um helper privado `fn kill_group(child: &Child)` (ou método associado) e chamá-lo em ambos os pontos para uma única fonte da lógica de kill de grupo.

**`crates/btv-sidecar/src/service.rs`**
- L256 · _alta_ · correcao [A] ✓confirmado — Em SquadPool::acquire, se SquadSupervisor::spawn ou wait_ready falham (os `?` nas linhas 256-262), o `permit` do semaforo e liberado no Drop mas o `slot` ja retirado de `self.free` NUNCA volta para a lista de livres — free-list e semaforo saem de sincronia.
  → Nos caminhos de erro de spawn/wait_ready empurrar `slot` de volta para `self.free` antes de retornar (ex.: um guard com Drop que faz push, desarmado so ao construir o SquadLease com sucesso). Caso contrario cada falha de subida do sidecar (Python ausente/timeout) vaza um slot permanentemente e, apos capacity falhas, free.pop() na linha 239 atinge o .expect e derruba o processo.
- L138 · _baixa_ · DRY [E] — SidecarService::client (64-77) e MemoryService::client (138-154) sao logicamente identicos (guard, health-check, respawn, cache) diferindo so nos tipos de supervisor/cliente; SidecarState/MemoryState/SquadSlotState tambem tem a mesma forma.
  → Extrair a politica singleton health-check/respawn para um tipo generico parametrizado por um trait de supervisor (spawn+wait_ready+health), reduzindo a triplicacao; se o custo de abstracao nao compensar, ao menos comentar a intencao de espelhamento.

**`crates/btv-sidecar/src/supervisor.rs`**
- L122 · _baixa_ · DRY [A] — O group-kill via `libc::kill(-(pid as i32), SIGKILL)` esta duplicado literalmente em `kill` (linhas 65-69) e em `Drop::drop` (linhas 124-128).
  → Extrair um helper privado `fn signal_group(pid: u32)` (ou metodo `&self`) e chamar dos dois pontos.

**`crates/btv-sidecar/tests/client_over_uds.rs`**
- L83 · _baixa_ · correcao [A] — Sincronizacao do servidor por sleep fixo de 50ms introduz race/flakiness no teste.
  → Trocar o sleep fixo por poll ativo do socket (loop curto testando UnixStream::connect com backoff) ou por um canal de readiness sinalizado pela task do servidor.

**`crates/btv-sidecar/tests/core_server_inprocess.rs`**
- L131 · _baixa_ · correcao [A] — wait_for_socket retorna silenciosamente após 100 tentativas mesmo sem o socket aparecer; um arranque lento do servidor vira um erro de conexão obscuro em connect_client em vez de uma falha de espera clara.
  → Ao esgotar o loop sem sock.exists(), dar panic!("socket não apareceu a tempo") para o modo de falha do teste ser autoexplicativo.

**`crates/btv-sidecar/tests/squad_e2e.rs`**
- L91 · _media_ · DRY [A] — O bloco spawn do CoreService + loop de 100 iteracoes esperando o socket existir e o match de SquadSupervisor::spawn esta duplicado quase identico nos tres testes (91-114, 260-280, 545-565).
  → Extrair um helper async (ex.: spawn_core_and_wait(backend, prefix) -> (JoinHandle, PathBuf) e wait_supervisor_or_skip(dir, squad_sock, core_sock)) reaproveitado pelos tres testes; o proprio arquivo ja tem spawn_test_core em service.rs como precedente.
- L116 · _baixa_ · DRY [A] — O literal SquadTask (mesmos decision_type/max_autonomy_level/tenant_id/actor) esta repetido nos tres testes com pequenas variacoes de task_id/description/evidence.
  → Criar um builder/funcao helper que devolve um SquadTask default e sobrescreve so os campos que variam por teste.


### `btv-squad` — 23 arquivo(s), 35 achado(s)

**`python/packages/btv-squad/src/btv_squad/agents/architect.py`**
- L68 · _baixa_ · correcao [A] — O default de `reasoning.get("recommendation", plan.get("architecture"))` e codigo morto: `_parse_reasoning` sempre insere a chave `recommendation` (com "" no pior caso), entao o fallback para `architecture` nunca dispara.
  → Trocar por `reasoning.get("recommendation") or plan.get("architecture")` para que uma recommendation vazia caia de fato no estilo arquitetural.
- L143 · _baixa_ · clean-code [E] — `create_plan` e declarada `async` mas nao possui nenhum `await` — nao ha operacao assincrona no corpo.
  → Tornar `create_plan` sincrona e ajustar o `await` no chamador (linha 63), ou documentar explicitamente que o async e reservado para I/O futuro.
- L170 · _baixa_ · clean-code [A] — `create_adr` interpola `trade_offs` (um dict) diretamente no markdown do ADR, produzindo o repr Python do dict (ex.: `{'opcao': 'trade-off'}`) na secao Consequences.
  → Formatar o dict antes da interpolacao (ex.: uma linha `- chave: valor` por item) ou serializar como bullets legiveis.

**`python/packages/btv-squad/src/btv_squad/agents/auditor.py`**
- L251 · _baixa_ · correcao [A] — float(parsed.get('confidence', 0.0)) em _parse_judgment (e linha 258 em _parse_validation) lanca ValueError se o modelo devolver confidence como string nao-numerica (ex.: "high").
  → Extrair um helper _safe_float(v, default) que faz try/except (ValueError, TypeError) e retorna o default, aplicando-o nos dois parsers para manter o fallback honesto de confianca 0.0.

**`python/packages/btv-squad/src/btv_squad/agents/base.py`**
- L25 · _baixa_ · Clean Code [A] — `confidence_threshold = 0.7` é um magic number embutido no __init__.
  → Promover a uma constante de módulo/classe nomeada (ex.: `DEFAULT_CONFIDENCE_THRESHOLD = 0.7`) para documentar a intenção e centralizar o ajuste.

**`python/packages/btv-squad/src/btv_squad/agents/designer.py`**
- L81 · _media_ · DRY [A] — `_parse_design` e uma copia quase literal de `_parse_plan` (ops.py): mesmo regex `_JSON_BLOCK`, mesmo bloco try/except e mesmas mensagens de warning.
  → Compartilhar a extracao/parse do bloco JSON num helper comum (ex.: em agents/base.py) e deixar cada agente apenas mapear seus defaults especificos.

**`python/packages/btv-squad/src/btv_squad/agents/developer.py`**
- L200 · _media_ · correcao [A] — float(action.get('confidence', 0.0)) sobre saida nao confiavel do LLM lanca ValueError nao capturado se o modelo devolver confidence nao-numerico (ex.: 'high'), quebrando o loop ReAct.
  → Encapsular a conversao num helper defensivo (try/except ValueError/TypeError → 0.0) e usa-lo aqui e em _parse_result (linha 290), preservando o contrato de status='incomplete' em vez de propagar exceção.
- L144 · _baixa_ · naming [A] — Numero magico 30 no limiar de cobertura em auto_fix_issues sem constante nomeada explicando o significado.
  → Extrair para uma constante de modulo (ex.: _MIN_COVERAGE_PERCENT = 30) documentando que abaixo disso injeta o TODO de testes.

**`python/packages/btv-squad/src/btv_squad/agents/ops.py`**
- L85 · _media_ · DRY [A] — `_parse_plan` duplica quase integralmente o `_parse_design` de designer.py (mesmo `_JSON_BLOCK`, mesmo try/except, mesmos dois warnings).
  → Extrair um helper compartilhado (ex.: `parse_json_block(raw_text, logger) -> dict` em agents/base.py) e derivar os defaults por agente a partir dele, eliminando a copia.

**`python/packages/btv-squad/src/btv_squad/chains.py`**
- L20 · _baixa_ · correcao [A] — steps e tipado como Iterable e iterado direto; se um gerador for passado, uma segunda execucao do chain percorreria um iteravel ja esgotado.
  → Materializar em lista no __post_init__ (self.steps = list(self.steps)) ou tipar como Sequence[ChainStep] para garantir reiteracao segura.
- L36 · _baixa_ · correcao [A] — O retry entre tentativas usa asyncio.sleep(0), que so cede o loop e nao aplica nenhum backoff.
  → Substituir por um backoff (ex.: await asyncio.sleep(base * 2 ** (attempt - 1))) para nao re-executar imediatamente uma etapa que acabou de falhar, aliviando dependencias externas transitorias.

**`python/packages/btv-squad/src/btv_squad/grpc_clients.py`**
- L30 · _baixa_ · naming [A] — O parametro `channel` dos tres clients nao tem type hint, ao contrario do resto do modulo tipado.
  → Anotar `channel: grpc.aio.Channel` (ou o tipo apropriado) nos tres `__init__` para consistencia e checagem estatica.

**`python/packages/btv-squad/src/btv_squad/memory.py`**
- L104 · _baixa_ · correcao [A] — `recall_similar` sempre devolve `n_results: k` mesmo quando ha menos que `k` casamentos, sugerindo mais resultados do que existem.
  → Retornar `n_results: len(ranked)` (numero real de matches) em vez do teto `k` solicitado.

**`python/packages/btv-squad/src/btv_squad/orchestrator.py`**
- L165 · _media_ · SRP [E] — execute_complex_task (~140 linhas) faz recall, planejamento, consenso, emissão de eventos, narração de chat, gate HITL, execução, fail-closed de verificação, validação, memória e aprendizado num só método.
  → Extrair etapas em métodos privados (ex.: _run_consensus_gate, _resolve_final_validation) para reduzir o corpo e tornar o fluxo testável por partes.
- L383 · _baixa_ · magic-number [A] — Limiar de qualidade `< 0.6` que dispara replan está hardcoded no meio do laço, sem nome nem config.
  → Extrair para uma constante nomeada (ex.: REPLAN_QUALITY_THRESHOLD) no topo do módulo, junto de outros limiares como 0.5/85.
- L452 · _baixa_ · dead-code [E] — _attempt_recovery é um método privado que não é chamado em nenhum ponto deste módulo, sugerindo código morto (recursão para execute_complex_task nunca acionada).
  → Confirmar via grep no pacote se há chamador; se não houver, remover ou wirear no caminho de erro de execute_complex_task.

**`python/packages/btv-squad/src/btv_squad/parallel.py`**
- L28 · _baixa_ · correcao [A] — Um 'max_concurrent' <= 0 nos limites produz 'asyncio.Semaphore(0)', que trava indefinidamente sem tarefas jamais iniciarem.
  → Aplicar um piso: 'max(1, int(self.limits.get("max_concurrent", 5)))' para garantir ao menos uma vaga.

**`python/packages/btv-squad/src/btv_squad/planning.py`**
- L190 · _media_ · correcao [A] — replan_from_point indexa step["step"] e failed_step["step"] diretamente, mas os steps vêm do parse do LLM (_parse_decomposition retorna parsed.get("steps", []) sem validar a chave "step").
  → Usar step.get("step") com fallback (ou normalizar os steps em _parse_decomposition atribuindo "step" sequencial) antes de comparar, evitando KeyError num replan sobre um plano cujo LLM omitiu a chave.
- L38 · _baixa_ · correcao [A] — _JSON_BLOCK usa \{.*\} com DOTALL (guloso): captura do primeiro { ao último } da resposta, podendo englobar texto/prosa entre blocos quando o modelo não obedece o "SOMENTE JSON".
  → Tolerar melhor a resposta ruidosa: tentar json.loads do texto inteiro primeiro e, no fallito, varrer candidatos balanceados em vez de um único match guloso.
- L112 · _baixa_ · correcao [A] — plan_history e failure_patterns crescem sem limite ao longo da vida do planner; failure_patterns só é incrementado, nunca lido.
  → Limitar plan_history (ex.: deque com maxlen) e remover failure_patterns se não há consumidor, ou expô-lo em analyze_failure/replan onde a contagem realmente influa na decisão.

**`python/packages/btv-squad/src/btv_squad/routing.py`**
- L18 · _baixa_ · correcao [E] — O 'LearningRouter' acumula 'route_performance' mas 'smart_route' nunca consulta esses dados, apenas retorna 'preferred_route' ou 'default'.
  → Se o roteamento adaptativo e escopo desta onda, usar as estatisticas (ex.: escolher a rota com maior success_rate quando nao ha preferred_route); caso contrario documentar explicitamente que a aprendizagem ainda nao alimenta a decisao.

**`python/packages/btv-squad/src/btv_squad/sandbox.py`**
- L52 · _baixa_ · correcao [A] — Com `auto_remove=True`, ler `container.logs()` apos `wait()` pode correr com a remocao automatica do contêiner e perder/errar os logs em caso de falha.
  → Capturar os logs antes de confiar na auto-remocao (ex.: `logs = container.logs()` uma vez apos o wait e reusar), ou usar `auto_remove=False` + remocao explicita no finally para garantir os logs de diagnostico.

**`python/packages/btv-squad/src/btv_squad/security.py`**
- L48 · _media_ · seguranca [E] — A checagem de padrões proibidos roda regex sobre str(params), o repr do dict, o que é impreciso (aspas/chaves afetam o match) e trivialmente contornável.
  → Percorrer os valores string de params recursivamente e aplicar os padrões a cada valor, em vez de casar contra o repr do dict; mantém a natureza de defesa-em-profundidade mas reduz falsos negativos/positivos.
- L18 · _baixa_ · Clean Code [E] — MAX_EXECUTION_TIME_SECONDS, MAX_MEMORY_PER_TOOL_MB, MAX_CONCURRENT_TOOLS e ALLOWED_DOMAINS são declarados mas não referenciados por validate_tool_call, sugerindo config não aplicada aqui.
  → Se esses limites são impostos em outro lugar, documentar onde; caso contrário, remover ou aplicar de fato para não dar falsa impressão de enforcement.
- L35 · _baixa_ · 12-Factor [E] — ALLOWED_DOMAINS traz domínio de produção hardcoded (api.buildtoflip.com) no código.
  → Carregar a allowlist de domínios de configuração/env em vez de literal no código-fonte.

**`python/packages/btv-squad/src/btv_squad/server.py`**
- L197 · _media_ · correcao [A] — O channel gRPC é criado na linha 197, mas o try/finally que o fecha só começa na 225; se AgentMemorySystem() ou UnifiedOrchestrator(...) (linhas 203-213) levantarem, o channel vaza sem close().
  → Mover a criação do channel para dentro do bloco try (ou envolver a construção do orquestrador no try já existente) de modo que o finally com channel.close() cubra toda a vida do channel.
- L64 · _baixa_ · correcao [A] — _to_squad_event acessa event["kind"] e chaves específicas (event["agent"], event["confidence"]...) por indexação direta; um evento malformado do orquestrador vira KeyError não tratado dentro do gerador do stream.
  → Usar event.get(...) com defaults para os campos ou envolver o mapeamento num try que emita um SquadEvent de erro, mantendo o padrão fail-closed já usado no resto do arquivo.

**`python/packages/btv-squad/tests/test_architect.py`**
- L78 · _baixa_ · manutenibilidade [A] — Teste de erro usa try/except com 'assert False' em vez de pytest.raises, padrão mais frágil e verboso.
  → Trocar por 'with pytest.raises(RuntimeError, match="attach_gateway"):' para deixar a intenção clara e evitar que um sucesso silencioso escape.

**`python/packages/btv-squad/tests/test_developer.py`**
- L44 · _baixa_ · Clean Code [A] — test_execute_sem_gateway usa try/except com `assert False` manual para verificar RuntimeError em vez de pytest.raises.
  → Usar `with pytest.raises(RuntimeError, match='attach_gateway')` para tornar a expectativa de excecao clara e evitar o padrao 'assert False' que pode ser mal sinalizado se a excecao mudar.

**`python/packages/btv-squad/tests/test_hitl.py`**
- L52 · _baixa_ · correcao [A] — O teste usa try/except com `assert False` em vez de pytest.raises para verificar o RuntimeError esperado.
  → Substituir por `with pytest.raises(RuntimeError, match="attach_permission_client"):` para clareza e para nao mascarar outras excecoes.

**`python/packages/btv-squad/tests/test_ops.py`**
- L73 · _baixa_ · Clean Code [A] — O teste usa `try/except` com `assert False` manual para verificar exceção, em vez do idioma padrão de pytest.
  → Substituir por `with pytest.raises(RuntimeError, match="attach_gateway"): asyncio.run(...)` — mais claro e sem o assert-falso frágil.

**`python/packages/btv-squad/tests/test_orchestrator.py`**
- L216 · _baixa_ · DRY [E] — Dois testes (linhas 216 e 298) reconstroem inteiros os dicts de resposta de architect/developer/auditor/designer/ops inline, duplicando o mapeamento que _gateway ja centraliza, apenas para variar planner/developer.
  → Extrair um helper que aceite overrides parciais por requester (ex.: _gateway_with(overrides)) e reusa-lo nesses testes, eliminando a repeticao dos agentes que nao variam.

**`python/packages/btv-squad/tests/test_planning.py`**
- L54 · _baixa_ · DRY [A] — O trio 'planner = AdaptivePlanner(); planner.attach_gateway(...)' se repete em ~7 testes.
  → Extrair um helper `_planner_com(*responses)` (ou fixture) que constroi o planner com um ScriptedGatewayClient, reduzindo o boilerplate repetido.
- L68 · _baixa_ · manutenibilidade [A] — Teste de erro usa try/except manual com assert False em vez de pytest.raises.
  → Trocar o bloco try/except por `with pytest.raises(RuntimeError, match="attach_gateway")` — mais idiomático e sem o risco do assert-False silencioso.

**`python/packages/btv-squad/tests/test_tenant.py`**
- L40 · _baixa_ · correcao [A] — pytest.raises(Exception) e amplo demais para provar imutabilidade — passaria por qualquer erro, nao so o de atribuicao a modelo frozen.
  → Restringir para pytest.raises(ValidationError) (pydantic) ou TypeError, o erro concreto que um BaseModel frozen levanta ao atribuir.


### `btv-store` — 7 arquivo(s), 20 achado(s)

**`crates/btv-store/src/btv.rs`**
- L243 · _media_ · correcao [E] — max_run_task_seq engole qualquer erro de storage retornando 0, o que pode fazer a proxima ativacao gerar sq1 e colidir em UNIQUE(runs.task_id).
  → Propagar o erro (retornar Result<u64,_>) ou, se a assinatura legada nao pode mudar, ao menos logar a falha de prepare/query_map antes de cair no 0, para nao mascarar uma colisao de task_id como banco vazio.
- L323 · _media_ · Big-O [A] — get_run_by_task carrega TODAS as runs em memoria e faz find linear em vez de consultar direto pelo indice UNIQUE(tenant_id, task_id).
  → Substituir por um query_row com WHERE task_id = ?1 AND tenant_id = ?2 usando row_to_run e .optional(), como ja faz o trait RunRepository::get — evita desserializar N linhas para achar uma.
- L359 · _media_ · Big-O [A] — get_deliverable materializa toda a lista de deliverables e faz find linear por id em vez de um SELECT direto por id.
  → Consultar diretamente com WHERE tenant_id = ?1 AND id = ?2 via query_row(...).optional(), espelhando get_deliverable do trait em vez de list_deliverables().into_iter().find().
- L132 · _baixa_ · correcao [A] — let _ = conn.execute(ALTER TABLE users ADD COLUMN pin_hash) descarta qualquer erro, nao apenas o de coluna duplicada, mascarando falha real do schema.
  → Checar a existencia da coluna via pragma_table_info('users') antes do ALTER, ou inspecionar o erro e reengolir somente o caso de coluna duplicada, propagando os demais.
- L591 · _baixa_ · DRY [A] — A logica de verificacao de PIN (match stored: None=>NoPin, Some=>compara hash) esta duplicada entre verify_user_pin (legado) e verify_pin (trait).
  → Extrair uma funcao livre pin_check(stored: Option<String>, created_ts, email, nome, pin) -> PinCheck e chama-la nos dois metodos.
- L941 · _baixa_ · DRY [A] — O literal strftime('%Y-%m-%dT%H:%M:%SZ','now') aparece repetido em ~7 lugares (set_override, insert_custom, update_custom, set_published, create).
  → Extrair uma const SQL_NOW (ou helper que devolve o timestamp) e reusar, evitando divergencia de formato entre os call sites.

**`crates/btv-store/src/events.rs`**
- L96 · _media_ · correcao [A] — query_row do seq usa unwrap_or(0), engolindo qualquer erro real do SQLite (db locked, corrupcao) e tratando como head=0.
  → Trocar por rusqlite OptionalExtension: `.optional()?` distingue QueryReturnedNoRows (→ 0) de erro genuino (→ propaga EventError::Storage). Mesmo padrao em head_seq (linha 168).

**`crates/btv-store/src/ledger.rs`**
- L262 · _baixa_ · DRY [A] — export_chain e recent_in_chain repetem o mesmo loop de deserializacao (from_str do body + reatribuir entry.seq = seq a partir da coluna).
  → Extrair um helper 'rows_para_entries' que recebe o query_map de (seq, body) e devolve Vec<LedgerEntry> com o seq corrigido, reutilizado pelos dois metodos.
- L460 · _baixa_ · Big-O [A] — verifica_cadeia_rows aloca uma String nova (body_tenant.to_string()) por linha so para comparar com o tenant da cadeia, no caminho quente de verificacao.
  → Parsear o parametro tenant para TenantId uma unica vez antes do loop e comparar por igualdade de TenantId, evitando uma alocacao por entrada.

**`crates/btv-store/src/pg.rs`**
- L144 · _media_ · DRY [E] — O boilerplate begin/fixa_tenant/commit da transacao esta duplicado em ~18 metodos praticamente identico.
  → Extrair um helper (ex.: fn com_tx<T>(&self, tenant, f: impl FnOnce(&mut Transaction)->Fut) -> Result<T>) que abre a tx, chama fixa_tenant, roda o corpo e comita; cada metodo passa apenas a query. Reduz superficie de erro (esquecer fixa_tenant vazaria RLS) e ruido.
- L73 · _baixa_ · correcao [A] — linha_para_run/linha_para_deliverable usam try_get posicional (0..11) acoplado a ordem exata de RUN_COLS/DELIVERABLE_COLS definidos em outro modulo.
  → Trocar os indices numericos por try_get("nome_coluna"): mudanca na ordem das colunas passa a falhar de forma explicita em vez de mapear campo errado silenciosamente.
- L127 · _baixa_ · 12-Factor [A] — max_connections(4) do pool esta hardcoded, sem configuracao por ambiente.
  → Ler o tamanho do pool de uma env var (ex.: BTV_PG_MAX_CONNECTIONS) com fallback 4, para permitir ajuste em producao SaaS sem recompilar.
- L170 · _baixa_ · Big-O [E] — list/list_deliverables fazem SELECT ... ORDER BY id DESC sem LIMIT, carregando todas as linhas do tenant em memoria.
  → Se o contrato permitir, adicionar paginacao (LIMIT/OFFSET ou keyset) para runs/deliverables; caso a trait fixe a assinatura, ao menos documentar o limite pratico. Cresce O(n) por tenant sem teto.
- L865 · _baixa_ · correcao [A] — O retry otimista do append gira sem backoff entre as ate 64 tentativas sob contencao.
  → Adicionar um pequeno backoff (ex.: yield/sleep crescente com jitter) entre iteracoes que sofrem conflito 23505, reduzindo thundering-herd de re-leituras do topo sob alta concorrencia no mesmo tenant.

**`crates/btv-store/src/prompt_library.rs`**
- L131 · _media_ · correcao [E] — `row_to_prompt` engole erros de parse de `fields`/`tags` com `unwrap_or(Value::Null)`/`unwrap_or_default()`, transformando dado corrompido em silêncio (perda invisível).
  → Propagar o erro convertendo o parse em `rusqlite::Error` (ex.: `FromSqlConversionFailure`) ou ao menos logar/telemetrar a linha corrompida, em vez de degradar silenciosamente para Null/vazio.
- L67 · _baixa_ · correcao [A] — `serde_json::to_string(tags).unwrap_or_else(|_| "[]")` grava tags vazias silenciosamente se a serialização falhar, escondendo o erro no caminho de escrita.
  → Como serializar `&[String]` não falha na prática, usar `expect` com mensagem clara ou propagar o erro em vez de mascará-lo como lista vazia.
- L76 · _baixa_ · Big-O [E] — `list(Some(tag))` carrega todas as linhas e filtra a tag em memória em Rust, sempre O(n) mesmo quando a tag é seletiva.
  → Aceitável para biblioteca local pequena; se crescer, considerar índice/tabela de tags normalizada ou filtro `LIKE` no SQL como pré-filtro. Documentar a escolha O(n).

**`crates/btv-store/src/telemetry.rs`**
- L214 · _media_ · correcao [A] — O handle Telemetry usa `.lock().expect("...poisoned")` embora o contrato documentado (linhas 208-209) prometa que falhas de telemetria nunca quebram o caminho principal.
  → Um mutex envenenado (panic de outra thread segurando o lock) fara `record`/`recent`/`summary`/etc. entrar em panic, violando a promessa. Recupere o guard com `.lock().unwrap_or_else(|e| e.into_inner())` para nunca propagar o poison ao caminho principal.
- L94 · _baixa_ · correcao [A] — `serde_json::from_str(...).unwrap_or(Value::Null)` engole silenciosamente props corrompidas transformando-as em Null.
  → Como os props sempre sao gravados via `Value::to_string`, o parse nao deveria falhar; se falhar e um sinal de corrupcao de dados. Logue em stderr no ramo de erro (como faz `record`) antes de cair para Value::Null, em vez de mascarar silenciosamente.

**`crates/btv-store/tests/contract_pg.rs`**
- L19 · _media_ · DRY [A] — O guard `if !harness::disponivel() { return; }` esta duplicado literalmente em 6 testes.
  → Extrair um helper (ex.: `fn skip_se_indisponivel() -> bool`) ou uma macro que encapsule o early-return, reduzindo a repeticao em cada teste.


### `btv-tools` — 6 arquivo(s), 14 achado(s)

**`crates/btv-tools/src/bash.rs`**
- L80 · _alta_ · correcao [A] ✓confirmado — stdout/stderr são pipes lidos só APÓS o processo sair; um comando que produz mais que a capacidade do pipe (~64KB) bloqueia na escrita, nunca sai, e o loop try_wait só termina no timeout — todo comando com saída grande falha por timeout.
  → Ler stdout e stderr concorrentemente (threads dedicadas ou async) enquanto se aguarda o processo, ou usar Command::output com controle de timeout, em vez de ler depois do wait.
- L69 · _baixa_ · correcao [E] — No timeout, child.kill() mata apenas o processo sh; comandos que criam subprocessos podem deixar netos órfãos (sem kill de grupo de processos).
  → Colocar o filho em seu próprio grupo de processos e enviar o kill ao grupo no timeout, consistente com a correção já feita no sidecar do squad.

**`crates/btv-tools/src/lib.rs`**
- L92 · _baixa_ · correcao [A] — O nome do arquivo de overflow deriva só do timestamp em nanossegundos; duas ferramentas transbordando no mesmo nanossegundo geram o mesmo nome e uma sobrescreve a outra.
  → Compor o nome com um discriminador adicional (contador atômico, PID/thread id ou sufixo aleatório) além dos nanos para garantir unicidade sob concorrência.

**`crates/btv-tools/src/lsp.rs`**
- L321 · _media_ · correcao [E] — ensure_open envia didOpen apenas uma vez por URI (set opened) e nunca emite didChange; se o arquivo mudar em disco entre consultas, o servidor mantem o texto da versao 1 e definition/references/diagnostics retornam contra conteudo obsoleto.
  → Detectar mudanca (mtime ou hash do texto lido em read_file) e emitir textDocument/didChange com nova versao antes da consulta, ou didClose+didOpen quando o conteudo divergir do ja aberto.
- L379 · _media_ · DRY [A] — O loop de retry-enquanto-indexa (match request -> retryable/Err/Ok-vazio + sleep 300ms + checagem READY_TIMEOUT) esta duplicado byte-a-byte em position_query (379-390) e symbol (401-416).
  → Extrair um helper fn request_with_index_retry(proc, method, params) -> Result<Value,String> que encapsula o loop de retry e o teto READY_TIMEOUT, e chamar dos dois pontos.
- L457 · _baixa_ · Clean Code [A] — Intervalos e prazos de espera aparecem como magic numbers inline (sleep 300ms nas linhas 389/415, sleep 200ms na 466, budget tardio from_secs(3) na 457), enquanto os demais timeouts sao consts nomeadas no topo.
  → Promover esses valores a constantes nomeadas (ex.: POLL_INTERVAL, DIAG_POLL_INTERVAL, LATE_DIAG_GRACE) junto de READY_TIMEOUT/DIAG_BUDGET para uniformidade e ajuste central.
- L671 · _baixa_ · seguranca [A] — read_msg aloca vec![0u8; len] com len vindo direto do header Content-Length do servidor, sem teto; um servidor bugado/malicioso pode induzir alocacao enorme (OOM).
  → Validar len contra um limite maximo razoavel (ex.: alguns MB) e retornar erro se exceder, antes de alocar o buffer.
- L704 · _baixa_ · correcao [E] — is_retryable_lsp_error decide re-tentativa por e.contains("-32801")/"-32802" sobre a string do erro formatada do JSON; a substring pode aparecer numa mensagem de erro nao relacionada, gerando retry indevido.
  → Propagar o codigo numerico do erro LSP (extrair error.code do objeto JSON em reader_loop) e comparar o inteiro, em vez de casar substring na mensagem.
- L743 · _baixa_ · DRY [A] — A extracao de range.start -> (line, character) com fallback (0,0) esta repetida em render_locations (743), render_symbols (777) e render_diagnostics (805).
  → Extrair fn range_start(range: Option<&Value>) -> (u64,u64) e reutilizar nos tres renderizadores.

**`crates/btv-tools/src/mcp.rs`**
- L191 · _baixa_ · Clean Code [A] — Controle de fluxo de timeout usa a string sentinela magica "__timeout__" comparada por igualdade em varios pontos (linhas 187/191/207/211/226), fragil e colidivel com uma mensagem de erro real.
  → Modelar o timeout como uma variante de erro dedicada (enum ou tipo) em vez de uma string magica, para o matching ser type-safe e nao depender do texto.

**`crates/btv-tools/src/sandbox.rs`**
- L157 · _media_ · SRP [A] — run_with concentra ping, pull de imagem, montagem de config, criação, start, espera com timeout, coleta de logs e remoção num único corpo de ~130 linhas.
  → Extrair helpers privados (ex.: build_host_config/build_config, wait_with_timeout, collect_logs) para reduzir o corpo e isolar cada responsabilidade; a interface pública permanece igual.
- L76 · _baixa_ · naming [E] — O campo `stdout` de SandboxOutput na verdade concatena stdout E stderr (LogsOptions com stdout:true, stderr:true), enganando o consumidor.
  → Renomear para `output`/`combined_output` ou separar em dois campos; documentar explicitamente que agrega os dois fluxos.
- L273 · _baixa_ · correcao [E] — Chunks de log com erro são silenciosamente descartados (`if let Ok(out) = chunk`), podendo truncar a saída sem sinal.
  → Registrar (log/trace) quando um chunk vier Err, ou anexar um marcador, em vez de perder a saída sem rastro.

**`crates/btv-tools/src/skill.rs`**
- L166 · _media_ · correcao [A] — stdout/stderr so sao lidos DEPOIS que o processo termina; uma skill que emite mais que a capacidade do pipe (~64KB) bloqueia no write, nunca sai, e o loop sempre estoura no timeout.
  → Drenar stdout e stderr concorrentemente enquanto se espera o processo (threads de leitura, ou wait_with_output com timeout em thread separada) em vez de ler apenas apos o exit.


### `btv-tui` — 1 arquivo(s), 2 achado(s)

**`crates/btv-tui/src/lib.rs`**
- L69 · _baixa_ · SRP [A] — render() tem ~106 linhas concentrando construcao das linhas do transcript, status, input e modal de permissao numa unica funcao.
  → Extrair funcoes puras auxiliares (ex.: build_transcript_lines(state) -> Vec<Line>, render_status/render_input/render_permission_modal) para reduzir o corpo e permitir teste isolado da montagem de linhas.
- L119 · _baixa_ · DRY [A] — O bloco que renderiza streaming (linhas 119-127) duplica exatamente a logica de prefixo/estilo do ramo Item::Assistant (linhas 90-98).
  → Extrair um helper `push_assistant_lines(&mut lines, text)` e chama-lo tanto no match de Item::Assistant quanto no bloco de streaming.


### `btv-verify` — 1 arquivo(s), 2 achado(s)

**`crates/btv-verify/src/vetter.rs`**
- L242 · _media_ · efficiency [A] — scan_dangerous_patterns e scan_permission_mismatch percorrem a MESMA lista de arquivos e fazem read_to_string de cada arquivo duas vezes (uma por função).
  → Ler cada arquivo uma vez num único passo e passar o conteúdo (ou um Vec<(PathBuf,String)>) para ambas as checagens, evitando I/O duplicado por arquivo da skill.
- L325 · _media_ · correcao [A] — list_skill_statuses chama vet_skill para cada subdir, e vet_skill executa os `[[verify]]` do manifesto (run_pipeline lança programas externos) — ou seja, apenas LISTAR status na tela admin pode executar comandos declarados pela skill.
  → Para o caminho de listagem/status, rodar só as checagens estáticas (dangerous patterns + permission mismatch + presença de manifesto), pulando run_pipeline; ou expor um modo `vet_skill_static` sem execução de verify_steps.


### `btv-web` — 23 arquivo(s), 40 achado(s)

**`btv-web/src/api/btv.ts`**
- L148 · _baixa_ · correcao [A] — `updateCustomPersona` e `deleteCustomPersona` interpolam `${id}` na URL sem `encodeURIComponent`, ao contrario de todos os outros segmentos (templateId, papel) que sao codificados.
  → Por consistencia e robustez, aplicar `encodeURIComponent(String(id))` nos dois pontos (linhas 148 e 156), mesmo sendo `id` numerico hoje.

**`btv-web/src/api/client.ts`**
- L42 · _baixa_ · correcao [A] — No caminho de sucesso, JSON.parse(text) lanca SyntaxError cru se o corpo 200 nao for JSON valido, escapando o contrato de sempre lancar ApiError.
  → Envolver o JSON.parse em try/catch e relançar como ApiError(`corpo invalido de ${url}`, 'invalid_json'), simetrico ao tratamento do ramo de erro.

**`btv-web/src/api/squad.ts`**
- L97 · _baixa_ · DRY [A] — postSquadMessage e emergencyStopSquad repetem o mesmo padrao fetch+headers+checagem !response.ok+throw ApiError.
  → Extrair um helper `postSemCorpo(url, body)` que faz o fetch e lanca ApiError em erro, reusado pelas duas funcoes.

**`btv-web/src/components/screens/admin/Ledger.tsx`**
- L74 · _media_ · correcao [A] — O regex que classifica ator humano inclui o nome hardcoded 'marina', um dado de demo vazando na logica de producao — qualquer ator chamado assim e marcado como humano indevidamente.
  → Remover 'marina' do regex; derivar 'humano' apenas dos kinds de gate e de marcadores genericos (human/voce/usuario), sem nomes proprios embutidos.

**`btv-web/src/components/screens/admin/Modelos.tsx`**
- L45 · _media_ · DRY [A] — As linhas do grid para fluxos (rascunhos do Designer) e para templates da galeria duplicam quase toda a estrutura de grid e o mesmo conjunto de estilos inline de célula.
  → Extrair um componente `LinhaModelo` (props: bullet color, nome, versão, categoria, origem, status pill, célula de ação) e usá-lo nos dois mapeamentos.
- L48 · _baixa_ · Clean Code [A] — Cor `#2b7a8c` do bullet do Designer é magic string, enquanto os templates usam token/`t.cor`.
  → Mover para uma constante nomeada ou usar uma CSS custom property de tema para consistência com o resto do arquivo.

**`btv-web/src/components/screens/admin/Providers.tsx`**
- L65 · _baixa_ · correcao [A] — `Math.round(l.window_secs / 60)` exibe '0 min' para janelas menores que 30s e usa o magico 60 inline.
  → Extrair uma constante SECS_PER_MIN e exibir segundos quando window_secs < 60 (ex.: mostrar em s ou usar decimal) para evitar '0 min'.

**`btv-web/src/components/screens/admin/Usuarios.tsx`**
- L39 · _media_ · correcao [A] — createUser(...).then() em 'adicionar' não tem .catch: falha de criação some silenciosamente (unhandled rejection), diferente de 'remover' que trata erro.
  → Encadear `.catch((e: Error) => setErro(e.message))` (ou similar) em createUser para dar feedback consistente ao usuário.
- L59 · _media_ · correcao [A] — verifyUserPin(...).then() em 'confirmarPin' não tem .catch: erro de rede/backend na verificação de PIN não vira feedback nem pinErro.
  → Adicionar `.catch(() => setPinErro('Não consegui verificar o PIN.'))` para diferenciar falha de verificação de PIN incorreto.
- L131 · _baixa_ · correcao [A] — setUserAtivo(...).then(recarregar) sem .catch: alternar acesso que falha no backend deixa a UI dessincronizada sem aviso.
  → Encadear .catch que reporte o erro (setErro) e/ou recarregue para reverter o toggle visual.

**`btv-web/src/components/screens/admin/comum.tsx`**
- L19 · _baixa_ · performance [A] — O objeto css (Record de estilos por tone) e recriado a cada render do Pill, embora seja constante.
  → Elevar o mapa css para uma constante de modulo fora do componente, para evitar recriacao a cada render.

**`btv-web/src/components/screens/user/Biblioteca.tsx`**
- L56 · _media_ · DRY [E] — A lista de formatos sem conversor ('png','midi') é hardcoded no frontend e duplica conhecimento que vive no backend, arriscando divergência silenciosa quando o backend ganhar/perder conversores.
  → Derivar 'em breve' de um campo vindo do backend (ex.: flag 'exportable'/'conversor_disponivel' no BtvDeliverable ou no formato do template) em vez de manter a lista literal no cliente.

**`btv-web/src/components/screens/user/Designer.tsx`**
- L51 · _baixa_ · correcao [A] — A promise de `ledgerRef.append(...)` em `registrar` usa `void ... .then()` sem `.catch`, engolindo rejeições como unhandled rejection.
  → Adicionar `.catch` ao encadeamento (ou try/catch com await) para registrar/exibir falha ao anexar no AuditLedger, em vez de deixar a rejeição escapar silenciosamente.
- L222 · _baixa_ · correcao [A] — Lista de auditoria usa `key={i}` com itens prependados, então as keys deslizam a cada novo evento.
  → Usar a hash da entrada (`l.hash`) ou `ts+hash` como key em vez do índice do array, evitando remontagem/estado incorreto ao inserir no topo.

**`btv-web/src/components/screens/user/Inicio.tsx`**
- L60 · _baixa_ · correcao [A] — ONDA_CSS[template.onda] e ONDA_LABEL[template.onda] não têm fallback: onda fora de {1,2,3} renderiza sem estilo/rótulo silenciosamente.
  → Adicionar fallback (ex.: `...(ONDA_CSS[template.onda] ?? {})` e `ONDA_LABEL[template.onda] ?? `onda ${template.onda}``) para não perder a badge se o backend emitir uma onda nova.

**`btv-web/src/components/screens/user/Minhas.tsx`**
- L86 · _media_ · Clean Code [A] — A definicao de `acao` e um encadeamento de ternarios de 4 niveis dentro do map, dificil de ler e manter.
  → Extrair uma funcao pura `acaoDaRun(r, isLive, isGate, template, dispatch, abrirRun)` que retorna {label,on} via if/else claros, reduzindo o aninhamento no corpo do render.
- L42 · _baixa_ · DRY [A] — Cores de erro ('#f7e7e3','#e0b8ad','#a54334') aparecem hardcoded tanto na caixa de erro quanto no PILL 'erro', duplicando o mesmo esquema.
  → Centralizar o esquema de cor de erro em uma constante/objeto reusado pela caixa de erro e pelo PILL.
- L66 · _baixa_ · Clean Code [A] — Calculo de `status` tambem usa ternarios aninhados (ativa/live/gate/concluida), misturando logica de dominio com JSX.
  → Extrair um helper `rotuloStatus(r, isLive, view)` para isolar o mapeamento de estados em um unico ponto testavel.

**`btv-web/src/components/screens/user/Personas.tsx`**
- L33 · _media_ · correcao [A] — `recarregar` seta `erro` no catch mas nunca o limpa em sucesso, então um erro antigo persiste no banner mesmo após recarga bem-sucedida ou troca de template.
  → Chamar `setErro(null)` no `.then(setData)` (ou antes do fetch) para que um recarregamento bem-sucedido limpe o banner de erro anterior.
- L93 · _media_ · correcao [A] — As mutações (`restoreAllPersonas`, `createCustomPersona`, `setPersonaOverride`, `restorePersona`, `updateCustomPersona`, `deleteCustomPersona`) usam `void ...then(recarregar)` sem `.catch`, engolindo falhas de escrita sem feedback ao usuário.
  → Encadear `.catch((e) => setErro(e.message))` (ou toast) nessas promessas para que uma falha de gravação seja visível, em vez de rejeição não tratada silenciosa.
- L81 · _baixa_ · clean-code [A] — Cores hex do banner de erro (`#f7e7e3`, `#e0b8ad`, `#a54334`) e das badges estão hardcoded inline em vez de tokens de tema.
  → Mover para variáveis CSS de tema para consistência com o resto do design system.

**`btv-web/src/components/screens/user/Vivo.tsx`**
- L26 · _media_ · SRP [A] — O componente Vivo (~450 linhas de JSX) acumula esteira, card de erro, gate humano, papel ativo, conclusão, cockpit/chat e feed num único componente com estilos inline extensos.
  → Decompor em subcomponentes (Esteira, GateCard, PapelAtivo, ConclusaoCard, Cockpit, Feed) recebendo props do useSquadRun, reduzindo o tamanho e facilitando teste isolado.
- L66 · _media_ · correcao [A] — Em contar(), uma falha de listDeliverables é engolida com catch → return 0, tornando erro de rede indistinguível de zero entregas e renderizando o alarmante "Concluída, mas sem artefato real".
  → Retornar null (ou lançar) no catch e só cravar artefatosDaTask=0 quando a contagem for confirmada; enquanto houver erro, não exibir a mensagem de "sem artefato" (manter null / estado neutro).
- L46 · _baixa_ · naming [A] — Timeouts mágicos 5000ms (fila) e 1400ms (refetch de entregas) estão embutidos inline sem nome.
  → Extrair constantes nomeadas (ex.: FILA_HINT_DELAY_MS = 5000, REFETCH_DELAY_MS = 1400) no topo do módulo.
- L468 · _baixa_ · correcao [A] — feed.map e chat.map usam o índice do array como key React, o que pode causar reconciliação incorreta se a lista não for estritamente append-only.
  → Usar uma chave estável derivada do item (ex.: `${f.ts}-${i}` ou um id do evento) em vez do índice puro.

**`btv-web/src/components/shell/Sidebar.tsx`**
- L6 · _baixa_ · DRY [E] — Dezenas de números mágicos de layout (fontSize 13.5, 9.5, larguras, paddings) repetidos inline em vez de tokens/constantes.
  → Extrair as constantes de tipografia/espaçamento recorrentes para tokens CSS ou constantes nomeadas compartilhadas com Topbar/GearDrawer.
- L72 · _baixa_ · correcao [A] — O botão 'Entregas' passa itemStyle(false) fixo, então nunca fica destacado como ativo mesmo quando a tela é 'biblioteca'.
  → Trocar itemStyle(false) por itemStyle(screen === 'biblioteca') para refletir o estado ativo, como os demais itens.

**`btv-web/src/components/wizard/Wizard.tsx`**
- L447 · _media_ · correcao [A] — WizardOverlay renderiza <WizardInner> sem `key` por template; o estado (answers, step, papeisOff, refs) é inicializado só na montagem e não reseta se outro template for aberto sem desmontar o wizard.
  → Passar `key={wizardTemplateId}` (ou template.id) em <WizardInner> para forçar remontagem e reinicializar o estado quando o template muda.
- L391 · _baixa_ · DRY [E] — Vários botões (Voltar, adicionar, anexar, Ativar) repetem o mesmo bloco de estilo inline (border/borderRadius/fontFamily/padding), com só cor/label variando.
  → Extrair um objeto/base de estilo de botão (como já foi feito com inputStyle) e compor variações, reduzindo a duplicação.

**`btv-web/src/designer/bases.ts`**
- L28 · _baixa_ · DRY [A] — O literal `createdBy: 'você'` está repetido em createDiagram e createEdge.
  → Extrair uma constante local (ex.: `const AUTOR = 'você'`) e reutilizar nas duas chamadas.
- L74 · _baixa_ · Clean Code [A] — Coordenadas de layout (40/80/115/190/300/160 etc.) são magic numbers espalhados por baseInicial e baseDoModelo.
  → Agrupar os passos/offsets de layout em constantes nomeadas (COL_STEP, ROW_TOP, ROW_ZIGZAG) para dar significado e facilitar ajuste.
- L94 · _baixa_ · Clean Code [A] — Variável intermediária `d` sem utilidade antes do return.
  → Retornar diretamente `montar(...)`.

**`btv-web/src/designer/btvPlugin.tsx`**
- L31 · _baixa_ · DRY [A] — Cores de marca/decisão sao repetidas como literais hex dentro de cardShape em vez de reusar BLOCO_META ou tokens centrais.
  → Extrair as cores fixas ('#14614f' brand, '#a85b3f' decision, '#d2c7ae', '#fbf8f1', '#f6ebe4') para constantes nomeadas ou derivar de BLOCO_META/tokens CSS, evitando duplicacao com as linhas 14/133.
- L82 · _baixa_ · magic-number [A] — Truncamento de label usa os numeros magicos 16/15 embutidos no JSX.
  → Extrair uma constante LABEL_MAX (=16) e derivar o slice dela para tornar a regra de truncamento explicita e ajustavel.

**`btv-web/src/designer/flow.ts`**
- L87 · _baixa_ · clean-code [A] — Ternario aninhado em tres niveis (prompt/endpoint/fonte) para montar 'detalhe' dificulta leitura e re-avalia n.properties.prompt duas vezes.
  → Extrair uma pequena funcao auxiliar detalheDoNo(n) com early-returns por caso, reutilizando o valor de prompt ja calculado.

**`btv-web/src/hooks/useAsyncAction.ts`**
- L16 · _baixa_ · correcao [A] — setState e chamado apos o await sem guarda de montagem, podendo atualizar estado de um componente ja desmontado.
  → Opcionalmente rastrear montagem (useRef/AbortController) e ignorar o setState pos-await quando o componente estiver desmontado, evitando atualizacao de estado obsoleta.

**`btv-web/src/lib/esteira.ts`**
- L62 · _media_ · SRP [A] — esteiraFromEvents concentra ~100 linhas com ordenação de itens, laço principal e múltiplos ramos aninhados (ação/gate/Error/Hitl/Consensus/Step), dificultando leitura e teste de cada regra isoladamente.
  → Extrair os manipuladores por tipo (aplicarAcao, aplicarHitl, aplicarStep) como funções puras que recebem/retornam o estado da esteira, deixando o laço como orquestrador fino.
- L82 · _baixa_ · naming [A] — A intercalação de ações usa 'afterEventIndex - 0.5' como chave de ordenação — um truque numérico não óbvio para posicionar a ação logo após o evento correspondente.
  → Documentar com uma const nomeada (ex.: EPSILON_ORDEM = 0.5) ou usar uma chave composta (índice inteiro + flag de tipo) para tornar a intenção explícita.

**`btv-web/src/state/AppContext.tsx`**
- L55 · _baixa_ · Clean Code [A] — Ternario aninhado triplo no calculo de `screen` do SET_PERSONA e dificil de ler e verificar.
  → Extrair uma funcao pura `nextScreenForPersona(persona, state)` com early-returns/if para achatar a arvore de decisao; mantem a interface do reducer intacta.

**`btv-web/src/state/SquadRunContext.tsx`**
- L116 · _media_ · correcao [A] — JSON.parse(btvRun.papeis_json || '[]') em abrirRun nao tem try/catch; papeis_json malformado lanca excecao nao tratada e quebra a reabertura do run.
  → Envolver o parse em try/catch (ou funcao helper safeParseArray) que devolve [] em caso de JSON invalido, degradando para nenhum papel ativo em vez de estourar.


### `web` — 28 arquivo(s), 39 achado(s)

**`web/src/api/client.ts`**
- L50 · _baixa_ · correcao [A] — JSON.parse de um corpo nao-vazio porem malformado lanca SyntaxError cru, escapando do contrato ApiError — mesma classe do bug ja tratado para corpo vazio.
  → Envolver JSON.parse(text) em try/catch e relançar como ApiError (ex.: code 'invalid_json'), mantendo o invariante de que fetchJson so lanca ApiError.

**`web/src/api/models.ts`**
- L12 · _baixa_ · correcao [A] — primaryModelName usa found.models.split(' · ')[0] sem tratar string vazia; para o tier 'medium'/'large' funciona, mas um models vazio retornaria string vazia em vez de fallback.
  → Retornar found.models.split(' · ')[0] || tier para garantir fallback quando models estiver vazio, evitando cabecalho de sessao em branco.

**`web/src/api/onboarding.ts`**
- L27 · _baixa_ · correcao [E] — copyToClipboard vira no-op silencioso quando navigator.clipboard nao existe (contexto nao-seguro), sem sinalizar falha ao chamador.
  → Retornar/lancar quando a API nao esta disponivel (ex.: Promise.reject com erro claro ou boolean de sucesso) para o chamador poder exibir fallback ao usuario.

**`web/src/api/squad.ts`**
- L1 · _baixa_ · DRY [E] — Este arquivo e quase identico a btv-web/src/api/squad.ts (interfaces, HANDOFF_PHASE_LABELS, todas as funcoes), duplicando o contrato do squad entre os dois frontends.
  → Considerar extrair os tipos/labels compartilhados para um pacote comum importado por ambos os apps, mantendo apenas as diferencas (ex.: parametro model) por app.
- L97 · _baixa_ · DRY [A] — postSquadMessage e emergencyStopSquad repetem o mesmo padrao fetch+headers+checagem !response.ok+throw ApiError.
  → Extrair um helper `postSemCorpo(url, body)` que faz o fetch e lanca ApiError em erro, reusado pelas duas funcoes.

**`web/src/api/telemetry.ts`**
- L16 · _baixa_ · DRY [A] — getSummary e getEvents repetem o mesmo padrao fetch + checagem de r.ok + throw + cast de r.json.
  → Extrair um helper local (ex.: fetchOk<T>(url)) que faca fetch, valide r.ok e retorne r.json() tipado, reutilizado pelas duas funcoes.

**`web/src/api/verify.ts`**
- L69 · _baixa_ · correcao [A] — O catch descarta o erro original da chamada fetch, dificultando diagnostico de falhas de rede.
  → Capturar o erro (`catch (e)`) e encadea-lo como causa no `ApiError` (ex.: passar `e` como campo/cause) para preservar o stack original.

**`web/src/components/primitives/Card.tsx`**
- L17 · _baixa_ · magic-number [A] — borderRadius (11) e padding (16) sao literais em vez de tokens de design, ao contrario das cores que usam var().
  → Expor esses valores como CSS custom properties (ex.: var(--radius), var(--space-2)) ou constantes nomeadas, coerente com o uso de var(--panel)/var(--line) ja presente.

**`web/src/components/primitives/Toast.tsx`**
- L22 · _baixa_ · correcao [E] — O setTimeout que remove o toast não é limpo; se o ToastProvider desmontar antes dos 4000ms, dispara setState em componente desmontado.
  → Guardar os ids de timeout e limpá-los em um cleanup (useEffect de unmount) ou usar AbortController; na prática o provider é raiz, mas o vazamento é real.
- L24 · _baixa_ · Clean Code [A] — Duração do toast (4000ms) é magic number embutido no push.
  → Extrair para uma constante nomeada (ex.: TOAST_DURATION_MS) no topo do módulo.

**`web/src/components/screens/admin/Ledger.tsx`**
- L163 · _baixa_ · naming [A] — Cor de fundo do `<pre>` hardcoded como `#0a0d12` em vez de um token CSS (`var(--...)`), quebrando a consistencia de tema usada no resto do arquivo.
  → Substituir `#0a0d12` por uma variavel de tema existente (ex.: `var(--panel)` ou `var(--bg)`).

**`web/src/components/screens/admin/Modelos.tsx`**
- L27 · _baixa_ · Big-O [A] — `top` é obtido ordenando uma cópia do array inteiro (`slice().sort()[0]`) — O(n log n) só para achar o máximo por `calls`.
  → Usar um `reduce` de máximo em O(n) (ex.: `usage.reduce((a,b)=>b.calls>a.calls?b:a, usage[0])`), tratando array vazio.

**`web/src/components/screens/admin/Skills.tsx`**
- L146 · _baixa_ · DRY [A] — A lista de perfis ['build','plan'] aparece hardcoded no <thead> (linhas 138-139) e novamente no map (linha 146); adicionar um perfil exige editar dois pontos.
  → Extrair uma const PROFILES = ['build','plan'] as const e derivar tanto os cabeçalhos quanto as células dela.
- L220 · _baixa_ · naming [A] — Cor de fundo do bloco do modal hardcoded como '#0a0d12' enquanto todo o resto do arquivo usa tokens CSS (var(--line), var(--faint), etc.), quebrando o tema.
  → Substituir '#0a0d12' por um token de superfície existente (ex.: var(--surface)/var(--bg-elev)) para manter consistência e suporte a tema.

**`web/src/components/screens/admin/Telemetria.tsx`**
- L22 · _baixa_ · correcao [A] — `max` pode ser 0 quando o maior count e 0, gerando divisao 0/0 = NaN no valor da ProgressBar.
  → Trocar por `const max = bars[0]?.[1] || 1` para garantir denominador nao-nulo.

**`web/src/components/screens/admin/Verify.tsx`**
- L24 · _baixa_ · clean-code [A] — Intervalo de polling `500` é magic number embutido na chamada `usePolling`.
  → Extrair para constante nomeada (ex.: `const VERIFY_POLL_MS = 500`) para clareza e ajuste centralizado.
- L125 · _baixa_ · clean-code [A] — Cor de fundo `#0a0d12` hardcoded no `<pre>` em vez de token CSS, divergindo do resto que usa `var(--...)`.
  → Substituir por uma variável de tema (ex.: `var(--code-bg)`) para consistência de theming.

**`web/src/components/screens/user/Designer/NodeView.tsx`**
- L19 · _baixa_ · DRY [A] — A expressao node.kind === 'pill' e reavaliada mais de dez vezes no mesmo componente para escolher dimensoes, raio, layout e estilo.
  → Computar const isPill = node.kind === 'pill' uma vez no topo e reutilizar em todas as ternarias subsequentes.

**`web/src/components/screens/user/Designer/Palette.tsx`**
- L13 · _baixa_ · DRY [A] — Objeto de estilo inline com varios magic numbers e recriado a cada iteracao do map, dificultando reuso e temizacao.
  → Extrair para uma classe CSS (ou constante de estilo definida fora do render) para os botoes da paleta, eliminando os numeros magicos repetidos e a realocacao por item.

**`web/src/components/screens/user/Designer/geometry.ts`**
- L29 · _baixa_ · correcao [A] — O guard `if (scale === 0)` e codigo morto: scale so seria 0 se dx e dy fossem ambos 0, caso ja tratado na linha anterior.
  → Remover o bloco `if (scale === 0) return { x: box.cx, y: box.cy }` — inalcancavel apos o guard `dx === 0 && dy === 0`.
- L68 · _baixa_ · Clean Code [A] — Numeros magicos 2.7 e 2 no calculo de labelX para deslocar a label pelo comprimento do texto.
  → Extrair constante nomeada (ex.: LABEL_CHAR_WIDTH_PX = 2.7) documentando que aproxima meia-largura media do caractere.

**`web/src/components/screens/user/Designer/reducer.ts`**
- L96 · _baixa_ · naming [A] — Números mágicos de posicionamento inicial de nó (280, 22, 60, 24) embutidos sem nome.
  → Extrair para constantes nomeadas (ex.: SPAWN_ORIGIN_X, SPAWN_STAGGER_X) junto das demais constantes de board importadas de templates.
- L109 · _baixa_ · DRY [A] — A string mágica 'task' (nó protegido/seleção de fallback) e 'developer' (seleção inicial) aparecem hardcoded em pontos distintos, acoplando o reducer a ids específicos de template.
  → Definir constantes como ROOT_NODE_ID='task' e DEFAULT_SELECTED_ID e reutilizá-las em REMOVE_NODE, initDesignerState e demais pontos.

**`web/src/components/screens/user/Permissao.tsx`**
- L46 · _baixa_ · correcao [A] — O listener global de keydown dispara em 's'/'n' independentemente do foco, podendo aprovar/negar enquanto o usuário digita em outro campo.
  → Ignorar o atalho quando o alvo do evento for um input/textarea/contenteditable (checar e.target.tagName / isContentEditable) antes de resolver a permissão.

**`web/src/components/screens/user/Prompts.tsx`**
- L36 · _baixa_ · DRY [A] — A expressao screenState.run().then((r) => setLibrary(r.library)) e repetida identica no useEffect e no onRetry (linha 128).
  → Extrair um helper local reloadScreen() que roda o run e seta a library, e usa-lo nos dois pontos.
- L261 · _baixa_ · correcao [A] — writeText().then() sem .catch() gera unhandled promise rejection se a escrita no clipboard for negada.
  → Encadear um .catch(() => toast.push('error', 'falha ao copiar')) na promise de navigator.clipboard.writeText, no mesmo padrao dos demais handlers.

**`web/src/components/screens/user/Sessao.tsx`**
- L181 · _media_ · correcao [E] — O painel CONTEXTO exibe valores estáticos fabricados ("época 2 · compaction 1×", "janela 14k/200k · 7%", barra fixa em 7%) que não vêm de estado real da sessão — viola o princípio "Nada Fake" do projeto.
  → Ligar esses números ao estado real da sessão/contexto ou, se ainda não houver fonte, rotular explicitamente como placeholder/em breve como as demais telas fazem, em vez de apresentar métricas fixas como reais.
- L34 · _baixa_ · clean-code [A] — Cor de fundo `#0a0d12` do bloco de diff está hardcoded em vez de token CSS.
  → Substituir por variável de tema para consistência com os demais `var(--...)`.

**`web/src/components/screens/user/Squad.tsx`**
- L30 · _media_ · SRP [E] — O componente Squad (~370 linhas) concentra disparo, gestao de conexao SSE, derivacao de propostas/consenso/log/HITL/chat e toda a renderizacao numa unica funcao.
  → Extraia a logica de derivacao de estado (os useMemo de proposals/consensus/executionLog/hitl/chat) para um hook `useSquadStream(events)` e quebre os blocos visuais (painel de propostas, coluna de consenso/HITL, conversa) em subcomponentes, reduzindo a superficie de uma unica funcao.
- L109 · _baixa_ · Clean Code [A] — O timeout de 5000ms para inferir o `slotHint` (capacidade 1 do pool) e um magic number embutido no JSX/efeito.
  → Extraia para uma constante nomeada no topo do modulo (ex.: `const SLOT_HINT_DELAY_MS = 5000`) documentando o motivo, facilitando ajuste e leitura.

**`web/src/components/screens/user/Sugestoes.tsx`**
- L58 · _baixa_ · naming [A] — `title={`ir para a tela relacionada`}` usa template literal sem interpolacao e texto generico igual para todos os cards.
  → Usar string simples e, idealmente, interpolar o destino (ex.: `ir para ${p.relatedScreen}`) para um tooltip informativo.

**`web/src/components/shell/PersonaToggle.tsx`**
- L9 · _baixa_ · seguranca [A] — Botoes de alternancia de perfil dentro de role="group" nao expoem estado ativo a tecnologia assistiva.
  → Adicionar aria-pressed={persona === 'user'} / aria-pressed={persona === 'admin'} aos botoes para que o estado selecionado seja anunciado, ja que o destaque e apenas visual (gradiente).
- L9 · _baixa_ · correcao [A] — Botoes sem atributo type assumem type="submit" por padrao.
  → Adicionar type="button" a ambos os botoes para evitar submit acidental caso passem a ficar dentro de um form.

**`web/src/components/shell/Shell.tsx`**
- L33 · _baixa_ · OCP [E] — A distinção surf/term depende de manter um Set manual de telas admin, fácil de desatualizar quando telas novas são adicionadas.
  → Derivar o stage a partir de metadado da própria tela (ex.: campo `stage` em SCREEN_META) em vez de um Set separado que precisa ser mantido em paralelo.

**`web/src/components/shell/ThemeSwitcher.tsx`**
- L13 · _baixa_ · seguranca [A] — Botoes de selecao de tema dentro de role="group" nao expoem qual tema esta ativo para leitores de tela.
  → Adicionar aria-pressed={active} ao botao, pois a selecao e comunicada apenas por estilo visual.
- L13 · _baixa_ · correcao [A] — Botao sem atributo type assume type="submit" por padrao.
  → Adicionar type="button" ao elemento button.

**`web/src/components/shell/Topbar.tsx`**
- L8 · _baixa_ · correcao [E] — O label 'sidecar saudavel' e derivado apenas do toggle de persona, nao de um health check real, exibindo um status potencialmente falso.
  → Ligar o rotulo a um estado real de saude do sidecar (ou trocar por texto neutro que nao afirme saude), evitando exibir 'sidecar saudavel' sem base real — coerente com a regra 'Nada Fake'.

**`web/src/hooks/usePolling.ts`**
- L30 · _baixa_ · correcao [A] — O setInterval dispara 'tick(false)' a cada intervalo sem esperar o tick anterior concluir; se 'fn' demorar mais que 'intervalMs', requisicoes se sobrepoem e podem entregar respostas fora de ordem.
  → Guardar um flag 'inFlight' (ou usar setTimeout reagendado apos o await) para evitar ticks concorrentes.

**`web/src/state/SessionContext.tsx`**
- L136 · _baixa_ · correcao [A] — resolvePermission não trata rejeição de fetchJson: se o POST /permission falhar, pending não é limpo nem lastError é setado, diferente de sendMessage.
  → Envolver em try/catch definindo lastError (e decidindo se mantém pending para retry) para dar feedback consistente ao usuário.


## 4. Arquivos revisados sem achados (prova de cobertura)

200 arquivos foram lidos e considerados limpos (ou só com convenções aceitas do projeto):

- **** (9): `brand-lint.test.ts`, `screenMeta.ts`, `sessao_durable_replay.rs`, `mcp_integration.rs`, `generators.py`, `gates.py`, `Sandbox.tsx`, `reducer.test.ts`, `Sidebar.tsx`
- **btv-cli** (6): `memory_console.rs`, `prompt_render.rs`, `rate_limit_gen.rs`, `tenant_border_sweep.rs`, `test_support.rs`, `wire_strings.rs`
- **btv-core** (6): `compaction.rs`, `agent.rs`, `compaction.rs`, `lib.rs`, `permission.rs`, `session.rs`
- **btv-domain** (9): `chat.rs`, `event.rs`, `ledger_kind.rs`, `lib.rs`, `persona.rs`, `run.rs`, `tenant.rs`, `tool.rs`, `user.rs`
- **btv-eval** (1): `__init__.py`
- **btv-llm** (6): `gateway.rs`, `chat.rs`, `lib.rs`, `model_tier.rs`, `pricing.rs`, `provider.rs`
- **btv-promptforge** (7): `__init__.py`, `hashing.py`, `server.py`, `test_generators.py`, `test_hashing.py`, `test_lint.py`, `test_server.py`
- **btv-proto** (2): `build.rs`, `lib.rs`
- **btv-proto-py** (1): `__init__.py`
- **btv-review** (5): `__init__.py`, `certification.py`, `test_gates.py`, `test_reviewers.py`, `test_score.py`
- **btv-schemas** (10): `canonical.rs`, `canonical.rs`, `experiment.rs`, `handoff.rs`, `ledger.rs`, `lib.rs`, `persona.rs`, `plan.rs`, `telemetry.rs`, `parity.rs`
- **btv-server** (12): `loadgen.rs`, `btv.rs`, `guard.rs`, `admin.rs`, `designer.rs`, `ledger.rs`, `prompts.rs`, `providers.rs`, `verify.rs`, `lsp_console.rs`, `sandbox_console.rs`, `golden_http.rs`
- **btv-sidecar** (4): `core_server.rs`, `lib.rs`, `squad_client.rs`, `python_sidecar.rs`
- **btv-squad** (29): `__init__.py`, `__init__.py`, `consensus.py`, `evaluation.py`, `gateway.py`, `hitl.py`, `memory_server.py`, `permission.py`, `recall.py`, `tenant.py`, `tool_client.py`, `verification.py`, `test_auditor.py`, `test_base_agent.py`, `test_chains.py`, `test_consensus.py`, `test_evaluation.py`, `test_gateway.py`, `test_grpc_clients.py`, `test_memory.py`, `test_memory_server.py`, `test_parallel.py`, `test_permission.py`, `test_recall.py`, `test_routing.py`, `test_sandbox.py`, `test_security.py`, `test_squad_server.py`, `test_verification.py`
- **btv-store** (9): `seed_btv.rs`, `seed_ledger.rs`, `seed_telemetry.rs`, `lib.rs`, `prompt_cache.rs`, `rule_store.rs`, `contract_sqlite.rs`, `migracao_ledger_pre_tenant.rs`, `migracao_pre_tenant.rs`
- **btv-tools** (8): `btv_lsp_fixture.rs`, `btv_mcp_fixture.rs`, `diff.rs`, `edit.rs`, `grep.rs`, `registry.rs`, `loop_com_ferramentas_reais.rs`, `lsp_integration.rs`
- **btv-verify** (6): `config.rs`, `exec.rs`, `lib.rs`, `parsers.rs`, `prompt_integrity.rs`, `schema_golden.rs`
- **btv-web** (20): `App.tsx`, `admin.ts`, `templates.ts`, `Permissoes.tsx`, `Telemetria.tsx`, `GearDrawer.tsx`, `Shell.tsx`, `Topbar.tsx`, `flow.test.ts`, `entregas.test.ts`, `entregas.ts`, `esteira.test.ts`, `nav.test.ts`, `nav.ts`, `screenComponents.tsx`, `main.tsx`, `AppContext.test.ts`, `TemplatesContext.tsx`, `useBrand.ts`, `domain.ts`
- **web** (50): `App.tsx`, `designer.ts`, `experiments.ts`, `ledger.ts`, `lsp.ts`, `mcp.ts`, `memory.ts`, `modelUsage.ts`, `permissions.ts`, `prompts.ts`, `providers.ts`, `ratelimit.ts`, `sandbox.ts`, `session.ts`, `skills.ts`, `stream.ts`, `AsyncStatus.tsx`, `Badge.tsx`, `Button.tsx`, `Gauge.tsx`, `Modal.tsx`, `ProgressBar.tsx`, `StatTile.tsx`, `Table.tsx`, `Experimentos.tsx`, `Lsp.tsx`, `Mcp.tsx`, `Memoria.tsx`, `Providers.tsx`, `RateLimits.tsx`, `Board.tsx`, `Designer.tsx`, `EdgesOverlay.tsx`, `PropertiesPanel.tsx`, `Toolbar.tsx`, `geometry.test.ts`, `templates.ts`, `Onboarding.tsx`, `AccentSwitcher.tsx`, `WindowChrome.tsx`, `useAsyncAction.test.ts`, `useAsyncAction.ts`, `nav.ts`, `screenComponents.tsx`, `screenMeta.ts`, `main.tsx`, `AppContext.tsx`, `useTheme.ts`, `themes.ts`, `domain.ts`
